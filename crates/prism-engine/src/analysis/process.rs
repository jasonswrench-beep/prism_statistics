//! Mediation and moderation (checklist B4), in the manner of Hayes's PROCESS models 4 and 1.
//!
//! * Mediation (`process.mediation`): X → one or more parallel mediators → Y, with optional covariates. The a paths come
//!   from each mediator regressed on X, the b paths and the direct effect c′ from Y regressed on X and every mediator, and
//!   the total effect c from Y regressed on X. Each indirect effect a × b (and their sum) gets a seeded percentile
//!   bootstrap interval, drawing cases with replacement.
//! * Moderation (`process.moderation`): Y regressed on X, W and X × W (optionally mean-centered), with the slope of X at
//!   the 16th, 50th and 84th percentiles of W (PROCESS's default) or at the mean and ±1 SD, and the Johnson-Neyman values
//!   of W where the slope of X turns significant.
//!
//! Regressions are ordinary least squares with the usual (homoskedastic) standard errors.

use prismqda_stats::dist::{t_quantile, t_two_sided};
use prismqda_stats::rng::SplitMix64;
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use super::linalg::{invert, mul, transpose, Mat};
use crate::apa;
use crate::chart::{self, Chart};
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{ResultBundle, Table};

pub const MEDIATION: &str = "process.mediation";
pub const MODERATION: &str = "process.moderation";
pub const METHOD_VERSION: u32 = 1;

/// An ordinary least-squares fit: coefficients (intercept first) and their covariance matrix.
pub struct Ols {
    pub b: Vec<f64>,
    pub vcov: Mat,
    pub df: f64,
    pub r2: f64,
}

/// Y on the columns of `x` (each a predictor), with an intercept.
pub fn ols(y: &[f64], x: &[Vec<f64>]) -> Option<Ols> {
    let n = y.len();
    let k = x.len() + 1;
    if n <= k {
        return None;
    }
    let design: Mat = (0..n).map(|i| std::iter::once(1.0).chain(x.iter().map(|c| c[i])).collect()).collect();
    let xt = transpose(&design);
    let xtx_inv = invert(&mul(&xt, &design))?;
    let xty: Mat = xt.iter().map(|r| vec![r.iter().zip(y).map(|(a, b)| a * b).sum()]).collect();
    let b: Vec<f64> = mul(&xtx_inv, &xty).into_iter().map(|r| r[0]).collect();
    let fitted: Vec<f64> = design.iter().map(|r| r.iter().zip(&b).map(|(a, c)| a * c).sum()).collect();
    let rss: f64 = y.iter().zip(&fitted).map(|(a, f)| (a - f).powi(2)).sum();
    let mean = y.iter().sum::<f64>() / n as f64;
    let tss: f64 = y.iter().map(|v| (v - mean).powi(2)).sum();
    let df = (n - k) as f64;
    let s2 = rss / df;
    Some(Ols { b, vcov: xtx_inv.iter().map(|r| r.iter().map(|v| v * s2).collect()).collect(), df, r2: 1.0 - rss / tss })
}

/// A coefficient row: estimate, SE, t, p and the interval at `level`.
fn coef_row(o: &Ols, j: usize, level: f64) -> (f64, f64, f64, f64, f64, f64) {
    let est = o.b[j];
    let se = o.vcov[j][j].sqrt();
    let t = est / se;
    let q = t_quantile(1.0 - (1.0 - level) / 2.0, o.df);
    (est, se, t, t_two_sided(t, o.df), est - q * se, est + q * se)
}

/// Complete cases of the named numeric variables (in order), with the tally.
fn complete(ds: &Dataset, names: &[&String]) -> Result<(Vec<Vec<f64>>, Tally, usize)> {
    for (i, a) in names.iter().enumerate() {
        if names[..i].iter().any(|b| ds.find(a) == ds.find(b)) {
            return Err(Error::Invalid(format!("{a} is used more than once.")));
        }
    }
    let cells: Vec<Vec<Slot<f64>>> = names.iter().map(|n| numeric(ds, n)).collect::<Result<_>>()?;
    let total = cells[0].len();
    let mut tally = Tally::default();
    let used: Vec<usize> = (0..total).filter(|&r| !cells.iter().zip(names).any(|(c, n)| tally.check(&c[r], n))).collect();
    Ok((cells.iter().map(|c| used.iter().map(|&r| c[r].value().expect("checked")).collect()).collect(), tally, total))
}

