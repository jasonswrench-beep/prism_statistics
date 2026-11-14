//! Linear panel models (checklist D-2), as R's plm fits them, for data in long form (one row per individual and
//! period):
//!
//! * pooled OLS (`pooling`);
//! * fixed effects (`within`), with individual, time or two-way effects (two-way by sweeping out the individual means
//!   and then the time dummies, which is exact for unbalanced panels too);
//! * random effects (`random`, individual effects) with Swamy-Arora variance components, as plm computes them for
//!   balanced and unbalanced panels, and the quasi-demeaning θᵢ = 1 − √(σ²ₑ / (Tᵢσ²ᵤ + σ²ₑ));
//! * the between estimator on individual means, and first differences within individuals (in period order).
//!
//! Standard errors are the usual ones, or cluster-robust by individual (plm's vcovHC with method "arellano", type
//! "HC0"). Tests: the F test for individual effects (pooled against within), Hausman's test (within against random)
//! and the Breusch-Pagan LM test for individual effects on the pooled residuals (with the unbalanced-panel form).

#![allow(clippy::needless_range_loop)]

use prismqda_stats::dist::{chisq_sf, f_sf, norm_quantile, norm_two_sided, t_quantile, t_two_sided};
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use super::linalg::{invert, Mat};
use super::regression::{self, Factor};
use crate::apa;
use crate::error::{Error, Result};
use crate::model::{Column, Dataset};
use crate::result::{Diagnostic, DiagnosticState, ResultBundle, Table};

pub const PANEL: &str = "panel.model";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Model {
    Pooling,
    #[default]
    Within,
    Random,
    Between,
    Fd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Effect {
    #[default]
    Individual,
    Time,
    Twoways,
}

fn level() -> f64 {
    0.95
}
fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub dependent: String,
    #[serde(default)]
    pub covariates: Vec<String>,
    #[serde(default)]
    pub factors: Vec<Factor>,
    /// The individual (unit) identifier.
    pub individual: String,
    /// The period identifier.
    pub time: String,
    #[serde(default)]
    pub model: Model,
    /// Fixed effects only: individual, time or both.
    #[serde(default)]
    pub effect: Effect,
    /// Cluster-robust standard errors by individual.
    #[serde(default)]
    pub robust: bool,
    /// The F test for effects, Hausman's test and the Breusch-Pagan LM test.
    #[serde(default = "yes")]
    pub tests: bool,
    #[serde(default = "level")]
    pub level: f64,
}

/// An OLS fit: coefficients, (X'X)⁻¹, residuals.
struct Ols {
    beta: Vec<f64>,
    inv: Mat,
    resid: Vec<f64>,
}

impl Ols {
    fn rss(&self) -> f64 {
        self.resid.iter().map(|r| r * r).sum()
    }
}

/// OLS of y on the columns of x (rows × p).
fn ols(y: &[f64], x: &[Vec<f64>]) -> Option<Ols> {
    let (n, p) = (y.len(), x.first().map_or(0, Vec::len));
    let mut xtx = vec![vec![0.0; p]; p];
    let mut xty = vec![0.0; p];
    for i in 0..n {
        for a in 0..p {
            xty[a] += x[i][a] * y[i];
            for c in 0..p {
                xtx[a][c] += x[i][a] * x[i][c];
            }
        }
    }
    let inv = invert(&xtx)?;
    let beta: Vec<f64> = (0..p).map(|a| (0..p).map(|c| inv[a][c] * xty[c]).sum()).collect();
    let resid = (0..n).map(|i| y[i] - (0..p).map(|a| x[i][a] * beta[a]).sum::<f64>()).collect();
    Some(Ols { beta, inv, resid })
}

/// Group means of a column, expanded back to rows.
fn means(v: &[f64], g: &[usize], ng: usize) -> Vec<f64> {
    let (mut s, mut c) = (vec![0.0; ng], vec![0.0; ng]);
    for (x, &k) in v.iter().zip(g) {
        s[k] += x;
        c[k] += 1.0;
    }
    g.iter().map(|&k| s[k] / c[k]).collect()
}

fn demean(v: &[f64], g: &[usize], ng: usize) -> Vec<f64> {
    let m = means(v, g, ng);
    v.iter().zip(&m).map(|(a, b)| a - b).collect()
}

fn col(x: &[Vec<f64>], j: usize) -> Vec<f64> {
    x.iter().map(|r| r[j]).collect()
}

