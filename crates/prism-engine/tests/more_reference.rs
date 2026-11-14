//! The analyses added after the preview, against R (reference/more_reference.json, made by make_more_reference.R from
//! preview.csv and scale.csv). As in preview_reference.rs, the data have empty cells and user-missing codes, and R gets
//! the same exclusions written out by hand.

use std::path::PathBuf;

use prism_engine::analysis::*;
use prism_engine::import::read_file;
use prism_engine::model::ValueLabel;
use prism_engine::result::Status;
use prism_engine::templates::paragraph;
use prism_engine::{Dataset, ResultBundle};

fn reference_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("reference")
}

fn r() -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(reference_dir().join("more_reference.json")).unwrap()).unwrap()
}

fn labels(pairs: &[(f64, &str)]) -> Vec<ValueLabel> {
    pairs.iter().map(|(v, l)| ValueLabel { value: *v, label: l.to_string() }).collect()
}

/// preview.csv with the labels and missing codes of preview_reference.rs.
fn preview() -> Dataset {
    let (mut ds, _) = read_file(&reference_dir().join("preview.csv")).unwrap();
    let c = ds.variable_mut("cond").unwrap();
    c.label = "Condition".into();
    c.value_labels = labels(&[(1.0, "Lecture"), (2.0, "Discussion"), (3.0, "Online")]);
    c.missing_values = vec![9.0];
    ds.variable_mut("sex").unwrap().value_labels = labels(&[(1.0, "Female"), (2.0, "Male")]);
    ds.variable_mut("smoker").unwrap().value_labels = labels(&[(1.0, "No"), (2.0, "Yes")]);
    for v in ["pre", "post"] {
        ds.variable_mut(v).unwrap().missing_values = vec![-99.0];
    }
    ds.variable_mut("score").unwrap().label = "exam score".into();
    ds.variable_mut("hours").unwrap().label = "study hours".into();
    ds
}

/// scale.csv: five agreement items (9 is user-missing) and a yes-or-no question asked twice.
fn scale() -> Dataset {
    let (mut ds, _) = read_file(&reference_dir().join("scale.csv")).unwrap();
    for i in ["i1", "i2", "i3", "i4", "i5"] {
        ds.variable_mut(i).unwrap().missing_values = vec![9.0];
    }
    for v in ["before", "after"] {
        ds.variable_mut(v).unwrap().value_labels = labels(&[(1.0, "No"), (2.0, "Yes")]);
    }
    ds
}

#[track_caller]
fn close(got: f64, want: &serde_json::Value, tol: f64) {
    let want = want.as_f64().unwrap_or_else(|| panic!("no reference value {want:?}"));
    let err = (got - want).abs() / want.abs().max(1.0);
    assert!(err <= tol, "got {got}, R gives {want} (relative error {err:e})");
}

fn f(b: &ResultBundle, k: &str) -> f64 {
    b.number(k).unwrap_or_else(|| panic!("no field {k} in {}", b.title))
}

const TOL: f64 = 1e-9;

#[test]
fn fisher_exact_in_crosstabs() {
    let r = r();
    let spec = |chi: bool| crosstabs::Spec { row: "sex".into(), column: "smoker".into(), percentages: vec![crosstabs::Percent::Row], chi_square: chi, continuity: false, expected: false, residuals: false, fisher: true, risk: false, ordinal: false, layer: None, bayes: Default::default(), sampling: Default::default() };
    let b = crosstabs::run(&preview(), &spec(true)).unwrap();
    close(f(&b, "fisher.p"), &r["fisher.p"], 1e-8);
    // fisher.test solves for the odds ratio and its interval at uniroot's default tolerance (about 1e-4); the .exact values are
    // the same equations solved to full precision in the reference script.
    close(f(&b, "fisher.or"), &r["fisher.or.exact"], 1e-9);
    close(f(&b, "fisher.or"), &r["fisher.or"], 1e-4);
    close(f(&b, "fisher.or.lower"), &r["fisher.lo.exact"], 1e-8);
    close(f(&b, "fisher.or.lower"), &r["fisher.lo"], 1e-3);
    close(f(&b, "fisher.or.upper"), &r["fisher.hi.exact"], 1e-8);
    close(f(&b, "fisher.or.upper"), &r["fisher.hi"], 1e-3);
    let text = paragraph(&b).unwrap();
    assert!(text.contains("Fisher's exact test gave *p* = .038, conditional odds ratio = 3.38, 95% CI [1.06, 11.46]."), "{text}");
    // Fisher alone.
    let b = crosstabs::run(&preview(), &spec(false)).unwrap();
    assert!(b.tables.iter().any(|t| t.id == "fisher"));
    let text = paragraph(&b).unwrap();
    assert!(text.starts_with("Fisher's exact test examined the relationship between sex and smoker (*N* = 60). The relationship was significant."), "{text}");
    // Not offered beyond two-by-two.
    let mut s = spec(true);
    s.row = "region".into();
    let b = crosstabs::run(&preview(), &s).unwrap();
    assert!(b.number("fisher.p").is_none() && b.method_note.contains("two-by-two tables only"));
}

#[test]
fn mcnemar_test() {
    let r = r();
    let b = &related::mcnemar(&scale(), &related::McNemarSpec { pairs: vec![("before".into(), "after".into())] }).unwrap()[0];
    close(f(b, "n"), &r["mc.n"], 0.0);
    close(f(b, "b"), &r["mc.b"], 0.0);
    close(f(b, "c"), &r["mc.c"], 0.0);
    close(f(b, "chi2"), &r["mc.chi2"], TOL);
    close(f(b, "p"), &r["mc.p"], 1e-8);
    close(f(b, "chi2.uncorrected"), &r["mc.chi2u"], TOL);
    close(f(b, "p.uncorrected"), &r["mc.pu"], 1e-8);
    close(f(b, "p.exact"), &r["mc.pexact"], 1e-8);
    let text = paragraph(b).unwrap();
    // 20 cases changed, so the exact p value is reported.
    assert!(text.contains("The share answering Yes changed significantly, exact *p* < .001 (20 of the 79 cases changed)."), "{text}");
    assert_eq!(b.cases.excluded[0].reason, "second variable empty");
    // A variable with three categories is refused.
    let b = &related::mcnemar(&scale(), &related::McNemarSpec { pairs: vec![("i1".into(), "i5".into())] }).unwrap()[0];
    assert!(matches!(&b.status, Status::Failed { reason } if reason.contains("same two categories")));
}

#[test]
fn friedman_test() {
    let r = r();
    let b = related::friedman(&scale(), &related::FriedmanSpec { variables: vec!["i1".into(), "i2".into(), "i3".into()] }).unwrap();
    close(f(&b, "n"), &r["fr.n"], 0.0);
    close(f(&b, "chi2"), &r["fr.chi2"], TOL);
    close(f(&b, "df"), &r["fr.df"], 0.0);
    close(f(&b, "p"), &r["fr.p"], 1e-8);
    close(f(&b, "W"), &r["fr.W"], TOL);
    close(f(&b, "group.1.mean_rank"), &r["fr.mr1"], TOL);
    close(f(&b, "group.3.mean_rank"), &r["fr.mr3"], TOL);
    let text = paragraph(&b).unwrap();
    assert!(text.contains("did not differ significantly, χ²(2, *N* = 78) = 0.18, *p* = .915, Kendall's *W* = .00"), "{text}");
    assert_eq!(b.cases.excluded.len(), 2);
}

#[test]
fn partial_correlation() {
    let r = r();
    let spec = |controls: &[&str]| partial_correlation::Spec { variables: vec!["hours".into(), "score".into()], controls: controls.iter().map(|c| c.to_string()).collect(), zero_order: true, level: 0.95 };
    let b = partial_correlation::run(&preview(), &spec(&["pre"])).unwrap();
    close(f(&b, "n"), &r["pc1.n"], 0.0);
    close(f(&b, "pair.1.2.r"), &r["pc1.r"], TOL);
    close(f(&b, "pair.1.2.df"), &r["pc1.df"], 0.0);
    close(f(&b, "pair.1.2.p"), &r["pc1.p"], 1e-8);
    close(f(&b, "pair.1.2.ci.lower"), &r["pc1.lo"], TOL);
    close(f(&b, "pair.1.2.ci.upper"), &r["pc1.hi"], TOL);
    close(f(&b, "pair.1.2.r0"), &r["pc1.r0"], TOL);
    let text = paragraph(&b).unwrap();
    assert!(text.contains("Study hours and exam score remained significantly positively correlated, partial *r*(54) = .46, *p* < .001, 95% CI [.23, .65] (zero-order *r* = .46)."), "{text}");
    let b = partial_correlation::run(&preview(), &spec(&["pre", "post"])).unwrap();
    close(f(&b, "n"), &r["pc2.n"], 0.0);
    close(f(&b, "pair.1.2.r"), &r["pc2.r"], TOL);
    close(f(&b, "pair.1.2.p"), &r["pc2.p"], 1e-8);
    assert!(partial_correlation::run(&preview(), &spec(&[])).is_err());
}

/// A factorial ANOVA of score with the defaults (Type III, every interaction).
fn fac(factors: &[&str]) -> anova_factorial::Spec {
    serde_json::from_value(serde_json::json!({ "dependent": "score", "factors": factors })).unwrap()
}

#[test]
fn factorial_anova_type_iii() {
    let r = r();
    let b = anova_factorial::run(&preview(), &fac(&["cond", "sex"])).unwrap();
    assert!(b.is_ok(), "{:?}", b.status);
    close(f(&b, "n"), &r["fa2.n"], 0.0);
    close(f(&b, "error.df"), &r["fa2.dfe"], 0.0);
    close(f(&b, "error.ss"), &r["fa2.rss"], TOL);
    close(f(&b, "r2"), &r["fa2.r2"], TOL);
    for (e, name) in [(1, "cond"), (2, "sex"), (3, "cond:sex")] {
        assert_eq!(b.text(&format!("effect.{e}.key")), Some(name));
        for st in ["ss", "df", "F", "pes"] {
            close(f(&b, &format!("effect.{e}.{st}")), &r[&format!("fa2.{name}.{st}")], 1e-8);
        }
        close(f(&b, &format!("effect.{e}.p")), &r[&format!("fa2.{name}.p")], 1e-8);
    }
    close(f(&b, "levene.F"), &r["fa2.leveneF"], TOL);
    close(f(&b, "levene.p"), &r["fa2.levenep"], 1e-8);
    close(f(&b, "factor.1.level.1.emm"), &r["fa2.emm.cond1"], TOL);
    let anova = b.tables.iter().find(|t| t.id == "anova").unwrap();
    let intercept = &anova.rows[1];
    assert!(matches!(intercept[4], prism_engine::result::Cell::Number { value, .. } if (value - r["fa2.intF"].as_f64().unwrap()).abs() < 1e-6));
    let text = paragraph(&b).unwrap();
    assert!(text.starts_with("A 3 × 2 factorial analysis of variance with Type III sums of squares examined exam score by Condition and sex (*N* = 57)."), "{text}");
    assert!(text.contains("The main effect of Condition was significant, *F*(2, 51) = 6.07, *p* = .004, η<sub>p</sub>² = .19."), "{text}");
    assert!(text.contains("The interaction of Condition and sex was not significant"), "{text}");

    let b = anova_factorial::run(&preview(), &fac(&["cond", "sex", "smoker"])).unwrap();
    assert!(b.is_ok(), "{:?}", b.status);
    close(f(&b, "error.df"), &r["fa3.dfe"], 0.0);
    for (e, name) in [(1, "cond"), (2, "sex"), (3, "smoker"), (4, "cond:sex"), (5, "cond:smoker"), (6, "sex:smoker"), (7, "cond:sex:smoker")] {
        assert_eq!(b.text(&format!("effect.{e}.key")), Some(name));
        close(f(&b, &format!("effect.{e}.ss")), &r[&format!("fa3.{name}.ss")], 1e-8);
        close(f(&b, &format!("effect.{e}.F")), &r[&format!("fa3.{name}.F")], 1e-8);
        close(f(&b, &format!("effect.{e}.p")), &r[&format!("fa3.{name}.p")], 1e-8);
    }
    // One factor is a one-way ANOVA, not this.
    assert!(anova_factorial::run(&preview(), &fac(&["cond"])).is_err());
}

