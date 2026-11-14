//! Data › Sort Cases and the Transform commands that work variable by variable: Rank Cases, Standardize (z-scores),
//! Reverse Score, and Date and Time (the parts of a date, and the time between two dates). Each is recorded by the API
//! like any other change, and each matches what the exported R script does (checked against R in
//! tests/transform_reference.rs).
//!
//! As everywhere, a user-missing code counts as missing: it sorts last, gets no rank or z-score, and is kept as it is by
//! Reverse Score.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::{Column, Dataset, Measure, NumericCell, ValueLabel};

fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(Error::Invalid(message.into()))
}

/// What a change made, for the message after it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    pub names: Vec<String>,
    /// Cases given a value (in the first variable made), and cases left missing.
    pub valid: usize,
    pub missing: usize,
    /// Text that couldn't be read as a date, with an example.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unreadable: Option<(usize, String)>,
}

fn valid_values(ds: &Dataset, name: &str) -> Result<Vec<Option<f64>>> {
    Ok(ds.numeric_cells(name)?.into_iter().map(|c| if let NumericCell::Value(v) = c { Some(v) } else { None }).collect())
}

/// Adds a new numeric variable (or, with `replace`, overwrites an existing one) with a label and measure.
fn put(ds: &mut Dataset, name: &str, values: Vec<Option<f64>>, label: String, measure: Measure, replace: bool) -> Result<()> {
    match ds.find(name) {
        Some(i) if replace => ds.columns[i] = Column::Numeric(values),
        Some(_) => return invalid(format!("There is already a variable named {name}. Choose another name.")),
        None => {
            ds.add_variable(name, Column::Numeric(values))?;
        }
    }
    let v = ds.variable_mut(name)?;
    v.label = label;
    v.measure = measure;
    Ok(())
}

/// Every new name is free, so a change never stops halfway through with some variables made.
fn free(ds: &Dataset, names: &[String]) -> Result<()> {
    if let Some(n) = names.iter().find(|n| ds.find(n).is_some()) {
        return invalid(format!("There is already a variable named {n}. Choose another name."));
    }
    let mut seen = std::collections::HashSet::new();
    if let Some(n) = names.iter().find(|n| !seen.insert(n.to_lowercase())) {
        return invalid(format!("The name {n} is given twice."));
    }
    Ok(())
}

fn counts(values: &[Option<f64>]) -> (usize, usize) {
    let valid = values.iter().filter(|v| v.is_some()).count();
    (valid, values.len() - valid)
}

/// The names of new variables: those given, or each source name with a prefix or suffix.
fn targets(sources: &[String], given: &[String], make: impl Fn(&str) -> String) -> Result<Vec<String>> {
    if given.is_empty() {
        return Ok(sources.iter().map(|s| make(s)).collect());
    }
    if given.len() != sources.len() {
        return invalid(format!("Give one new name for each of the {} variables, or leave the names empty.", sources.len()));
    }
    Ok(given.to_vec())
}

// ---------- Sort Cases ----------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SortKey {
    pub name: String,
    #[serde(default)]
    pub descending: bool,
}

/// Puts the cases in order of the keys, the first key first. Ties keep their order; missing values (empty or
/// user-missing) go last whichever way a key is sorted. Text sorts by character code, as R's `order(method = "radix")`.
pub fn sort_cases(ds: &mut Dataset, keys: &[SortKey]) -> Result<usize> {
    if keys.is_empty() {
        return invalid("Choose at least one variable to sort by.");
    }
    enum Key {
        Num(Vec<Option<f64>>),
        Text(Vec<Option<String>>),
    }
    let mut cols = Vec::new();
    for k in keys {
        let i = ds.find(&k.name).ok_or_else(|| Error::Invalid(format!("There is no variable named {}.", k.name)))?;
        cols.push((
            match &ds.columns[i] {
                Column::Numeric(_) => Key::Num(valid_values(ds, &k.name)?),
                Column::Text(t) => Key::Text(t.clone()),
            },
            k.descending,
        ));
    }
    let n = ds.n_rows();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        for (col, desc) in &cols {
            let o = match col {
                Key::Num(v) => match (v[a], v[b]) {
                    (Some(x), Some(y)) => {
                        let o = x.total_cmp(&y);
                        if *desc { o.reverse() } else { o }
                    }
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                },
                Key::Text(v) => match (&v[a], &v[b]) {
                    (Some(x), Some(y)) => {
                        let o = x.as_bytes().cmp(y.as_bytes());
                        if *desc { o.reverse() } else { o }
                    }
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                },
            };
            if o.is_ne() {
                return o;
            }
        }
        std::cmp::Ordering::Equal
    });
    for c in &mut ds.columns {
        match c {
            Column::Numeric(v) => *v = order.iter().map(|&i| v[i]).collect(),
            Column::Text(v) => *v = order.iter().map(|&i| v[i].clone()).collect(),
        }
    }
    Ok(n)
}

