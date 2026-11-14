//! Time-series basics (checklist B12). A series is one numeric variable, its cases in row order and equally spaced, with
//! no missing values inside it.
//!
//! * Autocorrelation (`ts.acf`): the ACF and PACF (Durbin-Levinson) with ±1.96/√n bounds, and the Ljung-Box test, as R's
//!   acf, pacf and Box.test.
//! * Smoothing (`ts.smooth`): simple exponential smoothing with α chosen to minimize the one-step squared errors (as R's
//!   HoltWinters without trend or season), or a given α; and a centered moving average.
//! * ARIMA (`ts.arima`): ARIMA(p, d, q) by exact maximum likelihood, with the likelihood from a Kalman filter on Harvey's
//!   state-space form and the stationary initial covariance (as R's arima), a mean when d = 0, standard errors from the
//!   Hessian, AIC, the Ljung-Box test of the residuals, and forecasts with intervals.

use prismqda_stats::dist::{chisq_sf, norm_quantile, norm_two_sided};
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use super::linalg::{invert, Mat};
use crate::apa;
use crate::chart::{self, Chart};
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Cell, ResultBundle, Table};

pub const ACF: &str = "ts.acf";
pub const SMOOTH: &str = "ts.smooth";
pub const ARIMA: &str = "ts.arima";
pub const METHOD_VERSION: u32 = 1;

/// The series: every case in row order; empty cells are allowed only at the start and end (they are trimmed).
fn series(ds: &Dataset, name: &str) -> Result<(Vec<f64>, usize, usize)> {
    let cells = numeric(ds, name)?;
    let total = cells.len();
    let first = cells.iter().position(|c| c.value().is_some()).ok_or_else(|| Error::Invalid(format!("{name} has no values.")))?;
    let last = cells.iter().rposition(|c| c.value().is_some()).expect("found one");
    let mut x = Vec::new();
    for c in &cells[first..=last] {
        match c.value() {
            Some(v) => x.push(v),
            None => return Err(Error::Invalid(format!("{name} has a missing value inside the series; a time series needs every case between the first and last value."))),
        }
    }
    let trimmed = total - x.len();
    Ok((x, total, trimmed))
}

fn mean(x: &[f64]) -> f64 {
    x.iter().sum::<f64>() / x.len() as f64
}

/// Autocorrelations at lags 0..=max (R's acf: divisor n at every lag).
pub fn acf(x: &[f64], max: usize) -> Vec<f64> {
    let n = x.len();
    let m = mean(x);
    let c0: f64 = x.iter().map(|v| (v - m).powi(2)).sum::<f64>() / n as f64;
    (0..=max).map(|k| (0..n - k).map(|t| (x[t] - m) * (x[t + k] - m)).sum::<f64>() / n as f64 / c0).collect()
}

/// Partial autocorrelations at lags 1..=max by Durbin-Levinson.
pub fn pacf(r: &[f64], max: usize) -> Vec<f64> {
    let mut out = Vec::new();
    let mut phi: Vec<f64> = Vec::new();
    for k in 1..=max {
        let num = r[k] - (1..k).map(|j| phi[j - 1] * r[k - j]).sum::<f64>();
        let den = 1.0 - (1..k).map(|j| phi[j - 1] * r[j]).sum::<f64>();
        let a = num / den;
        let mut next: Vec<f64> = (1..k).map(|j| phi[j - 1] - a * phi[k - j - 1]).collect();
        next.push(a);
        phi = next;
        out.push(a);
    }
    out
}

