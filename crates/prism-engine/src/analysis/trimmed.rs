//! Robust comparisons of trimmed means (checklist B6b), following Wilcox's methods as R's WRS2 implements them:
//!
//! * Yuen's test of two independent groups (`robust.yuen`, WRS2::yuen): trimmed means and winsorized variances, Welch-type
//!   degrees of freedom.
//! * Yuen's test of paired measures (`robust.yuen_paired`, WRS2::yuend).
//! * The heteroscedastic one-way ANOVA of trimmed means (`robust.t1way`, WRS2::t1way).
//!
//! The default trimming is 20% from each end, Wilcox's recommendation. Each variable or pair uses its own valid cases.

use prismqda_stats::dist::{f_sf, t_quantile, t_two_sided};
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::apa;
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::model::format_code;
use crate::result::{ResultBundle, Table};

pub const YUEN: &str = "robust.yuen";
pub const YUEN_PAIRED: &str = "robust.yuen_paired";
pub const T1WAY: &str = "robust.t1way";
pub const METHOD_VERSION: u32 = 1;

fn default_trim() -> f64 {
    0.2
}

fn default_level() -> f64 {
    0.95
}

/// The trimmed mean, the winsorized variance, and h (the number left after trimming).
pub fn trimmed_stats(x: &[f64], trim: f64) -> (f64, f64, usize) {
    let mut s = x.to_vec();
    s.sort_by(f64::total_cmp);
    let n = s.len();
    let g = (trim * n as f64).floor() as usize;
    let h = n - 2 * g;
    let tm = s[g..n - g].iter().sum::<f64>() / h as f64;
    let w = winsorize(x, trim);
    let m = w.iter().sum::<f64>() / n as f64;
    let var = w.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (n - 1) as f64;
    (tm, var, h)
}

/// The values winsorized at the trimming points (each value keeps its place).
fn winsorize(x: &[f64], trim: f64) -> Vec<f64> {
    let mut s = x.to_vec();
    s.sort_by(f64::total_cmp);
    let n = s.len();
    let g = (trim * n as f64).floor() as usize;
    let (lo, hi) = (s[g], s[n - g - 1]);
    x.iter().map(|v| v.clamp(lo, hi)).collect()
}

