//! More regression models for outcomes that aren't continuous (checklist B5):
//!
//! * Ordinal logistic regression (`reg.ordinal`): the proportional-odds cumulative logit model, logit P(Y ≤ j) =
//!   θⱼ − xβ, R's `MASS::polr` parametrization (a positive coefficient means higher categories are more likely).
//! * Multinomial logistic regression (`reg.multinomial`): a baseline-category logit for each category against the
//!   reference, as R's `nnet::multinom`, with a likelihood-ratio test of each predictor.
//! * Count regression (`reg.count`): Poisson (IRLS) or negative binomial (IRLS alternating with the maximum likelihood
//!   estimate of θ, as `MASS::glm.nb`), with an optional exposure variable entered as a log offset.
//!
//! Every model is fitted by Newton-Raphson (or IRLS) to tight convergence; standard errors come from the inverse of the
//! observed information. Predictors are entered as in linear regression: numeric covariates and dummy-coded factors.

use prismqda_stats::dist::{chisq_sf, norm_quantile, norm_two_sided};
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use super::linalg::{invert, Mat};
use super::regression::{self, Factor};
use crate::apa;
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Cell, ResultBundle, Table};

pub const ORDINAL: &str = "reg.ordinal";
pub const MULTINOMIAL: &str = "reg.multinomial";
pub const COUNT: &str = "reg.count";
pub const METHOD_VERSION: u32 = 1;

fn default_level() -> f64 {
    0.95
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub dependent: String,
    #[serde(default)]
    pub covariates: Vec<String>,
    #[serde(default)]
    pub factors: Vec<Factor>,
    /// Multinomial: the reference category of the outcome (code or text); the first when left out.
    #[serde(default)]
    pub reference: Option<String>,
    /// Count: the model.
    #[serde(default)]
    pub family: Family,
    /// Count: an exposure variable (time at risk, population), entered as log(exposure).
    #[serde(default)]
    pub exposure: Option<String>,
    #[serde(default = "default_level")]
    pub level: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    #[default]
    Poisson,
    NegativeBinomial,
}

fn logistic(z: f64) -> f64 {
    if z >= 0.0 { 1.0 / (1.0 + (-z).exp()) } else { let e = z.exp(); e / (1.0 + e) }
}

/// Newton-Raphson on a concave log-likelihood given by `f(params) -> (loglik, gradient, hessian)`, with step halving.
/// A log-likelihood with its gradient and Hessian.
type LogLik<'a> = &'a dyn Fn(&[f64]) -> Option<(f64, Vec<f64>, Mat)>;

fn newton(mut par: Vec<f64>, f: LogLik) -> Option<(Vec<f64>, f64, Mat)> {
    let (mut ll, mut g, mut h) = f(&par)?;
    for _ in 0..200 {
        let neg: Mat = h.iter().map(|r| r.iter().map(|v| -v).collect()).collect();
        let inv = invert(&neg)?;
        let step: Vec<f64> = inv.iter().map(|r| r.iter().zip(&g).map(|(a, b)| a * b).sum()).collect();
        let mut t = 1.0;
        let mut moved = false;
        for _ in 0..40 {
            let cand: Vec<f64> = par.iter().zip(&step).map(|(p, s)| p + t * s).collect();
            if let Some((lc, gc, hc)) = f(&cand) {
                if lc >= ll - 1e-12 {
                    let done = (lc - ll).abs() < 1e-12 * (1.0 + ll.abs()) && step.iter().map(|s| s.abs()).fold(0.0, f64::max) * t < 1e-10;
                    par = cand;
                    ll = lc;
                    g = gc;
                    h = hc;
                    moved = true;
                    if done {
                        return Some((par, ll, h));
                    }
                    break;
                }
            }
            t /= 2.0;
        }
        if !moved {
            break;
        }
        if g.iter().map(|v| v.abs()).fold(0.0, f64::max) < 1e-10 {
            break;
        }
    }
    Some((par, ll, h))
}

/// Shared setup: the design (with the outcome's categories or counts as `y`) and the bundle.
struct Setup {
    pr: regression::Prepared,
    levels: Vec<Level>,
}

