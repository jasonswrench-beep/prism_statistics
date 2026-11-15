//! Power and sample size (`power.*`), for planning a study before the data exist.
//!
//! Defaults (05 §1): α = .05, power .80, two-sided tests. The effect is entered either as a standardized size or in raw
//! units (a mean difference and an SD, group means and a common SD, or proportions), and the output names which; there
//! are no "small, medium, large" shortcuts and no "observed power" for completed tests. Equal group sizes.
//!
//! The computations follow the usual references: the noncentral t distribution for t tests, the noncentral F for
//! one-way ANOVA (Cohen's f, λ = f²N), Fisher's z approximation for Pearson's r, and the arcsine transformation (Cohen's
//! h) with the normal distribution for proportions, as R's `pwr` package does.

use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::apa;
use crate::error::{Error, Result};
use crate::result::{CaseAccounting, Diagnostic, DiagnosticState, ResultBundle, Table};
use prismqda_stats::dist::{beta_inc_pair, norm_cdf, norm_quantile, t_quantile};
use prismqda_stats::noncentral::t_cdf_noncentral;

pub const METHOD_ID: &str = "power";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Test {
    OneSample,
    Paired,
    Independent,
    Anova,
    Correlation,
    OneProportion,
    TwoProportions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Solve {
    /// The sample size needed for the power asked for.
    #[default]
    N,
    /// The power a given sample size has.
    Power,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub test: Test,
    #[serde(default)]
    pub solve_for: Solve,
    /// The standardized effect: d (t tests), f (ANOVA), r (correlation) or h (proportions). Leave out to give it in
    /// raw units instead.
    #[serde(default)]
    pub effect_size: Option<f64>,
    /// t tests in raw units: the mean difference (from the test value, between the measures, or between the groups)
    /// and the SD (of the scores, of the differences, or within each group).
    #[serde(default)]
    pub mean_difference: Option<f64>,
    #[serde(default)]
    pub sd: Option<f64>,
    /// ANOVA in raw units: the expected group means, with `sd` the SD within each group.
    #[serde(default)]
    pub means: Option<Vec<f64>>,
    /// ANOVA: the number of groups (taken from `means` when they are given).
    #[serde(default)]
    pub groups: Option<usize>,
    /// Proportions: the expected proportion, and the one it is compared with (the null value, or the other group).
    #[serde(default)]
    pub p1: Option<f64>,
    #[serde(default)]
    pub p2: Option<f64>,
    #[serde(default = "default_alpha")]
    pub alpha: f64,
    #[serde(default = "default_power")]
    pub power: f64,
    /// For solving power: the sample size (per group for the independent t test, ANOVA and two proportions).
    #[serde(default)]
    pub n: Option<usize>,
}

fn default_alpha() -> f64 {
    0.05
}

fn default_power() -> f64 {
    0.80
}

/// P(F' <= x) for the noncentral F with noncentrality `lambda`: the Poisson mixture of central (beta) distributions.
pub fn f_cdf_noncentral(x: f64, d1: f64, d2: f64, lambda: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let y = d1 * x / (d1 * x + d2);
    let yc = d2 / (d1 * x + d2);
    let half = lambda / 2.0;
    // Sum outward from the Poisson mode so large noncentralities lose no terms.
    let mode = half.floor() as i64;
    let weight = |j: i64| (-half + j as f64 * half.ln() - prismqda_stats::dist::ln_gamma(j as f64 + 1.0)).exp();
    let term = |j: i64| if half == 0.0 { if j == 0 { 1.0 } else { 0.0 } } else { weight(j) } * beta_inc_pair(y, yc, d1 / 2.0 + j as f64, d2 / 2.0);
    let mut total = term(mode);
    let mut j = mode + 1;
    loop {
        let t = term(j);
        total += t;
        if (t < 1e-16 && j > mode + 10) || j > mode + 100_000 {
            break;
        }
        j += 1;
    }
    let mut j = mode - 1;
    while j >= 0 {
        let t = term(j);
        total += t;
        if t < 1e-16 && j < mode - 10 {
            break;
        }
        j -= 1;
    }
    total.min(1.0)
}

/// The F distribution's quantile, by bisection on its CDF.
fn f_quantile(p: f64, d1: f64, d2: f64) -> f64 {
    prismqda_stats::dist::f_quantile(p, d1, d2)
}

/// Everything about one sample size.
struct Point {
    /// Per group where there are groups, otherwise the cases (or pairs).
    n: usize,
    total: usize,
    power: f64,
    df: Option<(f64, f64)>,
}

/// The power of `test` with `n` (per group where there are groups), effect `es` and `k` groups.
fn power_at(test: Test, n: usize, es: f64, alpha: f64, k: usize) -> Option<Point> {
    let nf = n as f64;
    let two_sided_t = |df: f64, ncp: f64| {
        let crit = t_quantile(1.0 - alpha / 2.0, df);
        1.0 - t_cdf_noncentral(crit, df, ncp) + t_cdf_noncentral(-crit, df, ncp)
    };
    let z = norm_quantile(1.0 - alpha / 2.0);
    Some(match test {
        Test::OneSample | Test::Paired => {
            if n < 2 {
                return None;
            }
            let df = nf - 1.0;
            Point { n, total: n, power: two_sided_t(df, es * nf.sqrt()), df: Some((df, f64::NAN)) }
        }
        Test::Independent => {
            if n < 2 {
                return None;
            }
            let df = 2.0 * nf - 2.0;
            Point { n, total: 2 * n, power: two_sided_t(df, es * (nf / 2.0).sqrt()), df: Some((df, f64::NAN)) }
        }
        Test::Anova => {
            if n < 2 {
                return None;
            }
            let (d1, d2) = ((k - 1) as f64, (k * (n - 1)) as f64);
            let crit = f_quantile(1.0 - alpha, d1, d2);
            let lambda = es * es * (k * n) as f64;
            Point { n, total: k * n, power: 1.0 - f_cdf_noncentral(crit, d1, d2, lambda), df: Some((d1, d2)) }
        }
        Test::Correlation => {
            if n < 4 {
                return None;
            }
            // pwr.r.test: the critical r from t, then Fisher's z with the small-sample bias term.
            let df = nf - 2.0;
            let t = t_quantile(1.0 - alpha / 2.0, df);
            let rc = (t * t / (t * t + df)).sqrt();
            let zr = es.atanh() + es / (2.0 * (nf - 1.0));
            let zrc = rc.atanh();
            let s = (nf - 3.0).sqrt();
            Point { n, total: n, power: norm_cdf((zr - zrc) * s) + norm_cdf((-zr - zrc) * s), df: Some((df, f64::NAN)) }
        }
        Test::OneProportion => {
            if n < 1 {
                return None;
            }
            let h = es.abs() * nf.sqrt();
            Point { n, total: n, power: norm_cdf(h - z) + norm_cdf(-h - z), df: None }
        }
        Test::TwoProportions => {
            if n < 1 {
                return None;
            }
            let h = es.abs() * (nf / 2.0).sqrt();
            Point { n, total: 2 * n, power: norm_cdf(h - z) + norm_cdf(-h - z), df: None }
        }
    })
}

/// The smallest n (per group where there are groups) with at least the power asked for.
fn solve_n(test: Test, es: f64, alpha: f64, power: f64, k: usize) -> Option<Point> {
    let start = match test {
        Test::Correlation => 4,
        Test::OneProportion | Test::TwoProportions => 1,
        _ => 2,
    };
    let ok = |n: usize| power_at(test, n, es, alpha, k).is_some_and(|p| p.power >= power);
    let mut hi = start;
    while !ok(hi) {
        hi *= 2;
        if hi > 100_000_000 {
            return None;
        }
    }
    let mut lo = (hi / 2).max(start);
    if ok(lo) {
        return power_at(test, lo, es, alpha, k);
    }
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if ok(mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    power_at(test, hi, es, alpha, k)
}

fn test_name(t: Test) -> &'static str {
    match t {
        Test::OneSample => "one-sample t test",
        Test::Paired => "paired-samples t test",
        Test::Independent => "independent-samples t test",
        Test::Anova => "one-way analysis of variance",
        Test::Correlation => "test of a Pearson correlation",
        Test::OneProportion => "test of one proportion",
        Test::TwoProportions => "comparison of two proportions",
    }
}

/// "a one-way analysis of variance", "an independent-samples t test".
fn with_article(name: &str) -> String {
    let an = name.starts_with(|c: char| "aeiou".contains(c)) && !name.starts_with("one");
    format!("{} {name}", if an { "an" } else { "a" })
}

fn symbol(t: Test) -> &'static str {
    match t {
        Test::OneSample | Test::Paired | Test::Independent => "d",
        Test::Anova => "f",
        Test::Correlation => "r",
        Test::OneProportion | Test::TwoProportions => "h",
    }
}

