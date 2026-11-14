//! Robust maximum likelihood (checklist C1h) and the information matrices full-information ML needs (C1i).
//!
//! * MLM (Satorra-Bentler): sandwich standard errors E⁻¹ (Δ'WΓWΔ) E⁻¹ / N with the expected information E = Δ'WΔ and
//!   Γ the asymptotic covariance of the sample moments from the raw data; the test is scaled by
//!   c = tr(UΓ)/df = [tr(WΓ) − tr(E⁻¹ Δ'WΓWΔ)]/df. W is the normal-theory weight at the model-implied moments
//!   (lavaan's default, h1.information = "structured"), or at the sample moments ("unstructured", which some other
//!   packages use).
//! * MLR (Yuan-Bentler, the variant that is lavaan's default for MLR): sandwich standard errors A⁻¹ B A⁻¹ / N with the
//!   observed information A and the cross-products B of casewise scores; the test is scaled by
//!   c = [tr(A₁⁻¹B₁) − tr(A⁻¹B)]/df, where A₁ and B₁ are the same two matrices for the unrestricted (saturated) model at
//!   its own estimates.
//!
//! All matrices here are per case (divided by N).

use super::estimate::{gradient_info, total_n, Block};
use super::model::{Implied, Model, Place};
use crate::analysis::linalg::{invert, Mat};

/// The unrestricted (saturated, "h1") moments of each group: the sample moments, or their EM estimates with missing
/// data.
#[derive(Debug, Clone)]
pub struct H1 {
    pub mean: Vec<Vec<f64>>,
    pub cov: Vec<Mat>,
}

fn matmul(a: &Mat, b: &Mat) -> Mat {
    let n = b.first().map_or(0, Vec::len);
    a.iter().map(|r| (0..n).map(|j| r.iter().zip(b).map(|(x, br)| x * br[j]).sum()).collect()).collect()
}

fn sub(m: &Mat, vars: &[usize]) -> Mat {
    vars.iter().map(|&i| vars.iter().map(|&j| m[i][j]).collect()).collect()
}

fn trace_ab(a: &Mat, b: &Mat) -> f64 {
    a.iter().enumerate().map(|(i, r)| r.iter().zip(b).map(|(x, br)| x * br[i]).sum::<f64>()).sum()
}

/// Per-parameter pieces for casewise scores in one block: M_k = W ∂Σ_k W, v_k = W ∂μ_k and tr(W ∂Σ_k).
struct ScoreParts {
    w: Mat,
    m: Vec<Mat>,
    v: Vec<Vec<f64>>,
    tr: Vec<f64>,
    /// ∂Σ_k and ∂μ_k on the block's variables.
    dc: Vec<Mat>,
    dm: Vec<Vec<f64>>,
}

/// W is the inverse of the model-implied covariance matrix, or of the sample one when `unstructured`.
fn score_parts(model: &Model, imp: &[Implied], b: &Block, sets: &[Vec<Place>], unstructured: bool) -> Option<ScoreParts> {
    let p = model.p();
    let im = &imp[b.group];
    let w = if unstructured { invert(&b.cov)? } else { invert(&sub(&im.full, &b.vars))? };
    let (mut m, mut v, mut tr) = (Vec::new(), Vec::new(), Vec::new());
    let (mut dcv, mut dmv) = (Vec::new(), Vec::new());
    for set in sets {
        let (mut dc, mut dm) = (vec![vec![0.0; p]; p], vec![0.0; p]);
        for pl in set.iter().filter(|pl| pl.group == b.group) {
            im.derivative(pl, p, &mut dc, &mut dm);
        }
        let dcs = sub(&dc, &b.vars);
        let dms: Vec<f64> = b.vars.iter().map(|&i| dm[i]).collect();
        let wd = matmul(&w, &dcs);
        tr.push((0..wd.len()).map(|i| wd[i][i]).sum());
        m.push(matmul(&wd, &w));
        v.push(w.iter().map(|r| r.iter().zip(&dms).map(|(a, c)| a * c).sum()).collect());
        dcv.push(dcs);
        dmv.push(dms);
    }
    Some(ScoreParts { w, m, v, tr, dc: dcv, dm: dmv })
}

