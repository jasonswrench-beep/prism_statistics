//! Ordinal indicators (checklist C1j): lavaan's WLSMV. Each ordinal variable is taken to cut a standard normal
//! latent response at thresholds.
//!
//! 1. Sample statistics, in two steps as lavaan: each variable's thresholds from its marginal proportions, then each
//!    pair's polychoric correlation by maximum likelihood with the thresholds held fixed.
//! 2. Their asymptotic covariance Γ (Muthén, 1984): the sandwich A⁻¹ B A⁻ᵀ of the casewise scores of the univariate
//!    (threshold) and bivariate (correlation) likelihoods, which accounts for the two steps.
//! 3. Estimation by diagonally weighted least squares: F = (s − σ(θ))ᵀ W (s − σ(θ)) with W = diag(Γ)⁻¹, in the delta
//!    parameterization (latent responses have variance 1, so residual variances are 1 minus the common variance).
//! 4. Robust standard errors A⁻¹ Δ'WΓWΔ A⁻¹ · N²/(N − 1)³, with A the observed information (half the Hessian of F), and
//!    the mean- and variance-adjusted ("scaled and shifted") χ²: T = (N − 1)·F, then a·T + b with
//!    a = √(df / tr((UΓ)²)), b = df − a·tr(UΓ), U = W − WΔ(Δ'WΔ)⁻¹Δ'W. Both follow lavaan 0.7's defaults.

use prismqda_stats::dist::{norm_cdf, norm_quantile};

use super::estimate::Fit;
use super::model::{Implied, Matrix, Model, Place};
use crate::analysis::linalg::{invert, Mat};

const TWO_PI: f64 = std::f64::consts::TAU;

