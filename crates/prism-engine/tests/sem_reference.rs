//! Structural equation models against lavaan (reference/sem_reference.json, made by make_sem_reference.R): every
//! parameter estimate, standard error, standardized estimate and its standard error, fit measure, R² and the chosen
//! modification indices.

use std::path::PathBuf;

use prism_engine::analysis::sem;
use prism_engine::import::read_file;
use prism_engine::result::Status;
use prism_engine::templates::paragraph;
use prism_engine::{Dataset, ResultBundle};

fn reference_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("reference")
}

fn r() -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_str(&std::fs::read_to_string(reference_dir().join("sem_reference.json")).unwrap()).unwrap()
}

fn data(file: &str) -> Dataset {
    read_file(&reference_dir().join(file)).unwrap().0
}

const HS: &str = "visual =~ x1 + x2 + x3
textual =~ x4 + x5 + x6
speed =~ x7 + x8 + x9";

const PD: &str = "ind60 =~ x1 + x2 + x3
dem60 =~ y1 + a*y2 + b*y3 + c*y4
dem65 =~ y5 + a*y6 + b*y7 + c*y8
dem60 ~ g1*ind60
dem65 ~ g2*ind60 + g3*dem60
y1 ~~ y5
y2 ~~ y4 + y6
y3 ~~ y7
y4 ~~ y8
y6 ~~ y8
indirect := g1*g3
total := g2 + g1*g3";

fn fit(ds: &Dataset, spec: serde_json::Value) -> ResultBundle {
    let b = sem::run(ds, &serde_json::from_value(spec).unwrap()).unwrap();
    assert_eq!(b.status, Status::Ok, "{:?}", b.status);
    b
}