#[test]
fn cronbach_alpha() {
    let r = r();
    let items: Vec<String> = ["i1", "i2", "i3", "i4", "i5"].iter().map(|s| s.to_string()).collect();
    let raw = reliability::run(&scale(), &reliability::Spec { items: items.clone(), reverse: vec![], scale_min: None, scale_max: None, level: 0.95 }).unwrap();
    close(f(&raw, "alpha"), &r["al.raw"], TOL);
    // Omega from a one-factor ML model, as a lavaan CFA gives it (ML estimates agree to the optimizer's tolerance).
    close(f(&raw, "omega"), &r["rel.omega"], 1e-5);
    assert!(paragraph(&raw).unwrap().contains("McDonald's omega"));
    assert!(raw.diagnostics.iter().any(|d| d.id == "direction" && d.state == prism_engine::result::DiagnosticState::ReviewNeeded && d.message.contains("i4")));
    let b = reliability::run(&scale(), &reliability::Spec { items, reverse: vec!["i4".into()], scale_min: Some(1.0), scale_max: Some(5.0), level: 0.95 }).unwrap();
    close(f(&b, "n"), &r["al.n"], 0.0);
    close(f(&b, "alpha"), &r["al.alpha"], TOL);
    close(f(&b, "alpha.ci.lower"), &r["al.lo"], 1e-8);
    close(f(&b, "alpha.ci.upper"), &r["al.hi"], 1e-8);
    close(f(&b, "alpha.std"), &r["al.std"], TOL);
    close(f(&b, "r.mean"), &r["al.rbar"], TOL);
    close(f(&b, "item.4.r_it"), &r["al.rit4"], TOL);
    close(f(&b, "item.4.alpha_deleted"), &r["al.del4"], TOL);
    close(f(&b, "item.1.alpha_deleted"), &r["al.del1"], TOL);
    let text = paragraph(&b).unwrap();
    assert!(text.contains("Internal consistency of the five-item scale (i1, i2, i3, i4, and i5) was estimated with Cronbach's alpha, α = .77, 95% CI [.67, .84] (*N* = 78). McDonald's omega, from a one-factor model, was ω = .78. Before the analysis, i4 was reverse-scored."), "{text}");
    assert!(b.method_note.contains("reverse-scored for this analysis as (1 + 5) minus the value."));
}

#[test]
fn logistic_regression() {
    let r = r();
    let spec = logistic::Spec { dependent: "smoker".into(), event: None, covariates: vec!["hours".into()], factors: vec![regression::Factor { name: "cond".into(), reference: None }], level: 0.95, blocks: vec![], interactions: vec![], center: false, roc: false };
    let b = logistic::run(&preview(), &spec).unwrap();
    assert!(b.is_ok(), "{:?}", b.status);
    close(f(&b, "n"), &r["lg.n"], 0.0);
    close(f(&b, "events"), &r["lg.events"], 0.0);
    close(f(&b, "deviance"), &r["lg.dev"], 1e-9);
    close(f(&b, "null_deviance"), &r["lg.null"], 1e-9);
    close(f(&b, "chi2"), &r["lg.chi2"], 1e-8);
    close(f(&b, "p"), &r["lg.p"], 1e-8);
    close(f(&b, "cox_snell"), &r["lg.coxsnell"], 1e-8);
    close(f(&b, "nagelkerke"), &r["lg.nagelkerke"], 1e-8);
    close(f(&b, "aic"), &r["lg.aic"], 1e-9);
    close(f(&b, "b0.b"), &r["lg.b0"], 1e-9);
    close(f(&b, "term.1.b"), &r["lg.hours.b"], 1e-9);
    close(f(&b, "term.1.se"), &r["lg.hours.se"], 1e-9);
    close(f(&b, "term.1.wald"), &r["lg.hours.wald"], 1e-9);
    close(f(&b, "term.1.p"), &r["lg.hours.p"], 1e-9);
    close(f(&b, "term.1.or"), &r["lg.hours.or"], 1e-9);
    close(f(&b, "term.1.or.wald.lower"), &r["lg.hours.orlo"], 1e-9);
    close(f(&b, "term.1.or.wald.upper"), &r["lg.hours.orhi"], 1e-9);
    // R's confint profiles the likelihood on a grid and interpolates, so its limits are good to about four digits.
    close(f(&b, "term.1.or.lower"), &r["lg.hours.orlo.profile"], 1e-4);
    close(f(&b, "term.1.or.upper"), &r["lg.hours.orhi.profile"], 1e-4);
    close(f(&b, "term.3.or.lower"), &r["lg.f3.orlo.profile"], 1e-4);
    close(f(&b, "term.3.or.upper"), &r["lg.f3.orhi.profile"], 1e-4);
    assert_eq!(b.text("term.3.label"), Some("Condition: Online (vs Lecture)"));
    close(f(&b, "term.3.b"), &r["lg.f3.b"], 1e-9);
    close(f(&b, "term.3.p"), &r["lg.f3.p"], 1e-9);
    close(f(&b, "pred.2.chi2"), &r["lg.cond.chi2"], 1e-8);
    close(f(&b, "pred.2.p"), &r["lg.cond.p"], 1e-8);
    close(f(&b, "accuracy"), &r["lg.accuracy"], TOL);
    let text = paragraph(&b).unwrap();
    assert!(text.starts_with("A binary logistic regression predicted the probability that smoker was Yes (rather than No) from study hours and Condition. The model fit significantly better than one with no predictors, χ²(3, *N* = 57) = 9.31, *p* = .025, Nagelkerke *R*² = .20, and classified 66.7% of the cases correctly."), "{text}");
    // The other category as the event flips the signs.
    let flipped = logistic::run(&preview(), &logistic::Spec { event: Some("1".into()), ..spec.clone() }).unwrap();
    close(-f(&flipped, "term.1.b"), &r["lg.hours.b"], 1e-9);
    // A three-category outcome is refused.
    let three = logistic::run(&preview(), &logistic::Spec { dependent: "cond".into(), factors: vec![], ..spec }).unwrap();
    assert!(matches!(&three.status, Status::Failed { reason } if reason.contains("exactly two categories")));
}

#[test]
fn canonical_correlation() {
    let r = r();
    let b = canonical::run(&preview(), &canonical::Spec { set1: vec!["pre".into(), "post".into()], set2: vec!["hours".into(), "score".into(), "sex".into()] }).unwrap();
    assert!(b.is_ok(), "{:?}", b.status);
    close(f(&b, "n"), &r["cc.n"], 0.0);
    close(f(&b, "functions"), &serde_json::json!(2), 0.0);
    for i in [1, 2] {
        close(f(&b, &format!("fn.{i}.r")), &r[&format!("cc.r{i}")], 1e-9);
        for k in ["lambda", "F", "df2", "p"] {
            close(f(&b, &format!("fn.{i}.{k}")), &r[&format!("cc.f{i}.{k}")], 1e-8);
        }
        close(f(&b, &format!("fn.{i}.a.1")), &r[&format!("cc.f{i}.a1")], 1e-7);
        close(f(&b, &format!("fn.{i}.a.2")), &r[&format!("cc.f{i}.a2")], 1e-7);
        close(f(&b, &format!("fn.{i}.b.2")), &r[&format!("cc.f{i}.b2")], 1e-7);
        close(f(&b, &format!("fn.{i}.sa.1")), &r[&format!("cc.f{i}.sa1")], 1e-7);
        close(f(&b, &format!("fn.{i}.sb.3")), &r[&format!("cc.f{i}.sb3")], 1e-7);
    }
    let text = paragraph(&b).unwrap();
    assert!(text.contains("The first canonical correlation was *r*<sub>c</sub> = .16. The full model (all functions together) was not significant, Wilks' λ = .97, *F*(6, 102) = 0.24, *p* = .962."), "{text}");
    assert!(canonical::run(&preview(), &canonical::Spec { set1: vec!["pre".into()], set2: vec!["post".into()] }).is_err());
}

#[test]
fn power_and_sample_size() {
    use power::{Solve, Spec, Test};
    let r = r();
    let spec = |test: Test, es: f64| Spec { test, solve_for: Solve::N, effect_size: Some(es), mean_difference: None, sd: None, means: None, groups: None, p1: None, p2: None, alpha: 0.05, power: 0.80, n: None };
    let check = |s: &Spec, n: &str, pw: &str| {
        let b = power::run(s).unwrap();
        close(f(&b, "n"), &r[n], 0.0);
        close(f(&b, "power"), &r[pw], 1e-9);
        b
    };
    check(&spec(Test::OneSample, 0.5), "pw.one.n", "pw.one.power");
    check(&spec(Test::Paired, 0.5), "pw.one.n", "pw.one.power");
    let b = check(&spec(Test::Independent, 0.5), "pw.ind.n", "pw.ind.power");
    assert_eq!(f(&b, "total"), 128.0);
    let text = paragraph(&b).unwrap();
    assert_eq!(text, "An a priori power analysis for a two-sided independent-samples t test indicated that 64 participants per group (*N* = 128) would give 80% power to detect an effect of *d* = 0.50 at α = .05 (achieved power = .801).");
    // The same effect in raw units: a 5-point difference with an SD of 10.
    let raw = Spec { effect_size: None, mean_difference: Some(5.0), sd: Some(10.0), ..spec(Test::Independent, 0.0) };
    check(&raw, "pw.ind.n", "pw.ind.power");
    let given = power::run(&Spec { solve_for: Solve::Power, n: Some(50), ..spec(Test::Independent, 0.4) }).unwrap();
    close(f(&given, "power"), &r["pw.ind.power50"], 1e-9);
    check(&Spec { groups: Some(3), ..spec(Test::Anova, 0.25) }, "pw.aov.n", "pw.aov.power");
    let means = power::run(&Spec { effect_size: None, means: Some(vec![10.0, 12.0, 14.0]), sd: Some(5.0), solve_for: Solve::Power, n: Some(20), ..spec(Test::Anova, 0.0) }).unwrap();
    close(f(&means, "effect"), &r["pw.aovraw.f"], 1e-12);
    close(f(&means, "power"), &r["pw.aovraw.power20"], 1e-9);
    check(&spec(Test::Correlation, 0.3), "pw.r.n", "pw.r.power");
    check(&Spec { effect_size: None, p1: Some(0.65), p2: Some(0.5), ..spec(Test::OneProportion, 0.0) }, "pw.p1.n", "pw.p1.power");
    let two = power::run(&Spec { effect_size: None, p1: Some(0.6), p2: Some(0.4), ..spec(Test::TwoProportions, 0.0) }).unwrap();
    close(f(&two, "n"), &r["pw.p2.n"], 0.0);
    assert!(power::run(&Spec { effect_size: None, ..spec(Test::Independent, 0.0) }).is_err());
}

#[test]
fn noncentral_f_against_r() {
    // pf(2.5, 3, 40, ncp = 6) and pf(1.2, 2, 150, ncp = 40) in R 4.6.1. R's noncentral beta stops its series at an error
    // bound of 1e-9, so agreement is checked to that.
    let a = power::f_cdf_noncentral(2.5, 3.0, 40.0, 6.0);
    let b = power::f_cdf_noncentral(1.2, 2.0, 150.0, 40.0);
    assert!((a - R_PF_A).abs() < 1e-9, "{a}");
    assert!((b - R_PF_B).abs() < 1e-9, "{b}");
}
const R_PF_A: f64 = 0.45183864573684523;
const R_PF_B: f64 = 4.621603853963837e-07;

