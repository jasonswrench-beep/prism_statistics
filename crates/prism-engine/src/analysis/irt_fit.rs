//! Fit statistics for the item response models in `irt.rs`, as R's mirt computes them (development list item 6).
//!
//! - **M2** (Maydeu-Olivares and Joe), mirt's `M2()` with its default type M2*: the observed means and cross-products of
//!   the item scores against the model's, weighted by the inverse of their asymptotic covariance with the directions the
//!   item parameters can move projected out. For right-or-wrong items it is M2 itself; for ordered items, M2* (Cai and
//!   Hansen). With it, as mirt reports them: RMSEA with its 90% interval, SRMSR (the root mean square of the residual
//!   correlations), and TLI and CFI against the null model whose items are independent (every slope 0).
//! - **S-X²** (Orlando and Thissen), mirt's `itemfit()` default: for each item, the observed and expected counts in
//!   each category at each summed score, the expected ones from the Lord-Wingersky recursion over the other items, with
//!   mirt's rules for pooling sparse cells (`collapseCells`, mincell 1), and its RMSEA and p value.
//!
//! Both integrate θ over the same 61-point grid from −6 to 6 as the estimation, and both need complete data.

// Numerical code reads more plainly with indices.
#![allow(clippy::needless_range_loop)]

use prismqda_stats::dist::chisq_sf;

use super::linalg::{invert, Mat};

/// A fitted model, as the fit statistics need it.
pub struct Model<'a> {
    /// The number of categories of each item; category indices are the item scores 0, 1, ….
    pub k: &'a [usize],
    pub a: Vec<f64>,
    pub d: Vec<Vec<f64>>,
    pub var: f64,
    /// Whether the slopes are estimated (not in the Rasch model).
    pub slopes_estimated: bool,
    /// Free parameters outside the items (the variance of θ in the Rasch model).
    pub group_params: usize,
}

const QUAD: usize = 61;

fn grid(var: f64) -> (Vec<f64>, Vec<f64>) {
    let th: Vec<f64> = (0..QUAD).map(|i| -6.0 + 12.0 * i as f64 / (QUAD - 1) as f64).collect();
    let w: Vec<f64> = th.iter().map(|t| (-t * t / (2.0 * var)).exp()).collect();
    let s: f64 = w.iter().sum();
    (th, w.into_iter().map(|x| x / s).collect())
}

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

/// Category probabilities of one item at θ.
fn probs(a: f64, d: &[f64], t: f64) -> Vec<f64> {
    let mut c = vec![1.0];
    c.extend(d.iter().map(|dk| sigmoid(a * t + dk)));
    c.push(0.0);
    (0..c.len() - 1).map(|k| c[k] - c[k + 1]).collect()
}

impl Model<'_> {
    fn item_params(&self, j: usize) -> usize {
        self.d[j].len() + usize::from(self.slopes_estimated)
    }

    fn est_params(&self) -> usize {
        (0..self.k.len()).map(|j| self.item_params(j)).sum::<usize>() + self.group_params
    }

    /// E[Y | θ] and E[Y² | θ] of item j on the grid, with its parameters moved by `shift` (for derivatives).
    fn moments(&self, j: usize, shift: Option<(usize, f64)>, th: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let mut a = self.a[j];
        let mut d = self.d[j].clone();
        if let Some((i, h)) = shift {
            if self.slopes_estimated {
                if i == 0 {
                    a += h;
                } else {
                    d[i - 1] += h;
                }
            } else {
                d[i] += h;
            }
        }
        th.iter()
            .map(|&t| {
                let p = probs(a, &d, t);
                let m1: f64 = p.iter().enumerate().map(|(k, pk)| k as f64 * pk).sum();
                let m2: f64 = p.iter().enumerate().map(|(k, pk)| (k * k) as f64 * pk).sum();
                (m1, m2)
            })
            .unzip()
    }
}