fn quad(m: &Mat, d: &[f64]) -> f64 {
    m.iter().zip(d).map(|(r, di)| di * r.iter().zip(d).map(|(a, c)| a * c).sum::<f64>()).sum()
}

/// The casewise scores of the model parameters, as cross-products: (1/N) Σᵢ sᵢ sᵢᵀ (the MLR meat). Without a mean
/// structure the means are the sample means.
pub fn first_order(model: &Model, blocks: &[Block], th: &[f64]) -> Option<Mat> {
    let sets = super::estimate::places(model);
    let q = sets.len();
    let n = total_n(blocks);
    let imp: Vec<Implied> = (0..model.groups).map(|g| model.implied_full(g, th)).collect::<Option<_>>()?;
    let mut b0 = vec![vec![0.0; q]; q];
    for b in blocks {
        let sp = score_parts(model, &imp, b, &sets, false)?;
        let mu: Vec<f64> = if model.meanstructure { b.vars.iter().map(|&i| imp[b.group].mean[i]).collect() } else { b.mean.clone() };
        for row in &b.rows {
            let d: Vec<f64> = row.iter().zip(&mu).map(|(x, m)| x - m).collect();
            let s: Vec<f64> = (0..q).map(|k| 0.5 * (quad(&sp.m[k], &d) - sp.tr[k]) + if model.meanstructure { sp.v[k].iter().zip(&d).map(|(a, c)| a * c).sum::<f64>() } else { 0.0 }).collect();
            for a in 0..q {
                for c in 0..q {
                    b0[a][c] += s[a] * s[c] / n;
                }
            }
        }
    }
    Some(b0)
}

/// The observed information per case, A = −(1/N) ∂² log L = ½ ∂²f, by central differences of the analytic gradient.
pub fn observed_info(model: &Model, blocks: &[Block], th: &[f64]) -> Option<Mat> {
    let q = th.len();
    let mut h = vec![vec![0.0; q]; q];
    for k in 0..q {
        let step = 1e-5 * th[k].abs().max(1.0);
        let (mut up, mut dn) = (th.to_vec(), th.to_vec());
        up[k] += step;
        dn[k] -= step;
        let (gu, _) = gradient_info(model, blocks, &up, &[])?;
        let (gd, _) = gradient_info(model, blocks, &dn, &[])?;
        for j in 0..q {
            h[j][k] = (gu[j] - gd[j]) / (2.0 * step);
        }
    }
    Some((0..q).map(|i| (0..q).map(|j| 0.25 * (h[i][j] + h[j][i])).collect()).collect())
}

/// MLM: the expected information E = Δ'WΔ, the Satorra-Bentler meat Δ'WΓWΔ and tr(WΓ), complete data only, with W
/// at the model-implied moments or (`unstructured`) at the sample moments.
pub fn satorra_bentler(model: &Model, blocks: &[Block], th: &[f64], unstructured: bool) -> Option<(Mat, Mat, f64)> {
    let sets = super::estimate::places(model);
    let q = sets.len();
    let n = total_n(blocks);
    let imp: Vec<Implied> = (0..model.groups).map(|g| model.implied_full(g, th)).collect::<Option<_>>()?;
    let mut meat = vec![vec![0.0; q]; q];
    let mut e = vec![vec![0.0; q]; q];
    let mut tr_wg = 0.0;
    for b in blocks {
        let sp = score_parts(model, &imp, b, &sets, unstructured)?;
        let ng = b.n;
        for (row, (mk, vk)) in e.iter_mut().zip(sp.m.iter().zip(&sp.v)) {
            for (cell, (dcl, dml)) in row.iter_mut().zip(sp.dc.iter().zip(&sp.dm)) {
                let mean_part = if model.meanstructure { vk.iter().zip(dml).map(|(a, c)| a * c).sum::<f64>() } else { 0.0 };
                *cell += ng / n * (0.5 * trace_ab(mk, dcl) + mean_part);
            }
        }
        let wsws = trace_ab(&matmul(&sp.w, &b.cov), &matmul(&sp.w, &b.cov));
        let trs: Vec<f64> = sp.m.iter().map(|m| trace_ab(m, &b.cov)).collect();
        let wsw = matmul(&matmul(&sp.w, &b.cov), &sp.w);
        let mut gsum = 0.0;
        for row in &b.rows {
            let d: Vec<f64> = row.iter().zip(&b.mean).map(|(x, m)| x - m).collect();
            let s: Vec<f64> = (0..q).map(|k| 0.5 * (quad(&sp.m[k], &d) - trs[k]) + if model.meanstructure { sp.v[k].iter().zip(&d).map(|(a, c)| a * c).sum::<f64>() } else { 0.0 }).collect();
            for a in 0..q {
                for c in 0..q {
                    meat[a][c] += s[a] * s[c] / n;
                }
            }
            let dwd = quad(&sp.w, &d);
            gsum += 0.5 * (dwd * dwd - 2.0 * quad(&wsw, &d) + wsws) + if model.meanstructure { dwd } else { 0.0 };
        }
        // Each group's tr(W_g Γ_g), with Γ_g per case of the group.
        tr_wg += gsum / ng;
    }
    Some((e, meat, tr_wg))
}

