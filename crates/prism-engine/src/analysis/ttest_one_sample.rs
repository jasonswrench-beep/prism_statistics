//! The one-sample t test (`ttest.one_sample`, task T3-08).
//!
//! Defaults (05 §1): test value 0, 95% interval, two-sided. Effect size: Cohen's d as the mean difference over the SD,
//! with Hedges' g beside it. Each test variable uses its own valid values. Computed by
//! `prismqda_stats::ttest::one_sample`.

use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::apa;
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{ResultBundle, Table};

pub const METHOD_ID: &str = "ttest.one_sample";
pub const METHOD_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub test_variables: Vec<String>,
    #[serde(default)]
    pub test_value: f64,
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

pub fn run(ds: &Dataset, spec: &Spec) -> Result<Vec<ResultBundle>> {
    if spec.test_variables.is_empty() {
        return Err(Error::Invalid("Choose at least one test variable.".into()));
    }
    if !spec.test_value.is_finite() {
        return Err(Error::Invalid("The test value must be a number.".into()));
    }
    check_level(spec.level)?;
    spec.test_variables
        .iter()
        .map(|name| run_one(ds, spec, name))
        .collect()
}

fn run_one(ds: &Dataset, spec: &Spec, name: &str) -> Result<ResultBundle> {
    let var = ds.variable(name)?;
    let cells = numeric(ds, name)?;
    let mut tally = Tally::default();
    let x: Vec<f64> = cells
        .iter()
        .filter(|c| !tally.check(*c, "test variable"))
        .filter_map(Slot::value)
        .collect();
    let mu = spec.test_value;
    let lv = level_text(spec.level);
    let note = format!(
        "One-sample t test of the mean against {}, {}, {lv} confidence interval for the mean difference. Effect size: Cohen's d (mean difference divided by the SD) and Hedges' g, with exact intervals from the noncentral t distribution. Cases were used when the test variable had a valid value. Computed by prismqda-stats {}.",
        apa::plain(mu),
        spec.alternative.words(),
        super::STATS_VERSION
    );
    let mut b = common::bundle(
        METHOD_ID,
        METHOD_VERSION,
        format!("One-Sample T Test: {}", var.display_name()),
        serde_json::to_value(spec).expect("a spec always serializes"),
        tally.accounting(cells.len(), x.len()),
        note,
    );
    set_text(&mut b, "y.label", var.display_name());
    set(&mut b, "test_value", mu);
    set(&mut b, "level", spec.level);
    set(&mut b, "n", x.len() as f64);
    b.tables.push(Table {
        id: "stats".into(),
        title: "One-sample statistics".into(),
        columns: vec![
            "Variable".into(),
            "N".into(),
            "Mean".into(),
            "SD".into(),
            "SE of mean".into(),
        ],
        rows: vec![summary_row(var.display_name(), &x)],
        footnotes: Vec::new(),
    });

    set_text(&mut b, "alternative", spec.alternative.key());
    let Some(mut t) = prismqda_stats::ttest::one_sample(&x, mu, spec.level) else {
        fail(
            &mut b,
            if x.len() < 2 {
                format!(
                    "The test needs at least two valid values; there are {}.",
                    x.len()
                )
            } else {
                "Every value is the same, so the t statistic is undefined.".to_string()
            },
        );
        return Ok(b);
    };
    super::sided::apply_t(&mut t, spec.alternative, spec.level);
    let s = t.groups[0];
    for (k, v) in [
        ("mean", s.mean),
        ("sd", s.sd),
        ("t", t.t),
        ("df", t.df),
        ("p", t.p),
        ("diff", t.estimate - mu),
        ("se", t.se),
        ("ci.lower", t.ci.0 - mu),
        ("ci.upper", t.ci.1 - mu),
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
    b.tables.push(Table {
        id: "test".into(),
        title: format!("One-sample t test (test value = {})", apa::plain(mu)),
        columns: vec![
            "Variable".into(),
            "t".into(),
            "df".into(),
            format!("p ({})", spec.alternative.words()),
            "Mean difference".into(),
            format!("{lv} CI lower"),
            format!("{lv} CI upper"),
        ],
        rows: vec![vec![
            text(var.display_name()),
            num(t.t, 3),
            num(t.df, 0),
            p(t.p),
            num(t.estimate - mu, 3),
            bound(t.ci.0 - mu, 3),
            bound(t.ci.1 - mu, 3),
        ]],
        footnotes: super::sided::footnotes(spec.alternative, "Mean difference = mean minus the test value.", "the mean is greater than the test value", "the mean is less than the test value"),
    });
    b.tables
        .push(effect_table(&lv, "Cohen's d (mean difference / SD)", &t));
    if spec.bayes {
        let r = spec.prior_scale.unwrap_or(std::f64::consts::FRAC_1_SQRT_2);
        let n = x.len() as f64;
        super::bayes::add(&mut b, super::bayes::ttest_log_bf(t.t, n, n - 1.0, r), &format!("Cauchy on the standardized effect, scale {}", apa::num(r, 3)));
    }
    b.diagnostics.push(size_diagnostic(
        x.len(),
        format!("{} valid values", x.len()),
    ));
    b.diagnostics.push(not_assessed("shape", "Shape and outliers", "Look at a histogram or boxplot of the values. With a small sample, a skewed shape or an outlier can move the mean a long way."));
    Ok(b)
}

/// The effect-size table shared by the one-sample and paired tests.
pub(crate) fn effect_table(lv: &str, d_name: &str, t: &prismqda_stats::ttest::TTest) -> Table {
    Table {
        id: "effect".into(),
        title: "Effect size".into(),
        columns: vec![
            "Measure".into(),
            "Estimate".into(),
            format!("{lv} CI lower"),
            format!("{lv} CI upper"),
        ],
        rows: vec![
            vec![
                text(d_name),
                num(t.d, 3),
                num(t.d_ci.0, 3),
                num(t.d_ci.1, 3),
            ],
            vec![
                text("Hedges' g (small-sample corrected d)"),
                num(t.g, 3),
                num(t.g_ci.0, 3),
                num(t.g_ci.1, 3),
            ],
        ],
        footnotes: Vec::new(),
    }
}
