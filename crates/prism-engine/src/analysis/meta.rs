//! Meta-analysis (checklist D-1), as R's metafor computes it (escalc and rma.uni).
//!
//! Effect sizes come from the data (one row per study) or are computed: Hedges' g (bias-corrected standardized mean
//! difference, large-sample variance), the raw mean difference, the log odds ratio, log risk ratio and risk difference
//! from event counts (½ added to every cell of a study with a zero cell, as metafor's default), and Fisher's z from
//! correlations.
//!
//! Models: the fixed-effect (equal-effects) model, or random effects with τ² by REML (the default), ML,
//! DerSimonian-Laird, Paule-Mandel, Hedges or Sidik-Jonkman, optionally with moderators (meta-regression) and the
//! Knapp-Hartung adjustment. Heterogeneity: Cochran's Q (fixed-effect weights), I² and H² from τ² and the typical
//! within-study variance, and Q-profile confidence intervals for τ², τ, I² and H². Prediction intervals for the
//! intercept-only model. Small-study effects: Egger's regression test (the standard error as a moderator, fitted
//! as the model was) and trim and fill (L0 estimator, side chosen as metafor chooses it). Leave-one-out analysis.
//!
//! Multilevel (three-level) models for several effects per cluster, as rma.mv(yi, vi, random = ~ 1 | cluster/effect):
//! REML or ML variance components between and within clusters, I² for each level (Cheung's), and likelihood-ratio
//! tests of each component against zero.

#![allow(clippy::needless_range_loop)]

use prismqda_stats::dist::{chisq_quantile, chisq_sf, f_sf, norm_quantile, norm_two_sided, t_quantile, t_two_sided};
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use super::linalg::{invert, Mat};
use crate::apa;
use crate::chart::{self, ForestRow};
use crate::error::{Error, Result};
use crate::model::{Column, Dataset, Measure as Level};
use crate::result::{Diagnostic, DiagnosticState, ResultBundle, Table};

pub const META: &str = "meta.analysis";
pub const METHOD_VERSION: u32 = 1;

/// How each study's effect size is obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum EffectMeasure {
    /// An effect size and its sampling variance (or standard error) given in the data.
    #[default]
    Generic,
    /// Hedges' g from two groups' means, standard deviations and sizes.
    Smd,
    /// The raw mean difference.
    Md,
    /// The log odds ratio from events and totals in two groups.
    Or,
    /// The log risk ratio.
    Rr,
    /// The risk difference.
    Rd,
    /// Fisher's z from a correlation and the sample size.
    Zcor,
}

/// The estimator of the between-study variance τ² (or the fixed-effect model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    Fe,
    #[default]
    Reml,
    Ml,
    Dl,
    Pm,
    He,
    Sj,
    /// Bayesian random effects (bayesmeta).
    Bayes,
}

fn half() -> f64 {
    0.5
}

fn level() -> f64 {
    0.95
}
fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    #[serde(default)]
    pub measure: EffectMeasure,
    /// Generic: the effect size and its variance or standard error.
    #[serde(default)]
    pub effect: Option<String>,
    #[serde(default)]
    pub variance: Option<String>,
    #[serde(default)]
    pub se: Option<String>,
    /// Two groups' means, standard deviations and sizes (smd, md).
    #[serde(default)]
    pub mean1: Option<String>,
    #[serde(default)]
    pub sd1: Option<String>,
    #[serde(default)]
    pub n1: Option<String>,
    #[serde(default)]
    pub mean2: Option<String>,
    #[serde(default)]
    pub sd2: Option<String>,
    #[serde(default)]
    pub n2: Option<String>,
    /// Two groups' events and totals (or, rr, rd); the totals are n1 and n2.
    #[serde(default)]
    pub events1: Option<String>,
    #[serde(default)]
    pub events2: Option<String>,
    /// A correlation and its sample size (zcor); the sample size is n1.
    #[serde(default)]
    pub r: Option<String>,
    /// Study labels.
    #[serde(default)]
    pub study: Option<String>,
    /// Moderators (meta-regression): numeric, or nominal (coded against the first category).
    #[serde(default)]
    pub moderators: Vec<String>,
    #[serde(default)]
    pub method: Method,
    /// The Knapp-Hartung adjustment: t and F tests with a rescaled covariance.
    #[serde(default)]
    pub knha: bool,
    #[serde(default = "level")]
    pub level: f64,
    /// Egger's test and trim and fill (intercept-only models).
    #[serde(default = "yes")]
    pub bias: bool,
    #[serde(default)]
    pub leave_one_out: bool,
    #[serde(default = "yes")]
    pub forest: bool,
    #[serde(default = "yes")]
    pub funnel: bool,
    /// Multilevel model: the cluster (such as the study or sample) each effect belongs to.
    #[serde(default)]
    pub cluster: Option<String>,
    /// Bayesian model: the prior for τ, its scale, and an optional normal prior for μ.
    #[serde(default)]
    pub tau_prior: super::bmeta::TauPrior,
    #[serde(default = "half")]
    pub tau_prior_scale: f64,
    #[serde(default)]
    pub mu_prior_mean: Option<f64>,
    #[serde(default)]
    pub mu_prior_sd: Option<f64>,
}

impl Default for Spec {
    fn default() -> Self {
        serde_json::from_value(serde_json::json!({})).expect("every field has a default")
    }
}

/// A fitted model.
#[derive(Clone)]
pub struct Fit {
    pub beta: Vec<f64>,
    pub vb: Mat,
    pub tau2: f64,
    pub tau2_se: f64,
    /// The weights used: 1/(v + τ²).
    pub w: Vec<f64>,
    /// Cochran's Q with fixed-effect weights, and its df.
    pub qe: f64,
    pub df: f64,
    pub i2: f64,
    pub h2: f64,
    /// The typical within-study variance (for I² and H²).
    pub vt: f64,
    /// The Knapp-Hartung factor s² (1 without it).
    pub s2: f64,
    pub ll: f64,
}

/// Weighted least squares: (β, (X'WX)⁻¹, Σ w r²).
fn wls(y: &[f64], w: &[f64], x: &Mat) -> Option<(Vec<f64>, Mat, f64)> {
    let (k, p) = (y.len(), x[0].len());
    let mut xtwx = vec![vec![0.0; p]; p];
    let mut xtwy = vec![0.0; p];
    for i in 0..k {
        for a in 0..p {
            xtwy[a] += x[i][a] * w[i] * y[i];
            for b in 0..p {
                xtwx[a][b] += x[i][a] * w[i] * x[i][b];
            }
        }
    }
    let inv = invert(&xtwx)?;
    let beta: Vec<f64> = (0..p).map(|a| (0..p).map(|b| inv[a][b] * xtwy[b]).sum()).collect();
    let rss = (0..k).map(|i| w[i] * (y[i] - (0..p).map(|a| x[i][a] * beta[a]).sum::<f64>()).powi(2)).sum();
    Some((beta, inv, rss))
}

/// P = W − W X (X'WX)⁻¹ X' W.
fn p_matrix(w: &[f64], x: &Mat, inv: &Mat) -> Mat {
    let (k, p) = (w.len(), x[0].len());
    (0..k)
        .map(|i| {
            (0..k)
                .map(|j| {
                    let h: f64 = (0..p).map(|a| (0..p).map(|b| x[i][a] * inv[a][b] * x[j][b]).sum::<f64>()).sum();
                    (if i == j { w[i] } else { 0.0 }) - w[i] * h * w[j]
                })
                .collect()
        })
        .collect()
}

fn trace(m: &Mat) -> f64 {
    (0..m.len()).map(|i| m[i][i]).sum()
}

/// The generalized Q statistic at τ²: Σ w (y − ŷ)² with w = 1/(v + τ²).
fn q_gen(y: &[f64], v: &[f64], x: &Mat, tau2: f64) -> Option<f64> {
    let w: Vec<f64> = v.iter().map(|vi| 1.0 / (vi + tau2)).collect();
    wls(y, &w, x).map(|r| r.2)
}

/// The τ² at which a decreasing function of τ² crosses zero, by bracketing and bisection.
fn root(f: impl Fn(f64) -> f64) -> f64 {
    if f(0.0) <= 0.0 {
        return 0.0;
    }
    let mut hi = 1.0;
    let mut n = 0;
    while f(hi) > 0.0 && n < 200 {
        hi *= 2.0;
        n += 1;
    }
    let mut lo = 0.0;
    for _ in 0..300 {
        let mid = 0.5 * (lo + hi);
        if f(mid) > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
        if hi - lo <= 1e-15 * hi.max(1e-300) {
            break;
        }
    }
    0.5 * (lo + hi)
}

