//! Test scoring and classical item analysis (`scale.item_analysis`): letter responses scored against a key, then the
//! classic item statistics.
//!
//! Each item's response (a letter, letters for a select-all item, or a numeric code) is compared with the key. A key
//! entry lists the answers that earn the item's points: `B`, or `B/D` when two answers are right, or `AC` for a
//! select-all item that needs both A and C. An item can also be dropped (left out of every figure) or credited to
//! everyone. A blank response earns nothing and is counted as a blank; a case with every item blank is left out. On an
//! item credited to everyone a blank is credited, never counted blank (PSTAT-1 revision 1.3): its sheets go in a row
//! `credited` among the options.
//!
//! Reported, and checked against R (psych's score.multiple.choice and alpha, and base R for the rest):
//! - difficulty, the share of the item's points earned;
//! - two point-biserial correlations, one with the total that includes the item (as an answer-sheet reader reports
//!   it) and one with the total of the other items (corrected), each labeled as such;
//! - the upper and lower groups, the top and bottom 27 percent by total score (round(0.27 N) cases each, ties broken
//!   by case order, which is how OpenAssess forms them), their difficulties and the discrimination index D = U − L;
//! - each option's count overall and in the upper, middle and lower groups, with blanks;
//! - KR-20 (Cronbach's alpha when items carry different points), KR-21 for right-or-wrong items of equal weight, the
//!   test mean and SD, and the standard error of measurement.
//!
//! - with `groups` (one label per item, such as each question's objective, chapter or Bloom level from OpenAssess's
//!   `assessment.json`), a subscore for each group: its items, points possible, mean, SD, mean share of the points
//!   and reliability (KR-20 or alpha of the group's items).
//!
//! KR-21 and the variances in KR-20 use N as the divisor, describing the class tested; KR-20 is the same with either
//! divisor. The SD reported for the total uses N − 1, as every other PrismStatistics table does.

use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::apa;
use crate::error::{Error, Result};
use crate::model::{format_code, Column, Dataset};
use crate::result::{Cell, Diagnostic, DiagnosticState, ResultBundle, Table};

pub const METHOD_ID: &str = "scale.item_analysis";
pub const METHOD_VERSION: u32 = 1;

