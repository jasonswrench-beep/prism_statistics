//! The paired-samples t test (`ttest.paired`, task T3-08).
//!
//! Defaults (05 §1): 95% interval, two-sided, the difference taken as first minus second (swapping the pair flips the
//! sign). Effect size: Cohen's d_z, the mean difference over the SD of the differences, named as such. Cases are used
//! when both variables of the pair have valid values; the others are counted. Computed by
//! `prismqda_stats::ttest::paired`.

use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{ResultBundle, Table};

pub const METHOD_ID: &str = "ttest.paired";
pub const METHOD_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    /// Each pair is (first, second); the difference is first minus second.
    pub pairs: Vec<(String, String)>,
    /// A Bayes factor for the effect (BayesFactor's default prior unless a scale is given).
    #[serde(default)]
    pub bayes: bool,
    #[serde(default)]
    pub prior_scale: Option<f64>,
    /// Two-sided, or one-sided in either direction.
    #[serde(default)]
    pub alternative: super::sided::Alternative,
    #[serde(default = "default_level")]
    pub level: f64,
}

fn default_level() -> f64 {
    0.95
}

/// The cases where both variables of a pair have values, with the reasons the others were left out.
pub(crate) fn matched(
    ds: &Dataset,
    first: &str,
    second: &str,
) -> Result<(Vec<f64>, Vec<f64>, Tally, usize)> {
    if ds.find(first) == ds.find(second) {
        return Err(Error::Invalid(
            "A pair needs two different variables.".into(),
        ));
    }
    let (a, c) = (numeric(ds, first)?, numeric(ds, second)?);
    let mut tally = Tally::default();
    let (mut x, mut y) = (Vec::new(), Vec::new());
    for (u, v) in a.iter().zip(&c) {
        if tally.check(u, "first variable") || tally.check(v, "second variable") {
            continue;
        }
        x.push(u.value().expect("checked"));
        y.push(v.value().expect("checked"));
    }
    Ok((x, y, tally, a.len()))
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<Vec<ResultBundle>> {
    if spec.pairs.is_empty() {
        return Err(Error::Invalid(
            "Choose at least one pair of variables.".into(),
        ));
    }
    check_level(spec.level)?;
    spec.pairs
        .iter()
        .map(|(a, c)| run_one(ds, spec, a, c))
        .collect()
}

fn run_one(ds: &Dataset, spec: &Spec, first: &str, second: &str) -> Result<ResultBundle> {
    let (v1, v2) = (ds.variable(first)?, ds.variable(second)?);
    let (l1, l2) = (v1.display_name().to_string(), v2.display_name().to_string());
    let (x, y, tally, total) = matched(ds, first, second)?;
    let lv = level_text(spec.level);
    let note = format!(
        "Paired-samples t test of the mean difference ({l1} minus {l2}) against zero, {}, {lv} confidence interval. Effect size: Cohen's d_z (mean difference divided by the SD of the differences) and Hedges' g_z, with exact intervals from the noncentral t distribution. Cases were used when both variables had valid values. Computed by prismqda-stats {}.",
        spec.alternative.words(),
        super::STATS_VERSION
    );
    let mut b = common::bundle(
        METHOD_ID,
        METHOD_VERSION,
        format!("Paired-Samples T Test: {l1} and {l2}"),
        serde_json::to_value(spec).expect("a spec always serializes"),
        tally.accounting(total, x.len()),
        note,
    );
    set_text(&mut b, "x1.label", l1.clone());
    set_text(&mut b, "x2.label", l2.clone());
    set(&mut b, "level", spec.level);
    set(&mut b, "n", x.len() as f64);
    b.tables.push(Table {
        id: "stats".into(),
        title: "Paired-samples statistics".into(),
        columns: vec![
            "Variable".into(),
            "N".into(),
            "Mean".into(),
            "SD".into(),
            "SE of mean".into(),
        ],
        rows: vec![summary_row(&l1, &x), summary_row(&l2, &y)],
        footnotes: vec![
            "Both rows use the same cases: those with valid values on both variables.".into(),
        ],
    });
    set_text(&mut b, "alternative", spec.alternative.key());
    let Some(mut t) = prismqda_stats::ttest::paired(&x, &y, spec.level) else {
        fail(
            &mut b,
            if x.len() < 2 {
                format!(
                    "The test needs at least two cases with both values; there are {}.",
                    x.len()
                )
            } else {
                "Every difference is the same, so the t statistic is undefined.".to_string()
            },
        );
        return Ok(b);
    };
    super::sided::apply_t(&mut t, spec.alternative, spec.level);
    let r = prismqda_stats::correlation::pearson(&x, &y, spec.level);
    for (k, v) in [
        ("x1.mean", common_mean(&x)),
        ("x1.sd", common_sd(&x)),
        ("x2.mean", common_mean(&y)),
        ("x2.sd", common_sd(&y)),
        ("diff", t.estimate),
        ("diff.sd", t.groups[0].sd),
        ("se", t.se),
        ("t", t.t),
        ("df", t.df),
        ("p", t.p),
        ("ci.lower", t.ci.0),
        ("ci.upper", t.ci.1),
        ("d", t.d),
        ("d.ci.lower", t.d_ci.0),
        ("d.ci.upper", t.d_ci.1),
        ("g", t.g),
        ("g.ci.lower", t.g_ci.0),
        ("g.ci.upper", t.g_ci.1),
    ] {
        if !v.is_infinite() {
            set(&mut b, k, v);
        }
    }
    if let Some(r) = &r {
        set(&mut b, "r", r.r);
        b.tables.push(Table {
            id: "correlation".into(),
            title: "Paired-samples correlation".into(),
            columns: vec![
                "Pair".into(),
                "N".into(),
                "Pearson r".into(),
                "p (two-sided)".into(),
            ],
            rows: vec![vec![
                text(format!("{l1} & {l2}")),
                count(r.n),
                num(r.r, 3),
                p(r.p),
            ]],
            footnotes: Vec::new(),
        });
    }
    b.tables.push(Table {
        id: "test".into(),
        title: "Paired-samples t test".into(),
        columns: vec![
            "Pair".into(),
            "Mean difference".into(),
            "SD".into(),
            "SE".into(),
            format!("{lv} CI lower"),
            format!("{lv} CI upper"),
            "t".into(),
            "df".into(),
            format!("p ({})", spec.alternative.words()),
        ],
        rows: vec![vec![
            text(format!("{l1} − {l2}")),
            num(t.estimate, 3),
            num(t.groups[0].sd, 3),
            num(t.se, 3),
            bound(t.ci.0, 3),
            bound(t.ci.1, 3),
            num(t.t, 3),
            num(t.df, 0),
            p(t.p),
        ]],
        footnotes: super::sided::footnotes(spec.alternative, &format!("Differences are {l1} minus {l2}."), &format!("{l1} is greater than {l2}"), &format!("{l1} is less than {l2}")),
    });
    b.tables.push(super::ttest_one_sample::effect_table(
        &lv,
        "Cohen's d_z (mean difference / SD of differences)",
        &t,
    ));
    if spec.bayes {
        let r = spec.prior_scale.unwrap_or(std::f64::consts::FRAC_1_SQRT_2);
        let n = x.len() as f64;
        super::bayes::add(&mut b, super::bayes::ttest_log_bf(t.t, n, n - 1.0, r), &format!("Cauchy on the standardized effect, scale {}", crate::apa::num(r, 3)));
    }
    b.diagnostics
        .push(size_diagnostic(x.len(), format!("{} pairs", x.len())));
    b.diagnostics.push(not_assessed("shape", "Shape of the differences", "Look at a histogram of the differences. The test is about their mean, so an outlying difference matters most with few pairs."));
    Ok(b)
}

fn common_mean(x: &[f64]) -> f64 {
    prismqda_stats::describe::mean(x).unwrap_or(f64::NAN)
}

fn common_sd(x: &[f64]) -> f64 {
    prismqda_stats::describe::variance(x).map_or(f64::NAN, f64::sqrt)
}
