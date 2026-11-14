//! Bayesian SEM (checklist C4g), with blavaan's default priors, sampled by the No-U-Turn sampler.
//!
//! Priors (blavaan's defaults): normal(0, 10) on loadings and regressions, normal(0, 32) on observed intercepts and
//! normal(0, 10) on latent means; gamma(1, 0.5) on every standard deviation; and a uniform prior on correlations (LKJ(1)
//! among factors, beta(1, 1) on (ρ + 1)/2 for residual covariances), within positive-definite covariance matrices. As in
//! blavaan, there is no mean structure unless intercepts are written, and observed predictors' moments are fixed.
//!
//! Sampling is on an unconstrained scale (log standard deviations, Fisher-z correlations, with the Jacobians in the
//! density). The likelihood's gradient is the analytic one of the ML fit function. Each chain starts near the ML
//! estimates and runs NUTS (Hoffman & Gelman, 2014) with step sizes tuned by dual averaging and a diagonal metric
//! estimated during warm-up, from a seeded generator. Reported: posterior means, standard deviations and 95% equal-tail
//! intervals, split-R̂ (PSRF), the posterior predictive p (χ² discrepancy), DIC and WAIC.

use prismqda_stats::rng::SplitMix64;

use super::estimate::{self, Block};
use super::model::{Matrix, Model};
use crate::analysis::linalg::{cholesky, Mat};

/// How a free parameter sits on the sampling scale.
#[derive(Debug, Clone, Copy)]
pub enum Kind {
    /// Loadings and regressions (prior sd 10), intercepts (prior sd 32 or 10).
    Normal(f64),
    /// A variance: θ = exp(2u).
    Var,
    /// A covariance between two variables: θ = tanh(u)·sd_a·sd_b, each sd from a free variance (its index) or fixed.
    Cov(Sd, Sd),
}

#[derive(Debug, Clone, Copy)]
pub enum Sd {
    Free(usize),
    Fixed(f64),
}

/// Each free parameter's kind, or None when the model has a parameter the sampler can't place.
pub fn kinds(model: &Model) -> Option<Vec<Kind>> {
    let mut out = Vec::new();
    for k in 0..model.nfree {
        let p = model.params.iter().find(|p| p.free == Some(k))?;
        out.push(match p.mat {
            Matrix::A => Kind::Normal(10.0),
            Matrix::M => Kind::Normal(if p.i < model.p() { 32.0 } else { 10.0 }),
            Matrix::S if p.i == p.j => Kind::Var,
            Matrix::S => {
                let sd = |v: usize| -> Option<Sd> {
                    let q = model.params.iter().find(|q| q.group == p.group && q.mat == Matrix::S && q.i == v && q.j == v)?;
                    Some(match q.free {
                        Some(f) => Sd::Free(f),
                        None => Sd::Fixed(q.value.max(0.0).sqrt()),
                    })
                };
                Kind::Cov(sd(p.i)?, sd(p.j)?)
            }
            Matrix::T => return None,
        });
    }
    Some(out)
}

pub struct Target<'a> {
    pub model: &'a Model,
    pub blocks: &'a [Block],
    pub n: f64,
    pub kinds: Vec<Kind>,
}

