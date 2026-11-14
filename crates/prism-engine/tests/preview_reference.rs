//! The preview analyses against R (reference/preview_reference.json, made by make_preview_reference.R from
//! preview.csv). The data have an empty cell or a user-missing code in most variables, and R gets the same exclusions
//! written out by hand, so these tests check the case handling as well as the numbers.

use std::path::PathBuf;

use prism_engine::analysis::*;
use prism_engine::import::read_file;
use prism_engine::model::ValueLabel;
use prism_engine::result::{DiagnosticState, Status};
use prism_engine::templates::paragraph;
use prism_engine::{Dataset, ResultBundle};

fn reference_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("reference")
}

fn r() -> serde_json::Value {
    serde_json::from_str(
        &std::fs::read_to_string(reference_dir().join("preview_reference.json")).unwrap(),
    )
    .unwrap()
}

fn labels(pairs: &[(f64, &str)]) -> Vec<ValueLabel> {
    pairs
        .iter()
        .map(|(v, l)| ValueLabel {
            value: *v,
            label: l.to_string(),
        })
        .collect()
}

fn dataset() -> Dataset {
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

#[track_caller]
fn close(got: f64, want: &serde_json::Value, tol: f64) {
    let want = want
        .as_f64()
        .unwrap_or_else(|| panic!("no reference value {want:?}"));
    let err = (got - want).abs() / want.abs().max(1.0);
    assert!(
        err <= tol,
        "got {got}, R gives {want} (relative error {err:e})"
    );
}

fn f(b: &ResultBundle, k: &str) -> f64 {
    b.number(k)
        .unwrap_or_else(|| panic!("no field {k} in {}", b.title))
}

const TOL: f64 = 1e-9;

#[test]
fn one_sample_t() {
    let r = r();
    let b = &ttest_one_sample::run(
        &dataset(),
        &ttest_one_sample::Spec {
            test_variables: vec!["score".into()],
            test_value: 60.0,
            level: 0.95, alternative: Default::default(), bayes: Default::default(), prior_scale: Default::default(), },
    )
    .unwrap()[0];
    assert_eq!(b.status, Status::Ok);
    close(f(b, "n"), &r["one.n"], 0.0);
    close(f(b, "mean"), &r["one.mean"], TOL);
    close(f(b, "sd"), &r["one.sd"], TOL);
    close(f(b, "t"), &r["one.t"], TOL);
    close(f(b, "df"), &r["one.df"], 0.0);
    close(f(b, "p"), &r["one.p"], 1e-8);
    close(f(b, "ci.lower"), &r["one.lo"], 1e-8);
    close(f(b, "ci.upper"), &r["one.hi"], 1e-8);
    close(f(b, "d"), &r["one.d"], TOL);
    assert_eq!(b.cases.excluded[0].reason, "test variable empty");
    let text = paragraph(b).unwrap();
    assert!(text.starts_with("A one-sample *t* test compared the mean exam score with 60. The mean exam score (*M* = 61.91, *SD* = 8.75) did not differ significantly from 60, *t*(58) = 1.68, *p* = .099"), "{text}");
    assert!(text.ends_with("(test variable empty: 1)."), "{text}");
}

#[test]
fn paired_t() {
    let r = r();
    let b = &ttest_paired::run(
        &dataset(),
        &ttest_paired::Spec {
            pairs: vec![("pre".into(), "post".into())],
            level: 0.95, alternative: Default::default(), bayes: Default::default(), prior_scale: Default::default(), },
    )
    .unwrap()[0];
    close(f(b, "n"), &r["pair.n"], 0.0);
    close(f(b, "diff"), &r["pair.mean"], TOL);
    close(f(b, "diff.sd"), &r["pair.sd"], TOL);
    close(f(b, "t"), &r["pair.t"], TOL);
    close(f(b, "p"), &r["pair.p"], 1e-8);
    close(f(b, "ci.lower"), &r["pair.lo"], 1e-8);
    close(f(b, "ci.upper"), &r["pair.hi"], 1e-8);
    close(f(b, "d"), &r["pair.dz"], TOL);
    close(f(b, "r"), &r["pair.r"], TOL);
    let reasons: Vec<&str> = b.cases.excluded.iter().map(|e| e.reason.as_str()).collect();
    assert_eq!(
        reasons,
        ["first variable user-missing", "second variable empty"]
    );
    let text = paragraph(b).unwrap();
    assert!(
        text.contains("Pre (*M* = ") && text.contains("was significantly lower than post"),
        "{text}"
    );
    // Swapping the pair flips the sign.
    let s = &ttest_paired::run(
        &dataset(),
        &ttest_paired::Spec {
            pairs: vec![("post".into(), "pre".into())],
            level: 0.95, alternative: Default::default(), bayes: Default::default(), prior_scale: Default::default(), },
    )
    .unwrap()[0];
    close(
        f(s, "diff"),
        &serde_json::json!(-r["pair.mean"].as_f64().unwrap()),
        TOL,
    );
}

#[test]
fn one_way_anova_with_post_hoc() {
    let r = r();
    let spec = |estimator, post_hoc| anova_oneway::Spec {
        dependent: vec!["score".into()],
        factor: "cond".into(),
        estimator,
        post_hoc,
        level: 0.95, ..Default::default()
    };
    let b = &anova_oneway::run(
        &dataset(),
        &spec(
            anova_oneway::Estimator::Conventional,
            anova_oneway::PostHoc::Tukey,
        ),
    )
    .unwrap()[0];
    assert_eq!(b.status, Status::Ok);
    close(f(b, "n"), &r["aov.n"], 0.0);
    close(f(b, "F"), &r["aov.F"], TOL);
    close(f(b, "df1"), &r["aov.df1"], 0.0);
    close(f(b, "df2"), &r["aov.df2"], 0.0);
    close(f(b, "p"), &r["aov.p"], 1e-8);
    close(f(b, "ss.between"), &r["aov.ssb"], TOL);
    close(f(b, "ss.within"), &r["aov.ssw"], TOL);
    close(f(b, "eta2"), &r["aov.eta2"], TOL);
    close(f(b, "omega2"), &r["aov.omega2"], TOL);
    close(f(b, "welch.F"), &r["aov.welchF"], 1e-8);
    close(f(b, "welch.df2"), &r["aov.welchdf2"], 1e-8);
    close(f(b, "welch.p"), &r["aov.welchp"], 1e-8);
    close(f(b, "levene.F"), &r["aov.leveneF"], 1e-8);
    close(f(b, "levene.p"), &r["aov.levenep"], 1e-8);
    close(f(b, "pair.1.diff"), &r["tukey.12.diff"], TOL);
    // The studentized range distribution is computed numerically, as in R; agreement to 1e-6 is what R itself promises.
    close(f(b, "pair.1.p"), &r["tukey.12.p"], 1e-6);
    close(f(b, "pair.2.p"), &r["tukey.13.p"], 1e-6);
    close(f(b, "pair.3.p"), &r["tukey.23.p"], 1e-6);
    let post = b.tables.iter().find(|t| t.id == "post_hoc").unwrap();
    assert_eq!(post.rows.len(), 3);
    let reasons: Vec<&str> = b.cases.excluded.iter().map(|e| e.reason.as_str()).collect();
    assert_eq!(
        reasons,
        [
            "factor user-missing",
            "factor empty",
            "dependent variable empty"
        ]
    );
    let text = paragraph(b).unwrap();
    assert!(
        text.contains("across the three groups of Condition: Lecture (*M* = "),
        "{text}"
    );
    assert!(
        text.contains("*F*(2, 54) = 6.07, *p* = .004, ω² = .15"),
        "{text}"
    );
    assert!(text.contains("Tukey HSD comparisons showed that these groups differed significantly: Discussion and Online (*p* = .003)."), "{text}");

    let g = &anova_oneway::run(
        &dataset(),
        &spec(
            anova_oneway::Estimator::Welch,
            anova_oneway::PostHoc::GamesHowell,
        ),
    )
    .unwrap()[0];
    close(f(g, "pair.1.p"), &r["gh.12.p"], 1e-6);
    close(f(g, "pair.2.p"), &r["gh.13.p"], 1e-6);
    close(f(g, "pair.3.p"), &r["gh.23.p"], 1e-6);
    assert!(paragraph(g)
        .unwrap()
        .contains("Welch's *F*(2, 35.89) = 5.88"));
}

#[test]
fn crosstabs_and_chi_square() {
    let r = r();
    let spec = |row: &str, col: &str| crosstabs::Spec {
        row: row.into(),
        column: col.into(),
        percentages: vec![crosstabs::Percent::Row],
        chi_square: true,
        continuity: false,
        expected: true,
        residuals: true,
        fisher: false,
        risk: false,
        ordinal: false,
        layer: None, bayes: Default::default(), sampling: Default::default(), };
    let b = crosstabs::run(&dataset(), &spec("sex", "smoker")).unwrap();
    close(f(&b, "chi2"), &r["x22.chi2"], TOL);
    close(f(&b, "p"), &r["x22.p"], 1e-8);
    close(f(&b, "n"), &r["x22.n"], 0.0);
    close(f(&b, "phi"), &r["x22.phi"], TOL);
    let text = paragraph(&b).unwrap();
    assert!(
        text.contains("χ²(1, *N* = 60) = 5.41, *p* = .020, φ = .30"),
        "{text}"
    );

    // Text rows (alphabetical) against numeric codes; the user-missing and empty condition cells are left out.
    let c = crosstabs::run(&dataset(), &spec("region", "cond")).unwrap();
    close(f(&c, "chi2"), &r["x33.chi2"], TOL);
    close(f(&c, "df"), &r["x33.df"], 0.0);
    close(f(&c, "p"), &r["x33.p"], 1e-8);
    close(f(&c, "n"), &r["x33.n"], 0.0);
    close(f(&c, "V"), &r["x33.V"], TOL);
    close(f(&c, "g2"), &r["x33.g2"], TOL);
    close(f(&c, "g2.p"), &r["x33.g2p"], 1e-8);
    close(f(&c, "min_expected"), &r["x33.minexp"], TOL);
    close(f(&c, "count.East.1"), &r["x33.East.1"], 0.0);
    let table = &c.tables[0];
    assert_eq!(
        table.columns,
        ["region", "", "Lecture", "Discussion", "Online", "Total"]
    );
    assert!(
        matches!(&table.rows[0][0], prism_engine::result::Cell::Text { text } if text == "East")
    );
    assert!(paragraph(&c).unwrap().contains("Cramér's *V* = .16"));
}

#[test]
fn correlations() {
    let r = r();
    let spec = |coefficients, missing| correlation::Spec {
        variables: vec!["pre".into(), "hours".into(), "score".into()],
        coefficients,
        missing,
        level: 0.95, adjust: Default::default(), alternative: Default::default(), bayes: Default::default(), prior_scale: Default::default(), };
    let out = correlation::run(
        &dataset(),
        &spec(
            vec![correlation::Kind::Pearson, correlation::Kind::Spearman],
            correlation::Missing::Listwise,
        ),
    )
    .unwrap();
    let (p, s) = (&out[0], &out[1]);
    close(p.cases.analyzed as f64, &r["cor.n"], 0.0);
    close(f(p, "pair.2.3.r"), &r["cor.hs.r"], TOL);
    close(f(p, "pair.2.3.p"), &r["cor.hs.p"], 1e-8);
    close(f(p, "pair.2.3.ci.lower"), &r["cor.hs.lo"], 1e-8);
    close(f(p, "pair.2.3.ci.upper"), &r["cor.hs.hi"], 1e-8);
    close(f(p, "pair.1.2.r"), &r["cor.ph.r"], TOL);
    close(f(p, "pair.1.2.p"), &r["cor.ph.p"], 1e-8);
    close(f(s, "pair.2.3.r"), &r["spear.hs.r"], TOL);
    close(f(s, "pair.2.3.p"), &r["spear.hs.p"], 1e-6);
    let text = paragraph(p).unwrap();
    assert!(text.contains("Study hours and exam score were significantly positively correlated, *r*(55) = .46, *p* < .001, 95% CI [.23, .64]."), "{text}");
    assert!(paragraph(s).unwrap().contains("*r*<sub>s</sub>(55) = .44"));

    let pw = &correlation::run(
        &dataset(),
        &spec(
            vec![correlation::Kind::Pearson],
            correlation::Missing::Pairwise,
        ),
    )
    .unwrap()[0];
    close(f(pw, "pair.2.3.n"), &r["cor.pairwise.n"], 0.0);
    close(f(pw, "pair.2.3.r"), &r["cor.pairwise.r"], TOL);
}

#[test]
fn rank_tests() {
    let r = r();
    let ds = dataset();
    let m = &rank_tests::mann_whitney(
        &ds,
        &rank_tests::MannWhitneySpec {
            test_variables: vec!["score".into()],
            grouping_variable: "sex".into(),
            groups: (1.0, 2.0), alternative: Default::default(), },
    )
    .unwrap()[0];
    close(f(m, "g1.n"), &r["mw.n1"], 0.0);
    close(f(m, "g2.n"), &r["mw.n2"], 0.0);
    close(f(m, "U"), &r["mw.U"], TOL);
    close(f(m, "p"), &r["mw.p"], 1e-8);
    close(f(m, "r"), &r["mw.r"], TOL);
    close(f(m, "g1.median"), &r["mw.mdn1"], TOL);
    close(f(m, "g2.median"), &r["mw.mdn2"], TOL);
    assert!(paragraph(m).unwrap().contains("*U* = 418.5"));

    let w = &rank_tests::wilcoxon(
        &ds,
        &rank_tests::WilcoxonSpec {
            pairs: vec![("pre".into(), "post".into())], alternative: Default::default(), },
    )
    .unwrap()[0];
    close(f(w, "V"), &r["wx.V"], TOL);
    close(f(w, "p"), &r["wx.p"], 1e-8);
    close(f(w, "neg"), &r["wx.neg"], 0.0);
    close(f(w, "pos"), &r["wx.pos"], 0.0);
    close(f(w, "ties"), &r["wx.ties"], 0.0);
    close(f(w, "r"), &r["wx.r"], TOL);
    assert!(paragraph(w)
        .unwrap()
        .contains("One pair with no difference was left out of the ranking."));

    let k = &rank_tests::kruskal(
        &ds,
        &rank_tests::KruskalSpec {
            dependent: vec!["score".into()],
            factor: "cond".into(),
            post_hoc: rank_tests::KruskalPostHoc::Dunn, dunn_adjust: Default::default(), },
    )
    .unwrap()[0];
    close(f(k, "H"), &r["kw.H"], TOL);
    close(f(k, "df"), &r["kw.df"], 0.0);
    close(f(k, "p"), &r["kw.p"], 1e-8);
    close(f(k, "pair.1.z"), &r["dunn.12.z"], TOL);
    close(f(k, "pair.1.p"), &r["dunn.12.padj"], 1e-8);
    close(f(k, "pair.3.p"), &r["dunn.23.padj"], 1e-8);
    assert!(paragraph(k).unwrap().contains("*H*(2) = 9.27, *p* = .010"));
}

#[test]
fn linear_regression() {
    let r = r();
    let spec = regression::Spec {
        dependent: "score".into(),
        covariates: vec!["hours".into()],
        factors: vec![regression::Factor {
            name: "cond".into(),
            reference: Some("1".into()),
        }],
        level: 0.95,
        blocks: vec![],
        interactions: vec![],
        center: false,
        plots: false, robust: Default::default(), bayes: Default::default(), prior_scale: Default::default(), };
    let b = regression::run(&dataset(), &spec).unwrap();
    assert_eq!(b.status, Status::Ok);
    close(f(&b, "n"), &r["reg.n"], 0.0);
    close(f(&b, "r2"), &r["reg.r2"], TOL);
    close(f(&b, "adj_r2"), &r["reg.adjr2"], TOL);
    close(f(&b, "F"), &r["reg.F"], TOL);
    close(f(&b, "df1"), &r["reg.df1"], 0.0);
    close(f(&b, "df2"), &r["reg.df2"], 0.0);
    close(f(&b, "p"), &r["reg.p"], 1e-8);
    close(f(&b, "sigma"), &r["reg.sigma"], TOL);
    close(f(&b, "b0.b"), &r["reg.b0"], TOL);
    close(f(&b, "term.1.b"), &r["reg.hours.b"], TOL);
    close(f(&b, "term.1.se"), &r["reg.hours.se"], TOL);
    close(f(&b, "term.1.t"), &r["reg.hours.t"], TOL);
    close(f(&b, "term.1.p"), &r["reg.hours.p"], 1e-8);
    close(f(&b, "term.1.ci.lower"), &r["reg.hours.lo"], 1e-8);
    close(f(&b, "term.1.ci.upper"), &r["reg.hours.hi"], 1e-8);
    close(f(&b, "term.1.beta"), &r["reg.hours.beta"], TOL);
    close(f(&b, "term.1.vif"), &r["reg.hours.vif"], 1e-8);
    close(f(&b, "term.3.b"), &r["reg.f3.b"], TOL);
    close(f(&b, "term.3.p"), &r["reg.f3.p"], 1e-8);
    close(f(&b, "bp"), &r["reg.bp"], 1e-8);
    close(f(&b, "bp.p"), &r["reg.bp.p"], 1e-8);
    assert_eq!(
        b.text("term.3.label"),
        Some("Condition: Online (vs Lecture)")
    );
    let text = paragraph(&b).unwrap();
    assert!(text.starts_with("A linear regression predicted exam score from study hours and Condition. The model significantly explained 41.1% of the variance"), "{text}");

    // A predictor that copies another is named, and nothing is interpreted.
    let mut ds = dataset();
    let hours = ds.columns[ds.find("hours").unwrap()].clone();
    ds.add_variable("hours2", hours).unwrap();
    let bad = regression::run(
        &ds,
        &regression::Spec {
            covariates: vec!["hours".into(), "hours2".into()],
            factors: vec![],
            ..spec
        },
    )
    .unwrap();
    assert!(
        matches!(&bad.status, Status::Failed { reason } if reason.contains("hours2")),
        "{:?}",
        bad.status
    );
    assert!(paragraph(&bad).is_none());
}

#[test]
fn failures_say_why_and_never_interpret() {
    let mut ds = Dataset::new("tiny");
    ds.add_variable(
        "g",
        prism_engine::Column::Numeric(vec![Some(1.0), Some(1.0), Some(2.0), Some(2.0)]),
    )
    .unwrap();
    ds.add_variable(
        "y",
        prism_engine::Column::Numeric(vec![Some(3.0), Some(3.0), Some(3.0), Some(3.0)]),
    )
    .unwrap();
    let a = &anova_oneway::run(
        &ds,
        &anova_oneway::Spec {
            dependent: vec!["y".into()],
            factor: "g".into(),
            estimator: Default::default(),
            post_hoc: Default::default(),
            level: 0.95, ..Default::default()
        },
    )
    .unwrap()[0];
    assert!(matches!(&a.status, Status::Failed { reason } if reason.contains("variation")));
    assert!(paragraph(a).is_none());
    let t = &ttest_one_sample::run(
        &ds,
        &ttest_one_sample::Spec {
            test_variables: vec!["y".into()],
            test_value: 0.0,
            level: 0.95, alternative: Default::default(), bayes: Default::default(), prior_scale: Default::default(), },
    )
    .unwrap()[0];
    assert!(matches!(&t.status, Status::Failed { .. }));
    let c = &correlation::run(
        &ds,
        &correlation::Spec {
            variables: vec!["g".into(), "y".into()],
            coefficients: vec![correlation::Kind::Pearson],
            missing: Default::default(),
            level: 0.95, adjust: Default::default(), alternative: Default::default(), bayes: Default::default(), prior_scale: Default::default(), },
    )
    .unwrap()[0];
    assert!(matches!(&c.status, Status::Failed { .. }));
    // A group of one has no SD; the paragraph gives its n instead of an undefined value.
    let mut one = Dataset::new("one");
    one.add_variable("g", prism_engine::Column::Numeric(vec![Some(1.0), Some(1.0), Some(1.0), Some(2.0)])).unwrap();
    one.add_variable("y", prism_engine::Column::Numeric(vec![Some(1.0), Some(2.0), Some(4.0), Some(3.0)])).unwrap();
    let a1 = &anova_oneway::run(&one, &anova_oneway::Spec { dependent: vec!["y".into()], factor: "g".into(), estimator: Default::default(), post_hoc: Default::default(), level: 0.95, ..Default::default() }).unwrap()[0];
    let text = paragraph(a1).unwrap();
    assert!(text.contains("2 (*M* = 3.00, *n* = 1)") && !text.contains("NaN"), "{text}");
    let small = &ttest_paired::run(
        &ds,
        &ttest_paired::Spec {
            pairs: vec![("g".into(), "y".into())],
            level: 0.95, alternative: Default::default(), bayes: Default::default(), prior_scale: Default::default(), },
    )
    .unwrap()[0];
    assert_eq!(small.diagnostics[0].state, DiagnosticState::ReviewNeeded);
}
