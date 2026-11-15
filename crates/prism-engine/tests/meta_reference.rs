//! Meta-analysis against metafor (reference/meta_reference.json, made by make_meta_reference.R): effect sizes and
//! variances, every τ² estimator, the Knapp-Hartung adjustment, meta-regression, heterogeneity and its Q-profile
//! intervals, prediction intervals, Egger's test, trim and fill and leave-one-out.

use std::path::PathBuf;

use prism_engine::analysis::meta;
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
    let b = meta::run(ds, &serde_json::from_value(spec).unwrap()).unwrap();
    assert_eq!(b.status, Status::Ok, "{:?}", b.status);
    b
}

/// Compares every reference value under `prefix`; returns how many were compared.
fn compare(b: &ResultBundle, prefix: &str, tol: f64) -> usize {
    let r: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(dir().join("meta_reference.json")).unwrap()).unwrap();
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
        // metafor refits the filled-in data with its default convergence (τ² to about 1e-5), which can't be changed.
        let tol = if field.starts_with("tf.") { tol.max(2e-5) } else { tol };
        if err > tol {
            failures.push(format!("{key}: got {got}, metafor gives {want} (relative error {err:e})"));
        }
        n += 1;
    }
    assert!(failures.is_empty(), "{} values differ:\n{}", failures.len(), failures.join("\n"));
    eprintln!("{prefix}: {n} values, largest relative error {worst:?}");
    n
}

fn counts(measure: &str) -> serde_json::Value {
    serde_json::json!({ "measure": measure, "events1": "tpos", "n1": "n1", "events2": "cpos", "n2": "n2", "study": "label" })
}

fn with(mut base: serde_json::Value, extra: serde_json::Value) -> serde_json::Value {
    for (k, v) in extra.as_object().unwrap() {
        base[k] = v.clone();
    }
    base
}

#[test]
fn odds_ratios_with_bias_and_leave_one_out() {
    let bcg = data("bcg.csv");
    let b = run(&bcg, with(counts("or"), serde_json::json!({ "leave_one_out": true })));
    assert!(compare(&b, "or", 1e-6) > 100);
    assert_eq!(b.charts.len(), 2);
    let text = paragraph(&b).unwrap();
    assert!(text.contains("odds ratio = 0.47") && text.contains("*I*² = 92.1%") && text.contains("trim and fill imputed no missing studies"), "{text}");
}

#[test]
fn every_tau2_estimator() {
    let bcg = data("bcg.csv");
    for m in ["fe", "reml", "ml", "dl", "pm", "he", "sj"] {
        let b = run(&bcg, with(counts("rr"), serde_json::json!({ "method": m })));
        assert!(compare(&b, &format!("rr{m}"), 1e-6) > 60, "{m}");
    }
    let b = run(&bcg, with(counts("rr"), serde_json::json!({ "knha": true })));
    assert!(compare(&b, "rrknha", 1e-6) > 60);
    let b = run(&bcg, with(counts("rd"), serde_json::json!({ "method": "dl" })));
    assert!(compare(&b, "rd", 1e-6) > 50);
}

#[test]
fn meta_regression() {
    let bcg = data("bcg.csv");
    let b = run(&bcg, with(counts("rr"), serde_json::json!({ "moderators": ["ablat", "alloc"] })));
    assert!(compare(&b, "mods", 1e-6) > 60);
    let notes: Vec<&String> = b.tables.iter().flat_map(|t| &t.footnotes).filter(|f| f.starts_with("Test of moderators")).collect();
    assert!(notes.len() == 1 && (notes[0].contains(", p = .") || notes[0].contains(", p < .001")), "{notes:?}");
    let b = run(&bcg, with(counts("rr"), serde_json::json!({ "moderators": ["ablat"], "knha": true })));
    assert!(compare(&b, "modsknha", 1e-6) > 50);
    for m in ["ml", "dl", "pm", "he", "sj"] {
        let b = run(&bcg, with(counts("rr"), serde_json::json!({ "moderators": ["ablat"], "method": m })));
        assert!(compare(&b, &format!("mods{m}"), 1e-6) > 50, "{m}");
    }
}

