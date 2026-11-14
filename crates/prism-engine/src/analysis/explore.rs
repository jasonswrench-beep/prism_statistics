//! Explore (`desc.explore`): a fuller look at numeric variables, overall or within the groups of a factor, before
//! choosing an analysis: descriptives with the 5% trimmed mean, skewness and kurtosis with standard errors,
//! percentiles, tests of normality (Shapiro-Wilk, and Kolmogorov-Smirnov with Lilliefors' correction), and box plots
//! and normal Q-Q plots.
//!
//! Defaults (05 §1): percentiles by R's default (type 7); skewness G1 and excess kurtosis G2 with their usual standard
//! errors; the Lilliefors p value by Dallal and Wilkinson's approximation, as R's nortest::lillie.test.

use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::apa;
use crate::chart::{self, BoxDraw, Chart};
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Cell, Diagnostic, DiagnosticState, ResultBundle, Table};

pub const METHOD_ID: &str = "desc.explore";
pub const METHOD_VERSION: u32 = 1;

const PERCENTILES: [f64; 7] = [0.05, 0.10, 0.25, 0.50, 0.75, 0.90, 0.95];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub dependent: Vec<String>,
    /// Describe within each category of this variable.
    #[serde(default)]
    pub factor: Option<String>,
    #[serde(default = "yes")]
    pub percentiles: bool,
    #[serde(default = "yes")]
    pub normality: bool,
    #[serde(default = "yes")]
    pub plots: bool,
    #[serde(default = "level95")]
    pub level: f64,
}

fn yes() -> bool {
    true
}

fn level95() -> f64 {
    0.95
}

