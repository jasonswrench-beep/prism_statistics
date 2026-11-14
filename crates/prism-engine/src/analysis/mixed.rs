//! Linear mixed models (`mixed.lmm`, checklist B7): a continuous outcome with fixed effects (numeric covariates and
//! dummy-coded factors) and, for one grouping variable, a random intercept and optional random slopes with an
//! unstructured covariance.
//!
//! Estimation follows lme4: the random-effects covariance is σ² ΛΛᵀ with Λ lower triangular, and the REML (or ML)
//! deviance, profiled over σ² and the fixed effects, is minimized over the elements of Λ (the diagonal kept at zero or
//! above, so a variance can be exactly zero: a singular fit). Fixed effects are tested with Satterthwaite's degrees of
//! freedom, as R's lmerTest computes them, from numerical derivatives of their variances and the deviance's Hessian.

// Matrix arithmetic here indexes rows and columns together.
#![allow(clippy::needless_range_loop)]

use prismqda_stats::dist::{t_quantile, t_two_sided};
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use super::linalg::{det, invert, Mat};
use super::regression::{self, Factor};
use crate::apa;
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Diagnostic, DiagnosticState, ResultBundle, Table};

pub const METHOD_ID: &str = "mixed.lmm";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub dependent: String,
    #[serde(default)]
    pub covariates: Vec<String>,
    #[serde(default)]
    pub factors: Vec<Factor>,
    /// The grouping variable (participants, schools).
    pub group: String,
    /// Numeric covariates (already among the fixed effects) whose slopes vary by group.
    #[serde(default)]
    pub random_slopes: Vec<String>,
    /// REML (the default) or maximum likelihood (to compare fixed effects across models).
    #[serde(default = "yes")]
    pub reml: bool,
    #[serde(default = "default_level")]
    pub level: f64,
}

fn yes() -> bool {
    true
}

fn default_level() -> f64 {
    0.95
}

/// One group's data: outcome, fixed-effects rows (intercept first) and random-effects rows.
struct Group {
    y: Vec<f64>,
    x: Mat,
    z: Mat,
}

/// Λ from θ (column-wise lower triangle, as lme4 orders it).
fn lambda(theta: &[f64], q: usize) -> Mat {
    let mut l = vec![vec![0.0; q]; q];
    let mut k = 0;
    for c in 0..q {
        for r in c..q {
            l[r][c] = theta[k];
            k += 1;
        }
    }
    l
}

/// The pieces of the profiled deviance at θ: Σ log|W_j|, XᵀW⁻¹X, XᵀW⁻¹y and yᵀW⁻¹y, where W_j = I + Z_jΛΛᵀZ_jᵀ.
struct Pieces {
    logdet_w: f64,
    xtwx: Mat,
    xtwy: Vec<f64>,
    ytwy: f64,
}

fn pieces(groups: &[Group], theta: &[f64], p: usize, q: usize) -> Option<Pieces> {
    let l = lambda(theta, q);
    let mut logdet_w = 0.0;
    let mut xtwx = vec![vec![0.0; p]; p];
    let mut xtwy = vec![0.0; p];
    let mut ytwy = 0.0;
    for g in groups {
        let n = g.y.len();
        // ZΛ (n × q), M = I + ΛᵀZᵀZΛ (q × q).
        let zl: Mat = g.z.iter().map(|zr| (0..q).map(|c| (0..q).map(|r| zr[r] * l[r][c]).sum()).collect()).collect();
        let mut m = vec![vec![0.0; q]; q];
        for a in 0..q {
            for c in 0..q {
                m[a][c] = (0..n).map(|i| zl[i][a] * zl[i][c]).sum::<f64>() + if a == c { 1.0 } else { 0.0 };
            }
        }
        let dm = det(&m);
        if dm <= 0.0 {
            return None;
        }
        logdet_w += dm.ln();
        let minv = invert(&m)?;
        // W⁻¹v = v − ZΛ M⁻¹ (ZΛ)ᵀ v, by Woodbury.
        let apply = |v: &[f64]| -> Vec<f64> {
            let zt: Vec<f64> = (0..q).map(|a| (0..n).map(|i| zl[i][a] * v[i]).sum()).collect();
            let s: Vec<f64> = (0..q).map(|a| (0..q).map(|c| minv[a][c] * zt[c]).sum()).collect();
            (0..n).map(|i| v[i] - (0..q).map(|a| zl[i][a] * s[a]).sum::<f64>()).collect()
        };
        let wy = apply(&g.y);
        let wx: Vec<Vec<f64>> = (0..p).map(|j| apply(&g.x.iter().map(|r| r[j]).collect::<Vec<_>>())).collect();
        ytwy += g.y.iter().zip(&wy).map(|(a, c)| a * c).sum::<f64>();
        for a in 0..p {
            xtwy[a] += (0..n).map(|i| g.x[i][a] * wy[i]).sum::<f64>();
            for c in 0..p {
                xtwx[a][c] += (0..n).map(|i| g.x[i][a] * wx[c][i]).sum::<f64>();
            }
        }
    }
    Some(Pieces { logdet_w, xtwx, xtwy, ytwy })
}

