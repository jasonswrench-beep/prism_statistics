//! Dynamic panel models (checklist E-4): Arellano and Bond's difference GMM as R's plm computes it (pgmm with
//! transformation "d").
//!
//! The model is first-differenced: Δyᵢₜ on its own lags, covariates (each at lags 0..L) and, for two-way effects,
//! differenced time dummies. Instruments are the lags `gmm_min..gmm_max` of y available in each period (a block for
//! each period, or one column per lag when collapsed), the differenced time dummies, and the differenced covariates
//! as ordinary instruments. The panel is completed to every individual × period (missing rows contribute zeros, as
//! plm does). One step uses the weight (Σ Wᵢ'HWᵢ/N)⁻¹ with H the tridiagonal 2, −1 matrix; two steps reweight by
//! (Σ Wᵢ'eᵢeᵢ'Wᵢ)⁻¹ from the one-step residuals. Robust standard errors are plm's vcovHC (with Windmeijer's
//! correction for two steps). Tests: Sargan (two-step weights), Arellano-Bond AR(1) and AR(2) tests of the
//! differenced residuals, and Wald tests of the coefficients and of the time dummies.

#![allow(clippy::needless_range_loop)]

use prismqda_stats::dist::{chisq_sf, norm_quantile, norm_two_sided};
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use super::linalg::{invert, pseudo_inverse_sym, symmetric_eigen, Mat};
use crate::apa;
use crate::error::{Error, Result};
use crate::model::{Column, Dataset};
use crate::result::{Diagnostic, DiagnosticState, ResultBundle, Table};