pub fn run(spec: &Spec) -> Result<ResultBundle> {
    let bad = |m: &str| Err(Error::Invalid(m.into()));
    if !(spec.alpha > 0.0 && spec.alpha < 0.5) {
        return bad("Alpha must be between 0 and .5.");
    }
    if spec.solve_for == Solve::N && !(spec.power > spec.alpha && spec.power < 1.0) {
        return bad("The power asked for must be above alpha and below 1.");
    }
    let mut k = 2;
    // The standardized effect, and how it was given.
    let (es, given) = match (spec.test, spec.effect_size) {
        (Test::Anova, _) => {
            if let Some(m) = &spec.means {
                let sd = spec.sd.ok_or(Error::Invalid("Give the SD within each group along with the group means.".into()))?;
                if m.len() < 2 || sd <= 0.0 {
                    return bad("Give at least two group means and an SD above zero.");
                }
                k = m.len();
                let grand = m.iter().sum::<f64>() / k as f64;
                let sm = (m.iter().map(|v| (v - grand).powi(2)).sum::<f64>() / k as f64).sqrt();
                (sm / sd, format!("from group means of {} and a within-group SD of {} (Cohen's f = SD of the means / SD within)", m.iter().map(|v| apa::plain(*v)).collect::<Vec<_>>().join(", "), apa::plain(sd)))
            } else {
                k = spec.groups.ok_or(Error::Invalid("Give the number of groups.".into()))?;
                if k < 2 {
                    return bad("ANOVA needs at least two groups.");
                }
                let f = spec.effect_size.ok_or(Error::Invalid("Give Cohen's f, or the group means and the SD.".into()))?;
                (f, "as Cohen's f".to_string())
            }
        }
        (Test::OneProportion | Test::TwoProportions, None) => {
            let (Some(a), Some(c)) = (spec.p1, spec.p2) else { return bad("Give both proportions, or Cohen's h.") };
            if !(0.0..=1.0).contains(&a) || !(0.0..=1.0).contains(&c) {
                return bad("Proportions must be between 0 and 1.");
            }
            let h = 2.0 * a.sqrt().asin() - 2.0 * c.sqrt().asin();
            (h, if spec.test == Test::OneProportion { format!("as an expected proportion of {} against {} (Cohen's h from the arcsine transformation)", apa::plain(a), apa::plain(c)) } else { format!("as proportions of {} and {} in the two groups (Cohen's h from the arcsine transformation)", apa::plain(a), apa::plain(c)) })
        }
        (Test::OneSample | Test::Paired | Test::Independent, None) => {
            let (Some(md), Some(sd)) = (spec.mean_difference, spec.sd) else { return bad("Give the mean difference and the SD, or Cohen's d.") };
            if sd <= 0.0 {
                return bad("The SD must be above zero.");
            }
            (md / sd, format!("as a mean difference of {} with an SD of {} (Cohen's d = difference / SD)", apa::plain(md), apa::plain(sd)))
        }
        (Test::Correlation, None) => return bad("Give the expected correlation."),
        (_, Some(e)) => (e, format!("as Cohen's {}", symbol(spec.test))),
    };
    if !es.is_finite() || es == 0.0 {
        return bad("The effect size must be a number other than zero.");
    }
    if spec.test == Test::Correlation && es.abs() >= 1.0 {
        return bad("A correlation must be between −1 and 1.");
    }
    let es_abs = es.abs();
    let grouped = matches!(spec.test, Test::Independent | Test::Anova | Test::TwoProportions);
    let point = match spec.solve_for {
        Solve::N => solve_n(spec.test, es_abs, spec.alpha, spec.power, k).ok_or(Error::Invalid("No sample size up to 100 million reaches that power; the effect is too small.".into()))?,
        Solve::Power => {
            let n = spec.n.ok_or(Error::Invalid(format!("Give the sample size{}.", if grouped { " per group" } else { "" })))?;
            power_at(spec.test, n, es_abs, spec.alpha, k).ok_or(Error::Invalid("That sample size is too small for this test.".into()))?
        }
    };
    let sym = symbol(spec.test);
    let note = format!(
        "Power analysis for {}{} at α = {}, with the effect given {given}. {}Equal group sizes. Computed by PrismStatistics.",
        if spec.test == Test::Anova { "" } else { "a two-sided " },
        if spec.test == Test::Anova { with_article(test_name(spec.test)) } else { test_name(spec.test).to_string() },
        apa::no_leading_zero(spec.alpha, 3).trim_end_matches('0').trim_end_matches('.'),
        match spec.test {
            Test::OneSample | Test::Paired | Test::Independent => "Power from the noncentral t distribution. ",
            Test::Anova => "Power from the noncentral F distribution with λ = f² × N. ",
            Test::Correlation => "Power from Fisher's z approximation with the critical r taken from the t distribution (as R's pwr.r.test). ",
            _ => "Power from the normal approximation on the arcsine scale (as R's pwr). ",
        }
    );
    let title = match spec.solve_for {
        Solve::N => format!("Power Analysis: sample size for {}", with_article(test_name(spec.test))),
        Solve::Power => format!("Power Analysis: power of {}", with_article(test_name(spec.test))),
    };
    let mut b = common::bundle(METHOD_ID, METHOD_VERSION, title, serde_json::to_value(spec).expect("a spec always serializes"), CaseAccounting::default(), note);
    set_text(&mut b, "test", test_name(spec.test));
    set_text(&mut b, "symbol", sym);
    set_text(&mut b, "solve_for", if spec.solve_for == Solve::N { "n" } else { "power" });
    set(&mut b, "effect", es);
    set(&mut b, "alpha", spec.alpha);
    set(&mut b, "target_power", spec.power);
    set(&mut b, "n", point.n as f64);
    set(&mut b, "total", point.total as f64);
    set(&mut b, "power", point.power);
    set(&mut b, "groups", if grouped { k as f64 } else { 1.0 });
    let mut rows = vec![
        vec![text("Test"), text(if spec.test == Test::Anova { test_name(spec.test).to_string() } else { format!("{} (two-sided)", test_name(spec.test)) })],
        vec![text(format!("Effect size ({sym})")), num(es, 3)],
        vec![text("α"), num(spec.alpha, 3)],
    ];
    if spec.test == Test::Anova {
        rows.push(vec![text("Groups"), count(k)]);
    }
    if spec.solve_for == Solve::N {
        rows.push(vec![text("Power asked for"), num(spec.power, 3)]);
    }
    if grouped {
        rows.push(vec![text("N per group"), count(point.n)]);
    }
    rows.push(vec![text(if matches!(spec.test, Test::Paired) { "Pairs" } else { "Total N" }), count(point.total)]);
    rows.push(vec![text(if spec.solve_for == Solve::N { "Power reached" } else { "Power" }), num(point.power, 3)]);
    if let Some((d1, d2)) = point.df {
        rows.push(vec![text("Degrees of freedom"), text(if d2.is_nan() { apa::df(d1) } else { format!("{}, {}", apa::df(d1), apa::df(d2)) })]);
    }
    b.tables.push(Table { id: "power".into(), title: "Power analysis".into(), columns: vec!["".into(), "Value".into()], rows, footnotes: Vec::new() });
    // Power at other sample sizes, for a sense of the curve.
    let mut curve = Vec::new();
    let base = point.n as f64;
    let mut seen = Vec::new();
    for f in [0.5, 0.75, 1.0, 1.25, 1.5, 2.0] {
        let n = ((base * f).round() as usize).max(1);
        if seen.contains(&n) {
            continue;
        }
        seen.push(n);
        if let Some(pt) = power_at(spec.test, n, es_abs, spec.alpha, k) {
            curve.push(vec![count(pt.n), count(pt.total), num(pt.power, 3)]);
        }
    }
    b.tables.push(Table { id: "curve".into(), title: "Power at other sample sizes".into(), columns: vec![if grouped { "N per group".into() } else { "N".into() }, "Total N".into(), "Power".into()], rows: curve, footnotes: Vec::new() });
    b.diagnostics.push(Diagnostic { id: "effect".into(), title: "Effect size".into(), state: DiagnosticState::NotAssessed, message: "The answer is only as good as the effect size it assumes. Base it on earlier studies or the smallest effect that would matter in practice, not on a label like \"medium\" or on the effect a finished study found.".into() });
    if spec.solve_for == Solve::N && point.total > 0 {
        b.diagnostics.push(Diagnostic { id: "attrition".into(), title: "Missing data and dropout".into(), state: DiagnosticState::NotAssessed, message: "This is the number of complete cases needed. Recruit more to allow for dropout and missing answers.".into() });
    }
    Ok(b)
}