fn from_cols(cols: &[Vec<f64>], n: usize) -> Vec<Vec<f64>> {
    (0..n).map(|i| cols.iter().map(|c| c[i]).collect()).collect()
}

/// Cluster-robust covariance (HC0, clusters by group): (X'X)⁻¹ Σ_g X_g'e_g e_g'X_g (X'X)⁻¹.
fn cluster_vcov(x: &[Vec<f64>], e: &[f64], inv: &Mat, g: &[usize], ng: usize) -> Mat {
    let p = inv.len();
    let mut s = vec![vec![0.0; p]; ng];
    for i in 0..x.len() {
        for a in 0..p {
            s[g[i]][a] += x[i][a] * e[i];
        }
    }
    let mut meat = vec![vec![0.0; p]; p];
    for sg in &s {
        for a in 0..p {
            for c in 0..p {
                meat[a][c] += sg[a] * sg[c];
            }
        }
    }
    let t: Mat = (0..p).map(|a| (0..p).map(|c| (0..p).map(|k| inv[a][k] * meat[k][c]).sum()).collect()).collect();
    (0..p).map(|a| (0..p).map(|c| (0..p).map(|k| t[a][k] * inv[k][c]).sum()).collect()).collect()
}

/// A fitted panel model: names of its coefficients, estimates, covariance, residual df, RSS, TSS (for R²).
struct Fitted {
    names: Vec<String>,
    beta: Vec<f64>,
    vcov: Mat,
    df: f64,
    rss: f64,
    /// R² as plm reports it: the squared correlation of the (transformed) outcome with the fitted values.
    r2: f64,
    /// Slopes (excluding the intercept) for the overall test.
    slopes: Vec<usize>,
    sigma2: Option<(f64, f64)>,
    theta: Vec<f64>,
    /// Two-way random effects: σ² of the time effects, and θ for time and in total.
    twoway: Option<(f64, f64, f64)>,
}

#[derive(Clone)]
struct Panel {
    y: Vec<f64>,
    /// Predictor columns (no intercept).
    x: Vec<Vec<f64>>,
    names: Vec<String>,
    id: Vec<usize>,
    ni: usize,
    t: Vec<usize>,
    nt: usize,
}

impl Panel {
    fn n(&self) -> usize {
        self.y.len()
    }
    fn balanced(&self) -> bool {
        let mut cnt = vec![0usize; self.ni];
        for &i in &self.id {
            cnt[i] += 1;
        }
        cnt.iter().all(|&c| c == self.nt)
    }
}

fn finish(names: Vec<String>, o: &Ols, xrows: &[Vec<f64>], y_for_tss: &[f64], df: f64, robust: Option<(&[usize], usize)>, slopes: Vec<usize>) -> Fitted {
    let rss = o.rss();
    let s2 = rss / df;
    let vcov = match robust {
        Some((g, ng)) => cluster_vcov(xrows, &o.resid, &o.inv, g, ng),
        None => o.inv.iter().map(|r| r.iter().map(|v| v * s2).collect()).collect(),
    };
    let fitted: Vec<f64> = y_for_tss.iter().zip(&o.resid).map(|(y, e)| y - e).collect();
    let n = y_for_tss.len() as f64;
    let (my, mf) = (y_for_tss.iter().sum::<f64>() / n, fitted.iter().sum::<f64>() / n);
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (y, f) in y_for_tss.iter().zip(&fitted) {
        sxy += (y - my) * (f - mf);
        sxx += (y - my).powi(2);
        syy += (f - mf).powi(2);
    }
    let r2 = sxy * sxy / (sxx * syy);
    Fitted { names, beta: o.beta.clone(), vcov, df, rss, r2, slopes, sigma2: None, theta: Vec::new(), twoway: None }
}

fn with_intercept(x: &[Vec<f64>], n: usize) -> Vec<Vec<f64>> {
    (0..n).map(|i| std::iter::once(1.0).chain(x.iter().map(|c| c[i])).collect()).collect()
}

