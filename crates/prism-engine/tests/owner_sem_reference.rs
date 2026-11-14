//! Structural equation models against the product owner's two SEM reference packages (checklist C4b), on shared
//! example models: a three-factor CFA, a path model, the political democracy model, configural and metric two-group
//! CFAs, a latent growth model and the Satorra-Bentler correction.
//!
//! - The command-driven package: reference/owner_sem_cd.json, made by make_owner_sem_reference.do in its batch mode.
//!   It always models means, so PrismStatistics is run with a mean structure; standard errors are from the expected
//!   information (as PrismStatistics').
//! - The graphical package: reference/owner_sem_gr.json, made by make_owner_sem_reference.ps1 through its scripting
//!   engine with maximum-likelihood moments and without a mean structure (except in the two-group and growth
//!   models). It reports χ² as (N − G)·F rather than N·F, and its standard errors are from an information matrix
//!   scaled by N − G rather than N; both are converted here.
//!
//! Each package names parameters its own way; the functions below map the names to PrismStatistics' labels.

use std::collections::HashMap;
use std::path::PathBuf;

use prism_engine::analysis::sem;
use prism_engine::import::read_file;
use prism_engine::result::Status;
use prism_engine::{Dataset, ResultBundle};

fn reference(file: &str) -> serde_json::Map<String, serde_json::Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("reference").join(file);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn data(file: &str) -> Dataset {
    read_file(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("reference").join(file)).unwrap().0
}

const HS: &str = "Visual =~ x1 + x2 + x3\nTextual =~ x4 + x5 + x6\nSpeed =~ x7 + x8 + x9";
const PATH: &str = "x4 ~ x1 + x2\nx9 ~ x4 + x1\nx5 ~ x2";
const PD: &str = "Ind60 =~ x1 + x2 + x3
Dem60 =~ y1 + a*y2 + b*y3 + c*y4
Dem65 =~ y5 + a*y6 + b*y7 + c*y8
Dem60 ~ Ind60
Dem65 ~ Ind60 + Dem60
y1 ~~ y5
y2 ~~ y4 + y6
y3 ~~ y7
y4 ~~ y8
y6 ~~ y8";
const LGM: &str = "I =~ 1*t1 + 1*t2 + 1*t3 + 1*t4\nS =~ 0*t1 + 1*t2 + 2*t3 + 3*t4";

fn fit(ds: &Dataset, spec: serde_json::Value) -> ResultBundle {
    let b = sem::run(ds, &serde_json::from_value(spec).unwrap()).unwrap();
    assert_eq!(b.status, Status::Ok, "{:?}", b.status);
    b
}

/// PrismStatistics' parameters by label ("Pasteur:Visual=~x2" in two-group models): (index, group size).
struct Prism<'a> {
    b: &'a ResultBundle,
    index: HashMap<String, usize>,
}

impl<'a> Prism<'a> {
    fn new(b: &'a ResultBundle) -> Self {
        let n = b.number("par.count").unwrap() as usize;
        let index = (1..=n)
            .map(|k| {
                let l = b.text(&format!("par.{k}.label")).unwrap().replace(' ', "");
                (b.text(&format!("par.{k}.group")).map_or(l.clone(), |g| format!("{g}:{l}")), k)
            })
            .collect();
        Prism { b, index }
    }
    /// A parameter's field, trying both orders of a covariance.
    fn get(&self, label: &str, field: &str) -> Option<f64> {
        let k = self.index.get(label).or_else(|| {
            let (g, rest) = label.rsplit_once(':').map_or(("", label), |(g, r)| (g, r));
            let (a, b) = rest.split_once("~~")?;
            let swapped = if g.is_empty() { format!("{b}~~{a}") } else { format!("{g}:{b}~~{a}") };
            self.index.get(&swapped)
        })?;
        self.b.number(&format!("par.{k}.{field}"))
    }
}

