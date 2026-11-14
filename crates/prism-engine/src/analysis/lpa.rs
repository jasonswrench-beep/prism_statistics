//! Latent profile analysis (checklist E-5): Gaussian mixture models of continuous indicators, as R's mclust fits them.
//!
//! Covariance structures (mclust's names): EEI (equal variances, no covariances: "class-invariant, diagonal"), VVI
//! (variances differ by class), EEE (equal full covariance matrices) and VVV (unrestricted); with one indicator, E
//! and V. Each is fitted by EM from many seeded starts (k-means++ seeds, then responsibilities from the nearest
//! seed), keeping the largest log-likelihood; EM runs until the relative change in the log-likelihood is below 1e-13.
//! Model comparison uses mclust's BIC (2 log L − p log n, larger is better), with AIC, the integrated completed
//! likelihood (ICL) and entropy. The chosen model's class shares, means, variances (and covariances), posterior
//! class sizes and average posterior probabilities are reported, with a profile chart of the class means.

#![allow(clippy::needless_range_loop)]

use prismqda_stats::rng::SplitMix64;
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use super::linalg::{cholesky, Mat};
use crate::apa;
use crate::chart;
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Diagnostic, DiagnosticState, ResultBundle, Table};

pub const LPA: &str = "lpa.mixture";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Structure {
    #[serde(rename = "EEI")]
    Eei,
    #[serde(rename = "VVI")]
    Vvi,
    #[serde(rename = "EEE")]
    Eee,
    #[serde(rename = "VVV")]
    Vvv,
}

impl Structure {
    pub fn name(self, d: usize) -> &'static str {
        match (self, d) {
            (Structure::Eei | Structure::Eee, 1) => "E",
            (Structure::Vvi | Structure::Vvv, 1) => "V",
            (Structure::Eei, _) => "EEI",
            (Structure::Vvi, _) => "VVI",
            (Structure::Eee, _) => "EEE",
            (Structure::Vvv, _) => "VVV",
        }
    }
    fn describe(self) -> &'static str {
        match self {
            Structure::Eei => "equal variances, no covariances",
            Structure::Vvi => "variances differ by class, no covariances",
            Structure::Eee => "equal variances and covariances",
            Structure::Vvv => "variances and covariances differ by class",
        }
    }
    fn diagonal(self) -> bool {
        matches!(self, Structure::Eei | Structure::Vvi)
    }
    fn shared(self) -> bool {
        matches!(self, Structure::Eei | Structure::Eee)
    }
    /// Covariance parameters (mclust's nVarParams).
    fn var_params(self, d: usize, g: usize) -> usize {
        match self {
            Structure::Eei => d,
            Structure::Vvi => g * d,
            Structure::Eee => d * (d + 1) / 2,
            Structure::Vvv => g * d * (d + 1) / 2,
        }
    }
}

fn default_models() -> Vec<Structure> {
    vec![Structure::Eei, Structure::Vvi, Structure::Eee, Structure::Vvv]
}
fn one() -> usize {
    1
}
fn six() -> usize {
    6
}
fn starts() -> usize {
    50
}
fn seed() -> u64 {
    20_260_101
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub indicators: Vec<String>,
    #[serde(default = "one")]
    pub min_classes: usize,
    #[serde(default = "six")]
    pub max_classes: usize,
    #[serde(default = "default_models")]
    pub models: Vec<Structure>,
    #[serde(default = "starts")]
    pub starts: usize,
    #[serde(default = "seed")]
    pub seed: u64,
    /// Report this model and number of classes instead of the one with the best BIC.
    #[serde(default)]
    pub choose_model: Option<Structure>,
    #[serde(default)]
    pub choose_classes: Option<usize>,
}

#[derive(Clone)]
pub struct Mixture {
    pub pro: Vec<f64>,
    pub mean: Vec<Vec<f64>>,
    /// One matrix per class (shared structures repeat it).
    pub sigma: Vec<Mat>,
    pub ll: f64,
    pub z: Vec<Vec<f64>>,
    pub iterations: usize,
}