/// Index pairs (a ≥ b) of the covariance parameters of the unrestricted model, after the means.
fn h1_layout(p: usize, means: bool) -> (usize, Vec<(usize, usize)>) {
    let mut cov = Vec::new();
    for a in 0..p {
        for b in 0..=a {
            cov.push((a, b));
        }
    }
    (if means { p } else { 0 }, cov)
}

/// ½ tr(W E_ab W E_cd) for symmetric unit matrices E (one entry when a = b).
fn half_tr(w: &Mat, (a, b): (usize, usize), (c, d): (usize, usize)) -> f64 {
    let e1: Vec<(usize, usize)> = if a == b { vec![(a, a)] } else { vec![(a, b), (b, a)] };
    let e2: Vec<(usize, usize)> = if c == d { vec![(c, c)] } else { vec![(c, d), (d, c)] };
    let mut s = 0.0;
    for &(i, j) in &e1 {
        for &(k, l) in &e2 {
            // tr(W e_i e_jᵀ W e_k e_lᵀ) = W[l][i] W[j][k]
            s += w[l][i] * w[j][k];
        }
    }
    0.5 * s
}

/// The total score of the unrestricted model over the blocks of one group, at moments (mu, sigma).
fn h1_gradient(blocks: &[&Block], mu: &[f64], sigma: &Mat, means: bool, p: usize) -> Option<Vec<f64>> {
    let (nm, cov) = h1_layout(p, means);
    let mut g = vec![0.0; nm + cov.len()];
    for b in blocks {
        let w = invert(&sub(sigma, &b.vars))?;
        let d: Vec<f64> = b.vars.iter().enumerate().map(|(k, &v)| b.mean[k] - mu[v]).collect();
        let pb = b.vars.len();
        // G = W (S_b + d dᵀ − Σ) W, embedded.
        let mid: Mat = (0..pb).map(|i| (0..pb).map(|j| b.cov[i][j] + d[i] * d[j] - sigma[b.vars[i]][b.vars[j]]).collect()).collect();
        let gm = matmul(&matmul(&w, &mid), &w);
        let mut full = vec![vec![0.0; p]; p];
        for i in 0..pb {
            for j in 0..pb {
                full[b.vars[i]][b.vars[j]] = gm[i][j];
            }
        }
        if means {
            for i in 0..pb {
                g[b.vars[i]] += b.n * (0..pb).map(|j| w[i][j] * d[j]).sum::<f64>();
            }
        }
        for (k, &(a, c)) in cov.iter().enumerate() {
            g[nm + k] += b.n * if a == c { 0.5 * full[a][a] } else { full[a][c] };
        }
    }
    Some(g)
}

