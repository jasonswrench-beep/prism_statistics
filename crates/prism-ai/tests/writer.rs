//! The AI writer's slots and checks on real results, with hand-written drafts that are right and drafts that make each
//! mistake the checks exist for. The last test runs the model itself when it is on this computer (PRISM_AI_MODEL_DIR).

use std::path::PathBuf;

use prism_ai::{finish, prepare, CheckState, Draft};
use prism_engine::analysis::{anova_oneway, ttest_independent};
use prism_engine::model::ValueLabel;
use prism_engine::{Dataset, ResultBundle};

fn dataset() -> Dataset {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../prism-engine/tests/reference/preview.csv");
    let (mut ds, _) = prism_engine::import::read_file(&path).unwrap();
    let c = ds.variable_mut("cond").unwrap();
    c.label = "Condition".into();
    c.value_labels = vec![ValueLabel { value: 1.0, label: "Lecture".into() }, ValueLabel { value: 2.0, label: "Discussion".into() }, ValueLabel { value: 3.0, label: "Online".into() }];
    c.missing_values = vec![9.0];
    let s = ds.variable_mut("sex").unwrap();
    s.value_labels = vec![ValueLabel { value: 1.0, label: "Female".into() }, ValueLabel { value: 2.0, label: "Male".into() }];
    ds.variable_mut("score").unwrap().label = "exam score".into();
    ds
}

/// Discussion vs Online on exam score: significant, Discussion higher.
fn ttest() -> ResultBundle {
    ttest_independent::run(&dataset(), &ttest_independent::Spec { test_variables: vec!["score".into()], grouping_variable: "cond".into(), groups: (2.0, 3.0), estimator: Default::default(), level: 0.95, alternative: Default::default(), bayes: Default::default(), prior_scale: Default::default() }).unwrap().remove(0)
}

fn state(d: &Draft, id: &str) -> CheckState {
    d.checks.iter().find(|c| c.id == id).unwrap().state
}

const GOOD: &str = "An independent-samples *t* test ({method}) compared {y.label} between the {g1.label} and {g2.label} groups. {y.label} was higher in the {g1.label} group (*M* = {g1.mean}, *SD* = {g1.sd}) than in the {g2.label} group (*M* = {g2.mean}, *SD* = {g2.sd}), *t*({df}) = {t}, *p* {p}, mean difference = {diff}, {ci}, Hedges' *g* = {g}, {g.ci}.";

#[test]
fn slots_carry_formatted_values_and_meanings() {
    let b = ttest();
    let input = prepare(&b).unwrap();
    let slot = |n: &str| input.slots.iter().find(|s| s.name == n).unwrap_or_else(|| panic!("no slot {n}"));
    assert!(slot("p").value.starts_with("= .") || slot("p").value == "< .001");
    assert!(slot("ci").value.starts_with("95% CI ["), "{}", slot("ci").value);
    assert!(slot("g.ci").meaning.contains("Hedges' g"));
    assert_eq!(slot("g1.label").value, "Discussion");
    assert!(slot("g1.mean").meaning.contains("Discussion"));
    assert!(input.user.contains("Discussion is higher than Online"), "{}", input.user);
    assert!(input.user.contains("was significant"));
    // Values are never shown to the model, only names and meanings.
    assert!(!input.user.contains(&slot("g1.mean").value), "{}", input.user);
    assert!(!input.slots.iter().any(|s| s.name.ends_with("ci.lower")));
    assert_eq!(input.slots.iter().filter(|s| s.name == "level").map(|s| s.value.as_str()).collect::<Vec<_>>(), ["95%"]);
}

#[test]
fn a_correct_draft_passes_every_check() {
    let b = ttest();
    let input = prepare(&b).unwrap();
    let d = finish(&b, &input, GOOD, 1.0);
    assert!(d.checks.iter().all(|c| c.state == CheckState::Pass || c.id == "diagnostics"), "{:?}", d.checks);
    assert!(d.text.contains("Discussion group (*M* = 66.22"), "{}", d.text);
    assert!(!d.text.contains('{'));
}

