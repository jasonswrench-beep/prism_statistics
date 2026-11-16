//! The training evaluation report by Kirkpatrick level against R (reference/training_reference.json, made by
//! make_training_reference.R with base R's t.test from the Training evaluation example data), and xAPI statements
//! read as data.

use std::path::PathBuf;

use prism_engine::analysis::kirkpatrick::{self, Grouped, Pair, Spec};
use prism_engine::import::{read_bytes, read_file};
use prism_engine::model::Column;
use prism_engine::{Dataset, ResultBundle};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn r() -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_str(&std::fs::read_to_string(root().join("tests/reference/training_reference.json")).unwrap()).unwrap()
}

fn data() -> Dataset {
    read_file(&root().join("../prism-api/data/examples/training_evaluation.csv")).unwrap().0
}

fn pair(a: &str, b: &str) -> Pair {
    Pair { pre: a.into(), post: b.into() }
}

fn check(b: &ResultBundle, keys: impl Fn(&str) -> bool) -> usize {
    let mut failures = Vec::new();
    let mut n = 0;
    for (key, want) in r().iter().filter(|(k, _)| !k.starts_with('_') && keys(k)) {
        let want = want.as_f64().unwrap();
        n += 1;
        match b.number(key) {
            Some(got) if (got - want).abs() <= 1e-9 * want.abs().max(1.0) => {}
            got => failures.push(format!("{key}: got {got:?}, R gives {want}")),
        }
    }
    assert!(failures.is_empty(), "{} of {n} differ:\n{}", failures.len(), failures.join("\n"));
    n
}

#[test]
fn every_level_matches_r() {
    let ds = data();
    let spec = Spec {
        reaction: (1..=4).map(|i| format!("react_{i}")).collect(),
        reaction_max: Some(5.0),
        learning: vec![pair("pre_test", "post_test")],
        learning_max: Some(20.0),
        behavior: vec![pair("behav_pre", "behav_post")],
        results: vec![pair("errors_pre", "errors_post")],
        results_items: vec!["errors_post".into()],
        ..Default::default()
    };
    let spec = Spec { level: 0.95, ..spec };
    let b = kirkpatrick::run(&ds, &spec).unwrap();
    assert!(b.is_ok());
    let n = check(&b, |k| !k.starts_with("learning.groups"));
    assert!(n > 50, "only {n}");
    // Behavior has six cases fewer than the others: each level uses its own complete cases.
    assert_eq!(b.number("behavior.1.n"), Some(54.0));
    assert_eq!(b.number("learning.1.n"), Some(60.0));
    assert!(b.diagnostics.iter().any(|d| d.id == "design" && d.message.contains("comparison group")));
    assert_eq!(b.tables[0].id, "summary");
    let p = prism_engine::templates::paragraph(&b).unwrap();
    assert!(p.contains("Kirkpatrick") && p.contains("normalized gain"), "{p}");
}

#[test]
fn learning_from_two_unlinked_groups_matches_welch() {
    // Everyone's before and after scores as two groups of cases, as two classes' results files added together.
    let wide = data();
    let col = |name: &str| match &wide.columns[wide.find(name).unwrap()] {
        Column::Numeric(v) => v.clone(),
        _ => unreachable!(),
    };
    let (pre, post) = (col("pre_test"), col("post_test"));
    let mut ds = Dataset::new("long");
    ds.add_variable("score", Column::Numeric(pre.iter().chain(&post).copied().collect())).unwrap();
    ds.add_variable("time", Column::Numeric(pre.iter().map(|_| Some(1.0)).chain(post.iter().map(|_| Some(2.0))).collect())).unwrap();
    let spec = Spec { learning_groups: Some(Grouped { score: "score".into(), time: "time".into(), pre: "1".into(), post: "2".into() }), learning_max: Some(20.0), level: 0.95, ..Default::default() };
    let b = kirkpatrick::run(&ds, &spec).unwrap();
    assert!(check(&b, |k| k.starts_with("learning.groups")) >= 10);
    // Levels with nothing chosen are named as not covered.
    assert!(b.diagnostics.iter().any(|d| d.id == "levels" && d.message.contains("1, 3")), "{:?}", b.diagnostics);
}

