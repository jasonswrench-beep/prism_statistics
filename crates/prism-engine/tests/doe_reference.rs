//! Designed experiments against R (reference/doe_reference.json, made by make_doe_reference.R): fractional factorials
//! (FrF2: every setting in standard order, resolution, word-length pattern, aliases), full factorials (DoE.base),
//! central composite and Box-Behnken designs (rsm), factorial effects with Lenth's method (BsMD) and t tests (lm), and
//! response-surface analysis (rsm: coefficients, sequential ANOVA, lack of fit, stationary point, eigenvalues).

use std::path::PathBuf;

use prism_engine::analysis::doe;
use prism_engine::import::read_file;
use prism_engine::result::Status;
use prism_engine::templates::paragraph;
use prism_engine::{Dataset, ResultBundle};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("reference")
}

fn reference() -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_str(&std::fs::read_to_string(dir().join("doe_reference.json")).unwrap()).unwrap()
}

fn data(file: &str) -> Dataset {
    read_file(&dir().join(file)).unwrap().0
}

/// Compares every value under `prefix`; `map` turns R's value into ours (for level coding).
fn compare(b: &ResultBundle, prefix: &str, map: &dyn Fn(&str, f64) -> f64) -> usize {
    let r = reference();
    let (mut n, mut failures) = (0, Vec::new());
    for (key, want) in r.iter().filter(|(k, _)| k.starts_with(&format!("{prefix}."))) {
        let field = &key[prefix.len() + 1..];
        match want {
            serde_json::Value::String(w) => {
                if b.text(field).unwrap_or("") != w {
                    failures.push(format!("{key}: got {:?}, R gives {w:?}", b.text(field)));
                }
            }
            _ => {
                let want = map(field, want.as_f64().unwrap());
                // Word-length entries beyond the design's own words are zero.
                let got = b.number(field).or_else(|| field.starts_with("wlp.").then_some(0.0));
                match got {
                    Some(g) if (g - want).abs() / want.abs().max(1.0) <= 1e-9 => {}
                    g => failures.push(format!("{key}: got {g:?}, R gives {want}")),
                }
            }
        }
        n += 1;
    }
    assert!(failures.is_empty(), "{} values differ:\n{}", failures.len(), failures.iter().take(30).cloned().collect::<Vec<_>>().join("\n"));
    eprintln!("{prefix}: {n} values");
    n
}

fn design(spec: serde_json::Value) -> ResultBundle {
    let b = doe::run(&Dataset::new("none"), &serde_json::from_value(spec).unwrap()).unwrap();
    assert_eq!(b.status, Status::Ok, "{:?}", b.status);
    b
}

fn factors(k: usize) -> Vec<serde_json::Value> {
    (0..k).map(|_| serde_json::json!({ "name": "" })).collect()
}

#[test]
fn fractional_factorials() {
    for (runs, k) in [(4, 3), (8, 4), (8, 5), (8, 7), (16, 6), (16, 8), (32, 7), (64, 10)] {
        let b = design(serde_json::json!({ "kind": "fractional", "factors": factors(k), "runs": runs }));
        assert!(compare(&b, &format!("frf2_{runs}_{k}"), &|_, v| v) > 10, "{runs} {k}");
    }
}