/// mirt's RMSEA from a chi-square: sqrt((X²/df − 1)/(N − 1)), 0 when X² is below df.
pub fn rmsea(x2: f64, df: f64, n: f64) -> f64 {
    if df <= 0.0 {
        return f64::NAN;
    }
    if x2 / df - 1.0 > 0.0 {
        ((x2 / df - 1.0) / (n - 1.0)).sqrt()
    } else {
        0.0
    }
}

pub struct M2 {
    pub m2: f64,
    pub df: f64,
    pub p: f64,
    pub rmsea: f64,
    pub rmsea_lower: f64,
    pub rmsea_upper: f64,
    pub srmsr: f64,
    pub tli: Option<f64>,
    pub cfi: Option<f64>,
}

/// The M2 (M2*) quadratic form N (p − e)' C (p − e), with C = Δc (Δc' Ξ Δc)⁻¹ Δc' and Δc the orthogonal complement
/// of Δ, formed as mirt forms it: the columns after the first ncol(Δ) of Q from R's `qr()` (LINPACK's dqrdc2, limited
/// column pivoting at tolerance 1e-7). When Δ has full rank this is Ξ⁻¹ − Ξ⁻¹Δ(Δ'Ξ⁻¹Δ)⁻¹Δ'Ξ⁻¹; when it does not
/// (the null model's intercepts of one ordered item move the margins in the same direction), mirt's choice of
/// subspace is kept so that the figures agree.
fn quadratic(n: f64, obs: &[f64], e: &[f64], xi: &Mat, delta: &Mat) -> Option<f64> {
    let m = obs.len();
    let r = delta.first().map_or(0, Vec::len);
    if r + 1 > m {
        return None;
    }
    let q = qr_q_complete(delta);
    let dc: Mat = (0..m).map(|i| q[i][r..].to_vec()).collect();
    let c = m - r;
    let xdc: Mat = (0..m).map(|i| (0..c).map(|b| (0..m).map(|k| xi[i][k] * dc[k][b]).sum()).collect()).collect();
    let mid: Mat = (0..c).map(|a| (0..c).map(|b| (0..m).map(|k| dc[k][a] * xdc[k][b]).sum()).collect()).collect();
    let mid_inv = invert(&mid)?;
    let resid: Vec<f64> = obs.iter().zip(e).map(|(o, x)| o - x).collect();
    let z: Vec<f64> = (0..c).map(|a| (0..m).map(|k| dc[k][a] * resid[k]).sum()).collect();
    let quad: f64 = (0..c).map(|a| (0..c).map(|b| z[a] * mid_inv[a][b] * z[b]).sum::<f64>()).sum();
    Some((n * quad).abs())
}

