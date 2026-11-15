//! Data health (DAT-05) and paste preview (SCR03).
//!
//! The health check reports, for every variable, its type, valid, missing and user-missing counts, unique values and
//! range, and lists findings: numbers stored as text, likely missing-value codes not marked as missing, constant and
//! empty variables, values without a value label in a labeled variable, duplicate identifiers and duplicate cases,
//! identifiers measured as scale, and category spellings that differ only in case or spaces. Three findings come
//! with a fix (convert text to numbers, mark codes as missing, set the measure to nominal); a fix is previewed first
//! (its scope, every value it changes) and only applied when asked. Nothing is ever changed by the check itself:
//! outliers, duplicate cases and spellings are reported, never removed or merged.
//!
//! Pasting a block of cells (tab-separated rows, as spreadsheets copy them) is planned first: every cell it changes
//! with its old and new value, cells whose text isn't a number in a numeric variable (left unchanged), values past
//! the last variable (left out) and the cases it adds past the last row.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::{format_code, Column, Dataset, Measure};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VarHealth {
    pub name: String,
    pub kind: &'static str,
    pub measure: Measure,
    pub valid: usize,
    pub missing: usize,
    pub user_missing: usize,
    pub unique: usize,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

/// A fix a finding offers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Fix {
    /// Convert a text variable to numbers; text that isn't a number becomes empty.
    ToNumeric { name: String },
    /// Add codes to a variable's user-missing values.
    MarkMissing { name: String, codes: Vec<f64> },
    /// Set a variable's measure.
    SetMeasure { name: String, measure: Measure },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Finding {
    pub kind: &'static str,
    /// "review" findings may be problems; "note" findings are worth knowing.
    pub level: &'static str,
    pub variables: Vec<String>,
    pub message: String,
    /// Some of the cases concerned (1-based), at most 20.
    pub cases: Vec<usize>,
    pub fix: Option<Fix>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Health {
    pub rows: usize,
    pub variables: Vec<VarHealth>,
    pub findings: Vec<Finding>,
}

fn first20(v: impl Iterator<Item = usize>) -> Vec<usize> {
    v.take(20).map(|i| i + 1).collect()
}

/// An identifier: nearly every case a different whole number, either running as a sequence (such as 1 to n) or under
/// a name such as id or participant. Opening a text file or a workbook measures one as nominal, and Data Health flags
/// one measured as scale.
pub(crate) fn looks_like_identifier(name: &str, vals: &[f64]) -> bool {
    if vals.is_empty() || !vals.iter().all(|x| x.fract() == 0.0) {
        return false;
    }
    let mut uniq: Vec<u64> = vals.iter().map(|x| x.to_bits()).collect();
    uniq.sort_unstable();
    uniq.dedup();
    let nearly_all = uniq.len() * 10 >= vals.len() * 9;
    let (lo, hi) = (vals.iter().copied().fold(f64::INFINITY, f64::min), vals.iter().copied().fold(f64::NEG_INFINITY, f64::max));
    let sequence = vals.len() >= 10 && nearly_all && hi - lo + 1.0 <= 1.2 * uniq.len() as f64;
    sequence || id_like_name(name) && nearly_all
}

fn id_like_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n == "id" || n.ends_with("_id") || n.ends_with("id") && n.len() <= 6 || n.starts_with("id_") || ["key", "case", "caseid", "respondent", "participant", "subject", "pid", "sid", "record"].contains(&n.as_str())
}

/// Common codes for "missing" in survey data.
const MISSING_CODES: [f64; 12] = [-99.0, -98.0, -97.0, -9.0, -8.0, -1.0, 99.0, 98.0, 999.0, 998.0, 9999.0, 99999.0];