/// The Ljung-Box statistic over lags 1..=h, with `fitdf` subtracted from the degrees of freedom.
pub fn ljung_box(x: &[f64], h: usize, fitdf: usize) -> (f64, f64, f64) {
    let n = x.len() as f64;
    let r = acf(x, h);
    let q = n * (n + 2.0) * (1..=h).map(|k| r[k] * r[k] / (n - k as f64)).sum::<f64>();
    let df = (h - fitdf) as f64;
    (q, df, chisq_sf(q, df))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcfSpec {
    pub variable: String,
    #[serde(default)]
    pub lags: Option<usize>,
}

pub fn run_acf(ds: &Dataset, spec: &AcfSpec) -> Result<ResultBundle> {
    let (x, total, trimmed) = series(ds, &spec.variable)?;
    let n = x.len();
    if n < 4 {
        return Err(Error::Invalid("A time series needs at least four values.".into()));
    }
    let lab = ds.variable(&spec.variable)?.display_name().to_string();
    let max = spec.lags.unwrap_or_else(|| ((10.0 * (n as f64).log10()).floor() as usize).min(n - 1)).clamp(1, n - 1);
    let r = acf(&x, max);
    let pr = pacf(&r, max);
    let bound = 1.96 / (n as f64).sqrt();
    let (q, df, pv) = ljung_box(&x, max, 0);
    let note = format!("Autocorrelations with the divisor n at every lag and partial autocorrelations by Durbin-Levinson (as R's acf and pacf); approximate 95% bounds ±1.96/√n. The Ljung-Box test covers lags 1 to {max}. The cases are taken in row order as equally spaced times. Computed by PrismStatistics.");
    let mut b = common::bundle(ACF, METHOD_VERSION, format!("Autocorrelation: {lab}"), serde_json::to_value(spec).expect("a spec always serializes"), crate::result::CaseAccounting { total, analyzed: n, excluded: if trimmed > 0 { vec![crate::result::Exclusion { reason: "empty before or after the series".into(), count: trimmed }] } else { Vec::new() } }, note);
    set_text(&mut b, "y.label", lab.clone());
    set(&mut b, "n", n as f64);
    set(&mut b, "lags", max as f64);
    for k in 1..=max {
        set(&mut b, format!("acf.{k}"), r[k]);
        set(&mut b, format!("pacf.{k}"), pr[k - 1]);
    }
    for (k, v) in [("lb.Q", q), ("lb.df", df), ("lb.p", pv), ("bound", bound)] {
        set(&mut b, k, v);
    }
    b.tables.push(Table {
        id: "acf".into(),
        title: "Autocorrelations".into(),
        columns: vec!["Lag".into(), "ACF".into(), "PACF".into(), "Beyond ±1.96/√n".into()],
        rows: (1..=max).map(|k| vec![count(k), num(r[k], 3), num(pr[k - 1], 3), text(if r[k].abs() > bound || pr[k - 1].abs() > bound { "yes" } else { "" })]).collect(),
        footnotes: vec![format!("Ljung-Box test of lags 1–{max}: Q = {}, df = {}, p {}.", apa::num(q, 3), apa::df(df), apa::p_clause(pv))],
    });
    let cats: Vec<(String, f64)> = (1..=max).map(|k| (k.to_string(), r[k])).collect();
    let alt = format!("Autocorrelations of {lab} at lags 1 to {max}.");
    b.charts.push(Chart { id: "acf".into(), title: "ACF".into(), alt: alt.clone(), svg: chart::bars(&cats, None, "Lag", "Autocorrelation", &alt) });
    let pcats: Vec<(String, f64)> = (1..=max).map(|k| (k.to_string(), pr[k - 1])).collect();
    let alt = format!("Partial autocorrelations of {lab} at lags 1 to {max}.");
    b.charts.push(Chart { id: "pacf".into(), title: "PACF".into(), alt: alt.clone(), svg: chart::bars(&pcats, None, "Lag", "Partial autocorrelation", &alt) });
    Ok(b)
}

// ---------------------------------------------------------------- Smoothing

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SmoothMethod {
    #[default]
    Exponential,
    MovingAverage,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmoothSpec {
    pub variable: String,
    #[serde(default)]
    pub method: SmoothMethod,
    /// Exponential: α; chosen by least squares when left out.
    #[serde(default)]
    pub alpha: Option<f64>,
    /// Moving average: the window (odd for a centered average).
    #[serde(default = "default_window")]
    pub window: usize,
    #[serde(default)]
    pub forecast: usize,
}

fn default_window() -> usize {
    3
}

/// One-step SSE of simple exponential smoothing started at the first value (HoltWinters' convention).
fn ses_sse(x: &[f64], a: f64) -> (f64, Vec<f64>) {
    let mut level = x[0];
    let mut sse = 0.0;
    let mut fitted = vec![f64::NAN];
    for &v in &x[1..] {
        fitted.push(level);
        sse += (v - level).powi(2);
        level = a * v + (1.0 - a) * level;
    }
    fitted.push(level);
    (sse, fitted)
}

/// Golden-section minimization on [lo, hi].
fn golden(f: &dyn Fn(f64) -> f64, mut lo: f64, mut hi: f64) -> f64 {
    let g = (5f64.sqrt() - 1.0) / 2.0;
    let (mut c, mut d) = (hi - g * (hi - lo), lo + g * (hi - lo));
    for _ in 0..200 {
        if f(c) < f(d) { hi = d } else { lo = c }
        c = hi - g * (hi - lo);
        d = lo + g * (hi - lo);
        if hi - lo < 1e-12 {
            break;
        }
    }
    (lo + hi) / 2.0
}

pub fn run_smooth(ds: &Dataset, spec: &SmoothSpec) -> Result<ResultBundle> {
    let (x, total, trimmed) = series(ds, &spec.variable)?;
    let n = x.len();
    if n < 3 {
        return Err(Error::Invalid("A time series needs at least three values.".into()));
    }
    let lab = ds.variable(&spec.variable)?.display_name().to_string();
    let accounting = crate::result::CaseAccounting { total, analyzed: n, excluded: if trimmed > 0 { vec![crate::result::Exclusion { reason: "empty before or after the series".into(), count: trimmed }] } else { Vec::new() } };
    let (title, note, smooth, extra): (String, String, Vec<f64>, Vec<(String, f64)>) = match spec.method {
        SmoothMethod::Exponential => {
            let a = match spec.alpha {
                Some(a) if a > 0.0 && a <= 1.0 => a,
                Some(_) => return Err(Error::Invalid("α must be above 0 and at most 1.".into())),
                None => golden(&|a| ses_sse(&x, a).0, 1e-6, 1.0),
            };
            let (sse, fitted) = ses_sse(&x, a);
            (format!("Exponential Smoothing: {lab}"), format!("Simple exponential smoothing (no trend or season) started at the first value, as R's HoltWinters(beta = FALSE, gamma = FALSE); α {}. Forecasts are the last level. Computed by PrismStatistics.", if spec.alpha.is_some() { "as given" } else { "minimizes the sum of squared one-step errors" }), fitted, vec![("α".into(), a), ("Sum of squared errors".into(), sse)])
        }
        SmoothMethod::MovingAverage => {
            let w = spec.window.max(2);
            let half = w / 2;
            let ma: Vec<f64> = (0..n)
                .map(|t| if w % 2 == 1 { if t >= half && t + half < n { mean(&x[t - half..=t + half]) } else { f64::NAN } } else if t >= half && t + half < n { (0.5 * x[t - half] + x[t - half + 1..t + half].iter().sum::<f64>() + 0.5 * x[t + half]) / w as f64 } else { f64::NAN })
                .collect();
            (format!("Moving Average: {lab}"), format!("Centered moving average over {w} cases (as R's stats::filter with sides = 2{}). Computed by PrismStatistics.", if w.is_multiple_of(2) { ", the end values weighted ½ for an even window" } else { "" }), ma, vec![("Window".into(), w as f64)])
        }
    };
    let mut b = common::bundle(SMOOTH, METHOD_VERSION, title, serde_json::to_value(spec).expect("a spec always serializes"), accounting, note);
    set_text(&mut b, "y.label", lab.clone());
    set(&mut b, "n", n as f64);
    for (k, v) in &extra {
        let key = match k.as_str() { "α" => "alpha", "Sum of squared errors" => "sse", _ => "window" };
        set(&mut b, key, *v);
    }
    b.tables.push(Table { id: "settings".into(), title: "Smoothing".into(), columns: vec!["".into(), "Value".into()], rows: extra.iter().map(|(k, v)| vec![text(k), num(*v, 4)]).collect(), footnotes: Vec::new() });
    let mut rows = Vec::new();
    for t in 0..n {
        rows.push(vec![count(t + 1), num(x[t], 3), num(smooth[t], 3)]);
    }
    if spec.method == SmoothMethod::Exponential {
        let last = smooth[n];
        set(&mut b, "level", last);
        for h in 1..=spec.forecast {
            rows.push(vec![count(n + h), Cell::Empty, num(last, 3)]);
        }
    }
    b.tables.push(Table { id: "values".into(), title: "Series and smoothed values".into(), columns: vec!["Case".into(), "Value".into(), if spec.method == SmoothMethod::Exponential { "One-step fitted value".into() } else { "Moving average".into() }], rows, footnotes: Vec::new() });
    let pts: Vec<(f64, f64)> = x.iter().enumerate().map(|(t, v)| ((t + 1) as f64, *v)).collect();
    let curve: Vec<(f64, f64)> = smooth.iter().take(n).enumerate().filter(|(_, v)| v.is_finite()).map(|(t, v)| ((t + 1) as f64, *v)).collect();
    let alt = format!("{lab} over time with the smoothed values.");
    b.charts.push(Chart { id: "series".into(), title: format!("{lab} and smoothed values"), alt: alt.clone(), svg: chart::scatter_curves(&pts, &[("Data".to_string(), pts.clone()), ("Smoothed".to_string(), curve)], "Case", &lab, &alt) });
    Ok(b)
}

// ---------------------------------------------------------------- ARIMA

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArimaSpec {
    pub variable: String,
    pub p: usize,
    #[serde(default)]
    pub d: usize,
    #[serde(default)]
    pub q: usize,
    /// Include a mean (only when d = 0).
    #[serde(default = "yes")]
    pub mean: bool,
    #[serde(default)]
    pub forecast: usize,
    #[serde(default = "default_level")]
    pub level: f64,
}

fn yes() -> bool {
    true
}

fn default_level() -> f64 {
    0.95
}

/// The exact ARMA(p, q) log-likelihood of a zero-mean series, concentrated over σ² (R's arima with method "ML"):
/// returns (−2 log L, σ̂², innovations).
fn arma_lik(w: &[f64], phi: &[f64], theta: &[f64]) -> Option<(f64, f64, Vec<f64>)> {
    let (p, q) = (phi.len(), theta.len());
    let r = p.max(q + 1);
    let mut tm = vec![vec![0.0; r]; r];
    for i in 0..r {
        if i < p {
            tm[i][0] = phi[i];
        }
        if i + 1 < r {
            tm[i][i + 1] = 1.0;
        }
    }
    let rv: Vec<f64> = (0..r).map(|i| if i == 0 { 1.0 } else if i - 1 < q { theta[i - 1] } else { 0.0 }).collect();
    // P0 from vec(P) = (I − T⊗T)⁻¹ vec(RRᵀ).
    let r2 = r * r;
    let mut a = vec![vec![0.0; r2]; r2];
    for i in 0..r {
        for j in 0..r {
            for k in 0..r {
                for l in 0..r {
                    a[i * r + j][k * r + l] = if i * r + j == k * r + l { 1.0 } else { 0.0 } - tm[i][k] * tm[j][l];
                }
            }
        }
    }
    let ainv = invert(&a)?;
    let rr: Vec<f64> = (0..r2).map(|ij| rv[ij / r] * rv[ij % r]).collect();
    let pv: Vec<f64> = ainv.iter().map(|row| row.iter().zip(&rr).map(|(x, y)| x * y).sum()).collect();
    let mut pm: Mat = (0..r).map(|i| (0..r).map(|j| pv[i * r + j]).collect()).collect();
    let mut state = vec![0.0; r];
    let n = w.len();
    let mut ssq = 0.0;
    let mut sumlog = 0.0;
    let mut innov = Vec::with_capacity(n);
    for &y in w {
        let f = pm[0][0];
        if f <= 0.0 {
            return None;
        }
        let v = y - state[0];
        ssq += v * v / f;
        sumlog += f.ln();
        innov.push(v / f.sqrt());
        // Update.
        let k: Vec<f64> = (0..r).map(|i| pm[i][0] / f).collect();
        let upd: Vec<f64> = (0..r).map(|i| state[i] + k[i] * v).collect();
        let pu: Mat = (0..r).map(|i| (0..r).map(|j| pm[i][j] - k[i] * pm[0][j]).collect()).collect();
        // Predict.
        state = (0..r).map(|i| (0..r).map(|j| tm[i][j] * upd[j]).sum()).collect();
        let tp: Mat = (0..r).map(|i| (0..r).map(|j| (0..r).map(|l| tm[i][l] * pu[l][j]).sum()).collect()).collect();
        pm = (0..r).map(|i| (0..r).map(|j| (0..r).map(|l| tp[i][l] * tm[j][l]).sum::<f64>() + rv[i] * rv[j]).collect()).collect();
    }
    let nf = n as f64;
    let s2 = ssq / nf;
    Some((nf * s2.ln() + sumlog + nf * (1.0 + (2.0 * std::f64::consts::PI).ln()), s2, innov))
}

/// True when every root of 1 − φ₁z − … lies outside the unit circle (checked through the partial autocorrelations).
fn stationary(phi: &[f64]) -> bool {
    let mut a = phi.to_vec();
    for k in (0..a.len()).rev() {
        let pk = a[k];
        if pk.abs() >= 1.0 {
            return false;
        }
        let prev: Vec<f64> = (0..k).map(|j| (a[j] + pk * a[k - 1 - j]) / (1.0 - pk * pk)).collect();
        a = prev;
    }
    true
}

pub fn run_arima(ds: &Dataset, spec: &ArimaSpec) -> Result<ResultBundle> {
    check_level(spec.level)?;
    let (x, total, trimmed) = series(ds, &spec.variable)?;
    let (p, d, q) = (spec.p, spec.d, spec.q);
    if p + q > 10 || d > 2 {
        return Err(Error::Invalid("Use at most 10 AR and MA terms together, and at most 2 differences.".into()));
    }
    let lab = ds.variable(&spec.variable)?.display_name().to_string();
    let mut w = x.clone();
    for _ in 0..d {
        w = w.windows(2).map(|v| v[1] - v[0]).collect();
    }
    let with_mean = spec.mean && d == 0;
    let k = p + q + usize::from(with_mean);
    let lv = level_text(spec.level);
    let note = format!("ARIMA({p}, {d}, {q}){} by exact maximum likelihood: the likelihood from a Kalman filter on the state-space form with the stationary initial covariance, as R's arima; standard errors from the numerical Hessian. Forecasts with {lv} intervals. Cases are taken in row order as equally spaced times. Computed by PrismStatistics.", if with_mean { " with a mean" } else { "" });
    let mut b = common::bundle(ARIMA, METHOD_VERSION, format!("ARIMA({p},{d},{q}): {lab}"), serde_json::to_value(spec).expect("a spec always serializes"), crate::result::CaseAccounting { total, analyzed: x.len(), excluded: if trimmed > 0 { vec![crate::result::Exclusion { reason: "empty before or after the series".into(), count: trimmed }] } else { Vec::new() } }, note);
    set_text(&mut b, "y.label", lab.clone());
    set(&mut b, "n", x.len() as f64);
    for (key, v) in [("p", p), ("d", d), ("q", q)] {
        set(&mut b, key, v as f64);
    }
    set(&mut b, "level", spec.level);
    if w.len() <= k + 2 {
        fail(&mut b, "The series is too short for this model.");
        return Ok(b);
    }
    let obj = |par: &[f64]| -> f64 {
        let (phi, rest) = par.split_at(p);
        let (theta, m) = rest.split_at(q);
        if !stationary(phi) {
            return f64::INFINITY;
        }
        let mu = m.first().copied().unwrap_or(0.0);
        let wc: Vec<f64> = w.iter().map(|v| v - mu).collect();
        arma_lik(&wc, phi, theta).map_or(f64::INFINITY, |r| r.0)
    };
    // Start from zeros (and the mean); refine by Nelder-Mead, then Newton with numerical derivatives.
    let mut par: Vec<f64> = vec![0.0; p + q];
    if with_mean {
        par.push(mean(&w));
    }
    let scale_mu = if with_mean { (w.iter().map(|v| (v - mean(&w)).powi(2)).sum::<f64>() / w.len() as f64).sqrt() } else { 1.0 };
    for step in [0.1, 0.02] {
        par = nm(&obj, &par, step, p + q, scale_mu);
    }
    let hstep = |v: f64| 1e-4 * v.abs().max(1e-2);
    let hess = |par: &[f64]| -> Mat {
        let f0 = obj(par);
        (0..k).map(|i| (0..k).map(|j| {
            let (hi, hj) = (hstep(par[i]), hstep(par[j]));
            let at = |di: f64, dj: f64| { let mut x = par.to_vec(); x[i] += di; x[j] += dj; obj(&x) };
            if i == j { (at(hi, 0.0) - 2.0 * f0 + at(-hi, 0.0)) / (hi * hi) } else { (at(hi, hj) - at(hi, -hj) - at(-hi, hj) + at(-hi, -hj)) / (4.0 * hi * hj) }
        }).collect()).collect()
    };
    for _ in 0..20 {
        let gr: Vec<f64> = (0..k).map(|i| { let h = hstep(par[i]); let mut u = par.clone(); u[i] += h; let mut dn = par.clone(); dn[i] -= h; (obj(&u) - obj(&dn)) / (2.0 * h) }).collect();
        let Some(inv) = invert(&hess(&par)) else { break };
        let stepv: Vec<f64> = inv.iter().map(|r| r.iter().zip(&gr).map(|(a, c)| a * c).sum()).collect();
        let cand: Vec<f64> = par.iter().zip(&stepv).map(|(a, s)| a - s).collect();
        if obj(&cand) <= obj(&par) {
            let small = stepv.iter().map(|s| s.abs()).fold(0.0, f64::max) < 1e-10;
            par = cand;
            if small {
                break;
            }
        } else {
            break;
        }
    }
    let m2ll = obj(&par);
    if !m2ll.is_finite() {
        fail(&mut b, "The model could not be fitted: no stationary estimate was found.");
        return Ok(b);
    }
    let (phi, rest) = par.split_at(p);
    let (theta, m) = rest.split_at(q);
    let mu = m.first().copied().unwrap_or(0.0);
    let wc: Vec<f64> = w.iter().map(|v| v - mu).collect();
    let (_, s2, innov) = arma_lik(&wc, phi, theta).expect("finite");
    // −2 log L is in m2ll; its Hessian / 2 is the information.
    let vcov = invert(&hess(&par)).map(|m| m.into_iter().map(|r| r.into_iter().map(|v| 2.0 * v).collect::<Vec<f64>>()).collect::<Mat>());
    let names: Vec<String> = (1..=p).map(|i| format!("ar{i}")).chain((1..=q).map(|i| format!("ma{i}"))).chain(if with_mean { Some("mean".to_string()) } else { None }).collect();
    let mut rows = Vec::new();
    for (i, nm_) in names.iter().enumerate() {
        let se = vcov.as_ref().map_or(f64::NAN, |v| v[i][i].sqrt());
        let z = par[i] / se;
        set(&mut b, format!("{nm_}.est"), par[i]);
        set(&mut b, format!("{nm_}.se"), se);
        set(&mut b, format!("{nm_}.p"), norm_two_sided(z));
        rows.push(vec![text(nm_), num(par[i], 4), num(se, 4), num(z, 3), common::p(norm_two_sided(z))]);
    }
    let loglik = -m2ll / 2.0;
    let aic = m2ll + 2.0 * (k as f64 + 1.0);
    for (kk, v) in [("sigma2", s2), ("loglik", loglik), ("aic", aic)] {
        set(&mut b, kk, v);
    }
    b.tables.push(Table { id: "coefficients".into(), title: "Coefficients".into(), columns: vec!["Term".into(), "Estimate".into(), "SE".into(), "z".into(), "p".into()], rows, footnotes: vec![format!("σ² = {}, log-likelihood = {}, AIC = {}.", apa::num(s2, 4), apa::num(loglik, 3), apa::num(aic, 3))] });
    // Residuals and the Ljung-Box test (lag min(10, n/5), fitdf = p + q).
    // The innovations v/√F are in the data's units already (F, scaled by σ², tends to 1), as R's arima residuals.
    let resid: Vec<f64> = innov.clone();
    let h = 10usize.min(resid.len() / 5).max(p + q + 1);
    if h < resid.len() {
        let (lq, ldf, lp) = ljung_box(&resid, h, p + q);
        set(&mut b, "lb.Q", lq);
        set(&mut b, "lb.df", ldf);
        set(&mut b, "lb.p", lp);
        b.diagnostics.push(crate::result::Diagnostic { id: "residuals".into(), title: "Residual autocorrelation".into(), state: if lp < 0.05 { crate::result::DiagnosticState::ReviewNeeded } else { crate::result::DiagnosticState::NoConcernDetected }, message: format!("Ljung-Box test of the residuals over {h} lags: Q = {}, df = {}, p {}.{}", apa::num(lq, 2), apa::df(ldf), apa::p_clause(lp), if lp < 0.05 { " The residuals are still autocorrelated; try other orders." } else { "" }) });
    }
    // Forecasts from the expanded model φ(B)(1 − B)^d x = θ(B) e.
    if spec.forecast > 0 {
        let mut ar_full = vec![1.0];
        for &f in phi {
            ar_full.push(-f);
        }
        for _ in 0..d {
            let mut next = vec![0.0; ar_full.len() + 1];
            for (i, c) in ar_full.iter().enumerate() {
                next[i] += c;
                next[i + 1] -= c;
            }
            ar_full = next;
        }
        let phis: Vec<f64> = ar_full[1..].iter().map(|c| -c).collect();
        let mut hist: Vec<f64> = x.iter().map(|v| v - if d == 0 { mu } else { 0.0 }).collect();
        let mut e: Vec<f64> = vec![0.0; x.len() - resid.len()].into_iter().chain(resid.iter().copied()).collect();
        // ψ weights for the forecast variance.
        let nh = spec.forecast;
        let mut psi = vec![1.0];
        for j in 1..nh {
            let mut v = if j <= q { theta[j - 1] } else { 0.0 };
            for (i, ph) in phis.iter().enumerate() {
                if j > i {
                    v += ph * psi[j - i - 1];
                }
            }
            psi.push(v);
        }
        let z = norm_quantile(1.0 - (1.0 - spec.level) / 2.0);
        let mut rows_f = Vec::new();
        let mut cum = 0.0;
        for hh in 1..=nh {
            let t = hist.len();
            let mut f = 0.0;
            for (i, ph) in phis.iter().enumerate() {
                if t > i {
                    f += ph * hist[t - 1 - i];
                }
            }
            for (j, th) in theta.iter().enumerate() {
                if e.len() > j {
                    f += th * e[e.len() - 1 - j];
                }
            }
            hist.push(f);
            e.push(0.0);
            cum += psi[hh - 1] * psi[hh - 1];
            let se = (s2 * cum).sqrt();
            let fc = f + if d == 0 { mu } else { 0.0 };
            set(&mut b, format!("forecast.{hh}"), fc);
            set(&mut b, format!("forecast.{hh}.se"), se);
            rows_f.push(vec![count(x.len() + hh), num(fc, 3), num(se, 3), num(fc - z * se, 3), num(fc + z * se, 3)]);
        }
        b.tables.push(Table { id: "forecast".into(), title: "Forecasts".into(), columns: vec!["Case".into(), "Forecast".into(), "SE".into(), format!("{lv} lower"), format!("{lv} upper")], rows: rows_f, footnotes: Vec::new() });
    }
    Ok(b)
}

/// Nelder-Mead for the ARIMA objective; the mean (last) gets a step on the series' own scale.
fn nm(f: &dyn Fn(&[f64]) -> f64, x0: &[f64], step: f64, n_arma: usize, scale_mu: f64) -> Vec<f64> {
    let d = x0.len();
    let mut simplex: Vec<Vec<f64>> = vec![x0.to_vec()];
    for i in 0..d {
        let mut v = x0.to_vec();
        v[i] += if i < n_arma { step } else { step * scale_mu };
        simplex.push(v);
    }
    let mut vals: Vec<f64> = simplex.iter().map(|v| f(v)).collect();
    for _ in 0..20000 {
        let mut order: Vec<usize> = (0..=d).collect();
        order.sort_by(|&a, &b| vals[a].total_cmp(&vals[b]));
        simplex = order.iter().map(|&i| simplex[i].clone()).collect();
        vals = order.iter().map(|&i| vals[i]).collect();
        if (vals[d] - vals[0]).abs() < 1e-13 && simplex.iter().skip(1).all(|v| v.iter().zip(&simplex[0]).all(|(a, b)| (a - b).abs() < 1e-10 * (1.0 + b.abs()))) {
            break;
        }
        let c: Vec<f64> = (0..d).map(|j| simplex[..d].iter().map(|v| v[j]).sum::<f64>() / d as f64).collect();
        let pt = |t: f64| -> Vec<f64> { c.iter().zip(&simplex[d]).map(|(a, w)| a + t * (a - w)).collect() };
        let xr = pt(1.0);
        let fr = f(&xr);
        if fr < vals[0] {
            let xe = pt(2.0);
            let fe = f(&xe);
            if fe < fr { simplex[d] = xe; vals[d] = fe } else { simplex[d] = xr; vals[d] = fr }
        } else if fr < vals[d - 1] {
            simplex[d] = xr;
            vals[d] = fr;
        } else {
            let xc = if fr < vals[d] { pt(0.5) } else { pt(-0.5) };
            let fc = f(&xc);
            if fc < vals[d].min(fr) {
                simplex[d] = xc;
                vals[d] = fc;
            } else {
                for i in 1..=d {
                    simplex[i] = simplex[i].iter().zip(&simplex[0]).map(|(a, b)| b + 0.5 * (a - b)).collect();
                    vals[i] = f(&simplex[i]);
                }
            }
        }
    }
    let best = (0..=d).min_by(|&a, &b| vals[a].total_cmp(&vals[b])).unwrap_or(0);
    simplex[best].clone()
}