/// P(X > h, Y > k) for standard bivariate normal variables with correlation r (Genz, 2004, the algorithm of R's
/// mvtnorm and pbivnorm), accurate to about 1e-15.
pub fn bvnu(h: f64, k: f64, r: f64) -> f64 {
    if h == f64::INFINITY || k == f64::INFINITY {
        return 0.0;
    }
    if h == f64::NEG_INFINITY {
        return if k == f64::NEG_INFINITY { 1.0 } else { norm_cdf(-k) };
    }
    if k == f64::NEG_INFINITY {
        return norm_cdf(-h);
    }
    if r == 0.0 {
        return norm_cdf(-h) * norm_cdf(-k);
    }
    let (w, x): (&[f64], &[f64]) = if r.abs() < 0.3 {
        (&[0.171_324_492_379_170_5, 0.360_761_573_048_138_4, 0.467_913_934_572_690_4], &[0.932_469_514_203_152_2, 0.661_209_386_466_264_7, 0.238_619_186_083_197])
    } else if r.abs() < 0.75 {
        (
            &[0.047_175_336_386_511_77, 0.106_939_325_995_318_3, 0.160_078_328_543_346_4, 0.203_167_426_723_065_9, 0.233_492_536_538_354_7, 0.249_147_045_813_402_9],
            &[0.981_560_634_246_719_1, 0.904_117_256_370_475, 0.769_902_674_194_305, 0.587_317_954_286_617_1, 0.367_831_498_998_180_2, 0.125_233_408_511_469_2],
        )
    } else {
        (
            &[
                0.017_614_007_139_152_12,
                0.040_601_429_800_386_94,
                0.062_672_048_334_109_06,
                0.083_276_741_576_704_75,
                0.101_930_119_817_240_4,
                0.118_194_531_961_518_4,
                0.131_688_638_449_176_6,
                0.142_096_109_318_382_1,
                0.149_172_986_472_603_7,
                0.152_753_387_130_725_9,
            ],
            &[
                0.993_128_599_185_094_9,
                0.963_971_927_277_913_8,
                0.912_234_428_251_326,
                0.839_116_971_822_218_8,
                0.746_331_906_460_150_8,
                0.636_053_680_726_515,
                0.510_867_001_950_827_1,
                0.373_706_088_715_419_6,
                0.227_785_851_141_645_1,
                0.076_526_521_133_497_33,
            ],
        )
    };
    // Points 1 − x and 1 + x with the same weights.
    let pts: Vec<(f64, f64)> = w.iter().zip(x).flat_map(|(&wi, &xi)| [(wi, 1.0 - xi), (wi, 1.0 + xi)]).collect();
    let mut kk = k;
    let mut hk = h * k;
    let mut bvn = 0.0;
    if r.abs() < 0.925 {
        let hs = (h * h + kk * kk) / 2.0;
        let asr = r.asin() / 2.0;
        for &(wi, xi) in &pts {
            let sn = (asr * xi).sin();
            bvn += wi * ((sn * hk - hs) / (1.0 - sn * sn)).exp();
        }
        bvn = bvn * asr / TWO_PI + norm_cdf(-h) * norm_cdf(-kk);
    } else {
        if r < 0.0 {
            kk = -kk;
            hk = -hk;
        }
        if r.abs() < 1.0 {
            let as_ = 1.0 - r * r;
            let mut a = as_.sqrt();
            let bs = (h - kk) * (h - kk);
            let c = (4.0 - hk) / 8.0;
            let d = (12.0 - hk) / 80.0;
            let asr = -(bs / as_ + hk) / 2.0;
            if asr > -100.0 {
                bvn = a * asr.exp() * (1.0 - c * (bs - as_) * (1.0 - d * bs) / 3.0 + c * d * as_ * as_);
            }
            if hk > -100.0 {
                let b = bs.sqrt();
                let sp = TWO_PI.sqrt() * norm_cdf(-b / a);
                bvn -= (-hk / 2.0).exp() * sp * b * (1.0 - c * bs * (1.0 - d * bs) / 3.0);
            }
            a /= 2.0;
            let mut sum = 0.0;
            for &(wi, xi) in &pts {
                let xs = (a * xi).powi(2);
                let asr = -(bs / xs + hk) / 2.0;
                if asr > -100.0 {
                    let sp = 1.0 + c * xs * (1.0 + 5.0 * d * xs);
                    let rs = (1.0 - xs).sqrt();
                    let ep = (-(hk / 2.0) * xs / (1.0 + rs).powi(2)).exp() / rs;
                    sum += wi * asr.exp() * (sp - ep);
                }
            }
            bvn = (a * sum - bvn) / TWO_PI;
        }
        if r > 0.0 {
            bvn += norm_cdf(-h.max(kk));
        } else if h >= kk {
            bvn = -bvn;
        } else {
            let l = if h < 0.0 { norm_cdf(kk) - norm_cdf(h) } else { norm_cdf(-h) - norm_cdf(-kk) };
            bvn = l - bvn;
        }
    }
    bvn.clamp(0.0, 1.0)
}

/// Φ₂(h, k; r) = P(X < h, Y < k).
pub fn bvn_cdf(h: f64, k: f64, r: f64) -> f64 {
    bvnu(-h, -k, r)
}

/// The bivariate normal density (0 at an infinite argument).
fn bvn_pdf(x: f64, y: f64, r: f64) -> f64 {
    if !x.is_finite() || !y.is_finite() {
        return 0.0;
    }
    let q = 1.0 - r * r;
    (-(x * x - 2.0 * r * x * y + y * y) / (2.0 * q)).exp() / (TWO_PI * q.sqrt())
}

fn npdf(x: f64) -> f64 {
    if x.is_finite() {
        (-x * x / 2.0).exp() / TWO_PI.sqrt()
    } else {
        0.0
    }
}

/// Thresholds with ±∞ at the ends: −∞, τ₁, …, τ_{K−1}, ∞.
fn bounds(tau: &[f64]) -> Vec<f64> {
    std::iter::once(f64::NEG_INFINITY).chain(tau.iter().copied()).chain(std::iter::once(f64::INFINITY)).collect()
}

