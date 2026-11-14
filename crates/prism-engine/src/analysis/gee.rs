//! Generalized estimating equations (checklist D-2), as R's geepack (geeglm) fits them: a marginal GLM for
//! clustered or repeated observations, with a working correlation within clusters and robust (sandwich) standard
//! errors that stay valid when the working correlation is wrong.
//!
//! Families with their canonical links: Gaussian (identity), binomial (logit) and Poisson (log). Working
//! correlations: independence, exchangeable, AR(1) and unstructured. As geepack, the scale is φ = Σ r²/N (Pearson
//! residuals), the exchangeable correlation is the mean of the within-cluster products r_j r_k / φ, AR(1) is the
//! least-squares fit of α^|j−k| to those products over all pairs, and unstructured is the mean product for each pair of
//! occasions. β is updated by Fisher scoring until it stops changing. Standard errors are the sandwich
//! B⁻¹ M B⁻¹ (geepack's san.se) with model-based ("naive") ones alongside; QIC, QICu and CIC as geepack's QIC().
//! Occasions are the order of rows within a cluster, or a wave variable when one is given.

#![allow(clippy::needless_range_loop)]

use prismqda_stats::dist::{chisq_sf, norm_quantile};
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use super::linalg::{invert, Mat};
use super::regression::{self, Factor};
use crate::apa;
use crate::error::{Error, Result};
use crate::model::{Column, Dataset};
use crate::result::{Diagnostic, DiagnosticState, ResultBundle, Table};