/// log N(x; μ, Σ) from the Cholesky factor of Σ.
fn log_dens(x: &[f64], mu: &[f64], chol: &Mat, log_det: f64) -> f64 {
    let d = x.len();
    // Solve L y = x − μ.
    let mut y = vec![0.0; d];
    for i in 0..d {
        let mut s = x[i] - mu[i];
        for j in 0..i {
            s -= chol[i][j] * y[j];
        }
        y[i] = s / chol[i][i];
    }
    let q: f64 = y.iter().map(|v| v * v).sum();
    -0.5 * (d as f64 * std::f64::consts::TAU.ln() + log_det + q)
}

/// E-step: responsibilities and log-likelihood (None when a covariance matrix is not positive definite).
fn e_step(x: &[Vec<f64>], m: &Mixture) -> Option<(Vec<Vec<f64>>, f64)> {
    let g = m.pro.len();
    let mut chols = Vec::with_capacity(g);
    for k in 0..g {
        let l = cholesky(&m.sigma[k])?;
        let ld = 2.0 * (0..l.len()).map(|i| l[i][i].ln()).sum::<f64>();
        if !ld.is_finite() {
            return None;
        }
        chols.push((l, ld));
    }
    let mut ll = 0.0;
    let mut z = Vec::with_capacity(x.len());
    for xi in x {
        let lp: Vec<f64> = (0..g).map(|k| m.pro[k].ln() + log_dens(xi, &m.mean[k], &chols[k].0, chols[k].1)).collect();
        let mx = lp.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let s: f64 = lp.iter().map(|v| (v - mx).exp()).sum();
        ll += mx + s.ln();
        z.push(lp.iter().map(|v| (v - mx).exp() / s).collect());
    }
    Some((z, ll))
}

/// M-step for a structure.
/// Shares, means and covariance matrices.
type Params = (Vec<f64>, Vec<Vec<f64>>, Vec<Mat>);

fn m_step(x: &[Vec<f64>], z: &[Vec<f64>], s: Structure) -> Option<Params> {
    let (n, d, g) = (x.len(), x[0].len(), z[0].len());
    let nk: Vec<f64> = (0..g).map(|k| z.iter().map(|r| r[k]).sum()).collect();
    if nk.iter().any(|v| *v < 1e-8) {
        return None;
    }
    let pro: Vec<f64> = nk.iter().map(|v| v / n as f64).collect();
    let mean: Vec<Vec<f64>> = (0..g).map(|k| (0..d).map(|j| x.iter().zip(z).map(|(xi, zi)| zi[k] * xi[j]).sum::<f64>() / nk[k]).collect()).collect();
    let mut w: Vec<Mat> = vec![vec![vec![0.0; d]; d]; g];
    for (xi, zi) in x.iter().zip(z) {
        for k in 0..g {
            let r: Vec<f64> = (0..d).map(|j| xi[j] - mean[k][j]).collect();
            for a in 0..d {
                for b in 0..=a {
                    w[k][a][b] += zi[k] * r[a] * r[b];
                }
            }
        }
    }
    for k in 0..g {
        for a in 0..d {
            for b in 0..a {
                w[k][b][a] = w[k][a][b];
            }
        }
    }
    let mut sigma: Vec<Mat> = if s.shared() {
        let mut t = vec![vec![0.0; d]; d];
        for wk in &w {
            for a in 0..d {
                for b in 0..d {
                    t[a][b] += wk[a][b] / n as f64;
                }
            }
        }
        vec![t; g]
    } else {
        (0..g).map(|k| w[k].iter().map(|r| r.iter().map(|v| v / nk[k]).collect()).collect()).collect()
    };
    if s.diagonal() {
        for sk in sigma.iter_mut() {
            for a in 0..d {
                for b in 0..d {
                    if a != b {
                        sk[a][b] = 0.0;
                    }
                }
            }
        }
    }
    Some((pro, mean, sigma))
}