#[test]
fn full_factorial_and_response_surface_designs() {
    let b = design(serde_json::json!({ "kind": "full", "factors": [{ "name": "A", "levels": 2 }, { "name": "B", "levels": 3 }, { "name": "C", "levels": 2 }] }));
    // fac.design codes two-level factors 1 and 2; here they are −1 and +1.
    compare(&b, "full_2_3_2", &|f, v| if f.starts_with("x.") && !f.ends_with(".2") { 2.0 * v - 3.0 } else { v });
    let b = design(serde_json::json!({ "kind": "ccd", "factors": factors(3), "center": 4, "center_axial": 2 }));
    let blocks = |b: &ResultBundle, p: &str| {
        // Blocks are in the run sheet, not the bundle's numbers: check them through the CSV.
        let spec: doe::Spec = serde_json::from_value(b.settings.clone()).unwrap();
        let d = doe::build(&spec).unwrap();
        let r = reference();
        for (i, bl) in d.blocks.as_ref().unwrap().iter().enumerate() {
            assert_eq!(*bl as f64, r[&format!("{p}.block.{}", i + 1)].as_f64().unwrap(), "{p} block {i}");
        }
    };
    let noblock = |b: &ResultBundle, p: &str| -> usize {
        let r = reference();
        let mut n = 0;
        for (key, want) in r.iter().filter(|(k, _)| k.starts_with(&format!("{p}.x.")) || *k == &format!("{p}.runs") || *k == &format!("{p}.alpha")) {
            let field = &key[p.len() + 1..];
            let got = b.number(field).unwrap_or_else(|| panic!("{key}"));
            assert!((got - want.as_f64().unwrap()).abs() < 1e-9, "{key}: {got} vs {want}");
            n += 1;
        }
        n
    };
    assert!(noblock(&b, "ccd3") > 50);
    blocks(&b, "ccd3");
    let b = design(serde_json::json!({ "kind": "ccd", "factors": factors(2), "center": 3, "alpha": "rotatable" }));
    assert!(noblock(&b, "ccd2r") > 20);
    let b = design(serde_json::json!({ "kind": "ccd", "factors": factors(4), "center": 2, "alpha": "face" }));
    assert!(noblock(&b, "ccd4f") > 100);
    for k in 3..=5 {
        let b = design(serde_json::json!({ "kind": "bbd", "factors": factors(k), "center": 2 }));
        assert!(noblock(&b, &format!("bbd{k}")) > 40);
        if k > 3 {
            blocks(&b, &format!("bbd{k}"));
        }
    }
}

#[test]
fn factorial_effects() {
    let fil = data("filtration.csv");
    let b = doe::effects(&fil, &serde_json::from_value(serde_json::json!({ "response": "rate", "factors": ["A", "B", "C", "D"] })).unwrap()).unwrap();
    assert!(compare(&b, "fil", &|_, v| v) >= 18);
    let text = paragraph(&b).unwrap();
    assert!(text.contains("Lenth"), "{text}");
    // Replicated, with a categorical factor ("lo" coded +1, as the reference codes it: categories in sorted order).
    let rep = data("replicated.csv");
    let b = doe::effects(&rep, &serde_json::from_value(serde_json::json!({ "response": "y", "factors": ["A", "B", "C"] })).unwrap()).unwrap();
    assert!(compare(&b, "rep", &|_, v| v) >= 28);
}

#[test]
fn response_surface() {
    let cr = data("chemreact.csv");
    let b = doe::response_surface(&cr, &serde_json::from_value(serde_json::json!({ "response": "Yield", "factors": ["x1", "x2"], "block": "Block" })).unwrap()).unwrap();
    assert_eq!(b.status, Status::Ok);
    assert!(compare(&b, "rsm", &|_, v| v) > 30);
    let text = paragraph(&b).unwrap();
    assert!(text.contains("stationary point"), "{text}");
}

#[test]
fn plackett_burman_designs() {
    for (runs, k) in [(12, 11), (12, 7), (20, 19), (20, 10), (24, 23), (36, 30), (44, 43), (48, 47)] {
        let b = design(serde_json::json!({ "kind": "pb", "factors": factors(k), "runs": runs }));
        assert!(compare(&b, &format!("pb_{runs}_{k}"), &|_, v| v) > runs * k, "{runs} {k}");
    }
    let text = paragraph(&design(serde_json::json!({ "kind": "pb", "factors": factors(11), "runs": 12 }))).unwrap();
    assert!(text.contains("Plackett-Burman"), "{text}");
    assert!(doe::run(&Dataset::new("none"), &serde_json::from_value(serde_json::json!({ "kind": "pb", "factors": factors(12), "runs": 12 })).unwrap()).is_err());
}

#[test]
fn mixture_designs() {
    for (q, m) in [(3, 2), (3, 3), (4, 3), (5, 2), (4, 1)] {
        let b = design(serde_json::json!({ "kind": "lattice", "factors": factors(q), "degree": m }));
        assert!(compare(&b, &format!("sld_{q}_{m}"), &|_, v| v) > q, "{q} {m}");
    }
    let b = design(serde_json::json!({ "kind": "lattice", "factors": factors(3), "degree": 2, "augment": true }));
    assert!(compare(&b, "sld_3_2a", &|_, v| v) > 30);
    for (q, augment, p) in [(3, false, "scd_3"), (4, false, "scd_4"), (4, true, "scd_4a")] {
        let b = design(serde_json::json!({ "kind": "centroid", "factors": factors(q), "augment": augment }));
        assert!(compare(&b, p, &|_, v| v) > 20, "{p}");
    }
    let text = paragraph(&design(serde_json::json!({ "kind": "centroid", "factors": factors(3) }))).unwrap();
    assert!(text.contains("mixture"), "{text}");
}