// ---------- Rank Cases ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ties {
    /// The mean of the tied ranks (R's "average"), the usual choice.
    #[default]
    Mean,
    Low,
    High,
    /// In the order of the cases (R's "first").
    Sequential,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankSpec {
    pub variables: Vec<String>,
    #[serde(default)]
    pub ties: Ties,
    /// Rank 1 for the largest value.
    #[serde(default)]
    pub descending: bool,
    /// Rank within each group of this variable.
    #[serde(default)]
    pub by: Option<String>,
    /// New names, one per variable; `r_<name>` when left out.
    #[serde(default)]
    pub into: Vec<String>,
}

fn rank_values(x: &[(usize, f64)], ties: Ties, descending: bool, out: &mut [Option<f64>]) {
    let mut idx: Vec<usize> = (0..x.len()).collect();
    // Stable: equal values keep case order, which "sequential" ranking needs.
    idx.sort_by(|&a, &b| {
        let o = x[a].1.total_cmp(&x[b].1);
        if descending { o.reverse() } else { o }
    });
    let mut i = 0;
    while i < idx.len() {
        let mut j = i;
        while j + 1 < idx.len() && x[idx[j + 1]].1 == x[idx[i]].1 {
            j += 1;
        }
        for (k, &p) in idx[i..=j].iter().enumerate() {
            let r = match ties {
                Ties::Mean => (i + j) as f64 / 2.0 + 1.0,
                Ties::Low => i as f64 + 1.0,
                Ties::High => j as f64 + 1.0,
                Ties::Sequential => (i + k) as f64 + 1.0,
            };
            out[x[p].0] = Some(r);
        }
        i = j + 1;
    }
}

pub fn rank_cases(ds: &mut Dataset, spec: &RankSpec) -> Result<Report> {
    if spec.variables.is_empty() {
        return invalid("Choose at least one variable to rank.");
    }
    let names = targets(&spec.variables, &spec.into, |s| format!("r_{s}"))?;
    free(ds, &names)?;
    let n = ds.n_rows();
    let groups: Vec<Option<String>> = match &spec.by {
        None => vec![Some(String::new()); n],
        Some(g) => {
            let i = ds.find(g).ok_or_else(|| Error::Invalid(format!("There is no variable named {g}.")))?;
            match &ds.columns[i] {
                Column::Numeric(_) => valid_values(ds, g)?.into_iter().map(|v| v.map(|x| format!("{x}"))).collect(),
                Column::Text(t) => t.clone(),
            }
        }
    };
    let mut made = Vec::new();
    for (src, target) in spec.variables.iter().zip(&names) {
        let x = valid_values(ds, src)?;
        let mut out = vec![None; n];
        let mut keys: Vec<&String> = groups.iter().flatten().collect();
        keys.sort();
        keys.dedup();
        for key in keys {
            let members: Vec<(usize, f64)> = (0..n).filter(|&r| groups[r].as_ref() == Some(key)).filter_map(|r| x[r].map(|v| (r, v))).collect();
            rank_values(&members, spec.ties, spec.descending, &mut out);
        }
        let label = format!("Rank of {}{}", ds.variable(src)?.display_name(), spec.by.as_ref().map_or(String::new(), |g| format!(" within {g}")));
        made.push((target.clone(), out, label));
    }
    let first = made[0].1.clone();
    for (name, values, label) in made {
        put(ds, &name, values, label, Measure::Ordinal, false)?;
    }
    let (valid, missing) = counts(&first);
    Ok(Report { names, valid, missing, unreadable: None })
}