fn setup(ds: &Dataset, spec: &Spec, categorical: bool) -> Result<Setup> {
    check_level(spec.level)?;
    let reg = regression::Spec { dependent: spec.dependent.clone(), covariates: spec.covariates.clone(), factors: spec.factors.clone(), level: spec.level, blocks: Vec::new(), interactions: Vec::new(), center: false, plots: false, robust: Default::default(), bayes: Default::default(), prior_scale: Default::default() };
    if categorical {
        let (ys, levels) = categories(ds, &spec.dependent)?;
        let cells: Vec<Slot<f64>> = ys.iter().map(|s| match s { Slot::Valid(i) => Slot::Valid(*i as f64), Slot::Empty => Slot::Empty, Slot::UserMissing => Slot::UserMissing }).collect();
        Ok(Setup { pr: regression::prepare_with(ds, &reg, cells)?, levels })
    } else {
        Ok(Setup { pr: regression::prepare(ds, &reg)?, levels: Vec::new() })
    }
}

/// A coefficient table row with the odds ratio (or rate ratio) and its interval.
#[allow(clippy::too_many_arguments)]
fn coef_cells(b: &mut ResultBundle, key: &str, name: &str, est: f64, se: f64, level: f64, ratio: bool, out: &mut Vec<Vec<Cell>>, first: Option<&str>) {
    let z = est / se;
    let pv = norm_two_sided(z);
    let q = norm_quantile(1.0 - (1.0 - level) / 2.0);
    for (f, v) in [("b", est), ("se", se), ("z", z), ("p", pv)] {
        set(b, format!("{key}.{f}"), v);
    }
    let mut row = Vec::new();
    if let Some(c) = first {
        row.push(text(c));
    }
    row.extend([text(name), num(est, 4), num(se, 4), num(z, 3), p(pv)]);
    if ratio {
        let (r, lo, hi) = (est.exp(), (est - q * se).exp(), (est + q * se).exp());
        set(b, format!("{key}.ratio"), r);
        set(b, format!("{key}.ratio.lower"), lo);
        set(b, format!("{key}.ratio.upper"), hi);
        row.extend([num(r, 3), num(lo, 3), num(hi, 3)]);
    }
    out.push(row);
}

fn pseudo_r2(b: &mut ResultBundle, ll: f64, ll0: f64, n: usize) {
    let mcf = 1.0 - ll / ll0;
    let cs = 1.0 - (2.0 * (ll0 - ll) / n as f64).exp();
    let nag = cs / (1.0 - (2.0 * ll0 / n as f64).exp());
    set(b, "mcfadden", mcf);
    set(b, "nagelkerke", nag);
}

// ---------------------------------------------------------------- Ordinal

/// The proportional-odds log-likelihood with gradient and Hessian, parameters (θ₁..θ_{K−1}, β).
fn ordinal_ll(par: &[f64], y: &[usize], x: &[Vec<f64>], k: usize) -> Option<(f64, Vec<f64>, Mat)> {
    let nt = k - 1;
    let np = par.len();
    let theta = &par[..nt];
    if theta.windows(2).any(|w| w[1] <= w[0]) {
        return None;
    }
    let beta = &par[nt..];
    let mut ll = 0.0;
    let mut g = vec![0.0; np];
    let mut h = vec![vec![0.0; np]; np];
    let fpdf = |z: f64| { let f = logistic(z); f * (1.0 - f) };
    let fd = |z: f64| { let f = logistic(z); f * (1.0 - f) * (1.0 - 2.0 * f) };
    for (i, &yi) in y.iter().enumerate() {
        let eta: f64 = beta.iter().enumerate().map(|(j, bj)| bj * x[j][i]).sum();
        let (fa, ga, da) = if yi < nt { let a = theta[yi] - eta; (logistic(a), fpdf(a), fd(a)) } else { (1.0, 0.0, 0.0) };
        let (fc, gc, dc) = if yi > 0 { let c = theta[yi - 1] - eta; (logistic(c), fpdf(c), fd(c)) } else { (0.0, 0.0, 0.0) };
        let pr = fa - fc;
        if pr <= 0.0 {
            return None;
        }
        ll += pr.ln();
        // da/du and dc/du as sparse vectors.
        let mut va = vec![0.0; np];
        let mut vc = vec![0.0; np];
        if yi < nt {
            va[yi] = 1.0;
        }
        if yi > 0 {
            vc[yi - 1] = 1.0;
        }
        for j in 0..beta.len() {
            va[nt + j] = -x[j][i];
            vc[nt + j] = -x[j][i];
        }
        let dp: Vec<f64> = (0..np).map(|u| ga * va[u] - gc * vc[u]).collect();
        for u in 0..np {
            g[u] += dp[u] / pr;
            for v in 0..np {
                h[u][v] += (da * va[u] * va[v] - dc * vc[u] * vc[v]) / pr - dp[u] * dp[v] / (pr * pr);
            }
        }
    }
    Some((ll, g, h))
}

