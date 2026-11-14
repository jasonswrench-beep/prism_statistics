//! Maximum likelihood estimation (checklist C1b–C1g).
//!
//! The data enter as blocks: one per group for complete data (and, for full-information ML, one per missing-data
//! pattern). Each block holds its cases' covariance matrix (divisor n) and means over the variables it observes. The
//! function minimized is −2 log L / N,
//!
//!   f(θ) = Σ_b (n_b/N) [p_b log 2π + log|Σ_b| + tr(S_b Σ_b⁻¹) + (x̄_b − μ_b)ᵀ Σ_b⁻¹ (x̄_b − μ_b)],
//!
//! by Fisher scoring with step halving. Standard errors come from the expected information, N/2 times
//! H_kl = Σ_b (n_b/N) [tr(Σ⁻¹ ∂_kΣ Σ⁻¹ ∂_lΣ) + 2 ∂_kμᵀ Σ⁻¹ ∂_lμ], as lavaan's defaults.

use super::model::{Implied, Matrix, Model, Place};
use crate::analysis::linalg::{cholesky, invert, symmetric_eigen, Mat};

#[derive(Debug, Clone)]
pub struct Block {
    pub group: usize,
    /// Observed-variable indices this block sees.
    pub vars: Vec<usize>,
    pub n: f64,
    pub cov: Mat,
    pub mean: Vec<f64>,
    /// The cases' values over `vars` (for robust standard errors).
    pub rows: Vec<Vec<f64>>,
}

pub fn total_n(blocks: &[Block]) -> f64 {
    blocks.iter().map(|b| b.n).sum()
}

/// The places of every free parameter.
pub fn places(model: &Model) -> Vec<Vec<Place>> {
    let mut out = vec![Vec::new(); model.nfree];
    for p in &model.params {
        if let Some(k) = p.free {
            out[k].push(Place { group: p.group, mat: p.mat, i: p.i, j: p.j });
        }
    }
    out
}

fn sub(m: &Mat, vars: &[usize]) -> Mat {
    vars.iter().map(|&i| vars.iter().map(|&j| m[i][j]).collect()).collect()
}

fn log_det_inv(m: &Mat) -> Option<(f64, Mat)> {
    let l = cholesky(m)?;
    let ld = 2.0 * l.iter().enumerate().map(|(i, r)| r[i].ln()).sum::<f64>();
    Some((ld, invert(m)?))
}

const LN_2PI: f64 = 1.837_877_066_409_345_3;

/// f(θ), or None when an implied covariance matrix is not positive definite.
pub fn objective(model: &Model, blocks: &[Block], th: &[f64]) -> Option<f64> {
    let n = total_n(blocks);
    let imp: Vec<Implied> = (0..model.groups).map(|g| model.implied_full(g, th)).collect::<Option<_>>()?;
    let mut f = 0.0;
    for b in blocks {
        let im = &imp[b.group];
        let sig = sub(&im.full, &b.vars);
        let (ld, w) = log_det_inv(&sig)?;
        let pb = b.vars.len();
        let tr: f64 = b.cov.iter().enumerate().map(|(i, row)| row.iter().zip(&w).map(|(c, wr)| c * wr[i]).sum::<f64>()).sum();
        let mut q = 0.0;
        if model.meanstructure {
            let d: Vec<f64> = b.vars.iter().enumerate().map(|(k, &v)| b.mean[k] - im.mean[v]).collect();
            for i in 0..pb {
                for j in 0..pb {
                    q += d[i] * w[i][j] * d[j];
                }
            }
        }
        f += b.n / n * (pb as f64 * LN_2PI + ld + tr + q);
    }
    Some(f)
}

/// The gradient of f and its expected Hessian H, over the free parameters followed by `extra` place sets (for
/// modification indices).
pub fn gradient_info(model: &Model, blocks: &[Block], th: &[f64], extra: &[Vec<Place>]) -> Option<(Vec<f64>, Mat)> {
    gradient_inner(model, blocks, th, extra, true)
}

/// The gradient of f alone (no information matrix), for samplers that need it at every step.
pub fn gradient(model: &Model, blocks: &[Block], th: &[f64]) -> Option<Vec<f64>> {
    gradient_inner(model, blocks, th, &[], false).map(|x| x.0)
}