// ---------- Standardize ----------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StandardizeSpec {
    pub variables: Vec<String>,
    /// New names; `z_<name>` when left out.
    #[serde(default)]
    pub into: Vec<String>,
}

/// z = (x − mean) / SD, with the sample SD (n − 1), as R's `scale()`.
pub fn standardize(ds: &mut Dataset, spec: &StandardizeSpec) -> Result<Report> {
    if spec.variables.is_empty() {
        return invalid("Choose at least one variable to standardize.");
    }
    let names = targets(&spec.variables, &spec.into, |s| format!("z_{s}"))?;
    free(ds, &names)?;
    let mut made = Vec::new();
    for (src, target) in spec.variables.iter().zip(&names) {
        let x = valid_values(ds, src)?;
        let v: Vec<f64> = x.iter().flatten().copied().collect();
        if v.len() < 2 {
            return invalid(format!("{src} needs at least two valid values to standardize."));
        }
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        let sd = (v.iter().map(|a| (a - mean).powi(2)).sum::<f64>() / (v.len() - 1) as f64).sqrt();
        if sd == 0.0 {
            return invalid(format!("Every valid value of {src} is the same, so it has no spread to standardize by."));
        }
        let z: Vec<Option<f64>> = x.iter().map(|a| a.map(|a| (a - mean) / sd)).collect();
        made.push((target.clone(), z, format!("z-score of {}", ds.variable(src)?.display_name())));
    }
    let first = made[0].1.clone();
    for (name, values, label) in made {
        put(ds, &name, values, label, Measure::Scale, false)?;
    }
    let (valid, missing) = counts(&first);
    Ok(Report { names, valid, missing, unreadable: None })
}

// ---------- Reverse Score ----------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReverseSpec {
    pub variables: Vec<String>,
    /// The lowest and highest possible answers, such as 1 and 5.
    pub min: f64,
    pub max: f64,
    /// New names; `<name>_r` when left out. Ignored with `replace`.
    #[serde(default)]
    pub into: Vec<String>,
    /// Reverse the variables in place instead of making new ones.
    #[serde(default)]
    pub replace: bool,
}