pub fn ordinal(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    let Setup { pr, levels } = setup(ds, spec, true)?;
    let n = pr.y.len();
    let present: Vec<usize> = (0..levels.len()).filter(|&l| pr.y.iter().any(|v| *v as usize == l)).collect();
    let y: Vec<usize> = pr.y.iter().map(|v| present.iter().position(|p| *p == *v as usize).expect("present")).collect();
    let k = present.len();
    let lv = level_text(spec.level);
    let note = format!("Ordinal logistic regression (proportional-odds cumulative logit model, as R's MASS::polr: logit P(Y ≤ j) = θⱼ − xβ, so a positive coefficient means higher categories are more likely), fitted by maximum likelihood. Odds ratios with {lv} Wald intervals. The categories are taken in code order. Cases were used when every variable had a valid value. Computed by PrismStatistics.");
    let mut b = common::bundle(ORDINAL, METHOD_VERSION, format!("Ordinal Logistic Regression: {}", pr.y_label), serde_json::to_value(spec).expect("a spec always serializes"), pr.tally.accounting(pr.total, n), note);
    set_text(&mut b, "y.label", pr.y_label.clone());
    set(&mut b, "n", n as f64);
    set(&mut b, "k", k as f64);
    set(&mut b, "level", spec.level);
    if k < 3 {
        fail(&mut b, format!("An ordinal outcome needs at least three categories among the cases used; {} has {k}. With two, use binary logistic regression.", pr.y_label));
        return Ok(b);
    }
    // Start: thresholds from the cumulative proportions, β = 0.
    let mut start = Vec::new();
    let mut cum = 0.0;
    for j in 0..k - 1 {
        cum += y.iter().filter(|&&v| v == j).count() as f64 / n as f64;
        start.push((cum / (1.0 - cum)).ln());
    }
    let null_ll: f64 = (0..k).map(|j| { let c = y.iter().filter(|&&v| v == j).count() as f64; if c > 0.0 { c * (c / n as f64).ln() } else { 0.0 } }).sum();
    start.extend(vec![0.0; pr.columns.len()]);
    let Some((par, ll, h)) = newton(start, &|p| ordinal_ll(p, &y, &pr.columns, k)) else {
        fail(&mut b, "The model could not be fitted: a predictor may separate the categories perfectly, or the predictors are collinear.");
        return Ok(b);
    };
    let Some(vcov) = invert(&h.iter().map(|r| r.iter().map(|v| -v).collect()).collect()) else {
        fail(&mut b, "The information matrix is singular, so standard errors can't be computed.");
        return Ok(b);
    };
    let mut rows = Vec::new();
    for (j, name) in pr.names.iter().enumerate() {
        let i = k - 1 + j;
        set_text(&mut b, format!("term.{}.label", j + 1), name.clone());
        coef_cells(&mut b, &format!("term.{}", j + 1), name, par[i], vcov[i][i].sqrt(), spec.level, true, &mut rows, None);
    }
    b.tables.push(Table {
        id: "coefficients".into(),
        title: "Coefficients".into(),
        columns: vec!["Predictor".into(), "b".into(), "SE".into(), "z".into(), "p".into(), "Odds ratio".into(), format!("{lv} CI lower"), format!("{lv} CI upper")],
        rows,
        footnotes: vec!["An odds ratio above 1 means higher values of the predictor go with higher categories of the outcome.".into()],
    });
    let mut rows_t = Vec::new();
    for j in 0..k - 1 {
        let name = format!("{} | {}", levels[present[j]].label, levels[present[j + 1]].label);
        set(&mut b, format!("theta.{}", j + 1), par[j]);
        rows_t.push(vec![text(name), num(par[j], 4), num(vcov[j][j].sqrt(), 4)]);
    }
    b.tables.push(Table { id: "thresholds".into(), title: "Thresholds".into(), columns: vec!["Between".into(), "θ".into(), "SE".into()], rows: rows_t, footnotes: Vec::new() });
    model_fit(&mut b, ll, null_ll, pr.columns.len() as f64, n);
    b.diagnostics.push(not_assessed("parallel", "Proportional odds", "The model assumes each predictor has the same effect at every threshold. Compare with a multinomial model, or fit binary models at each cut, to check."));
    Ok(b)
}

