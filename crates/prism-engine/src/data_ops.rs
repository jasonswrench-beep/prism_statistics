//! Changes to the data that make new variables from old ones (checklist D1–D3): compute a variable from a formula, recode
//! values into the same or new variables, and score a scale from its items (reverse-scored items, a mean or a sum, and a
//! minimum number of items answered). Each is recorded by the API like any other change, so results that used the old
//! values are marked out of date.
//!
//! Formulas use `prismqda_stats::formula`, a subset of R's syntax with R's rules for missing values, checked against R. A
//! user-missing code counts as missing in a formula and in a scale score, as it does in every analysis.

use prismqda_stats::formula::{self, Value};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::{Column, Dataset, NumericCell};

fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(Error::Invalid(message.into()))
}

/// What a change made, for the message after it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// The variables made or changed.
    pub names: Vec<String>,
    /// Cases given a value, and cases left missing (in the first variable).
    pub valid: usize,
    pub missing: usize,
}

/// A variable's values as a formula sees them: user-missing codes are missing.
fn formula_column(ds: &Dataset, index: usize) -> Vec<Value> {
    let var = &ds.variables[index];
    match &ds.columns[index] {
        Column::Numeric(v) => v.iter().map(|x| match x {
            Some(x) if !var.is_user_missing(*x) => Value::Num(*x),
            _ => Value::Na,
        }).collect(),
        Column::Text(v) => v.iter().map(|x| x.as_ref().map_or(Value::Na, |t| Value::Text(t.clone()))).collect(),
    }
}

/// Puts a column under `name`: a new variable, or (with `replace`) the existing one's values.
fn put(ds: &mut Dataset, name: &str, column: Column, label: Option<&str>, replace: bool) -> Result<()> {
    match ds.find(name) {
        Some(i) if replace => {
            ds.columns[i] = column;
        }
        Some(_) => return invalid(format!("There is already a variable named {name}. Choose another name, or choose to replace its values.")),
        None => {
            ds.add_variable(name, column)?;
        }
    }
    if let Some(l) = label.filter(|l| !l.trim().is_empty()) {
        ds.variable_mut(name)?.label = l.trim().to_string();
    }
    Ok(())
}

fn report(names: Vec<String>, column: &Column) -> Report {
    let valid = match column {
        Column::Numeric(v) => v.iter().filter(|x| x.is_some()).count(),
        Column::Text(v) => v.iter().filter(|x| x.is_some()).count(),
    };
    Report { names, valid, missing: column.len() - valid }
}

/// Whether the formula language can read `text`, with the reason when it can't (for the R script export, which rebuilds
/// a formula only after the engine has read it).
pub fn check_formula(text: &str) -> std::result::Result<(), String> {
    formula::parse(text).map(|_| ()).map_err(|e| e.message)
}