/// Each valid value v becomes min + max − v, and value labels move with their values. Empty cells and user-missing codes
/// are kept as they are, with the missing-value settings copied.
pub fn reverse_score(ds: &mut Dataset, spec: &ReverseSpec) -> Result<Report> {
    if spec.variables.is_empty() {
        return invalid("Choose at least one variable to reverse.");
    }
    if spec.min.partial_cmp(&spec.max) != Some(std::cmp::Ordering::Less) {
        return invalid("The lowest possible answer must be below the highest.");
    }
    let names = if spec.replace { spec.variables.clone() } else { targets(&spec.variables, &spec.into, |s| format!("{s}_r"))? };
    if !spec.replace {
        free(ds, &names)?;
    }
    // Check every variable before changing any.
    for src in &spec.variables {
        let i = ds.find(src).ok_or_else(|| Error::Invalid(format!("There is no variable named {src}.")))?;
        let Column::Numeric(raw) = &ds.columns[i] else {
            return invalid(format!("{src} holds text; only numbers can be reverse-scored."));
        };
        let var = &ds.variables[i];
        if let Some(out) = raw.iter().flatten().find(|v| !var.is_user_missing(**v) && (**v < spec.min || **v > spec.max)) {
            return invalid(format!("{src} has the value {out}, outside {} to {}. Check the lowest and highest possible answers, or set that value as missing first.", spec.min, spec.max));
        }
    }
    let sum = spec.min + spec.max;
    let mut first = None;
    for (src, target) in spec.variables.iter().zip(&names) {
        let i = ds.find(src).ok_or_else(|| Error::Invalid(format!("There is no variable named {src}.")))?;
        let Column::Numeric(raw) = &ds.columns[i] else {
            return invalid(format!("{src} holds text; only numbers can be reverse-scored."));
        };
        let var = ds.variables[i].clone();
        if let Some(out) = raw.iter().flatten().find(|v| !var.is_user_missing(**v) && (**v < spec.min || **v > spec.max)) {
            return invalid(format!("{src} has the value {out}, outside {} to {}. Check the lowest and highest possible answers, or set that value as missing first.", spec.min, spec.max));
        }
        let values: Vec<Option<f64>> = raw.iter().map(|c| c.map(|v| if var.is_user_missing(v) { v } else { sum - v })).collect();
        let labels: Vec<ValueLabel> = var.value_labels.iter().map(|l| ValueLabel { value: if var.is_user_missing(l.value) { l.value } else { sum - l.value }, label: l.label.clone() }).collect();
        if first.is_none() {
            first = Some(counts(&values.iter().zip(raw).map(|(v, r)| v.filter(|_| r.is_some_and(|x| !var.is_user_missing(x)))).collect::<Vec<_>>()));
        }
        let label = if spec.replace { var.label.clone() } else { format!("{} (reversed)", var.display_name()) };
        put(ds, target, values, label, var.measure, spec.replace)?;
        let v = ds.variable_mut(target)?;
        let mut labels = labels;
        labels.sort_by(|a, b| a.value.total_cmp(&b.value));
        v.value_labels = labels;
        v.missing_values = var.missing_values.clone();
        v.missing_ranges = var.missing_ranges.clone();
    }
    let (valid, missing) = first.unwrap_or((0, 0));
    Ok(Report { names, valid, missing, unreadable: None })
}

// ---------- Date and Time ----------

/// How the dates are stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateSource {
    /// Text with the year first: 2026-11-14, 2026/11/14 13:05.
    TextYmd,
    /// Text with the month first: 11/14/2026.
    TextMdy,
    /// Text with the day first: 14/11/2026, 14.11.2026.
    TextDmy,
    /// Spreadsheet date numbers: days since 1899-12-30, a fraction for the time.
    Spreadsheet,
    /// Seconds since 1582-10-14, as .sav files store dates.
    Seconds1582,
    /// Days since 1960-01-01, as .dta and .sas7bdat files store dates.
    Days1960,
    /// Seconds since 1960-01-01, as .dta and .sas7bdat files store date-times.
    Seconds1960,
}