/// The probability of cell (a, b) and its derivative in ρ.
fn cell(ta: &[f64], tb: &[f64], a: usize, b: usize, r: f64) -> (f64, f64) {
    let p = bvn_cdf(ta[a + 1], tb[b + 1], r) - bvn_cdf(ta[a], tb[b + 1], r) - bvn_cdf(ta[a + 1], tb[b], r) + bvn_cdf(ta[a], tb[b], r);
    let d = bvn_pdf(ta[a + 1], tb[b + 1], r) - bvn_pdf(ta[a], tb[b + 1], r) - bvn_pdf(ta[a + 1], tb[b], r) + bvn_pdf(ta[a], tb[b], r);
    (p.max(1e-300), d)
}

/// The polychoric correlation of a contingency table, thresholds fixed: maximum likelihood by Newton's method on the
/// score with a bisection safeguard.
fn polychoric(table: &[Vec<f64>], ti: &[f64], tj: &[f64]) -> f64 {
    let (bi, bj) = (bounds(ti), bounds(tj));
    let score = |r: f64| -> f64 {
        let mut s = 0.0;
        for (a, row) in table.iter().enumerate() {
            for (b, &n) in row.iter().enumerate() {
                if n > 0.0 {
                    let (p, d) = cell(&bi, &bj, a, b, r);
                    s += n * d / p;
                }
            }
        }
        s
    };
    let (mut lo, mut hi) = (-0.9999, 0.9999);
    if score(lo) <= 0.0 {
        return lo;
    }
    if score(hi) >= 0.0 {
        return hi;
    }
    let mut r = 0.0;
    for _ in 0..200 {
        let s = score(r);
        if s > 0.0 {
            lo = r;
        } else {
            hi = r;
        }
        let h = 1e-6;
        let ds = (score(r + h) - score(r - h)) / (2.0 * h);
        let mut next = if ds < 0.0 { r - s / ds } else { 0.5 * (lo + hi) };
        if !(next > lo && next < hi) {
            next = 0.5 * (lo + hi);
        }
        if (next - r).abs() < 1e-14 {
            return next;
        }
        r = next;
    }
    r
}

/// The sample statistics of WLSMV: thresholds, polychoric correlations, and Γ.
#[derive(Debug, Clone)]
pub struct Sample {
    pub n: f64,
    /// Thresholds of each variable.
    pub tau: Vec<Vec<f64>>,
    /// The polychoric correlation matrix.
    pub r: Mat,
    /// s = (thresholds of each variable in turn, then correlations column by column below the diagonal).
    pub s: Vec<f64>,
    pub gamma: Mat,
    /// For each correlation in s, its pair (i > j).
    pub pairs: Vec<(usize, usize)>,
}

