//! Gradient boosting against gbm (reference/boost_reference.json, made by make_boost_reference.R). With bag.fraction =
//! 1 gbm is deterministic, so every iteration's training deviance, every fitted value and the relative influences are
//! compared: squared error with one and three splits per tree (Boston), Bernoulli (Pima), missing predictor values
//! (airquality) and a categorical predictor (birth weight).

use std::path::PathBuf;

use prism_engine::analysis::boost;
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
    let b = boost::run(ds, &serde_json::from_value(spec).unwrap()).unwrap();
    assert_eq!(b.status, Status::Ok, "{:?}", b.status);
    b
}

fn compare(b: &ResultBundle, prefix: &str, tol: f64) -> usize {
    let r: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(dir().join("boost_reference.json")).unwrap()).unwrap();
    let (mut n, mut failures, mut worst) = (0, Vec::new(), (0.0f64, String::new()));
    for (key, want) in r.iter().filter(|(k, _)| k.starts_with(&format!("{prefix}."))) {
        let field = &key[prefix.len() + 1..];
        let want = want.as_f64().unwrap();
        match b.number(field) {
            Some(got) => {
                let err = (got - want).abs() / want.abs().max(1.0);
                if err > worst.0 {
                    worst = (err, key.clone());
                }
                if err > tol {
                    failures.push(format!("{key}: got {got}, gbm gives {want}"));
                }
            }
            None => failures.push(format!("{key}: missing")),
        }
        n += 1;
    }
    assert!(failures.is_empty(), "{} values differ:\n{}", failures.len(), failures.iter().take(30).cloned().collect::<Vec<_>>().join("\n"));
    eprintln!("{prefix}: {n} values, largest relative error {worst:?}");
    n
}

fn spec(dep: &str, preds: &[&str], extra: serde_json::Value) -> serde_json::Value {
    let mut s = serde_json::json!({ "dependent": dep, "predictors": preds, "bag_fraction": 1.0, "cv_folds": 0 });
    s.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    s
}

#[test]
fn squared_error() {
    let bos = data("boston.csv");
    let preds = ["crim", "rm", "lstat", "ptratio", "dis", "nox", "chas", "age", "tax"];
    let b = run(&bos, spec("medv", &preds, serde_json::json!({ "depth": 1 })));
    assert!(compare(&b, "bos1", 1e-9) > 600);
    let b = run(&bos, spec("medv", &preds, serde_json::json!({ "depth": 3 })));
    assert!(compare(&b, "bos3", 1e-9) > 600);
    let aq = data("airquality.csv");
    let b = run(&aq, spec("Ozone", &["SolarR", "Wind", "Temp", "Month", "Day"], serde_json::json!({ "depth": 2, "min_obs": 5 })));
    assert!(compare(&b, "aq", 1e-9) > 200);
}

#[test]
fn bernoulli() {
    let pima = data("pima.csv");
    let b = run(&pima, spec("type", &["npreg", "glu", "bp", "skin", "bmi", "ped", "age"], serde_json::json!({ "depth": 2 })));
    assert!(compare(&b, "pima", 1e-9) > 300);
    let bw = data("birthwt.csv");
    let b = run(&bw, spec("low", &["age", "lwt", "race", "smoke", "ptl", "ht", "ui"], serde_json::json!({ "depth": 3, "min_obs": 5, "distribution": "bernoulli" })));
    assert!(compare(&b, "bw", 1e-9) > 250);
}

#[test]
fn cross_validation_chooses_the_trees() {
    let bos = data("boston.csv");
    let preds = ["crim", "rm", "lstat", "ptratio", "dis", "nox", "chas", "age", "tax"];
    let b = run(&bos, serde_json::json!({ "dependent": "medv", "predictors": preds, "depth": 2, "n_trees": 300, "fold_variable": "fold" }));
    let best = b.number("best_trees").unwrap();
    assert!(best > 50.0 && best <= 300.0, "{best}");
    let cv = b.number("cv.r2").unwrap();
    assert!(cv > 0.7 && cv < b.number("train.r2").unwrap(), "{cv}");
    let text = paragraph(&b).unwrap();
    assert!(text.contains("boosting"), "{text}");
}
