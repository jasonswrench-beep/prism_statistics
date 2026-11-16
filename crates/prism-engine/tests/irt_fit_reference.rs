//! Item response model fit against R's mirt (reference/irt_fit_reference.json, made by make_irt_fit_reference.R):
//! M2 (M2* for ordered items) with RMSEA and its 90% interval, SRMSR, TLI and CFI, and each item's S-X² with its df,
//! RMSEA and p value, for the Rasch and 2PL models on LSAT7 and the graded response model on Science and on bfi3.
//!
//! PrismStatistics's estimates agree with mirt's to about 1e-6 (latent_reference.rs), so the fit statistics, which are
//! computed from them, are compared at 1e-4 relative; the ends of mirt's RMSEA interval come from R's uniroot, whose
//! default tolerance is about 1e-4, so they are compared at 1e-3.

use std::path::PathBuf;

use prism_engine::analysis::irt;
use prism_engine::import::read_file;
use prism_engine::ResultBundle;

fn reference_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("reference")
}

fn r() -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_str(&std::fs::read_to_string(reference_dir().join("irt_fit_reference.json")).unwrap()).unwrap()
}

fn fit(file: &str, model: &str) -> ResultBundle {
    let ds = read_file(&reference_dir().join(file)).unwrap().0;
    let items: Vec<String> = ds.variables.iter().map(|v| v.name.clone()).collect();
    let b = irt::run(&ds, &serde_json::from_value(serde_json::json!({ "items": items, "model": model })).unwrap()).unwrap();
    assert!(b.is_ok(), "{file}: {:?}", b.status);
    b
}

fn compare(b: &ResultBundle, prefix: &str) -> usize {
    let mut failures = Vec::new();
    let mut n = 0;
    for (key, want) in r().iter().filter(|(k, _)| k.starts_with(&format!("{prefix}."))) {
        let field = &key[prefix.len() + 1..];
        let field = if field == "m2.m2" { "m2" } else { field };
        let want = want.as_f64().unwrap();
        n += 1;
        let Some(got) = b.number(field) else {
            failures.push(format!("{key}: missing"));
            continue;
        };
        let tol = if field.starts_with("m2.rmsea_") { 1e-3 } else { 1e-4 };
        let err = (got - want).abs() / want.abs().max(if field.contains(".p") { 1e-3 } else { 1.0 });
        if err > tol {
            failures.push(format!("{key}: got {got}, mirt gives {want} ({err:e})"));
        }
    }
    assert!(failures.is_empty(), "{prefix}: {} of {n} differ:\n{}", failures.len(), failures.join("\n"));
    n
}

#[test]
fn rasch_and_2pl_fit_match_mirt() {
    assert!(compare(&fit("lsat7.csv", "rasch"), "rasch") >= 29);
    assert!(compare(&fit("lsat7.csv", "graded"), "twopl") >= 29);
}

#[test]
fn graded_model_fit_matches_mirt() {
    let sci = fit("science.csv", "graded");
    assert!(compare(&sci, "grm") >= 16);
    // Science has 10 margins and 16 parameters, so M2* has no degrees of freedom; mirt stops, and so does this.
    assert!(sci.number("m2").is_none());
    assert!(sci.diagnostics.iter().any(|d| d.id == "fit" && d.message.contains("could not be computed")));
    assert!(compare(&fit("bfi3.csv", "graded"), "bfi3") >= 35);
}