#[test]
fn classic_nonparametric_tests() {
    let r = r();
    let ds = preview();
    let g = classic_np::goodness_of_fit(&ds, &classic_np::GofSpec { variable: "region".into(), expected: None }).unwrap();
    close(f(&g, "chi2"), &r["gof.chi2"], TOL);
    close(f(&g, "p"), &r["gof.p"], 1e-9);
    close(f(&g, "n"), &r["gof.n"], 0.0);
    assert!(paragraph(&g).unwrap().contains("χ²(2, *N* = 60) = 7.90, *p* = .019"), "{:?}", paragraph(&g));
    // East, North, South (alphabetical) in proportions 1 : 2 : 2, given as weights.
    let g2 = classic_np::goodness_of_fit(&ds, &classic_np::GofSpec { variable: "region".into(), expected: Some(vec![1.0, 2.0, 2.0]) }).unwrap();
    close(f(&g2, "chi2"), &r["gof2.chi2"], TOL);
    close(f(&g2, "p"), &r["gof2.p"], 1e-9);
    assert!(classic_np::goodness_of_fit(&ds, &classic_np::GofSpec { variable: "region".into(), expected: Some(vec![1.0, 2.0]) }).is_err());

    let b = &classic_np::binomial(&ds, &classic_np::BinomialSpec { variables: vec!["sex".into()], test_proportion: 0.5, success: None, level: 0.95 }).unwrap()[0];
    close(f(b, "x"), &r["bin.x"], 0.0);
    close(f(b, "p"), &r["bin.p"], 1e-9);
    close(f(b, "ci.lower"), &r["bin.lo"], 1e-8);
    close(f(b, "ci.upper"), &r["bin.hi"], 1e-8);

    let ru = &classic_np::runs(&ds, &classic_np::RunsSpec { variables: vec!["score".into()], cut: classic_np::Cut::Median, value: None }).unwrap()[0];
    close(f(ru, "runs"), &r["runs.runs"], 0.0);
    close(f(ru, "n_above"), &r["runs.above"], 0.0);

    let s = &classic_np::sign(&ds, &classic_np::SignSpec { pairs: vec![("pre".into(), "post".into())] }).unwrap()[0];
    close(f(s, "pos"), &r["sign.pos"], 0.0);
    close(f(s, "p"), &r["sign.p"], 1e-9);

    let m = &classic_np::median_test(&ds, &classic_np::GroupsSpec { dependent: vec!["score".into()], factor: "cond".into() }).unwrap()[0];
    close(f(m, "chi2"), &r["med.chi2"], TOL);
    close(f(m, "p"), &r["med.p"], 1e-9);

    let k = &classic_np::ks2(&ds, &classic_np::Ks2Spec { test_variables: vec!["score".into()], grouping_variable: "sex".into(), groups: (1.0, 2.0) }).unwrap()[0];
    close(f(k, "D"), &r["ks.D"], TOL);
    close(f(k, "p"), &r["ks.p"], 1e-6);
    for b in [&g, b, ru, s, m, k] {
        assert!(paragraph(b).is_some(), "{} has no paragraph", b.title);
    }
}

#[test]
fn crosstabs_risk_ordinal_and_layers() {
    let r = r();
    let spec = |row: &str, col: &str, layer: Option<&str>| crosstabs::Spec { row: row.into(), column: col.into(), percentages: vec![], chi_square: true, continuity: false, expected: false, residuals: false, fisher: false, risk: true, ordinal: true, layer: layer.map(str::to_string), bayes: Default::default(), sampling: Default::default() };
    let b = crosstabs::run(&preview(), &spec("sex", "smoker", None)).unwrap();
    close(f(&b, "risk.or"), &r["risk.or"], TOL);
    close(f(&b, "risk.or.lower"), &r["risk.or.lo"], TOL);
    close(f(&b, "risk.or.upper"), &r["risk.or.hi"], TOL);
    close(f(&b, "risk.rr1"), &r["risk.rr1"], TOL);
    close(f(&b, "risk.rr1.lower"), &r["risk.rr1.lo"], TOL);
    let o = crosstabs::run(&preview(), &spec("cond", "region", None)).unwrap();
    let z = 1.959963984540054;
    for (k, rk) in [("gamma", "gamma"), ("tau_b", "taub"), ("tau_c", "tauc"), ("somers_c", "somers_c")] {
        close(f(&o, &format!("ord.{k}")), &r[&format!("ord.{rk}")], 1e-9);
        close(f(&o, &format!("ord.{k}")) - z * f(&o, &format!("ord.{k}.ase")), &r[&format!("ord.{rk}.lo")], 1e-7);
    }
    close(f(&o, "ord.somers_r"), &r["ord.somers_r"], 1e-9);
    assert!(o.method_note.contains("two-by-two tables only"));
    let c = crosstabs::run(&preview(), &spec("sex", "smoker", Some("region"))).unwrap();
    close(f(&c, "cmh.chi2"), &r["cmh.chi2"], TOL);
    close(f(&c, "cmh.p"), &r["cmh.p"], 1e-9);
    close(f(&c, "cmh.or"), &r["cmh.or"], TOL);
    close(f(&c, "cmh.or.lower"), &r["cmh.or.lo"], 1e-8);
    close(f(&c, "cmh.or.upper"), &r["cmh.or.hi"], 1e-8);
    close(f(&c, "bd.chi2"), &r["bd.chi2"], 1e-7);
    close(f(&c, "bd.p"), &r["bd.p"], 1e-7);
}

#[test]
fn cochran_q() {
    let r = r();
    let b = related::cochran(&scale(), &related::FriedmanSpec { variables: vec!["before".into(), "after".into()] }).unwrap();
    close(f(&b, "Q"), &r["cq.Q"], TOL);
    close(f(&b, "p"), &r["cq.p"], 1e-9);
}

#[test]
fn rater_agreement() {
    let r = r();
    let ds = scale();
    let k = |w| agreement::kappa(&ds, &agreement::KappaSpec { rater1: "i1".into(), rater2: "i2".into(), weights: w, level: 0.95 }).unwrap();
    let k0 = k(agreement::Weights::None);
    close(f(&k0, "n"), &r["kap.n"], 0.0);
    close(f(&k0, "kappa"), &r["kap.k"], TOL);
    close(f(&k0, "z"), &r["kap.z"], 1e-8);
    close(f(&k0, "p"), &r["kap.p"], 1e-8);
    close(f(&k0, "ci.lower"), &r["kap.lo"], 1e-8);
    close(f(&k0, "ci.upper"), &r["kap.hi"], 1e-8);
    let kw = k(agreement::Weights::Linear);
    close(f(&kw, "kappa"), &r["kapw.k"], TOL);
    close(f(&kw, "z"), &r["kapw.z"], 1e-8);
    let kq = k(agreement::Weights::Quadratic);
    close(f(&kq, "kappa"), &r["kapq.k"], TOL);
    close(f(&kq, "z"), &r["kapq.z"], 1e-8);
    let raters: Vec<String> = ["i1", "i2", "i3"].iter().map(|s| s.to_string()).collect();
    let fl = agreement::fleiss(&ds, &agreement::RatersSpec { raters: raters.clone() }).unwrap();
    close(f(&fl, "kappa"), &r["fl.k"], TOL);
    close(f(&fl, "z"), &r["fl.z"], 1e-8);
    close(f(&fl, "p"), &r["fl.p"], 1e-8);
    for (m, key) in [(agreement::Metric::Nominal, "kr.nominal"), (agreement::Metric::Ordinal, "kr.ordinal"), (agreement::Metric::Interval, "kr.interval"), (agreement::Metric::Ratio, "kr.ratio")] {
        let b = agreement::kripp(&ds, &agreement::KrippSpec { raters: raters.clone(), metric: m }).unwrap();
        close(f(&b, "alpha"), &r[key], 1e-9);
    }
    use agreement::{IccModel::*, IccType::*, IccUnit::*};
    for (model, kind, unit, name) in [(OneWay, Consistency, Single, "oneway.consistency.single"), (OneWay, Consistency, Average, "oneway.consistency.average"), (TwoWay, Consistency, Single, "twoway.consistency.single"), (TwoWay, Consistency, Average, "twoway.consistency.average"), (TwoWay, Agreement, Single, "twoway.agreement.single"), (TwoWay, Agreement, Average, "twoway.agreement.average")] {
        let b = agreement::icc(&ds, &agreement::IccSpec { raters: raters.clone(), model, kind, unit, level: 0.95 }).unwrap();
        close(f(&b, "icc"), &r[&format!("icc.{name}")], 1e-9);
        close(f(&b, "ci.lower"), &r[&format!("icc.{name}.lo")], 1e-7);
        close(f(&b, "ci.upper"), &r[&format!("icc.{name}.hi")], 1e-7);
        close(f(&b, "F"), &r[&format!("icc.{name}.F")], 1e-9);
        close(f(&b, "p"), &r[&format!("icc.{name}.p")], 1e-7);
    }
}

#[test]
fn explore_with_normality() {
    let r = r();
    let all = &explore::run(&preview(), &explore::Spec { dependent: vec!["score".into()], factor: None, percentiles: true, normality: true, plots: true, level: 0.95 }).unwrap()[0];
    for (k, rk, tol) in [("trimmed", "ex.trim", TOL), ("p10", "ex.p10", TOL), ("p95", "ex.p95", TOL), ("skew", "ex.skew", 1e-9), ("kurt", "ex.kurt", 1e-9), ("skew.se", "ex.skew.se", 1e-9), ("kurt.se", "ex.kurt.se", 1e-9), ("sw.W", "ex.sw.W", 1e-6), ("sw.p", "ex.sw.p", 1e-5), ("ks.D", "ex.ks.D", 1e-9), ("ks.p", "ex.ks.p", 1e-7)] {
        close(f(all, &format!("group.1.{k}")), &r[rk], tol);
    }
    assert_eq!(all.charts.len(), 2, "a box plot and a Q-Q plot");
    assert!(paragraph(all).unwrap().starts_with("For exam score, *n* = 59"), "{:?}", paragraph(all));
    let by = &explore::run(&preview(), &explore::Spec { dependent: vec!["score".into()], factor: Some("cond".into()), percentiles: true, normality: true, plots: true, level: 0.95 }).unwrap()[0];
    close(f(by, "group.2.sw.p"), &r["ex2.sw.p"], 1e-5);
    close(f(by, "group.2.ks.p"), &r["ex2.ks.p"], 1e-7);
    close(f(by, "group.2.trimmed"), &r["ex2.trim"], TOL);
    assert_eq!(by.charts.len(), 4, "a box plot and three Q-Q plots");
}

#[test]
fn hierarchical_regression_with_a_centered_interaction() {
    let r = r();
    let spec = regression::Spec {
        dependent: "score".into(),
        covariates: vec!["hours".into()],
        factors: vec![],
        level: 0.95,
        blocks: vec![regression::Block { covariates: vec![], factors: vec![regression::Factor { name: "cond".into(), reference: None }] }],
        interactions: vec![("hours".into(), "cond".into())],
        center: true,
        plots: true, robust: Default::default(), bayes: Default::default(), prior_scale: Default::default(), };
    let b = regression::run(&preview(), &spec).unwrap();
    assert!(b.is_ok(), "{:?}", b.status);
    close(f(&b, "blocks"), &serde_json::json!(3), 0.0);
    close(f(&b, "step.1.r2"), &r["hier.r2.1"], TOL);
    close(f(&b, "step.2.dr2"), &r["hier.dr2.2"], TOL);
    close(f(&b, "step.2.F"), &r["hier.F.2"], 1e-9);
    close(f(&b, "step.2.p"), &r["hier.p.2"], 1e-9);
    close(f(&b, "step.3.F"), &r["hier.F.3"], 1e-8);
    close(f(&b, "step.3.df1"), &r["hier.df1.3"], 0.0);
    close(f(&b, "step.3.p"), &r["hier.p.3"], 1e-9);
    close(f(&b, "r2"), &r["hier.r2.3"], TOL);
    close(f(&b, "term.1.b"), &r["hier.hc.b"], 1e-9);
    assert_eq!(b.text("term.5.label"), Some("study hours × Condition: Online (vs Lecture)"));
    close(f(&b, "term.5.b"), &r["hier.int.b"], 1e-9);
    assert_eq!(b.charts.len(), 3, "residual plots");
    assert!(b.method_note.contains("study hours was mean-centered"), "{}", b.method_note);
    let vals = regression::case_values(&preview(), &spec).unwrap();
    assert_eq!(vals.len(), f(&b, "n") as usize);
}

#[test]
fn logistic_blocks_and_roc() {
    let r = r();
    let spec = logistic::Spec { dependent: "smoker".into(), event: None, covariates: vec!["hours".into()], factors: vec![], level: 0.95, blocks: vec![regression::Block { covariates: vec![], factors: vec![regression::Factor { name: "cond".into(), reference: None }] }], interactions: vec![], center: false, roc: true };
    let b = logistic::run(&preview(), &spec).unwrap();
    assert!(b.is_ok(), "{:?}", b.status);
    close(f(&b, "step.1.chi2"), &r["lgb.chi2.1"], 1e-8);
    close(f(&b, "step.2.chi2"), &r["lgb.chi2.2"], 1e-8);
    close(f(&b, "step.2.p"), &r["lgb.p.2"], 1e-8);
    close(f(&b, "roc.auc"), &r["roc.auc"], 1e-12);
    close(f(&b, "roc.lower"), &r["roc.lo"], 1e-9);
    close(f(&b, "roc.upper"), &r["roc.hi"], 1e-9);
    assert_eq!(b.charts.len(), 1);
    let with_int = logistic::run(&preview(), &logistic::Spec { interactions: vec![("hours".into(), "cond".into())], center: true, ..spec }).unwrap();
    assert!(with_int.is_ok() && f(&with_int, "blocks") == 3.0, "{:?}", with_int.status);
}

