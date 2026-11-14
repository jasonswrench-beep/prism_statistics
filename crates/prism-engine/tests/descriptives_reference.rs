//! Frequencies and Descriptives against R (reference/descriptives_reference.json), and the participants paragraph.

use std::path::PathBuf;

use prism_engine::analysis::descriptives::{self, Statistic};
use prism_engine::analysis::frequencies;
use prism_engine::apa;
use prism_engine::import::read_file;
use prism_engine::model::ValueLabel;
use prism_engine::Dataset;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("reference")
}

fn reference() -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir().join("descriptives_reference.json")).unwrap()).unwrap()
}

fn dataset() -> Dataset {
    let (mut ds, _) = read_file(&dir().join("demographics.csv")).unwrap();
    let sex = ds.variable_mut("sex").unwrap();
    sex.label = "sex".into();
    sex.value_labels = vec![
        ValueLabel { value: 1.0, label: "female".into() },
        ValueLabel { value: 2.0, label: "male".into() },
        ValueLabel { value: 3.0, label: "another identity".into() },
    ];
    let age = ds.variable_mut("age").unwrap();
    age.label = "age".into();
    age.missing_values = vec![-99.0];
    let eth = ds.variable_mut("ethnicity").unwrap();
    eth.missing_values = vec![-9.0];
    eth.value_labels = vec![ValueLabel { value: -9.0, label: "Refused".into() }];
    ds
}

fn close(got: f64, want: &serde_json::Value) {
    let want = want.as_f64().unwrap();
    assert!((got - want).abs() <= 1e-10 * want.abs().max(1.0), "got {got}, R gives {want}");
}

#[test]
fn descriptives_match_r() {
    let r = reference();
    let spec = descriptives::Spec {
        variables: vec!["age".into()],
        statistics: vec![Statistic::Mean, Statistic::Sd, Statistic::Min, Statistic::Max, Statistic::Median, Statistic::Se, Statistic::CiMean, Statistic::Range],
        level: 0.95,
    };
    let b = descriptives::run(&dataset(), &spec).unwrap();
    close(b.number("age.n").unwrap(), &r["age_n"]);
    close(b.number("age.mean").unwrap(), &r["age_mean"]);
    close(b.number("age.sd").unwrap(), &r["age_sd"]);
    close(b.number("age.min").unwrap(), &r["age_min"]);
    close(b.number("age.max").unwrap(), &r["age_max"]);
    close(b.number("age.median").unwrap(), &r["age_median"]);
    close(b.number("age.se").unwrap(), &r["age_se"]);
    close(b.number("age.ci.lower").unwrap(), &r["age_ci_lo"]);
    close(b.number("age.ci.upper").unwrap(), &r["age_ci_hi"]);
    assert_eq!(b.number("age.missing"), Some(2.0));
}

#[test]
fn frequencies_count_like_r_and_keep_missing_apart() {
    let r = reference();
    let ds = dataset();
    let out = frequencies::run(&ds, &frequencies::Spec { variables: vec!["sex".into(), "ethnicity".into()] }).unwrap();
    let sex = &out[0];
    close(sex.number("cat.1.n").unwrap(), &r["sex_1"]);
    close(sex.number("cat.2.n").unwrap(), &r["sex_2"]);
    close(sex.number("cat.3.n").unwrap(), &r["sex_3"]);
    close(sex.number("n.missing").unwrap(), &r["sex_empty"]);
    let eth = &out[1];
    close(eth.number("n.missing").unwrap(), &r["eth_missing"]);
    // The user-missing code is listed as missing, never as a category.
    assert!(eth.number("cat.-9.n").is_none());
    let rows: Vec<String> = eth.tables[0].rows.iter().map(|r| format!("{:?}", r[0])).collect();
    assert!(rows.iter().any(|r| r.contains("Missing: Refused (-9)")), "{rows:?}");
}

#[test]
fn participants_paragraph_reports_what_was_run() {
    let ds = dataset();
    let freq = frequencies::run(&ds, &frequencies::Spec { variables: vec!["sex".into()] }).unwrap();
    let spec = descriptives::Spec { variables: vec!["age".into()], statistics: vec![Statistic::Mean, Statistic::Sd, Statistic::Min, Statistic::Max, Statistic::Range, Statistic::Median], level: 0.95 };
    let desc = descriptives::run(&ds, &spec).unwrap();
    let text = apa::participants_paragraph(&[&freq[0]], Some(&desc)).unwrap();
    assert_eq!(
        text,
        "The sample included 12 participants. For sex, 50.0% female (*n* = 6), 33.3% male (*n* = 4), 8.3% another identity (*n* = 1), and 8.3% did not respond (*n* = 1). Of the 12 participants, 10 reported age. The mean age was 22.50 (*SD* = 4.12). The lowest age was 18.00 and the highest was 31.00, a range of 13.00. The median age was 21.50."
    );
    // Only what was run is mentioned.
    let spec = descriptives::Spec { variables: vec!["age".into()], statistics: vec![Statistic::Mean], level: 0.95 };
    let desc = descriptives::run(&ds, &spec).unwrap();
    let text = apa::participants_paragraph(&[], Some(&desc)).unwrap();
    assert!(!text.contains("SD") && !text.contains("median") && !text.contains("lowest"), "{text}");
}

#[test]
fn category_percentages_add_to_100() {
    let ds = dataset();
    let freq = frequencies::run(&ds, &frequencies::Spec { variables: vec!["sex".into()] }).unwrap();
    let b = &freq[0];
    let sum: f64 = ["1", "2", "3"].iter().map(|k| b.number(&format!("cat.{k}.pct")).unwrap()).sum::<f64>() + 100.0 * b.number("n.missing").unwrap() / b.number("n.total").unwrap();
    assert!((sum - 100.0).abs() < 1e-9);
}