pub const GEE: &str = "gee.model";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Family {
    #[default]
    Gaussian,
    Binomial,
    Poisson,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Corstr {
    Independence,
    #[default]
    Exchangeable,
    Ar1,
    Unstructured,
}

fn level() -> f64 {
    0.95
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub dependent: String,
    #[serde(default)]
    pub covariates: Vec<String>,
    #[serde(default)]
    pub factors: Vec<Factor>,
    /// The cluster (subject) identifier.
    pub cluster: String,
    /// The occasion of each observation (whole numbers); the row order within each cluster when left out.
    #[serde(default)]
    pub wave: Option<String>,
    #[serde(default)]
    pub family: Family,
    #[serde(default)]
    pub corstr: Corstr,
    #[serde(default = "level")]
    pub level: f64,
}

impl Family {
    fn mean(self, eta: f64) -> f64 {
        match self {
            Family::Gaussian => eta,
            Family::Binomial => 1.0 / (1.0 + (-eta).exp()),
            Family::Poisson => eta.exp(),
        }
    }
    /// The variance function, which for a canonical link is also dμ/dη.
    fn var(self, mu: f64) -> f64 {
        match self {
            Family::Gaussian => 1.0,
            Family::Binomial => mu * (1.0 - mu),
            Family::Poisson => mu,
        }
    }
    fn quasi(self, y: f64, mu: f64) -> f64 {
        match self {
            Family::Gaussian => -(y - mu).powi(2) / 2.0,
            Family::Binomial => y * (mu / (1.0 - mu)).ln() + (1.0 - mu).ln(),
            Family::Poisson => y * mu.ln() - mu,
        }
    }
}

/// One cluster: its rows' outcomes, designs (with the intercept) and occasions (0-based).
struct Cluster {
    y: Vec<f64>,
    x: Vec<Vec<f64>>,
    t: Vec<usize>,
}

/// The working correlation parameters.
#[derive(Clone)]
enum Alpha {
    None,
    Exch(f64),
    Ar1(f64),
    Unstr(Mat),
}

impl Alpha {
    fn r(&self, t: &[usize]) -> Mat {
        let n = t.len();
        (0..n)
            .map(|j| {
                (0..n)
                    .map(|k| {
                        if j == k {
                            return 1.0;
                        }
                        match self {
                            Alpha::None => 0.0,
                            Alpha::Exch(a) => *a,
                            Alpha::Ar1(a) => a.powi(t[j].abs_diff(t[k]) as i32),
                            Alpha::Unstr(m) => m[t[j].min(t[k])][t[j].max(t[k])],
                        }
                    })
                    .collect()
            })
            .collect()
    }
    fn values(&self) -> Vec<f64> {
        match self {
            Alpha::None => Vec::new(),
            Alpha::Exch(a) | Alpha::Ar1(a) => vec![*a],
            Alpha::Unstr(m) => (0..m.len()).flat_map(|j| (j + 1..m.len()).map(move |k| (j, k))).map(|(j, k)| m[j][k]).collect(),
        }
    }
}

struct Fitted {
    beta: Vec<f64>,
    robust: Mat,
    naive: Mat,
    phi: f64,
    alpha: Alpha,
    iterations: usize,
    converged: bool,
}

fn linear(x: &[f64], b: &[f64]) -> f64 {
    x.iter().zip(b).map(|(a, c)| a * c).sum()
}

/// Pearson residuals of every cluster at β.
fn pearson(cl: &[Cluster], b: &[f64], fam: Family) -> Vec<Vec<f64>> {
    cl.iter()
        .map(|c| {
            c.y.iter()
                .zip(&c.x)
                .map(|(y, x)| {
                    let mu = fam.mean(linear(x, b));
                    (y - mu) / fam.var(mu).sqrt()
                })
                .collect()
        })
        .collect()
}

fn estimate_alpha(cl: &[Cluster], r: &[Vec<f64>], phi: f64, cs: Corstr, waves: usize) -> Alpha {
    match cs {
        Corstr::Independence => Alpha::None,
        Corstr::Exchangeable => {
            let (mut s, mut n) = (0.0, 0.0);
            for ri in r {
                for j in 0..ri.len() {
                    for k in j + 1..ri.len() {
                        s += ri[j] * ri[k];
                        n += 1.0;
                    }
                }
            }
            Alpha::Exch(if n > 0.0 { s / n / phi } else { 0.0 })
        }
        Corstr::Ar1 => {
            let mut pairs: Vec<(f64, i32)> = Vec::new();
            for (c, ri) in cl.iter().zip(r) {
                for j in 0..ri.len() {
                    for k in j + 1..ri.len() {
                        pairs.push((ri[j] * ri[k] / phi, c.t[j].abs_diff(c.t[k]) as i32));
                    }
                }
            }
            // Least squares of the products on α^d: the root of Σ (e − α^d) d α^(d−1), by Newton from the lag-1 mean.
            let lag1: Vec<f64> = pairs.iter().filter(|p| p.1 == 1).map(|p| p.0).collect();
            let mut a = if lag1.is_empty() { 0.3 } else { (lag1.iter().sum::<f64>() / lag1.len() as f64).clamp(-0.95, 0.95) };
            let g = |a: f64| pairs.iter().map(|&(e, d)| (e - a.powi(d)) * d as f64 * a.powi(d - 1)).sum::<f64>();
            for _ in 0..200 {
                let h = 1e-7;
                let (f0, d) = (g(a), (g(a + h) - g(a - h)) / (2.0 * h));
                if d == 0.0 {
                    break;
                }
                let next = (a - f0 / d).clamp(-0.999_999, 0.999_999);
                if (next - a).abs() < 1e-15 {
                    a = next;
                    break;
                }
                a = next;
            }
            Alpha::Ar1(a)
        }
        Corstr::Unstructured => {
            let mut s = vec![vec![0.0; waves]; waves];
            let mut n = vec![vec![0.0; waves]; waves];
            for (c, ri) in cl.iter().zip(r) {
                for j in 0..ri.len() {
                    for k in 0..ri.len() {
                        if c.t[j] < c.t[k] {
                            s[c.t[j]][c.t[k]] += ri[j] * ri[k];
                            n[c.t[j]][c.t[k]] += 1.0;
                        }
                    }
                }
            }
            for j in 0..waves {
                for k in j + 1..waves {
                    s[j][k] = if n[j][k] > 0.0 { s[j][k] / n[j][k] / phi } else { 0.0 };
                }
            }
            Alpha::Unstr(s)
        }
    }
}

/// B = Σ D'V⁻¹D, M = Σ D'V⁻¹ e e' V⁻¹ D and the score Σ D'V⁻¹ e, with V = φ A^½ R A^½.
fn pieces(cl: &[Cluster], b: &[f64], fam: Family, phi: f64, alpha: &Alpha) -> Option<(Mat, Mat, Vec<f64>)> {
    let p = b.len();
    let (mut bb, mut mm, mut u) = (vec![vec![0.0; p]; p], vec![vec![0.0; p]; p], vec![0.0; p]);
    for c in cl {
        let n = c.y.len();
        let mu: Vec<f64> = c.x.iter().map(|x| fam.mean(linear(x, b))).collect();
        let v: Vec<f64> = mu.iter().map(|m| fam.var(*m)).collect();
        let r = alpha.r(&c.t);
        let vi: Mat = (0..n).map(|j| (0..n).map(|k| phi * v[j].sqrt() * r[j][k] * v[k].sqrt()).collect()).collect();
        let vinv = invert(&vi)?;
        // D = A X (canonical link: dμ/dη = v).
        let d: Mat = (0..n).map(|j| c.x[j].iter().map(|x| v[j] * x).collect()).collect();
        let e: Vec<f64> = (0..n).map(|j| c.y[j] - mu[j]).collect();
        // W = D'V⁻¹ (p × n).
        let w: Mat = (0..p).map(|a| (0..n).map(|k| (0..n).map(|j| d[j][a] * vinv[j][k]).sum()).collect()).collect();
        let we: Vec<f64> = (0..p).map(|a| (0..n).map(|k| w[a][k] * e[k]).sum()).collect();
        for a in 0..p {
            u[a] += we[a];
            for c2 in 0..p {
                bb[a][c2] += (0..n).map(|k| w[a][k] * d[k][c2]).sum::<f64>();
                mm[a][c2] += we[a] * we[c2];
            }
        }
    }
    Some((bb, mm, u))
}

fn fit(cl: &[Cluster], p: usize, fam: Family, cs: Corstr, waves: usize) -> Option<Fitted> {
    let nobs: usize = cl.iter().map(|c| c.y.len()).sum();
    // Start from the independence GLM.
    let mut b = vec![0.0; p];
    if fam != Family::Gaussian {
        let ybar = cl.iter().flat_map(|c| c.y.iter()).sum::<f64>() / nobs as f64;
        b[0] = match fam {
            Family::Binomial => (ybar / (1.0 - ybar)).ln(),
            _ => ybar.max(1e-8).ln(),
        };
    }
    let (mut iterations, mut converged) = (0, false);
    for stage in [Corstr::Independence, cs] {
        for it in 0..500 {
            let r = pearson(cl, &b, fam);
            let phi = r.iter().flatten().map(|z| z * z).sum::<f64>() / nobs as f64;
            let alpha = estimate_alpha(cl, &r, phi, stage, waves);
            let (bb, _, u) = pieces(cl, &b, fam, phi, &alpha)?;
            let inv = invert(&bb)?;
            let step: Vec<f64> = (0..p).map(|a| (0..p).map(|c| inv[a][c] * u[c]).sum()).collect();
            b.iter_mut().zip(&step).for_each(|(x, s)| *x += s);
            iterations = it + 1;
            if step.iter().zip(&b).all(|(s, x)| s.abs() <= 1e-12 * x.abs().max(1.0)) {
                converged = true;
                break;
            }
        }
    }
    let r = pearson(cl, &b, fam);
    let phi = r.iter().flatten().map(|z| z * z).sum::<f64>() / nobs as f64;
    let alpha = estimate_alpha(cl, &r, phi, cs, waves);
    let (bb, mm, _) = pieces(cl, &b, fam, phi, &alpha)?;
    let naive = invert(&bb)?;
    let robust: Mat = {
        let t: Mat = (0..p).map(|a| (0..p).map(|c| (0..p).map(|k| naive[a][k] * mm[k][c]).sum()).collect()).collect();
        (0..p).map(|a| (0..p).map(|c| (0..p).map(|k| t[a][k] * naive[k][c]).sum()).collect()).collect()
    };
    Some(Fitted { beta: b, robust, naive, phi, alpha, iterations, converged })
}

/// Each used row's cluster key and occasion.
fn cluster_keys(ds: &Dataset, name: &str) -> Result<Vec<Option<String>>> {
    let i = ds.find(name).ok_or_else(|| Error::UnknownVariable(name.to_string()))?;
    Ok(match &ds.columns[i] {
        Column::Text(t) => t.clone(),
        Column::Numeric(_) => numeric(ds, name)?.iter().map(|s| s.value().map(apa::plain)).collect(),
    })
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    check_level(spec.level)?;
    let reg = regression::Spec { dependent: spec.dependent.clone(), covariates: spec.covariates.clone(), factors: spec.factors.clone(), level: spec.level, blocks: Vec::new(), interactions: Vec::new(), center: false, plots: false, robust: Default::default(), bayes: Default::default(), prior_scale: Default::default() };
    let mut pr = regression::prepare(ds, &reg)?;
    let keys = cluster_keys(ds, &spec.cluster)?;
    let waves: Option<Vec<Slot<f64>>> = spec.wave.as_ref().map(|w| numeric(ds, w)).transpose()?;
    // Drop rows without a cluster (or wave).
    let keep: Vec<usize> = (0..pr.used.len())
        .filter(|&i| {
            let r = pr.used[i];
            keys[r].is_some() && waves.as_ref().is_none_or(|w| matches!(w[r], Slot::Valid(v) if v.fract() == 0.0))
        })
        .collect();
    for _ in 0..pr.used.len() - keep.len() {
        pr.tally.add(format!("no valid {}", if spec.wave.is_some() { "cluster or wave" } else { "cluster" }));
    }
    let used: Vec<usize> = keep.iter().map(|&i| pr.used[i]).collect();
    let y: Vec<f64> = keep.iter().map(|&i| pr.y[i]).collect();
    let cols: Vec<Vec<f64>> = pr.columns.iter().map(|c| keep.iter().map(|&i| c[i]).collect()).collect();
    let n = y.len();
    let p = cols.len() + 1;
    // Clusters in order of first appearance, rows in data order.
    let mut order: Vec<String> = Vec::new();
    let mut members: Vec<Vec<usize>> = Vec::new();
    for (i, &r) in used.iter().enumerate() {
        let k = keys[r].clone().expect("kept");
        match order.iter().position(|o| *o == k) {
            Some(g) => members[g].push(i),
            None => {
                order.push(k);
                members.push(vec![i]);
            }
        }
    }
    let wave_min = waves.as_ref().map(|w| used.iter().filter_map(|&r| w[r].value()).fold(f64::INFINITY, f64::min));
    let clusters: Vec<Cluster> = members
        .iter()
        .map(|m| Cluster {
            y: m.iter().map(|&i| y[i]).collect(),
            x: m.iter().map(|&i| std::iter::once(1.0).chain(cols.iter().map(|c| c[i])).collect()).collect(),
            t: m.iter().enumerate().map(|(j, &i)| waves.as_ref().map_or(j, |w| (w[used[i]].value().expect("kept") - wave_min.expect("waves")) as usize)).collect(),
        })
        .collect();
    let nwaves = clusters.iter().flat_map(|c| c.t.iter()).max().map_or(0, |m| m + 1);
    let fam = spec.family;
    let link = match fam {
        Family::Gaussian => "identity",
        Family::Binomial => "logit",
        Family::Poisson => "log",
    };
    let lv = level_text(spec.level);
    let cs_name = match spec.corstr {
        Corstr::Independence => "independence",
        Corstr::Exchangeable => "exchangeable",
        Corstr::Ar1 => "first-order autoregressive (AR(1))",
        Corstr::Unstructured => "unstructured",
    };
    let note = format!(
        "Generalized estimating equations as R's geepack (geeglm): {} family with the {link} link and a{} {cs_name} working correlation within clusters of {}; scale φ = Σr²/N from Pearson residuals; robust (sandwich) standard errors and Wald tests, with model-based standard errors alongside; QIC as geepack. Computed by PrismStatistics.",
        match fam {
            Family::Gaussian => "Gaussian",
            Family::Binomial => "binomial",
            Family::Poisson => "Poisson",
        },
        if matches!(spec.corstr, Corstr::Independence | Corstr::Unstructured) { "n" } else { "" },
        ds.variable(&spec.cluster)?.display_name()
    );
    let mut b = common::bundle(GEE, METHOD_VERSION, format!("Generalized Estimating Equations: {}", pr.y_label), serde_json::to_value(spec).expect("a spec always serializes"), pr.tally.clone().accounting(pr.total, n), note);
    set_text(&mut b, "y.label", pr.y_label.clone());
    set_text(&mut b, "family", format!("{fam:?}").to_lowercase());
    set_text(&mut b, "corstr", cs_name);
    set(&mut b, "n", n as f64);
    set(&mut b, "clusters", clusters.len() as f64);
    let sizes: Vec<usize> = clusters.iter().map(|c| c.y.len()).collect();
    set(&mut b, "size.min", *sizes.iter().min().unwrap_or(&0) as f64);
    set(&mut b, "size.max", *sizes.iter().max().unwrap_or(&0) as f64);
    set(&mut b, "size.mean", n as f64 / clusters.len().max(1) as f64);
    if clusters.len() < 2 || n <= p {
        fail(&mut b, "There are too few clusters or observations for the model.");
        return Ok(b);
    }
    match fam {
        Family::Binomial if y.iter().any(|v| *v != 0.0 && *v != 1.0) => {
            fail(&mut b, format!("{} must be coded 0 and 1 for the binomial family.", pr.y_label));
            return Ok(b);
        }
        Family::Poisson if y.iter().any(|v| *v < 0.0 || v.fract() != 0.0) => {
            fail(&mut b, format!("{} must be counts (whole numbers of zero or more) for the Poisson family.", pr.y_label));
            return Ok(b);
        }
        _ => {}
    }
    if waves.is_some() && clusters.iter().any(|c| (1..c.t.len()).any(|j| c.t[..j].contains(&c.t[j]))) {
        fail(&mut b, "A cluster has two rows for the same wave; each occasion may appear once per cluster.");
        return Ok(b);
    }
    let Some(f) = fit(&clusters, p, fam, spec.corstr, nwaves) else {
        fail(&mut b, "The estimating equations could not be solved: the predictors may be collinear or the working correlation singular.");
        return Ok(b);
    };
    let q = norm_quantile(1.0 - (1.0 - spec.level) / 2.0);
    let ratio = fam != Family::Gaussian;
    let ratio_name = if fam == Family::Binomial { "Odds ratio" } else { "Rate ratio" };
    let names: Vec<String> = std::iter::once("(Intercept)".to_string()).chain(pr.names.iter().cloned()).collect();
    let mut rows = Vec::new();
    for j in 0..p {
        let (est, se, nse) = (f.beta[j], f.robust[j][j].sqrt(), f.naive[j][j].sqrt());
        let wald = (est / se).powi(2);
        let pv = chisq_sf(wald, 1.0);
        let key = format!("coef.{}", j + 1);
        set_text(&mut b, format!("{key}.name"), names[j].clone());
        for (k, v) in [("b", est), ("se", se), ("naive_se", nse), ("wald", wald), ("p", pv), ("lower", est - q * se), ("upper", est + q * se)] {
            set(&mut b, format!("{key}.{k}"), v);
        }
        let mut row = vec![text(names[j].clone()), num(est, 4), num(se, 4), num(nse, 4), num(wald, 2), common::p(pv), text(format!("[{}, {}]", apa::num(est - q * se, 3), apa::num(est + q * se, 3)))];
        if ratio {
            set(&mut b, format!("{key}.ratio"), est.exp());
            row.push(text(if j == 0 { String::new() } else { format!("{} [{}, {}]", apa::num(est.exp(), 3), apa::num((est - q * se).exp(), 3), apa::num((est + q * se).exp(), 3)) }));
        }
        rows.push(row);
    }
    let mut cols_t = vec!["Term".to_string(), "Estimate".into(), "Robust SE".into(), "Model SE".into(), "Wald χ²(1)".into(), "p".into(), format!("{lv} CI")];
    if ratio {
        cols_t.push(format!("{ratio_name} [{lv} CI]"));
    }
    b.tables.push(Table {
        id: "coefficients".into(),
        title: "Coefficients".into(),
        columns: cols_t,
        rows,
        footnotes: vec![format!("Wald tests and {lv} intervals use the robust (sandwich) standard errors, which remain valid if the working correlation is misspecified.{}", if pr.references.is_empty() { String::new() } else { format!(" Reference categories: {}.", pr.references.join("; ")) })],
    });
    // Correlation and scale.
    set(&mut b, "phi", f.phi);
    let avals = f.alpha.values();
    for (k, a) in avals.iter().enumerate() {
        set(&mut b, format!("alpha.{}", k + 1), *a);
    }
    let mut crow = vec![vec![text("Scale (φ)"), num(f.phi, 4)]];
    match &f.alpha {
        Alpha::None => {}
        Alpha::Exch(a) => crow.push(vec![text("Within-cluster correlation (α)"), num(*a, 4)]),
        Alpha::Ar1(a) => crow.push(vec![text("Lag-1 correlation (α)"), num(*a, 4)]),
        Alpha::Unstr(m) => {
            for j in 0..m.len() {
                for k in j + 1..m.len() {
                    crow.push(vec![text(format!("Correlation, occasions {} and {}", j + 1, k + 1)), num(m[j][k], 4)]);
                }
            }
        }
    }
    // QIC, as geepack: the quasi-likelihood and the trace of the independence model's naive information times the
    // robust covariance.
    let quasi: f64 = clusters.iter().flat_map(|c| c.y.iter().zip(&c.x).map(|(y, x)| fam.quasi(*y, fam.mean(linear(x, &f.beta))))).sum();
    let indep = if spec.corstr == Corstr::Independence { Some(f.naive.clone()) } else { fit(&clusters, p, fam, Corstr::Independence, nwaves).map(|g| g.naive) };
    if let Some(ai) = indep.and_then(|m| invert(&m)) {
        let cic: f64 = (0..p).map(|a| (0..p).map(|c| ai[a][c] * f.robust[c][a]).sum::<f64>()).sum();
        let kpm = (p + avals.len()) as f64;
        let qic = -2.0 * (quasi - cic);
        let qicc = qic + 2.0 * kpm * (kpm + 1.0) / (clusters.len() as f64 - kpm - 1.0);
        for (k, v) in [("qic", qic), ("qicu", -2.0 * (quasi - p as f64)), ("quasi", quasi), ("cic", cic), ("qicc", qicc)] {
            set(&mut b, k, v);
        }
        crow.push(vec![text("QIC"), num(qic, 2)]);
        crow.push(vec![text("QICu"), num(-2.0 * (quasi - p as f64), 2)]);
        crow.push(vec![text("CIC"), num(cic, 2)]);
    }
    crow.push(vec![text("Clusters"), count(clusters.len())]);
    crow.push(vec![text("Cluster size (min–max)"), text(format!("{}–{}", sizes.iter().min().unwrap_or(&0), sizes.iter().max().unwrap_or(&0)))]);
    b.tables.push(Table { id: "fit".into(), title: "Working correlation and fit".into(), columns: vec!["Statistic".into(), "Value".into()], rows: crow, footnotes: vec!["QIC compares working correlations (lower is better); QICu compares sets of predictors.".into()] });
    if !f.converged {
        b.diagnostics.push(Diagnostic { id: "convergence".into(), title: "Convergence".into(), state: DiagnosticState::ReviewNeeded, message: format!("The estimates had not settled after {} iterations.", f.iterations) });
    }
    b.diagnostics.push(if clusters.len() < 40 {
        Diagnostic { id: "clusters".into(), title: "Number of clusters".into(), state: DiagnosticState::ReviewNeeded, message: format!("{} clusters: robust standard errors can be too small with fewer than about 40.", clusters.len()) }
    } else {
        Diagnostic { id: "clusters".into(), title: "Number of clusters".into(), state: DiagnosticState::NoConcernDetected, message: format!("{} clusters.", clusters.len()) }
    });
    Ok(b)
}