/// Fits the model.
pub fn fit(y: &[f64], v: &[f64], x: &Mat, method: Method, knha: bool) -> Option<Fit> {
    let k = y.len();
    let p = x[0].len();
    if k <= p {
        return None;
    }
    let kp = (k - p) as f64;
    let w0: Vec<f64> = v.iter().map(|vi| 1.0 / vi).collect();
    let (_, inv0, qe) = wls(y, &w0, x)?;
    let p0 = p_matrix(&w0, x, &inv0);
    let tr_p0 = trace(&p0);
    let tau2 = match method {
        Method::Fe => 0.0,
        Method::Dl => ((qe - kp) / tr_p0).max(0.0),
        Method::He => {
            let ones = vec![1.0; k];
            let (_, inv, rss) = wls(y, &ones, x)?;
            let pm = p_matrix(&ones, x, &inv);
            let tr_pv: f64 = (0..k).map(|i| pm[i][i] * v[i]).sum();
            ((rss - tr_pv) / kp).max(0.0)
        }
        Method::Sj => {
            let mean = y.iter().sum::<f64>() / k as f64;
            let t0 = y.iter().map(|yi| (yi - mean).powi(2)).sum::<f64>() / k as f64;
            t0 * q_gen(y, v, x, t0)? / kp
        }
        Method::Pm => root(|t| q_gen(y, v, x, t).unwrap_or(f64::NAN) - kp),
        // (The Bayesian model never reaches here; REML stands in.)
        Method::Ml | Method::Reml | Method::Bayes => {
            // Fisher scoring from the DerSimonian-Laird value, to full precision.
            let mut t = ((qe - kp) / tr_p0).max(0.0);
            for _ in 0..2000 {
                let w: Vec<f64> = v.iter().map(|vi| 1.0 / (vi + t)).collect();
                let (beta, inv, _) = wls(y, &w, x)?;
                let r: Vec<f64> = (0..k).map(|i| y[i] - (0..p).map(|a| x[i][a] * beta[a]).sum::<f64>()).collect();
                let (score, info) = if method == Method::Ml {
                    (-0.5 * w.iter().sum::<f64>() + 0.5 * (0..k).map(|i| w[i] * w[i] * r[i] * r[i]).sum::<f64>(), 0.5 * w.iter().map(|x| x * x).sum::<f64>())
                } else {
                    let pm = p_matrix(&w, x, &inv);
                    let py: Vec<f64> = (0..k).map(|i| w[i] * r[i]).collect();
                    (-0.5 * trace(&pm) + 0.5 * py.iter().map(|z| z * z).sum::<f64>(), 0.5 * pm.iter().flatten().map(|z| z * z).sum::<f64>())
                };
                let next = (t + score / info).max(0.0);
                let done = (next - t).abs() <= 1e-14 * t.max(1e-10);
                t = next;
                if done {
                    break;
                }
            }
            t
        }
    };
    let w: Vec<f64> = v.iter().map(|vi| 1.0 / (vi + tau2)).collect();
    let (beta, inv, rss) = wls(y, &w, x)?;
    let pm = p_matrix(&w, x, &inv);
    let s2 = if knha { rss / kp } else { 1.0 };
    let vb: Mat = inv.iter().map(|r| r.iter().map(|z| z * s2).collect()).collect();
    let sum_pp: f64 = pm.iter().flatten().map(|z| z * z).sum();
    let tau2_se = match method {
        Method::Fe => f64::NAN,
        Method::Ml => (2.0 / w.iter().map(|z| z * z).sum::<f64>()).sqrt(),
        Method::Reml => (2.0 / sum_pp).sqrt(),
        Method::Dl => {
            let s0: f64 = p0.iter().flatten().map(|z| z * z).sum();
            (1.0 / (tr_p0 * tr_p0) * (2.0 * kp + 4.0 * tau2 * tr_p0 + 2.0 * tau2 * tau2 * s0)).sqrt()
        }
        Method::Pm => (2.0 * (k * k) as f64 / kp / w.iter().sum::<f64>().powi(2)).sqrt(),
        Method::He | Method::Sj | Method::Bayes => f64::NAN,
    };
    let vt = kp / tr_p0;
    let (i2, h2) = if method == Method::Fe { ((100.0 * (qe - kp) / qe).max(0.0), qe / kp) } else { (100.0 * tau2 / (vt + tau2), (vt + tau2) / vt) };
    let ln2pi = std::f64::consts::TAU.ln();
    let sum_log: f64 = v.iter().map(|vi| (vi + tau2).ln()).sum();
    let ll = if method == Method::Reml {
        let xtx: Mat = (0..p).map(|a| (0..p).map(|b| (0..k).map(|i| x[i][a] * x[i][b]).sum()).collect()).collect();
        let xtwx = invert(&inv)?;
        -0.5 * kp * ln2pi + 0.5 * log_det(&xtx)? - 0.5 * sum_log - 0.5 * log_det(&xtwx)? - 0.5 * rss
    } else {
        -0.5 * k as f64 * ln2pi - 0.5 * sum_log - 0.5 * rss
    };
    Some(Fit { beta, vb, tau2, tau2_se, w, qe, df: kp, i2, h2, vt, s2, ll })
}

/// The three-level model's variance components (between clusters, within clusters), their I², and its size.
#[derive(Clone, Debug)]
pub struct Multilevel {
    pub sigma2: [f64; 2],
    pub i2: [f64; 2],
    pub clusters: usize,
}

/// Restricted (or full) log-likelihood and its pieces for the three-level model at (σ²₁, σ²₂): V = diag(v + σ²₂) +
/// σ²₁ (1 within a cluster). Returns (ll, V⁻¹, β, (X'V⁻¹X)⁻¹).
fn ml3_eval(y: &[f64], v: &[f64], x: &Mat, members: &[Vec<usize>], s: [f64; 2], reml: bool) -> Option<(f64, Mat, Vec<f64>, Mat)> {
    let (k, p) = (y.len(), x[0].len());
    let mut vinv = vec![vec![0.0; k]; k];
    let mut log_det_v = 0.0;
    for m in members {
        let d: Vec<f64> = m.iter().map(|&i| v[i] + s[1]).collect();
        let sum_inv: f64 = d.iter().map(|z| 1.0 / z).sum();
        let c = s[0] / (1.0 + s[0] * sum_inv);
        log_det_v += d.iter().map(|z| z.ln()).sum::<f64>() + (1.0 + s[0] * sum_inv).ln();
        for (a, &i) in m.iter().enumerate() {
            for (bb, &j) in m.iter().enumerate() {
                vinv[i][j] = if a == bb { 1.0 / d[a] } else { 0.0 } - c / (d[a] * d[bb]);
            }
        }
    }
    // V⁻¹X, X'V⁻¹X.
    let vx: Mat = (0..k).map(|i| (0..p).map(|a| members_row(&vinv, i, x, a)).collect()).collect();
    let mut xvx = vec![vec![0.0; p]; p];
    let mut xvy = vec![0.0; p];
    for i in 0..k {
        for a in 0..p {
            xvy[a] += vx[i][a] * y[i];
            for bb in 0..p {
                xvx[a][bb] += x[i][a] * vx[i][bb];
            }
        }
    }
    let inv = invert(&xvx)?;
    let beta: Vec<f64> = (0..p).map(|a| (0..p).map(|bb| inv[a][bb] * xvy[bb]).sum()).collect();
    let r: Vec<f64> = (0..k).map(|i| y[i] - (0..p).map(|a| x[i][a] * beta[a]).sum::<f64>()).collect();
    let rvr: f64 = (0..k).map(|i| r[i] * (0..k).map(|j| vinv[i][j] * r[j]).sum::<f64>()).sum();
    let ln2pi = std::f64::consts::TAU.ln();
    let ll = if reml {
        let xtx: Mat = (0..p).map(|a| (0..p).map(|bb| (0..k).map(|i| x[i][a] * x[i][bb]).sum()).collect()).collect();
        -0.5 * ((k - p) as f64 * ln2pi + log_det_v + log_det(&xvx)? - log_det(&xtx)? + rvr)
    } else {
        -0.5 * (k as f64 * ln2pi + log_det_v + rvr)
    };
    Some((ll, vinv, beta, inv))
}

fn members_row(vinv: &Mat, i: usize, x: &Mat, a: usize) -> f64 {
    vinv[i].iter().zip(x).map(|(w, xr)| w * xr[a]).sum()
}