fn fit(p: &Panel, model: Model, effect: Effect, robust: bool) -> Option<Fitted> {
    let n = p.n();
    let k = p.x.len();
    let cl = robust.then_some((p.id.as_slice(), p.ni));
    let mut names_i = vec!["(Intercept)".to_string()];
    names_i.extend(p.names.iter().cloned());
    match model {
        Model::Pooling => {
            let xr = with_intercept(&p.x, n);
            let o = ols(&p.y, &xr)?;
            Some(finish(names_i, &o, &xr, &p.y, (n - k - 1) as f64, cl, (1..=k).collect()))
        }
        Model::Within => {
            let (y, x, df) = match effect {
                Effect::Individual => (demean(&p.y, &p.id, p.ni), p.x.iter().map(|c| demean(c, &p.id, p.ni)).collect::<Vec<_>>(), (n - p.ni - k) as f64),
                Effect::Time => (demean(&p.y, &p.t, p.nt), p.x.iter().map(|c| demean(c, &p.t, p.nt)).collect::<Vec<_>>(), (n - p.nt - k) as f64),
                Effect::Twoways => {
                    // Sweep out individual means, then partial out the (individually demeaned) time dummies.
                    let yd = demean(&p.y, &p.id, p.ni);
                    let xd: Vec<Vec<f64>> = p.x.iter().map(|c| demean(c, &p.id, p.ni)).collect();
                    let dums: Vec<Vec<f64>> = (1..p.nt).map(|tt| demean(&p.t.iter().map(|&s| if s == tt { 1.0 } else { 0.0 }).collect::<Vec<_>>(), &p.id, p.ni)).collect();
                    let dr = from_cols(&dums, n);
                    let resid = |v: &[f64]| ols(v, &dr).map(|o| o.resid);
                    (resid(&yd)?, xd.iter().map(|c| resid(c)).collect::<Option<Vec<_>>>()?, (n - p.ni - p.nt + 1 - k) as f64)
                }
            };
            let xr = from_cols(&x, n);
            let o = ols(&y, &xr)?;
            Some(finish(p.names.clone(), &o, &xr, &y, df, cl, (0..k).collect()))
        }
        Model::Between => {
            let mut ys = vec![0.0; p.ni];
            let mut xs = vec![vec![0.0; k]; p.ni];
            let mut c = vec![0.0; p.ni];
            for r in 0..n {
                let g = p.id[r];
                ys[g] += p.y[r];
                c[g] += 1.0;
                for j in 0..k {
                    xs[g][j] += p.x[j][r];
                }
            }
            for g in 0..p.ni {
                ys[g] /= c[g];
                xs[g].iter_mut().for_each(|v| *v /= c[g]);
            }
            let xr: Vec<Vec<f64>> = xs.iter().map(|r| std::iter::once(1.0).chain(r.iter().copied()).collect()).collect();
            let o = ols(&ys, &xr)?;
            let ids: Vec<usize> = (0..p.ni).collect();
            Some(finish(names_i, &o, &xr, &ys, (p.ni - k - 1) as f64, robust.then_some((ids.as_slice(), p.ni)), (1..=k).collect()))
        }
        Model::Fd => {
            // Rows of each individual in period order; differences of consecutive rows.
            let mut rows: Vec<Vec<usize>> = vec![Vec::new(); p.ni];
            for r in 0..n {
                rows[p.id[r]].push(r);
            }
            let (mut y, mut xr, mut g) = (Vec::new(), Vec::new(), Vec::new());
            for (gi, rs) in rows.iter_mut().enumerate() {
                rs.sort_by_key(|&r| p.t[r]);
                for w in rs.windows(2) {
                    y.push(p.y[w[1]] - p.y[w[0]]);
                    xr.push(std::iter::once(1.0).chain((0..k).map(|j| p.x[j][w[1]] - p.x[j][w[0]])).collect::<Vec<f64>>());
                    g.push(gi);
                }
            }
            let o = ols(&y, &xr)?;
            let m = y.len();
            Some(finish(names_i, &o, &xr, &y, (m - k - 1) as f64, robust.then_some((g.as_slice(), p.ni)), (1..=k).collect()))
        }
        Model::Random => match effect {
            Effect::Twoways => {
                if !p.balanced() {
                    return None;
                }
                let (ys, xs, s2, th) = swar_twoways(p)?;
                let o = ols(&ys, &xs)?;
                let mut f = finish(names_i, &o, &xs, &ys, (n - k - 1) as f64, cl, (1..=k).collect());
                f.sigma2 = Some((s2[0], s2[1]));
                f.theta = vec![th[0]];
                f.twoway = Some((s2[2], th[1], th[2]));
                Some(f)
            }
            _ => {
                // Time effects: the same estimator with the roles of individuals and periods swapped.
                let swapped;
                let q = if effect == Effect::Time {
                    swapped = Panel { id: p.t.clone(), ni: p.nt, t: p.id.clone(), nt: p.ni, ..p.clone() };
                    &swapped
                } else {
                    p
                };
                let (ys, xs, s2e, s2u, theta) = swar_oneway(q)?;
                let o = ols(&ys, &xs)?;
                let mut f = finish(names_i, &o, &xs, &ys, (n - k - 1) as f64, cl, (1..=k).collect());
                f.sigma2 = Some((s2e, s2u));
                f.theta = theta;
                Some(f)
            }
        },
    }
}