/// The likelihood-ratio test against the intercept-only model, pseudo R² and AIC.
fn model_fit(b: &mut ResultBundle, ll: f64, ll0: f64, df: f64, n: usize) {
    let chi = 2.0 * (ll - ll0);
    let pv = chisq_sf(chi, df);
    for (k, v) in [("loglik", ll), ("loglik.null", ll0), ("chi2", chi), ("df", df), ("p", pv)] {
        set(b, k, v);
    }
    pseudo_r2(b, ll, ll0, n);
    b.tables.push(Table {
        id: "fit".into(),
        title: "Model fit".into(),
        columns: vec!["−2 log-likelihood".into(), "Chi-square".into(), "df".into(), "p".into(), "McFadden R²".into(), "Nagelkerke R²".into()],
        rows: vec![vec![num(-2.0 * ll, 3), num(chi, 3), num(df, 0), p(pv), num(1.0 - ll / ll0, 3), num(b.number("nagelkerke").unwrap_or(f64::NAN), 3)]],
        footnotes: vec!["The chi-square compares the model with one that has no predictors.".into()],
    });
}

// ---------------------------------------------------------------- Multinomial

/// The multinomial log-likelihood with gradient and Hessian; parameters are, for each non-reference category in turn,
/// the intercept then the slopes.
fn multinom_ll(par: &[f64], y: &[usize], x: &[Vec<f64>], k: usize) -> Option<(f64, Vec<f64>, Mat)> {
    let q = x.len() + 1;
    let np = par.len();
    let mut ll = 0.0;
    let mut g = vec![0.0; np];
    let mut h = vec![vec![0.0; np]; np];
    let n = y.len();
    for i in 0..n {
        let xi: Vec<f64> = std::iter::once(1.0).chain(x.iter().map(|c| c[i])).collect();
        let etas: Vec<f64> = (0..k - 1).map(|c| (0..q).map(|j| par[c * q + j] * xi[j]).sum()).collect();
        let mx = etas.iter().copied().fold(0.0f64, f64::max);
        let denom = (-mx).exp() + etas.iter().map(|e| (e - mx).exp()).sum::<f64>();
        let probs: Vec<f64> = etas.iter().map(|e| (e - mx).exp() / denom).collect();
        let p_ref = (-mx).exp() / denom;
        let yi = y[i];
        ll += if yi == 0 { p_ref.ln() } else { probs[yi - 1].ln() };
        for c in 0..k - 1 {
            let ind = if yi == c + 1 { 1.0 } else { 0.0 };
            for j in 0..q {
                g[c * q + j] += (ind - probs[c]) * xi[j];
            }
            for d in 0..k - 1 {
                let w = if c == d { probs[c] * (1.0 - probs[c]) } else { -probs[c] * probs[d] };
                for j in 0..q {
                    for l in 0..q {
                        h[c * q + j][d * q + l] -= w * xi[j] * xi[l];
                    }
                }
            }
        }
    }
    Some((ll, g, h))
}