#[test]
fn factorial_follow_ups_and_ancova() {
    let r = r();
    let with = |extra: serde_json::Value| -> ResultBundle {
        let mut v = serde_json::json!({ "dependent": "score", "factors": ["cond", "sex"] });
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        anova_factorial::run(&preview(), &serde_json::from_value(v).unwrap()).unwrap()
    };
    let t2 = with(serde_json::json!({ "ss_type": "2" }));
    close(f(&t2, "effect.1.ss"), &r["fx.t2.cond.ss"], 1e-9);
    close(f(&t2, "effect.2.ss"), &r["fx.t2.sex.ss"], 1e-9);
    close(f(&t2, "effect.2.p"), &r["fx.t2.sex.p"], 1e-9);
    let tk = with(serde_json::json!({ "post_hoc": "tukey", "simple_effects": true, "plots": true }));
    close(f(&tk, "factor.1.level.2.emm"), &r["fx.emm2"], 1e-12);
    close(f(&tk, "factor.1.level.2.se"), &r["fx.emm2.se"], 1e-9);
    close(f(&tk, "factor.1.level.2.ci.lower"), &r["fx.emm2.lo"], 1e-9);
    close(f(&tk, "pair.1.p"), &r["fx.tukey.12.p"], 1e-6);
    close(f(&tk, "pair.3.p"), &r["fx.tukey.23.p"], 1e-6);
    close(f(&tk, "pair.1.ci.lower"), &r["fx.tukey.12.lo"], 1e-6);
    // emmeans prints joint_tests' F to three decimals; its p value is exact.
    close(f(&tk, "simple.1.F"), &r["fx.simple.sex1.F"], 1e-3);
    close(f(&tk, "simple.1.p"), &r["fx.simple.sex1.p"], 1e-8);
    close(f(&tk, "simple.5.F"), &r["fx.simple.cond3.F"], 3e-3);
    assert_eq!(tk.charts.len(), 3, "profile and residual plots");
    let bf = with(serde_json::json!({ "post_hoc": "bonferroni" }));
    close(f(&bf, "pair.2.p"), &r["fx.bonf.13.p"], 1e-9);
    close(f(&bf, "pair.2.ci.upper"), &r["fx.bonf.13.hi"], 1e-8);
    let hm = with(serde_json::json!({ "post_hoc": "holm" }));
    close(f(&hm, "pair.1.p"), &r["fx.holm.12.p"], 1e-9);
    let main = with(serde_json::json!({ "interactions": false, "ss_type": "2" }));
    close(f(&main, "effect.1.ss"), &r["fx.main.cond.ss"], 1e-9);
    close(f(&main, "error.df"), &r["fx.main.dfe"], 0.0);
    let anc = with(serde_json::json!({ "covariates": ["pre"] }));
    close(f(&anc, "n"), &r["fx.anc.n"], 0.0);
    assert_eq!(anc.text("effect.1.key"), Some("pre"));
    close(f(&anc, "effect.1.F"), &r["fx.anc.pre.F"], 1e-9);
    close(f(&anc, "effect.2.F"), &r["fx.anc.cond.F"], 1e-9);
    close(f(&anc, "effect.2.p"), &r["fx.anc.cond.p"], 1e-9);
    close(f(&anc, "factor.1.level.1.emm"), &r["fx.anc.emm1"], 1e-9);
    close(f(&anc, "slopes.F"), &r["fx.anc.slopes.F"], 1e-8);
    close(f(&anc, "slopes.p"), &r["fx.anc.slopes.p"], 1e-8);
}

#[test]
fn oneway_contrasts_and_adjusted_pairs() {
    let r = r();
    let with = |extra: serde_json::Value| {
        let mut v = serde_json::json!({ "dependent": ["score"], "factor": "cond", "contrasts": [[1.0, -0.5, -0.5], [0.0, 1.0, -1.0]] });
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        anova_oneway::run(&preview(), &serde_json::from_value(v).unwrap()).unwrap().remove(0)
    };
    let holm = with(serde_json::json!({ "contrast_adjust": "holm", "plots": true }));
    assert!(!matches!(holm.status, Status::Failed { .. }));
    close(f(&holm, "contrast.1.est"), &r["ow.c1.est"], 1e-12);
    close(f(&holm, "contrast.1.se"), &r["ow.c1.se"], 1e-10);
    close(f(&holm, "contrast.1.t"), &r["ow.c1.t"], 1e-9);
    close(f(&holm, "contrast.1.df"), &r["ow.c1.df"], 0.0);
    close(f(&holm, "contrast.1.p"), &r["ow.c1.p"], 1e-9);
    close(f(&holm, "contrast.1.lower"), &r["ow.c1.lo"], 1e-9);
    close(f(&holm, "contrast.1.p_adj"), &r["ow.c1.p_holm"], 1e-9);
    close(f(&holm, "contrast.2.p_adj"), &r["ow.c2.p_holm"], 1e-8);
    assert_eq!(holm.charts.len(), 3, "means, residual and Q-Q plots");
    let text = paragraph(&holm).unwrap();
    assert!(text.contains("Contrast 2 (0, 1, −1) was significant"), "{text}");
    assert!(text.contains("(Holm-adjusted)"), "{text}");
    let bonf = with(serde_json::json!({ "contrast_adjust": "bonferroni" }));
    close(f(&bonf, "contrast.2.upper"), &r["ow.c2.hi_bonf"], 1e-9);
    close(f(&bonf, "contrast.2.p_adj"), &r["ow.c2.p_bonf"], 1e-8);
    let welch = with(serde_json::json!({ "estimator": "welch" }));
    close(f(&welch, "contrast.1.se"), &r["ow.c1.welch.se"], 1e-10);
    close(f(&welch, "contrast.1.df"), &r["ow.c1.welch.df"], 1e-10);
    close(f(&welch, "contrast.1.p"), &r["ow.c1.welch.p"], 1e-9);
    let poly = with(serde_json::json!({ "contrasts": [], "polynomial": true }));
    assert_eq!(poly.text("contrast.1.label"), Some("Linear trend"));
    close(f(&poly, "contrast.1.t"), &r["ow.lin.t"], 1e-9);
    close(f(&poly, "contrast.1.est"), &r["ow.lin.est"], 1e-10);
    close(f(&poly, "contrast.2.p"), &r["ow.quad.p"], 1e-8);
    assert!(paragraph(&poly).unwrap().contains("The quadratic trend was significant"));
    let bp = with(serde_json::json!({ "contrasts": [], "post_hoc": "bonferroni" }));
    close(f(&bp, "pair.2.p"), &r["ow.bonf.31.p"], 1e-9);
    let hp = with(serde_json::json!({ "contrasts": [], "post_hoc": "holm" }));
    close(f(&hp, "pair.1.p"), &r["ow.holm.21.p"], 1e-9);
    let sp = with(serde_json::json!({ "contrasts": [], "post_hoc": "scheffe" }));
    assert!(f(&sp, "pair.1.p") >= f(&hp, "pair.1.p"));
    let bad = serde_json::json!({ "dependent": ["score"], "factor": "cond", "contrasts": [[1.0, 1.0, 0.0]] });
    assert!(anova_oneway::run(&preview(), &serde_json::from_value(bad).unwrap()).is_err());
}

#[test]
fn one_sided_tests() {
    let r = r();
    let run = |cmd: &str, v: serde_json::Value, ds: &Dataset| -> ResultBundle {
        match cmd {
            "one" => ttest_one_sample::run(ds, &serde_json::from_value(v).unwrap()).unwrap().remove(0),
            "paired" => ttest_paired::run(ds, &serde_json::from_value(v).unwrap()).unwrap().remove(0),
            "ind" => ttest_independent::run(ds, &serde_json::from_value(v).unwrap()).unwrap().remove(0),
            "corr" => correlation::run(ds, &serde_json::from_value(v).unwrap()).unwrap().remove(0),
            "mw" => rank_tests::mann_whitney(ds, &serde_json::from_value(v).unwrap()).unwrap().remove(0),
            _ => rank_tests::wilcoxon(ds, &serde_json::from_value(v).unwrap()).unwrap().remove(0),
        }
    };
    let ds = preview();
    let g = run("one", serde_json::json!({ "test_variables": ["score"], "test_value": 70, "alternative": "greater" }), &ds);
    close(f(&g, "p"), &r["os.t1.p"], 1e-9);
    close(f(&g, "ci.lower"), &r["os.t1.lo"], 1e-10);
    assert!(g.number("ci.upper").is_none());
    let text = paragraph(&g).unwrap();
    assert!(text.contains("(one-sided)") && text.contains(", ∞]"), "{text}");
    let l = run("one", serde_json::json!({ "test_variables": ["score"], "test_value": 70, "alternative": "less" }), &ds);
    close(f(&l, "p"), &r["os.t2.p"], 1e-8);
    close(f(&l, "ci.upper"), &r["os.t2.hi"], 1e-10);
    assert!(paragraph(&l).unwrap().contains("[−∞, "));
    let pa = run("paired", serde_json::json!({ "pairs": [["pre", "post"]], "alternative": "less" }), &ds);
    close(f(&pa, "p"), &r["os.paired.p"], 1e-8);
    close(f(&pa, "ci.upper"), &r["os.paired.hi"], 1e-10);
    let ind = run("ind", serde_json::json!({ "test_variables": ["score"], "grouping_variable": "sex", "groups": [1, 2], "alternative": "greater" }), &ds);
    close(f(&ind, "p"), &r["os.ind.p"], 1e-9);
    close(f(&ind, "ci.lower"), &r["os.ind.lo"], 1e-10);
    assert!(paragraph(&ind).unwrap().contains("(one-sided)"));
    let mw = run("mw", serde_json::json!({ "test_variables": ["score"], "grouping_variable": "sex", "groups": [1, 2], "alternative": "less" }), &ds);
    close(f(&mw, "p"), &r["os.mw.p"], 1e-9);
    let wx = run("wx", serde_json::json!({ "pairs": [["pre", "post"]], "alternative": "greater" }), &ds);
    close(f(&wx, "p"), &r["os.wx.p"], 1e-9);
    let pg = run("corr", serde_json::json!({ "variables": ["score", "hours"], "alternative": "greater" }), &ds);
    close(f(&pg, "pair.1.2.p"), &r["os.pear.p"], 1e-9);
    close(f(&pg, "pair.1.2.ci.lower"), &r["os.pear.lo"], 1e-10);
    close(f(&pg, "pair.1.2.ci.upper"), &serde_json::json!(1.0), 0.0);
    let pl = run("corr", serde_json::json!({ "variables": ["score", "hours"], "alternative": "less" }), &ds);
    close(f(&pl, "pair.1.2.p"), &r["os.pear.less.p"], 1e-9);
    close(f(&pl, "pair.1.2.ci.upper"), &r["os.pear.less.hi"], 1e-10);
    let sp = run("corr", serde_json::json!({ "variables": ["score", "hours"], "coefficients": ["spearman"], "alternative": "less" }), &ds);
    close(f(&sp, "pair.1.2.p"), &r["os.spear.t.p"], 1e-9);
    let kd = run("corr", serde_json::json!({ "variables": ["score", "hours"], "coefficients": ["kendall"], "alternative": "greater" }), &ds);
    close(f(&kd, "pair.1.2.p"), &r["os.kend.z.p"], 1e-9);
    let holm = run("corr", serde_json::json!({ "variables": ["score", "hours", "pre"], "adjust": "holm" }), &ds);
    for k in ["12", "13", "23"] {
        let key = format!("pair.{}.{}.p_adj", &k[..1], &k[1..]);
        close(f(&holm, &key), &r[format!("adj.holm.{k}")], 1e-9);
    }
    assert!(paragraph(&holm).unwrap().contains("adjusted for the number of pairs by Holm's method"));

    // The exact distributions, on small samples with no ties.
    let x8 = [1.2, 3.4, 2.2, 5.1, 4.0, 6.3, 0.7, 2.9];
    let y8 = [2.0, 3.1, 1.5, 4.8, 5.5, 5.9, 1.1, 2.4];
    let dd = [0.5, -1.3, 2.1, 0.8, -0.2, 1.7, 2.6, -0.9];
    let col = |v: &[f64]| prism_engine::Column::Numeric(v.iter().map(|x| Some(*x)).collect());
    let mut small = Dataset::new("small");
    small.add_variable("x", col(&x8)).unwrap();
    small.add_variable("y", col(&y8)).unwrap();
    small.add_variable("b", col(&x8.iter().zip(&dd).map(|(a, d)| a - d).collect::<Vec<_>>())).unwrap();
    let mut twelve = Dataset::new("twelve");
    twelve.add_variable("x", col(&[&x8[..], &[7.7, 0.2, 3.9, 8.8]].concat())).unwrap();
    twelve.add_variable("y", col(&[&y8[..], &[4.1, 6.6, 0.4, 7.2]].concat())).unwrap();
    let mut groups = Dataset::new("groups");
    groups.add_variable("g", col(&[[1.0; 8], [2.0; 8]].concat())).unwrap();
    groups.add_variable("v", col(&[x8, y8].concat())).unwrap();
    for alt in ["greater", "less"] {
        let sp8 = run("corr", serde_json::json!({ "variables": ["x", "y"], "coefficients": ["spearman"], "alternative": alt }), &small);
        close(f(&sp8, "pair.1.2.p"), &r[format!("xs.spear8.{alt}")], 1e-12);
        let sp12 = run("corr", serde_json::json!({ "variables": ["x", "y"], "coefficients": ["spearman"], "alternative": alt }), &twelve);
        close(f(&sp12, "pair.1.2.p"), &r[format!("xs.spear12.{alt}")], 1e-12);
        let k8 = run("corr", serde_json::json!({ "variables": ["x", "y"], "coefficients": ["kendall"], "alternative": alt }), &small);
        close(f(&k8, "pair.1.2.p"), &r[format!("xs.kend8.{alt}")], 1e-12);
        let m = run("mw", serde_json::json!({ "test_variables": ["v"], "grouping_variable": "g", "groups": [1, 2], "alternative": alt }), &groups);
        close(f(&m, "p"), &r[format!("xs.mw.{alt}")], 1e-12);
        let w = run("wx", serde_json::json!({ "pairs": [["x", "b"]], "alternative": alt }), &small);
        close(f(&w, "p"), &r[format!("xs.wx.{alt}")], 1e-12);
    }
}

