//! Decision trees against rpart (reference/tree_reference.json, made by make_tree_reference.R): the complexity table
//! with its cross-validated error and standard error (same folds), every node of the tree (size, prediction, risk,
//! split point) and the trees pruned at the minimum CV error and by the one-SE rule, for regression and
//! classification (two and three classes), with numeric and categorical predictors.

use std::path::PathBuf;

use prism_engine::analysis::tree;
use prism_engine::import::read_file;
use prism_engine::result::Status;
use prism_engine::templates::paragraph;
use prism_engine::{Dataset, ResultBundle};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("reference")
}

fn data(file: &str) -> Dataset {
    read_file(&dir().join(file)).unwrap().0
}

fn run(ds: &Dataset, spec: serde_json::Value) -> ResultBundle {
    let b = tree::run(ds, &serde_json::from_value(spec).unwrap()).unwrap();
    assert_eq!(b.status, Status::Ok, "{:?}", b.status);
    b
}

/// Compares the values under `prefix` (and nothing deeper: "boston." but not "bostonmin."); with `nodes_only`, the
/// node values alone (the pruned trees).
fn compare(b: &ResultBundle, prefix: &str, tol: f64) -> usize {
    compare_except(b, prefix, tol, &[])
}

/// As compare, leaving out the fields that contain any of `skip`.
fn compare_except(b: &ResultBundle, prefix: &str, tol: f64, skip: &[&str]) -> usize {
    let r: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(dir().join("tree_reference.json")).unwrap()).unwrap();
    let (mut n, mut failures) = (0, Vec::new());
    for (key, want) in r.iter().filter(|(k, _)| k.starts_with(&format!("{prefix}."))) {
        let field = &key[prefix.len() + 1..];
        if skip.iter().any(|s| field.contains(s)) {
            continue;
        }
        if let Some(w) = want.as_str() {
            if b.text(field) != Some(w) {
                failures.push(format!("{key}: got {:?}, rpart gives {w:?}", b.text(field)));
            }
            n += 1;
            continue;
        }
        let want = want.as_f64().unwrap();
        match b.number(field) {
            Some(got) if (got - want).abs() / want.abs().max(1.0) <= tol => {}
            got => failures.push(format!("{key}: got {got:?}, rpart gives {want}")),
        }
        n += 1;
    }
    // Every node of ours must be in rpart's tree too.
    let theirs = r.keys().filter(|k| k.starts_with(&format!("{prefix}.node.")) && k.ends_with(".n")).count();
    let ours = b.fields.keys().filter(|k| k.starts_with("node.") && k.ends_with(".n")).count();
    if theirs != ours {
        failures.push(format!("{prefix}: {ours} nodes, rpart has {theirs}"));
    }
    assert!(failures.is_empty(), "{} values differ:\n{}", failures.len(), failures.join("\n"));
    eprintln!("{prefix}: {n} values");
    n
}

fn check(ds: &Dataset, spec: serde_json::Value, prefix: &str) {
    for (prune, p) in [("none", prefix.to_string()), ("min", format!("{prefix}min")), ("1se", format!("{prefix}1se"))] {
        let mut s = spec.clone();
        s["prune"] = serde_json::json!(prune);
        s["fold_variable"] = serde_json::json!("fold");
        let b = run(ds, s);
        assert!(compare(&b, &p, 1e-9) >= 3, "{p}");
    }
}

#[test]
fn regression_tree() {
    let bos = data("boston.csv");
    check(&bos, serde_json::json!({ "dependent": "medv", "predictors": ["crim", "rm", "lstat", "ptratio", "dis", "nox", "chas", "age", "tax"] }), "boston");
}

#[test]
fn classification_trees() {
    let pima = data("pima.csv");
    check(&pima, serde_json::json!({ "dependent": "type", "predictors": ["npreg", "glu", "bp", "skin", "bmi", "ped", "age"] }), "pima");
    let ir = data("iris.csv");
    check(&ir, serde_json::json!({ "dependent": "species", "predictors": ["sepal_length", "sepal_width", "petal_length", "petal_width"] }), "iris");
}

#[test]
fn categorical_predictor() {
    let bw = data("birthwt.csv");
    let preds = ["age", "lwt", "smoke", "ptl", "ht", "ui", "race"];
    check(&bw, serde_json::json!({ "dependent": "bwt", "predictors": preds, "kind": "regression" }), "bwt");
    check(&bw, serde_json::json!({ "dependent": "low", "predictors": preds, "kind": "classification" }), "low");
    let b = run(&bw, serde_json::json!({ "dependent": "bwt", "predictors": preds, "kind": "regression" }));
    let text = paragraph(&b).unwrap();
    assert!(text.contains("regression tree") && text.contains("cross-validat"), "{text}");
}

#[test]
fn missing_values_and_surrogates() {
    let aq = data("airquality.csv");
    check(&aq, serde_json::json!({ "dependent": "Ozone", "predictors": ["SolarR", "Wind", "Temp", "Month", "Day"] }), "aq");
    // Kyphosis has cases with every predictor missing and nodes whose primary split divides the cases evenly. There
    // rpart's own cross-validation is not reproducible: the CV errors change from run to run (ours equal one of the
    // runs). The tree, surrogates and importance are compared; the CV columns and the CV-pruned trees are not.
    let ky = data("kyphosis.csv");
    let spec = serde_json::json!({ "dependent": "Kyphosis", "predictors": ["Age", "Number", "Start"], "minsplit": 10, "fold_variable": "fold", "prune": "none" });
    let b = run(&ky, spec.clone());
    assert!(compare_except(&b, "ky", 1e-9, &["xerror", "xstd"]) > 50);
    let mut s0 = spec.clone();
    s0["maxsurrogate"] = serde_json::json!(0);
    let b0 = run(&ky, s0);
    assert!(compare_except(&b0, "ky0", 1e-9, &["xerror", "xstd"]) > 30);
    assert!(b0.number("node.1.sur.1.agree").is_none());
    assert_eq!(b.number("n"), Some(79.0), "two cases have every predictor missing");
    let bm = data("birthwt_missing.csv");
    let preds = ["age", "lwt", "smoke", "ptl", "ht", "ui", "race"];
    check(&bm, serde_json::json!({ "dependent": "bwt", "predictors": preds, "kind": "regression" }), "bwm");
    check(&bm, serde_json::json!({ "dependent": "low", "predictors": preds, "kind": "classification" }), "lowm");
}