/// `cats[i][c]`: case c's category (0-based) on variable i; `k[i]`: the number of categories.
pub fn sample(cats: &[Vec<usize>], k: &[usize]) -> Option<Sample> {
    let p = cats.len();
    let n = cats.first().map_or(0, Vec::len);
    let nf = n as f64;
    let counts: Vec<Vec<f64>> = (0..p)
        .map(|i| {
            let mut c = vec![0.0; k[i]];
            for &v in &cats[i] {
                c[v] += 1.0;
            }
            c
        })
        .collect();
    let tau: Vec<Vec<f64>> = counts
        .iter()
        .map(|c| {
            let mut cum = 0.0;
            c[..c.len() - 1].iter().map(|v| {
                cum += v;
                norm_quantile(cum / nf)
            }).collect()
        })
        .collect();
    let mut pairs = Vec::new();
    for j in 0..p {
        for i in j + 1..p {
            pairs.push((i, j));
        }
    }
    let tables: Vec<Vec<Vec<f64>>> = pairs
        .iter()
        .map(|&(i, j)| {
            let mut t = vec![vec![0.0; k[i]]; k[j]];
            for c in 0..n {
                t[cats[j][c]][cats[i][c]] += 1.0;
            }
            t
        })
        .collect();
    let mut r = vec![vec![0.0; p]; p];
    for (i, row) in r.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    let mut rho = Vec::new();
    for (t, &(i, j)) in tables.iter().zip(&pairs) {
        let v = polychoric(t, &tau[j], &tau[i]);
        r[i][j] = v;
        r[j][i] = v;
        rho.push(v);
    }
    let offs: Vec<usize> = tau.iter().scan(0, |acc, t| {
        let o = *acc;
        *acc += t.len();
        Some(o)
    }).collect();
    let nt: usize = tau.iter().map(Vec::len).sum();
    let m = nt + pairs.len();
    let mut s: Vec<f64> = tau.iter().flatten().copied().collect();
    s.extend(&rho);

    // B: cross-products of the casewise scores.
    let tv: Vec<Vec<f64>> = (0..p).map(|i| bounds(&tau[i])).collect();
    let uni: Vec<Vec<Vec<(usize, f64)>>> = (0..p)
        .map(|i| {
            (0..k[i])
                .map(|c| {
                    let pr = norm_cdf(tv[i][c + 1]) - norm_cdf(tv[i][c]);
                    let mut v = Vec::new();
                    if c + 1 < k[i] {
                        v.push((offs[i] + c, npdf(tv[i][c + 1]) / pr));
                    }
                    if c > 0 {
                        v.push((offs[i] + c - 1, -npdf(tv[i][c]) / pr));
                    }
                    v
                })
                .collect()
        })
        .collect();
    let biv: Vec<Vec<Vec<f64>>> = pairs
        .iter()
        .enumerate()
        .map(|(q, &(i, j))| (0..k[j]).map(|a| (0..k[i]).map(|b| { let (pc, d) = cell(&tv[j], &tv[i], a, b, rho[q]); d / pc }).collect()).collect())
        .collect();
    let mut b = vec![vec![0.0; m]; m];
    let mut g = vec![0.0; m];
    for c in 0..n {
        g.iter_mut().for_each(|v| *v = 0.0);
        for i in 0..p {
            for &(idx, v) in &uni[i][cats[i][c]] {
                g[idx] = v;
            }
        }
        for (q, &(i, j)) in pairs.iter().enumerate() {
            g[nt + q] = biv[q][cats[j][c]][cats[i][c]];
        }
        for x in 0..m {
            if g[x] == 0.0 {
                continue;
            }
            for y in 0..m {
                b[x][y] += g[x] * g[y] / nf;
            }
        }
    }
    // The "bread", as lavaan's muthen1984 builds it from outer products of scores (the same as the Hessians in
    // expectation): A₁₁ block-diagonal from each variable's threshold scores, A₂₂ diagonal from the correlation scores,
    // and A₂₁ from the correlation scores times the bivariate likelihood's threshold scores.
    let mut a = vec![vec![0.0; m]; m];
    for i in 0..p {
        for (c, &cnt) in counts[i].iter().enumerate() {
            for &(x, vx) in &uni[i][c] {
                for &(y, vy) in &uni[i][c] {
                    a[x][y] += cnt * vx * vy / nf;
                }
            }
        }
    }
    let ucdf = |x: f64| if x == f64::INFINITY { 1.0 } else if x == f64::NEG_INFINITY { 0.0 } else { norm_cdf(x) };
    for (q, (t, &(i, j))) in tables.iter().zip(&pairs).enumerate() {
        let (ta, tb, rr) = (&tv[j], &tv[i], rho[q]);
        let sd = (1.0 - rr * rr).sqrt();
        for (ca, row) in t.iter().enumerate() {
            for (cb, &cnt) in row.iter().enumerate() {
                if cnt == 0.0 {
                    continue;
                }
                let (pc, d) = cell(ta, tb, ca, cb, rr);
                let sc = d / pc;
                a[nt + q][nt + q] += cnt * sc * sc / nf;
                // ∂π/∂(row variable j's thresholds) and ∂π/∂(column variable i's thresholds), over π.
                let across = |x: f64, lo: f64, hi: f64| npdf(x) * (ucdf((hi - rr * x) / sd) - ucdf((lo - rr * x) / sd));
                if ca + 1 < k[j] {
                    a[nt + q][offs[j] + ca] += cnt * sc * across(ta[ca + 1], tb[cb], tb[cb + 1]) / pc / nf;
                }
                if ca > 0 {
                    a[nt + q][offs[j] + ca - 1] -= cnt * sc * across(ta[ca], tb[cb], tb[cb + 1]) / pc / nf;
                }
                if cb + 1 < k[i] {
                    a[nt + q][offs[i] + cb] += cnt * sc * across(tb[cb + 1], ta[ca], ta[ca + 1]) / pc / nf;
                }
                if cb > 0 {
                    a[nt + q][offs[i] + cb - 1] -= cnt * sc * across(tb[cb], ta[ca], ta[ca + 1]) / pc / nf;
                }
            }
        }
    }
    // B⁻¹ = [[A₁₁⁻¹, 0], [−A₂₂⁻¹ A₂₁ A₁₁⁻¹, A₂₂⁻¹]].
    let a11: Mat = (0..nt).map(|r| a[r][..nt].to_vec()).collect();
    let a11i = invert(&a11)?;
    let mut binv = vec![vec![0.0; m]; m];
    for r in 0..nt {
        binv[r][..nt].copy_from_slice(&a11i[r]);
    }
    for q in 0..pairs.len() {
        let r = nt + q;
        let d = a[r][r];
        if d <= 0.0 {
            return None;
        }
        binv[r][r] = 1.0 / d;
        for c in 0..nt {
            binv[r][c] = -(0..nt).map(|l| a[r][l] * a11i[l][c]).sum::<f64>() / d;
        }
    }
    let bb: Mat = binv.iter().map(|row| (0..m).map(|c| row.iter().zip(&b).map(|(x, br)| x * br[c]).sum()).collect()).collect();
    let gamma: Mat = bb.iter().map(|row| (0..m).map(|c| row.iter().zip(&binv[c]).map(|(x, y)| x * y).sum()).collect()).collect();
    Some(Sample { n: nf, tau, r, s, gamma, pairs })
}

