//! Frequencies (`desc.frequencies`, 05 §1). One table per variable: each value with its label, count, percent of all
//! cases, valid percent and cumulative valid percent, then each user-missing code and the empty cells as their own rows
//! (show missing: on). Fields hold the counts and both percentages for each category, so the participant paragraph
//! (10 §3a) can cite them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::{format_code, Column, Dataset, NumericCell};
use crate::result::{CaseAccounting, Cell, Exclusion, Field, ResultBundle, Status, Table};

pub const METHOD_ID: &str = "desc.frequencies";
pub const METHOD_VERSION: u32 = 1;

/// More distinct values than this, and the variable is almost certainly not categorical; the table is refused.
const MAX_CATEGORIES: usize = 200;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub variables: Vec<String>,
}

/// One category of a frequency table, kept for the paragraph writer.
#[derive(Debug, Clone, PartialEq)]
pub struct Category {
    pub key: String,
    pub label: String,
    pub count: usize,
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<Vec<ResultBundle>> {
    if spec.variables.is_empty() {
        return Err(Error::Invalid("Choose at least one variable.".into()));
    }
    spec.variables.iter().map(|name| run_one(ds, name)).collect()
}

fn run_one(ds: &Dataset, name: &str) -> Result<ResultBundle> {
    let i = ds.find(name).ok_or_else(|| Error::UnknownVariable(name.to_string()))?;
    let var = &ds.variables[i];
    let total = ds.n_rows();
    // (key, label, count) for valid values, in value order; missing kept apart.
    let valid: Vec<(String, String, usize)>;
    let mut user_missing: BTreeMap<String, (String, usize)> = BTreeMap::new();
    let mut empty = 0usize;
    match &ds.columns[i] {
        Column::Numeric(_) => {
            let mut counts: Vec<(f64, usize)> = Vec::new();
            let mut miss: Vec<(f64, usize)> = Vec::new();
            for c in ds.numeric_cells(name)? {
                match c {
                    NumericCell::SystemMissing => empty += 1,
                    NumericCell::UserMissing(v) => bump(&mut miss, v),
                    NumericCell::Value(v) => bump(&mut counts, v),
                }
            }
            counts.sort_by(|a, b| a.0.total_cmp(&b.0));
            miss.sort_by(|a, b| a.0.total_cmp(&b.0));
            valid = counts.into_iter().map(|(v, n)| (format_code(v), var.value_text(v), n)).collect();
            for (v, n) in miss {
                user_missing.insert(format_code(v), (var.value_text(v), n));
            }
        }
        Column::Text(values) => {
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            for v in values {
                match v {
                    None => empty += 1,
                    Some(s) => *counts.entry(s.as_str()).or_default() += 1,
                }
            }
            valid = counts.into_iter().map(|(s, n)| (s.to_string(), s.to_string(), n)).collect();
        }
    }

    let n_valid: usize = valid.iter().map(|v| v.2).sum();
    let n_missing = total - n_valid;
    let mut bundle = ResultBundle {
        method_id: METHOD_ID.into(),
        method_version: METHOD_VERSION,
        title: format!("Frequencies: {}", var.display_name()),
        settings: serde_json::json!({ "variables": [name] }),
        status: Status::Ok,
        cases: CaseAccounting {
            total,
            analyzed: n_valid,
            excluded: [("empty", empty), ("user-missing", n_missing - empty)]
                .into_iter()
                .filter(|(_, n)| *n > 0)
                .map(|(r, n)| Exclusion { reason: r.into(), count: n })
                .collect(),
        },
        tables: Vec::new(),
        diagnostics: Vec::new(),
        method_note: "Percent is of all cases; valid percent and cumulative percent are of cases with a valid value. User-missing codes and empty cells are listed separately and never counted as categories.".into(),
        fields: BTreeMap::new(),
        charts: Vec::new(),
    };
    if valid.len() > MAX_CATEGORIES {
        bundle.status = Status::Failed {
            reason: format!("{} has {} different values. A frequency table is for categories; use Descriptives for a measured variable.", var.display_name(), valid.len()),
        };
        return Ok(bundle);
    }

    let pct = |n: usize, base: usize| if base == 0 { f64::NAN } else { 100.0 * n as f64 / base as f64 };
    let mut rows = Vec::new();
    let mut cum = 0usize;
    bundle.fields.insert("var.label".into(), Field::Text(var.display_name().to_string()));
    bundle.fields.insert("var.name".into(), Field::Text(var.name.clone()));
    bundle.fields.insert("n.total".into(), Field::Number(total as f64));
    bundle.fields.insert("n.valid".into(), Field::Number(n_valid as f64));
    bundle.fields.insert("n.missing".into(), Field::Number(n_missing as f64));
    bundle.fields.insert("categories".into(), Field::Text(valid.iter().map(|v| v.0.clone()).collect::<Vec<_>>().join("\u{1f}")));
    for (key, label, n) in &valid {
        cum += n;
        rows.push(vec![
            Cell::Text { text: if label == key { key.clone() } else { format!("{label} ({key})") } },
            Cell::Count { value: *n },
            Cell::Number { value: pct(*n, total), decimals: 1 },
            Cell::Number { value: pct(*n, n_valid), decimals: 1 },
            Cell::Number { value: pct(cum, n_valid), decimals: 1 },
        ]);
        bundle.fields.insert(format!("cat.{key}.label"), Field::Text(label.clone()));
        bundle.fields.insert(format!("cat.{key}.n"), Field::Number(*n as f64));
        bundle.fields.insert(format!("cat.{key}.pct"), Field::Number(pct(*n, total)));
        bundle.fields.insert(format!("cat.{key}.valid_pct"), Field::Number(pct(*n, n_valid)));
    }
    rows.push(vec![Cell::Text { text: "Total valid".into() }, Cell::Count { value: n_valid }, Cell::Number { value: pct(n_valid, total), decimals: 1 }, Cell::Number { value: 100.0, decimals: 1 }, Cell::Empty]);
    for (key, (label, n)) in &user_missing {
        let text = if label == key { format!("Missing: code {key}") } else { format!("Missing: {label} ({key})") };
        rows.push(vec![Cell::Text { text }, Cell::Count { value: *n }, Cell::Number { value: pct(*n, total), decimals: 1 }, Cell::Empty, Cell::Empty]);
    }
    if empty > 0 {
        rows.push(vec![Cell::Text { text: "Missing: empty".into() }, Cell::Count { value: empty }, Cell::Number { value: pct(empty, total), decimals: 1 }, Cell::Empty, Cell::Empty]);
    }
    rows.push(vec![Cell::Text { text: "Total".into() }, Cell::Count { value: total }, Cell::Number { value: 100.0, decimals: 1 }, Cell::Empty, Cell::Empty]);
    bundle.tables.push(Table {
        id: "frequencies".into(),
        title: var.display_name().to_string(),
        columns: vec!["Value".into(), "Frequency".into(), "Percent".into(), "Valid percent".into(), "Cumulative percent".into()],
        rows,
        footnotes: Vec::new(),
    });
    Ok(bundle)
}

fn bump(v: &mut Vec<(f64, usize)>, x: f64) {
    match v.iter_mut().find(|(k, _)| *k == x) {
        Some((_, n)) => *n += 1,
        None => v.push((x, 1)),
    }
}

/// The categories of a frequency result in table order, for the paragraph writer.
pub fn categories(b: &ResultBundle) -> Vec<Category> {
    let Some(keys) = b.text("categories") else { return Vec::new() };
    keys.split('\u{1f}')
        .filter(|k| !k.is_empty())
        .map(|k| Category {
            key: k.to_string(),
            label: b.text(&format!("cat.{k}.label")).unwrap_or(k).to_string(),
            count: b.number(&format!("cat.{k}.n")).unwrap_or(0.0) as usize,
        })
        .collect()
}