#[test]
fn curve_estimation() {
    let r = r();
    let kinds = ["linear", "logarithmic", "inverse", "quadratic", "cubic", "power", "exponential", "s_curve"];
    let spec = serde_json::json!({ "dependent": ["score"], "independent": "hours", "models": kinds });
    let b = curve::run(&preview(), &serde_json::from_value(spec).unwrap()).unwrap().remove(0);
    close(f(&b, "n"), &r["cv.n"], 0.0);
    for (i, k) in kinds.iter().enumerate() {
        let m = format!("model.{}", i + 1);
        assert_eq!(b.text(&format!("{m}.kind")), Some(*k));
        close(f(&b, &format!("{m}.r2")), &r[format!("cv.{k}.r2")], 1e-9);
        close(f(&b, &format!("{m}.F")), &r[format!("cv.{k}.F")], 1e-8);
        close(f(&b, &format!("{m}.p")), &r[format!("cv.{k}.p")], 1e-8);
        close(f(&b, &format!("{m}.term.1.se")), &r[format!("cv.{k}.se1")], 1e-8);
        close(f(&b, &format!("{m}.b1")), &r[format!("cv.{k}.b1")], 1e-8);
        let b0 = r[format!("cv.{k}.b0")].as_f64().unwrap();
        let want = if matches!(*k, "power" | "exponential") { b0.exp() } else { b0 };
        close(f(&b, &format!("{m}.b0")), &serde_json::json!(want), 1e-9);
    }
    close(f(&b, "model.5.b3"), &r["cv.cubic.b3"], 1e-7);
    close(f(&b, "model.6.r2_original"), &r["cv.power.r2o"], 1e-9);
    assert_eq!(b.charts.len(), 1);
    let text = paragraph(&b).unwrap();
    assert!(text.starts_with("Curve estimation related exam score to study hours (*N* = 58)."), "{text}");
    assert!(text.contains("on the logarithmic scale"), "{text}");
    // A curve that can't be fitted is named, and the others still run.
    let mut ds = preview();
    let hours = match &ds.columns[ds.find("hours").unwrap()] {
        prism_engine::Column::Numeric(v) => v.iter().map(|v| v.map(|x| x - 10.0)).collect(),
        _ => unreachable!(),
    };
    ds.add_variable("centered", prism_engine::Column::Numeric(hours)).unwrap();
    let spec = serde_json::json!({ "dependent": ["score"], "independent": "centered", "models": ["linear", "logarithmic"] });
    let b = curve::run(&ds, &serde_json::from_value(spec).unwrap()).unwrap().remove(0);
    assert!(b.number("model.1.r2").is_some() && b.number("model.2.r2").is_none());
    assert!(paragraph(&b).unwrap().contains("The logarithmic curve could not be fitted."));
}

#[test]
fn bootstrap_intervals() {
    let r = r();
    let ds = preview();
    let run = |v: serde_json::Value| bootstrap::run(&ds, &serde_json::from_value(v).unwrap()).unwrap();
    for (stat, key) in [("mean", "bt.mean"), ("median", "bt.median"), ("trimmed_mean", "bt.trim"), ("sd", "bt.sd")] {
        let b = run(serde_json::json!({ "statistic": stat, "variables": ["score"] }));
        close(f(&b, "row.1.estimate"), &r[key], 1e-12);
        let (lo, hi) = (f(&b, "row.1.bca.lower"), f(&b, "row.1.bca.upper"));
        assert!(lo < f(&b, "row.1.estimate") && f(&b, "row.1.estimate") < hi, "{stat}: [{lo}, {hi}]");
    }
    // The bootstrap SE of a mean is close to s / sqrt(n); 2,000 resamples put it within a few percent.
    let m = run(serde_json::json!({ "statistic": "mean", "variables": ["score"] }));
    close(f(&m, "row.1.se"), &r["bt.se.mean"], 0.06);
    // The same seed repeats exactly; another seed moves the interval a little.
    let again = run(serde_json::json!({ "statistic": "mean", "variables": ["score"] }));
    assert_eq!(f(&m, "row.1.bca.lower"), f(&again, "row.1.bca.lower"));
    let other = run(serde_json::json!({ "statistic": "mean", "variables": ["score"], "seed": 7 }));
    assert_ne!(f(&m, "row.1.bca.lower"), f(&other, "row.1.bca.lower"));
    let c = run(serde_json::json!({ "statistic": "correlation", "variables": ["score", "hours"] }));
    close(f(&c, "row.1.estimate"), &r["bt.r"], 1e-12);
    assert_eq!(c.text("row.1.key"), Some("pair.1.2"));
    let s = run(serde_json::json!({ "statistic": "correlation", "variables": ["score", "hours"], "coefficient": "spearman" }));
    close(f(&s, "row.1.estimate"), &r["bt.rho"], 1e-12);
    let g = run(serde_json::json!({ "statistic": "regression", "dependent": "score", "variables": ["hours", "pre"], "resamples": 1000 }));
    close(f(&g, "row.1.estimate"), &r["bt.b0"], 1e-10);
    close(f(&g, "row.2.estimate"), &r["bt.b1"], 1e-10);
    // With well-behaved errors the bootstrap SE of a slope is near the usual one.
    let ratio = f(&g, "row.2.se") / r["bt.b1.se"].as_f64().unwrap();
    assert!((0.7..1.4).contains(&ratio), "SE ratio {ratio}");
    let text = paragraph(&g).unwrap();
    assert!(text.starts_with("Bias-corrected and accelerated bootstrap intervals (1,000 resamples) were computed for the coefficients of the regression of exam score:"), "{text}");
    assert!(bootstrap::run(&ds, &serde_json::from_value(serde_json::json!({ "variables": ["score"], "resamples": 10 })).unwrap()).is_err());
}

#[test]
fn data_changes() {
    use prism_engine::data_ops::{self, From, Rule, ScaleMethod, ScaleSpec, To};
    let r = r();
    let numbers = |ds: &Dataset, name: &str| match &ds.columns[ds.find(name).unwrap()] {
        prism_engine::Column::Numeric(v) => v.clone(),
        _ => panic!("{name} is not numeric"),
    };
    // D1: user-missing codes are missing in a formula, as R's NA.
    let mut ds = preview();
    let rep = data_ops::compute(&mut ds, "mix", "ifelse(pre > 50, (pre + post) / 2, log(post))", Some("A mixture"), false).unwrap();
    let v = numbers(&ds, "mix");
    close(v.iter().flatten().sum(), &r["dc.compute.sum"], 1e-12);
    close(rep.missing as f64, &r["dc.compute.na"], 0.0);
    assert_eq!(ds.variable("mix").unwrap().label, "A mixture");
    assert!(data_ops::compute(&mut ds, "mix", "pre", None, false).is_err(), "won't replace without asking");
    data_ops::compute(&mut ds, "mix", "pre * 2", None, true).unwrap();
    let t = data_ops::compute(&mut ds, "older", "ifelse(score > 60, \"high\", \"low\")", None, false).unwrap();
    assert_eq!(t.valid + t.missing, ds.n_rows());
    assert!(matches!(ds.columns[ds.find("older").unwrap()], prism_engine::Column::Text(_)));
    assert!(data_ops::compute(&mut ds, "bad", "nosuch + 1", None, false).unwrap_err().to_string().contains("nosuch"));

    // D3: scale scores.
    let mut sc = scale();
    let spec = |method: ScaleMethod, target: &str| ScaleSpec {
        target: target.into(),
        items: ["i1", "i2", "i3", "i4", "i5"].map(String::from).to_vec(),
        method,
        reverse: vec!["i2".into()],
        scale_min: Some(1.0),
        scale_max: Some(5.0),
        min_items: Some(4),
        label: None,
        replace: false,
    };
    data_ops::scale_score(&mut sc, &spec(ScaleMethod::Mean, "anx")).unwrap();
    let a = numbers(&sc, "anx");
    close(a.iter().flatten().sum(), &r["dc.scale.sum"], 1e-12);
    close(a[0].unwrap(), &r["dc.scale.1"], 1e-12);
    data_ops::scale_score(&mut sc, &spec(ScaleMethod::ProratedSum, "anx_sum")).unwrap();
    close(numbers(&sc, "anx_sum").iter().flatten().sum(), &r["dc.prorated.sum"], 1e-12);
    assert!(sc.variable("anx").unwrap().label.contains("(i2 reversed)"));

    // D2: recode into a new variable; rules apply in order.
    let mut ds = preview();
    let rules = vec![
        Rule { from: From::Missing, to: To::Value { value: 0.0 } },
        Rule { from: From::Value { value: 1.0 }, to: To::Value { value: 1.0 } },
        Rule { from: From::Range { lo: None, hi: Some(3.0) }, to: To::Value { value: 2.0 } },
        Rule { from: From::Else, to: To::Missing },
    ];
    data_ops::recode(&mut ds, &["cond".into()], &rules, Some(&["cond2".into()]), None).unwrap();
    let c = numbers(&ds, "cond2");
    for (val, key) in [(1.0, "dc.recode.1"), (2.0, "dc.recode.2"), (0.0, "dc.recode.0")] {
        close(c.iter().filter(|x| **x == Some(val)).count() as f64, &r[key], 0.0);
    }
    close(c.iter().filter(|x| x.is_none()).count() as f64, &r["dc.recode.na"], 0.0);
    // In place, unmatched values stay.
    data_ops::recode(&mut ds, &["cond".into()], &[Rule { from: From::Value { value: 3.0 }, to: To::Value { value: 30.0 } }], None, None).unwrap();
    let c = numbers(&ds, "cond");
    assert!(c.contains(&Some(30.0)) && c.contains(&Some(1.0)) && !c.contains(&Some(3.0)));
}