fn label(ds: &Dataset, n: &str) -> Result<String> {
    Ok(ds.variable(n)?.display_name().to_string())
}

// ---------------------------------------------------------------- Mediation

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediationSpec {
    pub x: String,
    pub mediators: Vec<String>,
    pub y: String,
    #[serde(default)]
    pub covariates: Vec<String>,
    #[serde(default = "default_resamples")]
    pub resamples: usize,
    #[serde(default = "default_seed")]
    pub seed: u64,
    #[serde(default = "default_level")]
    pub level: f64,
}

fn default_resamples() -> usize {
    5000
}

fn default_seed() -> u64 {
    20_260_101
}

fn default_level() -> f64 {
    0.95
}

/// The a paths, b paths, direct effect and total effect of one set of cases.
fn mediation_paths(x: &[f64], ms: &[Vec<f64>], y: &[f64], covs: &[Vec<f64>]) -> Option<(Vec<Ols>, Ols, Ols)> {
    let a: Option<Vec<Ols>> = ms.iter().map(|m| ols(m, &std::iter::once(x.to_vec()).chain(covs.iter().cloned()).collect::<Vec<_>>())).collect();
    let preds: Vec<Vec<f64>> = std::iter::once(x.to_vec()).chain(ms.iter().cloned()).chain(covs.iter().cloned()).collect();
    let yfit = ols(y, &preds)?;
    let total = ols(y, &std::iter::once(x.to_vec()).chain(covs.iter().cloned()).collect::<Vec<_>>())?;
    Some((a?, yfit, total))
}