pub fn multinomial(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    let Setup { pr, levels } = setup(ds, spec, true)?;
    let n = pr.y.len();
    let mut present: Vec<usize> = (0..levels.len()).filter(|&l| pr.y.iter().any(|v| *v as usize == l)).collect();
    let k = present.len();
    let lv = level_text(spec.level);
    let mut b = common::bundle(MULTINOMIAL, METHOD_VERSION, format!("Multinomial Logistic Regression: {}", pr.y_label), serde_json::to_value(spec).expect("a spec always serializes"), pr.tally.accounting(pr.total, n), String::new());
    set_text(&mut b, "y.label", pr.y_label.clone());
    set(&mut b, "n", n as f64);
    set(&mut b, "k", k as f64);
    set(&mut b, "level", spec.level);
    if k < 3 {
        b.method_note = "Multinomial logistic regression.".into();
        fail(&mut b, format!("A multinomial outcome needs at least three categories among the cases used; {} has {k}. With two, use binary logistic regression.", pr.y_label));
        return Ok(b);
    }
    // The reference category goes first.
    if let Some(r) = &spec.reference {
        let pos = present.iter().position(|&l| levels[l].key() == *r || levels[l].label == *r).ok_or_else(|| Error::Invalid(format!("{r} is not a category of {} among the cases used.", pr.y_label)))?;
        let refl = present.remove(pos);
        present.insert(0, refl);
    }
    let ref_label = levels[present[0]].label.clone();
    set_text(&mut b, "reference.label", ref_label.clone());
    b.method_note = format!("Multinomial logistic regression (baseline-category logits against {ref_label}, as R's nnet::multinom), fitted by maximum likelihood. Odds ratios with {lv} Wald intervals; each predictor also has a likelihood-ratio test. Cases were used when every variable had a valid value. Computed by PrismStatistics.");
    let y: Vec<usize> = pr.y.iter().map(|v| present.iter().position(|p| *p == *v as usize).expect("present")).collect();
    let q = pr.columns.len() + 1;
    let fit = |cols: &[Vec<f64>]| -> Option<(Vec<f64>, f64, Mat)> {
        let qq = cols.len() + 1;
        newton(vec![0.0; (k - 1) * qq], &|p| multinom_ll(p, &y, cols, k))
    };
    let null_ll: f64 = (0..k).map(|j| { let c = y.iter().filter(|&&v| v == j).count() as f64; if c > 0.0 { c * (c / n as f64).ln() } else { 0.0 } }).sum();
    let Some((par, ll, h)) = fit(&pr.columns) else {
        fail(&mut b, "The model could not be fitted: a predictor may separate the categories perfectly, or the predictors are collinear.");
        return Ok(b);
    };
    let Some(vcov) = invert(&h.iter().map(|r| r.iter().map(|v| -v).collect()).collect()) else {
        fail(&mut b, "The information matrix is singular (often complete separation), so standard errors can't be computed.");
        return Ok(b);
    };
    let mut rows = Vec::new();
    for c in 0..k - 1 {
        let cl = levels[present[c + 1]].label.clone();
        set_text(&mut b, format!("cat.{}.label", c + 1), cl.clone());
        for j in 0..q {
            let i = c * q + j;
            let name = if j == 0 { "(Intercept)".to_string() } else { pr.names[j - 1].clone() };
            set_text(&mut b, format!("cat.{}.term.{j}.label", c + 1), name.clone());
            coef_cells(&mut b, &format!("cat.{}.term.{j}", c + 1), &name, par[i], vcov[i][i].sqrt(), spec.level, j > 0, &mut rows, Some(if j == 0 { &cl } else { "" }));
            if j == 0 {
                // The intercept has no odds ratio; pad the row.
                if let Some(r) = rows.last_mut() {
                    r.extend([Cell::Empty, Cell::Empty, Cell::Empty]);
                }
            }
        }
    }
    b.tables.push(Table {
        id: "coefficients".into(),
        title: format!("Coefficients (each category against {ref_label})"),
        columns: vec!["Category".into(), "Predictor".into(), "b".into(), "SE".into(), "z".into(), "p".into(), "Odds ratio".into(), format!("{lv} CI lower"), format!("{lv} CI upper")],
        rows,
        footnotes: vec![format!("Odds ratios compare the odds of each category with {ref_label}.")],
    });
    // Likelihood-ratio test of each predictor.
    let mut rows_lr = Vec::new();
    for (pi, pl) in pr.pred_labels.iter().enumerate() {
        let keep: Vec<Vec<f64>> = pr.columns.iter().zip(&pr.owner).filter(|(_, o)| **o != pi).map(|(c, _)| c.clone()).collect();
        let dropped = pr.columns.len() - keep.len();
        if let Some((_, l0, _)) = fit(&keep) {
            let chi = 2.0 * (ll - l0);
            let dfv = (dropped * (k - 1)) as f64;
            let pv = chisq_sf(chi, dfv);
            set(&mut b, format!("lr.{}.chi2", pi + 1), chi);
            set(&mut b, format!("lr.{}.p", pi + 1), pv);
            set_text(&mut b, format!("lr.{}.label", pi + 1), pl.clone());
            rows_lr.push(vec![text(pl), num(chi, 3), num(dfv, 0), p(pv)]);
        }
    }
    b.tables.push(Table { id: "lr".into(), title: "Likelihood-ratio tests".into(), columns: vec!["Predictor".into(), "Chi-square".into(), "df".into(), "p".into()], rows: rows_lr, footnotes: vec!["Each compares the full model with one that leaves out that predictor.".into()] });
    model_fit(&mut b, ll, null_ll, ((k - 1) * (q - 1)) as f64, n);
    Ok(b)
}