/// EM from initial responsibilities.
fn em(x: &[Vec<f64>], z0: Vec<Vec<f64>>, s: Structure) -> Option<Mixture> {
    let mut z = z0;
    let mut last = f64::NEG_INFINITY;
    for it in 1..=5000 {
        let (pro, mean, sigma) = m_step(x, &z, s)?;
        let mut m = Mixture { pro, mean, sigma, ll: 0.0, z: Vec::new(), iterations: it };
        let (zn, ll) = e_step(x, &m)?;
        z = zn;
        if (ll - last).abs() <= 1e-13 * ll.abs().max(1.0) {
            m.ll = ll;
            // The final parameters from the converged responsibilities, as mclust reports them.
            let (pro, mean, sigma) = m_step(x, &z, s)?;
            let mut f = Mixture { pro, mean, sigma, ll: 0.0, z: Vec::new(), iterations: it };
            let (zf, llf) = e_step(x, &f)?;
            f.ll = llf;
            f.z = zf;
            return Some(f);
        }
        last = ll;
    }
    None
}

/// The best of `starts` EM runs for g classes.
pub fn fit(x: &[Vec<f64>], g: usize, s: Structure, starts: usize, seed: u64) -> Option<(Mixture, usize)> {
    let n = x.len();
    let d = x[0].len();
    if g == 1 {
        return em(x, vec![vec![1.0]; n], s).map(|m| (m, 1));
    }
    // Scale for distances.
    let sd: Vec<f64> = (0..d)
        .map(|j| {
            let m = x.iter().map(|r| r[j]).sum::<f64>() / n as f64;
            (x.iter().map(|r| (r[j] - m).powi(2)).sum::<f64>() / n as f64).sqrt().max(1e-12)
        })
        .collect();
    let dist = |a: &[f64], b: &[f64]| -> f64 { (0..d).map(|j| ((a[j] - b[j]) / sd[j]).powi(2)).sum() };
    let mut rng = SplitMix64::new(seed ^ (g as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (s as u64 + 1));
    let mut best: Option<Mixture> = None;
    let mut hits = 0;
    for start in 0..starts.max(1) {
        if crate::job::step(start, starts.max(1)) {
            break;
        }
        // k-means++ seeds.
        let mut centers: Vec<usize> = vec![rng.below(n)];
        while centers.len() < g {
            let dmin: Vec<f64> = x.iter().map(|xi| centers.iter().map(|&c| dist(xi, &x[c])).fold(f64::INFINITY, f64::min)).collect();
            let tot: f64 = dmin.iter().sum();
            if tot <= 0.0 {
                break;
            }
            let mut u = (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64 * tot;
            let mut pick = n - 1;
            for (i, v) in dmin.iter().enumerate() {
                if u < *v {
                    pick = i;
                    break;
                }
                u -= v;
            }
            centers.push(pick);
        }
        if centers.len() < g {
            continue;
        }
        // A few k-means steps, then hard responsibilities.
        let mut cen: Vec<Vec<f64>> = centers.iter().map(|&c| x[c].clone()).collect();
        let mut lab = vec![0usize; n];
        for _ in 0..10 {
            for (i, xi) in x.iter().enumerate() {
                lab[i] = (0..g).min_by(|&a, &b| dist(xi, &cen[a]).total_cmp(&dist(xi, &cen[b]))).unwrap_or(0);
            }
            for k in 0..g {
                let members: Vec<&Vec<f64>> = x.iter().zip(&lab).filter(|(_, l)| **l == k).map(|(xi, _)| xi).collect();
                if !members.is_empty() {
                    cen[k] = (0..d).map(|j| members.iter().map(|m| m[j]).sum::<f64>() / members.len() as f64).collect();
                }
            }
        }
        let z0: Vec<Vec<f64>> = lab.iter().map(|&l| (0..g).map(|k| if k == l { 1.0 } else { 0.0 }).collect()).collect();
        let Some(m) = em(x, z0, s) else { continue };
        match &best {
            Some(b) if m.ll <= b.ll + 1e-7 * b.ll.abs().max(1.0) => {
                if (m.ll - b.ll).abs() <= 1e-6 * b.ll.abs().max(1.0) {
                    hits += 1;
                }
            }
            _ => {
                hits = 1;
                best = Some(m);
            }
        }
    }
    best.map(|b| (b, hits))
}

/// Parameters, BIC (mclust: 2 log L − p log n), ICL and entropy of a fit.
pub fn npar(s: Structure, d: usize, g: usize) -> usize {
    g * d + s.var_params(d, g) + g - 1
}

fn entropy(z: &[Vec<f64>]) -> f64 {
    let n = z.len() as f64;
    let g = z[0].len();
    if g < 2 {
        return 1.0;
    }
    let e: f64 = z.iter().flatten().filter(|v| **v > 0.0).map(|v| -v * v.ln()).sum();
    1.0 - e / (n * (g as f64).ln())
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    let d = spec.indicators.len();
    if d == 0 {
        return Err(Error::Invalid("Choose the indicators.".into()));
    }
    if spec.min_classes < 1 || spec.max_classes < spec.min_classes || spec.max_classes > 12 {
        return Err(Error::Invalid("Choose between 1 and 12 classes.".into()));
    }
    if spec.models.is_empty() {
        return Err(Error::Invalid("Choose at least one covariance structure.".into()));
    }
    let cols: Vec<Vec<Slot<f64>>> = spec.indicators.iter().map(|v| numeric(ds, v)).collect::<Result<_>>()?;
    let labels: Vec<String> = spec.indicators.iter().map(|v| ds.variable(v).map(|x| x.display_name().to_string())).collect::<Result<_>>()?;
    let total = ds.n_rows();
    let mut tally = Tally::default();
    let mut x: Vec<Vec<f64>> = Vec::new();
    'r: for r in 0..total {
        for (j, c) in cols.iter().enumerate() {
            if tally.check(&c[r], &spec.indicators[j]) {
                continue 'r;
            }
        }
        x.push(cols.iter().map(|c| c[r].value().expect("checked")).collect());
    }
    let n = x.len();
    let note = format!(
        "Latent profile analysis as R's mclust: Gaussian mixture models fitted by EM (the best of {} seeded starts per model, to a relative change in the log-likelihood below 1e-13); BIC = 2 log L − p log n (larger is better), as mclust reports it. Computed by PrismStatistics.",
        spec.starts
    );
    let mut b = common::bundle(LPA, METHOD_VERSION, "Latent Profile Analysis".into(), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, n), note);
    set(&mut b, "n", n as f64);
    if n < 2 * d + 2 {
        fail(&mut b, "There are too few complete cases.");
        return Ok(b);
    }
    let nf = n as f64;
    // Fit every model and number of classes.
    let mut models: Vec<Structure> = Vec::new();
    for s in &spec.models {
        // With one indicator, EEE is EEI and VVV is VVI.
        let s = if d == 1 { if s.shared() { Structure::Eei } else { Structure::Vvi } } else { *s };
        if !models.contains(&s) {
            models.push(s);
        }
    }
    type Fitted = (Structure, usize, Option<(Mixture, usize)>);
    let mut fits: Vec<Fitted> = Vec::new();
    for &s in &models {
        for g in spec.min_classes..=spec.max_classes {
            if g >= n || npar(s, d, g) >= n {
                fits.push((s, g, None));
                continue;
            }
            fits.push((s, g, fit(&x, g, s, spec.starts, spec.seed)));
        }
    }
    let mut cmp_rows = Vec::new();
    let mut best: Option<usize> = None;
    for (idx, (s, g, f)) in fits.iter().enumerate() {
        let name = s.name(d);
        let key = format!("fit.{name}.{g}");
        match f {
            Some((m, _)) => {
                let p = npar(*s, d, *g) as f64;
                let bic = 2.0 * m.ll - p * nf.ln();
                let icl = bic + 2.0 * m.z.iter().map(|r| r.iter().fold(0.0f64, |a, v| a.max(*v)).ln()).sum::<f64>();
                let e = entropy(&m.z);
                for (kk, v) in [("ll", m.ll), ("npar", p), ("bic", bic), ("aic", 2.0 * m.ll - 2.0 * p), ("icl", icl), ("entropy", e)] {
                    set(&mut b, format!("{key}.{kk}"), v);
                }
                let smallest = m.pro.iter().copied().fold(f64::INFINITY, f64::min);
                cmp_rows.push(vec![text(name), count(*g), num(m.ll, 2), count(p as usize), num(bic, 2), num(2.0 * m.ll - 2.0 * p, 2), num(icl, 2), if *g > 1 { num(e, 3) } else { text("") }, num(100.0 * smallest, 1)]);
                if best.is_none_or(|bi| {
                    let (bs, bg, bf) = &fits[bi];
                    let bm = &bf.as_ref().expect("fitted").0;
                    bic > 2.0 * bm.ll - npar(*bs, d, *bg) as f64 * nf.ln()
                }) {
                    best = Some(idx);
                }
            }
            None => cmp_rows.push(vec![text(name), count(*g), text("—"), count(npar(*s, d, *g)), text("not estimable"), text(""), text(""), text(""), text("")]),
        }
    }
    b.tables.push(Table {
        id: "compare".into(),
        title: "Model comparison".into(),
        columns: vec!["Model".into(), "Classes".into(), "Log-likelihood".into(), "Parameters".into(), "BIC".into(), "AIC".into(), "ICL".into(), "Entropy".into(), "Smallest class (%)".into()],
        rows: cmp_rows,
        footnotes: vec![format!(
            "BIC, AIC and ICL as mclust reports them (2 log L minus the penalty: larger is better). Models: {}.",
            models.iter().map(|s| format!("{} ({})", s.name(d), s.describe())).collect::<Vec<_>>().join("; ")
        )],
    });
    let chosen = match (spec.choose_model, spec.choose_classes) {
        (Some(s), Some(g)) => fits.iter().position(|(fs, fg, f)| *fs == s && *fg == g && f.is_some()),
        _ => best,
    };
    let Some(ci) = chosen else {
        fail(&mut b, "No model could be estimated (a class collapsed or a covariance matrix was singular in every start).");
        return Ok(b);
    };
    let (s, g, f) = &fits[ci];
    let (m, hits) = f.as_ref().expect("fitted");
    let g = *g;
    // Order classes by size (largest first) for a stable report.
    let mut order: Vec<usize> = (0..g).collect();
    order.sort_by(|&a, &c| m.pro[c].total_cmp(&m.pro[a]));
    set_text(&mut b, "model", s.name(d));
    set(&mut b, "classes", g as f64);
    set(&mut b, "ll", m.ll);
    set(&mut b, "bic", 2.0 * m.ll - npar(*s, d, g) as f64 * nf.ln());
    set(&mut b, "entropy", entropy(&m.z));
    let assigned: Vec<usize> = m.z.iter().map(|r| (0..g).max_by(|&a, &c| r[a].total_cmp(&r[c])).unwrap_or(0)).collect();
    let mut class_rows = Vec::new();
    for (pos, &k) in order.iter().enumerate() {
        let c = pos + 1;
        let cnt = assigned.iter().filter(|a| **a == k).count();
        let members: Vec<usize> = (0..n).filter(|&i| assigned[i] == k).collect();
        let avg = if members.is_empty() { f64::NAN } else { members.iter().map(|&i| m.z[i][k]).sum::<f64>() / members.len() as f64 };
        set(&mut b, format!("class.{c}.pro"), m.pro[k]);
        set(&mut b, format!("class.{c}.n"), cnt as f64);
        for j in 0..d {
            set(&mut b, format!("class.{c}.mean.{}", j + 1), m.mean[k][j]);
            set(&mut b, format!("class.{c}.var.{}", j + 1), m.sigma[k][j][j]);
        }
        class_rows.push(vec![text(format!("Class {c}")), num(100.0 * m.pro[k], 1), count(cnt), if avg.is_nan() { text("—") } else { num(avg, 3) }]);
    }
    b.tables.push(Table { id: "classes".into(), title: format!("Classes ({} model, {g} classes)", s.name(d)), columns: vec!["Class".into(), "Share (%)".into(), "Cases assigned".into(), "Average posterior probability".into()], rows: class_rows, footnotes: vec!["Cases are assigned to their most probable class; the average posterior probability of those assigned shows how cleanly the class separates (above .80 is usually considered good).".into()] });
    let mut mrows = Vec::new();
    for j in 0..d {
        let mut row = vec![text(labels[j].clone())];
        for &k in &order {
            row.push(text(format!("{} ({})", apa::num(m.mean[k][j], 3), apa::num(m.sigma[k][j][j].sqrt(), 3))));
        }
        mrows.push(row);
    }
    let mut mcols = vec!["Indicator".to_string()];
    mcols.extend((1..=g).map(|c| format!("Class {c}")));
    b.tables.push(Table { id: "profiles".into(), title: "Class means (standard deviations)".into(), columns: mcols, rows: mrows, footnotes: vec![if s.shared() { "The standard deviations are equal across classes in this model.".into() } else { "Each class has its own standard deviations in this model.".into() }] });
    if !s.diagonal() && d > 1 {
        let mut crow = Vec::new();
        for (pos, &k) in order.iter().enumerate() {
            if s.shared() && pos > 0 {
                break;
            }
            for a in 0..d {
                for c in 0..a {
                    let r = m.sigma[k][a][c] / (m.sigma[k][a][a] * m.sigma[k][c][c]).sqrt();
                    crow.push(vec![text(if s.shared() { "All classes".to_string() } else { format!("Class {}", pos + 1) }), text(format!("{} with {}", labels[a], labels[c])), num(r, 3)]);
                }
            }
        }
        b.tables.push(Table { id: "correlations".into(), title: "Within-class correlations".into(), columns: vec!["Class".into(), "Indicators".into(), "r".into()], rows: crow, footnotes: Vec::new() });
    }
    // Profile chart: class means by indicator.
    let series: Vec<(String, Vec<f64>)> = order.iter().enumerate().map(|(pos, &k)| (format!("Class {} ({}%)", pos + 1, apa::num(100.0 * m.pro[k], 0)), m.mean[k].clone())).collect();
    let alt = format!("Profile plot: the mean of each of {d} indicators in each of {g} classes.");
    b.charts.push(chart::Chart { id: "profiles".into(), title: "Profile plot".into(), svg: chart::profile(&labels, &series, "Indicator", "Mean", "Class", &alt), alt });
    b.diagnostics.push(if *hits >= 2 || g == 1 {
        Diagnostic { id: "starts".into(), title: "Local maxima".into(), state: DiagnosticState::NoConcernDetected, message: format!("The best log-likelihood of the chosen model was reached from {hits} of {} starts.", spec.starts) }
    } else {
        Diagnostic { id: "starts".into(), title: "Local maxima".into(), state: DiagnosticState::ReviewNeeded, message: "Only one start reached the chosen model's best log-likelihood; run again with more starts to make sure it is the maximum.".into() }
    });
    let smallest = m.pro.iter().copied().fold(f64::INFINITY, f64::min);
    if g > 1 && smallest * nf < 0.05 * nf.max(20.0) {
        b.diagnostics.push(Diagnostic { id: "small".into(), title: "Small class".into(), state: DiagnosticState::ReviewNeeded, message: format!("The smallest class holds {}% of the cases; very small classes are often spurious.", apa::num(100.0 * smallest, 1)) });
    }
    Ok(b)
}