fn gradient_inner(model: &Model, blocks: &[Block], th: &[f64], extra: &[Vec<Place>], want_h: bool) -> Option<(Vec<f64>, Mat)> {
    let n = total_n(blocks);
    let p = model.p();
    let mut sets = places(model);
    sets.extend(extra.iter().cloned());
    let q = sets.len();
    let imp: Vec<Implied> = (0..model.groups).map(|g| model.implied_full(g, th)).collect::<Option<_>>()?;
    // Derivatives per group: (parameter, ∂Σ, ∂μ) for the parameters that sit in the group.
    let mut der: Vec<Vec<(usize, Mat, Vec<f64>)>> = vec![Vec::new(); model.groups];
    for (k, set) in sets.iter().enumerate() {
        for g in 0..model.groups {
            let here: Vec<&Place> = set.iter().filter(|pl| pl.group == g).collect();
            if here.is_empty() {
                continue;
            }
            let (mut dc, mut dm) = (vec![vec![0.0; p]; p], vec![0.0; p]);
            for pl in here {
                imp[g].derivative(pl, p, &mut dc, &mut dm);
            }
            der[g].push((k, dc, dm));
        }
    }
    let mut grad = vec![0.0; q];
    let mut h = vec![vec![0.0; q]; q];
    for b in blocks {
        let im = &imp[b.group];
        let sig = sub(&im.full, &b.vars);
        let w = invert(&sig)?;
        let pb = b.vars.len();
        let wt = b.n / n;
        let d: Vec<f64> = if model.meanstructure { b.vars.iter().enumerate().map(|(k, &v)| b.mean[k] - im.mean[v]).collect() } else { vec![0.0; pb] };
        // R = W (Σ − S − d dᵀ) W.
        let mid: Mat = (0..pb).map(|i| (0..pb).map(|j| sig[i][j] - b.cov[i][j] - d[i] * d[j]).collect()).collect();
        let wm: Mat = (0..pb).map(|i| (0..pb).map(|j| (0..pb).map(|k| w[i][k] * mid[k][j]).sum()).collect()).collect();
        let r: Mat = (0..pb).map(|i| (0..pb).map(|j| (0..pb).map(|k| wm[i][k] * w[k][j]).sum()).collect()).collect();
        let wd: Vec<f64> = (0..pb).map(|i| (0..pb).map(|j| w[i][j] * d[j]).sum()).collect();
        let mut mk: Vec<(usize, Mat, Vec<f64>, Vec<f64>)> = Vec::new();
        for (k, dc, dm) in &der[b.group] {
            let dcs: Mat = b.vars.iter().map(|&i| b.vars.iter().map(|&j| dc[i][j]).collect()).collect();
            let dms: Vec<f64> = b.vars.iter().map(|&i| dm[i]).collect();
            let mut g = 0.0;
            for i in 0..pb {
                for j in 0..pb {
                    g += r[i][j] * dcs[j][i];
                }
            }
            if model.meanstructure {
                g -= 2.0 * wd.iter().zip(&dms).map(|(a, c)| a * c).sum::<f64>();
            }
            grad[*k] += wt * g;
            if !want_h {
                continue;
            }
            let m: Mat = (0..pb).map(|i| (0..pb).map(|j| (0..pb).map(|l| w[i][l] * dcs[l][j]).sum()).collect()).collect();
            let wmu: Vec<f64> = (0..pb).map(|i| (0..pb).map(|j| w[i][j] * dms[j]).sum()).collect();
            mk.push((*k, m, wmu, dms));
        }
        for (a, (ka, ma, wa, _)) in mk.iter().enumerate() {
            for (kb, mb, _, dmb) in &mk[a..] {
                let mut t = 0.0;
                for i in 0..pb {
                    for j in 0..pb {
                        t += ma[i][j] * mb[j][i];
                    }
                }
                if model.meanstructure {
                    t += 2.0 * wa.iter().zip(dmb).map(|(x, y)| x * y).sum::<f64>();
                }
                h[*ka][*kb] += wt * t;
                if ka != kb {
                    h[*kb][*ka] += wt * t;
                }
            }
        }
    }
    Some((grad, h))
}

#[derive(Debug, Clone)]
pub struct Fit {
    pub theta: Vec<f64>,
    /// −2 log L / N at the estimates.
    pub f: f64,
    pub iterations: usize,
    pub converged: bool,
    /// The expected Hessian of f at the estimates.
    pub h: Mat,
}

pub enum FitError {
    Start,
    NotIdentified,
}