/// Fewer cases than this and the figures are flagged as unstable (the same line OpenAssess draws).
pub const SMALL_GROUP: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Adjustment {
    #[default]
    None,
    /// Left out of the total and every figure.
    Dropped,
    /// Every case earns the item's points, whatever was marked.
    CreditAll,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyEntry {
    /// The answers that earn the points. "AC" means both A and C must be marked (a select-all item).
    pub correct: Vec<String>,
    #[serde(default = "one")]
    pub points: f64,
    #[serde(default)]
    pub adjustment: Adjustment,
}

fn one() -> f64 {
    1.0
}

fn default_fraction() -> f64 {
    0.27
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    /// The response variables, one per item.
    pub items: Vec<String>,
    /// One key entry per item, in the same order. With `scored`, only each entry's points are read (the points
    /// possible), and the key may be left empty to take each item's highest value seen.
    #[serde(default)]
    pub key: Vec<KeyEntry>,
    /// The items already hold points earned (such as a results file from OpenAssess), not answers. An empty cell then
    /// means the item does not count for that case, and the case is left out (listwise), as Reliability does.
    #[serde(default)]
    pub scored: bool,
    /// The share of cases in each of the upper and lower groups.
    #[serde(default = "default_fraction")]
    pub group_fraction: f64,
    /// One label per item, in the same order, to report a subscore for each label (an empty label: the item is in no
    /// group). Empty: no subscores.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,
    /// What the labels are, for the table's title and first column ("Objective", "Chapter", "Bloom level").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group_name: String,
}

/// The question details OpenAssess keeps in `assessment.json` (PSTAT-1 section 7.2) that items can be grouped by:
/// the member of `details` and its name in a table.
pub const DETAILS: [(&str, &str); 5] = [("objective", "Objective"), ("chapter", "Chapter"), ("section", "Section"), ("bloom", "Bloom level"), ("difficulty", "Difficulty level")];

const BLOOM: [&str; 6] = ["remember", "understand", "apply", "analyze", "evaluate", "create"];
const LEVELS: [&str; 3] = ["easy", "medium", "hard"];

/// Each question's value of one detail (`objective`, `chapter`, `bloom`, ...) from `assessment.json`, by the names of
/// both its points variable and its answer variable, so it serves either analysis. Bloom levels and difficulty levels
/// start with a capital ("Apply", "Hard"); the others are kept as written.
pub fn details_from_assessment(ds: &Dataset, part: &serde_json::Value, detail: &str) -> std::collections::BTreeMap<String, String> {
    let name_of = |id: &serde_json::Value| id.as_u64().and_then(|id| ds.variables.iter().find(|v| u64::from(v.id) == id)).map(|v| v.name.clone());
    let mut out = std::collections::BTreeMap::new();
    for q in part["questions"].as_array().into_iter().flatten() {
        let value = match &q["details"][detail] {
            serde_json::Value::String(s) => s.trim().to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            _ => continue,
        };
        if value.is_empty() {
            continue;
        }
        let value = if detail == "bloom" || detail == "difficulty" {
            let mut c = value.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
        } else {
            value
        };
        for member in ["points_variable", "answer_variable"] {
            if let Some(name) = name_of(&q[member]) {
                out.insert(name, value.clone());
            }
        }
    }
    out
}

/// The order groups are listed in: Bloom's levels and easy, medium, hard in their own order; anything else in
/// natural order, so chapter 2 comes before chapter 10.
fn group_order(labels: &mut [String]) {
    let rank = |list: &[&str], s: &str| list.iter().position(|w| w.eq_ignore_ascii_case(s));
    for list in [&BLOOM[..], &LEVELS[..]] {
        if labels.iter().all(|l| rank(list, l).is_some()) {
            labels.sort_by_key(|l| rank(list, l));
            return;
        }
    }
    labels.sort_by(|a, z| natural(a, z));
}

/// Compares text with its runs of digits read as numbers ("2.10" after "2.9", "Chapter 10" after "Chapter 2").
fn natural(a: &str, z: &str) -> std::cmp::Ordering {
    fn parts(s: &str) -> Vec<(bool, String)> {
        let mut out: Vec<(bool, String)> = Vec::new();
        for c in s.chars() {
            let d = c.is_ascii_digit();
            match out.last_mut() {
                Some((was, t)) if *was == d => t.push(c),
                _ => out.push((d, c.to_string())),
            }
        }
        out
    }
    let (pa, pz) = (parts(a), parts(z));
    for (x, y) in pa.iter().zip(&pz) {
        let o = match (x, y) {
            ((true, x), (true, y)) => {
                let (x, y) = (x.trim_start_matches('0'), y.trim_start_matches('0'));
                x.len().cmp(&y.len()).then_with(|| x.cmp(y))
            }
            ((_, x), (_, y)) => x.to_lowercase().cmp(&y.to_lowercase()),
        };
        if o != std::cmp::Ordering::Equal {
            return o;
        }
    }
    pa.len().cmp(&pz.len()).then_with(|| a.cmp(z))
}

/// Parses a key typed as text: entries separated by commas or spaces, alternatives within an entry by "/" or "|".
/// "B, A, B/D, AC, -, *" is six items: the third with two right answers, the fourth a select-all item, the fifth dropped
/// and the sixth credited to everyone. The dialog parses the same way (app/ui/src/analyses.ts, `parseKey`).
pub fn parse_key(text: &str) -> Vec<KeyEntry> {
    text.split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .filter(|s| !s.trim().is_empty())
        .map(|s| match s.trim() {
            "-" => KeyEntry { correct: vec![], points: 1.0, adjustment: Adjustment::Dropped },
            "*" => KeyEntry { correct: vec![], points: 1.0, adjustment: Adjustment::CreditAll },
            s => KeyEntry { correct: s.split(['/', '|']).map(normalize).filter(|a| !a.is_empty()).collect(), points: 1.0, adjustment: Adjustment::None },
        })
        .collect()
}

/// The answer variables and key of an `OpenAssess item results 2` file (PSTAT-1 section 7.2), from its part
/// `assessment.json`, for scoring the letters here. A question whose key differs by version, gives partial credit or
/// has no letters has no equivalent in this key; it is listed in the second value (by its number on version A) to be
/// analysed from its points instead, as the spec says.
pub fn key_from_assessment(ds: &Dataset, part: &serde_json::Value) -> (Vec<(String, KeyEntry)>, Vec<u64>) {
    let name_of = |id: &serde_json::Value| id.as_u64().and_then(|id| ds.variables.iter().find(|v| u64::from(v.id) == id)).map(|v| v.name.clone());
    let mut keyed = Vec::new();
    let mut points_only = Vec::new();
    for q in part["questions"].as_array().into_iter().flatten() {
        let number = q["number"].as_u64().unwrap_or(0);
        if q["points_variable"].is_null() {
            continue; // left out of the file
        }
        let Some(var) = name_of(&q["answer_variable"]) else {
            points_only.push(number);
            continue;
        };
        let entries = q["key"].as_array().map(Vec::as_slice).unwrap_or_default();
        let simple = entries.len() == 1 && entries[0].get("versions").is_none() && entries[0]["partial_credit"].as_bool() != Some(true) && q["type"].as_str() != Some("other");
        if !simple {
            points_only.push(number);
            continue;
        }
        let e = &entries[0];
        let adjustment = match e["adjustment"].as_str() {
            Some("dropped") => Adjustment::Dropped,
            Some("credit_all") => Adjustment::CreditAll,
            _ => Adjustment::None,
        };
        let correct = e["correct"].as_array().into_iter().flatten().filter_map(|c| c.as_str().map(str::to_string)).collect();
        keyed.push((var, KeyEntry { correct, points: e["points"].as_f64().unwrap_or(1.0), adjustment }));
    }
    (keyed, points_only)
}

/// A response or key answer in one comparable form: upper case, with spaces and separators removed and the letters of a
/// select-all answer in order, so "c, a" and "AC" are the same answer. Numbers keep their digits ("10" stays "10").
pub fn normalize(s: &str) -> String {
    let t: String = s.chars().filter(|c| !c.is_whitespace() && !matches!(c, ',' | ';' | '+' | '&')).collect::<String>().to_uppercase();
    if !t.is_empty() && t.chars().all(|c| c.is_ascii_alphabetic()) {
        let mut letters: Vec<char> = t.chars().collect();
        letters.sort_unstable();
        letters.dedup();
        letters.into_iter().collect()
    } else {
        t
    }
}

/// One case's answer to one item, as text, or None for a blank.
fn responses(ds: &Dataset, name: &str) -> Result<Vec<Option<String>>> {
    let i = ds.find(name).ok_or_else(|| Error::UnknownVariable(name.to_string()))?;
    Ok(match &ds.columns[i] {
        Column::Text(v) => v.iter().map(|c| c.as_deref().map(normalize).filter(|s| !s.is_empty())).collect(),
        Column::Numeric(_) => numeric(ds, name)?.iter().map(|s| s.value().map(format_code)).collect(),
    })
}

fn mean(x: &[f64]) -> f64 {
    x.iter().sum::<f64>() / x.len() as f64
}

/// The variance with N as the divisor.
fn var_n(x: &[f64]) -> f64 {
    let m = mean(x);
    x.iter().map(|v| (v - m).powi(2)).sum::<f64>() / x.len() as f64
}

fn pearson(x: &[f64], y: &[f64]) -> f64 {
    let (mx, my) = (mean(x), mean(y));
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (a, b) in x.iter().zip(y) {
        sxy += (a - mx) * (b - my);
        sxx += (a - mx).powi(2);
        syy += (b - my).powi(2);
    }
    if sxx <= 1e-12 || syy <= 1e-12 {
        return f64::NAN;
    }
    sxy / (sxx * syy).sqrt()
}

/// Cronbach's alpha (KR-20 for right-or-wrong items) of item score columns.
fn alpha(cols: &[&Vec<f64>]) -> f64 {
    let k = cols.len() as f64;
    if cols.len() < 2 {
        return f64::NAN;
    }
    let n = cols[0].len();
    let totals: Vec<f64> = (0..n).map(|r| cols.iter().map(|c| c[r]).sum()).collect();
    let vt = var_n(&totals);
    if vt <= 1e-12 {
        return f64::NAN;
    }
    k / (k - 1.0) * (1.0 - cols.iter().map(|c| var_n(c)).sum::<f64>() / vt)
}

/// Upper and lower groups: the indices of the top and bottom `g` cases by total, ties broken by case order.
pub fn groups(totals: &[f64], fraction: f64) -> Option<(Vec<usize>, Vec<usize>)> {
    let n = totals.len();
    // The C3 rule: g = floor((pct n + 50) / 100), the percentage of n rounded half up in whole numbers, so that no
    // rounding of 0.27 × n can move it (27 percent of 150 is 40.5, which gives 41).
    let pct = (fraction * 100.0).round() as usize;
    let g = (pct * n + 50) / 100;
    if g < 2 || 2 * g > n {
        return None;
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| totals[b].total_cmp(&totals[a]).then(a.cmp(&b)));
    Some((order[..g].to_vec(), order[n - g..].to_vec()))
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    if spec.items.len() < 2 {
        return Err(Error::Invalid("Choose at least two items.".into()));
    }
    if spec.key.len() != spec.items.len() && !(spec.scored && spec.key.is_empty()) {
        return Err(Error::Invalid(format!(
            "The key has {} {} but {} {} chosen; give one key entry for each item, in the same order.",
            spec.key.len(),
            if spec.key.len() == 1 { "entry" } else { "entries" },
            spec.items.len(),
            if spec.items.len() == 1 { "item is" } else { "items are" }
        )));
    }
    for (i, a) in spec.items.iter().enumerate() {
        if spec.items[..i].iter().any(|b| ds.find(a) == ds.find(b)) {
            return Err(Error::Invalid(format!("{a} is listed twice.")));
        }
    }
    if !spec.groups.is_empty() && spec.groups.len() != spec.items.len() {
        return Err(Error::Invalid(format!("Give one group label for each item: there are {} labels for {} items.", spec.groups.len(), spec.items.len())));
    }
    if !(spec.group_fraction > 0.0 && spec.group_fraction <= 0.5) {
        return Err(Error::Invalid("The upper and lower groups must each be more than 0 and at most half of the cases.".into()));
    }
    for (name, k) in spec.items.iter().zip(&spec.key) {
        if !(k.points.is_finite() && k.points > 0.0) {
            return Err(Error::Invalid(format!("{name}'s points must be a number above zero.")));
        }
        if !spec.scored && k.adjustment == Adjustment::None && k.correct.iter().all(|c| normalize(c).is_empty()) {
            return Err(Error::Invalid(format!("{name} has no right answer in the key.")));
        }
    }
    let labels: Vec<String> = spec.items.iter().map(|n| ds.variable(n).map(|v| v.display_name().to_string())).collect::<Result<_>>()?;
    let total_rows = ds.n_rows();
    if spec.scored {
        return run_scored(ds, spec, labels);
    }
    let answers: Vec<Vec<Option<String>>> = spec.items.iter().map(|n| responses(ds, n)).collect::<Result<_>>()?;
    let keys: Vec<Vec<String>> = spec.key.iter().map(|k| k.correct.iter().map(|c| normalize(c)).filter(|c| !c.is_empty()).collect()).collect();

    // Items that count, and cases with at least one answer to them.
    let live: Vec<usize> = (0..spec.items.len()).filter(|&i| spec.key[i].adjustment != Adjustment::Dropped).collect();
    if live.len() < 2 {
        return Err(Error::Invalid("At least two items must count; the others are dropped.".into()));
    }
    let mut tally = Tally::default();
    let used: Vec<usize> = (0..total_rows)
        .filter(|&r| {
            let any = live.iter().any(|&i| answers[i][r].is_some());
            if !any {
                tally.add("every item blank");
            }
            any
        })
        .collect();
    let n = used.len();

    // Points earned, per item and case.
    let earned: Vec<Vec<f64>> = (0..spec.items.len())
        .map(|i| {
            used.iter()
                .map(|&r| match spec.key[i].adjustment {
                    Adjustment::CreditAll => spec.key[i].points,
                    _ => match &answers[i][r] {
                        Some(a) if keys[i].iter().any(|c| c == a) => spec.key[i].points,
                        _ => 0.0,
                    },
                })
                .collect()
        })
        .collect();
    let points: Vec<f64> = spec.key.iter().map(|k| k.points).collect();
    let adjustments: Vec<Adjustment> = spec.key.iter().map(|k| k.adjustment).collect();
    let letters = Letters { answers: &answers, keys: &keys, used: &used };
    Ok(report(spec, &labels, &live, &earned, &points, &adjustments, Some(letters), tally.accounting(total_rows, n), total_rows, ""))
}

/// The letter answers behind the scores, for the option table.
struct Letters<'a> {
    answers: &'a [Vec<Option<String>>],
    keys: &'a [Vec<String>],
    used: &'a [usize],
}