fn norm(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

/// The complete Q of R's `qr(x)` (LINPACK dqrdc2 with tol = 1e-7, then `qr.qy` on the identity with k = the rank).
fn qr_q_complete(x: &Mat) -> Mat {
    let n = x.len();
    let p = x.first().map_or(0, Vec::len);
    // Column-major working copy.
    let mut a: Vec<Vec<f64>> = (0..p).map(|j| (0..n).map(|i| x[i][j]).collect()).collect();
    let tol = 1e-7;
    let mut qraux: Vec<f64> = a.iter().map(|c| norm(c)).collect();
    let mut work1 = qraux.clone();
    let mut work2: Vec<f64> = qraux.iter().map(|v| if *v == 0.0 { 1.0 } else { *v }).collect();
    let lup = n.min(p);
    let mut k = p + 1; // as in the Fortran, 1-based
    for l in 0..lup {
        // Move negligible columns to the end (at most once each).
        while l + 1 < k && qraux[l] < work2[l] * tol {
            let col = a.remove(l);
            a.push(col);
            let (t, tt, ttt) = (qraux.remove(l), work1.remove(l), work2.remove(l));
            qraux.push(t);
            work1.push(tt);
            work2.push(ttt);
            k -= 1;
        }
        if l + 1 == n {
            break;
        }
        let mut nrmxl = norm(&a[l][l..]);
        if nrmxl == 0.0 {
            continue;
        }
        if a[l][l] != 0.0 {
            nrmxl = nrmxl.copysign(a[l][l]);
        }
        for v in a[l][l..].iter_mut() {
            *v /= nrmxl;
        }
        a[l][l] += 1.0;
        for j in (l + 1)..p {
            let t = -(l..n).map(|i| a[l][i] * a[j][i]).sum::<f64>() / a[l][l];
            for i in l..n {
                let v = a[l][i];
                a[j][i] += t * v;
            }
            if qraux[j] != 0.0 {
                let tt = (1.0 - (a[j][l].abs() / qraux[j]).powi(2)).max(0.0);
                if tt.abs() < 1e-6 {
                    qraux[j] = norm(&a[j][(l + 1)..]);
                    work1[j] = qraux[j];
                } else {
                    qraux[j] *= tt.sqrt();
                }
            }
        }
        qraux[l] = a[l][l];
        a[l][l] = -nrmxl;
    }
    let rank = (k - 1).min(n);
    // qr.qy(qr, I): apply H_ju … H_1 in reverse order to each column of the identity, ju = min(rank, n − 1).
    let ju = rank.min(n.saturating_sub(1));
    let mut q: Mat = (0..n).map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect();
    for col in 0..n {
        let mut y: Vec<f64> = (0..n).map(|i| q[i][col]).collect();
        for j in (0..ju).rev() {
            if qraux[j] == 0.0 {
                continue;
            }
            let mut u = a[j].clone();
            u[j] = qraux[j];
            let t = -(j..n).map(|i| u[i] * y[i]).sum::<f64>() / u[j];
            for i in j..n {
                y[i] += t * u[i];
            }
        }
        for i in 0..n {
            q[i][col] = y[i];
        }
    }
    q
}

/// The margins (item means, then the cross-products of each pair i < j), their covariance Ξ under the model, and
/// their derivatives with respect to the item parameters.
fn margins(model: &Model, th: &[f64], w: &[f64]) -> (Vec<f64>, Mat, Mat, Vec<f64>, Vec<f64>) {
    let j = model.k.len();
    let mom: Vec<(Vec<f64>, Vec<f64>)> = (0..j).map(|i| model.moments(i, None, th)).collect();
    let pairs: Vec<(usize, usize)> = (0..j).flat_map(|a| ((a + 1)..j).map(move |b| (a, b))).collect();
    // Each margin as the items it multiplies.
    let terms: Vec<Vec<usize>> = (0..j).map(|i| vec![i]).chain(pairs.iter().map(|&(a, b)| vec![a, b])).collect();
    let expect = |items: &[usize]| -> f64 {
        let mut count = vec![0usize; j];
        for &i in items {
            count[i] += 1;
        }
        (0..th.len())
            .map(|q| {
                w[q] * count.iter().enumerate().filter(|(_, c)| **c > 0).map(|(i, c)| if *c == 1 { mom[i].0[q] } else { mom[i].1[q] }).product::<f64>()
            })
            .sum()
    };
    let e: Vec<f64> = terms.iter().map(|t| expect(t)).collect();
    let m = terms.len();
    let mut xi = vec![vec![0.0; m]; m];
    for a in 0..m {
        for b in a..m {
            let both: Vec<usize> = terms[a].iter().chain(&terms[b]).copied().collect();
            let v = expect(&both) - e[a] * e[b];
            xi[a][b] = v;
            xi[b][a] = v;
        }
    }
    // Derivatives by central differences of each item's E[Y | θ].
    let mut cols = Vec::new();
    for i in 0..j {
        for p in 0..model.item_params(i) {
            let h = 1e-5;
            let up = model.moments(i, Some((p, h)), th).0;
            let dn = model.moments(i, Some((p, -h)), th).0;
            let dm: Vec<f64> = up.iter().zip(&dn).map(|(u, d)| (u - d) / (2.0 * h)).collect();
            let col: Vec<f64> = terms
                .iter()
                .map(|t| {
                    if !t.contains(&i) {
                        return 0.0;
                    }
                    let other = t.iter().find(|&&x| x != i);
                    (0..th.len()).map(|q| w[q] * dm[q] * other.map_or(1.0, |&o| mom[o].0[q])).sum()
                })
                .collect();
            cols.push(col);
        }
    }
    let delta: Mat = (0..m).map(|r| cols.iter().map(|c| c[r]).collect()).collect();
    let e1: Vec<f64> = (0..j).map(|i| (0..th.len()).map(|q| w[q] * mom[i].0[q]).sum()).collect();
    let e11: Vec<f64> = (0..j).map(|i| (0..th.len()).map(|q| w[q] * mom[i].1[q]).sum()).collect();
    (e, xi, delta, e1, e11)
}

/// M2 and the statistics that come with it, for complete data given as response patterns (category indices).
pub fn m2(model: &Model, patterns: &[Vec<usize>], counts: &[f64]) -> Option<M2> {
    let j = model.k.len();
    let n: f64 = counts.iter().sum();
    let (th, w) = grid(model.var);
    // Observed margins.
    let mut mean = vec![0.0; j];
    let mut cross = vec![vec![0.0; j]; j];
    for (y, c) in patterns.iter().zip(counts) {
        for a in 0..j {
            mean[a] += c * y[a] as f64;
            for b in 0..j {
                cross[a][b] += c * (y[a] * y[b]) as f64;
            }
        }
    }
    mean.iter_mut().for_each(|v| *v /= n);
    cross.iter_mut().flatten().for_each(|v| *v /= n);
    let pairs: Vec<(usize, usize)> = (0..j).flat_map(|a| ((a + 1)..j).map(move |b| (a, b))).collect();
    let obs: Vec<f64> = mean.iter().copied().chain(pairs.iter().map(|&(a, b)| cross[a][b])).collect();
    let (e, xi, delta, e1, e11) = margins(model, &th, &w);
    let m2 = quadratic(n, &obs, &e, &xi, &delta)?;
    let df = obs.len() as f64 - model.est_params() as f64;
    // SRMSR: observed against model-implied correlations of the item scores.
    let corr = |cov: &dyn Fn(usize, usize) -> f64| -> Vec<f64> { pairs.iter().map(|&(a, b)| cov(a, b) / (cov(a, a) * cov(b, b)).sqrt()).collect() };
    let robs = corr(&|a, b| cross[a][b] - mean[a] * mean[b]);
    let e2 = |a: usize, b: usize| if a == b { e11[a] } else { e[j + pairs.iter().position(|&p| p == (a.min(b), a.max(b))).expect("a pair")] };
    let rmod = corr(&|a, b| e2(a, b) - e1[a] * e1[b]);
    let srmsr = (robs.iter().zip(&rmod).map(|(o, m)| (o - m).powi(2)).sum::<f64>() / pairs.len() as f64).sqrt();
    let ci = |target: f64| -> f64 {
        if df <= 0.0 {
            return 0.0;
        }
        (super::sem::estimate::ncp_for(m2, df, target) / (n * df)).sqrt()
    };
    // The null model: every slope 0, so the items are independent with their observed category proportions.
    let null_d: Vec<Vec<f64>> = (0..j)
        .map(|i| {
            (1..model.k[i])
                .map(|kk| {
                    let above: f64 = patterns.iter().zip(counts).filter(|(y, _)| y[i] >= kk).map(|(_, c)| c).sum::<f64>() / n;
                    (above / (1.0 - above)).ln()
                })
                .collect()
        })
        .collect();
    let null = Model { k: model.k, a: vec![0.0; j], d: null_d, var: 1.0, slopes_estimated: false, group_params: 0 };
    let (ne, nxi, ndelta, _, _) = margins(&null, &th, &w);
    let null_m2 = quadratic(n, &obs, &ne, &nxi, &ndelta);
    let null_df = obs.len() as f64 - null.est_params() as f64;
    let (tli, cfi) = match null_m2 {
        Some(x0) if x0 > m2 => (Some((x0 / null_df - m2 / df) / (x0 / null_df - 1.0)), Some((1.0 - (m2 - df) / (x0 - null_df)).clamp(0.0, 1.0))),
        _ => (None, None),
    };
    Some(M2 { m2, df, p: if df > 0.0 { chisq_sf(m2, df) } else { f64::NAN }, rmsea: rmsea(m2, df, n), rmsea_lower: ci(0.95), rmsea_upper: ci(0.05), srmsr, tli, cfi })
}

/// One item's S-X².
pub struct ItemFit {
    pub x2: f64,
    pub df: f64,
    pub rmsea: f64,
    pub p: f64,
}

/// The Lord-Wingersky recursion: the probability of each summed score at each grid point, over the given items.
fn lord_wingersky(cat: &[Vec<Vec<f64>>], items: &[usize], q: usize) -> Vec<f64> {
    let mut l = vec![1.0];
    for &i in items {
        let p = &cat[i][q];
        let mut next = vec![0.0; l.len() + p.len() - 1];
        for (s, ls) in l.iter().enumerate() {
            for (k, pk) in p.iter().enumerate() {
                next[s + k] += ls * pk;
            }
        }
        l = next;
    }
    l
}

/// S-X² for every item, or None for an item with no degrees of freedom left (as mirt, which gives NaN).
pub fn s_x2(model: &Model, patterns: &[Vec<usize>], counts: &[f64]) -> Vec<Option<ItemFit>> {
    let j = model.k.len();
    let n: f64 = counts.iter().sum();
    let (th, w) = grid(model.var);
    let cat: Vec<Vec<Vec<f64>>> = (0..j).map(|i| th.iter().map(|&t| probs(model.a[i], &model.d[i], t)).collect()).collect();
    let top: usize = model.k.iter().map(|k| k - 1).sum();
    let all: Vec<usize> = (0..j).collect();
    let l_all: Vec<Vec<f64>> = (0..QUAD).map(|q| lord_wingersky(&cat, &all, q)).collect();
    let total: Vec<f64> = (0..=top).map(|s| (0..QUAD).map(|q| l_all[q][s] * w[q]).sum()).collect();
    // Observed: cases by summed score (rows 1..top-1) and category.
    let score = |y: &Vec<usize>| y.iter().sum::<usize>();
    let rows = top.saturating_sub(1);
    let mut obs: Vec<Vec<Vec<f64>>> = (0..j).map(|i| vec![vec![0.0; model.k[i]]; rows]).collect();
    for (y, c) in patterns.iter().zip(counts) {
        let s = score(y);
        if s >= 1 && s < top {
            for i in 0..j {
                obs[i][s - 1][y[i]] += c;
            }
        }
    }
    let nk: Vec<f64> = (0..rows).map(|r| obs[0][r].iter().sum()).collect();
    let mut out = Vec::with_capacity(j);
    let mut stop = false;
    for i in 0..j {
        let others: Vec<usize> = (0..j).filter(|&x| x != i).collect();
        let l_rest: Vec<Vec<f64>> = (0..QUAD).map(|q| lord_wingersky(&cat, &others, q)).collect();
        let mut e = vec![vec![f64::NAN; model.k[i]]; top + 1];
        for kk in 0..model.k[i] {
            for s2 in 0..l_rest[0].len() {
                let s = s2 + kk;
                let v: f64 = (0..QUAD).map(|q| l_rest[q][s2] * cat[i][q][kk] * w[q]).sum::<f64>() / total[s];
                e[s][kk] = v;
            }
        }
        let mut e: Vec<Vec<f64>> = e[1..top].to_vec();
        for (r, row) in e.iter_mut().enumerate() {
            row.iter_mut().for_each(|v| *v *= nk[r]);
        }
        if stop {
            out.push(None);
            continue;
        }
        let (o, e, halt) = collapse(obs[i].clone(), e, 1.0);
        stop = halt;
        let mut x2 = 0.0;
        let mut cells = 0usize;
        for (orow, erow) in o.iter().zip(&e) {
            for (ov, ev) in orow.iter().zip(erow) {
                if ev.is_finite() {
                    x2 += (ov - ev).powi(2) / ev;
                    cells += 1;
                }
            }
        }
        let df = cells as f64 - e.len() as f64 - model.item_params(i) as f64 - model.group_params as f64;
        if df <= 0.0 || e.is_empty() {
            out.push(None);
            continue;
        }
        out.push(Some(ItemFit { x2, df, rmsea: rmsea(x2, df, n), p: chisq_sf(x2, df) }));
    }
    out
}

fn add_row(m: &mut [Vec<f64>], to: usize, from: usize) {
    for c in 0..m[to].len() {
        m[to][c] += m[from][c];
    }
}

fn add_col(m: &mut [Vec<f64>], to: usize, from: usize) {
    for row in m.iter_mut() {
        row[to] += row[from];
    }
}

/// mirt's `collapseCells` for one item (mincell as given), returning the pooled observed and expected tables (an
/// expected NaN is a cell not used) and whether mirt would stop collapsing the remaining items (a table left empty).
fn collapse(mut on: Vec<Vec<f64>>, mut en: Vec<Vec<f64>>, mincell: f64) -> (Vec<Vec<f64>>, Vec<Vec<f64>>, bool) {
    // Rows that a category can't reach at that score: the first half are pooled into the row below them, the
    // second half into the row above.
    let drop: Vec<usize> = (0..en.len()).filter(|&r| en[r].iter().any(|v| v.is_nan())).collect();
    en.iter_mut().flatten().filter(|v| v.is_nan()).for_each(|v| *v = 0.0);
    if !drop.is_empty() {
        let half = drop.len() / 2;
        let up: Vec<usize> = (drop[0]..=drop[half.max(1) - 1]).collect();
        let low: Vec<usize> = (drop[half]..=drop[drop.len() - 1]).collect();
        let below = *up.iter().max().expect("rows") + 1;
        for &r in &up {
            add_row(&mut en, below, r);
            add_row(&mut on, below, r);
        }
        let above = *low.iter().min().expect("rows") - 1;
        for &r in &low {
            add_row(&mut en, above, r);
            add_row(&mut on, above, r);
        }
        let gone: Vec<usize> = up.iter().chain(&low).copied().collect();
        let keep = |m: Vec<Vec<f64>>| m.into_iter().enumerate().filter(|(r, _)| !gone.contains(r)).map(|(_, x)| x).collect::<Vec<_>>();
        en = keep(en);
        on = keep(on);
    }
    if on.is_empty() {
        return (on, en, true);
    }
    let keep: Vec<bool> = on.iter().map(|r| r.iter().sum::<f64>() != 0.0).collect();
    on = on.into_iter().zip(&keep).filter(|(_, k)| **k).map(|(r, _)| r).collect();
    en = en.into_iter().zip(&keep).filter(|(_, k)| **k).map(|(r, _)| r).collect();
    // mirt marks small expected cells here, by row position, before the rows of one case are pooled.
    let small: Vec<Vec<bool>> = en.iter().map(|r| r.iter().map(|v| *v < mincell).collect()).collect();
    let mut dropped = Vec::new();
    for r in 0..on.len().saturating_sub(1) {
        if on[r].iter().sum::<f64>() == 1.0 {
            dropped.push(r);
            add_row(&mut on, r + 1, r);
            add_row(&mut en, r + 1, r);
        }
    }
    if !dropped.is_empty() {
        on = on.into_iter().enumerate().filter(|(r, _)| !dropped.contains(r)).map(|(_, x)| x).collect();
        en = en.into_iter().enumerate().filter(|(r, _)| !dropped.contains(r)).map(|(_, x)| x).collect();
    }
    if on.len() >= 2 && on[on.len() - 1].iter().sum::<f64>() == 1.0 {
        let last = on.len() - 1;
        add_row(&mut on, last - 1, last);
        add_row(&mut en, last - 1, last);
        on.pop();
        en.pop();
    }
    let ncol = en.first().map_or(0, Vec::len);
    if ncol > 2 {
        for r in 0..en.len() {
            if !small.get(r).is_some_and(|s| s.iter().any(|x| *x)) {
                continue;
            }
            let (mut tmp, mut tmp2) = (en[r].clone(), on[r].clone());
            while tmp.len() > 2 {
                let m = tmp.iter().cloned().fold(f64::INFINITY, f64::min);
                let whc = tmp.iter().rposition(|v| *v == m).expect("a minimum");
                if whc == 0 {
                    tmp[1] += tmp[0];
                    tmp2[1] += tmp2[0];
                } else if whc == tmp.len() - 1 {
                    tmp[whc - 1] += tmp[whc];
                    tmp2[whc - 1] += tmp2[whc];
                } else {
                    let left = tmp[whc - 1].min(tmp[whc + 1]) == tmp[whc - 1];
                    let pick = if left { whc - 1 } else { whc + 1 };
                    tmp[pick] += tmp[whc];
                    tmp2[pick] += tmp2[whc];
                }
                tmp.remove(whc);
                tmp2.remove(whc);
                if tmp.iter().all(|v| *v >= mincell) {
                    break;
                }
            }
            tmp.resize(ncol, f64::NAN);
            tmp2.resize(ncol, 0.0);
            en[r] = tmp;
            on[r] = tmp2;
        }
    }
    en.iter_mut().flatten().filter(|v| v.is_nan()).for_each(|v| *v = 0.0);
    if ncol > 2 {
        loop {
            let nc = en[0].len();
            let need = mincell * (en.len() as f64 * 0.1).ceil();
            let last_small = en.iter().map(|r| r[nc - 1]).sum::<f64>() < need;
            if !last_small || nc == 2 {
                break;
            }
            add_col(&mut on, nc - 2, nc - 1);
            add_col(&mut en, nc - 2, nc - 1);
            on.iter_mut().for_each(|r| {
                r.pop();
            });
            en.iter_mut().for_each(|r| {
                r.pop();
            });
        }
    }
    let nc = en.first().map_or(0, Vec::len);
    let mut dropcol = vec![false; nc];
    for c in (1..nc).rev() {
        let share = en.iter().filter(|r| r[c] > 0.0).count() as f64 / en.len() as f64;
        if share < 0.05 {
            dropcol[c] = true;
            add_col(&mut en, c - 1, c);
            add_col(&mut on, c - 1, c);
        }
    }
    let strip = |m: Vec<Vec<f64>>| m.into_iter().map(|r| r.into_iter().zip(&dropcol).filter(|(_, d)| !**d).map(|(v, _)| v).collect::<Vec<f64>>()).collect::<Vec<_>>();
    en = strip(en);
    on = strip(on);
    // Rows with an expected count below mincell (but not 0) are pooled with a neighbour until none is left.
    loop {
        if en.len() < 2 {
            break;
        }
        let Some(whc) = en.iter().position(|r| r.iter().any(|v| *v < mincell && *v != 0.0)) else { break };
        let last = en.len() - 1;
        let target = if whc == 0 {
            1
        } else if whc == last {
            last - 1
        } else {
            let (a, b) = (on[whc - 1].iter().sum::<f64>(), on[whc + 1].iter().sum::<f64>());
            if a <= b { whc - 1 } else { whc + 1 }
        };
        add_row(&mut en, target, whc);
        add_row(&mut on, target, whc);
        en.remove(whc);
        on.remove(whc);
    }
    en.iter_mut().flatten().filter(|v| **v == 0.0).for_each(|v| *v = f64::NAN);
    (on, en, false)
}