#[test]
fn select_weight_and_split_cases() {
    use prism_engine::subset::{parts, DataSettings};
    let r = r();
    let ds = preview();
    let settings = |filter: Option<&str>, split: Option<&str>, weight: Option<&str>| DataSettings { filter: filter.map(str::to_string), split: split.map(str::to_string), weight: weight.map(str::to_string) };
    let one = |s: DataSettings| {
        let mut p = parts(&ds, &s).unwrap();
        assert_eq!(p.len(), 1);
        p.remove(0)
    };
    let f = one(settings(Some("sex == 1 & hours > 5"), None, None));
    let b = ttest_one_sample::run(&f.data, &serde_json::from_value(serde_json::json!({ "test_variables": ["score"] })).unwrap()).unwrap().remove(0);
    close(b.number("n").unwrap(), &r["sub.filter.n"], 0.0);
    close(b.number("mean").unwrap(), &r["sub.filter.mean"], 1e-12);
    assert!(f.note.contains("sex == 1 & hours > 5"));
    // Frequency weights: cond (1-3, with 9 user-missing) as the weight.
    let w = one(settings(None, None, Some("cond")));
    let t = ttest_independent::run(&w.data, &serde_json::from_value(serde_json::json!({ "test_variables": ["score"], "grouping_variable": "sex", "groups": [1, 2] })).unwrap()).unwrap().remove(0);
    close(t.number("t").unwrap(), &r["sub.weight.t"], 1e-10);
    close(t.number("df").unwrap(), &r["sub.weight.df"], 1e-10);
    close(t.cases.analyzed as f64, &r["sub.weight.n"], 0.0);
    assert!(w.note.contains("frequency weights"));
    // Split file: one part for each sex.
    let s = parts(&ds, &settings(None, Some("sex"), None)).unwrap();
    assert_eq!(s.iter().map(|p| p.group.clone().unwrap()).collect::<Vec<_>>(), ["sex = Female", "sex = Male"]);
    for (k, p) in s.iter().enumerate() {
        let a = anova_oneway::run(&p.data, &serde_json::from_value(serde_json::json!({ "dependent": ["score"], "factor": "cond" })).unwrap()).unwrap().remove(0);
        close(a.number("F").unwrap(), &r[format!("sub.split.F.{}", k + 1)], 1e-10);
    }
    // Weights must be whole numbers.
    let mut odd = preview();
    prism_engine::data_ops::compute(&mut odd, "half", "hours / 2", None, false).unwrap();
    assert!(parts(&odd, &settings(None, None, Some("half"))).err().unwrap().to_string().contains("whole numbers"));
    assert!(parts(&ds, &settings(Some("nosuch > 1"), None, None)).is_err());
}

#[test]
fn repeated_measures_and_mixed_anova() {
    let r = r();
    let run = |v: serde_json::Value| rm_anova::run(&preview(), &serde_json::from_value(v).unwrap()).unwrap();
    let rm = run(serde_json::json!({ "measures": ["pre", "post", "score"], "post_hoc": "holm", "plots": true }));
    close(f(&rm, "within.F"), &r["rm.F"], 1e-10);
    close(f(&rm, "within.ss"), &r["rm.ss"], 1e-10);
    close(f(&rm, "within.p"), &r["rm.p"], 1e-8);
    close(f(&rm, "mauchly.W"), &r["rm.mauchly.W"], 1e-10);
    close(f(&rm, "mauchly.p"), &r["rm.mauchly.p"], 1e-8);
    close(f(&rm, "eps.gg"), &r["rm.gg"], 1e-10);
    close(f(&rm, "eps.hf"), &r["rm.hf"], 1e-10);
    close(f(&rm, "within.gg.p"), &r["rm.gg.p"], 1e-8);
    close(f(&rm, "within.hf.p"), &r["rm.hf.p"], 1e-8);
    close(f(&rm, "within.ges"), &r["rm.ges"], 1e-10);
    close(f(&rm, "within.pes"), &r["rm.pes"], 1e-10);
    close(f(&rm, "pair.2.p"), &r["rm.holm.31"], 1e-8);
    assert_eq!(rm.charts.len(), 1);
    let text = paragraph(&rm).unwrap();
    assert!(text.contains("Greenhouse-Geisser"), "{text}");
    let mx = run(serde_json::json!({ "measures": ["pre", "post", "score"], "between": "sex" }));
    close(f(&mx, "between.F"), &r["mx.sex.F"], 1e-10);
    close(f(&mx, "between.p"), &r["mx.sex.p"], 1e-9);
    close(f(&mx, "within.F"), &r["mx.time.F"], 1e-10);
    close(f(&mx, "interaction.F"), &r["mx.int.F"], 1e-10);
    close(f(&mx, "interaction.p"), &r["mx.int.p"], 1e-9);
    close(f(&mx, "eps.gg"), &r["mx.gg"], 1e-10);
    close(f(&mx, "eps.hf"), &r["mx.hf"], 1e-10);
    close(f(&mx, "mauchly.W"), &r["mx.mauchly.W"], 1e-10);
    close(f(&mx, "mauchly.p"), &r["mx.mauchly.p"], 1e-8);
    close(f(&mx, "interaction.ges"), &r["mx.int.ges"], 1e-10);
    close(f(&mx, "between.ges"), &r["mx.sex.ges"], 1e-10);
    let text = paragraph(&mx).unwrap();
    assert!(text.starts_with("A mixed analysis of variance"), "{text}");
}

#[test]
fn one_way_manova() {
    let r = r();
    let b = manova::run(&preview(), &serde_json::from_value(serde_json::json!({ "dependent": ["pre", "post", "score"], "factor": "cond" })).unwrap()).unwrap();
    for (key, rk) in [("pillai", "pilla"), ("wilks", "wilks"), ("hotelling", "hotel"), ("roy", "roy")] {
        close(f(&b, &format!("{key}.stat")), &r[format!("mv.{rk}.stat")], 1e-10);
        close(f(&b, &format!("{key}.F")), &r[format!("mv.{rk}.F")], 1e-9);
        close(f(&b, &format!("{key}.df2")), &r[format!("mv.{rk}.df2")], 1e-12);
        close(f(&b, &format!("{key}.p")), &r[format!("mv.{rk}.p")], 1e-9);
    }
    close(f(&b, "boxm.chi2"), &r["mv.boxm.chi2"], 1e-9);
    close(f(&b, "boxm.p"), &r["mv.boxm.p"], 1e-9);
    close(f(&b, "dv.3.F"), &r["mv.uni.score.F"], 1e-10);
    let text = paragraph(&b).unwrap();
    assert!(text.contains("Pillai's trace"), "{text}");
}

#[test]
fn factor_analysis() {
    let r = r();
    let run = |v: serde_json::Value| {
        let mut base = serde_json::json!({ "variables": ["i1", "i2", "i3", "i4", "i5"], "factors": 2 });
        base.as_object_mut().unwrap().extend(v.as_object().unwrap().clone());
        factor::run(&scale(), &serde_json::from_value(base).unwrap()).unwrap()
    };
    let ss = |b: &ResultBundle, k: usize| {
        let mut v = [f(b, "factor.1.ss"), f(b, "factor.2.ss")];
        v.sort_by(|a, b| b.total_cmp(a));
        v[k]
    };
    let ml = run(serde_json::json!({ "extraction": "ml", "rotation": "none" }));
    close(f(&ml, "n"), &r["fa.n"], 0.0);
    close(f(&ml, "kmo"), &r["fa.kmo"], 1e-10);
    close(f(&ml, "var.1.msa"), &r["fa.msa1"], 1e-10);
    close(f(&ml, "bartlett.chi2"), &r["fa.bart.chi2"], 1e-10);
    close(f(&ml, "bartlett.p"), &r["fa.bart.p"], 1e-6);
    close(1.0 - f(&ml, "var.1.communality"), &r["fa.ml.u1"], 1e-5);
    close(1.0 - f(&ml, "var.5.communality"), &r["fa.ml.u5"], 1e-5);
    close(f(&ml, "fit.chi2"), &r["fa.ml.chi2"], 1e-4);
    // Principal axis and minimum residual on one factor (two factors give an ultra-Heywood case here, flagged).
    let pa = run(serde_json::json!({ "extraction": "paf", "rotation": "none", "factors": 1 }));
    close(f(&pa, "var.1.communality"), &r["fa.pa.h1"], 1e-6);
    close(f(&pa, "var.4.communality"), &r["fa.pa.h4"], 1e-6);
    let mr = run(serde_json::json!({ "extraction": "minres", "rotation": "none", "factors": 1 }));
    close(f(&mr, "var.1.communality"), &r["fa.mr.h1"], 1e-5);
    let pa2 = run(serde_json::json!({ "extraction": "paf", "rotation": "none" }));
    assert!(pa2.diagnostics.iter().any(|d| d.id == "heywood"));
    let vm = run(serde_json::json!({ "extraction": "ml", "rotation": "varimax" }));
    close(ss(&vm, 0), &r["fa.vm.ss1"], 1e-5);
    close(ss(&vm, 1), &r["fa.vm.ss2"], 1e-5);
    let ob = run(serde_json::json!({ "extraction": "ml", "rotation": "oblimin" }));
    close(f(&ob, "phi.1.2").abs(), &r["fa.ob.phi"], 1e-5);
    close(ss(&ob, 0), &r["fa.ob.ss1"], 1e-5);
    let text = paragraph(&ob).unwrap();
    assert!(text.starts_with("A maximum likelihood factor analysis with oblimin rotation"), "{text}");
    assert!(text.contains("The two factors correlated"), "{text}");
    let pm = run(serde_json::json!({ "extraction": "ml", "rotation": "promax" }));
    close(f(&pm, "phi.1.2").abs(), &r["fa.pm.phi"], 1e-5);
    close(ss(&pm, 0), &r["fa.pm.ss1"], 1e-5);
    let pc = run(serde_json::json!({ "extraction": "pca", "rotation": "none" }));
    close(f(&pc, "eigen.1"), &r["fa.pc.ev1"], 1e-10);
    close(f(&pc, "var.3.communality"), &r["fa.pc.h3"], 1e-10);
    // Parallel analysis decides when no number is given; the same seed gives the same answer.
    let auto = run(serde_json::json!({ "factors": null }));
    let again = run(serde_json::json!({ "factors": null }));
    assert_eq!(f(&auto, "parallel.1"), f(&again, "parallel.1"));
    assert!(f(&auto, "m") >= 1.0);
    assert_eq!(auto.charts.len(), 1);
}

#[test]
fn mediation_and_moderation() {
    let r = r();
    let ds = preview();
    let me = process::mediation(&ds, &serde_json::from_value(serde_json::json!({ "x": "hours", "mediators": ["pre", "post"], "y": "score", "resamples": 1000 })).unwrap()).unwrap();
    close(f(&me, "n"), &r["me.n"], 0.0);
    close(f(&me, "a.1.b"), &r["me.a1"], 1e-10);
    close(f(&me, "a.1.se"), &r["me.a1.se"], 1e-10);
    close(f(&me, "b.2.b"), &r["me.b2"], 1e-10);
    close(f(&me, "b.2.se"), &r["me.b2.se"], 1e-10);
    close(f(&me, "direct.b"), &r["me.direct"], 1e-10);
    close(f(&me, "direct.p"), &r["me.direct.p"], 1e-8);
    close(f(&me, "total.b"), &r["me.total"], 1e-10);
    close(f(&me, "indirect.1.effect"), &r["me.ind1"], 1e-10);
    // Total = direct + indirect, exactly, for OLS.
    close(f(&me, "indirect.total.effect") + f(&me, "direct.b"), &r["me.total"], 1e-10);
    assert!(f(&me, "indirect.1.lower") < f(&me, "indirect.1.upper"));
    assert!(paragraph(&me).unwrap().contains("indirect"));
    let mo = process::moderation(&ds, &serde_json::from_value(serde_json::json!({ "x": "hours", "w": "pre", "y": "score" })).unwrap()).unwrap();
    close(f(&mo, "xw.b"), &r["mo.xw"], 1e-10);
    close(f(&mo, "xw.p"), &r["mo.xw.p"], 1e-8);
    close(f(&mo, "slope.1.w"), &r["mo.w16"], 1e-12);
    close(f(&mo, "slope.1.b"), &r["mo.slope16"], 1e-10);
    close(f(&mo, "slope.1.se"), &r["mo.slope16.se"], 1e-10);
    close(f(&mo, "jn.1"), &r["mo.jn.lo"], 1e-6);
    close(f(&mo, "jn.2"), &r["mo.jn.hi"], 1e-6);
    let mc = process::moderation(&ds, &serde_json::from_value(serde_json::json!({ "x": "hours", "w": "pre", "y": "score", "center": true })).unwrap()).unwrap();
    close(f(&mc, "x.b"), &r["mo.c.x"], 1e-10);
    // Centering doesn't move the simple slopes or the Johnson-Neyman values.
    close(f(&mc, "slope.1.b"), &r["mo.slope16"], 1e-10);
    close(f(&mc, "jn.1"), &r["mo.jn.lo"], 1e-6);
    assert_eq!(mo.charts.len(), 1);
    assert!(paragraph(&mo).unwrap().contains("Johnson-Neyman"));
}