/// Fits the three-level model (random effects of clusters and of effects within clusters, as metafor's
/// rma.mv(yi, vi, random = ~ 1 | cluster/effect)) by REML or ML, by Fisher scoring with the components kept at zero or
/// above. `fixed` holds components fixed at zero (for likelihood-ratio tests).
pub fn fit_multilevel(y: &[f64], v: &[f64], x: &Mat, cluster: &[usize], method: Method, fixed: [bool; 2]) -> Option<(Fit, Multilevel)> {
    let (k, p) = (y.len(), x[0].len());
    if k <= p {
        return None;
    }
    let reml = method != Method::Ml;
    let nc = cluster.iter().max().map_or(0, |m| m + 1);
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); nc];
    for (i, &c) in cluster.iter().enumerate() {
        members[c].push(i);
    }
    members.retain(|m| !m.is_empty());
    let kp = (k - p) as f64;
    let w0: Vec<f64> = v.iter().map(|vi| 1.0 / vi).collect();
    let (_, inv0, qe) = wls(y, &w0, x)?;
    let tr_p0 = trace(&p_matrix(&w0, x, &inv0));
    let start = ((qe - kp) / tr_p0).max(0.0).max(1e-4 * v.iter().sum::<f64>() / k as f64);
    let mut s = [if fixed[0] { 0.0 } else { start / 2.0 }, if fixed[1] { 0.0 } else { start / 2.0 }];
    let mut cur = ml3_eval(y, v, x, &members, s, reml)?;
    for _ in 0..500 {
        let (_, vinv, beta, inv) = &cur;
        // P (REML) or V⁻¹ (ML), and the score and information for (σ²₁, σ²₂).
        let pm: Mat = if reml {
            let vx: Mat = (0..k).map(|i| (0..p).map(|a| members_row(vinv, i, x, a)).collect()).collect();
            let h: Mat = (0..k).map(|i| (0..p).map(|a| (0..p).map(|bb| vx[i][bb] * inv[bb][a]).sum()).collect()).collect();
            (0..k).map(|i| (0..k).map(|j| vinv[i][j] - (0..p).map(|a| h[i][a] * vx[j][a]).sum::<f64>()).collect()).collect()
        } else {
            vinv.clone()
        };
        let r: Vec<f64> = if reml { y.to_vec() } else { (0..k).map(|i| y[i] - (0..p).map(|a| x[i][a] * beta[a]).sum::<f64>()).collect() };
        let pr: Vec<f64> = (0..k).map(|i| (0..k).map(|j| pm[i][j] * r[j]).sum()).collect();
        // A = P B₁ (B₁ = 1 within clusters).
        let mut a1 = vec![vec![0.0; k]; k];
        for m in &members {
            for i in 0..k {
                let sum: f64 = m.iter().map(|&l| pm[i][l]).sum();
                for &j in m {
                    a1[i][j] = sum;
                }
            }
        }
        let tr_a1: f64 = (0..k).map(|i| a1[i][i]).sum();
        let tr_p: f64 = (0..k).map(|i| pm[i][i]).sum();
        let q1: f64 = members.iter().map(|m| m.iter().map(|&i| pr[i]).sum::<f64>().powi(2)).sum();
        let q2: f64 = pr.iter().map(|z| z * z).sum();
        let i11: f64 = (0..k).map(|i| (0..k).map(|j| a1[i][j] * a1[j][i]).sum::<f64>()).sum();
        let i12: f64 = (0..k).map(|i| (0..k).map(|j| a1[i][j] * pm[j][i]).sum::<f64>()).sum();
        let i22: f64 = pm.iter().flatten().map(|z| z * z).sum();
        let score = [-0.5 * tr_a1 + 0.5 * q1, -0.5 * tr_p + 0.5 * q2];
        let info = [[0.5 * i11, 0.5 * i12], [0.5 * i12, 0.5 * i22]];
        // Free components: not fixed, and not held at zero by a negative score.
        let free: Vec<usize> = (0..2).filter(|&c| !fixed[c] && !(s[c] <= 0.0 && score[c] <= 0.0)).collect();
        if free.is_empty() {
            break;
        }
        let mut step = [0.0; 2];
        if free.len() == 2 {
            let det = info[0][0] * info[1][1] - info[0][1] * info[1][0];
            if det.abs() < 1e-300 {
                break;
            }
            step[0] = (info[1][1] * score[0] - info[0][1] * score[1]) / det;
            step[1] = (info[0][0] * score[1] - info[1][0] * score[0]) / det;
        } else {
            let c = free[0];
            step[c] = score[c] / info[c][c];
        }
        // Step halving keeps the likelihood from falling.
        let mut t = 1.0;
        let mut next = None;
        for _ in 0..60 {
            let cand = [(s[0] + t * step[0]).max(0.0), (s[1] + t * step[1]).max(0.0)];
            if let Some(e) = ml3_eval(y, v, x, &members, cand, reml) {
                if e.0 >= cur.0 - 1e-12 * cur.0.abs().max(1.0) {
                    next = Some((cand, e));
                    break;
                }
            }
            t /= 2.0;
        }
        let Some((cand, e)) = next else { break };
        let moved = (0..2).map(|c| (cand[c] - s[c]).abs() / s[c].max(1e-8)).fold(0.0, f64::max);
        s = cand;
        cur = e;
        if moved < 1e-13 {
            break;
        }
    }
    let (ll, vinv, beta, inv) = cur;
    let w: Vec<f64> = (0..k).map(|i| vinv[i][i]).collect();
    let vt = kp / tr_p0;
    let tot = s[0] + s[1];
    let i2 = [100.0 * s[0] / (tot + vt), 100.0 * s[1] / (tot + vt)];
    let fit = Fit { beta, vb: inv, tau2: tot, tau2_se: f64::NAN, w, qe, df: kp, i2: i2[0] + i2[1], h2: (vt + tot) / vt, vt, s2: 1.0, ll };
    Some((fit, Multilevel { sigma2: s, i2, clusters: members.len() }))
}

fn log_det(m: &Mat) -> Option<f64> {
    let l = super::linalg::cholesky(m)?;
    Some(2.0 * (0..l.len()).map(|i| l[i][i].ln()).sum::<f64>())
}

/// Q-profile bounds for τ² (lower, upper).
pub fn tau2_ci(y: &[f64], v: &[f64], x: &Mat, level: f64) -> (f64, f64) {
    let df = (y.len() - x[0].len()) as f64;
    let a = 1.0 - level;
    let (hi_q, lo_q) = (chisq_quantile(1.0 - a / 2.0, df), chisq_quantile(a / 2.0, df));
    let lower = root(|t| q_gen(y, v, x, t).unwrap_or(f64::NAN) - hi_q);
    let upper = root(|t| q_gen(y, v, x, t).unwrap_or(f64::NAN) - lo_q);
    (lower, upper)
}

/// One study's computed effect size and sampling variance.
fn effect(spec: &Spec, get: &dyn Fn(&Option<String>) -> Option<f64>) -> Option<(f64, f64)> {
    match spec.measure {
        EffectMeasure::Generic => {
            let yi = get(&spec.effect)?;
            let vi = match (&spec.variance, &spec.se) {
                (Some(_), _) => get(&spec.variance)?,
                (None, Some(_)) => get(&spec.se)?.powi(2),
                _ => return None,
            };
            Some((yi, vi))
        }
        EffectMeasure::Smd | EffectMeasure::Md => {
            let (m1, s1, n1, m2, s2, n2) = (get(&spec.mean1)?, get(&spec.sd1)?, get(&spec.n1)?, get(&spec.mean2)?, get(&spec.sd2)?, get(&spec.n2)?);
            if spec.measure == EffectMeasure::Md {
                return Some((m1 - m2, s1 * s1 / n1 + s2 * s2 / n2));
            }
            let mi = n1 + n2 - 2.0;
            let sp = (((n1 - 1.0) * s1 * s1 + (n2 - 1.0) * s2 * s2) / mi).sqrt();
            // The exact small-sample correction Γ(m/2) / (√(m/2) Γ((m − 1)/2)).
            let c = (prismqda_stats::dist::ln_gamma(mi / 2.0) - prismqda_stats::dist::ln_gamma((mi - 1.0) / 2.0)).exp() / (mi / 2.0).sqrt();
            let g = c * (m1 - m2) / sp;
            Some((g, 1.0 / n1 + 1.0 / n2 + g * g / (2.0 * (n1 + n2))))
        }
        EffectMeasure::Or | EffectMeasure::Rr | EffectMeasure::Rd => {
            let (mut a, n1, mut c, n2) = (get(&spec.events1)?, get(&spec.n1)?, get(&spec.events2)?, get(&spec.n2)?);
            let (mut b, mut d) = (n1 - a, n2 - c);
            if [a, b, c, d].iter().any(|z| *z < 0.0) {
                return None;
            }
            if spec.measure != EffectMeasure::Rd && [a, b, c, d].contains(&0.0) {
                a += 0.5;
                b += 0.5;
                c += 0.5;
                d += 0.5;
            }
            let (t1, t2) = (a + b, c + d);
            match spec.measure {
                EffectMeasure::Or => Some(((a * d / (b * c)).ln(), 1.0 / a + 1.0 / b + 1.0 / c + 1.0 / d)),
                EffectMeasure::Rr => Some(((a / t1 / (c / t2)).ln(), 1.0 / a - 1.0 / t1 + 1.0 / c - 1.0 / t2)),
                _ => {
                    let (p1, p2) = (a / t1, c / t2);
                    Some((p1 - p2, p1 * (1.0 - p1) / t1 + p2 * (1.0 - p2) / t2))
                }
            }
        }
        EffectMeasure::Zcor => {
            let (r, n) = (get(&spec.r)?, get(&spec.n1)?);
            (r.abs() < 1.0).then(|| (r.atanh(), 1.0 / (n - 3.0)))
        }
    }
}

