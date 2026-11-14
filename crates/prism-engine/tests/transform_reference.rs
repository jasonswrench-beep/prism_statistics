//! Sort Cases, Rank Cases, Standardize, Reverse Score and Date and Time against R (reference/transform_reference.json,
//! made by make_transform_reference.R from preview.csv). The data have user-missing codes, which R gets as NA.

use std::path::PathBuf;

use prism_engine::import::read_file;
use prism_engine::model::Column;
use prism_engine::transform::*;
use prism_engine::Dataset;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("reference")
}

fn r(key: &str) -> Vec<Option<f64>> {
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir().join("transform_reference.json")).unwrap()).unwrap();
    v[key].as_array().unwrap_or_else(|| panic!("no {key}")).iter().map(|x| x.as_f64()).collect()
}

fn preview() -> Dataset {
    let (mut ds, _) = read_file(&dir().join("preview.csv")).unwrap();
    ds.variable_mut("cond").unwrap().missing_values = vec![9.0];
    ds.variable_mut("pre").unwrap().missing_values = vec![-99.0];
    ds
}

fn col(ds: &Dataset, name: &str) -> Vec<Option<f64>> {
    match &ds.columns[ds.find(name).unwrap()] {
        Column::Numeric(v) => v.clone(),
        Column::Text(_) => panic!("{name} is text"),
    }
}

fn close(ours: &[Option<f64>], theirs: &[Option<f64>], what: &str) {
    assert_eq!(ours.len(), theirs.len(), "{what}");
    for (i, (a, b)) in ours.iter().zip(theirs).enumerate() {
        match (a, b) {
            (Some(a), Some(b)) => assert!((a - b).abs() <= 1e-9 * b.abs().max(1.0), "{what}[{i}]: {a} vs R {b}"),
            (None, None) => {}
            _ => panic!("{what}[{i}]: {a:?} vs R {b:?}"),
        }
    }
}

#[test]
fn ranks_match_r() {
    let mut ds = preview();
    for (ties, key) in [(Ties::Mean, "average"), (Ties::Low, "min"), (Ties::High, "max"), (Ties::Sequential, "first")] {
        let name = format!("r_{key}");
        rank_cases(&mut ds, &RankSpec { variables: vec!["score".into()], ties, descending: false, by: None, into: vec![name.clone()] }).unwrap();
        close(&col(&ds, &name), &r(&format!("rank.{key}")), key);
    }
    rank_cases(&mut ds, &RankSpec { variables: vec!["hours".into()], ties: Ties::Mean, descending: true, by: None, into: vec![] }).unwrap();
    close(&col(&ds, "r_hours"), &r("rank.desc"), "descending");
    rank_cases(&mut ds, &RankSpec { variables: vec!["pre".into()], ties: Ties::Low, descending: false, by: Some("cond".into()), into: vec!["r_pre_cond".into()] }).unwrap();
    close(&col(&ds, "r_pre_cond"), &r("rank.pre.by.cond"), "within groups");
}

#[test]
fn z_scores_and_reverse_scores_match_r() {
    let mut ds = preview();
    standardize(&mut ds, &StandardizeSpec { variables: vec!["pre".into(), "score".into()], into: vec![] }).unwrap();
    close(&col(&ds, "z_pre"), &r("z.pre"), "z pre");
    close(&col(&ds, "z_score"), &r("z.score"), "z score");
    reverse_score(&mut ds, &ReverseSpec { variables: vec!["cond".into()], min: 1.0, max: 3.0, into: vec![], replace: false }).unwrap();
    close(&col(&ds, "cond_r"), &r("rev.cond"), "reverse");
}

#[test]
fn sorting_matches_r() {
    let mut ds = preview();
    sort_cases(&mut ds, &[SortKey { name: "cond".into(), descending: false }, SortKey { name: "score".into(), descending: true }]).unwrap();
    close(&col(&ds, "id"), &r("sort.cond.score"), "by cond then score");
    let mut ds = preview();
    sort_cases(&mut ds, &[SortKey { name: "region".into(), descending: false }]).unwrap();
    close(&col(&ds, "id"), &r("sort.region"), "by region");
}

#[test]
fn dates_match_r() {
    let text = |v: &[&str]| Column::Text(v.iter().map(|s| Some(s.to_string())).collect());
    let mut ds = Dataset::new("dates");
    ds.add_variable("start", text(&["2000-02-29", "1999-12-31 23:30", "2024-03-15 13:45", "2026-11-14", "1970-01-01"])).unwrap();
    ds.add_variable("end", text(&["2026-02-28", "2026-12-30", "2024-03-16 09:15", "2027-11-13", "2027-01-01"])).unwrap();
    let parts = [(DatePart::Year, "year", "year"), (DatePart::Month, "month", "month"), (DatePart::Day, "day", "day"), (DatePart::Weekday, "weekday", "weekday"), (DatePart::DayOfYear, "yday", "yday"), (DatePart::Hour, "hour", "hour"), (DatePart::Minute, "minute", "minute")];
    date_parts(&mut ds, &DatePartsSpec { variable: "start".into(), source: DateSource::TextYmd, parts: parts.iter().map(|p| p.0).collect() }).unwrap();
    for (_, ours, theirs) in parts {
        close(&col(&ds, &format!("start_{ours}")), &r(&format!("date.{theirs}")), theirs);
    }
    for (unit, key) in [(Unit::Days, "days"), (Unit::Months, "months"), (Unit::Years, "years")] {
        date_difference(&mut ds, &DateDiffSpec { start: "start".into(), end: "end".into(), source: DateSource::TextYmd, unit, target: key.into(), label: None }).unwrap();
        close(&col(&ds, key), &r(&format!("date.{key}")), key);
    }
}