/// One-way Swamy-Arora random effects (plm's ercomp) for the effects of `p.id`: the quasi-demeaned outcome and design
/// (intercept first), σ²ε, σ²u and θ per group.
#[allow(clippy::type_complexity)]
fn swar_oneway(p: &Panel) -> Option<(Vec<f64>, Vec<Vec<f64>>, f64, f64, Vec<f64>)> {
    let n = p.n();
    let k = p.x.len();
        let within = fit(p, Model::Within, Effect::Individual, false)?;
        let o_n = n as f64;
        let ti: Vec<f64> = {
            let mut c = vec![0.0; p.ni];
            for &g in &p.id {
                c[g] += 1.0;
            }
            c
        };
        // The "Between" regression: group means expanded to every row (so weighted by Tᵢ), with an intercept.
        let yb = means(&p.y, &p.id, p.ni);
        let xb: Vec<Vec<f64>> = p.x.iter().map(|c| means(c, &p.id, p.ni)).collect();
        let zb = with_intercept(&xb, n);
        let ob = ols(&yb, &zb)?;
        let quad1 = within.rss;
        let quad2 = ob.rss();
        let kf = k as f64;
        let nf = p.ni as f64;
        let (m_wnu, m_idnu, m_ideta) = if p.balanced() {
            (o_n - nf - kf, nf - kf - 1.0, p.nt as f64 * (nf - kf - 1.0))
        } else {
            // O − tr((Z̄'Z̄)⁻¹ Z_S'Z), with Z_S each row replaced by its group's sum.
            let z = with_intercept(&p.x, n);
            let zs: Vec<Vec<f64>> = {
                let cols: Vec<Vec<f64>> = (0..=k).map(|j| {
                    let c = col(&z, j);
                    let m = means(&c, &p.id, p.ni);
                    m.iter().zip(&p.id).map(|(v, &g)| v * ti[g]).collect()
                }).collect();
                from_cols(&cols, n)
            };
            let q = k + 1;
            let mut zsz = vec![vec![0.0; q]; q];
            for r in 0..n {
                for a in 0..q {
                    for c in 0..q {
                        zsz[a][c] += zs[r][a] * z[r][c];
                    }
                }
            }
            let tr: f64 = (0..q).map(|a| (0..q).map(|c| ob.inv[a][c] * zsz[c][a]).sum::<f64>()).sum();
            (o_n - nf - kf, nf - kf - 1.0, o_n - tr)
        };
        let s2e = quad1 / m_wnu;
        let s2u = ((quad2 - m_idnu * s2e) / m_ideta).max(0.0);
        let theta: Vec<f64> = ti.iter().map(|t| 1.0 - (s2e / (t * s2u + s2e)).sqrt()).collect();
        let th_row: Vec<f64> = p.id.iter().map(|&g| theta[g]).collect();
        let ys: Vec<f64> = (0..n).map(|r| p.y[r] - th_row[r] * yb[r]).collect();
        let xs: Vec<Vec<f64>> = (0..n).map(|r| std::iter::once(1.0 - th_row[r]).chain((0..k).map(|j| p.x[j][r] - th_row[r] * xb[j][r])).collect()).collect();
    Some((ys, xs, s2e, s2u, theta))
}