/// Fisher scoring from the model's current values.
pub fn fit(model: &Model, blocks: &[Block]) -> Result<Fit, FitError> {
    let mut th = model.theta();
    let Some(mut f) = objective(model, blocks, &th) else {
        return Err(FitError::Start);
    };
    let mut converged = false;
    let mut iterations = 0;
    for it in 0..2000 {
        iterations = it + 1;
        let Some((g, h)) = gradient_info(model, blocks, &th, &[]) else {
            return Err(FitError::Start);
        };
        if g.iter().all(|v| v.abs() < 1e-11) {
            converged = true;
            break;
        }
        let Some(hi) = invert(&h) else {
            return Err(FitError::NotIdentified);
        };
        let step: Vec<f64> = hi.iter().map(|r| -r.iter().zip(&g).map(|(a, b)| a * b).sum::<f64>()).collect();
        let slope: f64 = step.iter().zip(&g).map(|(a, b)| a * b).sum();
        let mut alpha = 1.0;
        let mut moved = false;
        while alpha > 1e-12 {
            let cand: Vec<f64> = th.iter().zip(&step).map(|(t, s)| t + alpha * s).collect();
            if let Some(fc) = objective(model, blocks, &cand) {
                if fc <= f + 1e-4 * alpha * slope.min(0.0) || (fc - f).abs() < 1e-15 {
                    let tiny = step.iter().all(|s| (alpha * s).abs() < 1e-12);
                    th = cand;
                    let df = f - fc;
                    f = fc;
                    moved = true;
                    if tiny || df.abs() < 1e-16 * (1.0 + f.abs()) && it > 5 {
                        converged = true;
                    }
                    break;
                }
            }
            alpha /= 2.0;
        }
        if !moved {
            converged = g.iter().all(|v| v.abs() < 1e-6);
            break;
        }
        if converged {
            break;
        }
    }
    let Some((_, h)) = gradient_info(model, blocks, &th, &[]) else {
        return Err(FitError::Start);
    };
    let (vals, _) = symmetric_eigen(&h);
    let (lo, hi) = vals.iter().fold((f64::INFINITY, 0.0f64), |(l, u), v| (l.min(*v), u.max(v.abs())));
    if model.nfree > 0 && lo <= 1e-10 * hi.max(1e-300) {
        return Err(FitError::NotIdentified);
    }
    Ok(Fit { theta: th, f, iterations, converged, h })
}

/// The covariance matrix of the estimates, (N/2 · H)⁻¹.
pub fn vcov(fit: &Fit, n: f64) -> Option<Mat> {
    let inv = invert(&fit.h)?;
    Some(inv.iter().map(|r| r.iter().map(|v| v * 2.0 / n).collect()).collect())
}