/// Latent variables are capitalized in these models, observed variables are not.
fn latent(v: &str) -> bool {
    v.starts_with(|c: char| c.is_ascii_uppercase())
}

/// A path from `x` to `y`: a loading when a factor points at an indicator, else a regression.
fn path(y: &str, x: &str) -> String {
    if latent(x) && !latent(y) { format!("{x}=~{y}") } else { format!("{y}~{x}") }
}

/// The command-driven package's name ("x2:Visual", "/cov(e.y1,e.y5)", "x2:1bn.sch#c.Visual", "/var(e.x1)#2.sch")
/// as a PrismStatistics label. Its groups are numbered alphabetically (1 Grant-White, 2 Pasteur).
fn cd_label(raw: &str) -> Option<String> {
    let group = |s: &str| match s {
        "1bn.sch" | "1.sch" => Some("Grant-White"),
        "2.sch" | "2bn.sch" => Some("Pasteur"),
        _ => None,
    };
    let strip = |v: &str| v.strip_prefix("e.").unwrap_or(v).to_string();
    let (eq, name) = if let Some(rest) = raw.strip_prefix('/') { ("/", rest.trim_start_matches(':')) } else { raw.split_once(':')? };
    let (label, g) = if eq == "/" {
        let (body, g) = name.split_once('#').map_or((name, None), |(b, g)| (b, group(g)));
        let (f, args) = body.strip_suffix(')')?.split_once('(')?;
        let label = match f {
            "var" => format!("{0}~~{0}", strip(args)),
            "cov" => {
                let (a, b) = args.split_once(',')?;
                format!("{}~~{}", strip(a), strip(b))
            }
            "mean" => format!("{args}~1"),
            _ => return None,
        };
        (label, g)
    } else if name == "_cons" {
        (format!("{eq}~1"), None)
    } else if let Some((g, x)) = name.split_once("#c.") {
        (path(eq, x), group(g))
    } else if let Some(g) = group(name) {
        (format!("{eq}~1"), Some(g))
    } else {
        (path(eq, name), None)
    };
    Some(g.map_or(label.clone(), |g| format!("{g}:{label}")))
}

/// The graphical package's error variables as the variables they belong to.
fn gr_variable(model: &str, v: &str) -> String {
    match (model, v) {
        (_, "z60") => "Dem60".into(),
        (_, "z65") => "Dem65".into(),
        ("lgm", e) if e.starts_with('e') => format!("t{}", &e[1..]),
        (_, d) if d.starts_with('d') && d[1..].parse::<u32>().is_ok() => format!("y{}", &d[1..]),
        (_, e) if e.starts_with('e') && e[1..].parse::<u32>().is_ok() => format!("x{}", &e[1..]),
        (_, v) => v.into(),
    }
}

/// The graphical package's "g1:1:x2:Visual" (group, parameter type, left, right) as a PrismStatistics label; its
/// groups are numbered in the order given (1 Pasteur, 2 Grant-White).
fn gr_label(model: &str, raw: &str, groups: bool) -> Option<String> {
    let mut it = raw.splitn(4, ':');
    let g = it.next()?.strip_prefix('g')?;
    let kind = it.next()?;
    let (l, r) = (gr_variable(model, it.next()?), gr_variable(model, it.next()?));
    let label = match kind {
        "1" | "std" => path(&l, &r),
        "2" | "3" => format!("{l}~1"),
        "4" | "5" => format!("{l}~~{r}"),
        _ => return None,
    };
    Some(if groups { format!("{}:{label}", if g == "1" { "Pasteur" } else { "Grant-White" }) } else { label })
}

/// Tallies the relative differences, failing those over the tolerance.
#[derive(Default)]
struct Tally {
    checked: usize,
    failures: Vec<String>,
    worst: Vec<(f64, String)>,
}

