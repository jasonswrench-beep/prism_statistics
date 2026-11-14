//! The classic one- and k-sample nonparametric tests: chi-square goodness of fit (`cat.chisq_gof`), the binomial test
//! (`np.binomial`), the runs test (`np.runs`), the sign test (`np.sign`), the median test (`np.median`) and the
//! two-sample Kolmogorov-Smirnov test (`np.ks2`).
//!
//! Defaults (05 §1): goodness of fit against equal proportions unless others are given, small expected counts flagged
//! (categories never merged); the binomial test exact, with a Clopper-Pearson interval, against .5 unless another
//! proportion is given; the runs test cut at the median, exact p value with the normal approximation beside it; the
//! sign test exact; the median test's chi-square with Fisher's exact p for two groups; Kolmogorov-Smirnov exact for
//! small samples as R's `ks.test`. Computed by `prismqda_stats::discrete` and `prismqda_stats::kolmogorov`.

use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::apa;
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Cell, Diagnostic, DiagnosticState, ResultBundle, Table};

pub const GOF: &str = "cat.chisq_gof";
pub const BINOMIAL: &str = "np.binomial";
pub const RUNS: &str = "np.runs";
pub const SIGN: &str = "np.sign";
pub const MEDIAN: &str = "np.median";
pub const KS2: &str = "np.ks2";
pub const METHOD_VERSION: u32 = 1;

fn label(ds: &Dataset, name: &str) -> Result<String> {
    Ok(ds.variable(name)?.display_name().to_string())
}

fn mk(id: &str, title: String, spec: &impl Serialize, cases: crate::result::CaseAccounting, note: String) -> ResultBundle {
    common::bundle(id, METHOD_VERSION, title, serde_json::to_value(spec).expect("a spec always serializes"), cases, format!("{note} Computed by prismqda-stats {}.", super::STATS_VERSION))
}

// ---------------------------------------------------------------- chi-square goodness of fit

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GofSpec {
    pub variable: String,
    /// Expected proportions (or counts, or any weights: they are rescaled to sum to 1), one per category in the order
    /// the categories are listed (codes in order). None: equal proportions.
    #[serde(default)]
    pub expected: Option<Vec<f64>>,
}

pub fn goodness_of_fit(ds: &Dataset, spec: &GofSpec) -> Result<ResultBundle> {
    let l = label(ds, &spec.variable)?;
    let (slots, levels) = categories(ds, &spec.variable)?;
    let mut tally = Tally::default();
    let mut counts = vec![0usize; levels.len()];
    for s in &slots {
        if !tally.check(s, &spec.variable) {
            counts[s.value().expect("checked")] += 1;
        }
    }
    let n: usize = counts.iter().sum();
    let given = spec.expected.is_some();
    let mut b = mk(GOF, format!("Chi-Square Goodness of Fit: {l}"), spec, tally.accounting(slots.len(), n), format!("Pearson's chi-square test that the categories of {l} occur in {} proportions. Effect size: Cohen's w = √(χ²/N). Categories are never merged; small expected counts are flagged instead.", if given { "the given" } else { "equal" }));
    set_text(&mut b, "var.label", l.clone());
    set(&mut b, "n", n as f64);
    set(&mut b, "k", levels.len() as f64);
    let props = match &spec.expected {
        None => None,
        Some(e) => {
            if e.len() != levels.len() {
                return Err(Error::Invalid(format!("{l} has {} categories with data; give one expected proportion for each ({} given).", levels.len(), e.len())));
            }
            if e.iter().any(|v| !(v.is_finite() && *v > 0.0)) {
                return Err(Error::Invalid("Every expected proportion must be above zero.".into()));
            }
            let s: f64 = e.iter().sum();
            Some(e.iter().map(|v| v / s).collect::<Vec<f64>>())
        }
    };
    let obs: Vec<f64> = counts.iter().map(|c| *c as f64).collect();
    let Some(g) = prismqda_stats::discrete::goodness_of_fit(&obs, props.as_deref()) else {
        fail(&mut b, if levels.len() < 2 { format!("{l} needs at least two categories with data.") } else { format!("{l} has no valid values.") });
        return Ok(b);
    };
    let w = (g.chi2 / g.n).sqrt();
    for (k, v) in [("chi2", g.chi2), ("df", g.df), ("p", g.p), ("w", w), ("min_expected", g.smallest_expected)] {
        set(&mut b, k, v);
    }
    let mut rows = Vec::new();
    for (i, lv) in levels.iter().enumerate() {
        set_text(&mut b, format!("cat.{}.label", i + 1), lv.label.clone());
        set(&mut b, format!("cat.{}.n", i + 1), obs[i]);
        rows.push(vec![text(&lv.label), count(counts[i]), num(g.expected[i], 2), num(100.0 * obs[i] / g.n, 1), num(100.0 * g.expected[i] / g.n, 1), num(g.residuals[i], 2)]);
    }
    rows.push(vec![text("Total"), count(n), num(g.n, 2), num(100.0, 1), num(100.0, 1), Cell::Empty]);
    b.tables.push(Table { id: "counts".into(), title: format!("Observed and expected counts: {l}"), columns: vec![l.clone(), "Observed N".into(), "Expected N".into(), "Observed %".into(), "Expected %".into(), "Residual".into()], rows, footnotes: vec!["Residual: (observed − expected) / √expected.".into()] });
    b.tables.push(Table { id: "test".into(), title: "Chi-square test".into(), columns: vec!["Chi-square".into(), "df".into(), "p".into(), "Cohen's w".into()], rows: vec![vec![num(g.chi2, 3), num(g.df, 0), p(g.p), num(w, 3)]], footnotes: Vec::new() });
    b.diagnostics.push(if g.smallest_expected < 5.0 {
        Diagnostic { id: "expected".into(), title: "Expected counts".into(), state: DiagnosticState::ReviewNeeded, message: format!("The smallest expected count is {}. Below 5, the chi-square p value can be inaccurate; an exact multinomial test or more cases would help.", apa::num(g.smallest_expected, 2)) }
    } else {
        Diagnostic { id: "expected".into(), title: "Expected counts".into(), state: DiagnosticState::NoConcernDetected, message: format!("Every expected count is at least 5 (smallest: {}).", apa::num(g.smallest_expected, 2)) }
    });
    b.diagnostics.push(not_assessed("independence", "Independent observations", "Each case must be counted once, in one category."));
    Ok(b)
}

