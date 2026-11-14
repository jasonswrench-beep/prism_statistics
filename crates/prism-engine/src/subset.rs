//! Which cases an analysis sees (checklist D4–D6): Select cases (a filter formula), Split file (run separately for each
//! category of a variable), and Weight cases (frequency weights, each case counted as many times as its weight).
//!
//! The settings travel with each analysis when it runs, so running it again later (or checking whether it is out of date)
//! uses the same cases. Frequency weights must be whole numbers: a case with weight 3 is three identical cases, which is
//! what a frequency weight means, and every analysis then gives the weighted result. A case with a missing, zero or
//! negative weight is left out.

use prismqda_stats::formula::{self, Value};
use serde::{Deserialize, Serialize};

use crate::analysis::common::{categories, Slot};
use crate::error::{Error, Result};
use crate::model::{Column, Dataset, NumericCell};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataSettings {
    /// Keep the cases for which this formula is true, such as `age >= 18 & site == "North"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    /// Run every analysis separately for each category of this variable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub split: Option<String>,
    /// Frequency weights.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<String>,
}

impl DataSettings {
    pub fn is_empty(&self) -> bool {
        self.filter.is_none() && self.split.is_none() && self.weight.is_none()
    }

    /// Checks the settings against the data without running anything.
    pub fn check(&self, ds: &Dataset) -> Result<()> {
        parts(ds, self).map(|_| ())
    }
}

/// One part of the data an analysis runs on: all of it, or one category of the split variable.
pub struct Part {
    pub data: Dataset,
    /// "sex = Female" for a split; None otherwise.
    pub group: Option<String>,
    /// A sentence for the method note: which cases were used and how they were weighted.
    pub note: String,
}

/// The dataset with only `rows` (a row may repeat), keeping every variable's settings.
pub fn select_rows(ds: &Dataset, rows: &[usize]) -> Dataset {
    let mut out = ds.clone();
    out.columns = ds
        .columns
        .iter()
        .map(|c| match c {
            Column::Numeric(v) => Column::Numeric(rows.iter().map(|&r| v[r]).collect()),
            Column::Text(v) => Column::Text(rows.iter().map(|&r| v[r].clone()).collect()),
        })
        .collect();
    out
}

/// The rows a filter keeps: those where the formula is true (or a nonzero number). Missing counts as not selected, as
/// R's `subset` does.
fn filtered(ds: &Dataset, text: &str) -> Result<Vec<usize>> {
    let f = formula::parse(text).map_err(|e| Error::Invalid(format!("The Select cases condition can't be read: {}.", e.message)))?;
    let names: Vec<String> = ds.variables.iter().map(|v| v.name.clone()).collect();
    let columns: Vec<Vec<Value>> = (0..ds.variables.len())
        .map(|i| {
            let var = &ds.variables[i];
            match &ds.columns[i] {
                Column::Numeric(v) => v.iter().map(|x| match x {
                    Some(x) if !var.is_user_missing(*x) => Value::Num(*x),
                    _ => Value::Na,
                }).collect(),
                Column::Text(v) => v.iter().map(|x| x.as_ref().map_or(Value::Na, |t| Value::Text(t.clone()))).collect(),
            }
        })
        .collect();
    let values = formula::evaluate_columns(&f, &names, &columns).map_err(|e| Error::Invalid(format!("The Select cases condition can't be worked out: {}.", e.message)))?;
    if values.iter().any(|v| matches!(v, Value::Text(_))) {
        return Err(Error::Invalid("The Select cases condition gives text; it should be true or false for each case, such as age >= 18.".into()));
    }
    let n = ds.n_rows();
    if values.len() != n {
        // A condition naming no variable is the same for every case.
        let keep = matches!(values.first(), Some(Value::Bool(true))) || matches!(values.first(), Some(Value::Num(x)) if *x != 0.0);
        return Ok(if keep { (0..n).collect() } else { Vec::new() });
    }
    Ok(values
        .iter()
        .enumerate()
        .filter(|(_, v)| match v {
            Value::Bool(b) => *b,
            Value::Num(x) => *x != 0.0,
            _ => false,
        })
        .map(|(i, _)| i)
        .collect())
}

/// The parts an analysis runs on under `settings`.
pub fn parts(ds: &Dataset, settings: &DataSettings) -> Result<Vec<Part>> {
    let n = ds.n_rows();
    let mut rows: Vec<usize> = (0..n).collect();
    let mut notes = Vec::new();
    if let Some(f) = settings.filter.as_deref().filter(|f| !f.trim().is_empty()) {
        rows = filtered(ds, f)?;
        notes.push(format!("Cases were selected by the condition {} ({} of {n} cases).", f.trim(), rows.len()));
    }
    if let Some(w) = &settings.weight {
        let cells = ds.numeric_cells(w).map_err(|_| Error::Invalid(format!("The weight variable {w} must be a numeric variable in the data.")))?;
        let mut weighted = Vec::new();
        let mut dropped = 0;
        for &r in &rows {
            match cells[r] {
                NumericCell::Value(v) if v > 0.0 => {
                    if v.fract() != 0.0 {
                        return Err(Error::Invalid(format!("Frequency weights must be whole numbers: {w} is {v} in case {}.", r + 1)));
                    }
                    weighted.extend(std::iter::repeat_n(r, v as usize));
                }
                _ => dropped += 1,
            }
        }
        notes.push(format!(
            "Cases were weighted by {} (frequency weights: each case counts as many times as its weight, so counts are weighted counts){}.",
            ds.variable(w)?.display_name(),
            if dropped > 0 { format!("; {dropped} case{} with a missing, zero or negative weight {} left out", if dropped == 1 { "" } else { "s" }, if dropped == 1 { "was" } else { "were" }) } else { String::new() }
        ));
        rows = weighted;
    }
    let base = select_rows(ds, &rows);
    let note = notes.join(" ");
    let Some(split) = &settings.split else {
        return Ok(vec![Part { data: base, group: None, note }]);
    };
    let (slots, levels) = categories(&base, split).map_err(|_| Error::Invalid(format!("There is no variable named {split} to split the file by.")))?;
    let label = base.variable(split)?.display_name().to_string();
    let mut out = Vec::new();
    for (k, level) in levels.iter().enumerate() {
        let these: Vec<usize> = slots.iter().enumerate().filter(|(_, s)| matches!(s, Slot::Valid(v) if *v == k)).map(|(i, _)| i).collect();
        let group = format!("{label} = {}", level.label);
        out.push(Part {
            data: select_rows(&base, &these),
            note: format!("{}{}Split file: these results are for the cases with {group}.", note, if note.is_empty() { "" } else { " " }),
            group: Some(group),
        });
    }
    if out.is_empty() {
        return Err(Error::Invalid(format!("No selected case has a value of {label}, so there is nothing to split.")));
    }
    Ok(out)
}
