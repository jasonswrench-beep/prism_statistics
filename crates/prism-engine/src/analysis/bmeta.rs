//! Bayesian random-effects meta-analysis (checklist E-7), as R's bayesmeta computes it, by numerical integration.
//!
//! The normal-normal hierarchical model: yᵢ ~ N(θᵢ, σᵢ²), θᵢ ~ N(μ, τ²), with a uniform (improper) or normal prior for
//! μ and a uniform (improper), half-normal or half-Cauchy prior for τ. μ is integrated out analytically, giving the
//! marginal posterior of τ, which is integrated by adaptive Gauss-Kronrod quadrature (to a relative 1e-13) on τ =
//! c·u/(1 − u). Conditional on τ, μ, a new study's effect θ_new, and each study's θᵢ (its shrinkage estimate) are
//! normal, so their posteriors are normal mixtures over τ, evaluated on the same quadrature nodes. Each posterior is
//! summarized by its mode, median, mean, standard deviation and shortest 95% interval (bayesmeta's default).

use prismqda_stats::dist::norm_cdf;
use serde::{Deserialize, Serialize};

/// The prior for τ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TauPrior {
    #[default]
    Uniform,
    HalfNormal,
    HalfCauchy,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Priors {
    pub tau: TauPrior,
    /// The half-normal or half-Cauchy scale.
    pub tau_scale: f64,
    /// A normal prior for μ (mean, sd); uniform when None.
    pub mu: Option<(f64, f64)>,
}

/// mode, median, mean, sd, lower, upper.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Summary {
    pub mode: f64,
    pub median: f64,
    pub mean: f64,
    pub sd: f64,
    pub lower: f64,
    pub upper: f64,
}

pub struct Posterior {
    pub tau: Summary,
    pub mu: Summary,
    pub theta_new: Summary,
    /// Each study's shrinkage estimate.
    pub theta: Vec<Summary>,
    /// The typical within-study variance (for I²) and the posterior median of I².
    pub i2_median: f64,
    pub sigma2_hat: f64,
}

struct Model<'a> {
    y: &'a [f64],
    s2: &'a [f64],
    p: Priors,
}

impl Model<'_> {
    /// μ̂(τ), V(τ) and the log marginal likelihood (μ integrated out).
    fn cond(&self, tau: f64) -> (f64, f64, f64) {
        let t2 = tau * tau;
        let mut sw = 0.0;
        let mut swy = 0.0;
        let mut ld = 0.0;
        for (yi, si) in self.y.iter().zip(self.s2) {
            let w = 1.0 / (si + t2);
            sw += w;
            swy += w * yi;
            ld += (si + t2).ln();
        }
        let (prec, num) = match self.p.mu {
            Some((m, s)) => (sw + 1.0 / (s * s), swy + m / (s * s)),
            None => (sw, swy),
        };
        let mu = num / prec;
        let v = 1.0 / prec;
        let mut q: f64 = self.y.iter().zip(self.s2).map(|(yi, si)| (yi - mu).powi(2) / (si + t2)).sum();
        let mut extra = 0.0;
        if let Some((m, s)) = self.p.mu {
            q += (m - mu).powi(2) / (s * s);
            extra = -(s * s).ln() * 0.5;
        }
        let ll = -0.5 * ld + 0.5 * v.ln() - 0.5 * q + extra;
        (mu, v, ll)
    }

    fn log_prior(&self, tau: f64) -> f64 {
        let s = self.p.tau_scale;
        match self.p.tau {
            TauPrior::Uniform => 0.0,
            TauPrior::HalfNormal => (2.0 / s).ln() - 0.5 * (tau / s).powi(2) - 0.5 * std::f64::consts::TAU.ln(),
            TauPrior::HalfCauchy => (2.0 / (std::f64::consts::PI * s)).ln() - (1.0 + (tau / s).powi(2)).ln(),
        }
    }

    fn log_post(&self, tau: f64) -> f64 {
        self.log_prior(tau) + self.cond(tau).2
    }
}

const GK_X: [f64; 8] = [0.991_455_371_120_812_6, 0.949_107_912_342_758_5, 0.864_864_423_359_769_1, 0.741_531_185_599_394_4, 0.586_087_235_467_691_1, 0.405_845_151_377_397_2, 0.207_784_955_007_898_5, 0.0];
const GK_WK: [f64; 8] = [0.022_935_322_010_529_22, 0.063_092_092_629_978_55, 0.104_790_010_322_250_2, 0.140_653_259_715_525_9, 0.169_004_726_639_267_9, 0.190_350_578_064_785_4, 0.204_432_940_075_298_9, 0.209_482_141_084_727_8];
const GK_WG: [f64; 4] = [0.129_484_966_168_869_7, 0.279_705_391_489_276_7, 0.381_830_050_505_118_9, 0.417_959_183_673_469_4];

