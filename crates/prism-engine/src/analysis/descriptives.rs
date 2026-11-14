//! Descriptives (`desc.descriptives`, 05 §1). Default statistics: N, mean, SD, minimum, maximum (05 §1); the user can
//! add the median, range, standard error, variance and the confidence interval of the mean. Quantiles are Hyndman-Fan
//! type 7 and the SD uses n − 1 (defaults table, rows 13 and 14). Each variable uses its own valid values.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::{Dataset, NumericCell};
use crate::result::{CaseAccounting, Cell, Exclusion, Field, ResultBundle, Status, Table};

pub const METHOD_ID: &str = "desc.descriptives";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Statistic {
    Mean,
    Sd,
    Min,
    Max,
    Median,
    Range,
    Se,
    Variance,
    CiMean,
}

pub fn default_statistics() -> Vec<Statistic> {
    vec![Statistic::Mean, Statistic::Sd, Statistic::Min, Statistic::Max]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub variables: Vec<String>,
    #[serde(default = "default_statistics")]
    pub statistics: Vec<Statistic>,
    #[serde(default = "default_level")]
    pub level: f64,
}

fn default_level() -> f64 {
    0.95
}

/// One result for all the chosen variables, as a single table with one row per variable.
pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    if spec.variables.is_empty() {
        return Err(Error::Invalid("Choose at least one variable.".into()));
    }
    if !(spec.level > 0.0 && spec.level < 1.0) {
        return Err(Error::Invalid("The confidence level must be between 0 and 100%.".into()));
    }
    let stats: Vec<Statistic> = order(&spec.statistics);
    let level_txt = crate::apa::num(spec.level * 100.0, 0);
    let mut columns = vec!["Variable".to_string(), "N".to_string(), "Missing".to_string()];
    for s in &stats {
        match s {
            Statistic::CiMean => {
                columns.push(format!("{level_txt}% CI mean, lower"));
                columns.push(format!("{level_txt}% CI mean, upper"));
            }
            other => columns.push(heading(*other).to_string()),
        }
    }
    let mut rows = Vec::new();
    let mut fields = BTreeMap::new();
    let mut total = 0usize;
    let mut worst_missing = (0usize, 0usize);
    let mut names = Vec::new();
    for name in &spec.variables {
        let var = ds.variable(name)?;
        let cells = ds.numeric_cells(name)?;
        total = cells.len();
        let (mut empty, mut umiss) = (0usize, 0usize);
        let x: Vec<f64> = cells
            .iter()
            .filter_map(|c| match c {
                NumericCell::Value(v) => Some(*v),
                NumericCell::SystemMissing => {
                    empty += 1;
                    None
                }
                NumericCell::UserMissing(_) => {
                    umiss += 1;
                    None
                }
            })
            .collect();
        if empty + umiss > worst_missing.0 + worst_missing.1 {
            worst_missing = (empty, umiss);
        }
        let d = prismqda_stats::describe::descriptives(&x, spec.level);
        let key = &var.name;
        names.push(key.clone());
        fields.insert(format!("{key}.label"), Field::Text(var.display_name().to_string()));
        fields.insert(format!("{key}.n"), Field::Number(d.n as f64));
        fields.insert(format!("{key}.missing"), Field::Number((empty + umiss) as f64));
        let mut row = vec![Cell::Text { text: var.display_name().to_string() }, Cell::Count { value: d.n }, Cell::Count { value: empty + umiss }];
        let range = d.min.zip(d.max).map(|(a, b)| b - a);
        for s in &stats {
            let value = match s {
                Statistic::Mean => d.mean,
                Statistic::Sd => d.sd,
                Statistic::Min => d.min,
                Statistic::Max => d.max,
                Statistic::Median => d.median,
                Statistic::Range => range,
                Statistic::Se => d.se,
                Statistic::Variance => d.variance,
                Statistic::CiMean => {
                    let (lo, hi) = d.ci_mean.unzip();
                    row.push(opt(lo));
                    row.push(opt(hi));
                    if let (Some(lo), Some(hi)) = (lo, hi) {
                        fields.insert(format!("{key}.ci.lower"), Field::Number(lo));
                        fields.insert(format!("{key}.ci.upper"), Field::Number(hi));
                    }
                    continue;
                }
            };
            row.push(opt(value));
            if let Some(v) = value {
                fields.insert(format!("{key}.{}", field_name(*s)), Field::Number(v));
            }
        }
        rows.push(row);
    }
    fields.insert("variables".into(), Field::Text(names.join("\u{1f}")));
    fields.insert("statistics".into(), Field::Text(stats.iter().map(|s| field_name(*s)).collect::<Vec<_>>().join(",")));
    fields.insert("level".into(), Field::Number(spec.level));
    let analyzed = total - worst_missing.0 - worst_missing.1;
    Ok(ResultBundle {
        method_id: METHOD_ID.into(),
        method_version: METHOD_VERSION,
        title: if spec.variables.len() == 1 { format!("Descriptives: {}", ds.variable(&spec.variables[0])?.display_name()) } else { "Descriptives".into() },
        settings: serde_json::to_value(spec).expect("a spec always serializes"),
        status: Status::Ok,
        cases: CaseAccounting {
            total,
            analyzed,
            excluded: [("empty (variable with the most missing)", worst_missing.0), ("user-missing (variable with the most missing)", worst_missing.1)]
                .into_iter()
                .filter(|(_, n)| *n > 0)
                .map(|(r, n)| Exclusion { reason: r.into(), count: n })
                .collect(),
        },
        tables: vec![Table { id: "descriptives".into(), title: "Descriptive statistics".into(), columns, rows, footnotes: vec!["Each variable uses its own valid values. SD and variance use n − 1. The median is Hyndman-Fan type 7.".into()] }],
        diagnostics: Vec::new(),
        method_note: "Statistics are computed on each variable's valid values; user-missing codes and empty cells are left out and counted under Missing. Computed by prismqda-stats.".into(),
        fields,
        charts: Vec::new(),
    })
}

fn opt(v: Option<f64>) -> Cell {
    v.map_or(Cell::Empty, |value| Cell::Number { value, decimals: 2 })
}

fn order(chosen: &[Statistic]) -> Vec<Statistic> {
    use Statistic::*;
    [Mean, Sd, Se, Variance, CiMean, Median, Min, Max, Range].into_iter().filter(|s| chosen.contains(s)).collect()
}

fn heading(s: Statistic) -> &'static str {
    match s {
        Statistic::Mean => "Mean",
        Statistic::Sd => "SD",
        Statistic::Min => "Minimum",
        Statistic::Max => "Maximum",
        Statistic::Median => "Median",
        Statistic::Range => "Range",
        Statistic::Se => "SE of mean",
        Statistic::Variance => "Variance",
        Statistic::CiMean => "CI of mean",
    }
}

pub fn field_name(s: Statistic) -> &'static str {
    match s {
        Statistic::Mean => "mean",
        Statistic::Sd => "sd",
        Statistic::Min => "min",
        Statistic::Max => "max",
        Statistic::Median => "median",
        Statistic::Range => "range",
        Statistic::Se => "se",
        Statistic::Variance => "variance",
        Statistic::CiMean => "ci",
    }
}