/// The scale results are reported on: (name of the effect, back-transformed name, transform).
/// A back-transformed scale: its name and the transform (exp for ratios, tanh for Fisher's z).
pub type BackTransform = Option<(&'static str, fn(f64) -> f64)>;

fn scale(m: EffectMeasure) -> (&'static str, BackTransform) {
    match m {
        EffectMeasure::Generic => ("Effect size", None),
        EffectMeasure::Smd => ("Hedges' g", None),
        EffectMeasure::Md => ("Mean difference", None),
        EffectMeasure::Or => ("Log odds ratio", Some(("Odds ratio", f64::exp))),
        EffectMeasure::Rr => ("Log risk ratio", Some(("Risk ratio", f64::exp))),
        EffectMeasure::Rd => ("Risk difference", None),
        EffectMeasure::Zcor => ("Fisher's z", Some(("Correlation", f64::tanh))),
    }
}

fn method_name(m: Method) -> &'static str {
    match m {
        Method::Fe => "fixed-effect",
        Method::Reml => "restricted maximum likelihood",
        Method::Ml => "maximum likelihood",
        Method::Dl => "DerSimonian-Laird",
        Method::Pm => "Paule-Mandel",
        Method::He => "Hedges",
        Method::Sj => "Sidik-Jonkman",
        Method::Bayes => "Bayesian",
    }
}

/// The trim-and-fill result: missing studies, side, the filled data and the refitted model.
struct TrimFill {
    k0: usize,
    se_k0: f64,
    side: &'static str,
    filled: Vec<(f64, f64)>,
    fit: Fit,
}

fn trim_fill(y: &[f64], v: &[f64], method: Method) -> Option<TrimFill> {
    let k = y.len();
    // The side: where the regression on the standard error says studies are missing.
    let xs: Mat = v.iter().map(|vi| vec![1.0, vi.sqrt()]).collect();
    let side_fit = fit(y, v, &xs, method, false)?;
    let right = side_fit.beta[1] < 0.0;
    let sign = if right { -1.0 } else { 1.0 };
    let mut idx: Vec<usize> = (0..k).collect();
    idx.sort_by(|&a, &b| (sign * y[a]).total_cmp(&(sign * y[b])));
    let ys: Vec<f64> = idx.iter().map(|&i| sign * y[i]).collect();
    let vs: Vec<f64> = idx.iter().map(|&i| v[i]).collect();
    let ones: Mat = vec![vec![1.0]; k];
    let (mut k0, mut last) = (0usize, usize::MAX);
    let mut se_k0 = 0.0;
    let mut beta = 0.0;
    let mut iter = 0;
    while k0 != last {
        last = k0;
        iter += 1;
        if iter > 100 {
            return None;
        }
        let kt = k - k0;
        beta = fit(&ys[..kt], &vs[..kt], &ones[..kt].to_vec(), method, false)?.beta[0];
        let c: Vec<f64> = ys.iter().map(|z| z - beta).collect();
        // Ranks of |c| (ties by order), signed.
        let mut order: Vec<usize> = (0..k).collect();
        order.sort_by(|&a, &b| c[a].abs().total_cmp(&c[b].abs()).then(a.cmp(&b)));
        let mut rank = vec![0.0; k];
        for (r, &i) in order.iter().enumerate() {
            rank[i] = (r + 1) as f64;
        }
        let sr: f64 = (0..k).filter(|&i| c[i] > 0.0).map(|i| rank[i]).sum();
        let kf = k as f64;
        let l0 = (4.0 * sr - kf * (kf + 1.0)) / (2.0 * kf - 1.0);
        let var_sr = (kf * (kf + 1.0) * (2.0 * kf + 1.0) + 10.0 * l0.powi(3) + 27.0 * l0 * l0 + 17.0 * l0 - 18.0 * kf * l0 * l0 - 18.0 * kf * l0 + 6.0 * kf * kf * l0) / 24.0;
        se_k0 = (4.0 * var_sr.max(0.0).sqrt() / (2.0 * kf - 1.0)).max(0.0);
        k0 = l0.round().max(0.0) as usize;
        k0 = k0.min(k - 1);
    }
    let mut yf = y.to_vec();
    let mut vf = v.to_vec();
    let mut filled = Vec::new();
    for i in k - k0..k {
        // Mirror the most extreme studies about the trimmed estimate (on the original scale).
        let val = sign * (2.0 * beta - ys[i]);
        yf.push(val);
        vf.push(vs[i]);
        filled.push((val, vs[i].sqrt()));
    }
    let xf: Mat = vec![vec![1.0]; yf.len()];
    // As metafor, the filled data are refitted with z tests (the Knapp-Hartung setting is not carried over).
    let f = fit(&yf, &vf, &xf, method, false)?;
    Some(TrimFill { k0, se_k0, side: if right { "right" } else { "left" }, filled, fit: f })
}

/// A moderator's columns: the variable itself, or indicators for each category after the first.
fn moderator_columns(ds: &Dataset, name: &str) -> Result<Vec<(String, Vec<Slot<f64>>)>> {
    let var = ds.variable(name)?;
    let i = ds.find(name).ok_or_else(|| Error::UnknownVariable(name.to_string()))?;
    let categorical = matches!(ds.columns[i], Column::Text(_)) || var.measure == Level::Nominal;
    if !categorical {
        return Ok(vec![(var.display_name().to_string(), numeric(ds, name)?)]);
    }
    let (slots, levels) = categories(ds, name)?;
    Ok(levels
        .iter()
        .enumerate()
        .skip(1)
        .map(|(j, lv)| {
            (
                format!("{}: {}", var.display_name(), lv.label),
                slots
                    .iter()
                    .map(|s| match s {
                        Slot::Valid(c) => Slot::Valid(if *c == j { 1.0 } else { 0.0 }),
                        Slot::Empty => Slot::Empty,
                        Slot::UserMissing => Slot::UserMissing,
                    })
                    .collect(),
            )
        })
        .collect())
}