impl Target<'_> {
    pub fn theta(&self, u: &[f64]) -> Vec<f64> {
        let mut th = vec![0.0; u.len()];
        for (k, kind) in self.kinds.iter().enumerate() {
            th[k] = match kind {
                Kind::Normal(_) => u[k],
                Kind::Var => (2.0 * u[k]).exp(),
                Kind::Cov(..) => 0.0,
            };
        }
        for (k, kind) in self.kinds.iter().enumerate() {
            if let Kind::Cov(a, b) = kind {
                th[k] = u[k].tanh() * self.sd(a, u) * self.sd(b, u);
            }
        }
        th
    }

    fn sd(&self, s: &Sd, u: &[f64]) -> f64 {
        match s {
            Sd::Free(i) => u[*i].exp(),
            Sd::Fixed(v) => *v,
        }
    }

    /// The scale of each parameter: u from θ.
    pub fn unconstrain(&self, th: &[f64]) -> Option<Vec<f64>> {
        let mut u = vec![0.0; th.len()];
        for (k, kind) in self.kinds.iter().enumerate() {
            u[k] = match kind {
                Kind::Normal(_) => th[k],
                Kind::Var if th[k] > 0.0 => 0.5 * th[k].ln(),
                Kind::Var => return None,
                Kind::Cov(..) => 0.0,
            };
        }
        for (k, kind) in self.kinds.iter().enumerate() {
            if let Kind::Cov(a, b) = kind {
                let r = th[k] / (self.sd(a, &u) * self.sd(b, &u));
                if r.abs() >= 1.0 {
                    return None;
                }
                u[k] = r.atanh();
            }
        }
        Some(u)
    }

    /// The log posterior (up to a constant) and its gradient on the sampling scale.
    pub fn logp(&self, u: &[f64], grad: bool) -> Option<(f64, Vec<f64>)> {
        let th = self.theta(u);
        // Covariance matrices must be positive definite.
        for g in 0..self.model.groups {
            let (_, s, _) = self.model.matrices(g, &th);
            let used: Vec<usize> = (0..s.len()).filter(|&i| s[i][i] > 0.0).collect();
            let sub: Mat = used.iter().map(|&i| used.iter().map(|&j| s[i][j]).collect()).collect();
            cholesky(&sub)?;
        }
        let f = estimate::objective(self.model, self.blocks, &th)?;
        let mut lp = -self.n / 2.0 * f;
        for (k, kind) in self.kinds.iter().enumerate() {
            lp += match kind {
                Kind::Normal(sd) => -u[k] * u[k] / (2.0 * sd * sd),
                Kind::Var => -0.5 * u[k].exp() + u[k],
                Kind::Cov(..) => (1.0 - u[k].tanh().powi(2)).max(1e-300).ln(),
            };
        }
        if !grad {
            return Some((lp, Vec::new()));
        }
        let gf = estimate::gradient(self.model, self.blocks, &th)?;
        let gth: Vec<f64> = gf.iter().map(|g| -self.n / 2.0 * g).collect();
        let mut gu = vec![0.0; u.len()];
        for (k, kind) in self.kinds.iter().enumerate() {
            match kind {
                Kind::Normal(sd) => gu[k] += gth[k] - u[k] / (sd * sd),
                Kind::Var => gu[k] += gth[k] * 2.0 * th[k] - 0.5 * u[k].exp() + 1.0,
                Kind::Cov(a, b) => {
                    let r = u[k].tanh();
                    let (sa, sb) = (self.sd(a, u), self.sd(b, u));
                    gu[k] += gth[k] * (1.0 - r * r) * sa * sb - 2.0 * r;
                    // A covariance also moves with its variables' standard deviations.
                    for s in [a, b] {
                        if let Sd::Free(i) = s {
                            gu[*i] += gth[k] * th[k];
                        }
                    }
                }
            }
        }
        Some((lp, gu))
    }
}