#[test]
fn each_mistake_is_caught() {
    let b = ttest();
    let input = prepare(&b).unwrap();
    let wrong_way = GOOD.replace("was higher in the", "was lower in the");
    assert_eq!(state(&finish(&b, &input, &wrong_way, 1.0), "direction"), CheckState::Fail);
    let not_sig = format!("{GOOD} The difference was not significant.");
    assert_eq!(state(&finish(&b, &input, &not_sig, 1.0), "significance"), CheckState::Fail);
    let causal = GOOD.replace("compared", "showed the effect of the condition on");
    assert_eq!(state(&finish(&b, &input, &causal, 1.0), "causal"), CheckState::Flag);
    let relabeled = GOOD.replace("*t*({df}) = {t}", "*F*({df}) = {t}");
    assert_eq!(state(&finish(&b, &input, &relabeled, 1.0), "notation"), CheckState::Fail);
    let thin = "The {g1.label} group scored higher, *t*({df}) = {t}, *p* {p}.";
    let d = finish(&b, &input, thin, 1.0);
    assert_eq!(state(&d, "coverage"), CheckState::Flag);
    assert!(d.checks.iter().find(|c| c.id == "coverage").unwrap().message.contains("g1.mean"));
    assert_eq!(state(&finish(&b, &input, "It was {t} and {nonsense}.", 1.0), "numbers"), CheckState::Fail);
    assert_eq!(state(&finish(&b, &input, "There were 12 people.", 1.0), "numbers"), CheckState::Fail);
    assert_eq!(state(&finish(&b, &input, "The mean was sixty-six point two.", 1.0), "numbers"), CheckState::Fail);
}

#[test]
fn significance_is_checked_against_each_p_cited() {
    use prism_engine::analysis::correlation;
    let b = correlation::run(&dataset(), &correlation::Spec { variables: vec!["pre".into(), "hours".into(), "score".into()], coefficients: vec![correlation::Kind::Pearson], missing: Default::default(), level: 0.95, adjust: Default::default(), alternative: Default::default(), bayes: Default::default(), prior_scale: Default::default() }).unwrap().remove(0);
    let input = prepare(&b).unwrap();
    // pre and hours are not significantly correlated; hours and score are.
    let wrong = "The correlation between {var.1.label} and {var.2.label} was significant, *r* = {pair.1.2.r}, *p* {pair.1.2.p}.";
    assert_eq!(state(&finish(&b, &input, wrong, 1.0), "significance"), CheckState::Fail);
    let right = "The correlation between {var.1.label} and {var.2.label} was not significant, *r* = {pair.1.2.r}, *p* {pair.1.2.p}. The correlation between {var.2.label} and {var.3.label} was significant, *r* = {pair.2.3.r}, *p* {pair.2.3.p}.";
    assert_eq!(state(&finish(&b, &input, right, 1.0), "significance"), CheckState::Pass);
    let mixed = "{var.1.label} and {var.2.label} were not significantly correlated (*p* {pair.1.2.p}), and {var.2.label} and {var.3.label} were significantly correlated (*p* {pair.2.3.p}).";
    assert_eq!(state(&finish(&b, &input, mixed, 1.0), "significance"), CheckState::Pass);
    let swapped = "{var.1.label} and {var.2.label} were significantly correlated (*p* {pair.1.2.p}), and {var.2.label} and {var.3.label} were not (*p* {pair.2.3.p}).";
    assert_eq!(state(&finish(&b, &input, swapped, 1.0), "significance"), CheckState::Fail);
    assert_eq!(state(&finish(&b, &input, "With a Ninth confidence interval.", 1.0), "numbers"), CheckState::Fail);
    assert_eq!(state(&finish(&b, &input, "With a -ninefive percent interval.", 1.0), "numbers"), CheckState::Fail);
    assert_eq!(state(&finish(&b, &input, "None of the three nineteen-year-olds.", 1.0), "numbers"), CheckState::Fail);
    assert_eq!(state(&finish(&b, &input, "Someone often fivefold.", 1.0), "numbers"), CheckState::Pass);
}