/// The profiled deviance (−2 log-likelihood, REML or ML) at θ, with β̂ and σ̂².
fn profiled(groups: &[Group], theta: &[f64], p: usize, q: usize, nobs: usize, reml: bool) -> Option<(f64, Vec<f64>, f64, Mat)> {
    let pc = pieces(groups, theta, p, q)?;
    let inv = invert(&pc.xtwx)?;
    let beta: Vec<f64> = inv.iter().map(|r| r.iter().zip(&pc.xtwy).map(|(a, c)| a * c).sum()).collect();
    let r = pc.ytwy - beta.iter().zip(&pc.xtwy).map(|(a, c)| a * c).sum::<f64>();
    let nf = nobs as f64;
    let two_pi = 2.0 * std::f64::consts::PI;
    if reml {
        let dfr = nf - p as f64;
        let s2 = r / dfr;
        let dev = pc.logdet_w + det(&pc.xtwx).ln() + dfr * (1.0 + (two_pi * s2).ln());
        Some((dev, beta, s2, inv))
    } else {
        let s2 = r / nf;
        Some((pc.logdet_w + nf * (1.0 + (two_pi * s2).ln()), beta, s2, inv))
    }
}

/// The unprofiled deviance at (θ, σ), for Satterthwaite's derivatives.
fn deviance_full(groups: &[Group], theta: &[f64], sigma: f64, p: usize, q: usize, nobs: usize, reml: bool) -> Option<f64> {
    let pc = pieces(groups, theta, p, q)?;
    let inv = invert(&pc.xtwx)?;
    let beta: Vec<f64> = inv.iter().map(|r| r.iter().zip(&pc.xtwy).map(|(a, c)| a * c).sum()).collect();
    let r = pc.ytwy - beta.iter().zip(&pc.xtwy).map(|(a, c)| a * c).sum::<f64>();
    let s2 = sigma * sigma;
    let nf = nobs as f64;
    let two_pi = 2.0 * std::f64::consts::PI;
    let mut d = nf * (two_pi * s2).ln() + pc.logdet_w + r / s2;
    if reml {
        d += det(&pc.xtwx).ln() - p as f64 * (two_pi * s2).ln();
    }
    Some(d)
}