pub fn mediation(ds: &Dataset, spec: &MediationSpec) -> Result<ResultBundle> {
    if spec.mediators.is_empty() {
        return Err(Error::Invalid("Choose at least one mediator.".into()));
    }
    if !(100..=100_000).contains(&spec.resamples) {
        return Err(Error::Invalid("The number of resamples must be between 100 and 100,000.".into()));
    }
    check_level(spec.level)?;
    let names: Vec<&String> = std::iter::once(&spec.x).chain(&spec.mediators).chain(std::iter::once(&spec.y)).chain(&spec.covariates).collect();
    let (cols, tally, total) = complete(ds, &names)?;
    let k = spec.mediators.len();
    let x = &cols[0];
    let ms: Vec<Vec<f64>> = cols[1..=k].to_vec();
    let y = &cols[k + 1];
    let covs: Vec<Vec<f64>> = cols[k + 2..].to_vec();
    let n = x.len();
    let (xl, yl) = (label(ds, &spec.x)?, label(ds, &spec.y)?);
    let mls: Vec<String> = spec.mediators.iter().map(|m| label(ds, m)).collect::<Result<_>>()?;
    let lv = level_text(spec.level);
    let note = format!(
        "Mediation analysis (Hayes's PROCESS model 4) with {} mediator{} in parallel{}: ordinary least-squares regressions; indirect effects (a × b) with {lv} percentile bootstrap intervals from {} resamples of the cases (seed {}). Cases were used when every variable had a valid value. Computed by PrismStatistics.",
        k,
        if k == 1 { "" } else { "s" },
        if covs.is_empty() { String::new() } else { format!(", adjusting for {} covariate{}", covs.len(), if covs.len() == 1 { "" } else { "s" }) },
        spec.resamples,
        spec.seed
    );
    let mut b = common::bundle(MEDIATION, METHOD_VERSION, format!("Mediation: {xl} → {} → {yl}", mls.join(", ")), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, n), note);
    set_text(&mut b, "x.label", xl.clone());
    set_text(&mut b, "y.label", yl.clone());
    set(&mut b, "n", n as f64);
    set(&mut b, "k", k as f64);
    set(&mut b, "level", spec.level);
    let Some((a, yfit, tot)) = mediation_paths(x, &ms, y, &covs) else {
        fail(&mut b, "The regressions can't be estimated: too few cases, or a variable is constant or a combination of the others.");
        return Ok(b);
    };
    let mut rows = Vec::new();
    let mut push = |b: &mut ResultBundle, key: String, name: String, o: &Ols, j: usize| {
        let (est, se, t, pv, lo, hi) = coef_row(o, j, spec.level);
        for (f, v) in [("b", est), ("se", se), ("t", t), ("p", pv), ("lower", lo), ("upper", hi)] {
            set(b, format!("{key}.{f}"), v);
        }
        rows.push(vec![text(name), num(est, 4), num(se, 4), num(t, 3), p(pv), num(lo, 4), num(hi, 4)]);
    };
    for (i, (ai, ml)) in a.iter().zip(&mls).enumerate() {
        set_text(&mut b, format!("m.{}.label", i + 1), ml.clone());
        push(&mut b, format!("a.{}", i + 1), format!("a{}: {xl} → {ml}", i + 1), ai, 1);
    }
    for (i, ml) in mls.iter().enumerate() {
        push(&mut b, format!("b.{}", i + 1), format!("b{}: {ml} → {yl}", i + 1), &yfit, 2 + i);
    }
    push(&mut b, "direct".into(), format!("c′ (direct): {xl} → {yl}"), &yfit, 1);
    push(&mut b, "total".into(), format!("c (total): {xl} → {yl}"), &tot, 1);
    set(&mut b, "r2.y", yfit.r2);
    b.tables.push(Table {
        id: "paths".into(),
        title: "Paths".into(),
        columns: vec!["Path".into(), "b".into(), "SE".into(), "t".into(), "p".into(), format!("{lv} CI lower"), format!("{lv} CI upper")],
        rows,
        footnotes: vec![format!("Unstandardized coefficients. R² for {yl} with every predictor = {}.", apa::num(yfit.r2, 3))],
    });

    // Bootstrap the indirect effects.
    let ind: Vec<f64> = (0..k).map(|i| a[i].b[1] * yfit.b[2 + i]).collect();
    let mut rng = SplitMix64::new(spec.seed);
    let mut reps: Vec<Vec<f64>> = vec![Vec::with_capacity(spec.resamples); k + 1];
    for r in 0..spec.resamples {
        if crate::job::step(r, spec.resamples) {
            break;
        }
        let idx: Vec<usize> = (0..n).map(|_| rng.below(n)).collect();
        let pick = |c: &Vec<f64>| idx.iter().map(|&r| c[r]).collect::<Vec<f64>>();
        let (xs, ms_s, ys) = (pick(x), ms.iter().map(pick).collect::<Vec<_>>(), pick(y));
        let cs: Vec<Vec<f64>> = covs.iter().map(pick).collect();
        if let Some((aa, yy, _)) = mediation_paths(&xs, &ms_s, &ys, &cs) {
            let e: Vec<f64> = (0..k).map(|i| aa[i].b[1] * yy.b[2 + i]).collect();
            reps[k].push(e.iter().sum());
            for (r, v) in reps.iter_mut().zip(e) {
                r.push(v);
            }
        }
    }
    let pct = |v: &mut Vec<f64>, q: f64| -> f64 {
        v.sort_by(f64::total_cmp);
        let pos = q * (v.len() - 1) as f64;
        let (lo, hi) = (pos.floor() as usize, pos.ceil() as usize);
        v[lo] + (v[hi] - v[lo]) * (pos - lo as f64)
    };
    let alpha = 1.0 - spec.level;
    let mut rows_i = Vec::new();
    let mut entries: Vec<(String, String, f64)> = mls.iter().enumerate().map(|(i, ml)| (format!("indirect.{}", i + 1), format!("{xl} → {ml} → {yl}"), ind[i])).collect();
    if k > 1 {
        entries.push(("indirect.total".into(), "Total indirect".into(), ind.iter().sum()));
    }
    for (j, (key, name, est)) in entries.iter().enumerate() {
        let v = &mut reps[if k > 1 && j == k { k } else { j }];
        if v.len() < spec.resamples / 2 {
            continue;
        }
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        let se = (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (v.len() - 1) as f64).sqrt();
        let (lo, hi) = (pct(v, alpha / 2.0), pct(v, 1.0 - alpha / 2.0));
        for (f, val) in [("effect", *est), ("boot_se", se), ("lower", lo), ("upper", hi)] {
            set(&mut b, format!("{key}.{f}"), val);
        }
        set_text(&mut b, format!("{key}.label"), name.clone());
        rows_i.push(vec![text(name), num(*est, 4), num(se, 4), num(lo, 4), num(hi, 4)]);
    }
    b.tables.push(Table {
        id: "indirect".into(),
        title: "Indirect effects".into(),
        columns: vec!["Effect".into(), "Effect".into(), "Bootstrap SE".into(), format!("{lv} CI lower"), format!("{lv} CI upper")],
        rows: rows_i,
        footnotes: vec![format!("Percentile bootstrap intervals from {} resamples (seed {}); an interval that excludes zero indicates an indirect effect.", spec.resamples, spec.seed)],
    });
    b.diagnostics.push(size_diagnostic(n, format!("{n} cases with every variable")));
    b.diagnostics.push(not_assessed("causal", "Causal order", "Mediation estimates assume X comes before the mediators and the mediators before Y, and no unmeasured confounding. Cross-sectional data can't show that order."));
    Ok(b)
}