/// One Gauss-Kronrod 7-15 panel: (Kronrod estimate, error estimate, nodes, Kronrod weights).
fn gk(f: &dyn Fn(f64) -> f64, a: f64, b: f64) -> (f64, f64, Vec<(f64, f64)>) {
    let c = 0.5 * (a + b);
    let h = 0.5 * (b - a);
    let mut k = 0.0;
    let mut g = 0.0;
    let mut nodes = Vec::with_capacity(15);
    for i in 0..8 {
        let xs: Vec<f64> = if i == 7 { vec![c] } else { vec![c - h * GK_X[i], c + h * GK_X[i]] };
        for x in xs {
            let fx = f(x);
            k += GK_WK[i] * fx;
            nodes.push((x, GK_WK[i] * h));
            // Gauss nodes are the odd-indexed Kronrod nodes (1, 3, 5, and the center).
            if i % 2 == 1 {
                g += GK_WG[i / 2] * fx;
            }
        }
    }
    (k * h, ((k - g) * h).abs(), nodes)
}

/// Adaptive Gauss-Kronrod: the integral and the nodes with weights of the final partition.
fn integrate(f: &dyn Fn(f64) -> f64, a: f64, b: f64, tol: f64) -> (f64, Vec<(f64, f64)>) {
    // (start, end, estimate, error, nodes)
    type Panel = (f64, f64, f64, f64, Vec<(f64, f64)>);
    let mut panels: Vec<Panel> = Vec::new();
    let (v, e, n) = gk(f, a, b);
    panels.push((a, b, v, e, n));
    for _ in 0..4000 {
        let total: f64 = panels.iter().map(|p| p.2).sum();
        let err: f64 = panels.iter().map(|p| p.3).sum();
        if err <= tol * total.abs().max(1e-300) {
            break;
        }
        let (worst, _) = panels.iter().enumerate().fold((0, -1.0), |(bi, be), (i, p)| if p.3 > be { (i, p.3) } else { (bi, be) });
        let (pa, pb, _, _, _) = panels.swap_remove(worst);
        let m = 0.5 * (pa + pb);
        let (v1, e1, n1) = gk(f, pa, m);
        let (v2, e2, n2) = gk(f, m, pb);
        panels.push((pa, m, v1, e1, n1));
        panels.push((m, pb, v2, e2, n2));
    }
    let total = panels.iter().map(|p| p.2).sum();
    let nodes = panels.into_iter().flat_map(|p| p.4).collect();
    (total, nodes)
}

/// Brent-free bisection for an increasing function crossing zero in [lo, hi].
fn bisect(f: &dyn Fn(f64) -> f64, mut lo: f64, mut hi: f64) -> f64 {
    for _ in 0..200 {
        let m = 0.5 * (lo + hi);
        if f(m) < 0.0 {
            lo = m;
        } else {
            hi = m;
        }
        if hi - lo <= 1e-14 * hi.abs().max(1e-300) {
            break;
        }
    }
    0.5 * (lo + hi)
}

/// Golden-section minimum of f on [a, b].
fn golden(f: &dyn Fn(f64) -> f64, mut a: f64, mut b: f64) -> f64 {
    let r = 0.5 * (5f64.sqrt() - 1.0);
    let mut c = b - r * (b - a);
    let mut d = a + r * (b - a);
    let (mut fc, mut fd) = (f(c), f(d));
    for _ in 0..200 {
        if fc < fd {
            b = d;
            d = c;
            fd = fc;
            c = b - r * (b - a);
            fc = f(c);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + r * (b - a);
            fd = f(d);
        }
        if (b - a).abs() <= 1e-13 * (a.abs() + b.abs()).max(1e-300) {
            break;
        }
    }
    0.5 * (a + b)
}

/// A normal mixture Σ wⱼ N(mⱼ, vⱼ): its summaries.
fn mixture(w: &[f64], m: &[f64], v: &[f64], level: f64) -> Summary {
    let mean: f64 = w.iter().zip(m).map(|(a, b)| a * b).sum();
    let var: f64 = w.iter().zip(m.iter().zip(v)).map(|(a, (b, c))| a * (c + b * b)).sum::<f64>() - mean * mean;
    let sd = var.max(0.0).sqrt();
    let cdf = |x: f64| -> f64 { w.iter().zip(m.iter().zip(v)).map(|(a, (b, c))| a * norm_cdf((x - b) / c.sqrt())).sum() };
    let dens = |x: f64| -> f64 { w.iter().zip(m.iter().zip(v)).map(|(a, (b, c))| a * (-(x - b).powi(2) / (2.0 * c)).exp() / (std::f64::consts::TAU * c).sqrt()).sum() };
    let (lo0, hi0) = (mean - 40.0 * sd, mean + 40.0 * sd);
    let quant = |p: f64| bisect(&|x| cdf(x) - p, lo0, hi0);
    let median = quant(0.5);
    let mode = golden(&|x| -dens(x), mean - 5.0 * sd, mean + 5.0 * sd);
    let a = golden(&|a| quant(a + level) - quant(a), 1e-12, 1.0 - level - 1e-12);
    Summary { mode, median, mean, sd, lower: quant(a), upper: quant(a + level) }
}

