//! PLS-SEM against seminr (reference/pls_reference.json, made by make_pls_reference.R from seminr's mobi data): path
//! coefficients, R², loadings, weights, reliability, HTMT, f², total effects, VIF, the factorial scheme, and each group
//! of a two-group split; SRMR and Q²predict against independent R computations of their definitions.

use std::path::PathBuf;

use prism_engine::analysis::pls;
use prism_engine::import::read_file;
use prism_engine::model::Column;
use prism_engine::result::Status;
use prism_engine::templates::paragraph;
use prism_engine::{Dataset, ResultBundle};

fn reference_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("reference")
}

fn r() -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_str(&std::fs::read_to_string(reference_dir().join("pls_reference.json")).unwrap()).unwrap()
}

fn mobi() -> Dataset {
    let mut ds = read_file(&reference_dir().join("mobi.csv")).unwrap().0;
    let n = ds.n_rows();
    ds.add_variable("half", Column::Text((0..n).map(|i| Some(if i % 2 == 0 { "odd".to_string() } else { "even".to_string() })).collect())).unwrap();
    ds
}

const MODEL: &str = "Image =~ IMAG1 + IMAG2 + IMAG3 + IMAG4 + IMAG5
Expectation =~ CUEX1 + CUEX2 + CUEX3
Quality =~ PERQ1 + PERQ2 + PERQ3 + PERQ4 + PERQ5 + PERQ6 + PERQ7
Value =~ PERV1 + PERV2
Satisfaction =~ CUSA1 + CUSA2 + CUSA3
Complaints =~ CUSCO
Loyalty <~ CUSL1 + CUSL2 + CUSL3
Expectation ~ Image
Satisfaction ~ Image + Expectation + Quality + Value
Loyalty ~ Image + Satisfaction + Complaints
Quality ~ Expectation
Value ~ Expectation + Quality
Complaints ~ Satisfaction";

fn run(spec: serde_json::Value) -> ResultBundle {
    let b = pls::run(&mobi(), &serde_json::from_value(spec).unwrap()).unwrap();
    assert_eq!(b.status, Status::Ok, "{:?}", b.status);
    b
}

fn compare(b: &ResultBundle, prefix: &str, tol: f64) -> usize {
    let mut failures = Vec::new();
    let mut n = 0;
    for (key, want) in r().iter().filter(|(k, _)| k.starts_with(prefix)) {
        let field = &key[prefix.len()..];
        let want = want.as_f64().unwrap();
        let Some(got) = b.number(field) else {
            failures.push(format!("{key}: missing"));
            continue;
        };
        let err = (got - want).abs() / want.abs().max(1.0);
        if err > tol {
            failures.push(format!("{key}: got {got}, seminr gives {want} ({err:e})"));
        }
        n += 1;
    }
    assert!(failures.is_empty(), "{} values differ:\n{}", failures.len(), failures.join("\n"));
    n
}

#[test]
fn pls_path_model() {
    let b = run(serde_json::json!({ "model": MODEL }));
    // Everything seminr reports for the model (paths, R², loadings, weights, reliability, HTMT, f², total effects,
    // VIF), SRMR and Q²predict.
    assert!(compare(&b, "pls.", 1e-8) > 150);
    let chart = b.charts.iter().find(|c| c.id == "diagram").expect("a path diagram");
    assert!(chart.alt.contains("Loyalty is a composite formed from CUSL1 (") && chart.svg.contains(r#"data-param="Satisfaction ~ Quality""#), "{}", chart.alt);
    let text = paragraph(&b).unwrap();
    assert!(text.contains("Image → Expectation (β = .51)") && text.contains("SRMR (saturated model) was .056"), "{text}");
}

#[test]
fn factorial_scheme_and_groups() {
    let b = run(serde_json::json!({ "model": MODEL, "scheme": "factorial", "folds": 0 }));
    assert!(compare(&b, "plsf.", 1e-8) >= 12);
    let g = run(serde_json::json!({ "model": MODEL, "group": "half", "permutations": 100, "folds": 0 }));
    let mut n = 0;
    for (key, want) in r().iter().filter(|(k, _)| k.starts_with("mga.")) {
        let got = g.number(key).unwrap_or_else(|| panic!("no {key}"));
        assert!((got - want.as_f64().unwrap()).abs() < 1e-8, "{key}: {got} vs {want}");
        n += 1;
    }
    assert_eq!(n, 24);
    let p = g.number("mga.p.Satisfaction.Loyalty").unwrap();
    assert!(p > 0.0 && p <= 1.0);
}

/// Bootstrap results can't match R's random draws: checked for sense against seminr's bootstrap standard errors
/// for the same model (bootstrap_model with 500 resamples gave about .06 for Image → Expectation).
#[test]
fn bootstrap() {
    let b = run(serde_json::json!({ "model": MODEL, "bootstrap": 300, "seed": 3, "folds": 0 }));
    assert_eq!(b.number("boot.used"), Some(300.0));
    let se = b.number("path.Image.Expectation.se").unwrap();
    assert!((0.03..0.09).contains(&se), "{se}");
    let (lo, hi) = (b.number("path.Image.Expectation.lower").unwrap(), b.number("path.Image.Expectation.upper").unwrap());
    let est = b.number("path.Image.Expectation").unwrap();
    assert!(lo < est && est < hi);
    let again = run(serde_json::json!({ "model": MODEL, "bootstrap": 300, "seed": 3, "folds": 0 }));
    assert_eq!(again.number("path.Image.Expectation.se"), Some(se));
}

#[test]
fn model_errors() {
    let err = |m: &str| pls::run(&mobi(), &serde_json::from_value(serde_json::json!({ "model": m })).unwrap()).unwrap_err().to_string();
    assert!(err("A =~ IMAG1 + IMAG2\nB =~ CUEX1\nB ~ C").contains("not a construct"));
    assert!(err("A =~ IMAG1 + IMAG2\nB =~ CUEX1").contains("no structural paths"));
    assert!(err("A =~ IMAG1\nB =~ CUEX1\nB ~ A\nA ~ B").contains("feedback loop"));
    assert!(err("A =~ IMAG1\nB =~ IMAG1\nB ~ A").contains("more than one construct"));
}