// ---------------------------------------------------------------- binomial test

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BinomialSpec {
    pub variables: Vec<String>,
    /// The proportion tested against, for the category counted as a success.
    #[serde(default = "half")]
    pub test_proportion: f64,
    /// The category counted as a success, by code or text. None: the first category.
    #[serde(default)]
    pub success: Option<String>,
    #[serde(default = "level95")]
    pub level: f64,
}

fn half() -> f64 {
    0.5
}

fn level95() -> f64 {
    0.95
}

pub fn binomial(ds: &Dataset, spec: &BinomialSpec) -> Result<Vec<ResultBundle>> {
    if spec.variables.is_empty() {
        return Err(Error::Invalid("Choose at least one variable.".into()));
    }
    if !(spec.test_proportion > 0.0 && spec.test_proportion < 1.0) {
        return Err(Error::Invalid("The test proportion must be between 0 and 1.".into()));
    }
    check_level(spec.level)?;
    spec.variables.iter().map(|v| binomial_one(ds, spec, v)).collect()
}

fn binomial_one(ds: &Dataset, spec: &BinomialSpec, name: &str) -> Result<ResultBundle> {
    let l = label(ds, name)?;
    let (slots, levels) = categories(ds, name)?;
    let mut tally = Tally::default();
    let mut counts = vec![0u64; levels.len()];
    for s in &slots {
        if !tally.check(s, name) {
            counts[s.value().expect("checked")] += 1;
        }
    }
    let n: u64 = counts.iter().sum();
    let lv = level_text(spec.level);
    let mut b = mk(BINOMIAL, format!("Binomial Test: {l}"), spec, tally.accounting(slots.len(), n as usize), format!("Exact two-sided binomial test of the proportion in one category against {}, with a {lv} Clopper-Pearson interval.", apa::plain(spec.test_proportion)));
    set_text(&mut b, "var.label", l.clone());
    let present: Vec<usize> = (0..levels.len()).filter(|&i| counts[i] > 0).collect();
    if present.len() != 2 {
        fail(&mut b, format!("The binomial test needs exactly two categories with data; {l} has {}.", present.len()));
        return Ok(b);
    }
    let success = match &spec.success {
        None => present[0],
        Some(key) => present.iter().copied().find(|&i| levels[i].key() == *key || levels[i].code.is_some_and(|c| key.parse::<f64>().ok() == Some(c))).ok_or_else(|| Error::Invalid(format!("The category {key} of {l} has no cases.")))?,
    };
    let other = if present[0] == success { present[1] } else { present[0] };
    let Some(t) = prismqda_stats::discrete::binomial_test(counts[success], n, spec.test_proportion, spec.level) else {
        fail(&mut b, "The test could not be computed.");
        return Ok(b);
    };
    set_text(&mut b, "success.label", levels[success].label.clone());
    set_text(&mut b, "other.label", levels[other].label.clone());
    for (k, v) in [("n", n as f64), ("x", counts[success] as f64), ("p0", spec.test_proportion), ("prop", t.estimate), ("p", t.p), ("ci.lower", t.ci.0), ("ci.upper", t.ci.1), ("level", spec.level)] {
        set(&mut b, k, v);
    }
    b.tables.push(Table {
        id: "test".into(),
        title: format!("Binomial test: {l}"),
        columns: vec!["Category".into(), "N".into(), "Observed proportion".into(), "Test proportion".into(), format!("{lv} CI lower"), format!("{lv} CI upper"), "p (exact, two-sided)".into()],
        rows: vec![
            vec![text(&levels[success].label), count(counts[success] as usize), num(t.estimate, 3), num(spec.test_proportion, 3), num(t.ci.0, 3), num(t.ci.1, 3), p(t.p)],
            vec![text(&levels[other].label), count(counts[other] as usize), num(1.0 - t.estimate, 3), Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty],
            vec![text("Total"), count(n as usize), num(1.0, 3), Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty],
        ],
        footnotes: vec![format!("The test and interval are for the proportion {}.", levels[success].label)],
    });
    b.diagnostics.push(not_assessed("independence", "Independent observations", "Each case must be one independent trial."));
    Ok(b)
}

