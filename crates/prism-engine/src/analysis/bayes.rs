//! Bayes factors (checklist B9), as R's BayesFactor package computes them with its default priors:
//!
//! * t tests: the JZS (Cauchy) prior on the standardized effect, scale √2/2 by default (`ttest.tstat`).
//! * Pearson correlation: Ly, Verhagen and Wagenmakers's exact result for a stretched beta prior of width 1/3
//!   (`correlationBF`).
//! * Contingency tables: Gunel and Dickey's Bayes factors for joint multinomial, independent multinomial (one margin
//!   fixed) and Poisson sampling, prior concentration 1 (`contingencyTableBF`).
//! * One-way ANOVA: the fixed-effects g prior of Rouder et al. (2012), scale 1/2, integrated over g.
//! * Linear regression: the Zellner-Siow prior on the coefficients, scale √2/4 (`linearReg.R2stat`).
//!
//! BF₁₀ above 1 favors the alternative (an effect); below 1, the null. One-dimensional integrals are done on log g by
//! composite Simpson's rule over the whole range where the integrand matters.

use prismqda_stats::dist::ln_gamma;

use super::common::{num, set, set_text, text};
use super::linalg::{det, invert, Mat};
use crate::result::{ResultBundle, Table};

/// Adds BF₁₀ (and BF₀₁) to a result: the fields `bf10` and `bf.prior`, and a table.
pub fn add(b: &mut ResultBundle, log_bf10: f64, prior: &str) {
    let bf = log_bf10.exp();
    set(b, "bf10", bf);
    set(b, "bf.log", log_bf10);
    set_text(b, "bf.prior", prior);
    b.tables.push(Table {
        id: "bayes".into(),
        title: "Bayes factor".into(),
        columns: vec!["BF₁₀".into(), "BF₀₁".into(), "Evidence".into(), "Prior".into()],
        rows: vec![vec![num(bf, 3), num(1.0 / bf, 3), text(describe(bf)), text(prior)]],
        footnotes: vec!["BF₁₀ is how many times more likely the data are if there is an effect than if there is none (BF₀₁ = 1 / BF₁₀). Computed as R's BayesFactor package does with these priors.".into()],
    });
}

/// ∫ exp(h(u)) du over the real line, for a smooth unimodal log-integrand; returns the log of the integral.
fn log_integral(h: &dyn Fn(f64) -> f64) -> f64 {
    // Find the mode on a coarse grid, then integrate ±60 around it.
    let mut best = (f64::NEG_INFINITY, 0.0);
    let mut u = -60.0;
    while u <= 60.0 {
        let v = h(u);
        if v > best.0 {
            best = (v, u);
        }
        u += 0.25;
    }
    let (hmax, umode) = best;
    let (lo, hi) = (umode - 60.0, umode + 60.0);
    let steps = 24000usize;
    let w = (hi - lo) / steps as f64;
    let mut sum = 0.0;
    for k in 0..=steps {
        let uk = lo + w * k as f64;
        let c = if k == 0 || k == steps { 1.0 } else if k % 2 == 1 { 4.0 } else { 2.0 };
        sum += c * (h(uk) - hmax).exp();
    }
    hmax + (sum * w / 3.0).ln()
}

/// The log density of the inverse gamma (shape ½, scale s) at g = eᵘ, times the Jacobian g, so it integrates over u.
fn log_ig_half(u: f64, scale: f64) -> f64 {
    0.5 * scale.ln() - 0.5 * std::f64::consts::PI.ln() - 0.5 * u - scale / u.exp()
}

/// log BF₁₀ of a t test (JZS prior with scale `r`): `n` is the sample size (one sample or pairs) or n₁n₂/(n₁+n₂),
/// `nu` the degrees of freedom.
pub fn ttest_log_bf(t: f64, n: f64, nu: f64, r: f64) -> f64 {
    let null = -(nu + 1.0) / 2.0 * (1.0 + t * t / nu).ln();
    let h = |u: f64| {
        let g = u.exp();
        let a = 1.0 + n * g * r * r;
        -0.5 * a.ln() - (nu + 1.0) / 2.0 * (1.0 + t * t / (a * nu)).ln() + log_ig_half(u, 0.5)
    };
    log_integral(&h) - null
}

