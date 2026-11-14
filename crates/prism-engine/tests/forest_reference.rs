//! Random forests against randomForest (reference/forest_reference.json, made by make_forest_reference.R).
//! Deterministic trees (every predictor tried, every case used once) must equal randomForest's exactly. Forests
//! depend on random draws that cannot be reproduced, so their out-of-bag error and importances must fall within
//! the spread of ten randomForest runs.

use std::path::PathBuf;

use prism_engine::analysis::forest;
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

fn reference() -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_str(&std::fs::read_to_string(dir().join("forest_reference.json")).unwrap()).unwrap()
}

fn run(ds: &Dataset, spec: serde_json::Value) -> ResultBundle {
    let b = forest::run(ds, &serde_json::from_value(spec).unwrap()).unwrap();
    assert_eq!(b.status, Status::Ok, "{:?}", b.status);
    b
}

const BOSTON: [&str; 9] = ["crim", "rm", "lstat", "ptratio", "dis", "nox", "chas", "age", "tax"];

/// The Boston tree with nodesize 5 ("det5" in the reference) is left out: one of its nodes has two different splits
/// with exactly equal sums of squares, which randomForest breaks at random.
#[test]
fn deterministic_trees_match_exactly() {
    let r = reference();
    let bos = data("boston.csv");
    let bw = data("birthwt.csv");
    for (p, ds, dep, preds, nodesize) in [("det20", &bos, "medv", BOSTON.to_vec(), 20), ("detcat", &bw, "bwt", vec!["age", "lwt", "race", "smoke", "ptl", "ht", "ui"], 5)] {
        let b = run(ds, serde_json::json!({ "dependent": dep, "predictors": preds, "kind": "regression", "n_trees": 1, "mtry": preds.len(), "replace": false, "nodesize": nodesize, "sample_size": ds.n_rows(), "importance": false }));
        let mut n = 0;
        for (k, want) in r.iter().filter(|(k, _)| k.starts_with(&format!("{p}."))) {
            let got = b.number(&k[p.len() + 1..]).unwrap();
            let want = want.as_f64().unwrap();
            assert!((got - want).abs() <= 1e-9 * want.abs().max(1.0), "{k}: got {got}, randomForest gives {want}");
            n += 1;
        }
        assert!(n > 180, "{p}");
    }
}

/// Our value against randomForest's mean ± k SDs over ten runs (with a floor for very stable quantities).
fn within(r: &serde_json::Map<String, serde_json::Value>, key: &str, got: f64, k: f64, floor: f64) {
    let m = r[&format!("{key}.mean")].as_f64().unwrap();
    let s = r[&format!("{key}.sd")].as_f64().unwrap();
    let tol = (k * s).max(floor * m.abs());
    assert!((got - m).abs() <= tol, "{key}: ours {got}, randomForest {m} ± {s}");
    eprintln!("{key}: ours {got:.4}, randomForest {m:.4} ± {s:.4}");
}

#[test]
fn regression_forest_agrees_statistically() {
    let r = reference();
    let bos = data("boston.csv");
    let b = run(&bos, serde_json::json!({ "dependent": "medv", "predictors": BOSTON }));
    within(&r, "bos.err", b.number("oob.rsq").unwrap(), 5.0, 0.01);
    for v in BOSTON {
        within(&r, &format!("bos.purity.{v}"), b.number(&format!("purity.{v}")).unwrap(), 5.0, 0.1);
    }
    let text = paragraph(&b).unwrap();
    assert!(text.contains("random forest"), "{text}");
}

#[test]
fn classification_forests_agree_statistically() {
    let r = reference();
    let ir = data("iris.csv");
    let preds = ["sepal_length", "sepal_width", "petal_length", "petal_width"];
    let b = run(&ir, serde_json::json!({ "dependent": "species", "predictors": preds }));
    within(&r, "iris.err.OOB", b.number("oob.error").unwrap(), 5.0, 0.5);
    for v in preds {
        within(&r, &format!("iris.purity.{v}"), b.number(&format!("purity.{v}")).unwrap(), 5.0, 0.1);
    }
    let pima = data("pima.csv");
    let preds = ["npreg", "glu", "bp", "skin", "bmi", "ped", "age"];
    let b = run(&pima, serde_json::json!({ "dependent": "type", "predictors": preds }));
    within(&r, "pima.err.OOB", b.number("oob.error").unwrap(), 5.0, 0.05);
    for v in preds {
        within(&r, &format!("pima.purity.{v}"), b.number(&format!("purity.{v}")).unwrap(), 5.0, 0.1);
    }
}
