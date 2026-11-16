//! xAPI statements (development list item 7): a JSON file of statements, as a learning record store exports them,
//! opened as data with one case per statement.
//!
//! Accepted: a JSON array of statements, an object with a `statements` array (an LRS query result), or one statement
//! per line (JSON Lines). Each statement needs `actor`, `verb` and `object`. The variables:
//!
//! - `learner`: who it was, as "Learner 1", "Learner 2", … in order of first appearance. The actor's name, email,
//!   account or other identifier is never read into the data; the same actor always gets the same number within the
//!   file, so statements can be grouped by learner (Data › Aggregate).
//! - `verb` and `verb_id`: the verb's English display name (or the last part of its IRI) and its IRI.
//! - `activity` and `activity_id`: the object's English name (or its id) and its id.
//! - `timestamp`: as written.
//! - `score_scaled`, `score_raw`, `score_min`, `score_max`, `success` (1 yes, 0 no), `completion` (1, 0) and
//!   `duration_s` (seconds, from the ISO 8601 duration): from `result`, empty where a statement has none.
//! - `registration`: the context's registration id, which ties the statements of one attempt together.

use serde_json::Value;

use crate::error::{Error, Result};
use crate::import::ImportReport;
use crate::model::{Column, Dataset, Measure, ValueLabel};

/// Whether parsed JSON looks like xAPI statements (so a .json file is read as them rather than refused).
pub fn statements(v: &Value) -> Option<Vec<&Value>> {
    let list: Vec<&Value> = match v {
        Value::Array(a) => a.iter().collect(),
        Value::Object(o) => match o.get("statements") {
            Some(Value::Array(a)) => a.iter().collect(),
            _ if o.contains_key("actor") && o.contains_key("verb") => vec![v],
            _ => return None,
        },
        _ => return None,
    };
    let is_statement = |s: &&Value| s.get("actor").is_some() && s.get("verb").is_some() && s.get("object").is_some();
    (!list.is_empty() && list.iter().all(is_statement)).then_some(list)
}

/// Parses JSON, a JSON array or JSON Lines.
pub fn parse(bytes: &[u8]) -> Option<Value> {
    let text = std::str::from_utf8(bytes).ok()?.trim_start_matches('\u{feff}');
    if let Ok(v) = serde_json::from_str::<Value>(text) {
        return Some(v);
    }
    let lines: Vec<Value> = text.lines().filter(|l| !l.trim().is_empty()).map(serde_json::from_str).collect::<std::result::Result<_, _>>().ok()?;
    Some(Value::Array(lines))
}

/// A language map's English entry (en-US, then any en-*, then the first).
fn english(map: Option<&Value>) -> Option<String> {
    let o = map?.as_object()?;
    o.get("en-US").or_else(|| o.iter().find(|(k, _)| k.starts_with("en")).map(|(_, v)| v)).or_else(|| o.values().next()).and_then(Value::as_str).map(str::to_string)
}

/// The identity an actor is grouped by: its inverse functional identifier, never shown.
fn actor_key(a: &Value) -> String {
    for k in ["mbox", "mbox_sha1sum", "openid"] {
        if let Some(s) = a.get(k).and_then(Value::as_str) {
            return format!("{k}:{s}");
        }
    }
    if let Some(acc) = a.get("account") {
        return format!("account:{}|{}", acc.get("homePage").and_then(Value::as_str).unwrap_or(""), acc.get("name").and_then(Value::as_str).unwrap_or(""));
    }
    // A group or an anonymous agent: its members, or the whole object.
    serde_json::to_string(a).unwrap_or_default()
}

/// Seconds in an ISO 8601 duration such as PT1H2M3.5S or P1DT2H.
pub fn duration_seconds(s: &str) -> Option<f64> {
    let s = s.strip_prefix('P')?;
    let (date, time) = s.split_once('T').unwrap_or((s, ""));
    let mut total = 0.0;
    let mut take = |part: &str, units: &[(char, f64)]| -> Option<()> {
        let mut num = String::new();
        for c in part.chars() {
            if c.is_ascii_digit() || c == '.' || c == ',' {
                num.push(if c == ',' { '.' } else { c });
            } else {
                let (_, mult) = units.iter().find(|(u, _)| *u == c)?;
                total += num.parse::<f64>().ok()? * mult;
                num.clear();
            }
        }
        num.is_empty().then_some(())
    };
    take(date, &[('Y', 365.25 * 86400.0), ('M', 30.4375 * 86400.0), ('W', 7.0 * 86400.0), ('D', 86400.0)])?;
    take(time, &[('H', 3600.0), ('M', 60.0), ('S', 1.0)])?;
    Some(total)
}

