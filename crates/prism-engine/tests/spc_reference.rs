//! Control charts and process capability against qcc 2.7 (reference/spc_reference.json, made by make_spc_reference.R):
//! every plotted statistic and limit, the center lines, σ, the points flagged beyond the limits and in runs, and the
//! capability indices with their intervals.

use std::path::PathBuf;

use prism_engine::analysis::spc;
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
    let b = spc::run(ds, &serde_json::from_value(spec).unwrap()).unwrap();
    assert_eq!(b.status, Status::Ok, "{:?}", b.status);
    b
}

/// Compares the values under `prefix` (qcc's key `prefix.<rest>` against ours `<rest>`), numbers and flagged points.
fn compare(b: &ResultBundle, prefix: &str) -> usize {
    let r: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(dir().join("spc_reference.json")).unwrap()).unwrap();
    let (mut n, mut failures) = (0, Vec::new());
    for (key, want) in r.iter().filter(|(k, _)| k.starts_with(&format!("{prefix}."))) {
        let field = &key[prefix.len() + 1..];
        match want {
            serde_json::Value::String(w) => {
                // qcc labels moving range k by the pair (k, k + 1); here it is labeled by the observation it ends at.
                let w = if field.starts_with("mr.") { w.split(", ").filter(|s| !s.is_empty()).map(|s| (s.parse::<usize>().unwrap() + 1).to_string()).collect::<Vec<_>>().join(", ") } else { w.clone() };
                if b.text(field).unwrap_or("") != w {
                    failures.push(format!("{key}: got {:?}, qcc gives {w:?}", b.text(field)));
                }
            }
            _ => {
                let want = want.as_f64().unwrap();
                match b.number(field) {
                    Some(got) if (got - want).abs() / want.abs().max(1.0) <= 1e-9 => {}
                    got => failures.push(format!("{key}: got {got:?}, qcc gives {want}")),
                }
            }
        }
        n += 1;
    }
    assert!(failures.is_empty(), "{} values differ:\n{}", failures.len(), failures.iter().take(30).cloned().collect::<Vec<_>>().join("\n"));
    eprintln!("{prefix}: {n} values");
    n
}

#[test]
fn xbar_r_and_s_charts() {
    for (file, p) in [("rings_trial.csv", "trial"), ("rings_all.csv", "all"), ("rings_unequal.csv", "unequal")] {
        let d = data(file);
        let b = run(&d, serde_json::json!({ "chart": "xbar_r", "measures": ["diameter"], "subgroup": "sample", "lsl": 73.95, "usl": 74.05 }));
        assert!(compare(&b, p) > 100, "{p}");
        let b = run(&d, serde_json::json!({ "chart": "xbar_s", "measures": ["diameter"], "subgroup": "sample" }));
        assert!(compare(&b, &format!("{p}s")) > 100, "{p}s");
    }
    // One row per subgroup, the measurements in five variables.
    let w = data("rings_wide.csv");
    let b = run(&w, serde_json::json!({ "chart": "xbar_r", "measures": ["d1", "d2", "d3", "d4", "d5"] }));
    for key in ["xbar.center", "std_dev", "xbar.ucl.1", "r.center", "r.ucl.1"] {
        let r: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(dir().join("spc_reference.json")).unwrap()).unwrap();
        let want = r[&format!("trial.{key}")].as_f64().unwrap();
        assert!((b.number(key).unwrap() - want).abs() < 1e-12, "{key}");
    }
}

#[test]
fn capability() {
    let d = data("rings_trial.csv");
    let b = run(&d, serde_json::json!({ "chart": "xbar_r", "measures": ["diameter"], "subgroup": "sample", "lsl": 73.95, "usl": 74.05 }));
    assert!(compare(&b, "trial") > 100);
    let b = run(&d, serde_json::json!({ "chart": "xbar_r", "measures": ["diameter"], "subgroup": "sample", "lsl": 73.95, "usl": 74.05, "target": 74.01 }));
    assert!(compare(&b, "trialt") >= 15);
    let text = paragraph(&b).unwrap();
    assert!(text.contains("control") && text.contains("Cpk"), "{text}");
}

#[test]
fn individuals_and_moving_range() {
    let d = data("rings_individuals.csv");
    let b = run(&d, serde_json::json!({ "chart": "imr", "measures": ["diameter"], "lsl": 73.97, "usl": 74.03 }));
    assert!(compare(&b, "imr") > 200);
}

#[test]
fn attribute_charts() {
    let oj = data("orangejuice.csv");
    assert!(compare(&run(&oj, serde_json::json!({ "chart": "p", "count": "D", "size": "size" })), "p") > 50);
    assert!(compare(&run(&oj, serde_json::json!({ "chart": "np", "count": "D", "size": "size" })), "np") > 50);
    let ov = data("orangejuice_varying.csv");
    assert!(compare(&run(&ov, serde_json::json!({ "chart": "p", "count": "D", "size": "size" })), "pv") > 50);
    let ci = data("circuit.csv");
    assert!(compare(&run(&ci, serde_json::json!({ "chart": "c", "count": "x" })), "c") > 50);
    let pc = data("pcmanufact.csv");
    assert!(compare(&run(&pc, serde_json::json!({ "chart": "u", "count": "x", "size": "size" })), "u") > 50);
}

#[test]
fn cusum_and_ewma() {
    let all = data("rings_all.csv");
    let sub = |chart: &str, extra: serde_json::Value| {
        let mut s = serde_json::json!({ "chart": chart, "measures": ["diameter"], "subgroup": "sample" });
        s.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        s
    };
    let b = run(&all, sub("cusum", serde_json::json!({})));
    assert!(compare(&b, "cusum") > 150);
    assert_eq!(b.charts.len(), 1);
    let text = paragraph(&b).unwrap();
    assert!(text.contains("CUSUM"), "{text}");
    let b = run(&all, sub("cusum", serde_json::json!({ "decision_interval": 4, "se_shift": 1.5, "head_start": 2, "center": 74, "std_dev": 0.01 })));
    assert!(compare(&b, "cusumo") > 150);
    let b = run(&all, sub("ewma", serde_json::json!({})));
    assert!(compare(&b, "ewma") > 120);
    let text = paragraph(&b).unwrap();
    assert!(text.contains("EWMA"), "{text}");
    let b = run(&all, sub("ewma", serde_json::json!({ "lambda": 0.3, "nsigmas": 2.7, "center": 74, "std_dev": 0.01 })));
    assert!(compare(&b, "ewmao") > 120);
    let ind = data("rings_individuals.csv");
    let b = run(&ind, serde_json::json!({ "chart": "cusum", "measures": ["diameter"] }));
    assert!(compare(&b, "cusumi") > 150);
    let b = run(&ind, serde_json::json!({ "chart": "ewma", "measures": ["diameter"] }));
    assert!(compare(&b, "ewmai") > 150);
    let uneq = data("rings_unequal.csv");
    let b = run(&uneq, sub("cusum", serde_json::json!({})));
    assert!(compare(&b, "cusumu") > 80);
    let b = run(&uneq, sub("ewma", serde_json::json!({})));
    assert!(compare(&b, "ewmau") >= 80);
}