#[test]
fn nothing_chosen_is_refused_in_words() {
    let e = kirkpatrick::run(&data(), &Spec { level: 0.95, ..Default::default() }).unwrap_err();
    assert!(e.to_string().contains("at least one level"), "{e}");
}

#[test]
fn xapi_statements_open_as_data_without_identities() {
    let json = r#"{"statements": [
      {"actor": {"mbox": "mailto:ana@example.org", "name": "Ana"}, "verb": {"id": "http://adlnet.gov/expapi/verbs/passed", "display": {"en-US": "passed"}},
       "object": {"id": "https://example.org/course/quiz1", "definition": {"name": {"en-US": "Quiz 1"}}},
       "result": {"score": {"scaled": 0.85, "raw": 17, "min": 0, "max": 20}, "success": true, "completion": true, "duration": "PT12M30S"},
       "timestamp": "2026-11-01T10:00:00Z", "context": {"registration": "6c3a1d9e-0000-4000-8000-000000000001"}},
      {"actor": {"account": {"homePage": "https://lms.example.org", "name": "u42"}}, "verb": {"id": "http://adlnet.gov/expapi/verbs/failed"},
       "object": {"id": "https://example.org/course/quiz1"}, "result": {"score": {"raw": 9}, "success": false, "duration": "P1DT1H"}},
      {"actor": {"mbox": "mailto:ana@example.org"}, "verb": {"id": "http://adlnet.gov/expapi/verbs/experienced", "display": {"en-GB": "experienced"}},
       "object": {"id": "https://example.org/course/video2"}}
    ]}"#;
    let (ds, report) = read_bytes("export.json", "export", json.as_bytes(), None).unwrap();
    assert_eq!(ds.n_rows(), 3);
    let text = |name: &str| match &ds.columns[ds.find(name).unwrap()] {
        Column::Text(v) => v.clone(),
        _ => unreachable!(),
    };
    let num = |name: &str| match &ds.columns[ds.find(name).unwrap()] {
        Column::Numeric(v) => v.clone(),
        _ => unreachable!(),
    };
    assert_eq!(text("learner"), vec![Some("Learner 1".into()), Some("Learner 2".into()), Some("Learner 1".into())]);
    assert_eq!(text("verb"), vec![Some("passed".into()), Some("failed".into()), Some("experienced".into())]);
    assert_eq!(text("activity")[0].as_deref(), Some("Quiz 1"));
    assert_eq!(num("score_raw"), vec![Some(17.0), Some(9.0), None]);
    assert_eq!(num("success"), vec![Some(1.0), Some(0.0), None]);
    assert_eq!(num("duration_s"), vec![Some(750.0), Some(90000.0), None]);
    // No name, email or account reaches the data or the report.
    let all = serde_json::to_string(&ds).unwrap() + &serde_json::to_string(&report).unwrap();
    for secret in ["ana@example.org", "Ana", "u42", "lms.example.org"] {
        assert!(!all.contains(secret), "{secret} leaked");
    }
    assert!(report.notes[0].contains("2 learners"), "{:?}", report.notes);
    // JSON Lines works too, and other JSON is still refused.
    let lines = "{\"actor\":{\"mbox\":\"mailto:a@b.c\"},\"verb\":{\"id\":\"x/y\"},\"object\":{\"id\":\"o\"}}\n{\"actor\":{\"mbox\":\"mailto:a@b.c\"},\"verb\":{\"id\":\"x/z\"},\"object\":{\"id\":\"o\"}}\n";
    assert_eq!(read_bytes("s.jsonl.json", "s", lines.as_bytes(), None).unwrap().0.n_rows(), 2);
    assert!(read_bytes("a.json", "a", b"{\"rows\": [1, 2]}", None).unwrap_err().to_string().contains("xAPI"));
}