/// tr(A₁⁻¹B₁) for the unrestricted model of every group (the first part of the Yuan-Bentler trace).
pub fn h1_trace(model: &Model, blocks: &[Block], h1: &H1, fiml: bool) -> Option<f64> {
    let p = model.p();
    let n = total_n(blocks);
    let means = model.meanstructure;
    let (nm, cov) = h1_layout(p, means);
    let dim = nm + cov.len();
    let mut total = 0.0;
    for g in 0..model.groups {
        let gb: Vec<&Block> = blocks.iter().filter(|b| b.group == g).collect();
        let (mu, sigma) = (&h1.mean[g], &h1.cov[g]);
        // A₁ per case of the whole sample.
        let a1: Mat = if fiml {
            let mut theta: Vec<f64> = Vec::with_capacity(dim);
            if means {
                theta.extend(mu.iter());
            }
            theta.extend(cov.iter().map(|&(a, c)| sigma[a][c]));
            let unpack = |t: &[f64]| -> (Vec<f64>, Mat) {
                let m = if means { t[..p].to_vec() } else { mu.clone() };
                let mut s = vec![vec![0.0; p]; p];
                for (k, &(a, c)) in cov.iter().enumerate() {
                    s[a][c] = t[nm + k];
                    s[c][a] = t[nm + k];
                }
                (m, s)
            };
            let mut h = vec![vec![0.0; dim]; dim];
            for k in 0..dim {
                let step = 1e-5 * theta[k].abs().max(1.0);
                let (mut up, mut dn) = (theta.clone(), theta.clone());
                up[k] += step;
                dn[k] -= step;
                let (mu_u, s_u) = unpack(&up);
                let (mu_d, s_d) = unpack(&dn);
                let gu = h1_gradient(&gb, &mu_u, &s_u, means, p)?;
                let gd = h1_gradient(&gb, &mu_d, &s_d, means, p)?;
                for j in 0..dim {
                    h[j][k] = -(gu[j] - gd[j]) / (2.0 * step) / n;
                }
            }
            (0..dim).map(|i| (0..dim).map(|j| 0.5 * (h[i][j] + h[j][i])).collect()).collect()
        } else {
            let w = invert(sigma)?;
            let ng = gb.iter().map(|b| b.n).sum::<f64>();
            let mut a = vec![vec![0.0; dim]; dim];
            if means {
                for i in 0..p {
                    for j in 0..p {
                        a[i][j] = w[i][j] * ng / n;
                    }
                }
            }
            for (k, &e1) in cov.iter().enumerate() {
                for (l, &e2) in cov.iter().enumerate() {
                    a[nm + k][nm + l] = half_tr(&w, e1, e2) * ng / n;
                }
            }
            a
        };
        // B₁: cross-products of the casewise unrestricted scores.
        let mut b1 = vec![vec![0.0; dim]; dim];
        for b in &gb {
            let w = invert(&sub(sigma, &b.vars))?;
            let pb = b.vars.len();
            for row in &b.rows {
                let d: Vec<f64> = b.vars.iter().zip(row).map(|(&v, x)| x - mu[v]).collect();
                let wd: Vec<f64> = (0..pb).map(|i| (0..pb).map(|j| w[i][j] * d[j]).sum()).collect();
                let mut full = vec![vec![0.0; p]; p];
                for i in 0..pb {
                    for j in 0..pb {
                        full[b.vars[i]][b.vars[j]] = wd[i] * wd[j] - w[i][j];
                    }
                }
                let mut s = vec![0.0; dim];
                if means {
                    for i in 0..pb {
                        s[b.vars[i]] = wd[i];
                    }
                }
                for (k, &(a, c)) in cov.iter().enumerate() {
                    s[nm + k] = if a == c { 0.5 * full[a][a] } else { full[a][c] };
                }
                for i in 0..dim {
                    if s[i] == 0.0 {
                        continue;
                    }
                    for j in 0..dim {
                        b1[i][j] += s[i] * s[j] / n;
                    }
                }
            }
        }
        // Parameters no case informs (a pair never observed together) drop out, as in lavaan.
        let keep: Vec<usize> = (0..dim).filter(|&i| a1[i][i] != 0.0).collect();
        let a1k: Mat = keep.iter().map(|&i| keep.iter().map(|&j| a1[i][j]).collect()).collect();
        let b1k: Mat = keep.iter().map(|&i| keep.iter().map(|&j| b1[i][j]).collect()).collect();
        total += trace_ab(&invert(&a1k)?, &b1k);
    }
    Some(total)
}

/// tr(X⁻¹ Y).
pub fn trace_inv(x: &Mat, y: &Mat) -> Option<f64> {
    Some(trace_ab(&invert(x)?, y))
}

/// The sandwich X⁻¹ Y X⁻¹ / N.
pub fn sandwich(x: &Mat, y: &Mat, n: f64) -> Option<Mat> {
    let xi = invert(x)?;
    Some(matmul(&matmul(&xi, y), &xi).into_iter().map(|r| r.into_iter().map(|v| v / n).collect()).collect())
}
