//! Data tools that change the shape of the data (checklist task "merge, append, aggregate and reshape"):
//!
//! * **Append cases**: another file's cases below these, variables matched by name (ignoring case); a variable in only
//!   one file is missing for the other's cases; value labels and missing codes are combined; optionally a variable
//!   recording which file each case came from.
//! * **Merge variables**: another file's variables joined on one or more key variables. Each case takes the other
//!   file's matching case; when one file has a key several times, its matches repeat the other's values (one-to-many
//!   or many-to-one); keys repeated in both files are refused. Cases without a match are kept (left join, the
//!   default), dropped (only matched cases) or, with "all", the other file's unmatched cases are added too. A key with
//!   a missing value matches nothing. Variables whose names clash get the suffix _2.
//! * **Aggregate**: summaries of variables within groups of break variables (mean, sum, median, min, max, sd, valid
//!   n, missing n, cases, first, last), as a new dataset of groups (sorted by the break values, missing last) or added
//!   to every case. User-missing codes count as missing.
//! * **Restructure**: wide to long (sets of variables stacked into one each, with an index and a case identifier,
//!   case by case), and long to wide (one row per identifier, a variable per index value, sorted by identifier).
//!
//! Variables keep their labels, value labels, measure and missing codes.

#![allow(clippy::needless_range_loop)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::{format_code, valid_name, Column, Dataset, Measure, ValueLabel, Variable};

fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(Error::Invalid(message.into()))
}

/// A cell of any column, for keys and grouping.
#[derive(Debug, Clone, PartialEq)]
enum Cell {
    Num(f64),
    Text(String),
    Missing,
}

impl Cell {
    fn key(&self) -> String {
        match self {
            Cell::Num(v) => format!("n{}", v.to_bits()),
            Cell::Text(t) => format!("t{t}"),
            Cell::Missing => "m".into(),
        }
    }
}

fn cell(col: &Column, i: usize) -> Cell {
    match col {
        Column::Numeric(v) => v[i].map_or(Cell::Missing, Cell::Num),
        Column::Text(v) => v[i].clone().map_or(Cell::Missing, Cell::Text),
    }
}

fn cmp_cells(a: &Cell, b: &Cell) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;
    match (a, b) {
        (Cell::Missing, Cell::Missing) => Equal,
        (Cell::Missing, _) => Greater,
        (_, Cell::Missing) => Less,
        (Cell::Num(x), Cell::Num(y)) => x.total_cmp(y),
        (Cell::Text(x), Cell::Text(y)) => x.cmp(y),
        (Cell::Num(_), Cell::Text(_)) => Less,
        (Cell::Text(_), Cell::Num(_)) => Greater,
    }
}

/// Picks rows of a column (None: a missing cell).
fn take(col: &Column, rows: &[Option<usize>]) -> Column {
    match col {
        Column::Numeric(v) => Column::Numeric(rows.iter().map(|r| r.and_then(|i| v[i])).collect()),
        Column::Text(v) => Column::Text(rows.iter().map(|r| r.and_then(|i| v[i].clone())).collect()),
    }
}

/// Adds a variable with another's settings.
fn add_like(ds: &mut Dataset, name: &str, column: Column, like: &Variable) -> Result<()> {
    ds.add_variable(name, column)?;
    let v = ds.variable_mut(name)?;
    v.label = like.label.clone();
    v.measure = like.measure;
    v.value_labels = like.value_labels.clone();
    v.missing_values = like.missing_values.clone();
    v.missing_ranges = like.missing_ranges.clone();
    Ok(())
}