/// Days from 1970-01-01 to a civil date (proleptic Gregorian).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// The civil date of a day count from 1970-01-01.
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Days (with a fraction for the time) since 1970-01-01, from text in the given order.
fn parse_text(s: &str, src: DateSource) -> Option<f64> {
    let parts: Vec<&str> = s.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).collect();
    if parts.len() < 3 {
        return None;
    }
    let n: Vec<i64> = parts.iter().map(|p| p.parse().ok()).collect::<Option<_>>()?;
    let (y, m, d) = match src {
        DateSource::TextYmd => (n[0], n[1], n[2]),
        DateSource::TextMdy => (n[2], n[0], n[1]),
        _ => (n[2], n[1], n[0]),
    };
    let year_digits = parts[if src == DateSource::TextYmd { 0 } else { 2 }].len();
    if year_digits != 4 || !(1..=12).contains(&m) || d < 1 || d > days_in_month(y, m) {
        return None;
    }
    let (h, mi, sec) = (n.get(3).copied().unwrap_or(0), n.get(4).copied().unwrap_or(0), n.get(5).copied().unwrap_or(0));
    if h > 23 || mi > 59 || sec > 59 {
        return None;
    }
    Some(days_from_civil(y, m, d) as f64 + (h * 3600 + mi * 60 + sec) as f64 / 86400.0)
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// The dates of a variable as days since 1970-01-01; and the count and first example of text that isn't a date.
fn dates(ds: &Dataset, name: &str, src: DateSource) -> Result<(Vec<Option<f64>>, usize, Option<String>)> {
    let i = ds.find(name).ok_or_else(|| Error::Invalid(format!("There is no variable named {name}.")))?;
    let text = matches!(src, DateSource::TextYmd | DateSource::TextMdy | DateSource::TextDmy);
    match (&ds.columns[i], text) {
        (Column::Text(t), true) => {
            let mut bad = 0;
            let mut example = None;
            let v = t
                .iter()
                .map(|c| {
                    let s = c.as_deref()?.trim();
                    if s.is_empty() {
                        return None;
                    }
                    let d = parse_text(s, src);
                    if d.is_none() {
                        bad += 1;
                        example.get_or_insert_with(|| s.to_string());
                    }
                    d
                })
                .collect();
            Ok((v, bad, example))
        }
        (Column::Numeric(_), false) => {
            let shift = match src {
                DateSource::Spreadsheet => days_from_civil(1899, 12, 30) as f64,
                DateSource::Seconds1582 => days_from_civil(1582, 10, 14) as f64,
                _ => days_from_civil(1960, 1, 1) as f64,
            };
            let scale = if matches!(src, DateSource::Seconds1582 | DateSource::Seconds1960) { 86400.0 } else { 1.0 };
            Ok((valid_values(ds, name)?.into_iter().map(|v| v.map(|x| shift + x / scale)).collect(), 0, None))
        }
        (Column::Text(_), false) => invalid(format!("{name} holds text; choose how its dates are written (year, month or day first).")),
        (Column::Numeric(_), true) => invalid(format!("{name} holds numbers; choose which kind of date numbers they are.")),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatePart {
    Year,
    Month,
    Day,
    /// 1 for Monday to 7 for Sunday (ISO 8601).
    Weekday,
    DayOfYear,
    Hour,
    Minute,
    /// Days since 1970-01-01: a plain number to sort, subtract or plot dates with.
    DayNumber,
}

impl DatePart {
    fn key(self) -> &'static str {
        match self {
            DatePart::Year => "year",
            DatePart::Month => "month",
            DatePart::Day => "day",
            DatePart::Weekday => "weekday",
            DatePart::DayOfYear => "yday",
            DatePart::Hour => "hour",
            DatePart::Minute => "minute",
            DatePart::DayNumber => "days",
        }
    }
    fn words(self) -> &'static str {
        match self {
            DatePart::Year => "Year",
            DatePart::Month => "Month",
            DatePart::Day => "Day of the month",
            DatePart::Weekday => "Day of the week (1 = Monday)",
            DatePart::DayOfYear => "Day of the year",
            DatePart::Hour => "Hour",
            DatePart::Minute => "Minute",
            DatePart::DayNumber => "Days since 1970-01-01",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatePartsSpec {
    pub variable: String,
    pub source: DateSource,
    pub parts: Vec<DatePart>,
}

/// New variables `<name>_year`, `<name>_month`, ... from a date variable.
pub fn date_parts(ds: &mut Dataset, spec: &DatePartsSpec) -> Result<Report> {
    if spec.parts.is_empty() {
        return invalid("Choose at least one part of the date.");
    }
    let (d, bad, example) = dates(ds, &spec.variable, spec.source)?;
    free(ds, &spec.parts.iter().map(|p| format!("{}_{}", spec.variable, p.key())).collect::<Vec<_>>())?;
    let src_label = ds.variable(&spec.variable)?.display_name().to_string();
    let mut names = Vec::new();
    let mut first = None;
    for &part in &spec.parts {
        let values: Vec<Option<f64>> = d
            .iter()
            .map(|x| {
                let x = (*x)?;
                let day = x.floor() as i64;
                let secs = ((x - x.floor()) * 86400.0).round() as i64;
                let (y, m, dd) = civil_from_days(day);
                Some(match part {
                    DatePart::Year => y as f64,
                    DatePart::Month => m as f64,
                    DatePart::Day => dd as f64,
                    DatePart::Weekday => ((day + 3).rem_euclid(7) + 1) as f64,
                    DatePart::DayOfYear => (day - days_from_civil(y, 1, 1) + 1) as f64,
                    DatePart::Hour => (secs / 3600) as f64,
                    DatePart::Minute => ((secs % 3600) / 60) as f64,
                    DatePart::DayNumber => x,
                })
            })
            .collect();
        let name = format!("{}_{}", spec.variable, part.key());
        let measure = if matches!(part, DatePart::Month | DatePart::Weekday) { Measure::Ordinal } else { Measure::Scale };
        first.get_or_insert_with(|| counts(&values));
        put(ds, &name, values, format!("{}: {}", src_label, part.words()), measure, false)?;
        if part == DatePart::Weekday {
            let days = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
            ds.variable_mut(&name)?.value_labels = days.iter().enumerate().map(|(i, l)| ValueLabel { value: (i + 1) as f64, label: l.to_string() }).collect();
        }
        names.push(name);
    }
    let (valid, missing) = first.unwrap_or((0, 0));
    Ok(Report { names, valid, missing, unreadable: (bad > 0).then(|| (bad, example.unwrap_or_default())) })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Days,
    Weeks,
    /// Whole months completed.
    Months,
    /// Whole years completed, as an age is counted.
    Years,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DateDiffSpec {
    pub start: String,
    pub end: String,
    pub source: DateSource,
    pub unit: Unit,
    pub target: String,
    #[serde(default)]
    pub label: Option<String>,
}

/// The time from `start` to `end` in each case (negative when end comes first). Days and weeks keep a fraction for the
/// time of day; months and years count whole ones completed.
pub fn date_difference(ds: &mut Dataset, spec: &DateDiffSpec) -> Result<Report> {
    let (a, bad_a, ex_a) = dates(ds, &spec.start, spec.source)?;
    let (b, bad_b, ex_b) = dates(ds, &spec.end, spec.source)?;
    free(ds, std::slice::from_ref(&spec.target))?;
    let whole = |from: f64, to: f64, months: bool| -> f64 {
        let (y1, m1, d1) = civil_from_days(from.floor() as i64);
        let (y2, m2, d2) = civil_from_days(to.floor() as i64);
        let mut n = (y2 - y1) * 12 + (m2 - m1);
        if to >= from {
            if d2 < d1 {
                n -= 1;
            }
        } else if d2 > d1 {
            n += 1;
        }
        if months { n as f64 } else { (n / 12) as f64 }
    };
    let values: Vec<Option<f64>> = a
        .iter()
        .zip(&b)
        .map(|(x, y)| {
            let (x, y) = ((*x)?, (*y)?);
            Some(match spec.unit {
                Unit::Days => y - x,
                Unit::Weeks => (y - x) / 7.0,
                Unit::Months => whole(x, y, true),
                Unit::Years => whole(x, y, false),
            })
        })
        .collect();
    let words = match spec.unit {
        Unit::Days => "Days",
        Unit::Weeks => "Weeks",
        Unit::Months => "Whole months",
        Unit::Years => "Whole years",
    };
    let label = spec.label.clone().filter(|l| !l.trim().is_empty()).unwrap_or_else(|| format!("{words} from {} to {}", spec.start, spec.end));
    let (valid, missing) = counts(&values);
    put(ds, &spec.target, values, label, Measure::Scale, false)?;
    let bad = bad_a + bad_b;
    Ok(Report { names: vec![spec.target.clone()], valid, missing, unreadable: (bad > 0).then(|| (bad, ex_a.or(ex_b).unwrap_or_default())) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ds() -> Dataset {
        let mut d = Dataset::new("t");
        d.add_variable("g", Column::Numeric(vec![Some(2.0), Some(1.0), Some(2.0), Some(1.0), Some(9.0), None])).unwrap();
        d.add_variable("x", Column::Numeric(vec![Some(3.0), Some(1.0), Some(3.0), Some(5.0), Some(2.0), Some(4.0)])).unwrap();
        d.add_variable("s", Column::Text(vec![Some("b".into()), Some("a".into()), None, Some("B".into()), Some("c".into()), Some("a".into())])).unwrap();
        d.variable_mut("g").unwrap().missing_values = vec![9.0];
        d
    }

    fn col(d: &Dataset, name: &str) -> Vec<Option<f64>> {
        match &d.columns[d.find(name).unwrap()] {
            Column::Numeric(v) => v.clone(),
            Column::Text(_) => panic!("text"),
        }
    }

    #[test]
    fn sort_puts_missing_last_and_keeps_ties_in_order() {
        let mut d = ds();
        sort_cases(&mut d, &[SortKey { name: "g".into(), descending: false }, SortKey { name: "x".into(), descending: true }]).unwrap();
        assert_eq!(col(&d, "x"), vec![Some(5.0), Some(1.0), Some(3.0), Some(3.0), Some(4.0), Some(2.0)]);
        assert_eq!(col(&d, "g"), vec![Some(1.0), Some(1.0), Some(2.0), Some(2.0), None, Some(9.0)], "the missing code 9 sorts with the empty cell, then by x");
        let mut d = ds();
        sort_cases(&mut d, &[SortKey { name: "s".into(), descending: false }]).unwrap();
        assert_eq!(col(&d, "x"), vec![Some(5.0), Some(1.0), Some(4.0), Some(3.0), Some(2.0), Some(3.0)], "B before a, by character code; the empty cell last");
        assert!(sort_cases(&mut d, &[]).is_err());
    }

    #[test]
    fn ranks_follow_r() {
        let mut d = ds();
        rank_cases(&mut d, &RankSpec { variables: vec!["x".into()], ties: Ties::Mean, descending: false, by: None, into: vec![] }).unwrap();
        assert_eq!(col(&d, "r_x"), vec![Some(3.5), Some(1.0), Some(3.5), Some(6.0), Some(2.0), Some(5.0)]);
        rank_cases(&mut d, &RankSpec { variables: vec!["x".into()], ties: Ties::Low, descending: true, by: Some("g".into()), into: vec!["rg".into()] }).unwrap();
        assert_eq!(col(&d, "rg"), vec![Some(1.0), Some(2.0), Some(1.0), Some(1.0), None, None], "within g; no group for the missing code");
        rank_cases(&mut d, &RankSpec { variables: vec!["x".into()], ties: Ties::Sequential, descending: false, by: None, into: vec!["rs".into()] }).unwrap();
        assert_eq!(col(&d, "rs"), vec![Some(3.0), Some(1.0), Some(4.0), Some(6.0), Some(2.0), Some(5.0)]);
    }

    #[test]
    fn z_scores_and_reversed_items() {
        let mut d = ds();
        standardize(&mut d, &StandardizeSpec { variables: vec!["x".into()], into: vec![] }).unwrap();
        let z = col(&d, "z_x");
        assert!((z[3].unwrap() - (5.0 - 3.0) / (2.0f64).sqrt()).abs() < 1e-12, "{z:?}");
        assert_eq!(d.variable("z_x").unwrap().label, "z-score of x");
        d.variable_mut("g").unwrap().value_labels = vec![ValueLabel { value: 1.0, label: "Low".into() }, ValueLabel { value: 9.0, label: "No answer".into() }];
        reverse_score(&mut d, &ReverseSpec { variables: vec!["g".into()], min: 1.0, max: 2.0, into: vec![], replace: false }).unwrap();
        assert_eq!(col(&d, "g_r"), vec![Some(1.0), Some(2.0), Some(1.0), Some(2.0), Some(9.0), None], "the missing code stays");
        let v = d.variable("g_r").unwrap();
        assert_eq!((v.value_labels[0].value, v.value_labels[0].label.as_str(), v.missing_values.clone()), (2.0, "Low", vec![9.0]));
        assert!(reverse_score(&mut d, &ReverseSpec { variables: vec!["x".into()], min: 1.0, max: 4.0, into: vec![], replace: false }).unwrap_err().to_string().contains("value 5"));
    }

    #[test]
    fn dates_in_every_form() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(civil_from_days(days_from_civil(2024, 2, 29)), (2024, 2, 29));
        let mut d = Dataset::new("t");
        d.add_variable("born", Column::Text(vec![Some("2000-02-29".into()), Some("1999/12/31 23:30".into()), Some("31-02-2000".into()), None])).unwrap();
        d.add_variable("seen", Column::Text(vec![Some("2026-02-28".into()), Some("2026-12-30".into()), Some("2026-01-01".into()), Some("2026-01-01".into())])).unwrap();
        let r = date_parts(&mut d, &DatePartsSpec { variable: "born".into(), source: DateSource::TextYmd, parts: vec![DatePart::Year, DatePart::Weekday, DatePart::DayOfYear, DatePart::Hour] }).unwrap();
        assert_eq!(r.unreadable, Some((1, "31-02-2000".into())));
        assert_eq!(col(&d, "born_year"), vec![Some(2000.0), Some(1999.0), None, None]);
        assert_eq!(col(&d, "born_weekday"), vec![Some(2.0), Some(5.0), None, None], "29 Feb 2000 was a Tuesday, 31 Dec 1999 a Friday");
        assert_eq!(col(&d, "born_yday"), vec![Some(60.0), Some(365.0), None, None]);
        assert_eq!(col(&d, "born_hour"), vec![Some(0.0), Some(23.0), None, None]);
        date_difference(&mut d, &DateDiffSpec { start: "born".into(), end: "seen".into(), source: DateSource::TextYmd, unit: Unit::Years, target: "age".into(), label: None }).unwrap();
        assert_eq!(col(&d, "age"), vec![Some(25.0), Some(26.0), None, None], "the day before a 29 February birthday in a common year is not yet the birthday");
        date_difference(&mut d, &DateDiffSpec { start: "born".into(), end: "seen".into(), source: DateSource::TextYmd, unit: Unit::Days, target: "days".into(), label: None }).unwrap();
        assert_eq!(col(&d, "days")[0], Some((days_from_civil(2026, 2, 28) - days_from_civil(2000, 2, 29)) as f64));
        // Numbers from files: a spreadsheet's 45000 is 2023-03-15; .sav seconds; .dta days.
        let mut n = Dataset::new("n");
        n.add_variable("xl", Column::Numeric(vec![Some(45000.5)])).unwrap();
        n.add_variable("sav", Column::Numeric(vec![Some(13_853_548_800.0)])).unwrap();
        n.add_variable("dta", Column::Numeric(vec![Some(23_450.0)])).unwrap();
        date_parts(&mut n, &DatePartsSpec { variable: "xl".into(), source: DateSource::Spreadsheet, parts: vec![DatePart::Year, DatePart::Month, DatePart::Day, DatePart::Hour] }).unwrap();
        assert_eq!([col(&n, "xl_year")[0], col(&n, "xl_month")[0], col(&n, "xl_day")[0], col(&n, "xl_hour")[0]], [Some(2023.0), Some(3.0), Some(15.0), Some(12.0)]);
        date_parts(&mut n, &DatePartsSpec { variable: "sav".into(), source: DateSource::Seconds1582, parts: vec![DatePart::Year, DatePart::Month, DatePart::Day] }).unwrap();
        assert_eq!([col(&n, "sav_year")[0], col(&n, "sav_month")[0], col(&n, "sav_day")[0]], [Some(2021.0), Some(10.0), Some(14.0)]);
        date_parts(&mut n, &DatePartsSpec { variable: "dta".into(), source: DateSource::Days1960, parts: vec![DatePart::Year, DatePart::Month, DatePart::Day] }).unwrap();
        assert_eq!([col(&n, "dta_year")[0], col(&n, "dta_month")[0], col(&n, "dta_day")[0]], [Some(2024.0), Some(3.0), Some(15.0)]);
        assert!(date_parts(&mut n, &DatePartsSpec { variable: "xl".into(), source: DateSource::TextYmd, parts: vec![DatePart::Year] }).is_err());
    }
}