/// Items that already hold points earned.
fn run_scored(ds: &Dataset, spec: &Spec, labels: Vec<String>) -> Result<ResultBundle> {
    let cells: Vec<Vec<Slot<f64>>> = spec.items.iter().map(|n| numeric(ds, n)).collect::<Result<_>>()?;
    let total_rows = ds.n_rows();
    let adjustments: Vec<Adjustment> = (0..spec.items.len()).map(|i| spec.key.get(i).map_or(Adjustment::None, |k| k.adjustment)).collect();
    let live: Vec<usize> = (0..spec.items.len()).filter(|&i| adjustments[i] != Adjustment::Dropped).collect();
    if live.len() < 2 {
        return Err(Error::Invalid("At least two items must count; the others are dropped.".into()));
    }
    let mut tally = Tally::default();
    let used: Vec<usize> = (0..total_rows).filter(|&r| !live.iter().any(|&i| tally.check(&cells[i][r], &spec.items[i]))).collect();
    let n = used.len();
    let mut earned: Vec<Vec<f64>> = cells.iter().map(|c| used.iter().map(|&r| c[r].value().unwrap_or(0.0)).collect()).collect();
    if let Some(bad) = live.iter().find(|&&i| earned[i].iter().any(|v| *v < 0.0)) {
        return Err(Error::Invalid(format!("{} has a negative score; scored items must hold the points earned.", labels[*bad])));
    }
    let from_data = spec.key.is_empty();
    let points: Vec<f64> = (0..spec.items.len())
        .map(|i| if from_data { earned[i].iter().cloned().fold(0.0, f64::max) } else { spec.key[i].points })
        .collect();
    for &i in &live {
        if adjustments[i] == Adjustment::CreditAll {
            earned[i].iter_mut().for_each(|v| *v = points[i]);
        }
        if points[i] <= 0.0 {
            return Err(Error::Invalid(format!("Nobody earned any points on {}, so its points possible are not known; give them in the key.", labels[i])));
        }
        if let Some(v) = earned[i].iter().find(|v| **v > points[i] + 1e-9) {
            return Err(Error::Invalid(format!("{} has a score of {} but is worth {} points.", labels[i], apa::plain(*v), apa::plain(points[i]))));
        }
    }
    let extra = if from_data { "Each item's points possible were taken as its highest score among the cases. " } else { "" };
    Ok(report(spec, &labels, &live, &earned, &points, &adjustments, None, tally.accounting(total_rows, n), total_rows, extra))
}