/// The posterior analysis.
pub fn analyze(y: &[f64], s2: &[f64], p: Priors, level: f64) -> Option<Posterior> {
    let k = y.len();
    if k < 2 || s2.iter().any(|v| v.is_nan() || *v <= 0.0) {
        return None;
    }
    let model = Model { y, s2, p };
    // Scale for the transformation: the DerSimonian-Laird τ, or the typical within-study SD.
    let w0: Vec<f64> = s2.iter().map(|v| 1.0 / v).collect();
    let sw: f64 = w0.iter().sum();
    let sw2: f64 = w0.iter().map(|w| w * w).sum();
    let sigma2_hat = (k as f64 - 1.0) * sw / (sw * sw - sw2);
    let c = sigma2_hat.sqrt().max(1e-8);
    // log posterior at its maximum, for numerical stability.
    let lmax = {
        let t = golden(&|u: f64| -model.log_post(c * u / (1.0 - u)), 0.0, 0.999);
        model.log_post(c * t / (1.0 - t)).max(model.log_post(0.0))
    };
    let dens_u = |u: f64| -> f64 {
        if u >= 1.0 {
            return 0.0;
        }
        let tau = c * u / (1.0 - u);
        (model.log_post(tau) - lmax).exp() * c / (1.0 - u).powi(2)
    };
    let (z, nodes) = integrate(&dens_u, 0.0, 1.0, 1e-13);
    if z.is_nan() || z <= 0.0 || !z.is_finite() {
        return None;
    }
    // Quadrature over τ: nodes and posterior weights.
    let taus: Vec<f64> = nodes.iter().map(|(u, _)| c * u / (1.0 - u)).collect();
    let wts: Vec<f64> = nodes.iter().map(|(u, h)| h * dens_u(*u) / z).collect();
    let tau_dens = |tau: f64| -> f64 { (model.log_post(tau) - lmax).exp() / z };
    let tau_cdf = |tau: f64| -> f64 {
        if tau <= 0.0 {
            return 0.0;
        }
        let ub = tau / (c + tau);
        integrate(&dens_u, 0.0, ub, 1e-13).0 / z
    };
    let tau_q = |pr: f64| -> f64 {
        let mut hi = c;
        while tau_cdf(hi) < pr {
            hi *= 2.0;
            if hi > 1e12 {
                break;
            }
        }
        bisect(&|t| tau_cdf(t) - pr, 0.0, hi)
    };
    let tau_mean: f64 = taus.iter().zip(&wts).map(|(t, w)| t * w).sum();
    let tau_sd = (taus.iter().zip(&wts).map(|(t, w)| t * t * w).sum::<f64>() - tau_mean * tau_mean).max(0.0).sqrt();
    let tau_mode = {
        let u = golden(&|u: f64| -model.log_post(c * u / (1.0 - u)), 0.0, 0.999);
        let t = c * u / (1.0 - u);
        if tau_dens(0.0) >= tau_dens(t) { 0.0 } else { t }
    };
    let tau_len = |a: f64| tau_q(a + level) - tau_q(a);
    let a = if tau_dens(0.0) >= tau_dens(tau_q(level)) { 0.0 } else { golden(&tau_len, 0.0, 1.0 - level) };
    let tau = Summary { mode: tau_mode, median: tau_q(0.5), mean: tau_mean, sd: tau_sd, lower: if a == 0.0 { 0.0 } else { tau_q(a) }, upper: tau_q(a + level) };
    // Mixtures over τ.
    let conds: Vec<(f64, f64, f64)> = taus.iter().map(|t| model.cond(*t)).collect();
    let mu_m: Vec<f64> = conds.iter().map(|c| c.0).collect();
    let mu_v: Vec<f64> = conds.iter().map(|c| c.1).collect();
    let mu = mixture(&wts, &mu_m, &mu_v, level);
    let new_v: Vec<f64> = mu_v.iter().zip(&taus).map(|(v, t)| v + t * t).collect();
    let theta_new = mixture(&wts, &mu_m, &new_v, level);
    let theta: Vec<Summary> = (0..k)
        .map(|i| {
            let (m, v): (Vec<f64>, Vec<f64>) = taus
                .iter()
                .zip(&conds)
                .map(|(t, (mh, vm, _))| {
                    let b = s2[i] / (s2[i] + t * t);
                    (y[i] - b * (y[i] - mh), s2[i] * (1.0 - b) + b * b * vm)
                })
                .unzip();
            mixture(&wts, &m, &v, level)
        })
        .collect();
    let i2_median = {
        let t = tau.median;
        t * t / (t * t + sigma2_hat)
    };
    Some(Posterior { tau, mu, theta_new, theta, i2_median, sigma2_hat })
}