/// D1: computes `target` from a formula such as `(pre + post) / 2` or `ifelse(age >= 18, 1, 0)`. A formula giving true or
/// false makes 1 or 0; one giving text makes a text variable.
pub fn compute(ds: &mut Dataset, target: &str, text: &str, label: Option<&str>, replace: bool) -> Result<Report> {
    let f = formula::parse(text).map_err(|e| Error::Invalid(format!("The formula can't be read: {}.", e.message)))?;
    let names: Vec<String> = ds.variables.iter().map(|v| v.name.clone()).collect();
    let columns: Vec<Vec<Value>> = (0..ds.variables.len()).map(|i| formula_column(ds, i)).collect();
    let n = ds.n_rows();
    let values = if columns.is_empty() {
        return invalid("Open data before computing a variable.");
    } else {
        formula::evaluate_columns(&f, &names, &columns).map_err(|e| Error::Invalid(format!("The formula can't be worked out: {}.", e.message)))?
    };
    // A formula that names no variable gives one value; every case gets it.
    let values = if values.len() == n { values } else { vec![values.into_iter().next().unwrap_or(Value::Na); n] };
    let texts = values.iter().any(|v| matches!(v, Value::Text(_)));
    let numbers = values.iter().any(|v| matches!(v, Value::Num(_) | Value::Bool(_)));
    if texts && numbers {
        return invalid("The formula gives numbers for some cases and text for others; a variable holds one or the other.");
    }
    let column = if texts {
        Column::Text(values.into_iter().map(|v| if let Value::Text(t) = v { Some(t) } else { None }).collect())
    } else {
        Column::Numeric(
            values
                .into_iter()
                .map(|v| match v {
                    Value::Num(x) if x.is_finite() => Some(x),
                    Value::Bool(b) => Some(if b { 1.0 } else { 0.0 }),
                    _ => None,
                })
                .collect(),
        )
    };
    if let Some(i) = ds.find(target) {
        if replace && matches!(ds.columns[i], Column::Numeric(_)) != matches!(column, Column::Numeric(_)) {
            return invalid(format!("{target} holds {} and the formula gives {}; compute into a new variable instead.", if matches!(ds.columns[i], Column::Numeric(_)) { "numbers" } else { "text" }, if texts { "text" } else { "numbers" }));
        }
    }
    let r = report(vec![target.to_string()], &column);
    put(ds, target, column, label, replace)?;
    Ok(r)
}

/// Which old values a recode rule matches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum From {
    Value { value: f64 },
    /// From `lo` to `hi`, both included; a missing end means lowest or highest.
    Range {
        #[serde(default)]
        lo: Option<f64>,
        #[serde(default)]
        hi: Option<f64>,
    },
    /// An empty cell or a user-missing code.
    Missing,
    /// Every value no earlier rule matched.
    Else,
}

/// What a matched value becomes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum To {
    Value { value: f64 },
    Missing,
    /// The old value, unchanged.
    Copy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub from: From,
    pub to: To,
}