#[test]
fn a_failed_result_gets_no_draft() {
    let mut ds = Dataset::new("x");
    ds.add_variable("g", prism_engine::Column::Numeric(vec![Some(1.0), Some(2.0)])).unwrap();
    ds.add_variable("y", prism_engine::Column::Numeric(vec![Some(1.0), Some(1.0)])).unwrap();
    let b = anova_oneway::run(&ds, &anova_oneway::Spec { dependent: vec!["y".into()], factor: "g".into(), estimator: Default::default(), post_hoc: Default::default(), level: 0.95, ..Default::default() }).unwrap().remove(0);
    assert!(prepare(&b).unwrap_err().contains("can't be interpreted"));
}

/// Runs the real model when its files are on this computer. Slow (about half a minute).
#[test]
fn the_model_writes_a_checked_paragraph() {
    let Some(dir) = std::env::var_os("PRISM_AI_MODEL_DIR").map(PathBuf::from) else { return };
    let b = ttest();
    let input = prepare(&b).unwrap();
    let (raw, secs) = prism_ai::generate(&dir, &input).unwrap();
    let d = finish(&b, &input, &raw, secs);
    eprintln!("{secs:.1}s\nRAW: {raw}\nTEXT: {}\nCHECKS: {:?}", d.text, d.checks.iter().map(|c| (&c.id, c.state)).collect::<Vec<_>>());
    assert!(!d.rejected(), "{:?}", d.checks);
    assert_eq!(state(&d, "numbers"), CheckState::Pass);
}

/// The model on other analyses, when it is on this computer: every draft must pass the hard checks.
#[test]
fn the_model_handles_other_methods() {
    use prism_engine::analysis::{correlation, crosstabs, regression};
    let Some(dir) = std::env::var_os("PRISM_AI_MODEL_DIR").map(PathBuf::from) else { return };
    let ds = dataset();
    let bundles = vec![
        anova_oneway::run(&ds, &anova_oneway::Spec { dependent: vec!["score".into()], factor: "cond".into(), estimator: Default::default(), post_hoc: anova_oneway::PostHoc::Tukey, level: 0.95, ..Default::default() }).unwrap().remove(0),
        correlation::run(&ds, &correlation::Spec { variables: vec!["pre".into(), "hours".into(), "score".into()], coefficients: vec![correlation::Kind::Pearson], missing: Default::default(), level: 0.95, adjust: Default::default(), alternative: Default::default(), bayes: Default::default(), prior_scale: Default::default() }).unwrap().remove(0),
        regression::run(&ds, &regression::Spec { dependent: "score".into(), covariates: vec!["hours".into()], factors: vec![regression::Factor { name: "cond".into(), reference: None }], level: 0.95, blocks: vec![], interactions: vec![], center: false, plots: false, robust: Default::default(), bayes: Default::default(), prior_scale: Default::default() }).unwrap(),
        crosstabs::run(&ds, &crosstabs::Spec { row: "sex".into(), column: "smoker".into(), percentages: vec![crosstabs::Percent::Row], chi_square: true, continuity: false, expected: false, residuals: false, fisher: false, risk: false, ordinal: false, layer: None, bayes: Default::default(), sampling: Default::default() }).unwrap(),
    ];
    for b in &bundles {
        let input = prepare(b).unwrap();
        let (raw, secs) = prism_ai::generate(&dir, &input).unwrap();
        let d = finish(b, &input, &raw, secs);
        eprintln!("\n== {} ({secs:.1}s)\n{}\n{:?}", b.title, d.text, d.checks.iter().filter(|c| c.state != CheckState::Pass).map(|c| (&c.id, c.state, &c.message)).collect::<Vec<_>>());
        assert_eq!(state(&d, "numbers"), CheckState::Pass, "{}", b.title);
    }
}