fn check_trim(t: f64) -> Result<()> {
    if !(0.0..0.5).contains(&t) {
        return Err(Error::Invalid("The proportion trimmed from each end must be at least 0 and below 0.5 (0.2 is usual).".into()));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct YuenSpec {
    pub test_variables: Vec<String>,
    pub grouping_variable: String,
    pub groups: (f64, f64),
    #[serde(default = "default_trim")]
    pub trim: f64,
    #[serde(default = "default_level")]
    pub level: f64,
}

pub fn yuen(ds: &Dataset, spec: &YuenSpec) -> Result<Vec<ResultBundle>> {
    check_trim(spec.trim)?;
    check_level(spec.level)?;
    if spec.test_variables.is_empty() {
        return Err(Error::Invalid("Choose at least one test variable.".into()));
    }
    spec.test_variables.iter().map(|y| yuen_one(ds, spec, y)).collect()
}

fn yuen_one(ds: &Dataset, spec: &YuenSpec, name: &str) -> Result<ResultBundle> {
    let y_var = ds.variable(name)?;
    let g_var = ds.variable(&spec.grouping_variable)?;
    let (ys, gs) = (numeric(ds, name)?, numeric(ds, &spec.grouping_variable)?);
    let (c1, c2) = spec.groups;
    let mut tally = Tally::default();
    let (mut x1, mut x2) = (Vec::new(), Vec::new());
    for (y, g) in ys.iter().zip(&gs) {
        if tally.check(g, "grouping variable") || tally.check(y, "test variable") {
            continue;
        }
        let gv = g.value().expect("checked");
        if gv == c1 {
            x1.push(y.value().expect("checked"));
        } else if gv == c2 {
            x2.push(y.value().expect("checked"));
        } else {
            tally.add("in neither group");
        }
    }
    let (l1, l2) = (g_var.value_text(c1), g_var.value_text(c2));
    let pct = apa::num(spec.trim * 100.0, 0);
    let lv = level_text(spec.level);
    let note = format!("Yuen's test of {pct}% trimmed means for two independent groups ({l1} = {}, {l2} = {}), with winsorized variances and Welch-type degrees of freedom, as R's WRS2::yuen; {lv} confidence interval for the difference ({l1} minus {l2}). Cases were used when both variables had valid values. Computed by PrismStatistics.", format_code(c1), format_code(c2));
    let mut b = common::bundle(YUEN, METHOD_VERSION, format!("Yuen's Test: {}", y_var.display_name()), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(ys.len(), x1.len() + x2.len()), note);
    set_text(&mut b, "y.label", y_var.display_name());
    set_text(&mut b, "g1.label", l1.clone());
    set_text(&mut b, "g2.label", l2.clone());
    set(&mut b, "trim", spec.trim);
    set(&mut b, "level", spec.level);
    if x1.len() < 3 || x2.len() < 3 {
        fail(&mut b, format!("Each group needs at least three values. {l1}: {}, {l2}: {}.", x1.len(), x2.len()));
        return Ok(b);
    }
    let (m1, v1, h1) = trimmed_stats(&x1, spec.trim);
    let (m2, v2, h2) = trimmed_stats(&x2, spec.trim);
    let d1 = (x1.len() - 1) as f64 * v1 / (h1 * (h1 - 1)) as f64;
    let d2 = (x2.len() - 1) as f64 * v2 / (h2 * (h2 - 1)) as f64;
    let se = (d1 + d2).sqrt();
    if se == 0.0 {
        fail(&mut b, "Neither group varies after winsorizing, so the test statistic is undefined.");
        return Ok(b);
    }
    let diff = m1 - m2;
    let t = diff / se;
    let df = (d1 + d2).powi(2) / (d1 * d1 / (h1 - 1) as f64 + d2 * d2 / (h2 - 1) as f64);
    let pv = t_two_sided(t, df);
    let q = t_quantile(1.0 - (1.0 - spec.level) / 2.0, df);
    for (k, v) in [("g1.n", x1.len() as f64), ("g2.n", x2.len() as f64), ("g1.tmean", m1), ("g2.tmean", m2), ("diff", diff), ("se", se), ("t", t), ("df", df), ("p", pv), ("ci.lower", diff - q * se), ("ci.upper", diff + q * se)] {
        set(&mut b, k, v);
    }
    b.tables.push(Table {
        id: "groups".into(),
        title: "Trimmed means".into(),
        columns: vec![g_var.display_name().into(), "N".into(), format!("{pct}% trimmed mean"), "Winsorized SD".into()],
        rows: vec![vec![text(&l1), count(x1.len()), num(m1, 3), num(v1.sqrt(), 3)], vec![text(&l2), count(x2.len()), num(m2, 3), num(v2.sqrt(), 3)]],
        footnotes: Vec::new(),
    });
    b.tables.push(Table {
        id: "test".into(),
        title: "Yuen's test".into(),
        columns: vec!["Difference".into(), "SE".into(), "t".into(), "df".into(), "p".into(), format!("{lv} CI lower"), format!("{lv} CI upper")],
        rows: vec![vec![num(diff, 3), num(se, 3), num(t, 3), num(df, 2), p(pv), num(diff - q * se, 3), num(diff + q * se, 3)]],
        footnotes: vec![format!("Difference = {l1} minus {l2}.")],
    });
    b.diagnostics.push(size_diagnostic(x1.len().min(x2.len()), format!("The smaller group has {} cases", x1.len().min(x2.len()))));
    Ok(b)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairedSpec {
    pub pairs: Vec<(String, String)>,
    #[serde(default = "default_trim")]
    pub trim: f64,
    #[serde(default = "default_level")]
    pub level: f64,
}

pub fn yuen_paired(ds: &Dataset, spec: &PairedSpec) -> Result<Vec<ResultBundle>> {
    check_trim(spec.trim)?;
    check_level(spec.level)?;
    if spec.pairs.is_empty() {
        return Err(Error::Invalid("Choose at least one pair of variables.".into()));
    }
    spec.pairs.iter().map(|(a, c)| paired_one(ds, spec, a, c)).collect()
}

fn paired_one(ds: &Dataset, spec: &PairedSpec, first: &str, second: &str) -> Result<ResultBundle> {
    let (l1, l2) = (ds.variable(first)?.display_name().to_string(), ds.variable(second)?.display_name().to_string());
    let (x, y, tally, total) = super::ttest_paired::matched(ds, first, second)?;
    let pct = apa::num(spec.trim * 100.0, 0);
    let lv = level_text(spec.level);
    let note = format!("Yuen's test of {pct}% trimmed means for paired measures ({l1} minus {l2}), with the winsorized covariance, as R's WRS2::yuend; {lv} confidence interval. Cases were used when both variables had valid values. Computed by PrismStatistics.");
    let mut b = common::bundle(YUEN_PAIRED, METHOD_VERSION, format!("Paired Yuen Test: {l1} and {l2}"), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, x.len()), note);
    set_text(&mut b, "x1.label", l1.clone());
    set_text(&mut b, "x2.label", l2.clone());
    set(&mut b, "trim", spec.trim);
    set(&mut b, "level", spec.level);
    let n = x.len();
    if n < 3 {
        fail(&mut b, format!("The test needs at least three cases with both values; there are {n}."));
        return Ok(b);
    }
    let g = (spec.trim * n as f64).floor() as usize;
    let h = n - 2 * g;
    let (wx, wy) = (winsorize(&x, spec.trim), winsorize(&y, spec.trim));
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    let (mx, my) = (mean(&wx), mean(&wy));
    let q1: f64 = wx.iter().map(|v| (v - mx).powi(2)).sum();
    let q2: f64 = wy.iter().map(|v| (v - my).powi(2)).sum();
    let q3: f64 = wx.iter().zip(&wy).map(|(a, c)| (a - mx) * (c - my)).sum();
    let se = ((q1 + q2 - 2.0 * q3) / (h * (h - 1)) as f64).sqrt();
    let (tx, _, _) = trimmed_stats(&x, spec.trim);
    let (ty, _, _) = trimmed_stats(&y, spec.trim);
    if se == 0.0 {
        fail(&mut b, "The winsorized differences don't vary, so the test statistic is undefined.");
        return Ok(b);
    }
    let diff = tx - ty;
    let t = diff / se;
    let df = (h - 1) as f64;
    let pv = t_two_sided(t, df);
    let q = t_quantile(1.0 - (1.0 - spec.level) / 2.0, df);
    for (k, v) in [("n", n as f64), ("x1.tmean", tx), ("x2.tmean", ty), ("diff", diff), ("se", se), ("t", t), ("df", df), ("p", pv), ("ci.lower", diff - q * se), ("ci.upper", diff + q * se)] {
        set(&mut b, k, v);
    }
    b.tables.push(Table {
        id: "test".into(),
        title: "Paired Yuen test".into(),
        columns: vec!["Pair".into(), format!("{pct}% trimmed mean (1)"), format!("{pct}% trimmed mean (2)"), "Difference".into(), "SE".into(), "t".into(), "df".into(), "p".into(), format!("{lv} CI lower"), format!("{lv} CI upper")],
        rows: vec![vec![text(format!("{l1} − {l2}")), num(tx, 3), num(ty, 3), num(diff, 3), num(se, 3), num(t, 3), num(df, 0), p(pv), num(diff - q * se, 3), num(diff + q * se, 3)]],
        footnotes: vec!["The difference is between the trimmed means of the two measures.".into()],
    });
    b.diagnostics.push(size_diagnostic(n, format!("{n} cases with both values")));
    Ok(b)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct T1waySpec {
    pub dependent: Vec<String>,
    pub factor: String,
    #[serde(default = "default_trim")]
    pub trim: f64,
}

pub fn t1way(ds: &Dataset, spec: &T1waySpec) -> Result<Vec<ResultBundle>> {
    check_trim(spec.trim)?;
    if spec.dependent.is_empty() {
        return Err(Error::Invalid("Choose at least one dependent variable.".into()));
    }
    spec.dependent.iter().map(|y| t1way_one(ds, spec, y)).collect()
}

fn t1way_one(ds: &Dataset, spec: &T1waySpec, name: &str) -> Result<ResultBundle> {
    let y_var = ds.variable(name)?;
    let f_var = ds.variable(&spec.factor)?;
    let super::anova_oneway::Split { groups, levels, tally, total } = super::anova_oneway::split(ds, name, &spec.factor)?;
    let pct = apa::num(spec.trim * 100.0, 0);
    let note = format!("Heteroscedastic one-way ANOVA of {pct}% trimmed means (Wilcox's method, as R's WRS2::t1way): it assumes neither equal variances nor normality. Groups are the categories of {} with data. Computed by PrismStatistics.", f_var.display_name());
    let analyzed: usize = groups.iter().map(Vec::len).sum();
    let mut b = common::bundle(T1WAY, METHOD_VERSION, format!("Robust ANOVA: {} by {}", y_var.display_name(), f_var.display_name()), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, analyzed), note);
    set_text(&mut b, "y.label", y_var.display_name());
    set_text(&mut b, "factor.label", f_var.display_name());
    set(&mut b, "trim", spec.trim);
    let j = groups.len();
    set(&mut b, "k", j as f64);
    if j < 2 || groups.iter().any(|g| g.len() < 3) {
        fail(&mut b, "The test needs at least two groups, each with at least three values.");
        return Ok(b);
    }
    let stats: Vec<(f64, f64, usize, usize)> = groups.iter().map(|g| { let (m, v, h) = trimmed_stats(g, spec.trim); (m, v, h, g.len()) }).collect();
    let w: Vec<f64> = stats.iter().map(|(_, v, h, n)| (h * (h - 1)) as f64 / ((n - 1) as f64 * v)).collect();
    if w.iter().any(|x| !x.is_finite()) {
        fail(&mut b, "A group doesn't vary after winsorizing, so the test is undefined.");
        return Ok(b);
    }
    let u: f64 = w.iter().sum();
    let xt: f64 = w.iter().zip(&stats).map(|(wi, s)| wi * s.0).sum::<f64>() / u;
    let jf = j as f64;
    let a = w.iter().zip(&stats).map(|(wi, s)| wi * (s.0 - xt).powi(2)).sum::<f64>() / (jf - 1.0);
    let sum: f64 = w.iter().zip(&stats).map(|(wi, s)| (1.0 - wi / u).powi(2) / (s.2 - 1) as f64).sum();
    let bb = 2.0 * (jf - 2.0) / (jf * jf - 1.0) * sum;
    let f = a / (1.0 + bb);
    let df1 = jf - 1.0;
    let df2 = 1.0 / (3.0 / (jf * jf - 1.0) * sum);
    let pv = f_sf(f, df1, df2);
    for (k, v) in [("F", f), ("df1", df1), ("df2", df2), ("p", pv), ("n", analyzed as f64)] {
        set(&mut b, k, v);
    }
    let mut rows = Vec::new();
    for (i, (l, s)) in levels.iter().zip(&stats).enumerate() {
        set_text(&mut b, format!("group.{}.label", i + 1), l.label.clone());
        set(&mut b, format!("group.{}.tmean", i + 1), s.0);
        rows.push(vec![text(&l.label), count(s.3), num(s.0, 3), num(s.1.sqrt(), 3)]);
    }
    b.tables.push(Table { id: "groups".into(), title: "Trimmed means".into(), columns: vec![f_var.display_name().into(), "N".into(), format!("{pct}% trimmed mean"), "Winsorized SD".into()], rows, footnotes: Vec::new() });
    b.tables.push(Table { id: "test".into(), title: "Robust ANOVA".into(), columns: vec!["F".into(), "df1".into(), "df2".into(), "p".into()], rows: vec![vec![num(f, 3), num(df1, 0), num(df2, 2), p(pv)]], footnotes: Vec::new() });
    Ok(b)
}