#[allow(clippy::too_many_arguments)]
fn report(
    spec: &Spec,
    labels: &[String],
    live: &[usize],
    earned: &[Vec<f64>],
    points: &[f64],
    adjustments: &[Adjustment],
    letters: Option<Letters>,
    cases: crate::result::CaseAccounting,
    total_rows: usize,
    extra_note: &str,
) -> ResultBundle {
    let _ = total_rows;
    let n = cases.analyzed;
    let k = live.len();
    let totals: Vec<f64> = (0..n).map(|r| live.iter().map(|&i| earned[i][r]).sum()).collect();
    let possible: f64 = live.iter().map(|&i| points[i]).sum();
    // KR-20 is alpha for right-or-wrong items (whatever each is worth); KR-21 also needs every item worth the same.
    let right_or_wrong = live.iter().all(|&i| earned[i].iter().all(|v| *v == 0.0 || (*v - points[i]).abs() < 1e-12));
    let equal_points = right_or_wrong && live.iter().all(|&i| points[i] == points[live[0]]);

    let dropped: Vec<String> = (0..spec.items.len()).filter(|&i| adjustments[i] == Adjustment::Dropped).map(|i| labels[i].clone()).collect();
    let credited: Vec<String> = (0..spec.items.len()).filter(|&i| adjustments[i] == Adjustment::CreditAll).map(|i| labels[i].clone()).collect();
    let pct = apa::plain((spec.group_fraction * 100.0).round());
    let reliability_name = if right_or_wrong { "KR-20" } else { "Cronbach's alpha" };
    let mut note = format!(
        "{}Difficulty is the share of an item's points earned. Two correlations with the total are given: the point-biserial with a total that includes the item, and the corrected item-total r with a total that leaves the item out. The upper and lower groups are the {pct} percent of cases with the highest and lowest totals (ties broken by case order), and D is the upper group's difficulty minus the lower group's. Reliability is {reliability_name}{}, with item and total variances divided by N; the standard error of measurement is the total's SD times the square root of 1 minus the reliability. {extra_note}",
        if letters.is_some() { format!("Responses to {k} items were scored against the key; a blank earns no points. ") } else { format!("The {k} items hold the points each case earned; cases were used when every item had a value (listwise). ") },
        if equal_points { ", with KR-21 from the mean and variance of the total" } else if right_or_wrong { " (KR-21 is not given, since the items are not all worth the same points)" } else { " (KR-20 is defined for right-or-wrong items, and some items give partial credit)" }
    );
    if !dropped.is_empty() {
        note += &format!("{} {} dropped from the key and left out of every figure. ", apa::series(&dropped), if dropped.len() == 1 { "was" } else { "were" });
    }
    if !credited.is_empty() {
        note += &format!("Every case was credited with {}. ", apa::series(&credited));
    }
    note += "Computed by PrismStatistics.";
    let mut b = common::bundle(METHOD_ID, METHOD_VERSION, format!("Item analysis: {k} items"), serde_json::to_value(spec).expect("a spec always serializes"), cases, note);
    set(&mut b, "k", k as f64);
    set(&mut b, "n", n as f64);
    set(&mut b, "possible", possible);
    set_text(&mut b, "items", live.iter().map(|&i| labels[i].clone()).collect::<Vec<_>>().join("\u{1f}"));
    set_text(&mut b, "reliability.kind", reliability_name);
    if n < 3 {
        fail(&mut b, format!("Item analysis needs at least three cases; there {} {n}.", if n == 1 { "is" } else { "are" }));
        return b;
    }

    // The test as a whole.
    let m = mean(&totals);
    let vn = var_n(&totals);
    let sd = (vn * n as f64 / (n as f64 - 1.0)).sqrt();
    let live_cols: Vec<&Vec<f64>> = live.iter().map(|&i| &earned[i]).collect();
    let rel = alpha(&live_cols);
    let kr21 = if equal_points && vn > 1e-12 {
        // On the number-correct scale.
        let w = points[live[0]];
        let (mc, vc) = (m / w, vn / (w * w));
        let kk = k as f64;
        Some(kk / (kk - 1.0) * (1.0 - mc * (kk - mc) / (kk * vc)))
    } else {
        None
    };
    let sem = if rel.is_finite() { sd * (1.0 - rel).max(0.0).sqrt() } else { f64::NAN };
    for (key, v) in [("mean", m), ("sd", sd), ("sd.n", vn.sqrt()), ("reliability", rel), ("sem", sem), ("mean.p", m / possible)] {
        set(&mut b, key, v);
    }
    if right_or_wrong {
        set(&mut b, "kr20", rel);
    } else {
        set(&mut b, "alpha", rel);
    }
    if let Some(v) = kr21 {
        set(&mut b, "kr21", v);
    }
    let (lo, hi) = totals.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, z), v| (a.min(*v), z.max(*v)));
    let split = groups(&totals, spec.group_fraction);
    if let Some((u, _)) = &split {
        set(&mut b, "group.size", u.len() as f64);
    }
    b.tables.push(Table {
        id: "test".into(),
        title: "Test summary".into(),
        columns: vec!["N".into(), "Items".into(), "Points possible".into(), "Mean".into(), "SD".into(), "Lowest".into(), "Highest".into(), if right_or_wrong { "KR-20".into() } else { "Cronbach's α".into() }, "KR-21".into(), "SEM".into()],
        rows: vec![vec![count(n), count(k), num(possible, 2), num(m, 2), num(sd, 2), num(lo, 2), num(hi, 2), num(rel, 3), opt(kr21, 3), num(sem, 2)]],
        footnotes: {
            let mut f = vec!["SD of the total uses N − 1. KR-20 and KR-21 use variances divided by N. SEM = standard error of measurement.".to_string()];
            if !right_or_wrong {
                f.push("Some items give partial credit, so Cronbach's alpha is given in place of KR-20, and KR-21 is not defined.".into());
            } else if !equal_points {
                f.push("The items are not all worth the same points, so KR-21 is not defined.".into());
            }
            f
        },
    });

    // Item statistics.
    let mut item_rows = Vec::new();
    let mut weak = Vec::new();
    let mut negative = Vec::new();
    for (pos, &i) in live.iter().enumerate() {
        let w = points[i];
        let frac: Vec<f64> = earned[i].iter().map(|e| e / w).collect();
        let p = mean(&frac);
        let rpb = pearson(&earned[i], &totals);
        let rest: Vec<f64> = (0..n).map(|r| totals[r] - earned[i][r]).collect();
        let rpb_c = pearson(&earned[i], &rest);
        let others: Vec<&Vec<f64>> = live.iter().filter(|&&j| j != i).map(|&j| &earned[j]).collect();
        let a_del = if k > 2 { alpha(&others) } else { f64::NAN };
        // A question credited to everyone is never counted blank (PSTAT-1 1.3, "Counting the answers"): an empty answer
        // there is credited, whether left blank or never settled.
        let blanks = letters.as_ref().map(|l| if adjustments[i] == Adjustment::CreditAll { 0 } else { l.used.iter().filter(|&&r| l.answers[i][r].is_none()).count() });
        let key_text = match (&letters, adjustments[i]) {
            (_, Adjustment::CreditAll) => "(all)".to_string(),
            (Some(l), _) => l.keys[i].join("/"),
            (None, _) => format!("{} pts", apa::plain(w)),
        };
        let key = format!("item.{}", pos + 1);
        set_text(&mut b, format!("{key}.label"), labels[i].clone());
        set_text(&mut b, format!("{key}.key"), key_text.clone());
        set(&mut b, format!("{key}.p"), p);
        set(&mut b, format!("{key}.mean"), mean(&earned[i]));
        set(&mut b, format!("{key}.possible"), w);
        if let Some(bl) = blanks {
            set(&mut b, format!("{key}.blank"), bl as f64);
        }
        if rpb.is_finite() {
            set(&mut b, format!("{key}.rpb"), rpb);
        }
        if rpb_c.is_finite() {
            set(&mut b, format!("{key}.rpb_corrected"), rpb_c);
            if rpb_c < 0.0 {
                negative.push(labels[i].clone());
            } else if rpb_c < 0.2 {
                weak.push(labels[i].clone());
            }
        }
        if a_del.is_finite() {
            set(&mut b, format!("{key}.alpha_deleted"), a_del);
        }
        let (up, low) = match &split {
            Some((u, l)) => {
                let up = u.iter().map(|&r| frac[r]).sum::<f64>() / u.len() as f64;
                let low = l.iter().map(|&r| frac[r]).sum::<f64>() / l.len() as f64;
                set(&mut b, format!("{key}.upper"), up);
                set(&mut b, format!("{key}.lower"), low);
                set(&mut b, format!("{key}.D"), up - low);
                (Some(up), Some(low))
            }
            None => (None, None),
        };
        item_rows.push(vec![
            text(labels[i].clone()),
            text(key_text),
            num(p, 2),
            num(rpb, 2),
            num(rpb_c, 2),
            opt(up, 2),
            opt(low, 2),
            opt(up.zip(low).map(|(a, z)| a - z), 2),
            num(a_del, 3),
            blanks.map_or(Cell::Empty, count),
        ]);
    }
    b.tables.push(Table {
        id: "items".into(),
        title: "Item statistics".into(),
        columns: vec![
            "Item".into(),
            if letters.is_some() { "Key".into() } else { "Points".into() },
            "Difficulty (p)".into(),
            POINT_BISERIAL_LABEL.into(),
            CORRECTED_LABEL.into(),
            format!("Upper {pct}% p"),
            format!("Lower {pct}% p"),
            "D".into(),
            format!("{} if deleted", if right_or_wrong { "KR-20" } else { "α" }),
            "Blank".into(),
        ],
        rows: item_rows,
        footnotes: {
            let mut f = vec!["The point-biserial correlates the item with a total that includes it; the corrected item-total r uses the total of the other items, so it is lower and does not credit the item for correlating with itself. A correlation with an item everyone got right, or wrong, is undefined and left empty.".to_string()];
            if split.is_none() {
                f.push(format!("With {n} cases the upper and lower {pct} percent groups would have fewer than two cases each, so they are not formed."));
            }
            f
        },
    });

    subscores(&mut b, spec, labels, live, earned, points, right_or_wrong);

    // Options by score group, when there are answers to count.
    let mut upside_down = Vec::new();
    let mut unused = Vec::new();
    if let Some(l) = &letters {
        let mut opt_rows = Vec::new();
        let group_of: Vec<u8> = (0..n)
            .map(|r| match &split {
                Some((u, _)) if u.contains(&r) => 0,
                Some((_, lw)) if lw.contains(&r) => 2,
                _ => 1,
            })
            .collect();
        for (pos, &i) in live.iter().enumerate() {
            let mut options: Vec<String> = l.keys[i].clone();
            options.extend(l.used.iter().filter_map(|&r| l.answers[i][r].clone()));
            // Single letters widen the list to every letter up to the highest one seen, so an option nobody chose
            // still shows (as a distractor that does no work).
            let single: Vec<char> = options.iter().filter(|o| o.len() == 1 && o.chars().all(|c| c.is_ascii_uppercase())).filter_map(|o| o.chars().next()).collect();
            if let Some(&max) = single.iter().max() {
                options.extend(('A'..=max).map(String::from));
            }
            options.sort_by(|a, z| a.len().cmp(&z.len()).then(a.cmp(z)));
            options.dedup();
            let label = &labels[i];
            let item_key = format!("item.{}", pos + 1);
            for o in options.iter().map(Some).chain(std::iter::once(None)) {
                let mut c = [0usize; 3];
                let mut total = 0;
                for (r, &row) in l.used.iter().enumerate() {
                    if l.answers[i][row].as_ref() == o {
                        total += 1;
                        c[group_of[r] as usize] += 1;
                    }
                }
                if o.is_none() && total == 0 {
                    continue;
                }
                let right = o.is_some_and(|o| l.keys[i].contains(o));
                let credited = o.is_none() && adjustments[i] == Adjustment::CreditAll;
                let name = o.cloned().unwrap_or_else(|| if credited { "credited" } else { "blank" }.into());
                let fk = format!("{item_key}.opt.{name}");
                set(&mut b, format!("{fk}.n"), total as f64);
                if split.is_some() {
                    set(&mut b, format!("{fk}.upper"), c[0] as f64);
                    set(&mut b, format!("{fk}.middle"), c[1] as f64);
                    set(&mut b, format!("{fk}.lower"), c[2] as f64);
                    if o.is_some() && !right && c[0] > c[2] && adjustments[i] != Adjustment::CreditAll {
                        upside_down.push(format!("{label} option {name}"));
                    }
                }
                if o.is_some() && !right && total == 0 && adjustments[i] != Adjustment::CreditAll {
                    unused.push(format!("{label} option {name}"));
                }
                opt_rows.push(vec![
                    text(label.clone()),
                    text(if credited { "(no answer, credited)".to_string() } else if o.is_none() { "(blank)".to_string() } else if right { format!("{name} (key)") } else { name }),
                    count(total),
                    num(total as f64 / n as f64, 2),
                    if split.is_some() { count(c[0]) } else { Cell::Empty },
                    if split.is_some() { count(c[1]) } else { Cell::Empty },
                    if split.is_some() { count(c[2]) } else { Cell::Empty },
                ]);
            }
        }
        b.tables.push(Table {
            id: "options".into(),
            title: "Options by score group".into(),
            columns: vec!["Item".into(), "Option".into(), "n".into(), "Proportion".into(), format!("Upper {pct}%"), "Middle".into(), format!("Lower {pct}%")],
            rows: opt_rows,
            footnotes: vec!["A working distractor draws more of the lower group than of the upper group.".into()],
        });
    }

    // Diagnostics.
    b.diagnostics.push(if n < SMALL_GROUP {
        Diagnostic { id: "size".into(), title: "Number of cases".into(), state: DiagnosticState::ReviewNeeded, message: format!("Only {n} cases: fewer than {SMALL_GROUP}, so these item figures are unstable and should not be over-read.") }
    } else {
        Diagnostic { id: "size".into(), title: "Number of cases".into(), state: DiagnosticState::NoConcernDetected, message: format!("{n} cases were analyzed.") }
    });
    b.diagnostics.push(if negative.is_empty() && weak.is_empty() {
        Diagnostic { id: "discrimination".into(), title: "Discrimination".into(), state: DiagnosticState::NoConcernDetected, message: "Every item's corrected item-total r is .20 or more.".into() }
    } else {
        let mut parts = Vec::new();
        if !negative.is_empty() {
            parts.push(format!("{} {} negatively with the rest of the test, which often means a wrong key or a misleading question", apa::series(&negative), if negative.len() == 1 { "correlates" } else { "correlate" }));
        }
        if !weak.is_empty() {
            parts.push(format!("{} {} a corrected item-total r below .20", apa::series(&weak), if weak.len() == 1 { "has" } else { "have" }));
        }
        Diagnostic { id: "discrimination".into(), title: "Discrimination".into(), state: DiagnosticState::ReviewNeeded, message: format!("{}. Check the key and the wording before reusing these items.", parts.join("; ")) }
    });
    if letters.is_some() {
        if split.is_some() {
            b.diagnostics.push(if upside_down.is_empty() {
                Diagnostic { id: "distractors".into(), title: "Distractors".into(), state: DiagnosticState::NoConcernDetected, message: "No wrong option drew more of the upper group than of the lower group.".into() }
            } else {
                Diagnostic { id: "distractors".into(), title: "Distractors".into(), state: DiagnosticState::ReviewNeeded, message: format!("More of the upper group than of the lower group chose {}. A wrong option the strongest students pick may be arguably right, or the key may be wrong.", apa::series(&upside_down)) }
            });
        } else {
            b.diagnostics.push(not_assessed("distractors", "Distractors", "There are too few cases to form upper and lower groups, so the distractors were not compared by group."));
        }
        if !unused.is_empty() {
            b.diagnostics.push(Diagnostic { id: "unused".into(), title: "Options nobody chose".into(), state: DiagnosticState::ReviewNeeded, message: format!("Nobody chose {}. An option nobody picks adds nothing to the item.", apa::series(&unused)) });
        }
    } else {
        b.diagnostics.push(not_assessed("distractors", "Distractors", "The items hold points, not the answers marked, so the options can't be counted. Score the letter answers here to see them."));
    }
    b.diagnostics.push(not_assessed("dimensionality", "One dimension", "KR-20 and the item-total correlations assume the test measures one thing. A high KR-20 does not show that; check the items' content or a factor analysis."));
    b
}