#[test]
fn d_optimal_designs() {
    let r = reference();
    for (p, k, model, levels, n) in [("opt_q3", 3, doe::OptModel::Quadratic, 3, 14), ("opt_q2", 2, doe::OptModel::Quadratic, 3, 7), ("opt_q3b", 3, doe::OptModel::Quadratic, 3, 20), ("opt_i4", 4, doe::OptModel::Interactions, 2, 12), ("opt_l5", 5, doe::OptModel::Linear, 2, 9), ("opt_q4", 4, doe::OptModel::Quadratic, 3, 18)] {
        let want = |f: &str| r[&format!("{p}.{f}")].as_f64().unwrap();
        // Our criteria of AlgDesign's own design match its evaluation.
        let grid: Vec<Vec<f64>> = (0..(levels as usize).pow(k as u32))
            .map(|i| (0..k).map(|j| -1.0 + 2.0 * ((i / (levels as usize).pow(j as u32)) % levels as usize) as f64 / (levels - 1) as f64).collect())
            .collect();
        let cand: Vec<Vec<f64>> = grid.iter().map(|x| doe::model_row(model, x)).collect();
        let rows: Vec<Vec<f64>> = (1..=n).map(|i| doe::model_row(model, &(1..=k).map(|j| r[&format!("{p}.x.{i}.{j}")].as_f64().unwrap()).collect::<Vec<_>>())).collect();
        let c = doe::criteria(&rows, &cand).unwrap();
        assert!((c.d - want("eval.d")).abs() < 1e-9 && (c.a - want("eval.a")).abs() < 1e-9, "{p}: {c:?}");
        // Our search does at least as well as AlgDesign's best of 300.
        let model_s = serde_json::to_value(model).unwrap();
        let b = design(serde_json::json!({ "kind": "optimal", "factors": factors(k), "model": model_s, "runs": n }));
        assert_eq!(b.number("runs"), Some(n as f64));
        let d = b.number("opt.d").unwrap();
        assert!(d >= want("d") * (1.0 - 1e-9), "{p}: D {d} below AlgDesign's {}", want("d"));
        eprintln!("{p}: D {d} (AlgDesign {}), A {} ({}), G {}", want("d"), b.number("opt.a").unwrap(), want("a"), b.number("opt.g").unwrap());
    }
    // Eight runs for a linear model in three two-level factors: the full factorial, with D = G = 1.
    let b = design(serde_json::json!({ "kind": "optimal", "factors": factors(3), "model": "linear", "runs": 8 }));
    assert!((b.number("opt.d").unwrap() - 1.0).abs() < 1e-12 && (b.number("opt.g").unwrap() - 1.0).abs() < 1e-12);
    let text = paragraph(&b).unwrap();
    assert!(text.contains("D-optimal"), "{text}");
}

#[test]
fn mixture_models() {
    let yarn = data("yarn.csv");
    let fit = |d: &Dataset, spec: serde_json::Value| doe::mixture(d, &serde_json::from_value(spec).unwrap()).unwrap();
    let b = fit(&yarn, serde_json::json!({ "response": "elongation", "components": ["x1", "x2", "x3"] }));
    assert_eq!(b.status, Status::Ok);
    assert!(compare(&b, "yarn", &|_, v| v) >= 30);
    let text = paragraph(&b).unwrap();
    assert!(text.contains("mixture"), "{text}");
    let b = fit(&yarn, serde_json::json!({ "response": "elongation", "components": ["x1", "x2", "x3"], "model": "linear" }));
    assert!(compare(&b, "yarnl", &|_, v| v) >= 20);
    // Components in percent.
    let blend = data("blend.csv");
    let b = fit(&blend, serde_json::json!({ "response": "y", "components": ["a", "b", "c"], "model": "special_cubic" }));
    assert!(compare(&b, "blend", &|_, v| v) >= 38);
}