/// Two-way Swamy-Arora random effects for a balanced panel (plm's ercomp): σ²ε from the two-way within regression,
/// σ²u and σ²λ from the individual and time between regressions; y* = y − θᵢȳᵢ. − θₜȳ.ₜ + θ̃ȳ.. (and likewise for X).
#[allow(clippy::type_complexity)]
fn swar_twoways(p: &Panel) -> Option<(Vec<f64>, Vec<Vec<f64>>, [f64; 3], [f64; 3])> {
    let n = p.n();
    let k = p.x.len();
    let (nf, tf, kf, of) = (p.ni as f64, p.nt as f64, k as f64, n as f64);
    let within = fit(p, Model::Within, Effect::Twoways, false)?;
    // Between regressions (means expanded to every row), each with an intercept.
    let between_rss = |g: &[usize], ng: usize| -> Option<(f64, Vec<f64>, Vec<Vec<f64>>)> {
        let yb = means(&p.y, g, ng);
        let xb: Vec<Vec<f64>> = p.x.iter().map(|c| means(c, g, ng)).collect();
        let ob = ols(&yb, &with_intercept(&xb, n))?;
        Some((ob.rss(), yb, xb))
    };
    let (q2, yi, xi) = between_rss(&p.id, p.ni)?;
    let (q3, yt, xt) = between_rss(&p.t, p.nt)?;
    let s2e = within.rss / (of - nf - tf + 1.0 - kf);
    let s2u = ((q2 - (nf - kf - 1.0) * s2e) / (tf * (nf - kf - 1.0))).max(0.0);
    let s2l = ((q3 - (tf - kf - 1.0) * s2e) / (nf * (tf - kf - 1.0))).max(0.0);
    let th_i = 1.0 - (1.0 + tf * s2u / s2e).powf(-0.5);
    let th_t = 1.0 - (1.0 + nf * s2l / s2e).powf(-0.5);
    let th = if s2u == 0.0 || s2l == 0.0 { 0.0 } else { th_i + th_t - 1.0 + (1.0 + nf * s2l / s2e + tf * s2u / s2e).powf(-0.5) };
    let ybar = p.y.iter().sum::<f64>() / of;
    let xbar: Vec<f64> = p.x.iter().map(|c| c.iter().sum::<f64>() / of).collect();
    let ys: Vec<f64> = (0..n).map(|r| p.y[r] - th_i * yi[r] - th_t * yt[r] + th * ybar).collect();
    let xs: Vec<Vec<f64>> = (0..n).map(|r| std::iter::once(1.0 - th_i - th_t + th).chain((0..k).map(|j| p.x[j][r] - th_i * xi[j][r] - th_t * xt[j][r] + th * xbar[j])).collect()).collect();
    Some((ys, xs, [s2e, s2u, s2l], [th_i, th_t, th]))
}