impl Tally {
    fn check(&mut self, key: &str, got: Option<f64>, want: f64, tol: f64) {
        let Some(got) = got else {
            self.failures.push(format!("{key}: PrismStatistics has no value (reference {want})"));
            return;
        };
        let err = (got - want).abs() / want.abs().max(1.0);
        self.worst.push((err, key.to_string()));
        if err > tol {
            self.failures.push(format!("{key}: got {got}, reference {want} (relative error {err:e})"));
        }
        self.checked += 1;
    }
    /// Prints the largest differences; returns how many values were compared and the failures.
    fn finish(mut self, what: &str) -> (usize, Vec<String>) {
        self.worst.sort_by(|a, b| b.0.total_cmp(&a.0));
        eprintln!("{what}: {} values, largest relative errors {:?}", self.checked, &self.worst[..4.min(self.worst.len())]);
        (self.checked, self.failures.into_iter().map(|f| format!("{what}: {f}")).collect())
    }
}

/// Fails with every difference from every comparison; each must also have compared at least its minimum of values.
fn all_agree(results: &[((usize, Vec<String>), usize)]) {
    let failures: Vec<&str> = results.iter().flat_map(|((_, f), _)| f.iter().map(String::as_str)).collect();
    assert!(failures.is_empty(), "{} values differ:\n{}", failures.len(), failures.join("\n"));
    for ((n, _), min) in results {
        assert!(n >= min, "only {n} values compared (expected at least {min})");
    }
}

/// The command-driven package's conventions for one comparison.
struct Cd<'a> {
    /// The sample size: its RMSEA (and interval) uses N − 1 where PrismStatistics, like lavaan, uses N.
    n: f64,
    /// Its SRMR leaves out the mean residuals: the comparable PrismStatistics value, where there is one.
    srmr: Option<f64>,
    /// A factor on PrismStatistics' standard errors (its Satorra-Bentler standard errors use N − 1).
    se_scale: f64,
    /// Fit measures compared with another PrismStatistics value.
    fit: &'a [(&'a str, f64)],
}

impl Default for Cd<'_> {
    fn default() -> Self {
        Cd { n: 301.0, srmr: None, se_scale: 1.0, fit: &[] }
    }
}

/// Compares PrismStatistics with the command-driven package's values for one model.
fn against_cd(b: &ResultBundle, model: &str, cd: &Cd, tol_est: f64, tol_se: f64) -> (usize, Vec<String>) {
    let r = reference("owner_sem_cd.json");
    let p = Prism::new(b);
    let mut t = Tally::default();
    for (key, want) in r.iter().filter(|(k, _)| k.starts_with(&format!("{model}."))) {
        let want = want.as_f64().unwrap();
        let rest = &key[model.len() + 1..];
        if let Some(m) = rest.strip_prefix("fit.") {
            if let Some((_, v)) = cd.fit.iter().find(|(k, _)| *k == m) {
                t.check(key, Some(*v), want, tol_est);
                continue;
            }
            match m {
                // P(RMSEA ≤ .05) with N − 1; the RMSEA and its interval are compared.
                "pclose" => continue,
                "srmr" => {
                    if cd.srmr.is_some() {
                        t.check(key, cd.srmr, want, tol_est);
                    }
                    continue;
                }
                _ => {}
            }
            let field = match m {
                "chi2_ms" => "fit.chisq".to_string(),
                "df_ms" => "fit.df".to_string(),
                "chi2_bs" => "fit.baseline.chisq".to_string(),
                "df_bs" => "fit.baseline.df".to_string(),
                "lb90_rmsea" => "fit.rmsea.lower".to_string(),
                "ub90_rmsea" => "fit.rmsea.upper".to_string(),
                "chi2sb_ms" => "fit.chisq.scaled".to_string(),
                "chi2sb_bs" => "fit.baseline.chisq.scaled".to_string(),
                "cfi_sb" => "fit.cfi.scaled".to_string(),
                "tli_sb" => "fit.tli.scaled".to_string(),
                "rmsea_sb" => "fit.rmsea.scaled".to_string(),
                "npar" => "npar".to_string(),
                other => format!("fit.{other}"),
            };
            let scale = if m.contains("rmsea") { (cd.n / (cd.n - 1.0)).sqrt() } else { 1.0 };
            t.check(key, b.number(&field).map(|v| v * scale), want, tol_est);
            continue;
        }
        let (name, field) = rest.rsplit_once('.').unwrap();
        let label = cd_label(name).unwrap_or_else(|| panic!("{key}: can't map the name {name:?}"));
        let (field, tol) = match field {
            "est" => ("est", tol_est),
            "se" => ("se", tol_se),
            "std" => ("std", tol_est),
            _ => ("stdse", tol_se),
        };
        let scale = if field.ends_with("se") { cd.se_scale } else { 1.0 };
        t.check(&format!("{key} ({label})"), p.get(&label, field).map(|v| v * scale), want, tol);
    }
    t.finish(&format!("command-driven {model}"))
}