/// log BF₁₀ of Pearson's r with n pairs (stretched beta prior of width κ; BayesFactor's .bf10Exact).
pub fn correlation_log_bf(n: f64, r: f64, kappa: f64) -> f64 {
    if n <= 2.0 {
        return 0.0;
    }
    // log ₂F₁((n−1)/2, (n−1)/2; (n+2/κ)/2; r²) by its series, in logs.
    let (a, c, z) = ((n - 1.0) / 2.0, (n + 2.0 / kappa) / 2.0, r * r);
    let mut log_term = 0.0f64;
    let mut terms = vec![0.0];
    for k in 0..100_000 {
        let kf = k as f64;
        log_term += 2.0 * (a + kf).ln() - (c + kf).ln() - (kf + 1.0).ln() + z.ln();
        terms.push(log_term);
        if log_term < terms.iter().copied().fold(f64::NEG_INFINITY, f64::max) - 40.0 && kf > 2.0 * a {
            break;
        }
    }
    let m = terms.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let log_hyper = m + terms.iter().map(|t| (t - m).exp()).sum::<f64>().ln();
    let lbeta = ln_gamma(1.0 / kappa) * 2.0 - ln_gamma(2.0 / kappa);
    (1.0 - 2.0 / kappa) * 2f64.ln() + 0.5 * std::f64::consts::PI.ln() - lbeta + ln_gamma((n + 2.0 / kappa - 1.0) / 2.0) - ln_gamma((n + 2.0 / kappa) / 2.0) + log_hyper
}

fn ldirich(a: &[f64]) -> f64 {
    a.iter().map(|v| ln_gamma(*v)).sum::<f64>() - ln_gamma(a.iter().sum())
}

/// How a contingency table was sampled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Sampling {
    /// Only the total was fixed.
    #[default]
    JointMultinomial,
    /// Each row's total was fixed.
    Rows,
    /// Each column's total was fixed.
    Columns,
    /// Nothing was fixed (counts over a period).
    Poisson,
}

/// log BF₁₀ (dependence against independence) of an I × J table, Gunel and Dickey with concentration `a` per cell.
pub fn contingency_log_bf(y: &[Vec<f64>], sampling: Sampling, a: f64) -> f64 {
    // Independent multinomial with fixed columns is the row-fixed case of the transposed table.
    if sampling == Sampling::Columns {
        let t: Vec<Vec<f64>> = (0..y[0].len()).map(|j| y.iter().map(|r| r[j]).collect()).collect();
        return contingency_log_bf(&t, Sampling::Rows, a);
    }
    let i = y.len();
    let j = y[0].len();
    let (fi, fj) = (i as f64, j as f64);
    let cells: Vec<f64> = y.iter().flatten().copied().collect();
    let acell: Vec<f64> = vec![a; i * j];
    let ar: Vec<f64> = vec![a * fj; i];
    let ac: Vec<f64> = vec![a * fi; j];
    let yr: Vec<f64> = y.iter().map(|r| r.iter().sum()).collect();
    let yc: Vec<f64> = (0..j).map(|c| y.iter().map(|r| r[c]).sum()).collect();
    let yac: Vec<f64> = cells.iter().zip(&acell).map(|(u, v)| u + v).collect();
    let sub = |v: &[f64], d: f64| -> Vec<f64> { v.iter().map(|x| x - d).collect() };
    let add = |u: &[f64], v: &[f64]| -> Vec<f64> { u.iter().zip(v).map(|(x, z)| x + z).collect() };
    match sampling {
        Sampling::JointMultinomial => ldirich(&yac) + ldirich(&sub(&ar, fj - 1.0)) + ldirich(&sub(&ac, fi - 1.0)) - ldirich(&acell) - ldirich(&sub(&add(&yr, &ar), fj - 1.0)) - ldirich(&sub(&add(&yc, &ac), fi - 1.0)),
        Sampling::Rows => ldirich(&sub(&ac, fi - 1.0)) + ldirich(&ar) + ldirich(&yac) - ldirich(&sub(&add(&yc, &ac), fi - 1.0)) - ldirich(&add(&yr, &ar)) - ldirich(&acell),
        Sampling::Poisson => {
            let n: f64 = cells.iter().sum();
            let b = fi * fj * a / n;
            let df = (fi - 1.0) * (fj - 1.0);
            let suma = a * fi * fj;
            let ldirich1: f64 = cells.iter().map(|yv| ln_gamma(a) - ln_gamma(a + yv)).sum();
            ln_gamma(suma - df) - df * (1.0 + 1.0 / b).ln() - ln_gamma(n + suma - df) - ldirich(&sub(&add(&yr, &ar), fj - 1.0)) + ldirich(&sub(&ar, fj - 1.0)) - ldirich(&sub(&add(&yc, &ac), fi - 1.0)) + ldirich(&sub(&ac, fi - 1.0)) - ldirich1
        }
        Sampling::Columns => unreachable!(),
    }
}