/// Starting values (lavaan-like): loadings by an instrumental ratio of covariances, factor variances from the
/// indicators' covariances, half the observed variance for residuals, zero for paths and covariances, and the sample
/// moments for fixed.x moments and intercepts. Values the user gave are kept.
pub fn start(model: &mut Model, cov: &[Mat], mean: &[Vec<f64>]) {
    let p = model.p();
    for g in 0..model.groups {
        let s = &cov[g];
        let var = |i: usize| s[i][i].max(1e-8);
        let mut psi = vec![0.0; model.t()];
        let mut lambda: Vec<(usize, usize, f64)> = Vec::new();
        // First-order factors.
        for (li, _) in model.lat.iter().enumerate() {
            let f = p + li;
            let rows: Vec<usize> = (0..model.params.len()).filter(|&k| model.params[k].group == g && model.params[k].op == "=~" && model.params[k].j == f).collect();
            let ind: Vec<usize> = rows.iter().map(|&k| model.params[k].i).filter(|&i| i < p).collect();
            if ind.is_empty() {
                continue;
            }
            let m = ind[0];
            let mut lam: Vec<f64> = Vec::new();
            for &i in &ind {
                let l = if i == m {
                    1.0
                } else {
                    let others: Vec<usize> = ind.iter().copied().filter(|&k| k != i && k != m).collect();
                    let (num, den): (f64, f64) = others.iter().fold((0.0, 0.0), |(a, b), &k| (a + s[i][k], b + s[m][k]));
                    if !others.is_empty() && den.abs() > 1e-8 && (num / den).abs() < 10.0 {
                        num / den
                    } else {
                        s[i][m].signum() * (var(i) / var(m)).sqrt()
                    }
                };
                lam.push(l);
            }
            let (mut num, mut den) = (0.0, 0.0);
            for a in 0..ind.len() {
                for b in a + 1..ind.len() {
                    num += s[ind[a]][ind[b]];
                    den += lam[a] * lam[b];
                }
            }
            let mut ps = if den.abs() > 1e-12 { num / den } else { 0.5 * var(m) };
            if ps.is_nan() || ps < 0.05 * var(m) {
                ps = 0.05 * var(m);
            }
            psi[f] = ps;
            for (a, &i) in ind.iter().enumerate() {
                lambda.push((i, f, lam[a]));
            }
        }
        // Higher-order factors: loadings 1, variance half that of the first indicator factor.
        for (li, _) in model.lat.iter().enumerate() {
            let f = p + li;
            if psi[f] == 0.0 {
                let first = model.params.iter().find(|q| q.group == g && q.op == "=~" && q.j == f).map(|q| q.i);
                psi[f] = first.map_or(1.0, |i| 0.5 * psi[i].max(0.05));
                for q in model.params.iter().filter(|q| q.group == g && q.op == "=~" && q.j == f) {
                    lambda.push((q.i, f, 1.0));
                }
            }
        }
        let endogenous = |model: &Model, v: usize| model.params.iter().any(|q| q.group == g && q.mat == Matrix::A && q.i == v);
        let has_lat_indicators = |model: &Model, f: usize| model.params.iter().any(|q| q.group == g && q.op == "=~" && q.j == f && q.i >= p);
        for k in 0..model.params.len() {
            let q = &model.params[k];
            if q.group != g {
                continue;
            }
            if q.exo {
                let v = match q.mat {
                    Matrix::S => s[q.i][q.j],
                    _ => mean[g][q.i],
                };
                model.params[k].value = v;
                continue;
            }
            if q.free.is_none() || q.start_given {
                continue;
            }
            let v = match q.mat {
                Matrix::A if q.op == "=~" => {
                    let raw = lambda.iter().find(|(i, f, _)| *i == q.i && *f == q.j).map_or(1.0, |x| x.2);
                    // With the factor variance fixed (std_lv), loadings carry the factor's scale.
                    let fixed_var = model.params.iter().any(|r| r.group == g && r.mat == Matrix::S && r.i == q.j && r.j == q.j && r.free.is_none());
                    if fixed_var {
                        raw * psi[q.j].sqrt()
                    } else {
                        raw
                    }
                }
                Matrix::A => 0.0,
                Matrix::S if q.i == q.j => {
                    if q.i < p {
                        if endogenous(model, q.i) {
                            0.5 * var(q.i)
                        } else {
                            var(q.i)
                        }
                    } else if has_lat_indicators(model, q.i) {
                        psi[q.i]
                    } else if model.params.iter().any(|r| r.group == g && r.op == "=~" && r.i == q.i) {
                        0.5 * psi[q.i].max(0.05)
                    } else {
                        psi[q.i].max(0.05)
                    }
                }
                Matrix::S => 0.0,
                Matrix::T => q.value,
                Matrix::M => {
                    if q.i < p {
                        mean[g][q.i]
                    } else {
                        0.0
                    }
                }
            };
            model.params[k].value = v;
        }
    }
    // Equal parameters share one value: the first.
    let th = model.theta();
    model.set_theta(&th);
}

/// The noncentral χ² distribution function, as a Poisson mixture of central ones.
pub fn pnchisq(x: f64, df: f64, ncp: f64) -> f64 {
    use prismqda_stats::dist::{chisq_cdf, ln_gamma};
    if ncp <= 0.0 {
        return chisq_cdf(x, df);
    }
    let half = ncp / 2.0;
    let top = (half + 40.0 * half.sqrt() + 60.0) as usize;
    let mut total = 0.0;
    for j in 0..=top {
        let lw = -half + j as f64 * half.ln() - ln_gamma(j as f64 + 1.0);
        total += lw.exp() * chisq_cdf(x, df + 2.0 * j as f64);
    }
    total.min(1.0)
}

/// The noncentrality λ at which P(χ²(df, λ) ≤ x) equals `target`, or 0 when it is below the target already at 0.
pub fn ncp_for(x: f64, df: f64, target: f64) -> f64 {
    if pnchisq(x, df, 0.0) <= target {
        return 0.0;
    }
    let mut hi = (x.max(1.0)) * 2.0 + 10.0;
    while pnchisq(x, df, hi) > target {
        hi *= 2.0;
    }
    let mut lo = 0.0;
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if pnchisq(x, df, mid) > target {
            lo = mid;
        } else {
            hi = mid;
        }
        if hi - lo < 1e-12 * (1.0 + hi) {
            break;
        }
    }
    0.5 * (lo + hi)
}