// ---------------------------------------------------------------- Count

/// log Γ, from the shared crate.
fn lgamma(x: f64) -> f64 {
    prismqda_stats::dist::ln_gamma(x)
}

fn digamma(mut x: f64) -> f64 {
    let mut r = 0.0;
    while x < 6.0 {
        r -= 1.0 / x;
        x += 1.0;
    }
    let f = 1.0 / (x * x);
    r + x.ln() - 0.5 / x - f * (1.0 / 12.0 - f * (1.0 / 120.0 - f * (1.0 / 252.0 - f * (1.0 / 240.0 - f / 132.0))))
}

fn trigamma(mut x: f64) -> f64 {
    let mut r = 0.0;
    while x < 6.0 {
        r += 1.0 / (x * x);
        x += 1.0;
    }
    let f = 1.0 / (x * x);
    r + 1.0 / x + f / 2.0 + f / x * (1.0 / 6.0 - f * (1.0 / 30.0 - f * (1.0 / 42.0 - f / 30.0)))
}

/// IRLS for a log-link GLM with variance μ + μ²/θ (θ = ∞ is Poisson). Returns β (intercept first), its covariance and μ.
fn irls(y: &[f64], x: &[Vec<f64>], offset: &[f64], theta: f64, start: Option<Vec<f64>>) -> Option<(Vec<f64>, Mat, Vec<f64>)> {
    let n = y.len();
    let q = x.len() + 1;
    let row = |i: usize| -> Vec<f64> { std::iter::once(1.0).chain(x.iter().map(|c| c[i])).collect() };
    let mut eta: Vec<f64> = match &start {
        Some(b) => (0..n).map(|i| row(i).iter().zip(b).map(|(a, c)| a * c).sum::<f64>() + offset[i]).collect(),
        None => y.iter().map(|v| (v + 0.1).ln()).collect(),
    };
    let mut beta = vec![0.0; q];
    let mut xtwx_inv = vec![vec![0.0; q]; q];
    let mut dev_old = f64::INFINITY;
    for _ in 0..100 {
        let mu: Vec<f64> = eta.iter().map(|e| e.exp()).collect();
        let w: Vec<f64> = mu.iter().map(|m| if theta.is_finite() { m / (1.0 + m / theta) } else { *m }).collect();
        let z: Vec<f64> = (0..n).map(|i| eta[i] - offset[i] + (y[i] - mu[i]) / mu[i]).collect();
        let mut xtwx = vec![vec![0.0; q]; q];
        let mut xtwz = vec![0.0; q];
        for i in 0..n {
            let r = row(i);
            for a in 0..q {
                xtwz[a] += w[i] * r[a] * z[i];
                for c in 0..q {
                    xtwx[a][c] += w[i] * r[a] * r[c];
                }
            }
        }
        xtwx_inv = invert(&xtwx)?;
        beta = xtwx_inv.iter().map(|r| r.iter().zip(&xtwz).map(|(a, c)| a * c).sum()).collect();
        eta = (0..n).map(|i| row(i).iter().zip(&beta).map(|(a, c)| a * c).sum::<f64>() + offset[i]).collect();
        let mu: Vec<f64> = eta.iter().map(|e| e.exp()).collect();
        let dev = deviance(y, &mu, theta);
        if (dev - dev_old).abs() < 1e-12 * (dev.abs() + 0.1) {
            break;
        }
        dev_old = dev;
    }
    let mu = eta.iter().map(|e| e.exp()).collect();
    Some((beta, xtwx_inv, mu))
}

fn deviance(y: &[f64], mu: &[f64], theta: f64) -> f64 {
    y.iter()
        .zip(mu)
        .map(|(&yi, &m)| {
            let t = if yi > 0.0 { yi * (yi / m).ln() } else { 0.0 };
            if theta.is_finite() { 2.0 * (t - (yi + theta) * ((yi + theta) / (m + theta)).ln()) } else { 2.0 * (t - (yi - m)) }
        })
        .sum()
}

fn count_ll(y: &[f64], mu: &[f64], theta: f64) -> f64 {
    y.iter()
        .zip(mu)
        .map(|(&yi, &m)| {
            if theta.is_finite() {
                lgamma(theta + yi) - lgamma(theta) - lgamma(yi + 1.0) + theta * (theta / (theta + m)).ln() + yi * (m / (theta + m)).ln()
            } else {
                yi * m.ln() - m - lgamma(yi + 1.0)
            }
        })
        .sum()
}