/// The Kolmogorov-Smirnov test against a normal distribution with the sample's own mean and SD, with Lilliefors'
/// correction: (D, p), the p value by Dallal and Wilkinson's approximation (as R's nortest::lillie.test).
pub fn lilliefors(x: &[f64]) -> Option<(f64, f64)> {
    let mut v: Vec<f64> = x.iter().copied().filter(|a| a.is_finite()).collect();
    let n = v.len();
    if n < 5 {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let nf = n as f64;
    let m = v.iter().sum::<f64>() / nf;
    let sd = (v.iter().map(|a| (a - m).powi(2)).sum::<f64>() / (nf - 1.0)).sqrt();
    if sd.is_nan() || sd <= 0.0 {
        return None;
    }
    let mut d: f64 = 0.0;
    for (i, a) in v.iter().enumerate() {
        let f = prismqda_stats::dist::norm_cdf((a - m) / sd);
        d = d.max((i as f64 + 1.0) / nf - f).max(f - i as f64 / nf);
    }
    let (kd, nd) = if n > 100 { (d * (nf / 100.0).powf(0.49), 100.0) } else { (d, nf) };
    let mut p = (-7.01256 * kd * kd * (nd + 2.78019) + 2.99587 * kd * (nd + 2.78019).sqrt() - 0.122119 + 0.974598 / nd.sqrt() + 1.67997 / nd).exp();
    if p > 0.1 {
        let kk = (nf.sqrt() - 0.01 + 0.85 / nf.sqrt()) * d;
        p = if kk <= 0.302 {
            1.0
        } else if kk <= 0.5 {
            2.76773 - 19.828315 * kk + 80.709644 * kk * kk - 138.55152 * kk.powi(3) + 81.218052 * kk.powi(4)
        } else if kk <= 0.9 {
            -4.901232 + 40.662806 * kk - 97.490286 * kk * kk + 94.029866 * kk.powi(3) - 32.355711 * kk.powi(4)
        } else if kk <= 1.31 {
            6.198765 - 19.558097 * kk + 23.186922 * kk * kk - 12.234627 * kk.powi(3) + 2.423045 * kk.powi(4)
        } else {
            0.0
        };
    }
    Some((d, p))
}

/// The mean of the values with the lowest and highest `share` of them left out (R's mean(x, trim = share)).
pub fn trimmed_mean(x: &[f64], share: f64) -> Option<f64> {
    let s = prismqda_stats::describe::sorted_finite(x);
    let n = s.len();
    if n == 0 {
        return None;
    }
    let lo = (n as f64 * share).floor() as usize;
    let kept = &s[lo..n - lo];
    (!kept.is_empty()).then(|| kept.iter().sum::<f64>() / kept.len() as f64)
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<Vec<ResultBundle>> {
    if spec.dependent.is_empty() {
        return Err(Error::Invalid("Choose at least one variable.".into()));
    }
    check_level(spec.level)?;
    spec.dependent.iter().map(|y| run_one(ds, spec, y)).collect()
}

fn run_one(ds: &Dataset, spec: &Spec, name: &str) -> Result<ResultBundle> {
    let yl = ds.variable(name)?.display_name().to_string();
    // The groups: one per category of the factor, or everyone.
    let (groups, labels, tally, total, fl) = match &spec.factor {
        Some(f) => {
            let s = super::anova_oneway::split(ds, name, f)?;
            let fl = ds.variable(f)?.display_name().to_string();
            (s.groups, s.levels.iter().map(|l| l.label.clone()).collect::<Vec<_>>(), s.tally, s.total, Some(fl))
        }
        None => {
            let cells = numeric(ds, name)?;
            let mut tally = Tally::default();
            let v: Vec<f64> = cells.iter().filter(|c| !tally.check(*c, name)).map(|c| c.value().expect("checked")).collect();
            (vec![v], vec!["All cases".to_string()], tally, cells.len(), None)
        }
    };
    let analyzed: usize = groups.iter().map(Vec::len).sum();
    let lv = level_text(spec.level);
    let title = match &fl {
        Some(f) => format!("Explore: {yl} by {f}"),
        None => format!("Explore: {yl}"),
    };
    let mut b = common::bundle(METHOD_ID, METHOD_VERSION, title, serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, analyzed), format!("Descriptive statistics{}, with a {lv} t-based interval of the mean, the 5% trimmed mean, skewness (G1) and excess kurtosis (G2) with their standard errors. Percentiles by R's default (type 7). Normality: Shapiro-Wilk (Royston's algorithm, as R's shapiro.test) and Kolmogorov-Smirnov with Lilliefors' correction (as R's nortest::lillie.test). Computed by prismqda-stats {}.", fl.as_ref().map(|f| format!(" within each group of {f}")).unwrap_or_default(), super::STATS_VERSION));
    set_text(&mut b, "y.label", yl.clone());
    set(&mut b, "level", spec.level);
    set(&mut b, "k", groups.len() as f64);
    let first = fl.clone().unwrap_or_default();
    let mut rows = Vec::new();
    let mut prow = Vec::new();
    let mut nrow = Vec::new();
    let mut non_normal = Vec::new();
    let mut boxes = Vec::new();
    for (i, (g, lab)) in groups.iter().zip(&labels).enumerate() {
        let d = prismqda_stats::describe::descriptives(g, spec.level);
        let key = format!("group.{}", i + 1);
        set_text(&mut b, format!("{key}.label"), lab.clone());
        set(&mut b, format!("{key}.n"), d.n as f64);
        let tm = trimmed_mean(g, 0.05);
        for (k, v) in [("mean", d.mean), ("sd", d.sd), ("median", d.median), ("trimmed", tm), ("skew", d.skewness), ("skew.se", d.se_skewness), ("kurt", d.kurtosis), ("kurt.se", d.se_kurtosis), ("min", d.min), ("max", d.max), ("iqr", d.iqr)] {
            if let Some(v) = v {
                set(&mut b, format!("{key}.{k}"), v);
            }
        }
        let range = d.min.zip(d.max).map(|(a, c)| c - a);
        rows.push(vec![text(lab), count(d.n), opt(d.mean, 3), opt(d.ci_mean.map(|c| c.0), 3), opt(d.ci_mean.map(|c| c.1), 3), opt(tm, 3), opt(d.median, 3), opt(d.sd, 3), opt(d.min, 3), opt(d.max, 3), opt(range, 3), opt(d.iqr, 3), opt(d.skewness, 3), opt(d.se_skewness, 3), opt(d.kurtosis, 3), opt(d.se_kurtosis, 3)]);
        if spec.percentiles {
            let s = prismqda_stats::describe::sorted_finite(g);
            let mut r = vec![text(lab)];
            for q in PERCENTILES.iter() {
                let v = prismqda_stats::describe::percentile_sorted(&s, *q);
                if let Some(v) = v {
                    set(&mut b, format!("{key}.p{}", (q * 100.0).round() as u32), v);
                }
                r.push(opt(v, 3));
            }
            prow.push(r);
        }
        if spec.normality {
            let sw = prismqda_stats::normality::shapiro_wilk(g);
            let li = lilliefors(g);
            if let Some(s) = &sw {
                set(&mut b, format!("{key}.sw.W"), s.w);
                set(&mut b, format!("{key}.sw.p"), s.p);
                if s.p < 0.05 {
                    non_normal.push(lab.clone());
                }
            }
            if let Some((dd, pp)) = li {
                set(&mut b, format!("{key}.ks.D"), dd);
                set(&mut b, format!("{key}.ks.p"), pp);
            }
            nrow.push(vec![text(lab), count(d.n), opt(sw.map(|s| s.w), 3), opt(sw.map(|s| s.p), 3).map_p(), opt(li.map(|l| l.0), 3), opt(li.map(|l| l.1), 3).map_p()]);
        }
        if let Some(s) = prismqda_stats::charts::box_stats(g) {
            boxes.push(BoxDraw { label: lab.clone(), lower_whisker: s.lower_whisker, q1: s.q1, median: s.median, q3: s.q3, upper_whisker: s.upper_whisker, outliers: s.outliers });
        }
        if spec.plots {
            if let Some((pts, line)) = prismqda_stats::charts::normal_qq(g) {
                let points: Vec<(f64, f64)> = pts.iter().map(|p| (p.theoretical, p.sample)).collect();
                let what = if fl.is_some() { format!("{yl}, {lab}") } else { yl.clone() };
                let alt = format!("Normal Q-Q plot of {what} for {} cases, with the reference line through the quartiles.", g.len());
                b.charts.push(Chart { id: format!("qq{}", i + 1), title: format!("Normal Q-Q plot of {what}"), alt: alt.clone(), svg: chart::scatter(&points, Some((line.intercept, line.slope)), "Normal quantile", &yl, &alt) });
            }
        }
    }
    let mut cols = vec![if first.is_empty() { String::new() } else { first.clone() }, "N".into(), "Mean".into(), format!("{lv} CI lower"), format!("{lv} CI upper"), "5% trimmed mean".into(), "Median".into(), "SD".into(), "Min".into(), "Max".into(), "Range".into(), "IQR".into(), "Skewness".into(), "SE".into(), "Kurtosis".into(), "SE".into()];
    if fl.is_none() {
        cols[0] = String::new();
    }
    b.tables.push(Table { id: "descriptives".into(), title: format!("Descriptive statistics: {yl}"), columns: cols, rows, footnotes: vec!["Kurtosis is excess kurtosis (0 for a normal distribution).".into()] });
    if spec.percentiles {
        let mut c = vec![first.clone()];
        c.extend(PERCENTILES.iter().map(|q| format!("{}th", (q * 100.0).round())));
        b.tables.push(Table { id: "percentiles".into(), title: "Percentiles".into(), columns: c, rows: prow, footnotes: vec!["Type 7 quantiles (R's default).".into()] });
    }
    if spec.normality {
        b.tables.push(Table { id: "normality".into(), title: "Tests of normality".into(), columns: vec![first.clone(), "N".into(), "Shapiro-Wilk W".into(), "p".into(), "Kolmogorov-Smirnov D".into(), "p (Lilliefors)".into()], rows: nrow, footnotes: vec!["With large samples these tests flag trivial departures; with small ones they miss real ones. Read them with the Q-Q plots.".into()] });
        b.diagnostics.push(if non_normal.is_empty() {
            Diagnostic { id: "normality".into(), title: "Normality".into(), state: DiagnosticState::NoConcernDetected, message: "The Shapiro-Wilk test found no clear departure from normality.".into() }
        } else {
            Diagnostic { id: "normality".into(), title: "Normality".into(), state: DiagnosticState::ReviewNeeded, message: format!("Shapiro-Wilk p < .05 for {}. Check the Q-Q plots: with large samples a small departure is often harmless; with small samples a rank-based test may suit better.", apa::series(&non_normal)) }
        });
    }
    if spec.plots && !boxes.is_empty() {
        let alt = format!("Box plots of {yl}{}.", fl.as_ref().map(|f| format!(" for each group of {f}")).unwrap_or_default());
        b.charts.insert(0, Chart { id: "boxplot".into(), title: match &fl { Some(f) => format!("{yl} by {f}"), None => yl.clone() }, alt: alt.clone(), svg: chart::boxplot(&boxes, fl.as_deref().unwrap_or(""), &yl, &alt) });
    }
    if analyzed == 0 {
        fail(&mut b, format!("{yl} has no valid values."));
    }
    Ok(b)
}

/// Shows a p value as a p cell (`< .001`) where it is a number.
trait MapP {
    fn map_p(self) -> Cell;
}

impl MapP for Cell {
    fn map_p(self) -> Cell {
        match self {
            Cell::Number { value, .. } => p(value),
            other => other,
        }
    }
}