/// Codes 0.. for an identifier column (texts or numbers), in sorted order.
fn codes(ds: &Dataset, name: &str) -> Result<Vec<Option<usize>>> {
    let i = ds.find(name).ok_or_else(|| Error::UnknownVariable(name.to_string()))?;
    Ok(match &ds.columns[i] {
        Column::Text(t) => {
            let mut keys: Vec<&String> = t.iter().flatten().collect();
            keys.sort();
            keys.dedup();
            t.iter().map(|c| c.as_ref().map(|v| keys.binary_search(&v).expect("listed"))).collect()
        }
        Column::Numeric(_) => {
            let cells = numeric(ds, name)?;
            let mut vals: Vec<f64> = cells.iter().filter_map(Slot::value).collect();
            vals.sort_by(f64::total_cmp);
            vals.dedup();
            cells.iter().map(|c| c.value().map(|v| vals.binary_search_by(|k| k.total_cmp(&v)).expect("listed"))).collect()
        }
    })
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    check_level(spec.level)?;
    let reg = regression::Spec { dependent: spec.dependent.clone(), covariates: spec.covariates.clone(), factors: spec.factors.clone(), level: spec.level, blocks: Vec::new(), interactions: Vec::new(), center: false, plots: false, robust: Default::default(), bayes: Default::default(), prior_scale: Default::default() };
    let mut pr = regression::prepare(ds, &reg)?;
    let (ids, ts) = (codes(ds, &spec.individual)?, codes(ds, &spec.time)?);
    let keep: Vec<usize> = (0..pr.used.len()).filter(|&i| ids[pr.used[i]].is_some() && ts[pr.used[i]].is_some()).collect();
    for _ in 0..pr.used.len() - keep.len() {
        pr.tally.add("no individual or period");
    }
    let used: Vec<usize> = keep.iter().map(|&i| pr.used[i]).collect();
    // Renumber individuals and periods among the rows used.
    let renumber = |raw: Vec<usize>| -> (Vec<usize>, usize) {
        let mut u = raw.clone();
        u.sort_unstable();
        u.dedup();
        (raw.iter().map(|v| u.binary_search(v).expect("listed")).collect(), u.len())
    };
    let (id, ni) = renumber(used.iter().map(|&r| ids[r].expect("kept")).collect());
    let (t, nt) = renumber(used.iter().map(|&r| ts[r].expect("kept")).collect());
    let panel = Panel { y: keep.iter().map(|&i| pr.y[i]).collect(), x: pr.columns.iter().map(|c| keep.iter().map(|&i| c[i]).collect()).collect(), names: pr.names.clone(), id, ni, t, nt };
    let n = panel.n();
    let lv = level_text(spec.level);
    let (mname, title) = match spec.model {
        Model::Pooling => ("pooled OLS", "Pooled OLS"),
        Model::Within => ("fixed effects (within)", "Fixed Effects"),
        Model::Random => ("random effects (Swamy-Arora)", "Random Effects"),
        Model::Between => ("between", "Between Estimator"),
        Model::Fd => ("first differences", "First Differences"),
    };
    let eff = match (spec.model, spec.effect) {
        (Model::Within, Effect::Time) => "time",
        (Model::Within | Model::Random, Effect::Twoways) => "two-way (individual and time)",
        (Model::Random, Effect::Time) => "time",
        _ => "individual",
    };
    let note = format!(
        "Linear panel model as R's plm: {mname}{}; {} standard errors. Computed by PrismStatistics.",
        if matches!(spec.model, Model::Within | Model::Random) { format!(" with {eff} effects") } else { String::new() },
        if spec.robust { "cluster-robust (by individual; plm's vcovHC, Arellano, HC0)" } else { "conventional" }
    );
    let mut b = common::bundle(PANEL, METHOD_VERSION, format!("Panel Model ({title}): {}", pr.y_label), serde_json::to_value(spec).expect("a spec always serializes"), pr.tally.clone().accounting(pr.total, n), note);
    set_text(&mut b, "y.label", pr.y_label.clone());
    set_text(&mut b, "model", mname);
    set_text(&mut b, "effect", eff);
    set(&mut b, "n", n as f64);
    set(&mut b, "individuals", ni as f64);
    set(&mut b, "periods", nt as f64);
    set(&mut b, "balanced", if panel.balanced() { 1.0 } else { 0.0 });
    if panel.x.is_empty() {
        fail(&mut b, "Add at least one predictor.");
        return Ok(b);
    }
    if ni < 2 || n <= ni + panel.x.len() {
        fail(&mut b, "There are too few individuals or observations for the model.");
        return Ok(b);
    }
    // Two rows for the same individual and period make the panel ill-defined.
    let mut seen = std::collections::HashSet::new();
    if (0..n).any(|r| !seen.insert((panel.id[r], panel.t[r]))) {
        fail(&mut b, "An individual has two rows for the same period.");
        return Ok(b);
    }
    if spec.model == Model::Random && spec.effect == Effect::Twoways && !panel.balanced() {
        fail(&mut b, "Two-way random effects need a balanced panel (every individual observed in every period).");
        return Ok(b);
    }
    let Some(f) = fit(&panel, spec.model, spec.effect, spec.robust) else {
        fail(&mut b, "The model could not be estimated: a predictor may not vary within individuals (fixed effects) or the predictors are collinear.");
        return Ok(b);
    };
    // As plm: z tests for random effects, t tests otherwise.
    let normal = spec.model == Model::Random;
    let q = if normal { norm_quantile(1.0 - (1.0 - spec.level) / 2.0) } else { t_quantile(1.0 - (1.0 - spec.level) / 2.0, f.df) };
    let mut rows = Vec::new();
    for j in 0..f.beta.len() {
        let (est, se) = (f.beta[j], f.vcov[j][j].sqrt());
        let tv = est / se;
        let pv = if normal { norm_two_sided(tv) } else { t_two_sided(tv, f.df) };
        let key = format!("coef.{}", j + 1);
        set_text(&mut b, format!("{key}.name"), f.names[j].clone());
        for (kk, v) in [("b", est), ("se", se), ("t", tv), ("p", pv), ("lower", est - q * se), ("upper", est + q * se)] {
            set(&mut b, format!("{key}.{kk}"), v);
        }
        rows.push(vec![text(f.names[j].clone()), num(est, 4), num(se, 4), num(tv, 3), common::p(pv), text(format!("[{}, {}]", apa::num(est - q * se, 4), apa::num(est + q * se, 4)))]);
    }
    let r2 = f.r2;
    // As plm: the observations the model was fitted to (all of them for fixed effects, the means for between, the
    // differences for first differences).
    let nobs = match spec.model {
        Model::Between => ni as f64,
        Model::Within => n as f64,
        _ => f.df + f.beta.len() as f64,
    };
    let adj = 1.0 - (1.0 - r2) * (nobs - 1.0) / f.df;
    for (kk, v) in [("rss", f.rss), ("r2", r2), ("adj_r2", adj), ("df", f.df)] {
        set(&mut b, kk, v);
    }
    // The overall test of the slopes: F for every model but random effects, which plm tests by Wald χ².
    let s = &f.slopes;
    let mut fit_foot = Vec::new();
    if !s.is_empty() {
        let sub: Mat = s.iter().map(|&a| s.iter().map(|&c| f.vcov[a][c]).collect()).collect();
        if let Some(si) = invert(&sub) {
            let bs: Vec<f64> = s.iter().map(|&a| f.beta[a]).collect();
            let w: f64 = (0..s.len()).map(|a| bs[a] * (0..s.len()).map(|c| si[a][c] * bs[c]).sum::<f64>()).sum();
            if spec.model == Model::Random {
                set(&mut b, "chisq", w);
                set(&mut b, "chisq.p", chisq_sf(w, s.len() as f64));
                fit_foot.push(format!("Wald χ²({}) = {}, p {}.", s.len(), apa::num(w, 2), apa::p_clause(chisq_sf(w, s.len() as f64))));
            } else {
                let fv = w / s.len() as f64;
                // As plm, a robust F test takes its denominator df from the clusters (individuals − 1).
                let df2 = if spec.robust { (ni - 1) as f64 } else { f.df };
                set(&mut b, "f", fv);
                set(&mut b, "f.p", f_sf(fv, s.len() as f64, df2));
                fit_foot.push(format!("F({}, {}) = {}, p {}{}.", s.len(), apa::num(df2, 0), apa::num(fv, 2), apa::p_clause(f_sf(fv, s.len() as f64, df2)), if spec.robust { " (robust Wald)" } else { "" }));
            }
        }
    }
    b.tables.push(Table {
        id: "coefficients".into(),
        title: "Coefficients".into(),
        columns: vec!["Term".into(), "Estimate".into(), if spec.robust { "Robust SE".into() } else { "SE".into() }, if normal { "z".into() } else { "t".into() }, "p".into(), format!("{lv} CI")],
        rows,
        footnotes: {
            let mut v = vec![format!("R² = {}, adjusted R² = {}{}.", apa::no_leading_zero(r2, 3), apa::no_leading_zero(adj, 3), if spec.model == Model::Within { " (within)" } else { "" })];
            v.extend(fit_foot);
            if !pr.references.is_empty() {
                v.push(format!("Reference categories: {}.", pr.references.join("; ")));
            }
            v
        },
    });
    let mut info = vec![vec![text("Observations"), count(n)], vec![text("Individuals"), count(ni)], vec![text("Periods"), count(nt)], vec![text("Balanced"), text(if panel.balanced() { "yes" } else { "no" })]];
    if let Some((s2e, s2u)) = f.sigma2 {
        set(&mut b, "sigma2.idios", s2e);
        set(&mut b, "sigma2.id", s2u);
        set(&mut b, "share.id", s2u / (s2u + s2e));
        let (lo, hi) = f.theta.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, c), v| (a.min(*v), c.max(*v)));
        set(&mut b, "theta.min", lo);
        set(&mut b, "theta.max", hi);
        info.push(vec![text("σ² idiosyncratic"), num(s2e, 4)]);
        let who = if spec.effect == Effect::Time { "period" } else { "individual" };
        let whom = if spec.effect == Effect::Time { "periods" } else { "individuals" };
        info.push(vec![text(format!("σ² {who}")), num(s2u, 4)]);
        if let Some((s2l, th_t, th)) = f.twoway {
            set(&mut b, "sigma2.time", s2l);
            set(&mut b, "theta.time", th_t);
            set(&mut b, "theta.total", th);
            info.push(vec![text("σ² period"), num(s2l, 4)]);
            info.push(vec![text("Share of variance between individuals"), num(s2u / (s2u + s2l + s2e), 3)]);
            info.push(vec![text("Share of variance between periods"), num(s2l / (s2u + s2l + s2e), 3)]);
            info.push(vec![text("θ (individual, period, total)"), text(format!("{}, {}, {}", apa::num(lo, 4), apa::num(th_t, 4), apa::num(th, 4)))]);
        } else {
            info.push(vec![text(format!("Share of variance between {whom}")), num(s2u / (s2u + s2e), 3)]);
            info.push(vec![text("θ"), text(if (hi - lo).abs() < 1e-12 { apa::num(lo, 4) } else { format!("{} to {}", apa::num(lo, 4), apa::num(hi, 4)) })]);
        }
    }
    b.tables.push(Table { id: "panel".into(), title: "Panel structure".into(), columns: vec!["".into(), "Value".into()], rows: info, footnotes: Vec::new() });

    if spec.tests {
        let mut trows = Vec::new();
        let pool = fit(&panel, Model::Pooling, Effect::Individual, false);
        let within = fit(&panel, Model::Within, Effect::Individual, false);
        if let (Some(pl), Some(w)) = (&pool, &within) {
            let (df1, df2) = (pl.df - w.df, w.df);
            let fv = ((pl.rss - w.rss) / df1) / (w.rss / df2);
            let pv = f_sf(fv, df1, df2);
            for (kk, v) in [("ftest.f", fv), ("ftest.df1", df1), ("ftest.df2", df2), ("ftest.p", pv)] {
                set(&mut b, kk, v);
            }
            trows.push(vec![text("F test for individual effects (fixed effects vs. pooled)"), text(format!("F({}, {}) = {}", apa::num(df1, 0), apa::num(df2, 0), apa::num(fv, 2))), common::p(pv)]);
            // Breusch-Pagan LM test on the pooled residuals (unbalanced form).
            let pr_resid: Vec<f64> = {
                let xr = with_intercept(&panel.x, n);
                ols(&panel.y, &xr).map(|o| o.resid).unwrap_or_default()
            };
            if !pr_resid.is_empty() {
                let mut sums = vec![0.0; ni];
                let mut ti = vec![0.0; ni];
                for r in 0..n {
                    sums[panel.id[r]] += pr_resid[r];
                    ti[panel.id[r]] += 1.0;
                }
                let ee: f64 = pr_resid.iter().map(|e| e * e).sum();
                let s2: f64 = sums.iter().map(|s| s * s).sum();
                let on = n as f64;
                let t2: f64 = ti.iter().map(|t| t * t).sum();
                let lm1 = (on * on / (2.0 * (t2 - on))).sqrt() * (s2 / ee - 1.0);
                let lm = lm1 * lm1;
                let pv = chisq_sf(lm, 1.0);
                set(&mut b, "bp.chisq", lm);
                set(&mut b, "bp.p", pv);
                trows.push(vec![text("Breusch-Pagan LM test for individual effects (random effects vs. pooled)"), text(format!("χ²(1) = {}", apa::num(lm, 2))), common::p(pv)]);
            }
        }
        if let (Some(w), Some(re)) = (&within, fit(&panel, Model::Random, Effect::Individual, false)) {
            // Hausman: the slopes both models estimate.
            let k = panel.x.len();
            let d: Vec<f64> = (0..k).map(|j| w.beta[j] - re.beta[j + 1]).collect();
            let vd: Mat = (0..k).map(|a| (0..k).map(|c| w.vcov[a][c] - re.vcov[a + 1][c + 1]).collect()).collect();
            if let Some(vi) = invert(&vd) {
                let h: f64 = (0..k).map(|a| d[a] * (0..k).map(|c| vi[a][c] * d[c]).sum::<f64>()).sum();
                let pv = chisq_sf(h, k as f64);
                set(&mut b, "hausman.chisq", h);
                set(&mut b, "hausman.df", k as f64);
                set(&mut b, "hausman.p", pv);
                trows.push(vec![text("Hausman test (fixed vs. random effects)"), text(format!("χ²({k}) = {}", apa::num(h, 2))), common::p(pv)]);
                b.diagnostics.push(if pv < 0.05 {
                    Diagnostic { id: "hausman".into(), title: "Fixed or random effects".into(), state: DiagnosticState::ReviewNeeded, message: "Hausman's test rejects the random-effects assumption that the individual effects are unrelated to the predictors; fixed effects are the safer estimates.".into() }
                } else {
                    Diagnostic { id: "hausman".into(), title: "Fixed or random effects".into(), state: DiagnosticState::NoConcernDetected, message: "Hausman's test does not reject random effects, which are then the more efficient estimates.".into() }
                });
            }
        }
        b.tables.push(Table { id: "tests".into(), title: "Specification tests".into(), columns: vec!["Test".into(), "Statistic".into(), "p".into()], rows: trows, footnotes: vec!["These tests use conventional (not robust) standard errors.".into()] });
    }
    if !spec.robust && spec.model != Model::Between {
        b.diagnostics.push(Diagnostic { id: "robust".into(), title: "Serial correlation".into(), state: DiagnosticState::ReviewNeeded, message: "Errors are often correlated within individuals over time; cluster-robust standard errors guard against that.".into() });
    }
    b.diagnostics.push(common::size_diagnostic(ni, format!("{ni} individuals")));
    Ok(b)
}