/// A study's label from the label variable (text or numeric), or "Study i".
fn study_label(ds: &Dataset, name: Option<&String>, row: usize) -> String {
    let fallback = format!("Study {}", row + 1);
    let Some(name) = name else { return fallback };
    let (Some(i), Ok(var)) = (ds.find(name), ds.variable(name)) else { return fallback };
    match &ds.columns[i] {
        Column::Text(t) => t[row].clone().unwrap_or(fallback),
        Column::Numeric(c) => c[row].map_or(fallback, |v| var.value_text(v)),
    }
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    common::check_level(spec.level)?;
    let need: Vec<(&Option<String>, &str)> = match spec.measure {
        EffectMeasure::Generic => vec![(&spec.effect, "an effect size")],
        EffectMeasure::Smd | EffectMeasure::Md => vec![(&spec.mean1, "group 1's mean"), (&spec.sd1, "group 1's SD"), (&spec.n1, "group 1's size"), (&spec.mean2, "group 2's mean"), (&spec.sd2, "group 2's SD"), (&spec.n2, "group 2's size")],
        EffectMeasure::Or | EffectMeasure::Rr | EffectMeasure::Rd => vec![(&spec.events1, "group 1's events"), (&spec.n1, "group 1's total"), (&spec.events2, "group 2's events"), (&spec.n2, "group 2's total")],
        EffectMeasure::Zcor => vec![(&spec.r, "the correlation"), (&spec.n1, "the sample size")],
    };
    if let Some((_, what)) = need.iter().find(|(v, _)| v.is_none()) {
        return Err(Error::Invalid(format!("Choose the variable that holds {what}.")));
    }
    if spec.measure == EffectMeasure::Generic && spec.variance.is_none() && spec.se.is_none() {
        return Err(Error::Invalid("Choose the variable that holds each effect's sampling variance or standard error.".into()));
    }
    let inputs: Vec<&String> = [&spec.effect, &spec.variance, &spec.se, &spec.mean1, &spec.sd1, &spec.n1, &spec.mean2, &spec.sd2, &spec.n2, &spec.events1, &spec.events2, &spec.r].into_iter().flatten().collect();
    let cols: Vec<(String, Vec<Slot<f64>>)> = inputs.iter().map(|n| numeric(ds, n).map(|c| ((*n).clone(), c))).collect::<Result<_>>()?;
    let mods: Vec<(String, Vec<Slot<f64>>)> = spec.moderators.iter().map(|m| moderator_columns(ds, m)).collect::<Result<Vec<_>>>()?.into_iter().flatten().collect();
    let total = ds.n_rows();
    let mut tally = Tally::default();
    if spec.cluster.is_some() && !matches!(spec.method, Method::Reml | Method::Ml) {
        return Err(Error::Invalid("A multilevel model is estimated by REML or ML.".into()));
    }
    let (mut y, mut v, mut xrows, mut labels) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut clusters: Vec<String> = Vec::new();
    'row: for row in 0..total {
        if let Some(cn) = &spec.cluster {
            let fallback = study_label(ds, None, row);
            let l = study_label(ds, Some(cn), row);
            if l == fallback {
                tally.add(format!("no {cn}"));
                continue 'row;
            }
        }
        for (name, c) in cols.iter() {
            if tally.check(&c[row], name) {
                continue 'row;
            }
        }
        for (name, c) in mods.iter() {
            if tally.check(&c[row], name) {
                continue 'row;
            }
        }
        let get = |n: &Option<String>| n.as_ref().and_then(|n| cols.iter().find(|(m, _)| m == n)).and_then(|(_, c)| c[row].value());
        match effect(spec, &get) {
            Some((yi, vi)) if yi.is_finite() && vi.is_finite() && vi > 0.0 => {
                y.push(yi);
                v.push(vi);
                xrows.push(std::iter::once(1.0).chain(mods.iter().map(|(_, c)| c[row].value().expect("checked"))).collect::<Vec<f64>>());
                labels.push(study_label(ds, spec.study.as_ref(), row));
                if spec.cluster.is_some() {
                    clusters.push(study_label(ds, spec.cluster.as_ref(), row));
                }
            }
            _ => tally.add("the effect size could not be computed (or its variance is not positive)"),
        }
    }
    let k = y.len();
    let (name, back) = scale(spec.measure);
    let lv = level_text(spec.level);
    let note = format!(
        "Meta-analysis as R's metafor (escalc, rma.uni): {} model{}{}; Cochran's Q with fixed-effect weights; I² and H² from τ² and the typical within-study variance; Q-profile {lv} intervals for τ², τ, I² and H². Computed by PrismStatistics.",
        method_name(spec.method),
        if spec.method == Method::Fe { "" } else { " (random effects)" },
        if spec.knha { ", Knapp-Hartung adjustment" } else { "" }
    );
    let title = if spec.moderators.is_empty() { "Meta-Analysis" } else { "Meta-Regression" };
    let mut b = common::bundle(META, METHOD_VERSION, title.into(), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, k), note);
    set(&mut b, "k", k as f64);
    set_text(&mut b, "measure", name);
    set_text(&mut b, "method", method_name(spec.method));
    let p = xrows.first().map_or(1, Vec::len);
    if k < 2 || k <= p {
        fail(&mut b, format!("There are {k} usable studies; the model needs at least {}.", (p + 1).max(2)));
        return Ok(b);
    }
    if spec.method == Method::Bayes {
        return bayes_report(b, spec, &y, &v, &labels, p, name, back);
    }
    let multi = spec.cluster.is_some();
    let mut cl_ix: Vec<usize> = Vec::new();
    let mut ml: Option<Multilevel> = None;
    let fitted = if multi {
        let mut names: Vec<String> = clusters.clone();
        names.sort();
        names.dedup();
        cl_ix = clusters.iter().map(|c| names.binary_search(c).expect("listed")).collect();
        fit_multilevel(&y, &v, &xrows, &cl_ix, spec.method, [false, false]).map(|(f, m)| {
            ml = Some(m);
            f
        })
    } else {
        fit(&y, &v, &xrows, spec.method, spec.knha)
    };
    let Some(f) = fitted else {
        fail(&mut b, "The model could not be fitted: the moderators are collinear or the variances are degenerate.");
        return Ok(b);
    };
    let df = f.df;
    let (crit, dist) = if spec.knha { (t_quantile(1.0 - (1.0 - spec.level) / 2.0, df), "t") } else { (norm_quantile(1.0 - (1.0 - spec.level) / 2.0), "z") };
    let pval = |stat: f64| if spec.knha { t_two_sided(stat, df) } else { norm_two_sided(stat) };
    let bt = |x: f64| back.map_or(x, |(_, t)| t(x));
    let fmt_bt = |x: f64| apa::num(bt(x), 2);

    // Studies.
    let sum_w: f64 = f.w.iter().sum();
    let zs = norm_quantile(1.0 - (1.0 - spec.level) / 2.0);
    let mut study_rows = Vec::new();
    for i in 0..k {
        let se = v[i].sqrt();
        set(&mut b, format!("yi.{}", i + 1), y[i]);
        set(&mut b, format!("vi.{}", i + 1), v[i]);
        set(&mut b, format!("weight.{}", i + 1), 100.0 * f.w[i] / sum_w);
        let mut row = vec![text(labels[i].clone()), num(y[i], 3), num(se, 3), text(format!("[{}, {}]", apa::num(y[i] - zs * se, 2), apa::num(y[i] + zs * se, 2)))];
        if back.is_some() {
            row.push(text(format!("{} [{}, {}]", fmt_bt(y[i]), fmt_bt(y[i] - zs * se), fmt_bt(y[i] + zs * se))));
        }
        row.push(num(100.0 * f.w[i] / sum_w, 1));
        study_rows.push(row);
    }
    let mut cols = vec!["Study".to_string(), name.into(), "SE".into(), format!("{lv} CI")];
    if let Some((bn, _)) = back {
        cols.push(format!("{bn} [{lv} CI]"));
    }
    cols.push("Weight (%)".into());
    b.tables.push(Table { id: "studies".into(), title: "Studies".into(), columns: cols, rows: study_rows, footnotes: vec![format!("Weights are each study's share of the {} weights.", if spec.method == Method::Fe { "inverse-variance" } else { "random-effects" })] });

    // Coefficients.
    let terms: Vec<String> = std::iter::once(if p == 1 { "Summary effect".to_string() } else { "Intercept".to_string() }).chain(mods.iter().map(|(n, _)| n.clone())).collect();
    let mut coef_rows = Vec::new();
    for j in 0..p {
        let se = f.vb[j][j].sqrt();
        let stat = f.beta[j] / se;
        let (lo, hi) = (f.beta[j] - crit * se, f.beta[j] + crit * se);
        for (key, val) in [("est", f.beta[j]), ("se", se), ("stat", stat), ("p", pval(stat)), ("lower", lo), ("upper", hi)] {
            set(&mut b, format!("beta.{}.{key}", j + 1), val);
        }
        set_text(&mut b, format!("beta.{}.term", j + 1), terms[j].clone());
        let mut row = vec![text(terms[j].clone()), num(f.beta[j], 3), num(se, 3), num(stat, 2), common::p(pval(stat)), text(format!("[{}, {}]", apa::num(lo, 2), apa::num(hi, 2)))];
        if back.is_some() && p == 1 {
            row.push(text(format!("{} [{}, {}]", fmt_bt(f.beta[j]), fmt_bt(lo), fmt_bt(hi))));
        }
        coef_rows.push(row);
    }
    let mut cols = vec!["Term".to_string(), "Estimate".into(), "SE".into(), dist.into(), "p".into(), format!("{lv} CI")];
    if let (Some((bn, _)), 1) = (back, p) {
        cols.push(format!("{bn} [{lv} CI]"));
    }
    let mut foot = Vec::new();
    if p == 1 && spec.method != Method::Fe {
        let se = f.vb[0][0].sqrt();
        let half = crit * (se * se + f.tau2).sqrt();
        set(&mut b, "pi.lower", f.beta[0] - half);
        set(&mut b, "pi.upper", f.beta[0] + half);
        foot.push(format!(
            "{lv} prediction interval [{}, {}]{}: the range in which the true effect of a new study is expected.",
            apa::num(f.beta[0] - half, 2),
            apa::num(f.beta[0] + half, 2),
            back.map_or(String::new(), |(bn, _)| format!(" ({bn} [{}, {}])", fmt_bt(f.beta[0] - half), fmt_bt(f.beta[0] + half)))
        ));
    }
    if spec.knha {
        foot.push(format!("Knapp-Hartung adjustment: t tests on {} df.", apa::num(df, 0)));
    }
    if p > 1 {
        let m = p - 1;
        let sub: Mat = (1..p).map(|a| (1..p).map(|c| f.vb[a][c]).collect()).collect();
        if let Some(si) = invert(&sub) {
            let bb: Vec<f64> = f.beta[1..].to_vec();
            let qm: f64 = (0..m).map(|a| bb[a] * (0..m).map(|c| si[a][c] * bb[c]).sum::<f64>()).sum();
            let (stat, pq) = if spec.knha { (qm / m as f64, f_sf(qm / m as f64, m as f64, df)) } else { (qm, chisq_sf(qm, m as f64)) };
            set(&mut b, "qm", stat);
            set(&mut b, "qm.p", pq);
            set(&mut b, "qm.df", m as f64);
            foot.push(if spec.knha {
                format!("Test of moderators: F({m}, {}) = {}, p {}.", apa::num(df, 0), apa::num(stat, 2), apa::p_clause(pq))
            } else {
                format!("Test of moderators: QM({m}) = {}, p {}.", apa::num(stat, 2), apa::p_clause(pq))
            });
        }
        if spec.method != Method::Fe && !multi {
            let ones: Mat = vec![vec![1.0]; k];
            if let Some(f0) = fit(&y, &v, &ones, spec.method, false) {
                if f0.tau2 > 0.0 {
                    let r2 = (100.0 * (f0.tau2 - f.tau2) / f0.tau2).max(0.0);
                    set(&mut b, "r2", r2);
                    foot.push(format!("R² (share of the heterogeneity the moderators account for) = {}%.", apa::num(r2, 2)));
                }
            }
        }
    }
    b.tables.push(Table { id: "model".into(), title: if p == 1 { "Summary effect".into() } else { "Meta-regression coefficients".into() }, columns: cols, rows: coef_rows, footnotes: foot });

    // Heterogeneity.
    set(&mut b, "tau2", f.tau2);
    set(&mut b, "tau2.se", f.tau2_se);
    set(&mut b, "qe", f.qe);
    set(&mut b, "qe.df", df);
    set(&mut b, "qe.p", chisq_sf(f.qe, df));
    set(&mut b, "i2", f.i2);
    set(&mut b, "h2", f.h2);
    set(&mut b, "logl", f.ll);
    let npar = p as f64 + if spec.method == Method::Fe { 0.0 } else { 1.0 };
    set(&mut b, "aic", -2.0 * f.ll + 2.0 * npar);
    set(&mut b, "bic", -2.0 * f.ll + npar * (if spec.method == Method::Reml { k - p } else { k } as f64).ln());
    let mut het_rows = vec![vec![text(format!("Q({})", apa::num(df, 0))), num(f.qe, 2), common::p(chisq_sf(f.qe, df)), text("")]];
    if let Some(m) = &ml {
        let npar = p as f64 + 2.0;
        set(&mut b, "aic", -2.0 * f.ll + 2.0 * npar);
        set(&mut b, "bic", -2.0 * f.ll + npar * (if spec.method == Method::Reml { k - p } else { k } as f64).ln());
        set(&mut b, "clusters", m.clusters as f64);
        let cname = spec.cluster.clone().unwrap_or_default();
        for (j, (what, n)) in [(format!("Between {cname} (level 3)"), m.clusters), ("Within clusters (level 2)".to_string(), k)].into_iter().enumerate() {
            set(&mut b, format!("sigma2.{}", j + 1), m.sigma2[j]);
            set(&mut b, format!("i2.{}", j + 1), m.i2[j]);
            // Likelihood-ratio test against the model without this component.
            let mut fixed = [false, false];
            fixed[j] = true;
            let (lrt, lp) = match fit_multilevel(&y, &v, &xrows, &cl_ix, spec.method, fixed) {
                Some((g, _)) => {
                    let l = (2.0 * (f.ll - g.ll)).max(0.0);
                    (l, chisq_sf(l, 1.0))
                }
                None => (f64::NAN, f64::NAN),
            };
            if lrt.is_finite() {
                set(&mut b, format!("lrt.{}", j + 1), lrt);
                set(&mut b, format!("lrt.{}.p", j + 1), lp);
            }
            het_rows.push(vec![text(format!("σ² {what}, {n} levels")), num(m.sigma2[j], 4), if lp.is_finite() { common::p(lp) } else { text("") }, text(format!("I² = {}%", apa::num(m.i2[j], 1)))]);
        }
        het_rows.push(vec![text("I² total (%)"), num(f.i2, 2), text(""), text("")]);
        b.tables.push(Table {
            id: "heterogeneity".into(),
            title: if p == 1 { "Variance components".into() } else { "Residual variance components".into() },
            columns: vec!["Component".into(), "Value".into(), "p".into(), "Share".into()],
            rows: het_rows,
            footnotes: vec![
                format!("Three-level model by {}: effects nested in {cname}. p for each σ² is a likelihood-ratio test (1 df) of the model without it. I² for each level is its share of the total variance, with the typical sampling variance {} (Cheung, 2014).", method_name(spec.method), apa::num(f.vt, 4)),
            ],
        });
    } else {
    if spec.method == Method::Fe {
        het_rows.push(vec![text("I² (%)"), num(f.i2, 2), text(""), text("")]);
        het_rows.push(vec![text("H²"), num(f.h2, 2), text(""), text("")]);
    } else {
        let (lo, hi) = tau2_ci(&y, &v, &xrows, spec.level);
        let i2 = |t: f64| 100.0 * t / (f.vt + t);
        let h2 = |t: f64| (f.vt + t) / f.vt;
        for (key, val) in [("tau2.lower", lo), ("tau2.upper", hi), ("tau.lower", lo.sqrt()), ("tau.upper", hi.sqrt()), ("i2.lower", i2(lo)), ("i2.upper", i2(hi)), ("h2.lower", h2(lo)), ("h2.upper", h2(hi))] {
            set(&mut b, key, val);
        }
        let ci = |a: f64, c: f64, d: usize| text(format!("[{}, {}]", apa::num(a, d), apa::num(c, d)));
        het_rows.push(vec![text("τ²"), num(f.tau2, 4), text(""), ci(lo, hi, 4)]);
        het_rows.push(vec![text("τ"), num(f.tau2.sqrt(), 4), text(""), ci(lo.sqrt(), hi.sqrt(), 4)]);
        het_rows.push(vec![text("I² (%)"), num(f.i2, 2), text(""), ci(i2(lo), i2(hi), 2)]);
        het_rows.push(vec![text("H²"), num(f.h2, 2), text(""), ci(h2(lo), h2(hi), 2)]);
    }
    b.tables.push(Table {
        id: "heterogeneity".into(),
        title: if p == 1 { "Heterogeneity".into() } else { "Residual heterogeneity".into() },
        columns: vec!["Statistic".into(), "Value".into(), "p".into(), format!("{lv} CI")],
        rows: het_rows,
        footnotes: vec![if spec.method == Method::Fe { "I² and H² from Q in the fixed-effect model.".into() } else { format!("τ² by {}; intervals by the Q-profile method.", method_name(spec.method)) }],
    });
    }

    // Small-study effects.
    let mut tf: Option<TrimFill> = None;
    if spec.bias && p == 1 && k >= 3 && !multi {
        let mut rows = Vec::new();
        let xs: Mat = v.iter().map(|vi| vec![1.0, vi.sqrt()]).collect();
        if let Some(e) = fit(&y, &v, &xs, spec.method, spec.knha) {
            let se = e.vb[1][1].sqrt();
            let stat = e.beta[1] / se;
            let pe = if spec.knha { t_two_sided(stat, e.df) } else { norm_two_sided(stat) };
            let crit_e = if spec.knha { t_quantile(1.0 - (1.0 - spec.level) / 2.0, e.df) } else { zs };
            let lse = e.vb[0][0].sqrt();
            set(&mut b, "egger.stat", stat);
            set(&mut b, "egger.p", pe);
            set(&mut b, "egger.limit", e.beta[0]);
            set(&mut b, "egger.limit.lower", e.beta[0] - crit_e * lse);
            set(&mut b, "egger.limit.upper", e.beta[0] + crit_e * lse);
            rows.push(vec![text("Egger's regression test"), text(format!("{dist} = {}", apa::num(stat, 2))), common::p(pe), text(format!("Limit estimate {} [{}, {}]", apa::num(e.beta[0], 3), apa::num(e.beta[0] - crit_e * lse, 2), apa::num(e.beta[0] + crit_e * lse, 2)))]);
        }
        tf = trim_fill(&y, &v, spec.method);
        if let Some(t) = &tf {
            let se = t.fit.vb[0][0].sqrt();
            let (est, lo, hi) = (t.fit.beta[0], t.fit.beta[0] - zs * se, t.fit.beta[0] + zs * se);
            set(&mut b, "tf.k0", t.k0 as f64);
            set(&mut b, "tf.se.k0", t.se_k0);
            set_text(&mut b, "tf.side", t.side);
            set(&mut b, "tf.est", est);
            set(&mut b, "tf.lower", lo);
            set(&mut b, "tf.upper", hi);
            set(&mut b, "tf.tau2", t.fit.tau2);
            rows.push(vec![
                text("Trim and fill"),
                text(format!("{} missing on the {}", t.k0, t.side)),
                text(""),
                text(format!("Adjusted estimate {} [{}, {}]{}", apa::num(est, 3), apa::num(lo, 2), apa::num(hi, 2), back.map_or(String::new(), |(bn, _)| format!("; {bn} {}", fmt_bt(est))))),
            ]);
        }
        b.tables.push(Table {
            id: "bias".into(),
            title: "Small-study effects (publication bias)".into(),
            columns: vec!["Method".into(), "Result".into(), "p".into(), "Estimate".into()],
            rows,
            footnotes: vec!["Egger's test regresses the effects on their standard errors; the limit estimate is the effect expected of a study with standard error 0. Trim and fill (L0 estimator) imputes the studies an asymmetric funnel suggests are missing. Both have little power with fewer than 10 studies.".into()],
        });
    }

    // Leave-one-out.
    if spec.leave_one_out && k >= 3 && !multi {
        let mut rows = Vec::new();
        for i in 0..k {
            let keep: Vec<usize> = (0..k).filter(|&j| j != i).collect();
            let (yy, vv, xx): (Vec<f64>, Vec<f64>, Mat) = (keep.iter().map(|&j| y[j]).collect(), keep.iter().map(|&j| v[j]).collect(), keep.iter().map(|&j| xrows[j].clone()).collect());
            if let Some(g) = fit(&yy, &vv, &xx, spec.method, spec.knha) {
                let se = g.vb[0][0].sqrt();
                let c = if spec.knha { t_quantile(1.0 - (1.0 - spec.level) / 2.0, g.df) } else { zs };
                set(&mut b, format!("loo.{}.est", i + 1), g.beta[0]);
                set(&mut b, format!("loo.{}.tau2", i + 1), g.tau2);
                set(&mut b, format!("loo.{}.i2", i + 1), g.i2);
                rows.push(vec![text(labels[i].clone()), num(g.beta[0], 3), text(format!("[{}, {}]", apa::num(g.beta[0] - c * se, 2), apa::num(g.beta[0] + c * se, 2))), num(g.qe, 2), num(g.i2, 2), num(g.tau2, 4)]);
            }
        }
        b.tables.push(Table { id: "loo".into(), title: "Leave-one-out analysis".into(), columns: vec!["Study omitted".into(), if p == 1 { "Estimate".into() } else { "Intercept".into() }, format!("{lv} CI"), "Q".into(), "I² (%)".into(), "τ²".into()], rows, footnotes: vec!["Each row refits the model without that study.".into()] });
    }

    // Charts.
    if spec.forest && p == 1 {
        let mut rows: Vec<ForestRow> = (0..k)
            .map(|i| {
                let se = v[i].sqrt();
                ForestRow { label: labels[i].clone(), est: y[i], lo: y[i] - zs * se, hi: y[i] + zs * se, weight: Some(f.w[i] / sum_w), right: format!("{} [{}, {}]", fmt_bt(y[i]), fmt_bt(y[i] - zs * se), fmt_bt(y[i] + zs * se)) }
            })
            .collect();
        let se = f.vb[0][0].sqrt();
        let (lo, hi) = (f.beta[0] - crit * se, f.beta[0] + crit * se);
        rows.push(ForestRow { label: if multi { "Three-level model".into() } else if spec.method == Method::Fe { "Fixed-effect model".into() } else { "Random-effects model".into() }, est: f.beta[0], lo, hi, weight: None, right: format!("{} [{}, {}]", fmt_bt(f.beta[0]), fmt_bt(lo), fmt_bt(hi)) });
        let exp_axis = matches!(spec.measure, EffectMeasure::Or | EffectMeasure::Rr);
        let axis = back.filter(|_| exp_axis).map_or(name, |(bn, _)| bn);
        let alt = format!("Forest plot of {k} studies: {} [{lv} CI] for each, with the pooled estimate {} [{}, {}] as a diamond.", axis.to_lowercase(), fmt_bt(f.beta[0]), fmt_bt(lo), fmt_bt(hi));
        let xlabel = if exp_axis { format!("{axis} (log scale)") } else { axis.to_string() };
        b.charts.push(chart::Chart { id: "forest".into(), title: "Forest plot".into(), svg: chart::forest(&rows, 0.0, &xlabel, &format!("{} [{lv} CI]", back.map_or(name, |(bn, _)| bn)), exp_axis, &alt), alt });
    }
    if spec.funnel && p == 1 && k >= 3 {
        let pts: Vec<(f64, f64)> = (0..k).map(|i| (y[i], v[i].sqrt())).collect();
        let filled = tf.as_ref().map_or(Vec::new(), |t| t.filled.clone());
        let alt = format!("Funnel plot of {k} studies' {} against their standard errors, centered on the pooled estimate {}{}.", name.to_lowercase(), apa::num(f.beta[0], 2), if filled.is_empty() { String::new() } else { format!("; {} studies imputed by trim and fill are hollow", filled.len()) });
        b.charts.push(chart::Chart { id: "funnel".into(), title: "Funnel plot".into(), svg: chart::funnel(&pts, &filled, f.beta[0], name, &alt), alt });
    }

    // Diagnostics.
    if spec.method != Method::Fe || p > 1 {
        b.diagnostics.push(if f.i2 >= 75.0 {
            Diagnostic { id: "heterogeneity".into(), title: "Heterogeneity".into(), state: DiagnosticState::ReviewNeeded, message: format!("I² = {}%: most of the variation is between studies, so the summary effect describes an average of different true effects; look for moderators and report the prediction interval.", apa::num(f.i2, 1)) }
        } else {
            Diagnostic { id: "heterogeneity".into(), title: "Heterogeneity".into(), state: DiagnosticState::NoConcernDetected, message: format!("I² = {}%.", apa::num(f.i2, 1)) }
        });
    }
    if spec.method == Method::Fe && chisq_sf(f.qe, df) < 0.10 {
        b.diagnostics.push(Diagnostic { id: "fixed".into(), title: "Fixed-effect assumption".into(), state: DiagnosticState::ReviewNeeded, message: "Q suggests the studies do not share one true effect; a random-effects model is usually more appropriate.".into() });
    }
    if multi && (spec.bias || spec.leave_one_out) {
        b.diagnostics.push(Diagnostic { id: "multilevel_extras".into(), title: "Small-study tests".into(), state: DiagnosticState::NotAssessed, message: "Egger's test, trim and fill and leave-one-out are for one effect per study; they were not run on the multilevel model.".into() });
    }
    if spec.bias && p == 1 && k < 10 && !multi {
        b.diagnostics.push(Diagnostic { id: "bias_power".into(), title: "Small-study tests".into(), state: DiagnosticState::ReviewNeeded, message: format!("With {k} studies, Egger's test and trim and fill have little power; a nonsignificant result does not rule out publication bias.") });
    }
    if spec.knha && f.s2 < 1.0 {
        b.diagnostics.push(Diagnostic { id: "knha".into(), title: "Knapp-Hartung".into(), state: DiagnosticState::NoConcernDetected, message: format!("The Knapp-Hartung factor is {} (below 1), which narrows the interval relative to the unadjusted one.", apa::num(f.s2, 3)) });
    }
    b.diagnostics.push(common::size_diagnostic(k, format!("{k} studies")));
    Ok(b)
}