pub const PGMM: &str = "panel.gmm";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Effect {
    Individual,
    #[default]
    Twoways,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Steps {
    Onestep,
    #[default]
    Twosteps,
}

fn one() -> usize {
    1
}
fn two() -> usize {
    2
}
fn ninety_nine() -> usize {
    99
}
fn yes() -> bool {
    true
}
fn level() -> f64 {
    0.95
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub dependent: String,
    #[serde(default)]
    pub covariates: Vec<String>,
    pub individual: String,
    pub time: String,
    /// Lags 1..y_lags of the outcome as regressors.
    #[serde(default = "one")]
    pub y_lags: usize,
    /// Each covariate at lags 0..x_lags.
    #[serde(default)]
    pub x_lags: usize,
    /// GMM instruments: lags gmm_min..gmm_max of the outcome.
    #[serde(default = "two")]
    pub gmm_min: usize,
    #[serde(default = "ninety_nine")]
    pub gmm_max: usize,
    #[serde(default)]
    pub collapse: bool,
    #[serde(default)]
    pub effect: Effect,
    #[serde(default)]
    pub model: Steps,
    #[serde(default = "yes")]
    pub robust: bool,
    #[serde(default = "level")]
    pub level: f64,
}

/// Codes 0.. in sorted order, with the labels.
fn codes(ds: &Dataset, name: &str) -> Result<(Vec<Option<usize>>, Vec<String>)> {
    let i = ds.find(name).ok_or_else(|| Error::UnknownVariable(name.to_string()))?;
    let var = ds.variable(name)?;
    Ok(match &ds.columns[i] {
        Column::Text(t) => {
            let mut keys: Vec<String> = t.iter().flatten().cloned().collect();
            keys.sort();
            keys.dedup();
            (t.iter().map(|c| c.as_ref().map(|v| keys.binary_search(v).expect("listed"))).collect(), keys)
        }
        Column::Numeric(_) => {
            let cells = numeric(ds, name)?;
            let mut vals: Vec<f64> = cells.iter().filter_map(Slot::value).collect();
            vals.sort_by(f64::total_cmp);
            vals.dedup();
            let labels = vals.iter().map(|v| var.value_text(*v)).collect();
            (cells.iter().map(|c| c.value().map(|v| vals.binary_search_by(|k| k.total_cmp(&v)).expect("listed"))).collect(), labels)
        }
    })
}

fn zeros(r: usize, c: usize) -> Mat {
    vec![vec![0.0; c]; r]
}

/// A'B.
fn tmul(a: &Mat, b: &Mat) -> Mat {
    let (n, p, q) = (a.len(), a.first().map_or(0, Vec::len), b.first().map_or(0, Vec::len));
    let mut out = zeros(p, q);
    for r in 0..n {
        for i in 0..p {
            let v = a[r][i];
            if v != 0.0 {
                for j in 0..q {
                    out[i][j] += v * b[r][j];
                }
            }
        }
    }
    out
}

fn mul(a: &Mat, b: &Mat) -> Mat {
    let (n, k, q) = (a.len(), b.len(), b.first().map_or(0, Vec::len));
    let mut out = zeros(n, q);
    for i in 0..n {
        for l in 0..k {
            let v = a[i][l];
            if v != 0.0 {
                for j in 0..q {
                    out[i][j] += v * b[l][j];
                }
            }
        }
    }
    out
}

fn t(a: &Mat) -> Mat {
    let (n, p) = (a.len(), a.first().map_or(0, Vec::len));
    (0..p).map(|j| (0..n).map(|i| a[i][j]).collect()).collect()
}

fn mv(a: &Mat, v: &[f64]) -> Vec<f64> {
    a.iter().map(|r| r.iter().zip(v).map(|(x, y)| x * y).sum()).collect()
}

fn tv(a: &Mat, v: &[f64]) -> Vec<f64> {
    let p = a.first().map_or(0, Vec::len);
    (0..p).map(|j| a.iter().zip(v).map(|(r, y)| r[j] * y).sum()).collect()
}

fn add(a: &mut Mat, b: &Mat) {
    for (r, s) in a.iter_mut().zip(b) {
        for (x, y) in r.iter_mut().zip(s) {
            *x += y;
        }
    }
}

/// plm's inverse: solve, or the generalized inverse when the smallest eigenvalue is below 1e-9.
fn inverse(a: &Mat) -> (Mat, bool) {
    let (vals, _) = symmetric_eigen(a);
    let min = vals.iter().copied().fold(f64::INFINITY, f64::min);
    if min < 1e-9 {
        (pseudo_inverse_sym(a), true)
    } else {
        match invert(a) {
            Some(m) => (m, false),
            None => (pseudo_inverse_sym(a), true),
        }
    }
}

/// One individual's differenced data: y, regressors (rows × K) and instruments (rows × p).
struct Unit {
    y: Vec<f64>,
    x: Mat,
    w: Mat,
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    common::check_level(spec.level)?;
    if spec.y_lags == 0 || spec.y_lags > 4 {
        return Err(Error::Invalid("Use 1 to 4 lags of the outcome.".into()));
    }
    if spec.gmm_min < 2 || spec.gmm_max < spec.gmm_min {
        return Err(Error::Invalid("The instruments are lags of the outcome from 2 on (the first lag is correlated with the differenced error).".into()));
    }
    let y = numeric(ds, &spec.dependent)?;
    let xs: Vec<Vec<Slot<f64>>> = spec.covariates.iter().map(|c| numeric(ds, c)).collect::<Result<_>>()?;
    let (idc, _) = codes(ds, &spec.individual)?;
    let (tc, tlabels) = codes(ds, &spec.time)?;
    let total = ds.n_rows();
    let mut tally = Tally::default();
    // The completed panel: every individual × period, missing where there is no row (or a missing value).
    let ni = idc.iter().flatten().max().map_or(0, |m| m + 1);
    let nt = tlabels.len();
    let nvar = 1 + spec.covariates.len();
    let mut grid = vec![vec![vec![f64::NAN; nvar]; nt]; ni];
    let mut seen = vec![vec![false; nt]; ni];
    let mut used = 0;
    for r in 0..total {
        let (Some(i), Some(tt)) = (idc[r], tc[r]) else {
            tally.add("no individual or period");
            continue;
        };
        if seen[i][tt] {
            return Err(Error::Invalid("An individual has two rows for the same period.".into()));
        }
        seen[i][tt] = true;
        grid[i][tt][0] = y[r].value().unwrap_or(f64::NAN);
        for (j, c) in xs.iter().enumerate() {
            grid[i][tt][1 + j] = c[r].value().unwrap_or(f64::NAN);
        }
        used += 1;
    }
    // Individuals that appear at all.
    let present: Vec<usize> = (0..ni).filter(|&i| seen[i].iter().any(|s| *s)).collect();
    let n_ind = present.len();
    let main_max = spec.y_lags.max(if spec.covariates.is_empty() { 0 } else { spec.x_lags });
    let tl1 = (main_max + 1).max(spec.gmm_min);
    let ylab = ds.variable(&spec.dependent)?.display_name().to_string();
    let effect_name = if spec.effect == Effect::Twoways { "two-way (individual and time) effects" } else { "individual effects" };
    let note = format!(
        "Dynamic panel model by Arellano and Bond's difference GMM, as R's plm (pgmm): {}-step estimator with {effect_name}; GMM instruments lags {} to {} of {}{}; {} standard errors. Computed by PrismStatistics.",
        if spec.model == Steps::Onestep { "one" } else { "two" },
        spec.gmm_min,
        spec.gmm_max,
        ylab,
        if spec.collapse { " (collapsed)" } else { "" },
        if spec.robust { if spec.model == Steps::Twosteps { "robust (Windmeijer-corrected)" } else { "robust" } } else { "conventional" }
    );
    let mut b = common::bundle(PGMM, METHOD_VERSION, format!("Dynamic Panel Model (GMM): {ylab}"), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, used), note);
    set_text(&mut b, "y.label", ylab.clone());
    set(&mut b, "individuals", n_ind as f64);
    set(&mut b, "periods", nt as f64);
    if nt <= tl1 + 1 {
        fail(&mut b, format!("The panel has {nt} periods; with these lags it needs at least {}.", tl1 + 2));
        return Ok(b);
    }
    if n_ind < 2 {
        fail(&mut b, "The model needs several individuals.");
        return Ok(b);
    }
    let rows = nt - tl1;
    // Regressor names.
    let mut names: Vec<String> = (1..=spec.y_lags).map(|k| format!("{ylab} (t−{k})")).collect();
    let mut keys: Vec<String> = (1..=spec.y_lags).map(|k| format!("{}.l{k}", spec.dependent)).collect();
    for (j, c) in spec.covariates.iter().enumerate() {
        let lab = ds.variable(c)?.display_name().to_string();
        for k in 0..=spec.x_lags {
            names.push(if k == 0 { lab.clone() } else { format!("{lab} (t−{k})") });
            keys.push(format!("{c}.l{k}"));
        }
        let _ = j;
    }
    let k_main = names.len();
    let tdum = if spec.effect == Effect::Twoways { rows } else { 0 };
    for tt in tl1..nt {
        names.push(format!("Period {}", tlabels[tt]));
        keys.push(format!("time.{}", tlabels[tt]));
    }
    names.truncate(k_main + tdum);
    keys.truncate(k_main + tdum);
    let kt = names.len();
    // Instruments per period row (0-based period tt = tl1 + r): y lags gmm_min..gmm_max available.
    let gmm_cols: Vec<Vec<usize>> = (0..rows)
        .map(|r| {
            let yy = tl1 + r + 1; // 1-based period, as plm's makegmm
            let lo = yy.saturating_sub(spec.gmm_max).max(1);
            let hi = yy - spec.gmm_min;
            (lo..=hi).map(|p| p - 1).collect()
        })
        .collect();
    let n_gmm = if spec.collapse { (nt - spec.gmm_min).min(spec.gmm_max + 1 - spec.gmm_min) } else { gmm_cols.iter().map(Vec::len).sum() };
    let n_iv = spec.covariates.len() * (spec.x_lags + 1);
    let p = n_gmm + tdum + n_iv;
    let value = |i: usize, tt: isize, v: usize| -> f64 { if tt < 0 { f64::NAN } else { grid[i][tt as usize][v] } };
    let mut units: Vec<Unit> = Vec::with_capacity(n_ind);
    let mut nobs = 0usize;
    for &i in &present {
        let (mut uy, mut ux, mut uw) = (vec![0.0; rows], zeros(rows, kt), zeros(rows, p));
        for r in 0..rows {
            let tt = (tl1 + r) as isize; // 0-based period of this differenced row
            let d = |v: usize, lag: usize| value(i, tt - lag as isize, v) - value(i, tt - lag as isize - 1, v);
            let mut row_x = Vec::with_capacity(kt);
            for k in 1..=spec.y_lags {
                row_x.push(d(0, k));
            }
            for j in 0..spec.covariates.len() {
                for k in 0..=spec.x_lags {
                    row_x.push(d(1 + j, k));
                }
            }
            let dy = d(0, 0);
            // Differenced time dummies (plm: diff(diag(rows + 1)) without its first column): +1 at r, −1 at r − 1.
            let mut tdv = vec![0.0; tdum];
            if tdum > 0 {
                tdv[r] = 1.0;
                if r >= 1 {
                    tdv[r - 1] = -1.0;
                }
            }
            let na = dy.is_nan() || row_x.iter().any(|v| v.is_nan());
            if na {
                continue;
            }
            nobs += 1;
            uy[r] = dy;
            for (c, v) in row_x.iter().chain(tdv.iter()).enumerate() {
                ux[r][c] = *v;
            }
            // GMM instruments.
            let mut col = 0;
            if spec.collapse {
                for (q, &pp) in gmm_cols[r].iter().rev().enumerate() {
                    let v = grid[i][pp][0];
                    uw[r][q] = if v.is_nan() { 0.0 } else { v };
                }
                col = n_gmm;
            } else {
                for rr in 0..rows {
                    for &pp in &gmm_cols[rr] {
                        if rr == r {
                            let v = grid[i][pp][0];
                            uw[r][col] = if v.is_nan() { 0.0 } else { v };
                        }
                        col += 1;
                    }
                }
            }
            for (q, v) in tdv.iter().enumerate() {
                uw[r][col + q] = *v;
            }
            col += tdum;
            for j in 0..spec.covariates.len() {
                for k in 0..=spec.x_lags {
                    let v = d(1 + j, k);
                    uw[r][col] = if v.is_nan() { 0.0 } else { v };
                    col += 1;
                }
            }
        }
        units.push(Unit { y: uy, x: ux, w: uw });
    }
    set(&mut b, "n", nobs as f64);
    set(&mut b, "instruments", p as f64);
    if nobs <= kt || p < kt {
        fail(&mut b, "There are too few usable observations or instruments for the model.");
        return Ok(b);
    }
    // H: 2 on the diagonal, −1 beside it.
    let mut h = zeros(rows, rows);
    for r in 0..rows {
        h[r][r] = 2.0;
        if r + 1 < rows {
            h[r][r + 1] = -1.0;
            h[r + 1][r] = -1.0;
        }
    }
    let nu = units.len() as f64;
    let mut a1 = zeros(p, p);
    let mut wx = zeros(p, kt);
    let mut wy = vec![0.0; p];
    for u in &units {
        add(&mut a1, &tmul(&u.w, &mul(&h, &u.w)));
        add(&mut wx, &tmul(&u.w, &u.x));
        let v = tv(&u.w, &u.y);
        wy.iter_mut().zip(v).for_each(|(a, c)| *a += c);
    }
    let (mut a1i, g1) = inverse(&a1);
    a1i.iter_mut().for_each(|r| r.iter_mut().for_each(|v| *v *= nu));
    let solve_step = |a: &Mat| -> Option<(Vec<f64>, Mat)> {
        let awx = mul(a, &wx);
        let bm = invert(&tmul(&wx, &awx))?;
        let coef = mv(&bm, &tv(&awx, &wy));
        Some((coef, bm))
    };
    let Some((coef1, b1)) = solve_step(&a1i) else {
        fail(&mut b, "The model could not be estimated: the regressors are collinear given the instruments.");
        return Ok(b);
    };
    let resid = |coef: &[f64]| -> Vec<Vec<f64>> { units.iter().map(|u| (0..rows).map(|r| u.y[r] - u.x[r].iter().zip(coef).map(|(x, c)| x * c).sum::<f64>()).collect()).collect() };
    let e1 = resid(&coef1);
    let mut a2 = zeros(p, p);
    for (u, e) in units.iter().zip(&e1) {
        let we = tv(&u.w, e);
        for a in 0..p {
            for c in 0..p {
                a2[a][c] += we[a] * we[c];
            }
        }
    }
    let (a2i, g2) = inverse(&a2);
    let (coef, vcov, a_used) = if spec.model == Steps::Twosteps {
        let Some((c2, v2)) = solve_step(&a2i) else {
            fail(&mut b, "The two-step estimate could not be computed.");
            return Ok(b);
        };
        (c2, v2, a2i.clone())
    } else {
        (coef1.clone(), b1.clone(), a1i.clone())
    };
    let e = resid(&coef);
    // Robust covariance (plm's vcovHC.pgmm).
    let sa2 = if g2 { inverse(&a2i).0 } else { a2.clone() };
    let mid = mul(&mul(&t(&wx), &mul(&a1i, &mul(&sa2, &a1i))), &wx);
    let vr1 = mul(&mul(&b1, &mid), &b1);
    let robust_v = if spec.model == Steps::Twosteps {
        let mut we = vec![0.0; p];
        for (u, ei) in units.iter().zip(&e) {
            let v = tv(&u.w, ei);
            we.iter_mut().zip(v).for_each(|(a, c)| *a += c);
        }
        let left = mul(&mul(&vcov, &t(&wx)), &a2i);
        let a2we = mv(&a2i, &we);
        let mut dmat = zeros(kt, kt);
        for k in 0..kt {
            // Σ W'(−x_k e₁' − e₁ x_k')W = −(a b' + b a') with a = W'x_k, b = W'e₁, summed over units.
            let mut wexkw = zeros(p, p);
            for (u, ei) in units.iter().zip(&e1) {
                let xk: Vec<f64> = (0..rows).map(|r| u.x[r][k]).collect();
                let av = tv(&u.w, &xk);
                let bv = tv(&u.w, ei);
                for a in 0..p {
                    for c in 0..p {
                        wexkw[a][c] -= av[a] * bv[c] + bv[a] * av[c];
                    }
                }
            }
            let dk = mv(&left, &mv(&wexkw, &a2we));
            for r in 0..kt {
                dmat[r][k] = -dk[r];
            }
        }
        let db = mul(&dmat, &vcov);
        let mut out = vcov.clone();
        let ddt = mul(&mul(&dmat, &vr1), &t(&dmat));
        for a in 0..kt {
            for c in 0..kt {
                out[a][c] += db[a][c] + db[c][a] + ddt[a][c];
            }
        }
        out
    } else {
        vr1
    };
    let vv = if spec.robust { robust_v } else { vcov.clone() };
    // Coefficients (z tests, as plm).
    let zq = norm_quantile(1.0 - (1.0 - spec.level) / 2.0);
    let mut rows_out = Vec::new();
    for j in 0..kt {
        let se = vv[j][j].sqrt();
        let z = coef[j] / se;
        let pv = norm_two_sided(z);
        let key = format!("coef.{}", keys[j]);
        for (kk, v) in [("est", coef[j]), ("se", se), ("z", z), ("p", pv), ("lower", coef[j] - zq * se), ("upper", coef[j] + zq * se)] {
            set(&mut b, format!("{key}.{kk}"), v);
        }
        if spec.model == Steps::Twosteps {
            set(&mut b, format!("{key}.step1"), coef1[j]);
        }
        if j < k_main {
            rows_out.push(vec![text(names[j].clone()), num(coef[j], 4), num(se, 4), num(z, 3), common::p(pv), text(format!("[{}, {}]", apa::num(coef[j] - zq * se, 4), apa::num(coef[j] + zq * se, 4)))]);
        }
    }
    // Tests.
    let wald = |lo: usize, hi: usize| -> Option<(f64, f64, usize)> {
        let sub: Mat = (lo..hi).map(|a| (lo..hi).map(|c| vv[a][c]).collect()).collect();
        let si = invert(&sub)?;
        let bb: Vec<f64> = coef[lo..hi].to_vec();
        let w: f64 = (0..bb.len()).map(|a| bb[a] * (0..bb.len()).map(|c| si[a][c] * bb[c]).sum::<f64>()).sum();
        Some((w, chisq_sf(w, bb.len() as f64), bb.len()))
    };
    let mut tests = Vec::new();
    // Sargan with the two-step weights.
    let mut z = vec![0.0; p];
    for (u, ei) in units.iter().zip(&e) {
        let v = tv(&u.w, ei);
        z.iter_mut().zip(v).for_each(|(a, c)| *a += c);
    }
    let sargan: f64 = (0..p).map(|a| z[a] * (0..p).map(|c| a2i[a][c] * z[c]).sum::<f64>()).sum();
    let sdf = (p - kt) as f64;
    let sp = chisq_sf(sargan, sdf);
    set(&mut b, "sargan", sargan);
    set(&mut b, "sargan.df", sdf);
    set(&mut b, "sargan.p", sp);
    tests.push(vec![text("Sargan test of overidentifying restrictions"), text(format!("χ²({}) = {}", apa::num(sdf, 0), apa::num(sargan, 2))), common::p(sp)]);
    // Arellano-Bond tests.
    let mut xz = zeros(p, kt);
    for u in &units {
        add(&mut xz, &tmul(&u.w, &u.x));
    }
    for order in [1usize, 2] {
        if order >= rows {
            continue;
        }
        let mut num_ = 0.0;
        let mut eve = 0.0;
        let mut ex = vec![0.0; kt];
        let mut zve = vec![0.0; p];
        for (u, ei) in units.iter().zip(&e) {
            let el: Vec<f64> = (0..rows).map(|r| if r >= order { ei[r - order] } else { 0.0 }).collect();
            let s: f64 = ei.iter().zip(&el).map(|(a, c)| a * c).sum();
            num_ += s;
            eve += s * s;
            let exu = tv(&u.x, &el);
            ex.iter_mut().zip(exu).for_each(|(a, c)| *a += c);
            let we = tv(&u.w, ei);
            zve.iter_mut().zip(we).for_each(|(a, c)| *a += c * s);
        }
        let bxz = mul(&vcov, &t(&xz)); // K × p
        let azve = mv(&a_used, &zve);
        let mid: f64 = ex.iter().zip(mv(&bxz, &azve)).map(|(a, c)| a * c).sum();
        let last: f64 = ex.iter().zip(mv(&vv, &ex)).map(|(a, c)| a * c).sum();
        let stat = num_ / (eve - 2.0 * mid + last).sqrt();
        let pv = norm_two_sided(stat);
        set(&mut b, format!("ar{order}"), stat);
        set(&mut b, format!("ar{order}.p"), pv);
        tests.push(vec![text(format!("Arellano-Bond test of order {order} (differenced residuals)")), text(format!("z = {}", apa::num(stat, 2))), common::p(pv)]);
    }
    if let Some((w, wp, df)) = wald(0, k_main) {
        set(&mut b, "wald", w);
        set(&mut b, "wald.p", wp);
        tests.push(vec![text("Wald test of the coefficients"), text(format!("χ²({df}) = {}", apa::num(w, 2))), common::p(wp)]);
    }
    if tdum > 0 {
        if let Some((w, wp, df)) = wald(k_main, kt) {
            set(&mut b, "wald.time", w);
            set(&mut b, "wald.time.p", wp);
            tests.push(vec![text("Wald test of the time dummies"), text(format!("χ²({df}) = {}", apa::num(w, 2))), common::p(wp)]);
        }
    }
    b.tables.push(Table {
        id: "coefficients".into(),
        title: "Coefficients (differenced equation)".into(),
        columns: vec!["Term".into(), "Estimate".into(), if spec.robust { "Robust SE".into() } else { "SE".into() }, "z".into(), "p".into(), format!("{} CI", level_text(spec.level))],
        rows: rows_out,
        footnotes: vec![format!("{} observations (differenced) from {} individuals; {} instruments{}.", nobs, n_ind, p, if tdum > 0 { "; time dummies estimated but not shown" } else { "" })],
    });
    b.tables.push(Table { id: "tests".into(), title: "Specification tests".into(), columns: vec!["Test".into(), "Statistic".into(), "p".into()], rows: tests, footnotes: vec!["A valid model shows first-order but not second-order autocorrelation in the differenced residuals, and a nonsignificant Sargan test (the Sargan test is not robust to heteroscedasticity).".into()] });
    if g1 || g2 {
        b.diagnostics.push(Diagnostic { id: "singular".into(), title: "Weight matrix".into(), state: DiagnosticState::ReviewNeeded, message: "A weighting matrix was singular, so a generalized inverse was used (as plm does): there may be too many instruments for the number of individuals; consider collapsing them or limiting their lags.".into() });
    }
    if p > n_ind {
        b.diagnostics.push(Diagnostic { id: "instruments".into(), title: "Instrument count".into(), state: DiagnosticState::ReviewNeeded, message: format!("There are {p} instruments for {n_ind} individuals; many instruments weaken the Sargan test and bias two-step estimates. Collapsing the instruments or limiting their lags helps.") });
    }
    if let Some(p2) = b.number("ar2.p") {
        if p2 < 0.05 {
            b.diagnostics.push(Diagnostic { id: "ar2".into(), title: "Second-order autocorrelation".into(), state: DiagnosticState::ReviewNeeded, message: "The AR(2) test is significant: lag 2 of the outcome is not a valid instrument; start the instruments at a later lag.".into() });
        }
    }
    Ok(b)
}
