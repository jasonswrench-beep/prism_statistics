//! Bayesian random-effects meta-analysis against bayesmeta (reference/bmeta_reference.json, made by
//! make_bmeta_reference.R): the posterior of τ as bayesmeta reports it (exact integrals), and the posteriors of μ, a
//! new study's effect and each study's shrinkage estimate as exact mixtures over bayesmeta's own posterior of τ and
//! conditional moments (bayesmeta's printed summaries of these come from its DIRECT grid approximation).

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

fn compare(b: &ResultBundle, prefix: &str, tol: f64) -> usize {
    let r: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(dir().join("bmeta_reference.json")).unwrap()).unwrap();
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
                    failures.push(format!("{key}: got {got}, bayesmeta gives {want}"));
                }
            }
            None => failures.push(format!("{key}: missing")),
        }
        n += 1;
    }
    assert!(failures.is_empty(), "{} values differ:\n{}", failures.len(), failures.join("\n"));
    eprintln!("{prefix}: {n} values, largest relative error {worst:?}");
    n
}

fn crins(extra: serde_json::Value) -> serde_json::Value {
    let mut s = serde_json::json!({ "measure": "or", "events1": "e1", "n1": "n1", "events2": "e2", "n2": "n2", "study": "study", "method": "bayes" });
    s.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    s
}

#[test]
fn crins_with_three_priors() {
    let cr = data("crins.csv");
    let b = run(&cr, crins(serde_json::json!({})));
    assert!(compare(&b, "crins", 1e-6) > 40);
    let text = paragraph(&b).unwrap();
    assert!(text.contains("Bayesian") && text.contains("credible"), "{text}");
    let b = run(&cr, crins(serde_json::json!({ "tau_prior": "half_normal", "tau_prior_scale": 0.5 })));
    assert!(compare(&b, "crins_hn", 1e-6) > 40);
    let b = run(&cr, crins(serde_json::json!({ "tau_prior": "half_cauchy", "tau_prior_scale": 0.5, "mu_prior_mean": 0, "mu_prior_sd": 4 })));
    assert!(compare(&b, "crins_hc", 1e-6) > 40);
}

#[test]
fn bcg_and_normand() {
    let bcg = data("bcg.csv");
    let b = run(&bcg, serde_json::json!({ "measure": "rr", "events1": "tpos", "n1": "n1", "events2": "cpos", "n2": "n2", "study": "label", "method": "bayes" }));
    assert!(compare(&b, "bcg", 1e-6) > 70);
    let nm = data("normand.csv");
    let b = run(&nm, serde_json::json!({ "measure": "md", "mean1": "m1i", "sd1": "sd1i", "n1": "n1i", "mean2": "m2i", "sd2": "sd2i", "n2": "n2i", "method": "bayes", "tau_prior": "half_normal", "tau_prior_scale": 10 }));
    assert!(compare(&b, "normand", 1e-6) > 60);
}