/// Nelder-Mead minimization.
fn nelder_mead(f: &dyn Fn(&[f64]) -> f64, x0: &[f64], step: f64) -> Vec<f64> {
    let d = x0.len();
    let mut simplex: Vec<Vec<f64>> = vec![x0.to_vec()];
    for i in 0..d {
        let mut v = x0.to_vec();
        v[i] += step;
        simplex.push(v);
    }
    let mut vals: Vec<f64> = simplex.iter().map(|v| f(v)).collect();
    for _ in 0..20000 {
        let mut order: Vec<usize> = (0..=d).collect();
        order.sort_by(|&a, &b| vals[a].total_cmp(&vals[b]));
        simplex = order.iter().map(|&i| simplex[i].clone()).collect();
        vals = order.iter().map(|&i| vals[i]).collect();
        let spread = vals[d] - vals[0];
        let size = simplex.iter().skip(1).map(|v| v.iter().zip(&simplex[0]).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max)).fold(0.0, f64::max);
        if spread.abs() < 1e-14 && size < 1e-10 {
            break;
        }
        let centroid: Vec<f64> = (0..d).map(|j| simplex[..d].iter().map(|v| v[j]).sum::<f64>() / d as f64).collect();
        let point = |t: f64| -> Vec<f64> { centroid.iter().zip(&simplex[d]).map(|(c, w)| c + t * (c - w)).collect() };
        let xr = point(1.0);
        let fr = f(&xr);
        if fr < vals[0] {
            let xe = point(2.0);
            let fe = f(&xe);
            if fe < fr {
                simplex[d] = xe;
                vals[d] = fe;
            } else {
                simplex[d] = xr;
                vals[d] = fr;
            }
        } else if fr < vals[d - 1] {
            simplex[d] = xr;
            vals[d] = fr;
        } else {
            let xc = if fr < vals[d] { point(0.5) } else { point(-0.5) };
            let fc = f(&xc);
            if fc < vals[d].min(fr) {
                simplex[d] = xc;
                vals[d] = fc;
            } else {
                for i in 1..=d {
                    simplex[i] = simplex[i].iter().zip(&simplex[0]).map(|(a, b)| b + 0.5 * (a - b)).collect();
                    vals[i] = f(&simplex[i]);
                }
            }
        }
    }
    let best = (0..=d).min_by(|&a, &b| vals[a].total_cmp(&vals[b])).unwrap_or(0);
    simplex[best].clone()
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    check_level(spec.level)?;
    let reg = regression::Spec { dependent: spec.dependent.clone(), covariates: spec.covariates.clone(), factors: spec.factors.clone(), level: spec.level, blocks: Vec::new(), interactions: Vec::new(), center: false, plots: false, robust: None, bayes: Default::default(), prior_scale: Default::default() };
    if ds.find(&spec.group) == ds.find(&spec.dependent) {
        return Err(Error::Invalid("The grouping variable can't be the outcome.".into()));
    }
    for s in &spec.random_slopes {
        if !spec.covariates.contains(s) {
            return Err(Error::Invalid(format!("{s} has a random slope, so it must also be a numeric fixed effect.")));
        }
    }
    // The fixed-effects design, then the cases that also have the grouping variable.
    let reg_spec = if spec.covariates.is_empty() && spec.factors.is_empty() { None } else { Some(&reg) };
    let (gslots, glevels) = categories(ds, &spec.group)?;
    let (y_all, cols_all, names, used, mut tally, total, y_label) = match reg_spec {
        Some(rs) => {
            let pr = regression::prepare(ds, rs)?;
            (pr.y, pr.columns, pr.names, pr.used, pr.tally, pr.total, pr.y_label)
        }
        None => {
            let ys = numeric(ds, &spec.dependent)?;
            let mut t = Tally::default();
            let used: Vec<usize> = (0..ys.len()).filter(|&r| !t.check(&ys[r], "dependent variable")).collect();
            (used.iter().map(|&r| ys[r].value().expect("checked")).collect(), Vec::new(), Vec::new(), used, t, ys.len(), ds.variable(&spec.dependent)?.display_name().to_string())
        }
    };
    let keep: Vec<usize> = (0..used.len()).filter(|&i| !tally.check(&gslots[used[i]], "grouping variable")).collect();
    let y: Vec<f64> = keep.iter().map(|&i| y_all[i]).collect();
    let cols: Vec<Vec<f64>> = cols_all.iter().map(|c| keep.iter().map(|&i| c[i]).collect()).collect();
    let gidx: Vec<usize> = keep.iter().map(|&i| gslots[used[i]].value().expect("checked")).collect();
    let nobs = y.len();
    let p = cols.len() + 1;
    let slope_cols: Vec<usize> = spec.random_slopes.iter().map(|s| names.iter().position(|n| n == ds.variable(s).map(|v| v.display_name()).unwrap_or(s)).or_else(|| names.iter().position(|n| n == s)).unwrap_or(usize::MAX)).collect();
    if slope_cols.contains(&usize::MAX) {
        return Err(Error::Invalid("A random slope's variable isn't among the fixed effects.".into()));
    }
    let q = 1 + slope_cols.len();
    let g_label = ds.variable(&spec.group)?.display_name().to_string();
    let lv = level_text(spec.level);
    let note = format!(
        "Linear mixed model fitted by {} (as R's lme4::lmer), with a random intercept{} for each {g_label}. Fixed effects are tested with Satterthwaite's degrees of freedom (as R's lmerTest); {lv} intervals use the same df. Cases were used when the outcome, every predictor and the grouping variable had valid values. Computed by PrismStatistics.",
        if spec.reml { "restricted maximum likelihood (REML)" } else { "maximum likelihood (ML)" },
        if q > 1 { format!(" and random slopes of {} (unstructured covariance)", spec.random_slopes.join(", ")) } else { String::new() }
    );
    let mut b = common::bundle(METHOD_ID, METHOD_VERSION, format!("Linear Mixed Model: {y_label}"), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, nobs), note);
    set_text(&mut b, "y.label", y_label.clone());
    set_text(&mut b, "group.label", g_label.clone());
    set(&mut b, "n", nobs as f64);
    set(&mut b, "level", spec.level);
    let mut present: Vec<usize> = gidx.clone();
    present.sort_unstable();
    present.dedup();
    let groups: Vec<Group> = present
        .iter()
        .map(|&g| {
            let rows: Vec<usize> = (0..nobs).filter(|&i| gidx[i] == g).collect();
            Group {
                y: rows.iter().map(|&i| y[i]).collect(),
                x: rows.iter().map(|&i| std::iter::once(1.0).chain(cols.iter().map(|c| c[i])).collect()).collect(),
                z: rows.iter().map(|&i| std::iter::once(1.0).chain(slope_cols.iter().map(|&c| cols[c][i])).collect()).collect(),
            }
        })
        .collect();
    set(&mut b, "groups", groups.len() as f64);
    if groups.len() < 3 || nobs <= p + 1 {
        fail(&mut b, format!("A mixed model needs at least three groups and more cases than fixed effects; there are {} groups of {g_label} and {nobs} cases.", groups.len()));
        return Ok(b);
    }
    let _ = glevels;
    // Optimize θ: Λ's diagonal is kept at zero or above.
    let nth = q * (q + 1) / 2;
    let diag: Vec<usize> = { let mut v = Vec::new(); let mut k = 0; for c in 0..q { v.push(k); k += q - c; } v };
    let fix = |t: &[f64]| -> Vec<f64> { t.iter().enumerate().map(|(i, v)| if diag.contains(&i) { v.abs() } else { *v }).collect() };
    let obj = |t: &[f64]| profiled(&groups, &fix(t), p, q, nobs, spec.reml).map_or(f64::INFINITY, |r| r.0);
    let start: Vec<f64> = (0..nth).map(|i| if diag.contains(&i) { 1.0 } else { 0.0 }).collect();
    let mut theta = nelder_mead(&obj, &start, 0.5);
    theta = nelder_mead(&obj, &theta, 0.05);
    theta = fix(&theta);
    let Some((dev, beta, s2, inv)) = profiled(&groups, &theta, p, q, nobs, spec.reml) else {
        fail(&mut b, "The model could not be fitted: the fixed effects may be collinear.");
        return Ok(b);
    };
    let sigma = s2.sqrt();
    // Random-effects covariance σ² ΛΛᵀ.
    let l = lambda(&theta, q);
    let g_cov: Mat = (0..q).map(|a| (0..q).map(|c| s2 * (0..q).map(|k| l[a][k] * l[c][k]).sum::<f64>()).collect()).collect();
    let re_names: Vec<String> = std::iter::once("(Intercept)".to_string()).chain(spec.random_slopes.iter().cloned()).collect();
    let mut rows_re = Vec::new();
    for a in 0..q {
        set(&mut b, format!("re.{}.var", a + 1), g_cov[a][a]);
        set(&mut b, format!("re.{}.sd", a + 1), g_cov[a][a].sqrt());
        set_text(&mut b, format!("re.{}.label", a + 1), re_names[a].clone());
        rows_re.push(vec![text(&g_label), text(&re_names[a]), num(g_cov[a][a], 4), num(g_cov[a][a].sqrt(), 4)]);
        for c in 0..a {
            let r = g_cov[a][c] / (g_cov[a][a] * g_cov[c][c]).sqrt();
            set(&mut b, format!("re.cor.{}.{}", c + 1, a + 1), r);
        }
    }
    rows_re.push(vec![text("Residual"), crate::result::Cell::Empty, num(s2, 4), num(sigma, 4)]);
    set(&mut b, "sigma2", s2);
    let singular = (0..q).any(|a| theta[diag[a]] < 1e-6);
    let mut re_foot = Vec::new();
    for a in 0..q {
        for c in 0..a {
            if let Some(r) = b.number(&format!("re.cor.{}.{}", c + 1, a + 1)) {
                re_foot.push(format!("Correlation of {} and {}: {}.", re_names[c], re_names[a], apa::num(r, 3)));
            }
        }
    }
    if q == 1 {
        let icc = g_cov[0][0] / (g_cov[0][0] + s2);
        set(&mut b, "icc", icc);
        re_foot.push(format!("Intraclass correlation (share of variance between groups): {}.", apa::num(icc, 3)));
    }
    b.tables.push(Table { id: "random".into(), title: "Random effects".into(), columns: vec!["Group".into(), "Effect".into(), "Variance".into(), "SD".into()], rows: rows_re, footnotes: re_foot });
    if singular {
        b.diagnostics.push(Diagnostic { id: "singular".into(), title: "Singular fit".into(), state: DiagnosticState::ReviewNeeded, message: "A random-effect variance is estimated at zero (or the effects are perfectly correlated): the data don't support this many random effects. Consider removing a random slope.".into() });
    }

    // Satterthwaite degrees of freedom from numerical derivatives in (θ, σ).
    let phi: Vec<f64> = theta.iter().copied().chain(std::iter::once(sigma)).collect();
    let np = phi.len();
    let dfun = |ph: &[f64]| deviance_full(&groups, &ph[..np - 1], ph[np - 1], p, q, nobs, spec.reml).unwrap_or(f64::NAN);
    let vfun = |ph: &[f64]| -> Option<Mat> {
        let pc = pieces(&groups, &ph[..np - 1], p, q)?;
        let s = ph[np - 1] * ph[np - 1];
        invert(&pc.xtwx).map(|m| m.into_iter().map(|r| r.into_iter().map(|v| v * s).collect()).collect())
    };
    // Richardson extrapolation over two step sizes, as numDeriv does for lmerTest.
    let hstep = |v: f64| 1e-3 * v.abs().max(1e-2);
    let second = |i: usize, j: usize, scale: f64| -> f64 {
        let (hi, hj) = (hstep(phi[i]) * scale, hstep(phi[j]) * scale);
        let at = |di: f64, dj: f64| { let mut x = phi.clone(); x[i] += di; x[j] += dj; dfun(&x) };
        if i == j { (at(hi, 0.0) - 2.0 * dfun(&phi) + at(-hi, 0.0)) / (hi * hi) } else { (at(hi, hj) - at(hi, -hj) - at(-hi, hj) + at(-hi, -hj)) / (4.0 * hi * hj) }
    };
    let mut hess = vec![vec![0.0; np]; np];
    for i in 0..np {
        for j in i..np {
            let v = (4.0 * second(i, j, 0.5) - second(i, j, 1.0)) / 3.0;
            hess[i][j] = v;
            hess[j][i] = v;
        }
    }
    let acov = invert(&hess).map(|m| m.into_iter().map(|r| r.into_iter().map(|v| 2.0 * v).collect::<Vec<f64>>()).collect::<Mat>());
    let first = |i: usize, scale: f64| -> Option<Mat> {
        let h = hstep(phi[i]) * scale;
        let mut up = phi.clone();
        up[i] += h;
        let mut dn = phi.clone();
        dn[i] -= h;
        let (vu, vd) = (vfun(&up)?, vfun(&dn)?);
        Some(vu.iter().zip(&vd).map(|(a, c)| a.iter().zip(c).map(|(x, y)| (x - y) / (2.0 * h)).collect()).collect())
    };
    let jac: Vec<Option<Mat>> = (0..np)
        .map(|i| {
            let (a, c) = (first(i, 0.5)?, first(i, 1.0)?);
            Some(a.iter().zip(&c).map(|(x, y)| x.iter().zip(y).map(|(u, v)| (4.0 * u - v) / 3.0).collect()).collect())
        })
        .collect();
    let vcov: Mat = inv.iter().map(|r| r.iter().map(|v| v * s2).collect()).collect();
    let mut rows_fe = Vec::new();
    for k in 0..p {
        let name = if k == 0 { "(Intercept)".to_string() } else { names[k - 1].clone() };
        let se = vcov[k][k].sqrt();
        let df = match (&acov, jac.iter().all(Option::is_some)) {
            (Some(a), true) => {
                let g: Vec<f64> = jac.iter().map(|j| j.as_ref().expect("checked")[k][k]).collect();
                let denom: f64 = (0..np).map(|i| (0..np).map(|j| g[i] * a[i][j] * g[j]).sum::<f64>()).sum();
                2.0 * vcov[k][k] * vcov[k][k] / denom
            }
            _ => f64::NAN,
        };
        let t = beta[k] / se;
        let pv = t_two_sided(t, df);
        let qq = t_quantile(1.0 - (1.0 - spec.level) / 2.0, df);
        let key = if k == 0 { "b0".to_string() } else { format!("term.{k}") };
        if k > 0 {
            set_text(&mut b, format!("{key}.label"), name.clone());
        }
        for (f, v) in [("b", beta[k]), ("se", se), ("df", df), ("t", t), ("p", pv), ("lower", beta[k] - qq * se), ("upper", beta[k] + qq * se)] {
            set(&mut b, format!("{key}.{f}"), v);
        }
        rows_fe.push(vec![text(name), num(beta[k], 4), num(se, 4), num(df, 2), num(t, 3), common::p(pv), num(beta[k] - qq * se, 4), num(beta[k] + qq * se, 4)]);
    }
    b.tables.push(Table { id: "fixed".into(), title: "Fixed effects".into(), columns: vec!["Term".into(), "Estimate".into(), "SE".into(), "df".into(), "t".into(), "p".into(), format!("{lv} CI lower"), format!("{lv} CI upper")], rows: rows_fe, footnotes: vec!["df: Satterthwaite's approximation.".into()] });
    let npar = (p + nth + 1) as f64;
    set(&mut b, "deviance", dev);
    set(&mut b, "aic", dev + 2.0 * npar);
    b.tables.push(Table {
        id: "fit".into(),
        title: "Model".into(),
        columns: vec!["Statistic".into(), "Value".into()],
        rows: vec![
            vec![text("Observations"), count(nobs)],
            vec![text(format!("Groups ({g_label})")), count(groups.len())],
            vec![text(if spec.reml { "REML criterion" } else { "−2 log-likelihood" }), num(dev, 3)],
            vec![text("AIC"), num(dev + 2.0 * npar, 3)],
        ],
        footnotes: if spec.reml { vec!["Compare models that differ in their fixed effects with ML, not REML.".into()] } else { Vec::new() },
    });
    Ok(b)
}
