//! Latent profile analysis against mclust (reference/lpa_reference.json, made by make_lpa_reference.R): for every
//! covariance structure and number of classes on iris, Old Faithful and its eruption times alone. mclust starts EM
//! from a hierarchical clustering, PrismStatistics from many seeded starts, so the test is that our log-likelihood is
//! at least mclust's, and where both reach the same maximum, that the parameters, BIC and ICL agree.

use std::path::PathBuf;

use prism_engine::analysis::lpa;
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
    let b = lpa::run(ds, &serde_json::from_value(spec).unwrap()).unwrap();
    assert_eq!(b.status, Status::Ok, "{:?}", b.status);
    b
}

/// Returns (same maximum, better than mclust).
fn check(ds: &Dataset, indicators: &[&str], prefix: &str, model: &str, g: usize) -> (bool, bool) {
    let r: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(dir().join("lpa_reference.json")).unwrap()).unwrap();
    let key = format!("{prefix}.{model}.{g}");
    let Some(want_ll) = r.get(&format!("{key}.ll")).and_then(|v| v.as_f64()) else { return (false, false) };
    let structure = match model {
        "E" => "EEI",
        "V" => "VVI",
        m => m,
    };
    let b = run(ds, serde_json::json!({ "indicators": indicators, "min_classes": g, "max_classes": g, "models": [structure], "starts": 30 }));
    let ll = b.number("ll").unwrap();
    let tol = 1e-7 * want_ll.abs();
    assert!(ll >= want_ll - tol, "{key}: log-likelihood {ll} is below mclust's {want_ll}");
    if ll > want_ll + 1e-5 * want_ll.abs() {
        eprintln!("{key}: ours {ll} beats mclust's {want_ll}");
        return (false, true);
    }
    let mut failures = Vec::new();
    for (k, v) in r.iter().filter(|(k, _)| k.starts_with(&format!("{key}."))) {
        let field = &k[key.len() + 1..];
        let want = v.as_f64().unwrap();
        let got = match field {
            "ll" | "bic" | "npar" | "icl" => b.number(&format!("fit.{model}.{g}.{field}")),
            f => b.number(f),
        };
        // At a flat maximum the parameters settle to about 1e-5 in either program.
        let tol = if field.starts_with("class.") { 1e-4 } else if field == "icl" { 1e-5 } else { 1e-7 };
        match got {
            Some(x) if (x - want).abs() <= tol * want.abs().max(1.0) => {}
            x => failures.push(format!("{k}: got {x:?}, mclust gives {want}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    (true, false)
}

#[test]
fn iris_every_structure() {
    let ir = data("iris.csv");
    let ind = ["sepal_length", "sepal_width", "petal_length", "petal_width"];
    let (mut same, mut better) = (0, 0);
    for model in ["EEI", "VVI", "EEE", "VVV"] {
        for g in 1..=4 {
            let (s, b) = check(&ir, &ind, "iris", model, g);
            same += s as usize;
            better += b as usize;
        }
    }
    eprintln!("iris: {same} at mclust's maximum, {better} higher");
    assert_eq!(same + better, 16);
}

#[test]
fn faithful_and_one_indicator() {
    let f = data("faithful.csv");
    let (mut same, mut better) = (0, 0);
    for model in ["EEI", "VVI", "EEE", "VVV"] {
        for g in 1..=4 {
            let (s, b) = check(&f, &["eruptions", "waiting"], "faith", model, g);
            same += s as usize;
            better += b as usize;
        }
    }
    for model in ["E", "V"] {
        for g in 1..=3 {
            let (s, b) = check(&f, &["eruptions"], "erupt", model, g);
            same += s as usize;
            better += b as usize;
        }
    }
    eprintln!("faithful: {same} at mclust's maximum, {better} higher");
    assert_eq!(same + better, 22);
    // The default run chooses by BIC; mclust's best for Old Faithful is EEE with 3 classes.
    let b = run(&f, serde_json::json!({ "indicators": ["eruptions", "waiting"], "max_classes": 4 }));
    assert_eq!((b.text("model"), b.number("classes")), (Some("EEE"), Some(3.0)));
    let text = paragraph(&b).unwrap();
    assert!(text.contains("latent profile") && text.contains("BIC"), "{text}");
}
