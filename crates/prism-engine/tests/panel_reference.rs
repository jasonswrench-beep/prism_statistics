//! Linear panel models against plm (reference/panel_reference.json, made by make_panel_reference.R): pooled, fixed
//! effects (individual, time, two-way), random effects (Swamy-Arora), between and first differences, conventional and
//! cluster-robust standard errors, R², the overall test, the variance components and the F, Hausman and
//! Breusch-Pagan tests, on balanced (Grunfeld, Produc) and unbalanced (EmplUK) panels.

use std::path::PathBuf;

use prism_engine::analysis::panel;
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
    let b = panel::run(ds, &serde_json::from_value(spec).unwrap()).unwrap();
    assert_eq!(b.status, Status::Ok, "{:?}", b.status);
    b
}

fn compare(b: &ResultBundle, prefix: &str, tol: f64) -> usize {
    let r: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(dir().join("panel_reference.json")).unwrap()).unwrap();
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
            failures.push(format!("{key}: got {got}, plm gives {want} (relative error {err:e})"));
        }
        n += 1;
    }
    assert!(failures.is_empty(), "{} values differ:\n{}", failures.len(), failures.join("\n"));
    eprintln!("{prefix}: {n} values, largest relative error {worst:?}");
    n
}

fn grunfeld(model: &str, extra: serde_json::Value) -> serde_json::Value {
    let mut s = serde_json::json!({ "dependent": "inv", "covariates": ["value", "capital"], "individual": "firm", "time": "year", "model": model });
    for (k, v) in extra.as_object().unwrap() {
        s[k] = v.clone();
    }
    s
}

#[test]
fn balanced_panel_every_model() {
    let g = data("grunfeld.csv");
    for (prefix, model, extra) in [
        ("gpool", "pooling", serde_json::json!({})),
        ("gwithin", "within", serde_json::json!({})),
        ("gtime", "within", serde_json::json!({ "effect": "time" })),
        ("gtwo", "within", serde_json::json!({ "effect": "twoways" })),
        ("grandom", "random", serde_json::json!({})),
        ("gbetween", "between", serde_json::json!({})),
        ("gfd", "fd", serde_json::json!({})),
        ("gwithinr", "within", serde_json::json!({ "robust": true })),
        ("grandomr", "random", serde_json::json!({ "robust": true })),
        ("gpoolr", "pooling", serde_json::json!({ "robust": true })),
    ] {
        let b = run(&g, grunfeld(model, extra));
        assert!(compare(&b, prefix, 1e-8) >= 10, "{prefix}");
    }
    let b = run(&g, grunfeld("within", serde_json::json!({})));
    assert!(compare(&b, "g", 1e-8) >= 9);
    let text = paragraph(&b).unwrap();
    assert!(text.contains("fixed effects") && text.contains("Hausman"), "{text}");
}

#[test]
fn balanced_panel_with_more_predictors() {
    let p = data("produc.csv");
    let spec = |m: &str| serde_json::json!({ "dependent": "lgsp", "covariates": ["lpcap", "lpc", "lemp", "unemp"], "individual": "state", "time": "year", "model": m });
    let b = run(&p, spec("within"));
    assert!(compare(&b, "pwithin", 1e-8) > 15);
    assert!(compare(&b, "p", 1e-8) >= 9);
    let b = run(&p, spec("random"));
    assert!(compare(&b, "prandom", 1e-8) > 20);
}

#[test]
fn unbalanced_panel() {
    let e = data("empluk.csv");
    let spec = |m: &str, eff: &str| serde_json::json!({ "dependent": "lemp", "covariates": ["lwage", "lcap"], "individual": "firm", "time": "year", "model": m, "effect": eff });
    let b = run(&e, spec("within", "individual"));
    assert_eq!(b.number("balanced"), Some(0.0));
    assert!(compare(&b, "ewithin", 1e-8) >= 10);
    assert!(compare(&b, "e", 1e-8) >= 9);
    for (prefix, m, eff) in [("etwo", "within", "twoways"), ("erandom", "random", "individual"), ("efd", "fd", "individual"), ("ebetween", "between", "individual")] {
        let b = run(&e, spec(m, eff));
        assert!(compare(&b, prefix, 1e-8) >= 10, "{prefix}");
    }
}

#[test]
fn two_way_and_time_random_effects() {
    let g = data("grunfeld.csv");
    for (prefix, extra) in [("grandom2", serde_json::json!({ "effect": "twoways" })), ("grandomt", serde_json::json!({ "effect": "time" })), ("grandom2r", serde_json::json!({ "effect": "twoways", "robust": true }))] {
        let b = run(&g, grunfeld("random", extra));
        assert!(compare(&b, prefix, 1e-8) >= 12, "{prefix}");
    }
    let p = data("produc.csv");
    let b = run(&p, serde_json::json!({ "dependent": "lgsp", "covariates": ["lpcap", "lpc", "lemp", "unemp"], "individual": "state", "time": "year", "model": "random", "effect": "twoways" }));
    assert!(compare(&b, "prandom2", 1e-8) > 20);
    let e = data("empluk.csv");
    let spec = |eff: &str| serde_json::json!({ "dependent": "lemp", "covariates": ["lwage", "lcap"], "individual": "firm", "time": "year", "model": "random", "effect": eff });
    let b = run(&e, spec("time"));
    assert!(compare(&b, "erandomt", 1e-8) > 12);
    // Two-way random effects need a balanced panel.
    let b = prism_engine::analysis::panel::run(&e, &serde_json::from_value(spec("twoways")).unwrap()).unwrap();
    assert!(matches!(b.status, prism_engine::result::Status::Failed { .. }), "{:?}", b.status);
}