#[test]
fn means_and_correlations() {
    let nm = data("normand.csv");
    let means = serde_json::json!({ "mean1": "m1i", "sd1": "sd1i", "n1": "n1i", "mean2": "m2i", "sd2": "sd2i", "n2": "n2i", "study": "source" });
    let b = run(&nm, with(means.clone(), serde_json::json!({ "measure": "smd" })));
    assert!(compare(&b, "smd", 1e-6) > 40);
    let b = run(&nm, with(means, serde_json::json!({ "measure": "md", "method": "dl" })));
    assert!(compare(&b, "md", 1e-6) > 30);
    let ml = data("molloy.csv");
    let b = run(&ml, serde_json::json!({ "measure": "zcor", "r": "ri", "n1": "ni", "study": "label", "leave_one_out": true }));
    assert!(compare(&b, "zcor", 1e-6) > 80);
}

#[test]
fn generic_effects_from_standard_errors() {
    // The log risk ratios and variances metafor computed, given back as effects and standard errors.
    let bcg = data("bcg.csv");
    let r = run(&bcg, counts("rr"));
    let k = r.number("k").unwrap() as usize;
    let mut ds = Dataset::new("generic");
    let col = |f: &dyn Fn(usize) -> f64| prism_engine::model::Column::Numeric((1..=k).map(|i| Some(f(i))).collect());
    ds.add_variable("yi", col(&|i| r.number(&format!("yi.{i}")).unwrap())).unwrap();
    ds.add_variable("sei", col(&|i| r.number(&format!("vi.{i}")).unwrap().sqrt())).unwrap();
    let b = run(&ds, serde_json::json!({ "effect": "yi", "se": "sei" }));
    for key in ["beta.1.est", "beta.1.se", "tau2", "i2", "qe"] {
        assert!((b.number(key).unwrap() - r.number(key).unwrap()).abs() < 1e-9, "{key}");
    }
}

#[test]
fn input_errors_are_plain() {
    let bcg = data("bcg.csv");
    let err = |s: serde_json::Value| meta::run(&bcg, &serde_json::from_value(s).unwrap()).unwrap_err().to_string();
    assert!(err(serde_json::json!({ "measure": "or", "events1": "tpos" })).contains("group 1's total"));
    assert!(err(serde_json::json!({ "effect": "tpos" })).contains("sampling variance or standard error"));
}

#[test]
fn three_level_models() {
    let kd = data("konstantopoulos.csv");
    let base = serde_json::json!({ "measure": "generic", "effect": "yi", "variance": "vi", "cluster": "district", "study": "school" });
    let b = run(&kd, base.clone());
    assert!(compare(&b, "ml_k", 1e-7) > 70);
    let text = prism_engine::templates::paragraph(&b).unwrap();
    assert!(text.contains("three-level"), "{text}");
    let b = run(&kd, with(base.clone(), serde_json::json!({ "method": "ml" })));
    assert!(compare(&b, "ml_kml", 1e-7) > 70);
    let b = run(&kd, with(base, serde_json::json!({ "knha": true })));
    assert!(compare(&b, "ml_kt", 1e-7) > 70);
    let a = data("assink.csv");
    let base = serde_json::json!({ "measure": "generic", "effect": "yi", "variance": "vi", "cluster": "study" });
    let b = run(&a, with(base.clone(), serde_json::json!({ "moderators": ["year"] })));
    assert!(compare(&b, "ml_a", 1e-6) > 100);
    // metafor's optimizers agree with one another only to about 1e-6 here; the likelihood is at least as high.
    let b = run(&a, base);
    assert!(compare(&b, "ml_a0", 2e-5) > 100);
    let r: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(dir().join("meta_reference.json")).unwrap()).unwrap();
    assert!(b.number("logl").unwrap() >= r["ml_a0.logl"].as_f64().unwrap() - 1e-10);
}