pub fn read(list: &[&Value], name: &str) -> Result<(Dataset, ImportReport)> {
    let mut keys: Vec<String> = Vec::new();
    let mut learner = Vec::new();
    let (mut verb, mut verb_id, mut act, mut act_id, mut stamp, mut reg) = (vec![], vec![], vec![], vec![], vec![], vec![]);
    let (mut scaled, mut raw, mut min, mut max, mut success, mut completion, mut duration) = (vec![], vec![], vec![], vec![], vec![], vec![], vec![]);
    let mut bad_duration = 0;
    for s in list {
        let k = actor_key(&s["actor"]);
        let i = match keys.iter().position(|x| *x == k) {
            Some(i) => i,
            None => {
                keys.push(k);
                keys.len() - 1
            }
        };
        learner.push(Some(format!("Learner {}", i + 1)));
        let vid = s["verb"].get("id").and_then(Value::as_str).map(str::to_string);
        verb.push(english(s["verb"].get("display")).or_else(|| vid.as_deref().and_then(|v| v.rsplit(['/', '#']).next()).map(str::to_string)));
        verb_id.push(vid);
        let oid = s["object"].get("id").and_then(Value::as_str).map(str::to_string);
        act.push(english(s["object"].get("definition").and_then(|d| d.get("name"))).or_else(|| oid.clone()));
        act_id.push(oid);
        stamp.push(s.get("timestamp").and_then(Value::as_str).map(str::to_string));
        reg.push(s.get("context").and_then(|c| c.get("registration")).and_then(Value::as_str).map(str::to_string));
        let r = s.get("result");
        let score = |f: &str| r.and_then(|r| r.get("score")).and_then(|sc| sc.get(f)).and_then(Value::as_f64);
        scaled.push(score("scaled"));
        raw.push(score("raw"));
        min.push(score("min"));
        max.push(score("max"));
        let flag = |f: &str| r.and_then(|r| r.get(f)).and_then(Value::as_bool).map(|b| if b { 1.0 } else { 0.0 });
        success.push(flag("success"));
        completion.push(flag("completion"));
        let d = r.and_then(|r| r.get("duration")).and_then(Value::as_str);
        let secs = d.and_then(duration_seconds);
        if d.is_some() && secs.is_none() {
            bad_duration += 1;
        }
        duration.push(secs);
    }
    let mut ds = Dataset::new(name);
    let text_vars = [("learner", "Learner (anonymous)", learner), ("verb", "Verb", verb), ("verb_id", "Verb IRI", verb_id), ("activity", "Activity", act), ("activity_id", "Activity id", act_id), ("timestamp", "Timestamp", stamp), ("registration", "Registration", reg)];
    for (n, l, v) in text_vars {
        ds.add_variable(n, Column::Text(v))?;
        ds.variable_mut(n)?.label = l.into();
    }
    let yes_no = vec![ValueLabel { value: 0.0, label: "No".into() }, ValueLabel { value: 1.0, label: "Yes".into() }];
    let num_vars = [("score_scaled", "Score (scaled, -1 to 1)", scaled), ("score_raw", "Score (raw)", raw), ("score_min", "Lowest possible score", min), ("score_max", "Highest possible score", max), ("success", "Success", success), ("completion", "Completed", completion), ("duration_s", "Duration (seconds)", duration)];
    for (n, l, v) in num_vars {
        ds.add_variable(n, Column::Numeric(v))?;
        let var = ds.variable_mut(n)?;
        var.label = l.into();
        if n == "success" || n == "completion" {
            var.measure = Measure::Nominal;
            var.value_labels = yes_no.clone();
        }
    }
    ds.variable_mut("learner")?.measure = Measure::Nominal;
    let mut notes = vec![format!(
        "Read {} xAPI statements from {} learners. Learners are numbered in the order they first appear; their names, emails and account ids were not read.",
        list.len(),
        keys.len()
    )];
    if bad_duration > 0 {
        notes.push(format!("{bad_duration} durations were not ISO 8601 durations and were left empty."));
    }
    if list.is_empty() {
        return Err(Error::Invalid("The file holds no xAPI statements.".into()));
    }
    let report = ImportReport { rows: list.len(), variables: ds.variables.len(), notes, ..Default::default() };
    Ok((ds, report))
}