/// log BF₁₀ of a one-way ANOVA (fixed effects, g prior with scale `r`), from the groups' values.
pub fn anova_log_bf(groups: &[Vec<f64>], r: f64) -> Option<f64> {
    let jn = groups.len();
    let y: Vec<f64> = groups.iter().flatten().copied().collect();
    let n = y.len();
    let nf = n as f64;
    let ybar = y.iter().sum::<f64>() / nf;
    let yc: Vec<f64> = y.iter().map(|v| v - ybar).collect();
    let syy: f64 = yc.iter().map(|v| v * v).sum();
    // Orthonormal sum-to-zero contrasts Q (J × (J−1)), X̃ = X Q, then centered.
    let q = super::anova_oneway::polynomial_contrasts(jn, jn - 1);
    let mut xt: Mat = Vec::with_capacity(n);
    for (gi, g) in groups.iter().enumerate() {
        for _ in g {
            xt.push(q.iter().map(|c| c[gi]).collect());
        }
    }
    let k = jn - 1;
    let means: Vec<f64> = (0..k).map(|c| xt.iter().map(|r| r[c]).sum::<f64>() / nf).collect();
    for row in xt.iter_mut() {
        for (v, m) in row.iter_mut().zip(&means) {
            *v -= m;
        }
    }
    let xtx: Mat = (0..k).map(|a| (0..k).map(|c| xt.iter().map(|r| r[a] * r[c]).sum()).collect()).collect();
    let xty: Vec<f64> = (0..k).map(|a| xt.iter().zip(&yc).map(|(r, v)| r[a] * v).sum()).collect();
    let h = |u: f64| -> f64 {
        let g = u.exp();
        let m: Mat = (0..k).map(|a| (0..k).map(|c| g * xtx[a][c] + if a == c { 1.0 } else { 0.0 }).collect()).collect();
        let (Some(inv), dm) = (invert(&m), det(&m)) else { return f64::NEG_INFINITY };
        let quad: f64 = (0..k).map(|a| (0..k).map(|c| xty[a] * inv[a][c] * xty[c]).sum::<f64>()).sum();
        let resid = (syy - g * quad) / syy;
        -0.5 * dm.ln() - (nf - 1.0) / 2.0 * resid.ln() + log_ig_half(u, r * r / 2.0)
    };
    Some(log_integral(&h))
}

/// log BF₁₀ of a linear regression with p predictors and R² (Zellner-Siow prior with scale `r`).
pub fn regression_log_bf(n: f64, p: f64, r2: f64, r: f64) -> f64 {
    let h = |u: f64| -> f64 {
        let g = u.exp();
        (n - p - 1.0) / 2.0 * (1.0 + g).ln() - (n - 1.0) / 2.0 * (1.0 + g * (1.0 - r2)).ln() + log_ig_half(u, r * r * n / 2.0)
    };
    log_integral(&h)
}

/// "BF₁₀ = 4.21, moderate evidence for an effect" in Jeffreys's (Lee and Wagenmakers's) words.
pub fn describe(bf10: f64) -> String {
    let (side, bf) = if bf10 >= 1.0 { ("for", bf10) } else { ("against", 1.0 / bf10) };
    let strength = if bf < 3.0 { "anecdotal" } else if bf < 10.0 { "moderate" } else if bf < 30.0 { "strong" } else if bf < 100.0 { "very strong" } else { "extreme" };
    format!("{strength} evidence {side} an effect")
}