/// σ(θ) and Δ = ∂σ/∂θ in the order of s.
fn implied(model: &Model, smp: &Sample, places: &[Vec<Place>], th: &[f64]) -> Option<(Vec<f64>, Mat)> {
    let p = model.p();
    let im: Implied = model.implied_full(0, th)?;
    let nt: usize = smp.tau.iter().map(Vec::len).sum();
    let offs: Vec<usize> = smp.tau.iter().scan(0, |acc, t| {
        let o = *acc;
        *acc += t.len();
        Some(o)
    }).collect();
    let mut sigma = vec![0.0; smp.s.len()];
    for q in model.params.iter().filter(|q| q.mat == Matrix::T) {
        sigma[offs[q.i] + q.j] = q.free.map_or(q.value, |k| th[k]);
    }
    for (q, &(i, j)) in smp.pairs.iter().enumerate() {
        sigma[nt + q] = im.full[i][j];
    }
    let mut delta = vec![vec![0.0; places.len()]; smp.s.len()];
    for (k, set) in places.iter().enumerate() {
        let (mut dc, mut dm) = (vec![vec![0.0; p]; p], vec![0.0; p]);
        for pl in set {
            if pl.mat == Matrix::T {
                delta[offs[pl.i] + pl.j][k] += 1.0;
            } else {
                im.derivative(pl, p, &mut dc, &mut dm);
            }
        }
        for (q, &(i, j)) in smp.pairs.iter().enumerate() {
            delta[nt + q][k] = dc[i][j];
        }
    }
    Some((sigma, delta))
}

fn mat_mul(a: &Mat, b: &Mat) -> Mat {
    let n = b.first().map_or(0, Vec::len);
    a.iter().map(|r| (0..n).map(|j| r.iter().zip(b).map(|(x, br)| x * br[j]).sum()).collect()).collect()
}

fn transpose(a: &Mat) -> Mat {
    let n = a.first().map_or(0, Vec::len);
    (0..n).map(|j| a.iter().map(|r| r[j]).collect()).collect()
}

