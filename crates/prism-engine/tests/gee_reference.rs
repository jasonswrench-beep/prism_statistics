//! Generalized estimating equations against geepack (reference/gee_reference.json, made by make_gee_reference.R):
//! coefficients, robust and model-based standard errors, Wald tests, the scale, the working correlation and QIC, for
//! the binomial, Gaussian and Poisson families and every working correlation.

use std::path::PathBuf;

use prism_engine::analysis::gee;
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
    let b = gee::run(ds, &serde_json::from_value(spec).unwrap()).unwrap();
    assert_eq!(b.status, Status::Ok, "{:?}", b.status);
    b
}

fn compare(b: &ResultBundle, prefix: &str, tol: f64) -> usize {
    let r: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(dir().join("gee_reference.json")).unwrap()).unwrap();
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
            failures.push(format!("{key}: got {got}, geepack gives {want} (relative error {err:e})"));
        }
        n += 1;
    }
    assert!(failures.is_empty(), "{} values differ:\n{}", failures.len(), failures.join("\n"));
    eprintln!("{prefix}: {n} values, largest relative error {worst:?}");
    n
}

#[test]
fn binary_outcome_every_working_correlation() {
    let ohio = data("ohio.csv");
    for cs in ["independence", "exchangeable", "ar1", "unstructured"] {
        let b = run(&ohio, serde_json::json!({ "dependent": "resp", "covariates": ["age", "smoke"], "cluster": "id", "family": "binomial", "corstr": cs }));
        assert!(compare(&b, &format!("ohio{cs}"), 1e-6) > 18, "{cs}");
    }
}

#[test]
fn gaussian_outcome_with_waves_and_a_factor() {
    let d = data("dietox.csv");
    for cs in ["exchangeable", "ar1"] {
        let b = run(&d, serde_json::json!({ "dependent": "Weight", "covariates": ["Time"], "factors": [{ "name": "Cu" }], "cluster": "Pig", "wave": "Time", "family": "gaussian", "corstr": cs }));
        assert!(compare(&b, &format!("dietox{cs}"), 1e-6) > 25, "{cs}");
    }
}

#[test]
fn count_outcome() {
    let e = data("epil.csv");
    let b = run(&e, serde_json::json!({ "dependent": "y", "covariates": ["base", "age"], "factors": [{ "name": "trt" }], "cluster": "subject", "family": "poisson" }));
    // The design orders covariates before factors; geepack's formula has trt first.
    let r: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(dir().join("gee_reference.json")).unwrap()).unwrap();
    let map = [(1, 1), (2, 3), (3, 4), (4, 2)];
    for (ours, theirs) in map {
        for f in ["b", "se", "naive_se", "wald", "p"] {
            let (got, want) = (b.number(&format!("coef.{ours}.{f}")).unwrap(), r[&format!("epil.coef.{theirs}.{f}")].as_f64().unwrap());
            assert!((got - want).abs() / want.abs().max(1.0) < 1e-6, "coef {theirs} {f}: {got} vs {want}");
        }
    }
    for k in ["phi", "alpha.1", "qic", "qicu", "cic", "qicc", "quasi", "clusters"] {
        let (got, want) = (b.number(k).unwrap(), r[&format!("epil.{k}")].as_f64().unwrap());
        assert!((got - want).abs() / want.abs().max(1.0) < 1e-6, "{k}: {got} vs {want}");
    }
    let text = paragraph(&b).unwrap();
    assert!(text.contains("generalized estimating equations") && text.contains("rate ratio ="), "{text}");
}

#[test]
fn input_errors_are_plain() {
    let ohio = data("ohio.csv");
    let b = gee::run(&ohio, &serde_json::from_value(serde_json::json!({ "dependent": "age", "covariates": ["smoke"], "cluster": "id", "family": "binomial" })).unwrap()).unwrap();
    assert!(matches!(b.status, Status::Failed { .. }));
}