/// Compares every reference value under `prefix` and returns how many were compared.
fn compare(b: &ResultBundle, prefix: &str, tol_est: f64, tol_se: f64) -> usize {
    let r = r();
    let n = b.number("par.count").unwrap() as usize;
    // Multigroup parameters are keyed by group, as "Pasteur:visual=~x1".
    let labels: Vec<String> = (1..=n)
        .map(|k| {
            let l = b.text(&format!("par.{k}.label")).unwrap().replace(' ', "");
            b.text(&format!("par.{k}.group")).map_or(l.clone(), |g| format!("{g}:{l}"))
        })
        .collect();
    let mis = b.number("mi.count").unwrap_or(0.0) as usize;
    let mi_labels: Vec<String> = (1..=mis).map(|k| b.text(&format!("mi.{k}.label")).unwrap().replace(' ', "")).collect();
    let mut checked = 0;
    let mut failures: Vec<String> = Vec::new();
    let mut worst: Vec<(f64, String)> = Vec::new();
    // With missing data lavaan's robust CFI, TLI and RMSEA are the FIML-corrected ones of Zhang and Savalei (2020),
    // which PrismStatistics does not compute.
    // With ordinal data, lavaan's robust indices are the catML ones of Savalei (2021), which are not computed either;
    // its scale parameters (~*~) are fixed at 1 and its intercepts and means (~1) at 0 in the delta parameterization.
    let skip = |k: &str| {
        (prefix.starts_with("fiml") && k.ends_with(".robust"))
            || (prefix == "ord" && (k.ends_with(".robust") || k.contains("~*~") || k.contains("~1.") || k.contains(".polychoric.") || k.contains(".gamma.") || k.ends_with("fit.shift")))
    };
    for (key, want) in r.iter().filter(|(k, _)| k.starts_with(&format!("{prefix}.")) && !skip(k)) {
        let rest = &key[prefix.len() + 1..];
        let want = want.as_f64().unwrap();
        let (got, tol) = if let Some(m) = rest.strip_prefix("fit.") {
            let field = match m {
                "rmsea.ci.lower" => "fit.rmsea.lower".to_string(),
                m if m.ends_with(".robust") && m.starts_with("rmsea.ci") => format!("fit.{m}"),
                "rmsea.ci.upper" => "fit.rmsea.upper".to_string(),
                "npar" => "npar".to_string(),
                other => format!("fit.{other}"),
            };
            (b.number(&field).unwrap_or_else(|| panic!("no {field}")), if m.starts_with("rmsea.ci") || m == "rmsea.pvalue" { 1e-5 } else { tol_est })
        } else if rest.starts_with("rel.") {
            (b.number(rest).unwrap_or_else(|| panic!("no {rest}")), tol_est)
        } else if let Some(v) = rest.strip_prefix("r2.") {
            (b.number(&format!("r2.{v}")).unwrap_or_else(|| panic!("no r2.{v}")), tol_est)
        } else if let Some((kind, par)) = ["mi.", "epc.", "sepc."].iter().find_map(|p| rest.strip_prefix(p).map(|x| (*p, x))) {
            let k = mi_labels.iter().position(|l| l == par).unwrap_or_else(|| panic!("no modification index for {par}")) + 1;
            let f = match kind {
                "mi." => "mi",
                "epc." => "epc",
                _ => "sepc",
            };
            (b.number(&format!("mi.{k}.{f}")).unwrap(), tol_se)
        } else {
            let (par, field) = rest.rsplit_once('.').unwrap();
            let Some(k) = labels.iter().position(|l| l == par) else { panic!("{prefix}: no parameter {par}") };
            let got = b.number(&format!("par.{}.{field}", k + 1)).unwrap_or_else(|| panic!("{prefix}: no {field} for {par}"));
            (got, if field == "est" || field == "std" { tol_est } else { tol_se })
        };
        let err = (got - want).abs() / want.abs().max(1.0);
        worst.push((err, key.clone()));
        if err > tol {
            failures.push(format!("{key}: got {got}, lavaan gives {want} (relative error {err:e})"));
        }
        checked += 1;
    }
    assert!(failures.is_empty(), "{} of {checked} values differ:
{}", failures.len(), failures.join("
"));
    worst.sort_by(|a, c| c.0.total_cmp(&a.0));
    eprintln!("{prefix}: {checked} values, largest relative errors {:?}", &worst[..3.min(worst.len())]);
    checked
}

#[test]
fn confirmatory_factor_analysis() {
    let hs = data("holzinger.csv");
    let b = fit(&hs, serde_json::json!({ "model": HS }));
    assert!(compare(&b, "cfa", 1e-6, 1e-5) > 80);
    let b = fit(&hs, serde_json::json!({ "model": HS, "std_lv": true }));
    assert!(compare(&b, "cfastd", 1e-6, 1e-5) > 60);
    let b = fit(&hs, serde_json::json!({ "model": HS, "meanstructure": true }));
    assert!(compare(&b, "cfamean", 1e-6, 1e-5) > 80);
}

#[test]
fn higher_order_factor() {
    let hs = data("holzinger.csv");
    let b = fit(&hs, serde_json::json!({ "model": format!("{HS}\ng =~ visual + textual + speed") }));
    assert!(compare(&b, "ho", 1e-6, 1e-5) > 60);
}

#[test]
fn full_structural_model() {
    let pd = data("poldem.csv");
    let b = fit(&pd, serde_json::json!({ "model": PD }));
    // With the loadings held equal across waves the likelihood is very flat: lavaan's optimizers all stop where χ²
    // still changes in the eleventh digit and some estimates in the sixth or seventh. PrismStatistics reaches a lower
    // χ² (that is, a better maximum likelihood solution), so the estimates are compared to 1e-5 here.
    assert!(b.number("fit.chisq").unwrap() <= r()["pd.fit.chisq"].as_f64().unwrap() + 1e-12);
    assert!(compare(&b, "pd", 1e-5, 1e-5) > 100);
}

#[test]
fn path_analysis_with_fixed_x() {
    let hs = data("holzinger.csv");
    let b = fit(&hs, serde_json::json!({ "model": "x4 ~ a*x1 + x2\nx9 ~ b*x4 + c*x1\nx5 ~ x2\nab := a*b" }));
    assert!(compare(&b, "path", 1e-6, 1e-5) > 40);
    let b = fit(&hs, serde_json::json!({ "model": "x4 ~ a*x1 + x2
x9 ~ b*x4 + c*x1
x5 ~ x2
ab := a*b", "fixed_x": false }));
    assert!(compare(&b, "pathfree", 1e-6, 1e-5) > 50);
}

#[test]
fn model_errors_are_plain() {
    let hs = data("holzinger.csv");
    let err = |m: &str| sem::run(&hs, &serde_json::from_value(serde_json::json!({ "model": m })).unwrap()).unwrap_err().to_string();
    assert!(err("visual =~ x1 + nothere").contains("not a variable in the data"));
    assert!(err("x1 =~ x2 + x3").contains("can't also be a latent"));
    assert!(err("visual =~ x1 + x2\nab := q*2").contains("not a label"));
    // Two indicators and a free variance: not identified.
    let b = sem::run(&hs, &serde_json::from_value(serde_json::json!({ "model": "f =~ NA*x1 + x2\nf ~~ f" })).unwrap()).unwrap();
    assert!(matches!(b.status, Status::Failed { .. }));
}

#[test]
fn robust_maximum_likelihood() {
    let hs = data("holzinger.csv");
    let b = fit(&hs, serde_json::json!({ "model": HS, "estimator": "mlm" }));
    assert!(compare(&b, "mlm", 1e-6, 1e-5) > 90);
    let text = paragraph(&b).unwrap();
    assert!(text.contains("Satorra-Bentler") && text.contains("χ²(24) = 80.87") && text.contains("robust CFI = .932") && text.contains("90% CI [.070, .113]"), "{text}");
    let b = fit(&hs, serde_json::json!({ "model": HS, "estimator": "mlm", "h1_information": "unstructured" }));
    assert!(compare(&b, "mlmu", 1e-6, 1e-5) > 90);
    let b = fit(&hs, serde_json::json!({ "model": HS, "estimator": "mlr" }));
    assert!(compare(&b, "mlr", 1e-6, 1e-5) > 90);
    let pd = data("poldem.csv");
    let b = fit(&pd, serde_json::json!({ "model": PD, "estimator": "mlr" }));
    assert!(compare(&b, "pdmlr", 1e-5, 1e-5) > 100);
}

#[test]
fn full_information_maximum_likelihood() {
    let hm = data("holzinger_missing.csv");
    // As with the political democracy model, lavaan stops slightly short of the maximum (its χ² is higher in the tenth
    // digit), so estimates are compared to 1e-5 and PrismStatistics' χ² must be no higher.
    let b = fit(&hm, serde_json::json!({ "model": HS, "missing": "fiml" }));
    assert_eq!(b.number("n").unwrap(), 301.0);
    assert!(b.number("fit.chisq").unwrap() <= r()["fiml.fit.chisq"].as_f64().unwrap() + 1e-12);
    assert!(compare(&b, "fiml", 1e-5, 1e-5) > 100);
    assert!(paragraph(&b).unwrap().starts_with("A confirmatory factor analysis was estimated by full-information maximum likelihood"));
    let b = fit(&hm, serde_json::json!({ "model": HS, "missing": "fiml", "estimator": "mlr" }));
    assert!(compare(&b, "fimlr", 1e-5, 1e-5) > 100);
}

#[test]
fn multigroup_models() {
    let hs = data("holzinger.csv");
    for (step, equal) in [("configural", vec![]), ("metric", vec!["loadings"]), ("scalar", vec!["loadings", "intercepts"]), ("strict", vec!["loadings", "intercepts", "residuals"])] {
        let b = fit(&hs, serde_json::json!({ "model": HS, "group": "school", "group_equal": equal }));
        assert_eq!(b.text("group.1.label"), Some("Pasteur"));
        assert!(b.number("fit.chisq").unwrap() <= r()[&format!("mg{step}.fit.chisq")].as_f64().unwrap() + 1e-9, "{step}");
        assert!(compare(&b, &format!("mg{step}"), 1e-5, 1e-5) > 150, "{step}");
    }
    let b = fit(&hs, serde_json::json!({ "model": HS, "group": "school", "group_equal": ["loadings", "intercepts"], "group_partial": ["x3 ~1"] }));
    assert!(compare(&b, "mgpartial", 1e-5, 1e-5) > 150);
    let b = fit(&hs, serde_json::json!({ "model": HS, "group": "school", "group_equal": ["loadings"], "std_lv": true }));
    assert!(compare(&b, "mgstd", 1e-5, 1e-5) > 150);
    let b = fit(&hs, serde_json::json!({ "model": HS, "group": "school", "group_equal": ["loadings", "intercepts"], "estimator": "mlr" }));
    for k in ["chisq.scaled", "chisq.scaling.factor", "cfi.robust", "rmsea.robust"] {
        let (got, want) = (b.number(&format!("fit.{k}")).unwrap(), r()[&format!("mgrscalar.fit.{k}")].as_f64().unwrap());
        assert!((got - want).abs() / want.abs().max(1.0) < 1e-6, "{k}: {got} vs {want}");
    }
}

#[test]
fn reliability_and_validity() {
    let hs = data("holzinger.csv");
    let b = fit(&hs, serde_json::json!({ "model": HS }));
    let r = r();
    let mut n = 0;
    for (k, want) in r.iter().filter(|(k, _)| k.starts_with("rel.")) {
        let got = b.number(k).unwrap_or_else(|| panic!("no {k}"));
        assert!((got - want.as_f64().unwrap()).abs() < 1e-6, "{k}: {got} vs {want}");
        n += 1;
    }
    assert_eq!(n, 12);
}

#[test]
fn measurement_invariance() {
    let hs = data("holzinger.csv");
    let run = |est: &str| sem::invariance::run(&hs, &serde_json::from_value(serde_json::json!({ "model": HS, "group": "school", "estimator": est })).unwrap()).unwrap();
    let r = r();
    let close = |got: f64, key: &str, tol: f64| {
        let want = r[key].as_f64().unwrap();
        assert!((got - want).abs() / want.abs().max(1.0) <= tol, "{key}: {got} vs {want}");
    };
    let b = run("ml");
    assert_eq!(b.number("steps"), Some(4.0));
    let text = paragraph(&b).unwrap();
    assert!(text.contains("metric invariance was not rejected (Δχ²(6) = 8.19, *p* = .224; ΔCFI = −.002") && text.contains("scalar invariance was rejected"), "{text}");
    for (k, step) in ["configural", "metric", "scalar", "strict"].iter().enumerate() {
        let key = format!("step.{}", k + 1);
        close(b.number(&format!("{key}.chisq")).unwrap(), &format!("mg{step}.fit.chisq"), 1e-8);
        close(b.number(&format!("{key}.cfi")).unwrap(), &format!("mg{step}.fit.cfi"), 1e-8);
        if k > 0 {
            close(b.number(&format!("{key}.dchisq")).unwrap(), &format!("inv.{step}.dchisq"), 1e-7);
            close(b.number(&format!("{key}.ddf")).unwrap(), &format!("inv.{step}.ddf"), 0.0);
            close(b.number(&format!("{key}.p")).unwrap(), &format!("inv.{step}.p"), 1e-7);
        }
    }
    let b = run("mlr");
    for (k, step) in ["configural", "metric", "scalar", "strict"].iter().enumerate() {
        let key = format!("step.{}", k + 1);
        close(b.number(&format!("{key}.cfi")).unwrap(), &format!("mgr{step}.fit.cfi.robust"), 1e-6);
        // The scaled difference divides by (df₁c₁ − df₀c₀)/Δdf, a small difference of two large products, which
        // multiplies the 1e-6 differences in the scaling factors (from lavaan's looser optimum) about ninefold.
        if k > 0 {
            close(b.number(&format!("{key}.dchisq")).unwrap(), &format!("invr.{step}.dchisq"), 2e-5);
            close(b.number(&format!("{key}.p")).unwrap(), &format!("invr.{step}.p"), 2e-5);
        }
    }
}

/// Bootstrap intervals use resampling that can't reproduce R's random draws, so they are checked for sense: the
/// bootstrap standard errors agree with the robust (MLR) ones within sampling error (these data are not normal, and
/// lavaan's bootstrap SEs track its MLR ones the same way), the intervals contain the estimate, the indirect effect's
/// interval is not symmetric, and the same seed gives the same result.
#[test]
fn bootstrap_intervals() {
    let hs = data("holzinger.csv");
    let model = "x4 ~ a*x1 + x2\nx9 ~ b*x4 + c*x1\nx5 ~ x2\nab := a*b";
    let plain = fit(&hs, serde_json::json!({ "model": model, "estimator": "mlr" }));
    let boot = fit(&hs, serde_json::json!({ "model": model, "bootstrap": 500, "seed": 7 }));
    assert_eq!(boot.number("boot.used"), Some(500.0));
    let n = boot.number("par.count").unwrap() as usize;
    for k in 1..=n {
        let key = format!("par.{k}");
        let (Some(se_b), Some(se_a)) = (boot.number(&format!("{key}.se")), plain.number(&format!("{key}.se"))) else { continue };
        let ratio = se_b / se_a;
        assert!((0.85..1.15).contains(&ratio), "{}: bootstrap SE {se_b} vs robust {se_a}", boot.text(&format!("{key}.label")).unwrap());
        let est = boot.number(&format!("{key}.est")).unwrap();
        assert!(boot.number(&format!("{key}.lower")).unwrap() < est && est < boot.number(&format!("{key}.upper")).unwrap());
    }
    let ab = (1..=n).find(|k| boot.text(&format!("par.{k}.label")).unwrap().starts_with("ab :=")).unwrap();
    let (est, lo, hi) = (boot.number(&format!("par.{ab}.est")).unwrap(), boot.number(&format!("par.{ab}.lower")).unwrap(), boot.number(&format!("par.{ab}.upper")).unwrap());
    assert!(((est - lo) - (hi - est)).abs() > 1e-6, "an indirect effect's percentile interval is not symmetric");
    let again = fit(&hs, serde_json::json!({ "model": model, "bootstrap": 500, "seed": 7 }));
    assert_eq!(again.number(&format!("par.{ab}.lower")), Some(lo));
    let bc = fit(&hs, serde_json::json!({ "model": model, "bootstrap": 500, "seed": 7, "bias_corrected": true }));
    assert_ne!(bc.number(&format!("par.{ab}.lower")), Some(lo));
    assert!(bc.method_note.contains("bias-corrected percentile intervals are from 500 bootstrap resamples"));
}

#[test]
fn ordinal_indicators_by_wlsmv() {
    let ho = data("holzinger_ordinal.csv");
    let r = r();
    // The sample statistics: polychoric correlations, and entries of their asymptotic covariance Γ (thresholds first,
    // then correlations column by column, as lavaan orders them).
    let cats: Vec<Vec<usize>> = (1..=9)
        .map(|i| ho.numeric_cells(&format!("x{i}")).unwrap().iter().map(|c| match c { prism_engine::model::NumericCell::Value(v) => *v as usize - 1, _ => panic!("complete") }).collect())
        .collect();
    let smp = sem::ordinal::sample(&cats, &[4; 9]).unwrap();
    for (k, want) in r.iter().filter(|(k, _)| k.starts_with("ord.polychoric.")) {
        let pair: Vec<&str> = k["ord.polychoric.".len()..].split('.').collect();
        let (j, i) = (pair[0][1..].parse::<usize>().unwrap() - 1, pair[1][1..].parse::<usize>().unwrap() - 1);
        // lavaan's polychoric optimizer stops within about 3e-8 of the maximum (for x6 and x7 an independent
        // maximization with mvtnorm agrees with PrismStatistics to 2e-9), so 1e-7 here.
        assert!((smp.r[i][j] - want.as_f64().unwrap()).abs() < 1e-7, "{k}: {} vs {want}", smp.r[i][j]);
    }
    for (a, c) in [(1, 1), (28, 28), (28, 29), (1, 28)] {
        let want = r[&format!("ord.gamma.{a}.{c}")].as_f64().unwrap();
        assert!((smp.gamma[a - 1][c - 1] - want).abs() < 1e-6 * want.abs().max(1.0), "gamma {a},{c}: {} vs {want}", smp.gamma[a - 1][c - 1]);
    }
    let b = fit(&ho, serde_json::json!({ "model": HS, "estimator": "wlsmv" }));
    let shift = b.number("fit.chisq.scaled").unwrap() - b.number("fit.chisq").unwrap() / b.number("fit.chisq.scaling.factor").unwrap();
    assert!((shift - r["ord.fit.shift"].as_f64().unwrap()).abs() < 1e-6, "shift {shift}");
    assert!(compare(&b, "ord", 1e-6, 1e-5) > 150);
    let text = paragraph(&b).unwrap();
    assert!(text.contains("polychoric correlations") && text.contains("χ²(24) = 74.15") && text.contains("scaled CFI = .962"), "{text}");
}

#[test]
fn path_diagram_figure() {
    let pd = data("poldem.csv");
    let b = fit(&pd, serde_json::json!({ "model": PD }));
    let chart = b.charts.iter().find(|c| c.id == "diagram").expect("a path diagram");
    assert!(chart.alt.contains("dem60 is a latent variable measured by y1 (") && chart.alt.contains("Paths: ind60 to dem60 (.45)"), "{}", chart.alt);
    assert!(chart.svg.contains(r#"data-param="dem65 ~ dem60""#));
    if let Ok(dir) = std::env::var("PRISM_DIAGRAM_DIR") {
        std::fs::write(format!("{dir}/pd.svg"), &chart.svg).unwrap();
        let hs = data("holzinger.csv");
        let c = fit(&hs, serde_json::json!({ "model": format!("{HS}\ng =~ visual + textual + speed") }));
        std::fs::write(format!("{dir}/ho.svg"), &c.charts[0].svg).unwrap();
        let c = fit(&hs, serde_json::json!({ "model": HS }));
        std::fs::write(format!("{dir}/cfa.svg"), &c.charts[0].svg).unwrap();
    }
}

#[test]
fn latent_growth_curves() {
    let g = data("growth.csv");
    let lg = "i =~ 1*t1 + 1*t2 + 1*t3 + 1*t4
s =~ 0*t1 + 1*t2 + 2*t3 + 3*t4";
    let b = fit(&g, serde_json::json!({ "model": lg, "growth": true }));
    assert!(compare(&b, "lgm", 1e-6, 1e-5) > 70);
    assert_eq!(b.title, "Latent Growth Model");
    assert!(paragraph(&b).unwrap().starts_with("A latent growth model was estimated by maximum likelihood"));
    let b = fit(&g, serde_json::json!({ "model": format!("{lg}
i + s ~ x1 + x2
t1 ~ c1
t2 ~ c2
t3 ~ c3
t4 ~ c4"), "growth": true }));
    assert!(compare(&b, "lgmc", 1e-6, 1e-5) > 150);
}

#[test]
fn two_level_models() {
    let d = data("twolevel.csv");
    let m = "level: 1\n  fw =~ y1 + y2 + y3\n  fw ~ x1 + x2 + x3\nlevel: 2\n  fb =~ y1 + y2 + y3\n  fb ~ w1 + w2";
    let b = fit(&d, serde_json::json!({ "model": m, "cluster": "cluster" }));
    // lavaan's two-level optimizer stops early (it fails outright with a tighter tolerance): its between-level
    // estimates are good to about 1e-4, and PrismStatistics reaches a lower χ².
    assert!(b.number("fit.chisq").unwrap() <= r()["ml.fit.chisq"].as_f64().unwrap() + 1e-9);
    assert!(compare(&b, "ml", 2e-4, 2e-4) > 100);
    let text = paragraph(&b).unwrap();
    assert!(text.starts_with("A two-level structural equation model was estimated by maximum likelihood (*N* = 2500 in 200 clusters)"), "{text}");
    assert_eq!(b.charts.len(), 2);
    let b = fit(&d, serde_json::json!({ "model": "level: 1\n  fw =~ y1 + y2 + y3\nlevel: 2\n  fb =~ y1 + y2 + y3", "cluster": "cluster" }));
    assert!(compare(&b, "mlcfa", 2e-4, 2e-4) > 40);
}