#[test]
fn i_optimal_designs() {
    let r = reference();
    for (p, k, model, levels, n) in [("iopt_q2", 2, doe::OptModel::Quadratic, 3usize, 8), ("iopt_q3", 3, doe::OptModel::Quadratic, 3, 14), ("iopt_i4", 4, doe::OptModel::Interactions, 2, 14)] {
        let want = |f: &str| r[&format!("{p}.{f}")].as_f64().unwrap();
        let grid: Vec<Vec<f64>> = (0..levels.pow(k as u32)).map(|i| (0..k).map(|j| -1.0 + 2.0 * ((i / levels.pow(j as u32)) % levels) as f64 / (levels - 1) as f64).collect()).collect();
        let cand: Vec<Vec<f64>> = grid.iter().map(|x| doe::model_row(model, x)).collect();
        let rows: Vec<Vec<f64>> = (1..=n).map(|i| doe::model_row(model, &(1..=k).map(|j| r[&format!("{p}.x.{i}.{j}")].as_f64().unwrap()).collect::<Vec<_>>())).collect();
        let c = doe::criteria(&rows, &cand).unwrap();
        assert!((c.i - want("eval.i")).abs() < 1e-9, "{p}: I of AlgDesign's design {} vs {}", c.i, want("eval.i"));
        let b = design(serde_json::json!({ "kind": "optimal", "criterion": "I", "factors": factors(k), "model": serde_json::to_value(model).unwrap(), "runs": n }));
        let i = b.number("opt.i").unwrap();
        assert!(i <= want("i") * (1.0 + 1e-9), "{p}: I {i} above AlgDesign's {}", want("i"));
        eprintln!("{p}: I {i} (AlgDesign {})", want("i"));
    }
    let text = paragraph(&design(serde_json::json!({ "kind": "optimal", "criterion": "I", "factors": factors(2), "runs": 8 }))).unwrap();
    assert!(text.contains("average prediction variance"), "{text}");
}

#[test]
fn constrained_mixture_designs() {
    let r = reference();
    for (p, lo, hi) in [("xv_flare", vec![0.4, 0.1, 0.1, 0.03], vec![0.6, 0.5, 0.5, 0.08]), ("xv_3", vec![0.1, 0.2, 0.1], vec![0.5, 0.6, 0.4]), ("xv_5", vec![0.05, 0.1, 0.0, 0.2, 0.1], vec![0.3, 0.4, 0.25, 0.5, 0.35])] {
        let q = lo.len();
        let fs: Vec<serde_json::Value> = (0..q).map(|j| serde_json::json!({ "name": "", "low": lo[j], "high": hi[j] })).collect();
        let b = design(serde_json::json!({ "kind": "vertices", "factors": fs }));
        let runs = r[&format!("{p}.runs")].as_f64().unwrap() as usize;
        assert_eq!(b.number("runs"), Some(runs as f64), "{p}");
        // The same vertices as cdd's, in any order.
        let ours: Vec<Vec<f64>> = (1..=runs).map(|i| (1..=q).map(|j| b.number(&format!("x.{i}.{j}")).unwrap()).collect()).collect();
        for i in 1..=runs {
            let v: Vec<f64> = (1..=q).map(|j| r[&format!("{p}.x.{i}.{j}")].as_f64().unwrap()).collect();
            assert!(ours.iter().any(|o| o.iter().zip(&v).all(|(a, c)| (a - c).abs() < 1e-12)), "{p}: cdd's vertex {v:?} is missing");
        }
        // Augmented: edge midpoints and the centroid, all within the bounds.
        let fs2 = fs.clone();
        let b = design(serde_json::json!({ "kind": "vertices", "factors": fs2, "augment": true }));
        let n = b.number("runs").unwrap() as usize;
        let edges = r[&format!("{p}.edges")].as_f64().unwrap() as usize;
        assert_eq!(n, runs + edges + 1, "{p}: cdd finds {edges} edges");
        for i in 1..=n {
            let x: Vec<f64> = (1..=q).map(|j| b.number(&format!("x.{i}.{j}")).unwrap()).collect();
            assert!((x.iter().sum::<f64>() - 1.0).abs() < 1e-12 && (0..q).all(|j| x[j] >= lo[j] - 1e-12 && x[j] <= hi[j] + 1e-12), "{p}: {x:?}");
        }
    }
    // Lower bounds only: a lattice in pseudo-components.
    let fs: Vec<serde_json::Value> = [0.2, 0.1, 0.1].iter().map(|l| serde_json::json!({ "name": "", "low": l })).collect();
    let b = design(serde_json::json!({ "kind": "lattice", "factors": fs, "degree": 2 }));
    assert_eq!(b.number("x.1.1"), Some(0.8));
    assert!((b.number("x.2.1").unwrap() - 0.5).abs() < 1e-12 && (b.number("x.2.2").unwrap() - 0.4).abs() < 1e-12);
}