#[test]
fn ordinal_multinomial_and_count_regression() {
    let r = r();
    let ds = preview();
    let spec = |v: serde_json::Value| -> glm_more::Spec { serde_json::from_value(v).unwrap() };
    let or = glm_more::ordinal(&ds, &spec(serde_json::json!({ "dependent": "cond", "covariates": ["hours"], "factors": [{ "name": "sex" }] }))).unwrap();
    close(f(&or, "n"), &r["or.n"], 0.0);
    close(f(&or, "term.1.b"), &r["or.hours"], 1e-6);
    close(f(&or, "term.1.se"), &r["or.hours.se"], 1e-4);
    close(f(&or, "term.2.b"), &r["or.sex"], 1e-6);
    close(f(&or, "theta.2"), &r["or.theta2"], 1e-6);
    close(f(&or, "loglik"), &r["or.ll"], 1e-9);
    assert!(paragraph(&or).unwrap().contains("ordinal"));
    let mn = glm_more::multinomial(&ds, &spec(serde_json::json!({ "dependent": "cond", "covariates": ["hours"], "factors": [{ "name": "sex" }] }))).unwrap();
    close(f(&mn, "cat.1.term.1.b"), &r["mn.c2.hours"], 1e-6);
    close(f(&mn, "cat.2.term.2.b"), &r["mn.c3.sex"], 1e-6);
    close(f(&mn, "cat.1.term.1.se"), &r["mn.c2.hours.se"], 1e-4);
    close(f(&mn, "loglik"), &r["mn.ll"], 1e-9);
    close(f(&mn, "lr.1.chi2"), &r["mn.lr.hours"], 1e-6);
    assert!(paragraph(&mn).unwrap().contains("multinomial"));
    let mut cd = preview();
    prism_engine::data_ops::compute(&mut cd, "cnt", "round(hours)", None, false).unwrap();
    let po = glm_more::count(&cd, &spec(serde_json::json!({ "dependent": "cnt", "covariates": ["pre"], "factors": [{ "name": "sex" }] }))).unwrap();
    close(f(&po, "n"), &r["ct.n"], 0.0);
    close(f(&po, "term.1.b"), &r["ct.p.pre"], 1e-9);
    close(f(&po, "term.1.se"), &r["ct.p.pre.se"], 1e-8);
    close(f(&po, "deviance"), &r["ct.p.dev"], 1e-9);
    close(f(&po, "aic"), &r["ct.p.aic"], 1e-9);
    assert!(po.diagnostics.iter().any(|d| d.id == "dispersion"));
    let nb = glm_more::count(&cd, &spec(serde_json::json!({ "dependent": "cnt", "covariates": ["pre"], "factors": [{ "name": "sex" }], "family": "negative_binomial" }))).unwrap();
    close(f(&nb, "term.1.b"), &r["ct.nb.pre"], 1e-7);
    close(f(&nb, "term.1.se"), &r["ct.nb.pre.se"], 1e-6);
    close(f(&nb, "theta"), &r["ct.nb.theta"], 1e-6);
    close(f(&nb, "loglik"), &r["ct.nb.ll"], 1e-9);
    assert!(paragraph(&nb).unwrap().contains("negative binomial"));
}

#[test]
fn robust_errors_and_trimmed_means() {
    let r = r();
    let ds = preview();
    let reg = |robust: &str| regression::run(&ds, &serde_json::from_value(serde_json::json!({ "dependent": "score", "covariates": ["hours", "pre"], "robust": robust })).unwrap()).unwrap();
    let h3 = reg("hc3");
    close(f(&h3, "term.1.se"), &r["hc3.hours.se"], 1e-10);
    close(f(&h3, "term.1.p"), &r["hc3.hours.p"], 1e-8);
    close(f(&h3, "robust.F"), &r["hc3.F"], 1e-10);
    close(f(&reg("hc1"), "term.1.se"), &r["hc1.hours.se"], 1e-10);
    let yu = trimmed::yuen(&ds, &serde_json::from_value(serde_json::json!({ "test_variables": ["score"], "grouping_variable": "sex", "groups": [1, 2] })).unwrap()).unwrap().remove(0);
    // WRS2::yuen reports |t|.
    close(f(&yu, "t").abs(), &r["yu.t"], 1e-10);
    close(f(&yu, "df"), &r["yu.df"], 1e-10);
    close(f(&yu, "p"), &r["yu.p"], 1e-9);
    close(f(&yu, "diff"), &r["yu.diff"], 1e-12);
    close(f(&yu, "ci.lower"), &r["yu.lo"], 1e-10);
    let yd = trimmed::yuen_paired(&ds, &serde_json::from_value(serde_json::json!({ "pairs": [["pre", "post"]] })).unwrap()).unwrap().remove(0);
    close(f(&yd, "t"), &r["yd.t"], 1e-10);
    close(f(&yd, "df"), &r["yd.df"], 0.0);
    close(f(&yd, "p"), &r["yd.p"], 1e-8);
    close(f(&yd, "diff"), &r["yd.diff"], 1e-12);
    let t1 = trimmed::t1way(&ds, &serde_json::from_value(serde_json::json!({ "dependent": ["score"], "factor": "cond" })).unwrap()).unwrap().remove(0);
    close(f(&t1, "F"), &r["t1.F"], 1e-10);
    close(f(&t1, "df2"), &r["t1.df2"], 1e-10);
    close(f(&t1, "p"), &r["t1.p"], 1e-9);
    for b in [&yu, &yd, &t1] {
        assert!(paragraph(b).is_some(), "{}", b.title);
    }
    assert!(paragraph(&h3).unwrap().contains("HC3"));
}

#[test]
fn linear_mixed_models() {
    let r = r();
    let (ds, _) = read_file(&reference_dir().join("sleepstudy.csv")).unwrap();
    let run = |v: serde_json::Value| mixed::run(&ds, &serde_json::from_value(v).unwrap()).unwrap();
    let m1 = run(serde_json::json!({ "dependent": "Reaction", "covariates": ["Days"], "group": "Subject" }));
    close(f(&m1, "term.1.b"), &r["lmm1.days"], 1e-8);
    close(f(&m1, "term.1.se"), &r["lmm1.days.se"], 1e-6);
    close(f(&m1, "term.1.df"), &r["lmm1.days.df"], 1e-4);
    close(f(&m1, "b0.df"), &r["lmm1.int.df"], 1e-4);
    close(f(&m1, "re.1.var"), &r["lmm1.tau"], 1e-6);
    close(f(&m1, "sigma2"), &r["lmm1.sigma2"], 1e-6);
    close(f(&m1, "deviance"), &r["lmm1.reml"], 1e-9);
    let m2 = run(serde_json::json!({ "dependent": "Reaction", "covariates": ["Days"], "group": "Subject", "random_slopes": ["Days"] }));
    close(f(&m2, "term.1.se"), &r["lmm2.days.se"], 1e-5);
    close(f(&m2, "term.1.df"), &r["lmm2.days.df"], 1e-3);
    close(f(&m2, "deviance"), &r["lmm2.reml"], 1e-9);
    close(f(&m2, "re.1.var"), &r["lmm2.tau00"], 1e-4);
    close(f(&m2, "re.2.var"), &r["lmm2.tau11"], 1e-4);
    close(f(&m2, "re.cor.1.2"), &r["lmm2.cor"], 1e-3);
    close(f(&m2, "sigma2"), &r["lmm2.sigma2"], 1e-5);
    let m3 = run(serde_json::json!({ "dependent": "Reaction", "covariates": ["Days"], "group": "Subject", "reml": false }));
    close(f(&m3, "deviance"), &r["lmm3.dev"], 1e-9);
    close(f(&m3, "re.1.var"), &r["lmm3.tau"], 1e-6);
    assert!(paragraph(&m2).unwrap().contains("mixed"));
}

#[test]
fn missing_patterns_and_multiple_imputation() {
    let r = r();
    let ds = preview();
    let pat = missing::patterns(&ds, &serde_json::from_value(serde_json::json!({ "variables": ["pre", "post", "score", "hours"] })).unwrap()).unwrap();
    close(f(&pat, "little.chi2"), &r["little.chi2"], 1e-8);
    close(f(&pat, "little.df"), &r["little.df"], 0.0);
    close(f(&pat, "little.chi2"), &r["little.naniar"], 1e-5);
    assert!(f(&pat, "patterns") >= 2.0);
    assert!(paragraph(&pat).unwrap().contains("Little"));
    // Rubin's rules as mice::pool.scalar.
    let pl = missing::pool(&[1.2, 1.5, 1.1, 1.4, 1.3], &[0.04, 0.05, 0.045, 0.05, 0.042], 97.0);
    close(pl.estimate, &r["pool.qbar"], 1e-12);
    close(pl.total, &r["pool.t"], 1e-12);
    close(pl.df, &r["pool.df"], 1e-10);
    close(pl.fmi, &r["pool.fmi"], 1e-10);
    close(pl.riv, &r["pool.r"], 1e-10);
    // With nothing missing every imputation is the data, so the pooled fit is ordinary least squares.
    let (sleep, _) = read_file(&reference_dir().join("sleepstudy.csv")).unwrap();
    let mi = missing::mi_regression(&sleep, &serde_json::from_value(serde_json::json!({ "dependent": "Reaction", "covariates": ["Days"], "imputations": 5 })).unwrap()).unwrap();
    let ols = regression::run(&sleep, &serde_json::from_value(serde_json::json!({ "dependent": "Reaction", "covariates": ["Days"] })).unwrap()).unwrap();
    close(f(&mi, "term.1.b"), &serde_json::json!(f(&ols, "term.1.b")), 1e-10);
    close(f(&mi, "term.1.se"), &serde_json::json!(f(&ols, "term.1.se")), 1e-10);
    // With missing values: reproducible from the seed, and the intervals widen beyond complete-case ones.
    let spec = serde_json::json!({ "dependent": "score", "covariates": ["hours", "pre"], "auxiliary": ["post"], "imputations": 10 });
    let a = missing::mi_regression(&ds, &serde_json::from_value(spec.clone()).unwrap()).unwrap();
    let again = missing::mi_regression(&ds, &serde_json::from_value(spec).unwrap()).unwrap();
    assert_eq!(f(&a, "term.1.b"), f(&again, "term.1.b"));
    assert!(f(&a, "term.1.fmi") > 0.0 && f(&a, "term.1.fmi") < 1.0);
    assert!((f(&a, "term.1.b") - r["bt.b1"].as_f64().unwrap()).abs() < 0.3, "MI slope near the complete-case one");
    assert!(paragraph(&a).unwrap().contains("imputation"));
}