pub fn check(ds: &Dataset) -> Health {
    let n = ds.n_rows();
    let mut vars = Vec::new();
    let mut findings = Vec::new();
    for (i, var) in ds.variables.iter().enumerate() {
        let name = var.name.clone();
        match &ds.columns[i] {
            Column::Numeric(v) => {
                let missing = v.iter().filter(|x| x.is_none()).count();
                let user = v.iter().filter(|x| x.is_some_and(|y| var.is_user_missing(y))).count();
                let vals: Vec<f64> = v.iter().flatten().copied().filter(|y| !var.is_user_missing(*y)).collect();
                let mut uniq: Vec<u64> = vals.iter().map(|x| x.to_bits()).collect();
                uniq.sort_unstable();
                uniq.dedup();
                let (min, max) = (vals.iter().copied().reduce(f64::min), vals.iter().copied().reduce(f64::max));
                vars.push(VarHealth { name: name.clone(), kind: "numeric", measure: var.measure, valid: vals.len(), missing, user_missing: user, unique: uniq.len(), min, max });
                if vals.is_empty() {
                    findings.push(Finding { kind: "empty", level: "review", variables: vec![name.clone()], message: format!("{name} has no valid values."), cases: Vec::new(), fix: None });
                    continue;
                }
                if uniq.len() == 1 && n > 1 {
                    findings.push(Finding { kind: "constant", level: "note", variables: vec![name.clone()], message: format!("{name} has the same value ({}) in every case with a value; it can't explain differences.", format_code(vals[0])), cases: Vec::new(), fix: None });
                }
                // Likely missing-value codes: a common code, not marked missing, sitting outside the other values.
                let others: Vec<f64> = vals.iter().copied().filter(|x| !MISSING_CODES.contains(x)).collect();
                if !others.is_empty() {
                    let (lo, hi) = (others.iter().copied().fold(f64::INFINITY, f64::min), others.iter().copied().fold(f64::NEG_INFINITY, f64::max));
                    let span = (hi - lo).max(1.0);
                    let mut codes: Vec<f64> = MISSING_CODES.iter().copied().filter(|c| vals.contains(c) && (*c < lo - 0.5 * span || *c > hi + 0.5 * span) && var.value_label(*c).is_none()).collect();
                    codes.sort_by(f64::total_cmp);
                    if !codes.is_empty() {
                        let rows: Vec<usize> = (0..n).filter(|&r| v[r].is_some_and(|y| codes.contains(&y))).collect();
                        findings.push(Finding {
                            kind: "missing_codes",
                            level: "review",
                            variables: vec![name.clone()],
                            message: format!(
                                "{name} has {} case{} coded {}, far outside its other values ({} to {}); these look like missing-value codes but are counted as data.",
                                rows.len(),
                                if rows.len() == 1 { "" } else { "s" },
                                codes.iter().map(|c| format_code(*c)).collect::<Vec<_>>().join(", "),
                                format_code(lo),
                                format_code(hi)
                            ),
                            cases: first20(rows.into_iter()),
                            fix: Some(Fix::MarkMissing { name: name.clone(), codes }),
                        });
                    }
                }
                // Values without a label in a labeled variable.
                if !var.value_labels.is_empty() && var.measure != Measure::Scale {
                    let mut bad: Vec<f64> = vals.iter().copied().filter(|x| var.value_label(*x).is_none()).collect();
                    bad.sort_by(f64::total_cmp);
                    bad.dedup();
                    if !bad.is_empty() {
                        let rows: Vec<usize> = (0..n).filter(|&r| v[r].is_some_and(|y| !var.is_user_missing(y) && var.value_label(y).is_none())).collect();
                        findings.push(Finding {
                            kind: "unlabeled",
                            level: "review",
                            variables: vec![name.clone()],
                            message: format!("{name} has value labels, but {} case{} hold{} codes without one ({}): typing errors, or labels still to add?", rows.len(), if rows.len() == 1 { "" } else { "s" }, if rows.len() == 1 { "s" } else { "" }, bad.iter().take(10).map(|c| format_code(*c)).collect::<Vec<_>>().join(", ")),
                            cases: first20(rows.into_iter()),
                            fix: None,
                        });
                    }
                }
                // Identifiers.
                if id_like_name(&name) && uniq.len() < vals.len() {
                    dup_finding(&mut findings, &name, (0..n).map(|r| v[r].map(|x| x.to_bits().to_string())).collect());
                }
                if var.measure == Measure::Scale && looks_like_identifier(&name, &vals) {
                    findings.push(Finding { kind: "identifier", level: "note", variables: vec![name.clone()], message: format!("{name} looks like an identifier (a different whole number for nearly every case) but is measured as scale, so it could be averaged by mistake."), cases: Vec::new(), fix: Some(Fix::SetMeasure { name: name.clone(), measure: Measure::Nominal }) });
                }
            }
            Column::Text(v) => {
                let missing = v.iter().filter(|x| x.is_none()).count();
                let vals: Vec<&str> = v.iter().flatten().map(String::as_str).collect();
                let mut uniq: Vec<&str> = vals.clone();
                uniq.sort_unstable();
                uniq.dedup();
                vars.push(VarHealth { name: name.clone(), kind: "text", measure: var.measure, valid: vals.len(), missing, user_missing: 0, unique: uniq.len(), min: None, max: None });
                if vals.is_empty() {
                    findings.push(Finding { kind: "empty", level: "review", variables: vec![name.clone()], message: format!("{name} has no values."), cases: Vec::new(), fix: None });
                    continue;
                }
                if uniq.len() == 1 && n > 1 {
                    findings.push(Finding { kind: "constant", level: "note", variables: vec![name.clone()], message: format!("{name} is \"{}\" in every case with a value.", uniq[0]), cases: Vec::new(), fix: None });
                }
                // Numbers stored as text.
                let numeric = vals.iter().filter(|t| t.trim().parse::<f64>().is_ok_and(f64::is_finite)).count();
                if numeric * 10 >= vals.len() * 9 && numeric > 0 {
                    let bad: Vec<usize> = (0..n).filter(|&r| v[r].as_ref().is_some_and(|t| !t.trim().parse::<f64>().is_ok_and(f64::is_finite))).collect();
                    findings.push(Finding {
                        kind: "numbers_as_text",
                        level: "review",
                        variables: vec![name.clone()],
                        message: if bad.is_empty() { format!("{name} holds numbers stored as text, so it can't be used where numbers are needed.") } else { format!("{name} is mostly numbers stored as text; {} case{} hold{} text that isn't a number.", bad.len(), if bad.len() == 1 { "" } else { "s" }, if bad.len() == 1 { "s" } else { "" }) },
                        cases: first20(bad.into_iter()),
                        fix: Some(Fix::ToNumeric { name: name.clone() }),
                    });
                }
                // Spellings that differ only in case or spaces.
                let mut groups: HashMap<String, Vec<&str>> = HashMap::new();
                for u in &uniq {
                    groups.entry(u.trim().to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")).or_default().push(u);
                }
                let mut clash: Vec<String> = groups.values().filter(|g| g.len() > 1).map(|g| g.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(" / ")).collect();
                clash.sort();
                if !clash.is_empty() {
                    findings.push(Finding { kind: "spellings", level: "review", variables: vec![name.clone()], message: format!("{name} has categories that differ only in capitals or spaces, so they count as different categories: {}.", clash.into_iter().take(5).collect::<Vec<_>>().join("; ")), cases: Vec::new(), fix: None });
                }
                if id_like_name(&name) && uniq.len() < vals.len() {
                    dup_finding(&mut findings, &name, v.to_vec());
                }
            }
        }
    }
    // Duplicate cases: identical in every variable.
    if n > 1 && !ds.variables.is_empty() {
        let mut seen: HashMap<String, usize> = HashMap::new();
        let mut dups = Vec::new();
        for r in 0..n {
            let key: String = ds.columns.iter().map(|c| match c {
                Column::Numeric(v) => v[r].map_or("·".to_string(), |x| x.to_bits().to_string()),
                Column::Text(v) => v[r].clone().unwrap_or_else(|| "·".into()),
            }).collect::<Vec<_>>().join("\u{1f}");
            if seen.insert(key, r).is_some() {
                dups.push(r);
            }
        }
        if !dups.is_empty() {
            findings.push(Finding { kind: "duplicate_cases", level: "review", variables: Vec::new(), message: format!("{} case{} repeat{} an earlier case exactly, in every variable. Check whether they were entered twice; nothing is removed.", dups.len(), if dups.len() == 1 { "" } else { "s" }, if dups.len() == 1 { "s" } else { "" }), cases: first20(dups.into_iter()), fix: None });
        }
    }
    Health { rows: n, variables: vars, findings }
}

fn dup_finding(findings: &mut Vec<Finding>, name: &str, keys: Vec<Option<String>>) {
    let mut first: HashMap<&str, usize> = HashMap::new();
    let mut rows = Vec::new();
    for (r, k) in keys.iter().enumerate() {
        if let Some(k) = k {
            if first.insert(k.as_str(), r).is_some() {
                rows.push(r);
            }
        }
    }
    if !rows.is_empty() {
        findings.push(Finding { kind: "duplicate_ids", level: "review", variables: vec![name.to_string()], message: format!("{name} looks like an identifier, but {} case{} repeat{} an earlier value.", rows.len(), if rows.len() == 1 { "" } else { "s" }, if rows.len() == 1 { "s" } else { "" }), cases: first20(rows.into_iter()), fix: None });
    }
}

/// What a fix would change: a summary, and up to 50 example changes (case, old, new).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FixPreview {
    pub summary: String,
    pub changes: Vec<(usize, String, String)>,
    pub count: usize,
}