/// Compares PrismStatistics with the graphical package's values for one model; `sizes` are the group sizes, in the
/// package's group order.
fn against_gr(b: &ResultBundle, model: &str, sizes: &[f64], tol_est: f64, tol_se: f64) -> (usize, Vec<String>) {
    let r = reference("owner_sem_gr.json");
    let p = Prism::new(b);
    let groups = sizes.len() > 1;
    let n: f64 = sizes.iter().sum();
    let mut t = Tally::default();
    for (key, want) in r.iter().filter(|(k, _)| k.starts_with(&format!("{model}."))) {
        let want = want.as_f64().unwrap();
        let rest = &key[model.len() + 1..];
        if let Some(m) = rest.strip_prefix("fit.") {
            let got = match m {
                // (N − G)·F rather than N·F.
                "cmin" => b.number("fit.chisq").map(|c| c * (n - sizes.len() as f64) / n),
                "df" => b.number("fit.df"),
                _ => b.number("npar"),
            };
            t.check(key, got, want, tol_est);
            continue;
        }
        if let Some(raw) = rest.strip_prefix("std:") {
            let label = gr_label(model, &format!("{}:std:{}", &raw[..2], &raw[3..]), groups).unwrap();
            t.check(&format!("{key} ({label})"), p.get(&label, "std"), want, tol_est);
            continue;
        }
        let (name, field) = rest.rsplit_once('.').unwrap();
        let label = gr_label(model, name, groups).unwrap_or_else(|| panic!("{key}: can't map the name {name:?}"));
        if field == "est" {
            t.check(&format!("{key} ({label})"), p.get(&label, "est"), want, tol_est);
        } else {
            // Information scaled by N − G: standard errors larger by √(N / (N − G)).
            let scale = (n / (n - sizes.len() as f64)).sqrt();
            t.check(&format!("{key} ({label})"), p.get(&label, "se").map(|s| s * scale), want, tol_se);
        }
    }
    t.finish(&format!("graphical {model}"))
}

/// SRMR without the mean residuals: a fit of the same model without a mean structure.
fn srmr_without_means(ds: &Dataset, model: &str) -> Option<f64> {
    fit(ds, serde_json::json!({ "model": model })).number("fit.srmr")
}

#[test]
fn confirmatory_factor_analysis() {
    let hs = data("holzinger.csv");
    let b = fit(&hs, serde_json::json!({ "model": HS, "meanstructure": true }));
    let plain = fit(&hs, serde_json::json!({ "model": HS }));
    let cd = Cd { srmr: plain.number("fit.srmr"), ..Cd::default() };
    all_agree(&[(against_cd(&b, "cfa", &cd, 1e-6, 1e-6), 140), (against_gr(&plain, "cfa", &[301.0], 1e-6, 1e-6), 50)]);
}

