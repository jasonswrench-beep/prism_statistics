//! Penalized regression with cross-validation against glmnet (reference/predict_reference.json, made by
//! make_predict_reference.R): the λ sequence and where it stops, coefficients along the path, the CV error and its
//! standard error at every λ (same folds), λ_min and λ_1se with their coefficients, and the out-of-fold performance.

use std::path::PathBuf;

use prism_engine::analysis::predict;
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
    let b = predict::run(ds, &serde_json::from_value(spec).unwrap()).unwrap();
    assert_eq!(b.status, Status::Ok, "{:?}", b.status);
    b
}

fn compare(b: &ResultBundle, prefix: &str, tol: f64) -> usize {
    let r: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(dir().join("predict_reference.json")).unwrap()).unwrap();
    let (mut n, mut failures, mut worst) = (0, Vec::new(), (0.0f64, String::new()));
    for (key, want) in r.iter().filter(|(k, _)| k.starts_with(&format!("{prefix}."))) {
        let field = &key[prefix.len() + 1..];
        let want = want.as_f64().unwrap();
        let Some(got) = b.number(field) else {
            failures.push(format!("{key}: no value"));
            continue;
        };
        let err = (got - want).abs() / want.abs().max(1.0);
        if err > worst.0 {
            worst = (err, key.clone());
        }
        if err > tol {
            failures.push(format!("{key}: got {got}, glmnet gives {want} (relative error {err:e})"));
        }
        n += 1;
    }
    assert!(failures.is_empty(), "{} values differ:\n{}", failures.len(), failures.iter().take(30).cloned().collect::<Vec<_>>().join("\n"));
    eprintln!("{prefix}: {n} values, largest relative error {worst:?}");
    n
}

const BOSTON: [&str; 13] = ["crim", "zn", "indus", "chas", "nox", "rm", "age", "dis", "rad", "tax", "ptratio", "black", "lstat"];

#[test]
fn linear_lasso_ridge_and_elastic_net() {
    let bos = data("boston.csv");
    for (prefix, penalty) in [("bostonlasso", "lasso"), ("bostonridge", "ridge"), ("bostonelastic", "elastic")] {
        let b = run(&bos, serde_json::json!({ "dependent": "medv", "covariates": BOSTON, "penalty": penalty, "alpha": 0.5, "fold_variable": "fold" }));
        assert!(compare(&b, prefix, 1e-6) > 200, "{prefix}");
    }
}

#[test]
fn logistic_lasso_and_ridge() {
    let pima = data("pima.csv");
    for (prefix, penalty) in [("pimalasso", "lasso"), ("pimaridge", "ridge")] {
        let b = run(&pima, serde_json::json!({ "dependent": "type", "covariates": ["npreg", "glu", "bp", "skin", "bmi", "ped", "age"], "outcome": "binary", "penalty": penalty, "fold_variable": "fold" }));
        assert!(compare(&b, prefix, 1e-6) > 150, "{prefix}");
        let text = paragraph(&b).unwrap();
        assert!(text.contains("cross-validat"), "{text}");
    }
}

#[test]
fn random_folds_are_reproducible() {
    let bos = data("boston.csv");
    let spec = serde_json::json!({ "dependent": "medv", "covariates": BOSTON, "folds": 5, "repeats": 3, "seed": 9 });
    let (a, b) = (run(&bos, spec.clone()), run(&bos, spec));
    assert_eq!(a.number("lambda.1se"), b.number("lambda.1se"));
    assert_eq!(a.number("cv.rmse"), b.number("cv.rmse"));
}
