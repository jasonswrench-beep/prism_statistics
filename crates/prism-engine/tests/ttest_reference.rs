//! The independent-samples t test against R (reference/ttest_reference.json, made by make_ttest_reference.R).
//!
//! The data include every kind of case the test must leave out: an empty grouping cell, a user-missing group code,
//! a third group, an empty score and a user-missing score. R gets the same exclusions written out by hand, so the test
//! checks the case handling as well as the numbers.

use std::path::PathBuf;

use prism_engine::analysis::ttest_independent::{run, Estimator, Spec};
use prism_engine::apa;
use prism_engine::import::read_file;
use prism_engine::model::ValueLabel;
use prism_engine::result::{DiagnosticState, Status};
use prism_engine::Dataset;

fn reference_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("reference")
}

fn reference() -> serde_json::Value {
    let text = std::fs::read_to_string(reference_dir().join("ttest_reference.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn dataset() -> Dataset {
    let (mut ds, _) = read_file(&reference_dir().join("two_groups.csv")).unwrap();
    let site = ds.variable_mut("site").unwrap();
    site.label = "Site".into();
    site.value_labels = vec![
        ValueLabel { value: 1.0, label: "Urban".into() },
        ValueLabel { value: 2.0, label: "Rural".into() },
        ValueLabel { value: 3.0, label: "Online".into() },
    ];
    site.missing_values = vec![9.0];
    let stress = ds.variable_mut("stress").unwrap();
    stress.label = "stress score".into();
    stress.missing_values = vec![-99.0];
    ds
}

fn spec(estimator: Estimator, level: f64) -> Spec {
    Spec { test_variables: vec!["stress".into()], grouping_variable: "site".into(), groups: (1.0, 2.0), estimator, level, alternative: Default::default(), bayes: Default::default(), prior_scale: Default::default() }
}

#[track_caller]
fn close(got: f64, want: &serde_json::Value, tol: f64) {
    let want = want.as_f64().unwrap();
    let err = (got - want).abs() / want.abs().max(1.0);
    assert!(err <= tol, "got {got}, R gives {want} (relative error {err:e})");
}

#[test]
fn welch_matches_r() {
    let r = reference();
    let out = run(&dataset(), &spec(Estimator::Welch, 0.95)).unwrap();
    assert_eq!(out.len(), 1);
    let b = &out[0];
    assert_eq!(b.status, Status::Ok);
    assert_eq!(b.cases.total as f64, r["n_total"].as_f64().unwrap());
    close(b.number("g1.n").unwrap(), &r["n1"], 0.0);
    close(b.number("g2.n").unwrap(), &r["n2"], 0.0);
    close(b.number("g1.mean").unwrap(), &r["mean1"], 1e-12);
    close(b.number("g1.sd").unwrap(), &r["sd1"], 1e-12);
    close(b.number("g2.mean").unwrap(), &r["mean2"], 1e-12);
    close(b.number("g2.sd").unwrap(), &r["sd2"], 1e-12);
    close(b.number("t").unwrap(), &r["welch_t"], 1e-10);
    close(b.number("df").unwrap(), &r["welch_df"], 1e-10);
    close(b.number("ci.lower").unwrap(), &r["welch_lo"], 1e-9);
    close(b.number("ci.upper").unwrap(), &r["welch_hi"], 1e-9);
    let p = b.number("p").unwrap();
    let rp = r["welch_p"].as_f64().unwrap();
    assert!((p - rp).abs() / rp < 1e-7, "p {p} vs R {rp}");
    close(b.number("d").unwrap(), &r["d"], 1e-10);
    close(b.number("g").unwrap(), &r["g"], 1e-10);
}

#[test]
fn student_at_90_percent_matches_r() {
    let r = reference();
    let b = &run(&dataset(), &spec(Estimator::Student, 0.90)).unwrap()[0];
    close(b.number("t").unwrap(), &r["student_t"], 1e-10);
    close(b.number("df").unwrap(), &r["student_df"], 0.0);
    close(b.number("ci.lower").unwrap(), &r["student90_lo"], 1e-9);
    close(b.number("ci.upper").unwrap(), &r["student90_hi"], 1e-9);
    let p = b.number("p").unwrap();
    let rp = r["student_p"].as_f64().unwrap();
    assert!((p - rp).abs() / rp < 1e-7, "p {p} vs R {rp}");
}

#[test]
fn every_excluded_case_has_one_reason() {
    let b = &run(&dataset(), &spec(Estimator::Welch, 0.95)).unwrap()[0];
    let excluded: usize = b.cases.excluded.iter().map(|e| e.count).sum();
    assert_eq!(b.cases.analyzed + excluded, b.cases.total);
    let reasons: Vec<&str> = b.cases.excluded.iter().map(|e| e.reason.as_str()).collect();
    for want in ["grouping variable empty", "grouping variable user-missing", "in neither compared group", "test variable empty", "test variable user-missing"] {
        assert!(reasons.contains(&want), "missing reason {want}: {reasons:?}");
    }
}

#[test]
fn swapping_groups_flips_the_sign_only() {
    let a = &run(&dataset(), &spec(Estimator::Welch, 0.95)).unwrap()[0];
    let mut s = spec(Estimator::Welch, 0.95);
    s.groups = (2.0, 1.0);
    let b = &run(&dataset(), &s).unwrap()[0];
    assert!((a.number("t").unwrap() + b.number("t").unwrap()).abs() < 1e-12);
    assert!((a.number("p").unwrap() - b.number("p").unwrap()).abs() < 1e-15);
    assert_eq!(b.text("g1.label"), Some("Rural"));
}

#[test]
fn template_paragraph_reads_correctly() {
    let b = &run(&dataset(), &spec(Estimator::Welch, 0.95)).unwrap()[0];
    let text = prism_engine::templates::paragraph(b).unwrap();
    assert!(text.starts_with("An independent-samples *t* test (Welch's unequal-variances t test) compared stress score between the Urban and Rural groups."), "{text}");
    assert!(text.contains("Stress score was higher in the Urban group (*M* = 5.91, *SD* = 1.20) than in the Rural group (*M* = 5.03, *SD* = 1.62)"), "{text}");
    assert!(text.contains("*t*(205.23) = 4.54, *p* < .001"), "{text}");
    assert_eq!(text.matches("In total, 47 cases were excluded").count(), 1, "said once: {text}");
}

#[test]
fn too_few_cases_fails_without_numbers() {
    let mut ds = Dataset::new("tiny");
    ds.add_variable("g", prism_engine::Column::Numeric(vec![Some(1.0), Some(2.0), Some(2.0)])).unwrap();
    ds.add_variable("y", prism_engine::Column::Numeric(vec![Some(3.0), Some(4.0), Some(5.0)])).unwrap();
    let s = Spec { test_variables: vec!["y".into()], grouping_variable: "g".into(), groups: (1.0, 2.0), estimator: Estimator::Welch, level: 0.95, alternative: Default::default(), bayes: Default::default(), prior_scale: Default::default() };
    let b = &run(&ds, &s).unwrap()[0];
    assert!(matches!(b.status, Status::Failed { .. }));
    assert!(b.number("t").is_none());
    assert!(apa::independent_t_paragraph(b).is_none());
}

#[test]
fn small_groups_are_flagged() {
    let mut ds = Dataset::new("small");
    let g: Vec<_> = (0..12).map(|i| Some(if i < 6 { 1.0 } else { 2.0 })).collect();
    let y: Vec<_> = (0..12).map(|i| Some(i as f64 * 1.3 % 5.0)).collect();
    ds.add_variable("g", prism_engine::Column::Numeric(g)).unwrap();
    ds.add_variable("y", prism_engine::Column::Numeric(y)).unwrap();
    let s = Spec { test_variables: vec!["y".into()], grouping_variable: "g".into(), groups: (1.0, 2.0), estimator: Estimator::Welch, level: 0.95, alternative: Default::default(), bayes: Default::default(), prior_scale: Default::default() };
    let b = &run(&ds, &s).unwrap()[0];
    let size = b.diagnostics.iter().find(|d| d.id == "group_size").unwrap();
    assert_eq!(size.state, DiagnosticState::ReviewNeeded);
}
