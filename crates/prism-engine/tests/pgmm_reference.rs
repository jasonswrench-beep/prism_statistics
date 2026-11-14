//! Dynamic panel GMM against plm's pgmm (reference/pgmm_reference.json, made by make_pgmm_reference.R): Arellano and
//! Bond's employment equation (one and two steps, individual and two-way effects, collapsed instruments, limited
//! lags) and Grunfeld (where plm falls back on generalized inverses): coefficients, robust (Windmeijer) and
//! conventional standard errors, Sargan, AR(1), AR(2) and Wald tests.

use std::path::PathBuf;

use prism_engine::analysis::dpanel;
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
    let b = dpanel::run(ds, &serde_json::from_value(spec).unwrap()).unwrap();
    assert_eq!(b.status, Status::Ok, "{:?}", b.status);
    b
}

fn compare(b: &ResultBundle, prefix: &str, tol: f64) -> usize {
    let r: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(dir().join("pgmm_reference.json")).unwrap()).unwrap();
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
                    failures.push(format!("{key}: got {got}, plm gives {want}"));
                }
            }
            None => failures.push(format!("{key}: missing")),
        }
        n += 1;
    }
    assert!(failures.is_empty(), "{} values differ:\n{}", failures.len(), failures.iter().take(40).cloned().collect::<Vec<_>>().join("\n"));
    eprintln!("{prefix}: {n} values, largest relative error {worst:?}");
    n
}

fn ab(extra: serde_json::Value) -> serde_json::Value {
    let mut s = serde_json::json!({ "dependent": "lemp", "covariates": ["lwage", "lcap"], "individual": "firm", "time": "year", "y_lags": 2, "x_lags": 1 });
    s.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    s
}

#[test]
fn arellano_bond() {
    let em = data("empluk.csv");
    for (p, extra) in [
        ("ab2", serde_json::json!({})),
        ("ab1", serde_json::json!({ "model": "onestep" })),
        ("ab1n", serde_json::json!({ "model": "onestep", "robust": false })),
        ("abi", serde_json::json!({ "effect": "individual" })),
        ("abc", serde_json::json!({ "collapse": true })),
    ] {
        let b = run(&em, ab(extra));
        assert!(compare(&b, p, 1e-7) >= 40, "{p}");
    }
    let b = run(&em, serde_json::json!({ "dependent": "lemp", "covariates": ["lwage", "lcap"], "individual": "firm", "time": "year", "gmm_max": 4 }));
    assert!(compare(&b, "ab4", 1e-7) > 30);
    let text = paragraph(&b).unwrap();
    assert!(text.contains("Arellano") && text.contains("Sargan"), "{text}");
}

#[test]
fn generalized_inverse_fallback() {
    let g = data("grunfeld.csv");
    let b = run(&g, serde_json::json!({ "dependent": "inv", "covariates": ["value", "capital"], "individual": "firm", "time": "year", "effect": "individual", "model": "onestep" }));
    // plm falls back on generalized inverses here; they agree to about 1e-6.
    assert!(compare(&b, "gr", 1e-5) > 15);
}