#[test]
fn bayes_factor_formulas() {
    use prism_engine::analysis::bayes;
    let r = r();
    let ds = preview();
    let t1 = ttest_one_sample::run(&ds, &serde_json::from_value(serde_json::json!({ "test_variables": ["score"], "test_value": 60 })).unwrap()).unwrap().remove(0);
    let n = f(&t1, "n");
    close(bayes::ttest_log_bf(f(&t1, "t"), n, n - 1.0, 2f64.sqrt() / 2.0).exp(), &r["bf.one"], 1e-7);
    let tp = ttest_paired::run(&ds, &serde_json::from_value(serde_json::json!({ "pairs": [["pre", "post"]] })).unwrap()).unwrap().remove(0);
    let n = f(&tp, "n");
    close(bayes::ttest_log_bf(f(&tp, "t"), n, n - 1.0, 2f64.sqrt() / 2.0).exp(), &r["bf.paired"], 1e-7);
    let ti = ttest_independent::run(&ds, &serde_json::from_value(serde_json::json!({ "test_variables": ["score"], "grouping_variable": "sex", "groups": [1, 2], "estimator": "student" })).unwrap()).unwrap().remove(0);
    let (n1, n2) = (30.0, 29.0);
    close(bayes::ttest_log_bf(f(&ti, "t"), n1 * n2 / (n1 + n2), n1 + n2 - 2.0, 2f64.sqrt() / 2.0).exp(), &r["bf.ind"], 1e-7);
    let c = correlation::run(&ds, &serde_json::from_value(serde_json::json!({ "variables": ["score", "hours"] })).unwrap()).unwrap().remove(0);
    close(bayes::correlation_log_bf(f(&c, "pair.1.2.n"), f(&c, "pair.1.2.r"), 1.0 / 3.0).exp(), &r["bf.cor"], 1e-8);
    // Through the analyses, with the option on.
    let run = |cmd: &str, v: serde_json::Value| -> ResultBundle {
        match cmd {
            "one" => ttest_one_sample::run(&ds, &serde_json::from_value(v).unwrap()).unwrap().remove(0),
            "ind" => ttest_independent::run(&ds, &serde_json::from_value(v).unwrap()).unwrap().remove(0),
            "cor" => correlation::run(&ds, &serde_json::from_value(v).unwrap()).unwrap().remove(0),
            "ct" => crosstabs::run(&ds, &serde_json::from_value(v).unwrap()).unwrap(),
            "aov" => anova_oneway::run(&ds, &serde_json::from_value(v).unwrap()).unwrap().remove(0),
            _ => regression::run(&ds, &serde_json::from_value(v).unwrap()).unwrap(),
        }
    };
    close(f(&run("one", serde_json::json!({ "test_variables": ["score"], "test_value": 60, "bayes": true })), "bf10"), &r["bf.one"], 1e-7);
    // Welch is the default test, but the Bayes factor uses the pooled t, as ttestBF does.
    close(f(&run("ind", serde_json::json!({ "test_variables": ["score"], "grouping_variable": "sex", "groups": [1, 2], "bayes": true })), "bf10"), &r["bf.ind"], 1e-7);
    close(f(&run("cor", serde_json::json!({ "variables": ["score", "hours"], "bayes": true })), "pair.1.2.bf10"), &r["bf.cor"], 1e-8);
    for (sampling, key) in [("joint_multinomial", "bf.ct.joint"), ("rows", "bf.ct.rows"), ("columns", "bf.ct.cols"), ("poisson", "bf.ct.pois")] {
        close(f(&run("ct", serde_json::json!({ "row": "sex", "column": "smoker", "bayes": true, "sampling": sampling })), "bf10"), &r[key], 1e-9);
    }
    // anovaBF reports this with a relative error of about 2e-4.
    close(f(&run("aov", serde_json::json!({ "dependent": ["score"], "factor": "cond", "bayes": true })), "bf10"), &r["bf.anova"], 1e-3);
    let reg = run("reg", serde_json::json!({ "dependent": "score", "covariates": ["hours", "pre"], "bayes": true }));
    close(f(&reg, "bf10"), &r["bf.reg"], 1e-6);
    assert!(paragraph(&reg).unwrap().contains("BF"));
}

#[test]
fn permutation_tests() {
    let r = r();
    let x8 = [1.2, 3.4, 2.2, 5.1, 4.0, 6.3, 0.7, 2.9];
    let y8 = [2.0, 3.1, 1.5, 4.8, 5.5, 5.9, 1.1, 2.4];
    let dd = [0.5, -1.3, 2.1, 0.8, -0.2, 1.7, 2.6, -0.9];
    let two = permutation::two_groups(&x8, &y8, 1000, 100_000, 1);
    assert!(two.exact && two.count == 12_870);
    close(two.p, &r["perm.two"], 1e-12);
    let pd = permutation::paired(&dd, 1000, 100_000, 1);
    assert!(pd.exact && pd.count == 256);
    close(pd.p, &r["perm.paired"], 1e-12);
    let pc = permutation::correlation(&x8, &y8, 1000, 100_000, 1);
    assert!(pc.exact && pc.count == 40_320);
    close(pc.p, &r["perm.cor"], 1e-12);
    // Monte Carlo on the preview data: seeded, and close to the usual tests' p values.
    let ds = preview();
    let run = |v: serde_json::Value| permutation::run(&ds, &serde_json::from_value(v).unwrap()).unwrap();
    let mc = run(serde_json::json!({ "test": "two_groups", "variables": ["score"], "group": "sex", "groups": [1, 2], "resamples": 20000 }));
    assert_eq!(f(&mc, "exact"), 0.0);
    let again = run(serde_json::json!({ "test": "two_groups", "variables": ["score"], "group": "sex", "groups": [1, 2], "resamples": 20000 }));
    assert_eq!(f(&mc, "p"), f(&again, "p"));
    let student = ttest_independent::run(&ds, &serde_json::from_value(serde_json::json!({ "test_variables": ["score"], "grouping_variable": "sex", "groups": [1, 2], "estimator": "student" })).unwrap()).unwrap().remove(0);
    assert!((f(&mc, "p") - f(&student, "p")).abs() < 0.03, "{} vs {}", f(&mc, "p"), f(&student, "p"));
    let an = run(serde_json::json!({ "test": "anova", "variables": ["score"], "group": "cond", "resamples": 20000 }));
    let aov = anova_oneway::run(&ds, &serde_json::from_value(serde_json::json!({ "dependent": ["score"], "factor": "cond" })).unwrap()).unwrap().remove(0);
    close(f(&an, "statistic"), &serde_json::json!(f(&aov, "F")), 1e-12);
    assert!((f(&an, "p") - f(&aov, "p")).abs() < 0.01);
    assert!(paragraph(&an).unwrap().contains("permutation"));
}

#[test]
fn survival_and_roc() {
    let r = r();
    let (lung, _) = read_file(&reference_dir().join("lung.csv")).unwrap();
    let km = survival::km(&lung, &serde_json::from_value(serde_json::json!({ "time": "time", "status": "status", "events": [2], "group": "sex" })).unwrap()).unwrap();
    close(f(&km, "group.1.median"), &r["km.med1"], 0.0);
    close(f(&km, "group.1.median.lower"), &r["km.med1.lo"], 0.0);
    close(f(&km, "group.1.median.upper"), &r["km.med1.hi"], 0.0);
    close(f(&km, "group.2.median"), &r["km.med2"], 0.0);
    close(f(&km, "logrank.chi2"), &r["lr.chi2"], 1e-10);
    // The overall curve at time 100.
    let ts = |n: &str| -> Vec<Option<f64>> { match &lung.columns[lung.find(n).unwrap()] { prism_engine::Column::Numeric(v) => v.clone(), _ => unreachable!() } };
    let data: Vec<(f64, bool)> = ts("time").iter().zip(ts("status")).map(|(t, s)| (t.unwrap(), s == Some(2.0))).collect();
    let c = survival::kaplan_meier(&data);
    let at100 = c.steps.iter().rev().find(|s| s.0 <= 100.0).unwrap();
    close(at100.4, &r["km.all.s100"], 1e-12);
    close(at100.4 * at100.5, &r["km.all.se100"], 1e-10);
    let cx = survival::cox(&lung, &serde_json::from_value(serde_json::json!({ "time": "time", "status": "status", "events": [2], "covariates": ["age"], "factors": [{ "name": "sex" }] })).unwrap()).unwrap();
    close(f(&cx, "term.1.b"), &r["cox.age"], 1e-8);
    close(f(&cx, "term.1.se"), &r["cox.age.se"], 1e-7);
    close(f(&cx, "term.2.hr"), &r["cox.sex.hr"], 1e-8);
    close(f(&cx, "lr.chi2"), &r["cox.lr"], 1e-8);
    close(f(&cx, "wald.chi2"), &r["cox.wald"], 1e-7);
    close(f(&cx, "score.chi2"), &r["cox.score"], 1e-8);
    let roc = survival::roc(&preview(), &serde_json::from_value(serde_json::json!({ "variables": ["score"], "state": "smoker", "positive": 2 })).unwrap()).unwrap();
    close(f(&roc, "var.1.auc"), &r["roc.auc2"], 1e-12);
    close(f(&roc, "var.1.lower"), &r["roc.lo2"], 1e-10);
    close(f(&roc, "var.1.cutoff"), &r["roc.best"], 1e-12);
    for b in [&km, &cx, &roc] {
        assert!(paragraph(b).is_some(), "{}", b.title);
    }
}

#[test]
fn time_series() {
    let r = r();
    let (lh, _) = read_file(&reference_dir().join("lakehuron.csv")).unwrap();
    let (nile, _) = read_file(&reference_dir().join("nile.csv")).unwrap();
    let a = timeseries::run_acf(&lh, &serde_json::from_value(serde_json::json!({ "variable": "level", "lags": 10 })).unwrap()).unwrap();
    close(f(&a, "acf.1"), &r["ts.acf1"], 1e-12);
    close(f(&a, "acf.5"), &r["ts.acf5"], 1e-12);
    close(f(&a, "pacf.2"), &r["ts.pacf2"], 1e-12);
    close(f(&a, "lb.Q"), &r["ts.lb"], 1e-10);
    let hw = timeseries::run_smooth(&nile, &serde_json::from_value(serde_json::json!({ "variable": "flow", "forecast": 2 })).unwrap()).unwrap();
    // R's optimize stops at about 1e-4 in α; the SSE is flat there.
    close(f(&hw, "alpha"), &r["hw.alpha"], 1e-3);
    close(f(&hw, "sse"), &r["hw.sse"], 1e-6);
    close(f(&hw, "level"), &r["hw.level"], 1e-4);
    let ar = timeseries::run_arima(&lh, &serde_json::from_value(serde_json::json!({ "variable": "level", "p": 1, "q": 1 })).unwrap()).unwrap();
    close(f(&ar, "ar1.est"), &r["ar.ar1"], 1e-5);
    close(f(&ar, "ma1.est"), &r["ar.ma1"], 1e-5);
    close(f(&ar, "mean.est"), &r["ar.mean"], 1e-7);
    close(f(&ar, "ar1.se"), &r["ar.ar1.se"], 1e-3);
    close(f(&ar, "sigma2"), &r["ar.sigma2"], 1e-6);
    close(f(&ar, "loglik"), &r["ar.loglik"], 1e-9);
    close(f(&ar, "aic"), &r["ar.aic"], 1e-9);
    let a2 = timeseries::run_arima(&nile, &serde_json::from_value(serde_json::json!({ "variable": "flow", "p": 0, "d": 1, "q": 1, "forecast": 3 })).unwrap()).unwrap();
    close(f(&a2, "ma1.est"), &r["ar2.ma1"], 1e-5);
    close(f(&a2, "sigma2"), &r["ar2.sigma2"], 1e-6);
    close(f(&a2, "forecast.1"), &r["ar2.f1"], 1e-6);
    close(f(&a2, "forecast.3.se"), &r["ar2.f3.se"], 1e-5);
    for b in [&a, &hw, &ar] {
        assert!(paragraph(b).is_some(), "{}", b.title);
    }
}

#[test]
fn survey_design() {
    let r = r();
    let (st, _) = read_file(&reference_dir().join("apistrat.csv")).unwrap();
    let (cl, _) = read_file(&reference_dir().join("apiclus1.csv")).unwrap();
    let est = |ds: &Dataset, v: serde_json::Value| survey::estimates(ds, &serde_json::from_value(v).unwrap()).unwrap();
    let m = est(&st, serde_json::json!({ "variables": ["api00", "enroll"], "weight": "pw", "strata": "stype" }));
    close(f(&m, "est.1.value"), &r["sv.mean.api"], 1e-12);
    close(f(&m, "est.1.se"), &r["sv.mean.api.se"], 1e-10);
    close(f(&m, "est.2.se"), &r["sv.mean.enroll.se"], 1e-10);
    let t = est(&st, serde_json::json!({ "variables": ["enroll"], "weight": "pw", "strata": "stype", "statistic": "total" }));
    close(f(&t, "est.1.value"), &r["sv.total"], 1e-12);
    close(f(&t, "est.1.se"), &r["sv.total.se"], 1e-10);
    let pr = est(&st, serde_json::json!({ "variables": ["sch_wide"], "weight": "pw", "strata": "stype" }));
    assert_eq!(pr.text("est.2.label"), Some("sch_wide: Yes"));
    close(f(&pr, "est.2.value"), &r["sv.prop.yes"], 1e-12);
    close(f(&pr, "est.2.se"), &r["sv.prop.yes.se"], 1e-10);
    let c = est(&cl, serde_json::json!({ "variables": ["api00"], "weight": "pw", "cluster": "dnum" }));
    close(f(&c, "est.1.value"), &r["sv.cl.mean"], 1e-12);
    close(f(&c, "est.1.se"), &r["sv.cl.se"], 1e-10);
    let g = survey::regression(&cl, &serde_json::from_value(serde_json::json!({ "dependent": "api00", "covariates": ["ell", "meals"], "weight": "pw", "cluster": "dnum" })).unwrap()).unwrap();
    close(f(&g, "term.1.b"), &r["sv.glm.ell"], 1e-10);
    close(f(&g, "term.1.se"), &r["sv.glm.ell.se"], 1e-9);
    close(f(&g, "term.1.p"), &r["sv.glm.ell.p"], 1e-8);
    for b in [&m, &g] {
        assert!(paragraph(b).is_some(), "{}", b.title);
    }
}