/// The Bayesian random-effects model's report.
#[allow(clippy::too_many_arguments)]
fn bayes_report(mut b: ResultBundle, spec: &Spec, y: &[f64], v: &[f64], labels: &[String], p: usize, name: &str, back: BackTransform) -> Result<ResultBundle> {
    use super::bmeta::{self, Priors, TauPrior};
    if p > 1 || spec.cluster.is_some() {
        return Err(Error::Invalid("The Bayesian model here has no moderators or clusters.".into()));
    }
    if spec.tau_prior != TauPrior::Uniform && (spec.tau_prior_scale.is_nan() || spec.tau_prior_scale <= 0.0) {
        return Err(Error::Invalid("The prior scale for τ must be positive.".into()));
    }
    let mu = match (spec.mu_prior_mean, spec.mu_prior_sd) {
        (Some(m), Some(s)) if s > 0.0 => Some((m, s)),
        (None, None) => None,
        _ => return Err(Error::Invalid("Give both the mean and the (positive) SD of the normal prior for μ, or neither.".into())),
    };
    let k = y.len();
    if k < 3 && spec.tau_prior == TauPrior::Uniform && mu.is_none() {
        fail(&mut b, "With fewer than three studies and uniform priors the posterior is improper; choose a proper prior for τ.");
        return Ok(b);
    }
    let priors = Priors { tau: spec.tau_prior, tau_scale: spec.tau_prior_scale, mu };
    let Some(post) = bmeta::analyze(y, v, priors, spec.level) else {
        fail(&mut b, "The posterior could not be computed.");
        return Ok(b);
    };
    let lv = level_text(spec.level);
    let prior_text = match spec.tau_prior {
        TauPrior::Uniform => "a uniform (improper) prior for τ".to_string(),
        TauPrior::HalfNormal => format!("a half-normal prior for τ with scale {}", apa::num(spec.tau_prior_scale, 2)),
        TauPrior::HalfCauchy => format!("a half-Cauchy prior for τ with scale {}", apa::num(spec.tau_prior_scale, 2)),
    };
    let mu_text = mu.map_or("a uniform (improper) prior for μ".to_string(), |(m, s)| format!("a normal prior for μ with mean {} and SD {}", apa::num(m, 2), apa::num(s, 2)));
    b.method_note = format!("Bayesian random-effects meta-analysis as R's bayesmeta: {prior_text} and {mu_text}; the marginal posterior of τ by numerical integration, and μ, a new study's effect and each study's shrinkage estimate as normal mixtures over it; shortest {lv} credible intervals. Computed by PrismStatistics.");
    set_text(&mut b, "tau_prior", match spec.tau_prior {
        TauPrior::Uniform => "uniform",
        TauPrior::HalfNormal => "half-normal",
        TauPrior::HalfCauchy => "half-Cauchy",
    });
    let put = |b: &mut ResultBundle, key: &str, s: &bmeta::Summary| {
        for (kk, val) in [("mode", s.mode), ("median", s.median), ("mean", s.mean), ("sd", s.sd), ("lower", s.lower), ("upper", s.upper)] {
            set(b, format!("{key}.{kk}"), val);
        }
    };
    put(&mut b, "tau", &post.tau);
    put(&mut b, "mu", &post.mu);
    put(&mut b, "pred", &post.theta_new);
    set(&mut b, "i2.median", 100.0 * post.i2_median);
    let bt = |x: f64| back.map_or(x, |(_, t)| t(x));
    let row = |label: &str, s: &bmeta::Summary, d: u8| vec![text(label), num(s.median, d), num(s.mean, d), num(s.sd, d), text(format!("[{}, {}]", apa::num(s.lower, d as usize), apa::num(s.upper, d as usize))), num(s.mode, d)];
    let mut rows = vec![row("μ (pooled effect)", &post.mu, 3), row("τ (between-study SD)", &post.tau, 3), row("θ in a new study (prediction)", &post.theta_new, 3)];
    if let Some((bn, _)) = back {
        rows.push(vec![text(format!("μ as {bn}")), num(bt(post.mu.median), 3), text(""), text(""), text(format!("[{}, {}]", apa::num(bt(post.mu.lower), 3), apa::num(bt(post.mu.upper), 3))), text("")]);
    }
    b.tables.push(Table {
        id: "posterior".into(),
        title: "Posterior summaries".into(),
        columns: vec!["Parameter".into(), "Median".into(), "Mean".into(), "SD".into(), format!("{lv} credible interval"), "Mode".into()],
        rows,
        footnotes: vec![format!("Effect sizes are {}; intervals are the shortest containing {lv} of the posterior. I² at the posterior median of τ: {}%.", name.to_lowercase(), apa::num(100.0 * post.i2_median, 1))],
    });
    let mut srows = Vec::new();
    for i in 0..k {
        let t = &post.theta[i];
        for (kk, val) in [("median", t.median), ("mean", t.mean), ("sd", t.sd), ("lower", t.lower), ("upper", t.upper)] {
            set(&mut b, format!("theta.{}.{kk}", i + 1), val);
        }
        srows.push(vec![text(labels[i].clone()), num(y[i], 3), num(v[i].sqrt(), 3), num(t.mean, 3), text(format!("[{}, {}]", apa::num(t.lower, 2), apa::num(t.upper, 2)))]);
    }
    b.tables.push(Table { id: "studies".into(), title: "Studies and shrinkage estimates".into(), columns: vec!["Study".into(), name.into(), "SE".into(), "Shrinkage estimate (posterior mean)".into(), format!("{lv} credible interval")], rows: srows, footnotes: vec!["Each study's true effect, estimated with the information from the others: pulled toward the pooled effect, more so for imprecise studies.".into()] });
    if spec.forest {
        let mut fr: Vec<ForestRow> = (0..k)
            .map(|i| {
                let t = &post.theta[i];
                ForestRow { label: labels[i].clone(), est: t.mean, lo: t.lower, hi: t.upper, weight: Some(1.0 / v[i] / v.iter().map(|x| 1.0 / x).sum::<f64>()), right: format!("{} [{}, {}]", apa::num(bt(t.mean), 2), apa::num(bt(t.lower), 2), apa::num(bt(t.upper), 2)) }
            })
            .collect();
        fr.push(ForestRow { label: "Pooled effect (μ)".into(), est: post.mu.median, lo: post.mu.lower, hi: post.mu.upper, weight: None, right: format!("{} [{}, {}]", apa::num(bt(post.mu.median), 2), apa::num(bt(post.mu.lower), 2), apa::num(bt(post.mu.upper), 2)) });
        let exp_axis = matches!(spec.measure, EffectMeasure::Or | EffectMeasure::Rr);
        let axis = back.filter(|_| exp_axis).map_or(name, |(bn, _)| bn);
        let alt = format!("Forest plot of {k} studies' shrinkage estimates with {lv} credible intervals, and the posterior median of the pooled effect, {}.", apa::num(bt(post.mu.median), 2));
        let xlabel = if exp_axis { format!("{axis} (log scale)") } else { axis.to_string() };
        b.charts.push(chart::Chart { id: "forest".into(), title: "Forest plot (shrinkage estimates)".into(), svg: chart::forest(&fr, 0.0, &xlabel, &format!("{} [{lv} CrI]", back.map_or(name, |(bn, _)| bn)), exp_axis, &alt), alt });
    }
    b.diagnostics.push(common::size_diagnostic(k, format!("{k} studies")));
    if spec.tau_prior == TauPrior::Uniform && k < 5 {
        b.diagnostics.push(Diagnostic { id: "prior".into(), title: "Prior for τ".into(), state: DiagnosticState::ReviewNeeded, message: "With few studies the uniform prior for τ dominates the posterior of the heterogeneity; a weakly informative half-normal or half-Cauchy prior is usually preferred.".into() });
    }
    Ok(b)
}