/// Previews a fix, or with `apply` makes it.
pub fn fix(ds: &mut Dataset, f: &Fix, apply: bool) -> Result<FixPreview> {
    match f {
        Fix::ToNumeric { name } => {
            let i = ds.find(name).ok_or_else(|| Error::UnknownVariable(name.clone()))?;
            let Column::Text(v) = &ds.columns[i] else { return Err(Error::Invalid(format!("{name} already holds numbers."))) };
            let new: Vec<Option<f64>> = v.iter().map(|t| t.as_ref().and_then(|s| s.trim().parse::<f64>().ok().filter(|x| x.is_finite()))).collect();
            let lost: Vec<usize> = (0..v.len()).filter(|&r| v[r].is_some() && new[r].is_none()).collect();
            let changes = lost.iter().take(50).map(|&r| (r + 1, v[r].clone().unwrap_or_default(), "(empty)".to_string())).collect();
            let summary = format!(
                "{name} becomes a numeric variable: {} values convert{}.",
                new.iter().filter(|x| x.is_some()).count(),
                if lost.is_empty() { String::new() } else { format!(", and {} text value{} that {} not numbers become{} empty (listed)", lost.len(), if lost.len() == 1 { "" } else { "s" }, if lost.len() == 1 { "is" } else { "are" }, if lost.len() == 1 { "s" } else { "" }) }
            );
            if apply {
                ds.columns[i] = Column::Numeric(new);
                let var = &mut ds.variables[i];
                if var.measure == Measure::Nominal {
                    var.measure = Measure::Scale;
                }
            }
            Ok(FixPreview { summary, count: lost.len(), changes })
        }
        Fix::MarkMissing { name, codes } => {
            let i = ds.find(name).ok_or_else(|| Error::UnknownVariable(name.clone()))?;
            let Column::Numeric(v) = &ds.columns[i] else { return Err(Error::NotNumeric(name.clone())) };
            let rows: Vec<usize> = (0..v.len()).filter(|&r| v[r].is_some_and(|x| codes.contains(&x))).collect();
            let changes = rows.iter().take(50).map(|&r| (r + 1, format_code(v[r].unwrap_or(f64::NAN)), "user-missing".to_string())).collect();
            let summary = format!("{} become{} missing codes of {name}: {} case{} will count as missing in analyses (the values stay in the data).", codes.iter().map(|c| format_code(*c)).collect::<Vec<_>>().join(", "), if codes.len() == 1 { "s" } else { "" }, rows.len(), if rows.len() == 1 { "" } else { "s" });
            if apply {
                let var = &mut ds.variables[i];
                for c in codes {
                    if !var.missing_values.contains(c) {
                        var.missing_values.push(*c);
                    }
                }
            }
            Ok(FixPreview { summary, count: rows.len(), changes })
        }
        Fix::SetMeasure { name, measure } => {
            let var = ds.variable_mut(name)?;
            let word = |m: Measure| match m {
                Measure::Nominal => "nominal",
                Measure::Ordinal => "ordinal",
                Measure::Scale => "scale",
            };
            let summary = format!("The measure of {name} changes from {} to {}; its values stay as they are.", word(var.measure), word(*measure));
            if apply {
                var.measure = *measure;
            }
            Ok(FixPreview { summary, count: 0, changes: Vec::new() })
        }
    }
}

