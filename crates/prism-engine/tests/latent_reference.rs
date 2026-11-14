//! Latent class analysis against poLCA and item response models against mirt (reference/latent_reference.json, made
//! by make_latent_reference.R from the packages' example data).

use std::path::PathBuf;

use prism_engine::analysis::lca;
use prism_engine::import::read_file;
use prism_engine::result::Status;
use prism_engine::templates::paragraph;
use prism_engine::{Dataset, ResultBundle};

fn reference_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("reference")
}

fn r() -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_str(&std::fs::read_to_string(reference_dir().join("latent_reference.json")).unwrap()).unwrap()
}

fn data(file: &str) -> Dataset {
    read_file(&reference_dir().join(file)).unwrap().0
}

/// Every reference value under `prefix` (a field of the same name), with its own tolerance for standard errors.
fn compare(b: &ResultBundle, prefix: &str, tol: f64, tol_se: f64) -> usize {
    let mut failures = Vec::new();
    let mut n = 0;
    for (key, want) in r().iter().filter(|(k, _)| k.starts_with(&format!("{prefix}."))) {
        let field = &key[prefix.len() + 1..];
        let want = want.as_f64().unwrap();
        let Some(got) = b.number(field) else {
            failures.push(format!("{key}: missing"));
            continue;
        };
        let t = if field.contains(".se") { tol_se } else { tol };
        let err = (got - want).abs() / want.abs().max(1.0);
        if err > t {
            failures.push(format!("{key}: got {got}, R gives {want} ({err:e})"));
        }
        n += 1;
    }
    assert!(failures.is_empty(), "{} values differ:\n{}", failures.len(), failures.join("\n"));
    n
}

fn lca_fit(ds: &Dataset, items: &[&str], classes: usize) -> ResultBundle {
    let b = lca::run(ds, &serde_json::from_value(serde_json::json!({ "variables": items, "classes": classes, "starts": 50 })).unwrap()).unwrap();
    assert_eq!(b.status, Status::Ok, "{:?}", b.status);
    b
}

#[test]
fn latent_class_analysis() {
    let car = data("carcinoma.csv");
    let items = ["A", "B", "C", "D", "E", "F", "G"];
    let b = lca_fit(&car, &items, 3);
    assert!(compare(&b, "car3", 1e-6, 1e-4) > 50);
    assert!(paragraph(&b).unwrap().contains("Estimated class sizes were 44.5% (class 1), 37.4% (class 2), and 18.2% (class 3)."), "{}", paragraph(&b).unwrap());
    assert!(compare(&lca_fit(&car, &items, 2), "car2", 1e-6, 1e-4) > 40);
    let val = data("values.csv");
    assert!(compare(&lca_fit(&val, &["A", "B", "C", "D"], 2), "val2", 1e-6, 1e-4) > 20);
}

fn irt_fit(ds: &Dataset, items: &[&str], model: &str) -> ResultBundle {
    let b = prism_engine::analysis::irt::run(ds, &serde_json::from_value(serde_json::json!({ "items": items, "model": model })).unwrap()).unwrap();
    assert_eq!(b.status, Status::Ok, "{:?}", b.status);
    b
}

#[test]
fn item_response_models() {
    let lsat = data("lsat7.csv");
    let items = ["i1", "i2", "i3", "i4", "i5"];
    let rb = irt_fit(&lsat, &items, "rasch");
    // mirt's EM stops a little short on the Rasch model's variance (its log-likelihood is lower in the ninth
    // significant digit), so this model is compared at 2e-5 and PrismStatistics' log-likelihood must be no lower.
    assert!(rb.number("logl").unwrap() >= r()["rasch.logl"].as_f64().unwrap() - 1e-9);
    assert!(compare(&rb, "rasch", 2e-5, 2e-5) > 25);
    assert!(compare(&irt_fit(&lsat, &items, "graded"), "twopl", 1e-6, 1e-5) > 25);
    let sci = data("science.csv");
    let b = irt_fit(&sci, &["Comfort", "Work", "Future", "Benefit"], "graded");
    assert!(compare(&b, "grm", 1e-6, 1e-5) > 40);
    assert!(paragraph(&b).unwrap().starts_with("A graded response model was fitted to 4 items"), "{}", paragraph(&b).unwrap());
    assert_eq!(b.charts.len(), 2);
}

/// Bayesian SEM against blavaan. Both are MCMC, so each posterior summary is compared within a share of the posterior
/// standard deviation that covers both programs' Monte Carlo error (blavaan's reference used 15,000 draws).
fn bayes_compare(b: &ResultBundle, prefix: &str) -> usize {
    let r: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(reference_dir().join("bayes_sem_reference.json")).unwrap()).unwrap();
    let n = b.number("par.count").unwrap() as usize;
    let labels: Vec<String> = (1..=n).map(|k| b.text(&format!("par.{k}.label")).unwrap().replace(' ', "")).collect();
    let mut failures = Vec::new();
    let mut count = 0;
    for (key, want) in r.iter().filter(|(k, _)| k.starts_with(&format!("{prefix}."))) {
        let rest = &key[prefix.len() + 1..];
        let want = want.as_f64().unwrap();
        if let Some(m) = rest.strip_prefix("fit.") {
            let got = b.number(&format!("fit.{m}")).or_else(|| b.number(m));
            let tol = match m {
                "ppp" => 0.1,
                "dic" | "waic" | "looic" => 3.0,
                "p_dic" | "p_waic" => 2.0,
                _ => 0.0,
            };
            if m == "looic" {
                continue;
            }
            match got {
                Some(g) if (g - want).abs() <= tol => {}
                other => failures.push(format!("{key}: got {other:?}, blavaan gives {want}")),
            }
            count += 1;
            continue;
        }
        let (par, field) = rest.rsplit_once('.').unwrap();
        let Some(k) = labels.iter().position(|l| l == par) else {
            failures.push(format!("{key}: no parameter"));
            continue;
        };
        let sd = r[&format!("{prefix}.{par}.sd")].as_f64().unwrap();
        let got = b.number(&format!("par.{}.{}", k + 1, field)).unwrap();
        let tol = match field {
            "mean" | "sd" => 0.12,
            _ => 0.25,
        } * sd;
        if (got - want).abs() > tol {
            failures.push(format!("{key}: got {got}, blavaan gives {want} (posterior sd {sd})"));
        }
        count += 1;
    }
    assert!(failures.is_empty(), "{} values differ:\n{}", failures.len(), failures.join("\n"));
    count
}

#[test]
fn bayesian_sem() {
    let hs = data("holzinger.csv");
    let run = |model: &str| {
        let b = prism_engine::analysis::sem::run(&hs, &serde_json::from_value(serde_json::json!({ "model": model, "estimator": "bayes", "chains": 3, "burnin": 500, "samples": 1000, "seed": 11 })).unwrap()).unwrap();
        assert_eq!(b.status, Status::Ok, "{:?}", b.status);
        assert!(b.number("rhat.max").unwrap() < 1.02);
        b
    };
    assert!(bayes_compare(&run("visual =~ x1 + x2 + x3\ntextual =~ x4 + x5 + x6\nspeed =~ x7 + x8 + x9"), "bcfa") > 80);
    assert!(bayes_compare(&run("x4 ~ x1 + x2\nx9 ~ x4 + x1"), "bpath") >= 30);
}