/// D2: recodes numeric variables by `rules` (the first rule that matches a value applies). With `into`, the new values go
/// to new variables (one name for each), and a value no rule matches is left empty there; without it, the variables are
/// changed in place and unmatched values stay as they were.
pub fn recode(ds: &mut Dataset, variables: &[String], rules: &[Rule], into: Option<&[String]>, label: Option<&str>) -> Result<Report> {
    if variables.is_empty() {
        return invalid("Choose at least one variable to recode.");
    }
    if rules.is_empty() {
        return invalid("Add at least one rule: an old value and the new value it becomes.");
    }
    if let Some(into) = into {
        if into.len() != variables.len() {
            return invalid(format!("Give one new name for each variable: {} variables, {} names.", variables.len(), into.len()));
        }
    }
    let mut out = Vec::new();
    let mut first: Option<Column> = None;
    for (k, name) in variables.iter().enumerate() {
        let i = ds.find(name).ok_or_else(|| Error::Invalid(format!("There is no variable named {name}.")))?;
        let Column::Numeric(old) = &ds.columns[i] else {
            return invalid(format!("{name} holds text; recoding works on numeric variables."));
        };
        let var = &ds.variables[i];
        let new: Vec<Option<f64>> = old
            .iter()
            .map(|x| {
                let missing = x.is_none_or(|v| var.is_user_missing(v));
                let rule = rules.iter().find(|r| match &r.from {
                    From::Value { value } => *x == Some(*value),
                    From::Range { lo, hi } => x.is_some_and(|v| !var.is_user_missing(v) && lo.is_none_or(|l| v >= l) && hi.is_none_or(|h| v <= h)),
                    From::Missing => missing,
                    From::Else => true,
                });
                match rule.map(|r| &r.to) {
                    Some(To::Value { value }) => Some(*value),
                    Some(To::Missing) => None,
                    Some(To::Copy) => *x,
                    None if into.is_some() => None,
                    None => *x,
                }
            })
            .collect();
        let column = Column::Numeric(new);
        match into {
            Some(into) => {
                let target = &into[k];
                let source_label = var.display_name().to_string();
                if first.is_none() {
                    first = Some(column.clone());
                }
                put(ds, target, column, Some(label.unwrap_or(&format!("{source_label} (recoded)"))), false)?;
                out.push(target.clone());
            }
            None => {
                if first.is_none() {
                    first = Some(column.clone());
                }
                ds.columns[i] = column;
                out.push(name.clone());
            }
        }
    }
    Ok(report(out, &first.expect("at least one variable")))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScaleMethod {
    /// The mean of the items answered.
    #[default]
    Mean,
    /// The sum of the items answered.
    Sum,
    /// The mean of the items answered times the number of items: a sum on the full scale's range when some are skipped.
    ProratedSum,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScaleSpec {
    pub target: String,
    pub items: Vec<String>,
    #[serde(default)]
    pub method: ScaleMethod,
    /// Items scored in reverse: each becomes (scale_min + scale_max − value).
    #[serde(default)]
    pub reverse: Vec<String>,
    #[serde(default)]
    pub scale_min: Option<f64>,
    #[serde(default)]
    pub scale_max: Option<f64>,
    /// The fewest items a case must answer to get a score; every item when left out.
    #[serde(default)]
    pub min_items: Option<usize>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub replace: bool,
}

/// D3: a scale score from its items.
pub fn scale_score(ds: &mut Dataset, spec: &ScaleSpec) -> Result<Report> {
    let k = spec.items.len();
    if k < 2 {
        return invalid("Choose at least two items for a scale score.");
    }
    if let Some(r) = spec.reverse.iter().find(|r| !spec.items.contains(r)) {
        return invalid(format!("{r} is marked for reverse scoring but isn't one of the items."));
    }
    if !spec.reverse.is_empty() && (spec.scale_min.is_none() || spec.scale_max.is_none()) {
        return invalid("Reverse scoring needs the lowest and highest possible answers (such as 1 and 5).");
    }
    let need = spec.min_items.unwrap_or(k);
    if need == 0 || need > k {
        return invalid(format!("The minimum number of items answered must be between 1 and {k}."));
    }
    let n = ds.n_rows();
    let mut values: Vec<Vec<Option<f64>>> = Vec::new();
    for item in &spec.items {
        let cells = ds.numeric_cells(item)?;
        let flip = spec.reverse.contains(item);
        let (lo, hi) = (spec.scale_min.unwrap_or(0.0), spec.scale_max.unwrap_or(0.0));
        values.push(
            cells
                .into_iter()
                .map(|c| match c {
                    NumericCell::Value(v) => Some(if flip { lo + hi - v } else { v }),
                    _ => None,
                })
                .collect(),
        );
    }
    let col: Vec<Option<f64>> = (0..n)
        .map(|row| {
            let answered: Vec<f64> = values.iter().filter_map(|v| v[row]).collect();
            if answered.len() < need {
                return None;
            }
            let sum: f64 = answered.iter().sum();
            Some(match spec.method {
                ScaleMethod::Mean => sum / answered.len() as f64,
                ScaleMethod::Sum => sum,
                ScaleMethod::ProratedSum => sum / answered.len() as f64 * k as f64,
            })
        })
        .collect();
    let column = Column::Numeric(col);
    let r = report(vec![spec.target.clone()], &column);
    let label = spec.label.clone().unwrap_or_else(|| {
        format!(
            "{} of {}{}",
            match spec.method {
                ScaleMethod::Mean => "Mean",
                ScaleMethod::Sum => "Sum",
                ScaleMethod::ProratedSum => "Prorated sum",
            },
            spec.items.join(", "),
            if spec.reverse.is_empty() { String::new() } else { format!(" ({} reversed)", spec.reverse.join(", ")) }
        )
    });
    put(ds, &spec.target, column, Some(&label), spec.replace)?;
    Ok(r)
}