/// The column labels, as the C3 test set asks each tool to word them.
pub const POINT_BISERIAL_LABEL: &str = "Point-biserial (total includes the question)";
pub const CORRECTED_LABEL: &str = "Corrected item-total r (total leaves the item out)";

/// A subscore for each group of items (`spec.groups`): the points earned on the group's items that count.
fn subscores(b: &mut ResultBundle, spec: &Spec, labels: &[String], live: &[usize], earned: &[Vec<f64>], points: &[f64], right_or_wrong: bool) {
    if spec.groups.is_empty() {
        return;
    }
    let what = if spec.group_name.trim().is_empty() { "Group".to_string() } else { spec.group_name.trim().to_string() };
    let mut names: Vec<String> = Vec::new();
    for &i in live {
        let g = spec.groups[i].trim();
        if !g.is_empty() && !names.iter().any(|n| n == g) {
            names.push(g.to_string());
        }
    }
    if names.is_empty() {
        return;
    }
    group_order(&mut names);
    let n = earned[live[0]].len();
    let reliability_name = if right_or_wrong { "KR-20" } else { "Cronbach's α" };
    let mut rows = Vec::new();
    let mut single = Vec::new();
    for (j, name) in names.iter().enumerate() {
        let members: Vec<usize> = live.iter().copied().filter(|&i| spec.groups[i].trim() == name).collect();
        let cols: Vec<&Vec<f64>> = members.iter().map(|&i| &earned[i]).collect();
        let sub: Vec<f64> = (0..n).map(|r| cols.iter().map(|c| c[r]).sum()).collect();
        let possible: f64 = members.iter().map(|&i| points[i]).sum();
        let m = mean(&sub);
        let sd = (var_n(&sub) * n as f64 / (n as f64 - 1.0)).sqrt();
        let rel = if members.len() >= 2 { alpha(&cols) } else { f64::NAN };
        if members.len() < 2 {
            single.push(name.clone());
        }
        let key = format!("subscore.{}", j + 1);
        set_text(b, format!("{key}.name"), name.clone());
        set_text(b, format!("{key}.items"), members.iter().map(|&i| labels[i].clone()).collect::<Vec<_>>().join("\u{1f}"));
        for (k, v) in [("k", members.len() as f64), ("possible", possible), ("mean", m), ("sd", sd), ("mean.p", m / possible), ("reliability", rel)] {
            if v.is_finite() {
                set(b, format!("{key}.{k}"), v);
            }
        }
        rows.push(vec![text(name.clone()), count(members.len()), num(possible, 2), num(m, 2), num(sd, 2), num(m / possible, 2), if rel.is_finite() { num(rel, 3) } else { Cell::Empty }]);
    }
    set(b, "subscores", names.len() as f64);
    let lower = what.to_lowercase();
    let ungrouped: Vec<String> = live.iter().filter(|&&i| spec.groups[i].trim().is_empty()).map(|&i| labels[i].clone()).collect();
    let mut footnotes = vec![format!("Each subscore is the points earned on the items of that {lower}. The share of points is the subscore's mean divided by its points possible; reliability is {reliability_name} of the group's items, computed as for the whole test.")];
    if !single.is_empty() {
        footnotes.push(format!("{} {} only one item, so no reliability is given for {}.", apa::series(&single), if single.len() == 1 { "has" } else { "have" }, if single.len() == 1 { "it" } else { "them" }));
    }
    if !ungrouped.is_empty() {
        footnotes.push(format!("{} {} no {lower} and {} counted in the whole test only.", apa::series(&ungrouped), if ungrouped.len() == 1 { "has" } else { "have" }, if ungrouped.len() == 1 { "is" } else { "are" }));
    }
    footnotes.push("A subscore made of few items is less reliable than the whole test; read small differences between groups with care.".into());
    b.tables.push(Table {
        id: "subscores".into(),
        title: format!("Subscores by {lower}"),
        columns: vec![what, "Items".into(), "Points possible".into(), "Mean".into(), "SD".into(), "Share of points".into(), reliability_name.into()],
        rows,
        footnotes,
    });
}