pub struct Rng(SplitMix64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(SplitMix64::new(seed))
    }
    pub fn uniform(&mut self) -> f64 {
        ((self.0.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }
    pub fn normal(&mut self) -> f64 {
        let (a, b) = (self.uniform(), self.uniform());
        (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos()
    }
}

struct State {
    u: Vec<f64>,
    r: Vec<f64>,
    g: Vec<f64>,
    lp: f64,
}

fn kinetic(r: &[f64], minv: &[f64]) -> f64 {
    0.5 * r.iter().zip(minv).map(|(x, m)| x * x * m).sum::<f64>()
}

fn leapfrog(t: &Target, s: &State, eps: f64, minv: &[f64]) -> Option<State> {
    let mut r: Vec<f64> = s.r.iter().zip(&s.g).map(|(r, g)| r + 0.5 * eps * g).collect();
    let u: Vec<f64> = s.u.iter().zip(&r).zip(minv).map(|((u, r), m)| u + eps * m * r).collect();
    let (lp, g) = t.logp(&u, true)?;
    for (ri, gi) in r.iter_mut().zip(&g) {
        *ri += 0.5 * eps * gi;
    }
    Some(State { u, r, g, lp })
}

struct Tree {
    minus: State,
    plus: State,
    prop: State,
    n: f64,
    s: bool,
    alpha: f64,
    n_alpha: f64,
    divergent: bool,
}

fn no_uturn(minus: &State, plus: &State, minv: &[f64]) -> bool {
    let du: Vec<f64> = plus.u.iter().zip(&minus.u).map(|(a, b)| a - b).collect();
    let dot = |r: &[f64]| du.iter().zip(r).zip(minv).map(|((d, r), m)| d * r * m).sum::<f64>();
    dot(&minus.r) >= 0.0 && dot(&plus.r) >= 0.0
}

fn clone(s: &State) -> State {
    State { u: s.u.clone(), r: s.r.clone(), g: s.g.clone(), lp: s.lp }
}

#[allow(clippy::too_many_arguments)]
fn build(t: &Target, s: &State, logu: f64, v: f64, j: u32, eps: f64, h0: f64, minv: &[f64], rng: &mut Rng) -> Tree {
    if j == 0 {
        let next = leapfrog(t, s, v * eps, minv);
        let Some(next) = next else {
            return Tree { minus: clone(s), plus: clone(s), prop: clone(s), n: 0.0, s: false, alpha: 0.0, n_alpha: 1.0, divergent: true };
        };
        let h = next.lp - kinetic(&next.r, minv);
        let n = f64::from(u8::from(logu <= h));
        let ok = logu < h + 1000.0;
        let alpha = (h - h0).exp().min(1.0);
        return Tree { minus: clone(&next), plus: clone(&next), prop: next, n, s: ok, alpha: if alpha.is_finite() { alpha } else { 0.0 }, n_alpha: 1.0, divergent: !ok };
    }
    let mut tr = build(t, s, logu, v, j - 1, eps, h0, minv, rng);
    if tr.s {
        let edge = if v < 0.0 { clone(&tr.minus) } else { clone(&tr.plus) };
        let t2 = build(t, &edge, logu, v, j - 1, eps, h0, minv, rng);
        if v < 0.0 {
            tr.minus = t2.minus;
        } else {
            tr.plus = t2.plus;
        }
        if t2.n > 0.0 && rng.uniform() < t2.n / (tr.n + t2.n) {
            tr.prop = t2.prop;
        }
        tr.alpha += t2.alpha;
        tr.n_alpha += t2.n_alpha;
        tr.n += t2.n;
        tr.divergent |= t2.divergent;
        tr.s = t2.s && no_uturn(&tr.minus, &tr.plus, minv);
    }
    tr
}

/// One NUTS transition. Returns the new state, the mean acceptance statistic and whether a divergence happened.
fn transition(t: &Target, cur: &State, eps: f64, minv: &[f64], rng: &mut Rng) -> (State, f64, bool) {
    let r: Vec<f64> = minv.iter().map(|m| rng.normal() / m.sqrt()).collect();
    let start = State { u: cur.u.clone(), r, g: cur.g.clone(), lp: cur.lp };
    let h0 = start.lp - kinetic(&start.r, minv);
    let logu = h0 + rng.uniform().ln();
    let (mut minus, mut plus) = (clone(&start), clone(&start));
    let mut prop = clone(cur);
    let (mut n, mut s, mut j) = (1.0, true, 0);
    let (mut alpha, mut n_alpha, mut div) = (0.0, 0.0, false);
    while s && j < 10 {
        let v = if rng.uniform() < 0.5 { -1.0 } else { 1.0 };
        let edge = if v < 0.0 { clone(&minus) } else { clone(&plus) };
        let tr = build(t, &edge, logu, v, j, eps, h0, minv, rng);
        if v < 0.0 {
            minus = tr.minus;
        } else {
            plus = tr.plus;
        }
        alpha = tr.alpha;
        n_alpha = tr.n_alpha;
        div |= tr.divergent;
        if tr.s && rng.uniform() < (tr.n / n).min(1.0) {
            prop = tr.prop;
        }
        n += tr.n;
        s = tr.s && no_uturn(&minus, &plus, minv);
        j += 1;
    }
    let stat = if n_alpha > 0.0 { alpha / n_alpha } else { 0.0 };
    (State { r: Vec::new(), ..prop }, stat, div)
}

/// A chain's draws (on the θ scale), and its count of divergent transitions.
pub fn chain(t: &Target, start: &[f64], minv0: &[f64], warmup: usize, samples: usize, seed: u64) -> Option<(Vec<Vec<f64>>, usize)> {
    let mut rng = Rng::new(seed);
    let mut minv = minv0.to_vec();
    let q = start.len();
    // Jitter the start (on the sampling scale) by a tenth of a posterior SD.
    let mut cur = None;
    for _ in 0..50 {
        let u: Vec<f64> = start.iter().zip(&minv).map(|(s, m)| s + 0.1 * m.sqrt() * rng.normal()).collect();
        if let Some((lp, g)) = t.logp(&u, true) {
            cur = Some(State { u, r: Vec::new(), g, lp });
            break;
        }
    }
    let mut cur = cur?;
    // Dual averaging of the step size (Stan's constants).
    let find_eps = |cur: &State, minv: &[f64], rng: &mut Rng| -> f64 {
        let mut eps = 0.1;
        let r: Vec<f64> = minv.iter().map(|m| rng.normal() / m.sqrt()).collect();
        let s0 = State { u: cur.u.clone(), r, g: cur.g.clone(), lp: cur.lp };
        let h0 = s0.lp - kinetic(&s0.r, minv);
        let accept = |eps: f64| leapfrog(t, &s0, eps, minv).map_or(0.0, |n| (n.lp - kinetic(&n.r, minv) - h0).exp());
        let up = accept(eps) > 0.5;
        for _ in 0..50 {
            let a = accept(eps);
            if up && a <= 0.5 || !up && a > 0.5 {
                break;
            }
            eps = if up { eps * 2.0 } else { eps / 2.0 };
        }
        eps
    };
    let mut eps = find_eps(&cur, &minv, &mut rng);
    let (delta, gamma, t0, kappa) = (0.8, 0.05, 10.0, 0.75);
    let mut mu = (10.0 * eps).ln();
    let (mut hbar, mut log_eps_bar, mut m_count) = (0.0, 0.0, 0.0);
    let first = (warmup as f64 * 0.15) as usize;
    let last = (warmup as f64 * 0.75) as usize;
    let mut window: Vec<Vec<f64>> = Vec::new();
    let mut draws = Vec::with_capacity(samples);
    let mut divergent = 0;
    for it in 0..warmup + samples {
        if crate::job::step(it, warmup + samples) {
            return None;
        }
        let (next, stat, div) = transition(t, &cur, eps, &minv, &mut rng);
        cur = next;
        if it < warmup {
            m_count += 1.0;
            let w = 1.0 / (m_count + t0);
            hbar = (1.0 - w) * hbar + w * (delta - stat);
            let log_eps = mu - m_count.sqrt() / gamma * hbar;
            let wk = m_count.powf(-kappa);
            log_eps_bar = wk * log_eps + (1.0 - wk) * log_eps_bar;
            eps = log_eps.exp();
            if it >= first && it < last {
                window.push(cur.u.clone());
            }
            if it + 1 == last && window.len() > 10 {
                let n = window.len() as f64;
                for k in 0..q {
                    let m = window.iter().map(|w| w[k]).sum::<f64>() / n;
                    let var = window.iter().map(|w| (w[k] - m).powi(2)).sum::<f64>() / (n - 1.0);
                    minv[k] = (n / (n + 5.0)) * var + 1e-3 * 5.0 / (n + 5.0);
                }
                eps = find_eps(&cur, &minv, &mut rng);
                mu = (10.0 * eps).ln();
                hbar = 0.0;
                log_eps_bar = 0.0;
                m_count = 0.0;
            }
            if it + 1 == warmup {
                eps = log_eps_bar.exp();
            }
        } else {
            if div {
                divergent += 1;
            }
            draws.push(t.theta(&cur.u));
        }
    }
    Some((draws, divergent))
}

/// Split-R̂ (the potential scale reduction factor) of one quantity over chains.
pub fn split_rhat(chains: &[Vec<f64>]) -> f64 {
    let mut halves: Vec<&[f64]> = Vec::new();
    for c in chains {
        let h = c.len() / 2;
        halves.push(&c[..h]);
        halves.push(&c[h..2 * h]);
    }
    let m = halves.len() as f64;
    let n = halves[0].len() as f64;
    if n < 2.0 {
        return f64::NAN;
    }
    let means: Vec<f64> = halves.iter().map(|h| h.iter().sum::<f64>() / n).collect();
    let grand = means.iter().sum::<f64>() / m;
    let b = n / (m - 1.0) * means.iter().map(|x| (x - grand).powi(2)).sum::<f64>();
    let w = halves.iter().zip(&means).map(|(h, mn)| h.iter().map(|x| (x - mn).powi(2)).sum::<f64>() / (n - 1.0)).sum::<f64>() / m;
    if w <= 0.0 {
        return 1.0;
    }
    (((n - 1.0) / n * w + b / n) / w).sqrt()
}

// ------------------------------------------------------------------------------------------------- the analysis

use super::{core, prepare, Estimator, Missing, Spec};
use crate::analysis::common::{self, *};
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Diagnostic, DiagnosticState, ResultBundle, Table};

fn quantile(sorted: &[f64], p: f64) -> f64 {
    let h = (sorted.len() - 1) as f64 * p;
    let lo = h.floor() as usize;
    let hi = (lo + 1).min(sorted.len() - 1);
    sorted[lo] + (h - lo as f64) * (sorted[hi] - sorted[lo])
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    if spec.group.is_some() || spec.missing != Missing::Listwise || spec.cluster.is_some() {
        return Err(Error::Invalid("Bayesian SEM here fits one group to complete cases.".into()));
    }
    if !(1..=8).contains(&spec.chains) || spec.samples < 100 || spec.burnin < 100 {
        return Err(Error::Invalid("Use 1 to 8 chains with at least 100 warm-up and 100 sampled draws each.".into()));
    }
    // As blavaan: no mean structure unless intercepts are written; observed predictors' moments fixed at the sample
    // values (fixed.x), as in its bsem.
    let ml = Spec { estimator: Estimator::Ml, modindices: false, bootstrap: 0, reliability: false, ..spec.clone() };
    let pr = prepare(ds, &ml)?;
    let n = estimate::total_n(&pr.blocks);
    let lv = level_text(0.95);
    let note = format!(
        "Bayesian SEM with blavaan's default priors (normal(0, 10) on loadings and paths, normal(0, 32) on intercepts, gamma(1, 0.5) on standard deviations, uniform on correlations), sampled by the No-U-Turn sampler: {} chains of {} draws after {} of warm-up (seed {}), started near the ML estimates. Posterior means, standard deviations and {lv} equal-tail intervals; PSRF is the split R̂. PPP is the posterior predictive p of the χ² discrepancy; DIC and WAIC as blavaan. Computed by PrismStatistics.",
        spec.chains, spec.samples, spec.burnin, spec.seed
    );
    let mut b = common::bundle(super::SEM, super::METHOD_VERSION, "Bayesian Structural Equation Model".into(), serde_json::to_value(spec).expect("a spec always serializes"), pr.tally.clone().accounting(pr.total, n as usize), note);
    set(&mut b, "n", n);
    set_text(&mut b, "estimator", "Bayes");
    let c = match core(&pr, &ml) {
        Ok(c) => c,
        Err(why) => {
            fail(&mut b, why);
            return Ok(b);
        }
    };
    let model = c.model;
    let Some(kinds) = kinds(&model) else {
        fail(&mut b, "The model has a parameter the sampler can't place (such as a threshold).");
        return Ok(b);
    };
    let target = Target { model: &model, blocks: &pr.blocks, n, kinds };
    let Some(start) = target.unconstrain(&c.fit.theta) else {
        fail(&mut b, "The ML estimates have a negative variance (a Heywood case), so the sampler has no valid starting point.");
        return Ok(b);
    };
    // The starting metric: the ML variances carried to the sampling scale.
    let th = &c.fit.theta;
    let minv0: Vec<f64> = target
        .kinds
        .iter()
        .enumerate()
        .map(|(k, kind)| {
            let d = match kind {
                Kind::Normal(_) => 1.0,
                Kind::Var => 2.0 * th[k],
                Kind::Cov(..) => {
                    let r = start[k].tanh();
                    (1.0 - r * r) * (th[k] / r.abs().max(1e-6)).abs().max(1e-6)
                }
            };
            (c.v[k][k] / (d * d)).clamp(1e-8, 100.0)
        })
        .collect();
    let mut chains: Vec<Vec<Vec<f64>>> = Vec::new();
    let mut divergent = 0;
    for ch in 0..spec.chains {
        if crate::job::cancelled() {
            break;
        }
        match chain(&target, &start, &minv0, spec.burnin, spec.samples, spec.seed.wrapping_add(ch as u64 * 7919)) {
            Some((d, dv)) => {
                chains.push(d);
                divergent += dv;
            }
            None => {
                fail(&mut b, "A chain could not start: no valid point was found near the ML estimates.");
                return Ok(b);
            }
        }
    }
    let draws: Vec<&Vec<f64>> = chains.iter().flatten().collect();
    let total_draws = draws.len();
    // Each parameter's value (and standardized value) in each draw.
    let est: Vec<Vec<f64>> = draws.iter().map(|t| super::unstandardized(&model, t)).collect();
    let std: Vec<Vec<f64>> = draws.iter().map(|t| super::standardized(&model, t)).collect();
    let names: Vec<String> = model.params.iter().map(|p| p.name()).chain(model.defined.iter().map(|(d, e)| format!("{d} := {}", super::show(e)))).collect();
    let free: Vec<bool> = model.params.iter().map(|p| p.free.is_some()).chain(model.defined.iter().map(|_| true)).collect();
    let mut rows = Vec::new();
    let mut max_rhat: f64 = 1.0;
    for (k, name) in names.iter().enumerate() {
        let key = format!("par.{}", k + 1);
        set_text(&mut b, format!("{key}.label"), name.clone());
        let vals: Vec<f64> = est.iter().map(|e| e[k]).collect();
        let mean = vals.iter().sum::<f64>() / vals.len() as f64;
        let smean = std.iter().map(|e| e[k]).sum::<f64>() / std.len() as f64;
        set(&mut b, format!("{key}.mean"), mean);
        set(&mut b, format!("{key}.std"), smean);
        if !free[k] {
            rows.push(vec![text(name.clone()), num(mean, 3), text(""), text(""), text(""), text(""), num(smean, 3)]);
            continue;
        }
        let sd = (vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (vals.len() as f64 - 1.0)).sqrt();
        let mut sorted = vals.clone();
        sorted.sort_by(f64::total_cmp);
        let (lo, hi) = (quantile(&sorted, 0.025), quantile(&sorted, 0.975));
        let per: Vec<Vec<f64>> = (0..chains.len()).map(|ci| est[ci * spec.samples..(ci + 1) * spec.samples].iter().map(|e| e[k]).collect()).collect();
        let rhat = split_rhat(&per);
        if rhat.is_finite() {
            max_rhat = max_rhat.max(rhat);
        }
        for (f, v) in [("sd", sd), ("lower", lo), ("upper", hi), ("rhat", rhat)] {
            set(&mut b, format!("{key}.{f}"), v);
        }
        rows.push(vec![text(name.clone()), num(mean, 3), num(sd, 3), num(lo, 3), num(hi, 3), num(rhat, 3), num(smean, 3)]);
    }
    set(&mut b, "par.count", names.len() as f64);
    // Fit: DIC, WAIC (on up to 4000 draws) and the posterior predictive p.
    let thin = (total_draws / 4000).max(1);
    let used: Vec<&Vec<f64>> = draws.iter().step_by(thin).copied().collect();
    let dev = |t: &[f64]| estimate::objective(&model, &pr.blocks, t).map(|f| n * f);
    let mean_theta: Vec<f64> = (0..model.nfree).map(|k| draws.iter().map(|d| d[k]).sum::<f64>() / total_draws as f64).collect();
    let dbar = used.iter().filter_map(|t| dev(t)).sum::<f64>() / used.len() as f64;
    let dhat = dev(&mean_theta).unwrap_or(f64::NAN);
    let p_dic = dbar - dhat;
    let block = &pr.blocks[0];
    let p = model.p();
    let mut ll = vec![Vec::with_capacity(used.len()); block.rows.len()];
    let mut rng = Rng::new(spec.seed ^ 0xBA7E5);
    let (mut exceed, mut ppp_n) = (0.0f64, 0.0f64);
    let ln2pi = std::f64::consts::TAU.ln();
    // As blavaan with fixed predictors: likelihoods are conditional on them. Their moments are fixed at the sample
    // values, so their own log-likelihood is a constant taken out of DIC and WAIC, and the replicated χ² leaves out
    // the predictors' own (random) misfit.
    let xs = &model.xvars;
    let px = xs.len() as f64;
    let sub = |m: &Mat| -> Mat { xs.iter().map(|&i| xs.iter().map(|&j| m[i][j]).collect()).collect() };
    let logl_x = if xs.is_empty() { 0.0 } else { super::log_det(&sub(&block.cov)).map_or(f64::NAN, |ld| -n / 2.0 * (px * ln2pi + ld + px)) };
    for t in &used {
        let Some(im) = model.implied_full(0, t) else { continue };
        let sig: Mat = (0..p).map(|i| (0..p).map(|j| im.full[i][j]).collect()).collect();
        let Some(l) = cholesky(&sig) else { continue };
        let Some(w) = crate::analysis::linalg::invert(&sig) else { continue };
        let ld = 2.0 * l.iter().enumerate().map(|(i, r)| r[i].ln()).sum::<f64>();
        // The predictors' implied moments (their sample values) for the conditional pointwise likelihood.
        let llx: Option<(f64, Mat)> = if xs.is_empty() {
            None
        } else {
            let sx = sub(&sig);
            match (super::log_det(&sx), crate::analysis::linalg::invert(&sx)) {
                (Some(a), Some(b)) => Some((a, b)),
                _ => continue,
            }
        };
        let mu: Vec<f64> = if model.meanstructure { im.mean[..p].to_vec() } else { block.mean.clone() };
        for (i, row) in block.rows.iter().enumerate() {
            let d: Vec<f64> = row.iter().zip(&mu).map(|(x, m)| x - m).collect();
            let q: f64 = (0..p).map(|a| d[a] * (0..p).map(|c| w[a][c] * d[c]).sum::<f64>()).sum();
            ll[i].push(-0.5 * (p as f64 * ln2pi + ld + q));
            if let Some(lx) = llx.as_ref() {
                let dx: Vec<f64> = xs.iter().map(|&a| d[a]).collect();
                let qx: f64 = (0..xs.len()).map(|a| dx[a] * (0..xs.len()).map(|c| lx.1[a][c] * dx[c]).sum::<f64>()).sum();
                *ll[i].last_mut().expect("just pushed") += 0.5 * (px * ln2pi + lx.0 + qx);
            }
        }
        // Replicated data from this draw.
        let nn = block.rows.len();
        let rep: Vec<Vec<f64>> = (0..nn)
            .map(|_| {
                let z: Vec<f64> = (0..p).map(|_| rng.normal()).collect();
                (0..p).map(|a| mu[a] + (0..=a).map(|c| l[a][c] * z[c]).sum::<f64>()).collect()
            })
            .collect();
        let rmean: Vec<f64> = (0..p).map(|j| rep.iter().map(|r| r[j]).sum::<f64>() / nn as f64).collect();
        let rcov: Mat = (0..p).map(|a| (0..p).map(|c| rep.iter().map(|r| (r[a] - rmean[a]) * (r[c] - rmean[c])).sum::<f64>() / nn as f64).collect()).collect();
        let sat = |s: &Mat| super::log_det(s).map(|ld| p as f64 * ln2pi + ld + p as f64);
        let rb = Block { group: 0, vars: (0..p).collect(), n: nn as f64, cov: rcov.clone(), mean: rmean, rows: Vec::new() };
        // The replicated predictors' own discrepancy (zero in the observed data, whose predictors fit exactly).
        let xmis = match llx.as_ref() {
            None => Some(0.0),
            Some((ldx, wx)) => {
                let sx = sub(&rcov);
                let tr: f64 = (0..xs.len()).map(|a| (0..xs.len()).map(|c| wx[a][c] * sx[c][a]).sum::<f64>()).sum();
                let mt: f64 = if model.meanstructure {
                    let dm: Vec<f64> = xs.iter().map(|&a| rb.mean[a] - mu[a]).collect();
                    (0..xs.len()).map(|a| dm[a] * (0..xs.len()).map(|c| wx[a][c] * dm[c]).sum::<f64>()).sum()
                } else {
                    0.0
                };
                super::log_det(&sx).map(|lds| ldx + tr + mt - lds - px)
            }
        };
        if let (Some(fo), Some(so), Some(fr), Some(sr), Some(xm)) = (estimate::objective(&model, &pr.blocks, t), sat(&block.cov), estimate::objective(&model, std::slice::from_ref(&rb), t), sat(&rcov), xmis) {
            ppp_n += 1.0;
            if fr - sr - xm >= fo - so {
                exceed += 1.0;
            }
        }
    }
    let (mut lppd, mut p_waic) = (0.0, 0.0);
    for li in &ll {
        let m = li.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let s = li.len() as f64;
        lppd += m + (li.iter().map(|x| (x - m).exp()).sum::<f64>() / s).ln();
        let mean = li.iter().sum::<f64>() / s;
        p_waic += li.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (s - 1.0);
    }
    let waic = -2.0 * (lppd - p_waic);
    let dhat = dhat + 2.0 * logl_x;
    let ppp = exceed / ppp_n.max(1.0);
    for (k, v) in [("fit.ppp", ppp), ("fit.dic", dhat + 2.0 * p_dic), ("fit.p_dic", p_dic), ("fit.waic", waic), ("fit.p_waic", p_waic), ("npar", model.nfree as f64), ("divergent", divergent as f64), ("rhat.max", max_rhat), ("draws", total_draws as f64)] {
        set(&mut b, k, v);
    }
    let small = |x: f64| crate::apa::no_leading_zero(x, 3);
    b.tables.push(Table {
        id: "fit".into(),
        title: "Model fit".into(),
        columns: vec!["Measure".into(), "Value".into()],
        rows: vec![
            vec![text("Posterior predictive p (PPP)"), text(small(ppp))],
            vec![text("DIC"), num(dhat + 2.0 * p_dic, 2)],
            vec![text("Effective parameters (DIC)"), num(p_dic, 2)],
            vec![text("WAIC"), num(waic, 2)],
            vec![text("Effective parameters (WAIC)"), num(p_waic, 2)],
            vec![text("Largest PSRF"), num(max_rhat, 3)],
            vec![text("Draws"), text(format!("{} chains × {}", spec.chains, spec.samples))],
            vec![text("Cases"), count(n as usize)],
        ],
        footnotes: vec!["A PPP near .5 means the model reproduces the data's covariances about as well as data simulated from it; values near 0 point to misfit. PSRF (split R̂) above 1.05 means the chains have not mixed.".into()],
    });
    b.tables.push(Table {
        id: "posterior".into(),
        title: "Posterior summary".into(),
        columns: vec!["Parameter".into(), "Mean".into(), "SD".into(), format!("{lv} interval lower"), format!("{lv} interval upper"), "PSRF".into(), "Standardized".into()],
        rows,
        footnotes: vec!["Parameters without a standard deviation are fixed.".into()],
    });
    b.diagnostics.push(if max_rhat <= 1.05 && divergent == 0 {
        Diagnostic { id: "convergence".into(), title: "Convergence".into(), state: DiagnosticState::NoConcernDetected, message: format!("Every PSRF is at most {} and no transition diverged.", crate::apa::num(max_rhat, 3)) }
    } else {
        Diagnostic { id: "convergence".into(), title: "Convergence".into(), state: DiagnosticState::ReviewNeeded, message: format!("The largest PSRF is {} and {divergent} transitions diverged; run longer chains before relying on the results.", crate::apa::num(max_rhat, 3)) }
    });
    b.diagnostics.push(common::size_diagnostic(n as usize, format!("{} complete cases", n as usize)));
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Column, Dataset};

    /// The log posterior's gradient (likelihood, priors and Jacobians) agrees with central differences.
    #[test]
    fn gradient_matches_finite_differences() {
        let mut ds = Dataset::new("t");
        let mut rng = Rng::new(3);
        let f: Vec<f64> = (0..60).map(|_| rng.normal()).collect();
        let g: Vec<f64> = (0..60).map(|_| rng.normal()).collect();
        for (k, name) in ["a", "b", "c", "d", "e", "h"].iter().enumerate() {
            let col: Vec<Option<f64>> = (0..60).map(|i| Some(if k < 3 { f[i] } else { 0.5 * f[i] + g[i] } * (1.0 + 0.2 * k as f64) + rng.normal())).collect();
            ds.add_variable(name, Column::Numeric(col)).unwrap();
        }
        let spec = Spec { model: "f =~ a + b + c\ng =~ d + e + h".into(), fixed_x: false, modindices: false, reliability: false, ..Spec::default() };
        let pr = prepare(&ds, &spec).unwrap();
        let c = core(&pr, &spec).ok().unwrap();
        let t = Target { model: &c.model, blocks: &pr.blocks, n: 60.0, kinds: kinds(&c.model).unwrap() };
        let mut u = t.unconstrain(&c.fit.theta).unwrap();
        for (k, x) in u.iter_mut().enumerate() {
            *x += 0.05 * ((k % 3) as f64 - 1.0);
        }
        let (_, gr) = t.logp(&u, true).unwrap();
        for k in 0..u.len() {
            let h = 1e-6;
            let (mut up, mut dn) = (u.clone(), u.clone());
            up[k] += h;
            dn[k] -= h;
            let num = (t.logp(&up, false).unwrap().0 - t.logp(&dn, false).unwrap().0) / (2.0 * h);
            assert!((num - gr[k]).abs() < 1e-5 * num.abs().max(1.0), "parameter {k}: {num} vs {}", gr[k]);
        }
    }
}