/// MASS::theta.ml: the maximum likelihood θ given μ.
fn theta_ml(y: &[f64], mu: &[f64]) -> f64 {
    let n = y.len() as f64;
    let mut t = n / y.iter().zip(mu).map(|(yi, m)| (yi / m - 1.0).powi(2)).sum::<f64>();
    for _ in 0..100 {
        let score: f64 = y.iter().zip(mu).map(|(&yi, &m)| digamma(t + yi) - digamma(t) + t.ln() + 1.0 - (t + m).ln() - (yi + t) / (m + t)).sum();
        let info: f64 = y.iter().zip(mu).map(|(&yi, &m)| -trigamma(t + yi) + trigamma(t) - 1.0 / t + 2.0 / (m + t) - (yi + t) / ((m + t) * (m + t))).sum();
        let step = score / info;
        t += step;
        if t <= 0.0 {
            t = 1e-4;
        }
        if step.abs() < 1e-12 * t {
            break;
        }
    }
    t
}

pub fn count(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    let Setup { mut pr, .. } = setup(ds, spec, false)?;
    // The exposure, on the same cases (cases without it are dropped too).
    let mut offset = vec![0.0; pr.y.len()];
    let mut exposure_label = None;
    if let Some(e) = &spec.exposure {
        let cells = numeric(ds, e)?;
        let keep: Vec<bool> = pr.used.iter().map(|&r| matches!(cells[r], Slot::Valid(v) if v > 0.0)).collect();
        if keep.iter().any(|k| !k) {
            let idx: Vec<usize> = (0..pr.used.len()).filter(|&i| keep[i]).collect();
            pr.y = idx.iter().map(|&i| pr.y[i]).collect();
            pr.columns = pr.columns.iter().map(|c| idx.iter().map(|&i| c[i]).collect()).collect();
            pr.used = idx.iter().map(|&i| pr.used[i]).collect();
        }
        offset = pr.used.iter().map(|&r| cells[r].value().expect("checked").ln()).collect();
        exposure_label = Some(ds.variable(e)?.display_name().to_string());
    }
    let n = pr.y.len();
    let nb = spec.family == Family::NegativeBinomial;
    let lv = level_text(spec.level);
    let note = format!(
        "{} regression with a log link{}, fitted by {}. Rate ratios with {lv} Wald intervals. Cases were used when every variable had a valid value. Computed by PrismStatistics.",
        if nb { "Negative binomial" } else { "Poisson" },
        exposure_label.as_ref().map_or(String::new(), |e| format!(" and log({e}) as an offset, so coefficients are effects on the rate per unit of {e}")),
        if nb { "iteratively reweighted least squares alternating with the maximum likelihood estimate of θ (as R's MASS::glm.nb); the variance is μ + μ²/θ" } else { "iteratively reweighted least squares (as R's glm)" }
    );
    let mut b = common::bundle(COUNT, METHOD_VERSION, format!("{} Regression: {}", if nb { "Negative Binomial" } else { "Poisson" }, pr.y_label), serde_json::to_value(spec).expect("a spec always serializes"), pr.tally.accounting(pr.total, n), note);
    set_text(&mut b, "y.label", pr.y_label.clone());
    set_text(&mut b, "family", if nb { "negative binomial" } else { "Poisson" });
    set(&mut b, "n", n as f64);
    set(&mut b, "level", spec.level);
    if pr.y.iter().any(|v| *v < 0.0 || v.fract() != 0.0) {
        fail(&mut b, format!("{} must be counts: whole numbers of zero or more.", pr.y_label));
        return Ok(b);
    }
    let fit = |cols: &[Vec<f64>]| -> Option<(Vec<f64>, Mat, Vec<f64>, f64)> {
        let (mut beta, mut v, mut mu) = irls(&pr.y, cols, &offset, f64::INFINITY, None)?;
        if !nb {
            return Some((beta, v, mu, f64::INFINITY));
        }
        let mut theta = theta_ml(&pr.y, &mu);
        for _ in 0..50 {
            let (b2, v2, m2) = irls(&pr.y, cols, &offset, theta, Some(beta.clone()))?;
            let t2 = theta_ml(&pr.y, &m2);
            let done = (t2 - theta).abs() < 1e-10 * theta && b2.iter().zip(&beta).all(|(a, c)| (a - c).abs() < 1e-10);
            beta = b2;
            v = v2;
            mu = m2;
            theta = t2;
            if done {
                break;
            }
        }
        Some((beta, v, mu, theta))
    };
    let Some((beta, vcov, mu, theta)) = fit(&pr.columns) else {
        fail(&mut b, "The model could not be fitted: the predictors may be collinear, or the counts are all zero.");
        return Ok(b);
    };
    let ll = count_ll(&pr.y, &mu, theta);
    let mut rows = Vec::new();
    for j in 0..beta.len() {
        let name = if j == 0 { "(Intercept)".to_string() } else { pr.names[j - 1].clone() };
        set_text(&mut b, format!("term.{j}.label"), name.clone());
        coef_cells(&mut b, &format!("term.{j}"), &name, beta[j], vcov[j][j].sqrt(), spec.level, j > 0, &mut rows, None);
        if j == 0 {
            if let Some(r) = rows.last_mut() {
                r.extend([Cell::Empty, Cell::Empty, Cell::Empty]);
            }
        }
    }
    b.tables.push(Table {
        id: "coefficients".into(),
        title: "Coefficients".into(),
        columns: vec!["Predictor".into(), "b".into(), "SE".into(), "z".into(), "p".into(), "Rate ratio".into(), format!("{lv} CI lower"), format!("{lv} CI upper")],
        rows,
        footnotes: vec!["A rate ratio is the factor by which the expected count changes for a one-unit increase (or against the reference category).".into()],
    });
    let dev = deviance(&pr.y, &mu, theta);
    let df_res = (n - beta.len()) as f64;
    let pearson: f64 = pr.y.iter().zip(&mu).map(|(yi, m)| (yi - m).powi(2) / if theta.is_finite() { m + m * m / theta } else { *m }).sum();
    let k_par = beta.len() as f64 + if nb { 1.0 } else { 0.0 };
    let aic = -2.0 * ll + 2.0 * k_par;
    for (kk, v) in [("deviance", dev), ("df.residual", df_res), ("pearson", pearson), ("dispersion", pearson / df_res), ("loglik", ll), ("aic", aic)] {
        set(&mut b, kk, v);
    }
    if nb {
        set(&mut b, "theta", theta);
    }
    // The model against the intercept-only model (θ re-estimated for the negative binomial).
    if let Some((_, _, mu0, th0)) = fit(&[]) {
        let ll0 = count_ll(&pr.y, &mu0, th0);
        let chi = 2.0 * (ll - ll0);
        let dfm = (beta.len() - 1) as f64;
        set(&mut b, "chi2", chi);
        set(&mut b, "df", dfm);
        set(&mut b, "p", chisq_sf(chi, dfm));
    }
    let mut rows_f = vec![vec![text("Deviance"), num(dev, 3), num(df_res, 0)], vec![text("Pearson chi-square"), num(pearson, 3), num(df_res, 0)], vec![text("Log-likelihood"), num(ll, 3), Cell::Empty], vec![text("AIC"), num(aic, 3), Cell::Empty]];
    if nb {
        rows_f.push(vec![text("θ (dispersion)"), num(theta, 4), Cell::Empty]);
    }
    if let (Some(c), Some(d), Some(pv)) = (b.number("chi2"), b.number("df"), b.number("p")) {
        rows_f.push(vec![text(format!("Likelihood-ratio chi-square ({} df), p {}", apa::df(d), apa::p_clause(pv))), num(c, 3), Cell::Empty]);
    }
    b.tables.push(Table { id: "fit".into(), title: "Model fit".into(), columns: vec!["Statistic".into(), "Value".into(), "df".into()], rows: rows_f, footnotes: Vec::new() });
    let disp = pearson / df_res;
    if !nb {
        b.diagnostics.push(if disp > 1.5 {
            crate::result::Diagnostic { id: "dispersion".into(), title: "Overdispersion".into(), state: crate::result::DiagnosticState::ReviewNeeded, message: format!("The Pearson chi-square is {} times its degrees of freedom: the counts vary more than a Poisson model allows, so its standard errors are too small. A negative binomial model is one click away.", apa::num(disp, 2)) }
        } else {
            crate::result::Diagnostic { id: "dispersion".into(), title: "Overdispersion".into(), state: crate::result::DiagnosticState::NoConcernDetected, message: format!("Pearson chi-square / df = {}.", apa::num(disp, 2)) }
        });
    }
    Ok(b)
}