// ---------------------------------------------------------------- Moderation

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Probe {
    /// The 16th, 50th and 84th percentiles of W (PROCESS's default).
    #[default]
    Percentiles,
    /// The mean of W and one SD either side.
    MeanSd,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModerationSpec {
    pub x: String,
    pub w: String,
    pub y: String,
    #[serde(default)]
    pub covariates: Vec<String>,
    /// Mean-center X and W before forming the product.
    #[serde(default)]
    pub center: bool,
    #[serde(default)]
    pub probe: Probe,
    #[serde(default = "yes")]
    pub johnson_neyman: bool,
    #[serde(default = "yes")]
    pub plot: bool,
    #[serde(default = "default_level")]
    pub level: f64,
}

fn yes() -> bool {
    true
}

/// R's type 7 quantile.
fn quantile(sorted: &[f64], q: f64) -> f64 {
    let pos = q * (sorted.len() - 1) as f64;
    let (lo, hi) = (pos.floor() as usize, pos.ceil() as usize);
    sorted[lo] + (sorted[hi] - sorted[lo]) * (pos - lo as f64)
}

pub fn moderation(ds: &Dataset, spec: &ModerationSpec) -> Result<ResultBundle> {
    check_level(spec.level)?;
    let names: Vec<&String> = [&spec.x, &spec.w, &spec.y].into_iter().chain(&spec.covariates).collect();
    let (cols, tally, total) = complete(ds, &names)?;
    let n = cols[0].len();
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    let (mx, mw) = (mean(&cols[0]), mean(&cols[1]));
    let (x, w): (Vec<f64>, Vec<f64>) = if spec.center { (cols[0].iter().map(|v| v - mx).collect(), cols[1].iter().map(|v| v - mw).collect()) } else { (cols[0].clone(), cols[1].clone()) };
    let y = &cols[2];
    let xw: Vec<f64> = x.iter().zip(&w).map(|(a, c)| a * c).collect();
    let (xl, wl, yl) = (label(ds, &spec.x)?, label(ds, &spec.w)?, label(ds, &spec.y)?);
    let lv = level_text(spec.level);
    let note = format!(
        "Moderation analysis (Hayes's PROCESS model 1): {yl} regressed on {xl}, {wl} and their product{}{} by ordinary least squares. The slope of {xl} is probed at {} of {wl}{}. Cases were used when every variable had a valid value. Computed by PrismStatistics.",
        if spec.center { ", with both mean-centered first" } else { "" },
        if spec.covariates.is_empty() { String::new() } else { format!(", adjusting for {}", spec.covariates.len()) },
        if spec.probe == Probe::Percentiles { "the 16th, 50th and 84th percentiles" } else { "the mean and one SD below and above" },
        if spec.johnson_neyman { ", with Johnson-Neyman values where it turns significant" } else { "" }
    );
    let mut b = common::bundle(MODERATION, METHOD_VERSION, format!("Moderation: {xl} × {wl} → {yl}"), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, n), note);
    for (k, v) in [("x.label", &xl), ("w.label", &wl), ("y.label", &yl)] {
        set_text(&mut b, k, v.clone());
    }
    set(&mut b, "n", n as f64);
    set(&mut b, "level", spec.level);
    let preds: Vec<Vec<f64>> = [x.clone(), w.clone(), xw].into_iter().chain(cols[3..].iter().cloned()).collect();
    let Some(o) = ols(y, &preds) else {
        fail(&mut b, "The regression can't be estimated: too few cases, or a variable is constant.");
        return Ok(b);
    };
    let names_t = ["(Constant)".to_string(), xl.clone(), wl.clone(), format!("{xl} × {wl}")];
    let mut rows = Vec::new();
    for j in 0..o.b.len() {
        let (est, se, t, pv, lo, hi) = coef_row(&o, j, spec.level);
        let key = ["b0", "x", "w", "xw"].get(j).map_or_else(|| format!("cov.{}", j - 3), |s| s.to_string());
        for (f, v) in [("b", est), ("se", se), ("t", t), ("p", pv), ("lower", lo), ("upper", hi)] {
            set(&mut b, format!("{key}.{f}"), v);
        }
        let name = names_t.get(j).cloned().unwrap_or_else(|| label(ds, &spec.covariates[j - 4]).unwrap_or_default());
        rows.push(vec![text(name), num(est, 4), num(se, 4), num(t, 3), p(pv), num(lo, 4), num(hi, 4)]);
    }
    set(&mut b, "r2", o.r2);
    set(&mut b, "df", o.df);
    // R² change from the interaction.
    let reduced: Vec<Vec<f64>> = [x.clone(), w.clone()].into_iter().chain(cols[3..].iter().cloned()).collect();
    if let Some(r0) = ols(y, &reduced) {
        let dr2 = o.r2 - r0.r2;
        let f = dr2 / ((1.0 - o.r2) / o.df);
        set(&mut b, "r2.change", dr2);
        set(&mut b, "r2.change.F", f);
        set(&mut b, "r2.change.p", prismqda_stats::dist::f_sf(f, 1.0, o.df));
    }
    b.tables.push(Table {
        id: "model".into(),
        title: format!("Model of {yl}"),
        columns: vec!["Term".into(), "b".into(), "SE".into(), "t".into(), "p".into(), format!("{lv} CI lower"), format!("{lv} CI upper")],
        rows,
        footnotes: vec![format!("R² = {}.{}", apa::num(o.r2, 3), if spec.center { format!(" {xl} and {wl} are mean-centered, so each main effect is the slope at the other's mean.") } else { String::new() })],
    });
    // Simple slopes.
    let slope = |wv: f64| -> (f64, f64) {
        let s = o.b[1] + o.b[3] * wv;
        let v = o.vcov[1][1] + 2.0 * wv * o.vcov[1][3] + wv * wv * o.vcov[3][3];
        (s, v.max(0.0).sqrt())
    };
    let shift = if spec.center { mw } else { 0.0 };
    let wraw = &cols[1];
    let mut sorted = wraw.clone();
    sorted.sort_by(f64::total_cmp);
    let sd = (wraw.iter().map(|v| (v - mw).powi(2)).sum::<f64>() / (n - 1) as f64).sqrt();
    let points: Vec<(String, f64)> = match spec.probe {
        Probe::Percentiles => vec![("16th percentile".into(), quantile(&sorted, 0.16)), ("50th percentile".into(), quantile(&sorted, 0.5)), ("84th percentile".into(), quantile(&sorted, 0.84))],
        Probe::MeanSd => vec![("Mean − 1 SD".into(), mw - sd), ("Mean".into(), mw), ("Mean + 1 SD".into(), mw + sd)],
    };
    let q = t_quantile(1.0 - (1.0 - spec.level) / 2.0, o.df);
    let mut rows_s = Vec::new();
    for (i, (name, wv)) in points.iter().enumerate() {
        let (s, se) = slope(wv - shift);
        let t = s / se;
        let pv = t_two_sided(t, o.df);
        let key = format!("slope.{}", i + 1);
        set_text(&mut b, format!("{key}.label"), name.clone());
        for (f, v) in [("w", *wv), ("b", s), ("se", se), ("t", t), ("p", pv), ("lower", s - q * se), ("upper", s + q * se)] {
            set(&mut b, format!("{key}.{f}"), v);
        }
        rows_s.push(vec![text(name), num(*wv, 3), num(s, 4), num(se, 4), num(t, 3), p(pv), num(s - q * se, 4), num(s + q * se, 4)]);
    }
    b.tables.push(Table {
        id: "slopes".into(),
        title: format!("Slope of {xl} at values of {wl}"),
        columns: vec!["At".into(), wl.clone(), "Slope".into(), "SE".into(), "t".into(), "p".into(), format!("{lv} CI lower"), format!("{lv} CI upper")],
        rows: rows_s,
        footnotes: vec![format!("Values of {wl} are on its own scale.")],
    });
    // Johnson-Neyman: (b1 + b3 w)² = q² (v11 + 2 w v13 + w² v33).
    if spec.johnson_neyman {
        let (b1, b3) = (o.b[1], o.b[3]);
        let (v11, v13, v33) = (o.vcov[1][1], o.vcov[1][3], o.vcov[3][3]);
        let qa = b3 * b3 - q * q * v33;
        let qb = 2.0 * (b1 * b3 - q * q * v13);
        let qc = b1 * b1 - q * q * v11;
        let disc = qb * qb - 4.0 * qa * qc;
        let (lo_w, hi_w) = (sorted[0], sorted[n - 1]);
        let mut bounds = Vec::new();
        if disc >= 0.0 && qa != 0.0 {
            let mut r = [(-qb - disc.sqrt()) / (2.0 * qa), (-qb + disc.sqrt()) / (2.0 * qa)];
            r.sort_by(f64::total_cmp);
            bounds = r.iter().map(|v| v + shift).collect();
        }
        let inside: Vec<f64> = bounds.iter().copied().filter(|v| *v >= lo_w && *v <= hi_w).collect();
        for (i, v) in bounds.iter().enumerate() {
            set(&mut b, format!("jn.{}", i + 1), *v);
        }
        set(&mut b, "jn.inside", inside.len() as f64);
        let sig_at = |wv: f64| {
            let (s, se) = slope(wv - shift);
            (s / se).abs() >= q
        };
        let describe = if bounds.is_empty() {
            if sig_at(mw) { format!("The slope of {xl} is significant at every value of {wl}.") } else { format!("The slope of {xl} is not significant at any value of {wl}.") }
        } else {
            let mid = (bounds[0] + bounds[1]) / 2.0;
            let between = sig_at(mid);
            format!(
                "The slope of {xl} is significant {} {} and {} (the observed range of {wl} is {} to {}).",
                if between { "between" } else { "below and above, that is outside" },
                apa::num(bounds[0], 3),
                apa::num(bounds[1], 3),
                apa::num(lo_w, 3),
                apa::num(hi_w, 3)
            )
        };
        set_text(&mut b, "jn.text", describe.clone());
        b.tables.push(Table { id: "jn".into(), title: "Johnson-Neyman values".into(), columns: vec!["Value of the moderator".into()], rows: if bounds.is_empty() { vec![vec![text("None")]] } else { bounds.iter().map(|v| vec![num(*v, 4)]).collect() }, footnotes: vec![describe, format!("At these values of {wl} the {lv} interval of the slope of {xl} just touches zero.")] });
    }
    if spec.plot {
        let (xmin, xmax) = cols[0].iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, c), v| (a.min(*v), c.max(*v)));
        let covmeans: Vec<f64> = cols[3..].iter().map(|c| mean(c)).collect();
        let predict = |xv: f64, wv: f64| {
            let (xc, wc) = if spec.center { (xv - mx, wv - mw) } else { (xv, wv) };
            o.b[0] + o.b[1] * xc + o.b[2] * wc + o.b[3] * xc * wc + covmeans.iter().enumerate().map(|(j, m)| o.b[4 + j] * m).sum::<f64>()
        };
        let curves: Vec<(String, Vec<(f64, f64)>)> = points.iter().map(|(name, wv)| (format!("{wl} at {} ({})", name.to_lowercase(), apa::num(*wv, 2)), (0..=20).map(|s| { let xv = xmin + (xmax - xmin) * s as f64 / 20.0; (xv, predict(xv, *wv)) }).collect())).collect();
        let pts: Vec<(f64, f64)> = cols[0].iter().copied().zip(y.iter().copied()).collect();
        let alt = format!("Predicted {yl} against {xl} at three values of {wl}, over the data.");
        b.charts.push(Chart { id: "slopes".into(), title: format!("Simple slopes of {xl} at values of {wl}"), alt: alt.clone(), svg: chart::scatter_curves(&pts, &curves, &xl, &yl, &alt) });
    }
    b.diagnostics.push(size_diagnostic(n, format!("{n} cases with every variable")));
    Ok(b)
}