/// One cell a paste changes.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CellChange {
    pub row: usize,
    pub name: String,
    pub old: Option<String>,
    pub new: Option<String>,
    /// Why the cell is left unchanged (text that isn't a number in a numeric variable).
    pub problem: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PastePlan {
    pub changes: Vec<CellChange>,
    /// Cases added past the last row.
    pub new_cases: usize,
    /// Values past the last variable, left out.
    pub left_out: usize,
    pub problems: usize,
}

fn cell_text(col: &Column, r: usize) -> Option<String> {
    match col {
        Column::Numeric(v) => v.get(r).copied().flatten().map(format_code),
        Column::Text(v) => v.get(r).cloned().flatten(),
    }
}

/// Plans (and with `apply`, makes) a paste of tab-separated rows at case `row`, variable `name`.
pub fn paste(ds: &mut Dataset, row: usize, name: &str, text: &str, apply: bool) -> Result<PastePlan> {
    let c0 = ds.find(name).ok_or_else(|| Error::UnknownVariable(name.to_string()))?;
    let n = ds.n_rows();
    if row > n {
        return Err(Error::Invalid(format!("There is no case {} (the data have {n}).", row + 1)));
    }
    let lines: Vec<&str> = text.trim_end_matches(['\r', '\n']).split('\n').map(|l| l.trim_end_matches('\r')).collect();
    if lines.is_empty() || (lines.len() == 1 && lines[0].is_empty()) {
        return Err(Error::Invalid("There is nothing to paste.".into()));
    }
    let nv = ds.variables.len();
    let mut changes = Vec::new();
    let (mut left_out, mut problems) = (0, 0);
    let last_row = row + lines.len();
    let new_cases = last_row.saturating_sub(n);
    for (dr, line) in lines.iter().enumerate() {
        for (dc, raw) in line.split('\t').enumerate() {
            let c = c0 + dc;
            if c >= nv {
                left_out += 1;
                continue;
            }
            let r = row + dr;
            let t = raw.trim();
            let new = if t.is_empty() { None } else { Some(t.to_string()) };
            let old = cell_text(&ds.columns[c], r);
            let problem = match (&ds.columns[c], &new) {
                (Column::Numeric(_), Some(s)) if !s.parse::<f64>().is_ok_and(f64::is_finite) => Some(format!("\"{s}\" is not a number and {} is numeric; the cell is left as it is", ds.variables[c].name)),
                _ => None,
            };
            let same = match (&ds.columns[c], &old, &new) {
                (Column::Numeric(_), Some(a), Some(b)) => a.parse::<f64>().ok() == b.parse::<f64>().ok() && a.parse::<f64>().is_ok(),
                (_, a, b) => a == b,
            };
            if problem.is_some() {
                problems += 1;
            } else if same && r < n {
                continue;
            }
            changes.push(CellChange { row: r, name: ds.variables[c].name.clone(), old, new, problem });
        }
    }
    let plan = PastePlan { changes, new_cases, left_out, problems };
    if apply {
        if new_cases > 0 {
            for col in ds.columns.iter_mut() {
                match col {
                    Column::Numeric(v) => v.extend(std::iter::repeat_n(None, new_cases)),
                    Column::Text(v) => v.extend(std::iter::repeat_n(None, new_cases)),
                }
            }
        }
        for ch in &plan.changes {
            if ch.problem.is_some() {
                continue;
            }
            let c = ds.find(&ch.name).expect("planned");
            match &mut ds.columns[c] {
                Column::Numeric(v) => v[ch.row] = ch.new.as_ref().and_then(|s| s.parse::<f64>().ok()),
                Column::Text(v) => v[ch.row] = ch.new.clone(),
            }
        }
    }
    Ok(plan)
}
