//! The independent-samples t test (`ttest.independent`, task T2-02).
//!
//! Defaults (05 §3): Welch's test, 95% interval, two-sided, Hedges' g with the pooled SD, complete cases per test
//! variable. Groups are chosen by **code**, never by label or position, so relabeling a group can't change which cases
//! are compared. The computation is `prismqda_stats::ttest::independent`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::{format_code, Dataset, NumericCell};
use crate::result::{CaseAccounting, Cell, Diagnostic, DiagnosticState, Exclusion, Field, ResultBundle, Status, Table};

pub const METHOD_ID: &str = "ttest.independent";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Estimator {
    /// Welch's unequal-variances test (the default).
    #[default]
    Welch,
    /// Student's pooled-variance test.
    Student,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    /// One result per test variable.
    pub test_variables: Vec<String>,
    pub grouping_variable: String,
    /// The two group codes to compare, in the order the difference is taken (first minus second).
    pub groups: (f64, f64),
    #[serde(default)]
    pub estimator: Estimator,
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

/// Below this many cases in a group, the result is flagged for review.
const SMALL_GROUP: usize = 10;

pub fn run(ds: &Dataset, spec: &Spec) -> Result<Vec<ResultBundle>> {
    if spec.test_variables.is_empty() {
        return Err(Error::Invalid("Choose at least one test variable.".into()));
    }
    if spec.groups.0 == spec.groups.1 {
        return Err(Error::Invalid("The two groups must have different codes.".into()));
    }
    if !(spec.level > 0.0 && spec.level < 1.0) {
        return Err(Error::Invalid("The confidence level must be between 0 and 100%.".into()));
    }
    let group_var = ds.variable(&spec.grouping_variable)?;
    let groups = ds.numeric_cells(&spec.grouping_variable)?;
    spec.test_variables.iter().map(|name| run_one(ds, spec, group_var, &groups, name)).collect()
}

fn run_one(ds: &Dataset, spec: &Spec, group_var: &crate::model::Variable, groups: &[NumericCell], name: &str) -> Result<ResultBundle> {
    let y_var = ds.variable(name)?;
    let ys = ds.numeric_cells(name)?;
    let (c1, c2) = spec.groups;

    let mut x1 = Vec::new();
    let mut x2 = Vec::new();
    let mut excluded: BTreeMap<&str, usize> = BTreeMap::new();
    // Each excluded case is counted once, for the first reason that applies.
    for (g, y) in groups.iter().zip(&ys) {
        let reason = match g {
            NumericCell::SystemMissing => Some("grouping variable empty"),
            NumericCell::UserMissing(_) => Some("grouping variable user-missing"),
            NumericCell::Value(v) if *v != c1 && *v != c2 => Some("in neither compared group"),
            NumericCell::Value(_) => match y {
                NumericCell::SystemMissing => Some("test variable empty"),
                NumericCell::UserMissing(_) => Some("test variable user-missing"),
                NumericCell::Value(_) => None,
            },
        };
        match (reason, g, y) {
            (Some(r), _, _) => *excluded.entry(r).or_default() += 1,
            (None, NumericCell::Value(gv), NumericCell::Value(yv)) => {
                if *gv == c1 {
                    x1.push(*yv)
                } else {
                    x2.push(*yv)
                }
            }
            _ => unreachable!("a case without an exclusion reason has values for both variables"),
        }
    }

    let label1 = group_var.value_text(c1);
    let label2 = group_var.value_text(c2);
    let cases = CaseAccounting {
        total: ys.len(),
        analyzed: x1.len() + x2.len(),
        excluded: excluded.into_iter().map(|(reason, count)| Exclusion { reason: reason.to_string(), count }).collect(),
    };
    let settings = serde_json::to_value(spec).expect("a spec always serializes");
    let method_name = match spec.estimator {
        Estimator::Welch => "Welch's unequal-variances t test",
        Estimator::Student => "Student's pooled-variance t test",
    };
    let method_note = format!(
        "{method_name}, {}, {}% confidence interval for the difference ({} minus {}). Effect size: Hedges' g with the pooled standard deviation, with an exact interval from the noncentral t distribution. Groups were chosen by code ({} = {}, {} = {}). Cases were used when both the grouping and test variables had valid values. Computed by prismqda-stats {}.",
        spec.alternative.words(),
        crate::apa::num(spec.level * 100.0, 0),
        label1,
        label2,
        format_code(c1),
        label1,
        format_code(c2),
        label2,
        STATS_VERSION,
    );

    let mut fields = BTreeMap::new();
    let mut put = |k: &str, v: Field| {
        fields.insert(k.to_string(), v);
    };
    put("y.label", Field::Text(y_var.display_name().to_string()));
    put("g1.label", Field::Text(label1.clone()));
    put("g2.label", Field::Text(label2.clone()));
    put("method", Field::Text(method_name.to_string()));
    put("level", Field::Number(spec.level));
    put("n.total", Field::Number(cases.total as f64));
    put("n.analyzed", Field::Number(cases.analyzed as f64));
    put("g1.n", Field::Number(x1.len() as f64));
    put("g2.n", Field::Number(x2.len() as f64));

    let mut bundle = ResultBundle {
        method_id: METHOD_ID.to_string(),
        method_version: METHOD_VERSION,
        title: format!("Independent-Samples T Test: {}", y_var.display_name()),
        settings,
        status: Status::Ok,
        cases,
        tables: Vec::new(),
        diagnostics: Vec::new(),
        method_note,
        fields,
        charts: Vec::new(),
    };

    let small = x1.len().min(x2.len());
    let result = prismqda_stats::ttest::independent(&x1, &x2, spec.estimator == Estimator::Welch, spec.level);
    bundle.fields.insert("alternative".into(), Field::Text(spec.alternative.key().into()));
    let Some(mut t) = result.filter(|t| t.t.is_finite() && t.df.is_finite()) else {
        let reason = if small < 2 {
            format!("Each group needs at least two valid values. {label1}: {}, {label2}: {}.", x1.len(), x2.len())
        } else {
            "Both groups have no variation, so the t statistic is undefined.".to_string()
        };
        bundle.status = Status::Failed { reason };
        bundle.tables.push(group_table(&label1, &label2, &x1, &x2));
        return Ok(bundle);
    };

    super::sided::apply_t(&mut t, spec.alternative, spec.level);
    for (prefix, g) in [("g1", &t.groups[0]), ("g2", &t.groups[1])] {
        bundle.fields.insert(format!("{prefix}.mean"), Field::Number(g.mean));
        bundle.fields.insert(format!("{prefix}.sd"), Field::Number(g.sd));
    }
    for (k, v) in [
        ("t", t.t),
        ("df", t.df),
        ("p", t.p),
        ("diff", t.estimate),
        ("se", t.se),
        ("ci.lower", t.ci.0),
        ("ci.upper", t.ci.1),
        ("g", t.g),
        ("g.ci.lower", t.g_ci.0),
        ("g.ci.upper", t.g_ci.1),
        ("d", t.d),
        ("d.ci.lower", t.d_ci.0),
        ("d.ci.upper", t.d_ci.1),
    ] {
        if v.is_infinite() {
            continue;
        }
        bundle.fields.insert(k.to_string(), Field::Number(v));
    }

    bundle.tables.push(group_table(&label1, &label2, &x1, &x2));
    let level = crate::apa::num(spec.level * 100.0, 0);
    bundle.tables.push(Table {
        id: "test".into(),
        title: "Independent-samples t test".into(),
        columns: vec![
            "Test".into(),
            "t".into(),
            "df".into(),
            format!("p ({})", spec.alternative.words()),
            "Mean difference".into(),
            "SE".into(),
            format!("{level}% CI lower"),
            format!("{level}% CI upper"),
        ],
        rows: vec![vec![
            Cell::Text { text: method_name.to_string() },
            Cell::Number { value: t.t, decimals: 3 },
            Cell::Number { value: t.df, decimals: if spec.estimator == Estimator::Welch { 2 } else { 0 } },
            Cell::P { value: t.p },
            Cell::Number { value: t.estimate, decimals: 3 },
            Cell::Number { value: t.se, decimals: 3 },
            super::common::bound(t.ci.0, 3),
            super::common::bound(t.ci.1, 3),
        ]],
        footnotes: super::sided::footnotes(spec.alternative, &format!("Mean difference = {label1} minus {label2}."), &format!("{label1} has the greater mean"), &format!("{label1} has the smaller mean")),
    });
    bundle.tables.push(Table {
        id: "effect".into(),
        title: "Effect size".into(),
        columns: vec!["Measure".into(), "Estimate".into(), format!("{level}% CI lower"), format!("{level}% CI upper")],
        rows: vec![
            vec![
                Cell::Text { text: "Hedges' g (pooled SD)".into() },
                Cell::Number { value: t.g, decimals: 3 },
                Cell::Number { value: t.g_ci.0, decimals: 3 },
                Cell::Number { value: t.g_ci.1, decimals: 3 },
            ],
            vec![
                Cell::Text { text: "Cohen's d (pooled SD)".into() },
                Cell::Number { value: t.d, decimals: 3 },
                Cell::Number { value: t.d_ci.0, decimals: 3 },
                Cell::Number { value: t.d_ci.1, decimals: 3 },
            ],
        ],
        footnotes: vec!["Both use the pooled standard deviation, whichever test was run, so they can be compared across studies.".into()],
    });
    if spec.bayes {
        if let Some(st) = prismqda_stats::ttest::independent(&x1, &x2, false, spec.level) {
            let r = spec.prior_scale.unwrap_or(std::f64::consts::FRAC_1_SQRT_2);
            let (n1, n2) = (x1.len() as f64, x2.len() as f64);
            super::bayes::add(&mut bundle, super::bayes::ttest_log_bf(st.t, n1 * n2 / (n1 + n2), n1 + n2 - 2.0, r), &format!("Cauchy on the standardized effect, scale {} (pooled-variance t)", crate::apa::num(r, 3)));
        }
    }

    bundle.diagnostics.push(if small < SMALL_GROUP {
        Diagnostic {
            id: "group_size".into(),
            title: "Group sizes".into(),
            state: DiagnosticState::ReviewNeeded,
            message: format!("A group has fewer than {SMALL_GROUP} cases ({label1}: {}, {label2}: {}). With groups this small the test depends heavily on the shape of the data, so look at the values before relying on the p value.", x1.len(), x2.len()),
        }
    } else {
        Diagnostic {
            id: "group_size".into(),
            title: "Group sizes".into(),
            state: DiagnosticState::NoConcernDetected,
            message: format!("{label1}: {}, {label2}: {}.", x1.len(), x2.len()),
        }
    });
    let (s1, s2) = (t.groups[0].sd, t.groups[1].sd);
    let ratio = s1.max(s2) / s1.min(s2);
    bundle.diagnostics.push(match spec.estimator {
        Estimator::Welch => Diagnostic {
            id: "variances".into(),
            title: "Unequal spread".into(),
            state: DiagnosticState::NoConcernDetected,
            message: format!("Welch's test doesn't assume equal spread. The larger SD is {} times the smaller.", crate::apa::num(ratio, 2)),
        },
        Estimator::Student if ratio > 2.0 => Diagnostic {
            id: "variances".into(),
            title: "Unequal spread".into(),
            state: DiagnosticState::ReviewNeeded,
            message: format!("The larger SD is {} times the smaller, and Student's test assumes equal spread. Welch's test is one click away.", crate::apa::num(ratio, 2)),
        },
        Estimator::Student => Diagnostic {
            id: "variances".into(),
            title: "Unequal spread".into(),
            state: DiagnosticState::NoConcernDetected,
            message: format!("The larger SD is {} times the smaller.", crate::apa::num(ratio, 2)),
        },
    });
    bundle.diagnostics.push(Diagnostic {
        id: "shape".into(),
        title: "Shape and outliers".into(),
        state: DiagnosticState::NotAssessed,
        message: "Look at the boxplot or histogram of each group.".into(),
    });
    Ok(bundle)
}

pub use super::STATS_VERSION;

fn group_table(label1: &str, label2: &str, x1: &[f64], x2: &[f64]) -> Table {
    let row = |label: &str, x: &[f64]| {
        let d = prismqda_stats::describe::descriptives(x, 0.95);
        let num = |v: Option<f64>| v.map_or(Cell::Empty, |value| Cell::Number { value, decimals: 3 });
        vec![Cell::Text { text: label.to_string() }, Cell::Count { value: d.n }, num(d.mean), num(d.sd), num(d.se)]
    };
    Table {
        id: "groups".into(),
        title: "Group statistics".into(),
        columns: vec!["Group".into(), "N".into(), "Mean".into(), "SD".into(), "SE of mean".into()],
        rows: vec![row(label1, x1), row(label2, x2)],
        footnotes: Vec::new(),
    }
}