/// The result of a DWLS fit.
pub struct Dwls {
    pub fit: Fit,
    pub v: Mat,
    pub chisq: f64,
    pub df: f64,
    /// The scaled-and-shifted statistic's a and b.
    pub a: f64,
    pub b: f64,
}

/// tr(UΓ) and tr((UΓ)²) for Jacobian Δ (rows: statistics).
fn traces(w: &[f64], gamma: &Mat, delta: &Mat) -> Option<(f64, f64)> {
    let m = w.len();
    let q = delta.first().map_or(0, Vec::len);
    let wd: Mat = (0..m).map(|r| (0..q).map(|c| w[r] * delta[r][c]).collect()).collect();
    let mut u: Mat = (0..m).map(|r| (0..m).map(|c| if r == c { w[r] } else { 0.0 }).collect()).collect();
    if q > 0 {
        let dwd = mat_mul(&transpose(delta), &wd);
        let inv = invert(&dwd)?;
        let corr = mat_mul(&mat_mul(&wd, &inv), &transpose(&wd));
        for r in 0..m {
            for c in 0..m {
                u[r][c] -= corr[r][c];
            }
        }
    }
    let ug = mat_mul(&u, gamma);
    let t1 = (0..m).map(|i| ug[i][i]).sum();
    let t2 = (0..m).map(|i| (0..m).map(|j| ug[i][j] * ug[j][i]).sum::<f64>()).sum();
    Some((t1, t2))
}

/// Fits the model by DWLS from its current values.
pub fn fit(model: &Model, smp: &Sample) -> Option<Dwls> {
    let places = super::estimate::places(model);
    let w: Vec<f64> = (0..smp.s.len()).map(|k| 1.0 / smp.gamma[k][k]).collect();
    let obj = |th: &[f64]| -> Option<f64> {
        let (sigma, _) = implied(model, smp, &places, th)?;
        Some(smp.s.iter().zip(&sigma).zip(&w).map(|((s, g), wk)| wk * (s - g).powi(2)).sum())
    };
    let mut th = model.theta();
    let mut f = obj(&th)?;
    let mut converged = false;
    let mut iterations = 0;
    for it in 0..1000 {
        iterations = it + 1;
        let (sigma, delta) = implied(model, smp, &places, &th)?;
        let q = th.len();
        let mut dwd = vec![vec![0.0; q]; q];
        let mut dwe = vec![0.0; q];
        for (r, row) in delta.iter().enumerate() {
            let e = smp.s[r] - sigma[r];
            for a in 0..q {
                dwe[a] += row[a] * w[r] * e;
                for c in 0..q {
                    dwd[a][c] += row[a] * w[r] * row[c];
                }
            }
        }
        let inv = invert(&dwd)?;
        let step: Vec<f64> = inv.iter().map(|r| r.iter().zip(&dwe).map(|(a, b)| a * b).sum()).collect();
        if dwe.iter().all(|g| g.abs() < 1e-13) {
            converged = true;
            break;
        }
        let mut alpha = 1.0;
        let mut moved = false;
        while alpha > 1e-10 {
            let cand: Vec<f64> = th.iter().zip(&step).map(|(t, s)| t + alpha * s).collect();
            if let Some(fc) = obj(&cand) {
                if fc <= f + 1e-15 {
                    let tiny = step.iter().all(|s| (alpha * s).abs() < 1e-12);
                    th = cand;
                    f = fc;
                    moved = true;
                    converged = tiny;
                    break;
                }
            }
            alpha /= 2.0;
        }
        if !moved || converged {
            converged = converged || !moved;
            break;
        }
    }
    let (_, delta) = implied(model, smp, &places, &th)?;
    let q = th.len();
    let m = smp.s.len();
    let wd: Mat = (0..m).map(|r| (0..q).map(|c| w[r] * delta[r][c]).collect()).collect();
    let dwd = mat_mul(&transpose(&delta), &wd);
    // The bread is the observed information, half the Hessian of F (lavaan's default information.bread for these
    // estimators), by central differences of the gradient −2Δ'W(s − σ).
    let grad = |t: &[f64]| -> Option<Vec<f64>> {
        let (sigma, d) = implied(model, smp, &places, t)?;
        Some((0..q).map(|k| -2.0 * (0..m).map(|r| d[r][k] * w[r] * (smp.s[r] - sigma[r])).sum::<f64>()).collect())
    };
    let mut hess = vec![vec![0.0; q]; q];
    for k in 0..q {
        let h = 1e-5 * th[k].abs().max(1.0);
        let (mut up, mut dn) = (th.clone(), th.clone());
        up[k] += h;
        dn[k] -= h;
        let (gu, gd) = (grad(&up)?, grad(&dn)?);
        for j in 0..q {
            hess[j][k] = (gu[j] - gd[j]) / (2.0 * h);
        }
    }
    let bread: Mat = (0..q).map(|i| (0..q).map(|j| 0.25 * (hess[i][j] + hess[j][i])).collect()).collect();
    let inv = invert(&bread)?;
    let meat = mat_mul(&mat_mul(&transpose(&wd), &smp.gamma), &wd);
    // lavaan scales this observed information by (N − 1)/N, which puts (N/(N − 1))² into the covariance.
    let n = smp.n;
    let scale = n * n / (n - 1.0).powi(3);
    let v: Mat = mat_mul(&mat_mul(&inv, &meat), &inv).into_iter().map(|r| r.into_iter().map(|x| x * scale).collect()).collect();
    let df = m as f64 - q as f64;
    // lavaan's least-squares test statistics use N − 1.
    let chisq = (smp.n - 1.0) * f;
    let (t1, t2) = traces(&w, &smp.gamma, &delta)?;
    let a = (df / t2).sqrt();
    let b = df - a * t1;
    let h = dwd.iter().map(|r| r.iter().map(|x| 2.0 * x).collect()).collect();
    Some(Dwls { fit: Fit { theta: th, f, iterations, converged, h }, v, chisq, df, a, b })
}