#[test]
fn satorra_bentler_correction() {
    // Its Satorra-Bentler correction evaluates the weight at the sample moments (lavaan's
    // h1.information = "unstructured"), and its robust standard errors use N − 1.
    let hs = data("holzinger.csv");
    let b = fit(&hs, serde_json::json!({ "model": HS, "meanstructure": true, "estimator": "mlm", "h1_information": "unstructured" }));
    let cd = Cd { srmr: srmr_without_means(&hs, HS), se_scale: (301.0f64 / 300.0).sqrt(), ..Cd::default() };
    all_agree(&[(against_cd(&b, "mlm", &cd, 1e-6, 1e-6), 140)]);
}

#[test]
fn path_model() {
    // Neither package adds lavaan's default covariance between the two pure outcomes (x9 and x5), so it is fixed at 0.
    let model = format!("{PATH}\nx9 ~~ 0*x5");
    let hs = data("holzinger.csv");
    let b = fit(&hs, serde_json::json!({ "model": model, "meanstructure": true }));
    // The command-driven package reports the joint log-likelihood (predictors included) but counts only the
    // conditional model's parameters in AIC and BIC.
    let joint = fit(&hs, serde_json::json!({ "model": model, "meanstructure": true, "fixed_x": false }));
    let (logl, k) = (joint.number("fit.logl").unwrap(), b.number("npar").unwrap());
    let overrides = [("logl", logl), ("aic", -2.0 * logl + 2.0 * k), ("bic", -2.0 * logl + 301f64.ln() * k)];
    let cd = Cd { srmr: srmr_without_means(&hs, &model), fit: &overrides, ..Cd::default() };
    let free = fit(&hs, serde_json::json!({ "model": model, "fixed_x": false }));
    all_agree(&[(against_cd(&b, "path", &cd, 1e-6, 1e-6), 50), (against_gr(&free, "path", &[301.0], 1e-6, 1e-6), 25)]);
}

#[test]
fn political_democracy_model() {
    let pd = data("poldem.csv");
    let b = fit(&pd, serde_json::json!({ "model": PD, "meanstructure": true }));
    let plain = fit(&pd, serde_json::json!({ "model": PD }));
    let cd = Cd { n: 75.0, srmr: plain.number("fit.srmr"), ..Cd::default() };
    all_agree(&[(against_cd(&b, "pd", &cd, 1e-6, 1e-6), 180), (against_gr(&plain, "pd", &[75.0], 1e-6, 1e-6), 70)]);
}

#[test]
fn two_group_models() {
    let hs = data("holzinger.csv");
    for (model, equal) in [("mgconfigural", vec![]), ("mgmetric", vec!["loadings"])] {
        let b = fit(&hs, serde_json::json!({ "model": HS, "group": "school", "group_equal": equal }));
        // The graphical package's optimizer stops a little short here (estimates in the sixth digit); the command-driven
        // package, converged tightly, agrees with PrismStatistics to 1e-7.
        all_agree(&[(against_cd(&b, model, &Cd::default(), 1e-6, 1e-6), 250), (against_gr(&b, model, &[156.0, 145.0], 1e-5, 1e-5), 100)]);
    }
}

#[test]
fn latent_growth_model() {
    let g = data("growth.csv");
    let b = fit(&g, serde_json::json!({ "model": LGM, "growth": true }));
    let n = g.n_rows() as f64;
    // With misfitting means, the command-driven package's expected-information standard errors differ from lavaan's
    // and PrismStatistics' in the fourth digit (they match neither of lavaan's h1.information choices); its estimates
    // and fit agree to 1e-6, and so do the graphical package's standard errors. The graphical package stops slightly
    // short here (a residual variance in the sixth digit).
    all_agree(&[(against_cd(&b, "lgm", &Cd { n, ..Cd::default() }, 1e-6, 5e-4), 70), (against_gr(&b, "lgm", &[n], 1e-5, 1e-6), 25)]);
}