// ---------------------------------------------------------------- runs test

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Cut {
    #[default]
    Median,
    Mean,
    Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunsSpec {
    pub variables: Vec<String>,
    #[serde(default)]
    pub cut: Cut,
    /// The cut point when `cut` is `value`.
    #[serde(default)]
    pub value: Option<f64>,
}

pub fn runs(ds: &Dataset, spec: &RunsSpec) -> Result<Vec<ResultBundle>> {
    if spec.variables.is_empty() {
        return Err(Error::Invalid("Choose at least one variable.".into()));
    }
    if spec.cut == Cut::Value && spec.value.is_none_or(|v| !v.is_finite()) {
        return Err(Error::Invalid("Give the cut point.".into()));
    }
    spec.variables.iter().map(|v| runs_one(ds, spec, v)).collect()
}

fn runs_one(ds: &Dataset, spec: &RunsSpec, name: &str) -> Result<ResultBundle> {
    let l = label(ds, name)?;
    let cells = numeric(ds, name)?;
    let mut tally = Tally::default();
    // Cases stay in file order: the order is what the test is about.
    let v: Vec<f64> = cells.iter().filter(|c| !tally.check(*c, name)).map(|c| c.value().expect("checked")).collect();
    let cut = match spec.cut {
        Cut::Median => median(&v).unwrap_or(f64::NAN),
        Cut::Mean => prismqda_stats::describe::mean(&v).unwrap_or(f64::NAN),
        Cut::Value => spec.value.unwrap_or(f64::NAN),
    };
    let how = match spec.cut {
        Cut::Median => "the median",
        Cut::Mean => "the mean",
        Cut::Value => "the value given",
    };
    let mut b = mk(RUNS, format!("Runs Test: {l}"), spec, tally.accounting(cells.len(), v.len()), format!("Wald-Wolfowitz runs test of whether the values of {l}, in the order of the cases, fall below and at-or-above {how} ({}) in a random order. Two-sided exact p value, with the normal approximation (continuity-corrected) beside it.", apa::plain(cut)));
    set_text(&mut b, "var.label", l.clone());
    set(&mut b, "cut", cut);
    let seq: Vec<bool> = v.iter().map(|x| *x >= cut).collect();
    let Some(r) = prismqda_stats::discrete::runs_test(&seq) else {
        fail(&mut b, format!("Every value of {l} is on the same side of the cut point, so there is nothing to test."));
        return Ok(b);
    };
    for (k, val) in [("runs", r.runs as f64), ("n_below", r.n0 as f64), ("n_above", r.n1 as f64), ("z", r.z), ("p", r.p_exact), ("p.normal", r.p_normal), ("n", v.len() as f64)] {
        set(&mut b, k, val);
    }
    b.tables.push(Table {
        id: "test".into(),
        title: format!("Runs test: {l}"),
        columns: vec!["Cut point".into(), "Cases below".into(), "Cases at or above".into(), "Runs".into(), "Expected runs".into(), "z".into(), "p (exact)".into(), "p (normal)".into()],
        rows: vec![vec![num(cut, 3), count(r.n0 as usize), count(r.n1 as usize), count(r.runs as usize), num(r.mean, 2), num(r.z, 3), p(r.p_exact), p(r.p_normal)]],
        footnotes: vec!["Fewer runs than expected suggest clustering; more suggest alternation.".into()],
    });
    b.diagnostics.push(not_assessed("order", "Meaningful order", "The cases must be in the order the test is about (time of collection, position)."));
    Ok(b)
}

// ---------------------------------------------------------------- sign test

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignSpec {
    /// Each pair is (first, second); differences are first minus second.
    pub pairs: Vec<(String, String)>,
}

pub fn sign(ds: &Dataset, spec: &SignSpec) -> Result<Vec<ResultBundle>> {
    if spec.pairs.is_empty() {
        return Err(Error::Invalid("Choose at least one pair of variables.".into()));
    }
    spec.pairs
        .iter()
        .map(|(a, c)| {
            let (l1, l2) = (label(ds, a)?, label(ds, c)?);
            let (x, y, tally, total) = super::ttest_paired::matched(ds, a, c)?;
            let mut b = mk(SIGN, format!("Sign Test: {l1} and {l2}"), spec, tally.accounting(total, x.len()), format!("Exact sign test of the differences ({l1} minus {l2}): a binomial test of how many are positive among those that are not zero."));
            set_text(&mut b, "x1.label", l1.clone());
            set_text(&mut b, "x2.label", l2.clone());
            let d: Vec<f64> = x.iter().zip(&y).map(|(u, v)| u - v).collect();
            let Some(t) = prismqda_stats::discrete::sign_test(&d, 0.95) else {
                fail(&mut b, "Every difference is zero, so there is nothing to test.");
                return Ok(b);
            };
            for (k, v) in [("n", x.len() as f64), ("pos", t.positive as f64), ("neg", t.negative as f64), ("zeros", t.zeros as f64), ("median_diff", t.median), ("p", t.p)] {
                set(&mut b, k, v);
            }
            b.tables.push(Table {
                id: "test".into(),
                title: "Sign test".into(),
                columns: vec!["Pair".into(), "Positive differences".into(), "Negative differences".into(), "Ties".into(), "Median difference".into(), "p (exact, two-sided)".into()],
                rows: vec![vec![text(format!("{l1} − {l2}")), count(t.positive as usize), count(t.negative as usize), count(t.zeros as usize), num(t.median, 2), p(t.p)]],
                footnotes: Vec::new(),
            });
            Ok(b)
        })
        .collect()
}

// ---------------------------------------------------------------- median test and two-sample Kolmogorov-Smirnov

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupsSpec {
    pub dependent: Vec<String>,
    pub factor: String,
}

pub fn median_test(ds: &Dataset, spec: &GroupsSpec) -> Result<Vec<ResultBundle>> {
    if spec.dependent.is_empty() {
        return Err(Error::Invalid("Choose at least one test variable.".into()));
    }
    spec.dependent
        .iter()
        .map(|y| {
            let (yl, fl) = (label(ds, y)?, label(ds, &spec.factor)?);
            let s = super::anova_oneway::split(ds, y, &spec.factor)?;
            let n: usize = s.groups.iter().map(Vec::len).sum();
            let mut b = mk(MEDIAN, format!("Median Test: {yl} by {fl}"), spec, s.tally.accounting(s.total, n), format!("Mood's median test: the cases above the pooled median and at or below it in each group of {fl}, compared by Pearson's chi-square{}.", if s.groups.len() == 2 { ", with Fisher's exact p value for two groups" } else { "" }));
            set_text(&mut b, "y.label", yl.clone());
            set_text(&mut b, "factor.label", fl.clone());
            let Some(m) = prismqda_stats::discrete::median_test(&s.groups) else {
                fail(&mut b, "The test needs at least two groups with data and values on both sides of the median.");
                return Ok(b);
            };
            for (k, v) in [("median", m.median), ("chi2", m.chi2), ("df", m.df), ("p", m.p), ("n", n as f64), ("k", s.groups.len() as f64)] {
                set(&mut b, k, v);
            }
            if let Some(fp) = m.fisher_p {
                set(&mut b, "fisher.p", fp);
            }
            b.tables.push(Table {
                id: "counts".into(),
                title: format!("Cases above and at or below the median ({})", apa::num(m.median, 2)),
                columns: vec![fl.clone(), "Above".into(), "At or below".into()],
                rows: s.levels.iter().enumerate().map(|(i, l)| vec![text(&l.label), count(m.above[i] as usize), count(m.at_or_below[i] as usize)]).collect(),
                footnotes: Vec::new(),
            });
            let mut rows = vec![vec![text("Pearson chi-square"), num(m.chi2, 3), num(m.df, 0), p(m.p)]];
            if let Some(fp) = m.fisher_p {
                rows.push(vec![text("Fisher's exact test"), Cell::Empty, Cell::Empty, p(fp)]);
            }
            b.tables.push(Table { id: "test".into(), title: "Median test".into(), columns: vec!["Test".into(), "Value".into(), "df".into(), "p".into()], rows, footnotes: Vec::new() });
            b.diagnostics.push(if m.smallest_expected < 5.0 {
                Diagnostic { id: "expected".into(), title: "Expected counts".into(), state: DiagnosticState::ReviewNeeded, message: format!("The smallest expected count is {}, so the chi-square p value may be inaccurate{}.", apa::num(m.smallest_expected, 2), if m.fisher_p.is_some() { "; the exact test is the safer guide" } else { "" }) }
            } else {
                Diagnostic { id: "expected".into(), title: "Expected counts".into(), state: DiagnosticState::NoConcernDetected, message: "Every expected count is at least 5.".into() }
            });
            Ok(b)
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ks2Spec {
    pub test_variables: Vec<String>,
    pub grouping_variable: String,
    pub groups: (f64, f64),
}

pub fn ks2(ds: &Dataset, spec: &Ks2Spec) -> Result<Vec<ResultBundle>> {
    if spec.test_variables.is_empty() {
        return Err(Error::Invalid("Choose at least one test variable.".into()));
    }
    spec.test_variables
        .iter()
        .map(|y| {
            let (yl, gl) = (label(ds, y)?, label(ds, &spec.grouping_variable)?);
            let gs = numeric(ds, &spec.grouping_variable)?;
            let ys = numeric(ds, y)?;
            let g_var = ds.variable(&spec.grouping_variable)?;
            let (c1, c2) = spec.groups;
            let mut tally = Tally::default();
            let (mut a, mut c) = (Vec::new(), Vec::new());
            for (g, v) in gs.iter().zip(&ys) {
                if tally.check(g, "grouping variable") {
                    continue;
                }
                let gv = g.value().expect("checked");
                if gv != c1 && gv != c2 {
                    tally.add("in neither compared group");
                    continue;
                }
                if tally.check(v, y) {
                    continue;
                }
                if gv == c1 { &mut a } else { &mut c }.push(v.value().expect("checked"));
            }
            let (n1, n2) = (g_var.value_text(c1), g_var.value_text(c2));
            let mut b = mk(KS2, format!("Two-Sample Kolmogorov-Smirnov Test: {yl} by {gl}"), spec, tally.accounting(ys.len(), a.len() + c.len()), format!("Two-sample Kolmogorov-Smirnov test of whether {yl} has the same distribution in {n1} and {n2}: the largest distance between the two cumulative distributions. The p value is exact when the product of the group sizes is under 10,000 (as R's ks.test), otherwise from the limiting distribution."));
            set_text(&mut b, "y.label", yl.clone());
            set_text(&mut b, "g1.label", n1.clone());
            set_text(&mut b, "g2.label", n2.clone());
            let Some(k) = prismqda_stats::kolmogorov::ks_two_sample(&a, &c) else {
                fail(&mut b, "Each group needs at least one value.");
                return Ok(b);
            };
            for (key, v) in [("D", k.d), ("p", k.p), ("n1", a.len() as f64), ("n2", c.len() as f64)] {
                set(&mut b, key, v);
            }
            b.tables.push(Table { id: "test".into(), title: "Kolmogorov-Smirnov test".into(), columns: vec!["Groups".into(), "N".into(), "D".into(), format!("p ({})", if k.exact { "exact" } else { "asymptotic" })], rows: vec![vec![text(format!("{n1} vs {n2}")), count(a.len() + c.len()), num(k.d, 3), p(k.p)]], footnotes: Vec::new() });
            b.diagnostics.push(size_diagnostic(a.len().min(c.len()), format!("The smaller group has {} cases", a.len().min(c.len()))));
            Ok(b)
        })
        .collect()
}