/// A name not yet used in the dataset: `name`, else name_2, name_3, ...
fn free_name(ds: &Dataset, name: &str) -> String {
    if ds.find(name).is_none() {
        return name.to_string();
    }
    (2..).map(|k| format!("{name}_{k}")).find(|n| ds.find(n).is_none()).expect("some name is free")
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppendReport {
    pub added_cases: usize,
    pub rows: usize,
    /// Variables only in the other file (missing for these cases).
    pub new_variables: Vec<String>,
    /// Variables only in this file (missing for the other's cases).
    pub unmatched_variables: Vec<String>,
}

/// Appends `other`'s cases. `source` names a variable recording each case's file (1 here, 2 the other).
pub fn append(ds: &mut Dataset, other: &Dataset, source: Option<&str>) -> Result<AppendReport> {
    let n0 = ds.n_rows();
    let n1 = other.n_rows();
    if let Some(s) = source {
        if !valid_name(s) || ds.find(s).is_some() || other.find(s).is_some() {
            return invalid(format!("Choose a new, valid name for the source variable (\"{s}\" is taken or not a valid name)."));
        }
    }
    // Check types first, so nothing changes on an error.
    for (j, v) in other.variables.iter().enumerate() {
        if let Some(i) = ds.find(&v.name) {
            let same = matches!((&ds.columns[i], &other.columns[j]), (Column::Numeric(_), Column::Numeric(_)) | (Column::Text(_), Column::Text(_)));
            if !same {
                return invalid(format!("{} holds numbers in one file and text in the other; make them the same type before appending.", v.name));
            }
        }
    }
    let mut unmatched = Vec::new();
    for i in 0..ds.variables.len() {
        let name = ds.variables[i].name.clone();
        match other.find(&name) {
            Some(j) => {
                match (&mut ds.columns[i], &other.columns[j]) {
                    (Column::Numeric(a), Column::Numeric(b)) => a.extend(b.iter().copied()),
                    (Column::Text(a), Column::Text(b)) => a.extend(b.iter().cloned()),
                    _ => unreachable!("types checked"),
                }
                let ov = &other.variables[j];
                let v = &mut ds.variables[i];
                for l in &ov.value_labels {
                    if v.value_label(l.value).is_none() {
                        v.value_labels.push(l.clone());
                    }
                }
                for m in &ov.missing_values {
                    if !v.missing_values.contains(m) {
                        v.missing_values.push(*m);
                    }
                }
                if v.label.trim().is_empty() {
                    v.label = ov.label.clone();
                }
            }
            None => {
                unmatched.push(name);
                match &mut ds.columns[i] {
                    Column::Numeric(a) => a.extend(std::iter::repeat_n(None, n1)),
                    Column::Text(a) => a.extend(std::iter::repeat_n(None, n1)),
                }
            }
        }
    }
    let mut new_vars = Vec::new();
    for (j, v) in other.variables.iter().enumerate() {
        if ds.find(&v.name).is_none() {
            let rows: Vec<Option<usize>> = (0..n0).map(|_| None).chain((0..n1).map(Some)).collect();
            add_like(ds, &v.name, take(&other.columns[j], &rows), v)?;
            new_vars.push(v.name.clone());
        }
    }
    if let Some(s) = source {
        ds.add_variable(s, Column::Numeric((0..n0 + n1).map(|i| Some(if i < n0 { 1.0 } else { 2.0 })).collect()))?;
        let v = ds.variable_mut(s)?;
        v.label = "Source file".into();
        v.measure = Measure::Nominal;
        let first = ds.name.clone();
        let v = ds.variable_mut(s)?;
        v.value_labels = vec![ValueLabel { value: 1.0, label: first }, ValueLabel { value: 2.0, label: other.name.clone() }];
    }
    Ok(AppendReport { added_cases: n1, rows: n0 + n1, new_variables: new_vars, unmatched_variables: unmatched })
}

/// Which cases a merge keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Join {
    /// Every case of this file (missing values where the other file has no match).
    #[default]
    Left,
    /// Only cases with a match.
    Inner,
    /// Every case of both files.
    All,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MergeReport {
    pub rows: usize,
    pub matched: usize,
    /// Cases of this file without a match, and of the other file without a match.
    pub unmatched_here: usize,
    pub unmatched_other: usize,
    pub added_variables: Vec<String>,
    /// (other file's name, its name here) for variables renamed because the name was taken.
    pub renamed: Vec<(String, String)>,
}

/// Joins `other`'s variables on the keys.
pub fn merge(ds: &mut Dataset, other: &Dataset, keys: &[String], join: Join) -> Result<MergeReport> {
    if keys.is_empty() {
        return invalid("Choose the key variable(s) that identify matching cases in both files.");
    }
    let mut ki = Vec::new();
    for k in keys {
        let (Some(a), Some(b)) = (ds.find(k), other.find(k)) else {
            return invalid(format!("The key {k} must be in both files."));
        };
        let same = matches!((&ds.columns[a], &other.columns[b]), (Column::Numeric(_), Column::Numeric(_)) | (Column::Text(_), Column::Text(_)));
        if !same {
            return invalid(format!("The key {k} holds numbers in one file and text in the other."));
        }
        ki.push((a, b));
    }
    let key_of = |d: &Dataset, side: usize, i: usize| -> Option<String> {
        let mut s = String::new();
        for &(a, b) in &ki {
            let c = cell(&d.columns[if side == 0 { a } else { b }], i);
            if c == Cell::Missing {
                return None;
            }
            s.push_str(&c.key());
            s.push('\u{1f}');
        }
        Some(s)
    };
    let (n0, n1) = (ds.n_rows(), other.n_rows());
    let mut other_rows: HashMap<String, Vec<usize>> = HashMap::new();
    for i in 0..n1 {
        if let Some(k) = key_of(other, 1, i) {
            other_rows.entry(k).or_default().push(i);
        }
    }
    let mut here_count: HashMap<String, usize> = HashMap::new();
    for i in 0..n0 {
        if let Some(k) = key_of(ds, 0, i) {
            *here_count.entry(k).or_default() += 1;
        }
    }
    for (k, rows) in &other_rows {
        if rows.len() > 1 && here_count.get(k).copied().unwrap_or(0) > 1 {
            return invalid("Some key values repeat in both files, so cases can't be matched one to one (a many-to-many merge). Make the keys unique in at least one file.");
        }
    }
    // Row plan: (row here, row there).
    let mut plan: Vec<(Option<usize>, Option<usize>)> = Vec::new();
    let mut used_other = vec![false; n1];
    let (mut matched, mut unmatched_here) = (0, 0);
    for i in 0..n0 {
        match key_of(ds, 0, i).and_then(|k| other_rows.get(&k)) {
            Some(rows) => {
                matched += 1;
                for &j in rows {
                    plan.push((Some(i), Some(j)));
                    used_other[j] = true;
                }
            }
            None => {
                unmatched_here += 1;
                if join != Join::Inner {
                    plan.push((Some(i), None));
                }
            }
        }
    }
    let unmatched_other = used_other.iter().filter(|u| !**u).count();
    if join == Join::All {
        for j in 0..n1 {
            if !used_other[j] {
                plan.push((None, Some(j)));
            }
        }
    }
    let here: Vec<Option<usize>> = plan.iter().map(|p| p.0).collect();
    let there: Vec<Option<usize>> = plan.iter().map(|p| p.1).collect();
    let mut out = Dataset::new(ds.name.clone());
    for (i, v) in ds.variables.iter().enumerate() {
        let mut col = take(&ds.columns[i], &here);
        // The other file's unmatched cases take their key values from it.
        if let Some(&(_, b)) = ki.iter().find(|(a, _)| *a == i) {
            let theirs = take(&other.columns[b], &there);
            col = match (col, theirs) {
                (Column::Numeric(mut x), Column::Numeric(y)) => {
                    for r in 0..x.len() {
                        if here[r].is_none() {
                            x[r] = y[r];
                        }
                    }
                    Column::Numeric(x)
                }
                (Column::Text(mut x), Column::Text(y)) => {
                    for r in 0..x.len() {
                        if here[r].is_none() {
                            x[r] = y[r].clone();
                        }
                    }
                    Column::Text(x)
                }
                (c, _) => c,
            };
        }
        add_like(&mut out, &v.name, col, v)?;
    }
    let mut added = Vec::new();
    let mut renamed = Vec::new();
    for (j, v) in other.variables.iter().enumerate() {
        if ki.iter().any(|(_, b)| *b == j) {
            continue;
        }
        let name = free_name(&out, &v.name);
        if name != v.name {
            renamed.push((v.name.clone(), name.clone()));
        }
        add_like(&mut out, &name, take(&other.columns[j], &there), v)?;
        added.push(name);
    }
    let rows = out.n_rows();
    // Keep the dataset's own name and ID counter moving forward.
    *ds = out;
    Ok(MergeReport { rows, matched, unmatched_here, unmatched_other, added_variables: added, renamed })
}

/// A summary function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fun {
    Mean,
    Sum,
    Median,
    Min,
    Max,
    Sd,
    /// Valid (non-missing) values.
    N,
    /// Missing values.
    Nmiss,
    /// Cases in the group (needs no source variable).
    Cases,
    First,
    Last,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    /// The variable summarized (not used by "cases").
    #[serde(default)]
    pub source: Option<String>,
    pub function: Fun,
    pub target: String,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AggregateSpec {
    pub breaks: Vec<String>,
    pub summaries: Vec<Summary>,
    /// Add the summaries to every case instead of making a dataset of groups.
    #[serde(default)]
    pub add_to_cases: bool,
    /// Treat a missing break value as a group of its own (true) or leave those cases out.
    #[serde(default = "yes")]
    pub missing_groups: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AggregateReport {
    pub groups: usize,
    pub rows: usize,
    pub names: Vec<String>,
}

fn summarize(f: Fun, vals: &[Option<f64>]) -> Option<f64> {
    let v: Vec<f64> = vals.iter().flatten().copied().collect();
    let n = v.len() as f64;
    match f {
        Fun::Cases => Some(vals.len() as f64),
        Fun::N => Some(n),
        Fun::Nmiss => Some(vals.len() as f64 - n),
        _ if v.is_empty() => None,
        Fun::Mean => Some(v.iter().sum::<f64>() / n),
        Fun::Sum => Some(v.iter().sum()),
        Fun::Min => v.iter().copied().reduce(f64::min),
        Fun::Max => v.iter().copied().reduce(f64::max),
        Fun::First => v.first().copied(),
        Fun::Last => v.last().copied(),
        Fun::Median => {
            let mut s = v.clone();
            s.sort_by(f64::total_cmp);
            let m = s.len();
            Some(if m % 2 == 1 { s[m / 2] } else { (s[m / 2 - 1] + s[m / 2]) / 2.0 })
        }
        Fun::Sd => {
            if v.len() < 2 {
                return None;
            }
            let m = v.iter().sum::<f64>() / n;
            Some((v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0)).sqrt())
        }
    }
}

/// Aggregates; replaces `ds` with the groups, or adds the summaries to its cases.
pub fn aggregate(ds: &mut Dataset, spec: &AggregateSpec) -> Result<AggregateReport> {
    if spec.summaries.is_empty() {
        return invalid("Add at least one summary.");
    }
    let bi: Vec<usize> = spec.breaks.iter().map(|b| ds.find(b).ok_or_else(|| Error::UnknownVariable(b.clone()))).collect::<Result<_>>()?;
    let n = ds.n_rows();
    // Each summary's values, with user-missing codes as missing.
    let mut sources: Vec<Vec<Option<f64>>> = Vec::new();
    for s in &spec.summaries {
        if !valid_name(&s.target) {
            return Err(Error::InvalidName(s.target.clone()));
        }
        if s.function == Fun::Cases {
            sources.push(vec![Some(0.0); n]);
            continue;
        }
        let Some(src) = &s.source else {
            return invalid(format!("Choose the variable to summarize for {}.", s.target));
        };
        let i = ds.find(src).ok_or_else(|| Error::UnknownVariable(src.clone()))?;
        let var = &ds.variables[i];
        match &ds.columns[i] {
            Column::Numeric(v) => sources.push(v.iter().map(|x| x.filter(|y| !var.is_user_missing(*y))).collect()),
            Column::Text(_) => return invalid(format!("{src} holds text; it can only be counted (use \"cases\").")),
        }
    }
    let mut targets: Vec<&str> = spec.summaries.iter().map(|s| s.target.as_str()).collect();
    targets.sort_unstable_by_key(|t| t.to_ascii_lowercase());
    if targets.windows(2).any(|w| w[0].eq_ignore_ascii_case(w[1])) {
        return invalid("Each summary needs its own name.");
    }
    // Groups.
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut group_key: Vec<Vec<Cell>> = Vec::new();
    let mut group_of: Vec<Option<usize>> = vec![None; n];
    for r in 0..n {
        let cells: Vec<Cell> = bi.iter().map(|&i| {
            let c = cell(&ds.columns[i], r);
            // A user-missing code in a break counts as missing.
            match c {
                Cell::Num(x) if ds.variables[i].is_user_missing(x) => Cell::Missing,
                c => c,
            }
        }).collect();
        if !spec.missing_groups && cells.contains(&Cell::Missing) {
            continue;
        }
        let k: String = cells.iter().map(Cell::key).collect::<Vec<_>>().join("\u{1f}");
        let g = *index.entry(k).or_insert_with(|| {
            group_key.push(cells.clone());
            group_key.len() - 1
        });
        group_of[r] = Some(g);
    }
    let ng = group_key.len();
    let mut order: Vec<usize> = (0..ng).collect();
    order.sort_by(|&a, &b| group_key[a].iter().zip(&group_key[b]).map(|(x, y)| cmp_cells(x, y)).find(|o| *o != std::cmp::Ordering::Equal).unwrap_or(std::cmp::Ordering::Equal));
    let mut values: Vec<Vec<Option<f64>>> = vec![vec![None; ng]; spec.summaries.len()];
    for (s, sum) in spec.summaries.iter().enumerate() {
        let mut members: Vec<Vec<Option<f64>>> = vec![Vec::new(); ng];
        for r in 0..n {
            if let Some(g) = group_of[r] {
                members[g].push(sources[s][r]);
            }
        }
        for g in 0..ng {
            values[s][g] = summarize(sum.function, &members[g]);
        }
    }
    let label_for = |s: &Summary| -> String {
        s.label.clone().filter(|l| !l.trim().is_empty()).unwrap_or_else(|| {
            let f = format!("{:?}", s.function).to_lowercase();
            match &s.source {
                Some(src) if s.function != Fun::Cases => format!("{f} of {src}"),
                _ => "cases".into(),
            }
        })
    };
    if spec.add_to_cases {
        for s in &spec.summaries {
            if ds.find(&s.target).is_some() {
                return invalid(format!("There is already a variable named {}.", s.target));
            }
        }
        for (s, sum) in spec.summaries.iter().enumerate() {
            ds.add_variable(&sum.target, Column::Numeric((0..n).map(|r| group_of[r].and_then(|g| values[s][g])).collect()))?;
            ds.variable_mut(&sum.target)?.label = label_for(sum);
        }
        return Ok(AggregateReport { groups: ng, rows: n, names: spec.summaries.iter().map(|s| s.target.clone()).collect() });
    }
    // A dataset of groups: one representative case per group for the break variables.
    let mut first_row = vec![usize::MAX; ng];
    for r in (0..n).rev() {
        if let Some(g) = group_of[r] {
            first_row[g] = r;
        }
    }
    let rows: Vec<Option<usize>> = order.iter().map(|&g| Some(first_row[g])).collect();
    let mut out = Dataset::new(ds.name.clone());
    for &i in &bi {
        let mut col = take(&ds.columns[i], &rows);
        // User-missing codes in a break show as one missing group.
        if let Column::Numeric(v) = &mut col {
            for x in v.iter_mut() {
                if x.is_some_and(|y| ds.variables[i].is_user_missing(y)) {
                    *x = None;
                }
            }
        }
        add_like(&mut out, &ds.variables[i].name, col, &ds.variables[i])?;
    }
    for (s, sum) in spec.summaries.iter().enumerate() {
        if out.find(&sum.target).is_some() {
            return invalid(format!("A summary is named like a break variable ({}).", sum.target));
        }
        out.add_variable(&sum.target, Column::Numeric(order.iter().map(|&g| values[s][g]).collect()))?;
        out.variable_mut(&sum.target)?.label = label_for(sum);
    }
    *ds = out;
    Ok(AggregateReport { groups: ng, rows: ng, names: spec.summaries.iter().map(|s| s.target.clone()).collect() })
}

/// A set of variables stacked into one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stack {
    pub target: String,
    pub variables: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LongSpec {
    pub stacks: Vec<Stack>,
    /// The index variable's name (1, 2, ... for the positions in each stack).
    #[serde(default = "index")]
    pub index: String,
    /// The case identifier: an existing variable, or a new one numbered 1, 2, ... (default "id").
    #[serde(default)]
    pub id: Option<String>,
    /// Leave out a long row whose stacked values are all missing.
    #[serde(default)]
    pub drop_missing: bool,
}

fn index() -> String {
    "index".into()
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReshapeReport {
    pub rows: usize,
    pub variables: usize,
    /// Variables left out (long to wide: not listed and not constant).
    pub dropped: Vec<String>,
}

/// Wide to long: each case becomes one row per stack position.
pub fn to_long(ds: &mut Dataset, spec: &LongSpec) -> Result<ReshapeReport> {
    let k = spec.stacks.first().map_or(0, |s| s.variables.len());
    if k < 2 || spec.stacks.iter().any(|s| s.variables.len() != k) {
        return invalid("Each stack needs the same number of variables (at least two).");
    }
    let mut stacked: Vec<usize> = Vec::new();
    for s in &spec.stacks {
        if !valid_name(&s.target) {
            return Err(Error::InvalidName(s.target.clone()));
        }
        let idx: Vec<usize> = s.variables.iter().map(|v| ds.find(v).ok_or_else(|| Error::UnknownVariable(v.clone()))).collect::<Result<_>>()?;
        let numeric = matches!(ds.columns[idx[0]], Column::Numeric(_));
        if idx.iter().any(|&i| matches!(ds.columns[i], Column::Numeric(_)) != numeric) {
            return invalid(format!("The variables stacked into {} must all be numbers or all text.", s.target));
        }
        for i in idx {
            if stacked.contains(&i) {
                return invalid(format!("{} is in two stacks.", ds.variables[i].name));
            }
            stacked.push(i);
        }
    }
    let n = ds.n_rows();
    let id_existing = spec.id.as_ref().and_then(|id| ds.find(id));
    let fixed: Vec<usize> = (0..ds.variables.len()).filter(|i| !stacked.contains(i)).collect();
    // Rows: (case, position).
    let mut plan: Vec<(usize, usize)> = Vec::new();
    for r in 0..n {
        for p in 0..k {
            if spec.drop_missing {
                let all_missing = spec.stacks.iter().all(|s| cell(&ds.columns[ds.find(&s.variables[p]).expect("checked")], r) == Cell::Missing);
                if all_missing {
                    continue;
                }
            }
            plan.push((r, p));
        }
    }
    let mut out = Dataset::new(ds.name.clone());
    if id_existing.is_none() {
        let name = spec.id.clone().unwrap_or_else(|| "id".into());
        if !valid_name(&name) || ds.find(&name).is_some() && spec.id.is_none() {
            return invalid(format!("Name the case identifier: {name} is taken or not valid."));
        }
        out.add_variable(&name, Column::Numeric(plan.iter().map(|(r, _)| Some((*r + 1) as f64)).collect()))?;
        out.variable_mut(&name)?.label = "Case".into();
        out.variable_mut(&name)?.measure = Measure::Nominal;
    }
    let rows: Vec<Option<usize>> = plan.iter().map(|(r, _)| Some(*r)).collect();
    for &i in &fixed {
        add_like(&mut out, &ds.variables[i].name, take(&ds.columns[i], &rows), &ds.variables[i])?;
    }
    if out.find(&spec.index).is_some() || !valid_name(&spec.index) {
        return invalid(format!("Name the index variable: {} is taken or not valid.", spec.index));
    }
    out.add_variable(&spec.index, Column::Numeric(plan.iter().map(|(_, p)| Some((*p + 1) as f64)).collect()))?;
    out.variable_mut(&spec.index)?.measure = Measure::Ordinal;
    for s in &spec.stacks {
        if out.find(&s.target).is_some() {
            return invalid(format!("{} is already a variable name.", s.target));
        }
        let idx: Vec<usize> = s.variables.iter().map(|v| ds.find(v).expect("checked")).collect();
        let col = match &ds.columns[idx[0]] {
            Column::Numeric(_) => Column::Numeric(plan.iter().map(|(r, p)| if let Column::Numeric(v) = &ds.columns[idx[*p]] { v[*r] } else { None }).collect()),
            Column::Text(_) => Column::Text(plan.iter().map(|(r, p)| if let Column::Text(v) = &ds.columns[idx[*p]] { v[*r].clone() } else { None }).collect()),
        };
        let first = ds.variables[idx[0]].clone();
        add_like(&mut out, &s.target, col, &first)?;
        // The label of the first variable usually names one occasion; keep it only if all share it.
        if idx.iter().any(|&i| ds.variables[i].label != first.label) {
            out.variable_mut(&s.target)?.label = String::new();
        }
        let v = out.variable_mut(&s.target)?;
        for &i in &idx[1..] {
            for m in &ds.variables[i].missing_values {
                if !v.missing_values.contains(m) {
                    v.missing_values.push(*m);
                }
            }
        }
    }
    let rows_n = out.n_rows();
    let vars = out.variables.len();
    *ds = out;
    Ok(ReshapeReport { rows: rows_n, variables: vars, dropped: Vec::new() })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WideSpec {
    /// The identifier variable(s): one row per combination.
    pub id: Vec<String>,
    /// The variable whose values become the new variables' suffixes.
    pub index: String,
    /// Variables spread into one per index value (all others besides id and index when empty).
    #[serde(default)]
    pub variables: Vec<String>,
}

/// Long to wide: one row per identifier, with name_<index value> variables.
pub fn to_wide(ds: &mut Dataset, spec: &WideSpec) -> Result<ReshapeReport> {
    if spec.id.is_empty() {
        return invalid("Choose the identifier variable(s).");
    }
    let ii: Vec<usize> = spec.id.iter().map(|v| ds.find(v).ok_or_else(|| Error::UnknownVariable(v.clone()))).collect::<Result<_>>()?;
    let xi = ds.find(&spec.index).ok_or_else(|| Error::UnknownVariable(spec.index.clone()))?;
    let spread: Vec<usize> = if spec.variables.is_empty() {
        (0..ds.variables.len()).filter(|i| !ii.contains(i) && *i != xi).collect()
    } else {
        spec.variables.iter().map(|v| ds.find(v).ok_or_else(|| Error::UnknownVariable(v.clone()))).collect::<Result<_>>()?
    };
    let n = ds.n_rows();
    // Index values, sorted.
    let mut levels: Vec<Cell> = Vec::new();
    for r in 0..n {
        let c = cell(&ds.columns[xi], r);
        if c == Cell::Missing {
            return invalid(format!("{} is missing for case {}; every case needs an index value.", spec.index, r + 1));
        }
        if !levels.contains(&c) {
            levels.push(c);
        }
    }
    levels.sort_by(cmp_cells);
    // Identifiers, sorted.
    let mut ids: HashMap<String, usize> = HashMap::new();
    let mut id_cells: Vec<Vec<Cell>> = Vec::new();
    let mut first_row: Vec<usize> = Vec::new();
    let mut slot: HashMap<(usize, usize), usize> = HashMap::new();
    for r in 0..n {
        let cells: Vec<Cell> = ii.iter().map(|&i| cell(&ds.columns[i], r)).collect();
        let key: String = cells.iter().map(Cell::key).collect::<Vec<_>>().join("\u{1f}");
        let g = *ids.entry(key).or_insert_with(|| {
            id_cells.push(cells.clone());
            first_row.push(r);
            id_cells.len() - 1
        });
        let l = levels.iter().position(|c| *c == cell(&ds.columns[xi], r)).expect("listed");
        if slot.insert((g, l), r).is_some() {
            return invalid(format!("An identifier has two cases with the same {} value; each identifier needs one case per index value.", spec.index));
        }
    }
    let mut order: Vec<usize> = (0..id_cells.len()).collect();
    order.sort_by(|&a, &b| id_cells[a].iter().zip(&id_cells[b]).map(|(x, y)| cmp_cells(x, y)).find(|o| *o != std::cmp::Ordering::Equal).unwrap_or(std::cmp::Ordering::Equal));
    let index_var = ds.variables[xi].clone();
    let suffix = |c: &Cell| -> String {
        let raw = match c {
            Cell::Num(v) => format_code(*v),
            Cell::Text(t) => t.clone(),
            Cell::Missing => "NA".into(),
        };
        raw.chars().map(|ch| if ch.is_alphanumeric() { ch } else { '_' }).collect()
    };
    let mut out = Dataset::new(ds.name.clone());
    let rows: Vec<Option<usize>> = order.iter().map(|&g| Some(first_row[g])).collect();
    for &i in &ii {
        add_like(&mut out, &ds.variables[i].name, take(&ds.columns[i], &rows), &ds.variables[i])?;
    }
    for &v in &spread {
        for (l, lev) in levels.iter().enumerate() {
            let name = format!("{}_{}", ds.variables[v].name, suffix(lev));
            if !valid_name(&name) || out.find(&name).is_some() {
                return invalid(format!("The new variable name {name} is not valid or is taken."));
            }
            let picks: Vec<Option<usize>> = order.iter().map(|&g| slot.get(&(g, l)).copied()).collect();
            add_like(&mut out, &name, take(&ds.columns[v], &picks), &ds.variables[v])?;
            let lab = match lev {
                Cell::Num(x) => index_var.value_text(*x),
                Cell::Text(t) => t.clone(),
                Cell::Missing => String::new(),
            };
            let base = ds.variables[v].display_name().to_string();
            out.variable_mut(&name)?.label = format!("{base} ({} {lab})", index_var.display_name());
        }
    }
    let dropped: Vec<String> = (0..ds.variables.len()).filter(|i| !ii.contains(i) && *i != xi && !spread.contains(i)).map(|i| ds.variables[i].name.clone()).collect();
    let rows_n = out.n_rows();
    let vars = out.variables.len();
    *ds = out;
    Ok(ReshapeReport { rows: rows_n, variables: vars, dropped })
}