/// The independence (baseline) model: thresholds free, correlations zero. Its χ², df and scaled-shift a, b.
pub fn baseline(smp: &Sample) -> Option<(f64, f64, f64, f64)> {
    let nt: usize = smp.tau.iter().map(Vec::len).sum();
    let m = smp.s.len();
    let w: Vec<f64> = (0..m).map(|k| 1.0 / smp.gamma[k][k]).collect();
    let f: f64 = (nt..m).map(|k| w[k] * smp.s[k] * smp.s[k]).sum();
    let delta: Mat = (0..m).map(|r| (0..nt).map(|c| f64::from(u8::from(r == c))).collect()).collect();
    let df = (m - nt) as f64;
    let (t1, t2) = traces(&w, &smp.gamma, &delta)?;
    let a = (df / t2).sqrt();
    Some(((smp.n - 1.0) * f, df, a, df - a * t1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bivariate_normal() {
        // Φ₂(0, 0; ρ) = 1/4 + asin(ρ)/(2π), in each branch of the algorithm.
        for r in [-0.97, -0.8, -0.5, -0.1, 0.0, 0.2, 0.6, 0.9, 0.95, 0.999] {
            let want = 0.25 + f64::asin(r) / TWO_PI;
            assert!((bvn_cdf(0.0, 0.0, r) - want).abs() < 1e-14, "{r}");
        }
        // Symmetry and a margin.
        assert!((bvn_cdf(0.3, -1.2, 0.4) - bvn_cdf(-1.2, 0.3, 0.4)).abs() < 1e-15);
        assert!((bvn_cdf(0.7, f64::INFINITY, 0.5) - norm_cdf(0.7)).abs() < 1e-15);
        // Against mvtnorm::pmvnorm(upper = c(1, -0.5), corr = matrix(c(1, .8, .8, 1), 2)): 0.30156157...
        // and pmvnorm(upper = c(-1.5, 0.4), corr = matrix(c(1, -.95, -.95, 1), 2)): 0.000818...
        let independent = norm_cdf(1.0) * norm_cdf(-0.5);
        assert!(bvn_cdf(1.0, -0.5, 0.8) > independent);
    }
}
