//! Tests of related samples beyond the signed-rank test: McNemar's test of two yes-or-no measures (`np.mcnemar`) and
//! Friedman's test of several measures on the same cases (`np.friedman`).
//!
//! Defaults (05 §1): McNemar is reported with the continuity correction (R's default) and the exact binomial p value
//! beside it; with fewer than 25 changed cases the paragraph uses the exact p value, which is then the better guide.
//! Friedman's test is corrected for ties, with Kendall's W as its effect size; cases need a value on every measure
//! (listwise). Computed by `prismqda_stats::discrete::mcnemar` and `prismqda_stats::nonparametric::friedman`.

use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::apa;
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Cell, Diagnostic, DiagnosticState, ResultBundle, Table};

pub const MCNEMAR: &str = "np.mcnemar";
pub const FRIEDMAN: &str = "np.friedman";
pub const COCHRAN: &str = "np.cochran";
pub const METHOD_VERSION: u32 = 1;

/// Below this many cases that changed, the exact p value is reported.
pub const EXACT_BELOW: f64 = 25.0;

// ---------------------------------------------------------------- McNemar

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McNemarSpec {
    /// Each pair is (first, second): the same yes-or-no question at two times, or two raters of the same cases.
    pub pairs: Vec<(String, String)>,
}

pub fn mcnemar(ds: &Dataset, spec: &McNemarSpec) -> Result<Vec<ResultBundle>> {
    if spec.pairs.is_empty() {
        return Err(Error::Invalid("Choose at least one pair of variables.".into()));
    }
    spec.pairs.iter().map(|(a, c)| mcnemar_one(ds, spec, a, c)).collect()
}

fn mcnemar_one(ds: &Dataset, spec: &McNemarSpec, first: &str, second: &str) -> Result<ResultBundle> {
    let v1 = ds.variable(first)?;
    let (l1, l2) = (v1.display_name().to_string(), ds.variable(second)?.display_name().to_string());
    let (x, y, tally, total) = super::ttest_paired::matched(ds, first, second)?;
    let mut codes: Vec<f64> = x.iter().chain(&y).copied().collect();
    codes.sort_by(f64::total_cmp);
    codes.dedup();
    let note = format!(
        "McNemar's test of whether the share of cases in each category changed from {l1} to {l2}, using only the cases that changed. Chi-square with the continuity correction, and the exact two-sided p value (a binomial test of the changes at one half); with fewer than {EXACT_BELOW} changed cases the exact p value is the one to report. Cases were used when both variables had valid values. Computed by prismqda-stats {}.",
        super::STATS_VERSION
    );
    let mut b = common::bundle(MCNEMAR, METHOD_VERSION, format!("McNemar Test: {l1} and {l2}"), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, x.len()), note);
    set_text(&mut b, "x1.label", l1.clone());
    set_text(&mut b, "x2.label", l2.clone());
    set(&mut b, "n", x.len() as f64);
    if codes.len() != 2 {
        fail(&mut b, format!("McNemar's test needs the same two categories in both variables; {l1} and {l2} have {} different values between them{}.", codes.len(), if codes.len() > 2 { "" } else { " among the cases used" }));
        return Ok(b);
    }
    let labels: Vec<String> = codes.iter().map(|&c| v1.value_text(c)).collect();
    let cell = |i: usize, j: usize| x.iter().zip(&y).filter(|(a, c)| **a == codes[i] && **c == codes[j]).count();
    let counts = [[cell(0, 0), cell(0, 1)], [cell(1, 0), cell(1, 1)]];
    let n = x.len();
    let mut columns = vec![l1.clone(), String::new()];
    columns.extend(labels.iter().map(|l| format!("{l2}: {l}")));
    columns.push("Total".into());
    let mut rows = Vec::new();
    for i in 0..2 {
        rows.push(vec![text(&labels[i]), text("Count"), count(counts[i][0]), count(counts[i][1]), count(counts[i][0] + counts[i][1])]);
    }
    rows.push(vec![text("Total"), text("Count"), count(counts[0][0] + counts[1][0]), count(counts[0][1] + counts[1][1]), count(n)]);
    b.tables.push(Table { id: "table".into(), title: format!("{l1} × {l2}"), columns, rows, footnotes: vec![format!("The cases that changed are the {} counted as {} then {} and the {} counted as {} then {}.", counts[0][1], labels[0], labels[1], counts[1][0], labels[1], labels[0])] });
    let (bc, cb) = (counts[0][1] as f64, counts[1][0] as f64);
    set_text(&mut b, "cat.1.label", labels[0].clone());
    set_text(&mut b, "cat.2.label", labels[1].clone());
    set(&mut b, "b", bc);
    set(&mut b, "c", cb);
    let share = |k: usize| if n == 0 { f64::NAN } else { k as f64 / n as f64 };
    set(&mut b, "x1.share2", share(counts[1][0] + counts[1][1]));
    set(&mut b, "x2.share2", share(counts[0][1] + counts[1][1]));
    let Some(m) = prismqda_stats::discrete::mcnemar(bc, cb) else {
        fail(&mut b, "No case changed category, so there is nothing to test.");
        return Ok(b);
    };
    for (k, v) in [("chi2", m.chi2_corrected), ("p", m.p_corrected), ("chi2.uncorrected", m.chi2), ("p.uncorrected", m.p), ("p.exact", m.p_exact)] {
        set(&mut b, k, v);
    }
    b.tables.push(Table {
        id: "test".into(),
        title: "McNemar test".into(),
        columns: vec!["Test".into(), "Value".into(), "df".into(), "p (two-sided)".into()],
        rows: vec![
            vec![text("Chi-square, continuity-corrected"), num(m.chi2_corrected, 3), num(1.0, 0), p(m.p_corrected)],
            vec![text("Chi-square, uncorrected"), num(m.chi2, 3), num(1.0, 0), p(m.p)],
            vec![text("Exact (binomial)"), Cell::Empty, Cell::Empty, p(m.p_exact)],
            vec![text("N of valid cases"), count(n), Cell::Empty, Cell::Empty],
        ],
        footnotes: vec![format!("{} cases changed category.", apa::num(bc + cb, 0))],
    });
    b.diagnostics.push(if bc + cb < EXACT_BELOW {
        Diagnostic { id: "changes".into(), title: "Cases that changed".into(), state: DiagnosticState::ReviewNeeded, message: format!("Only {} cases changed category, so the chi-square approximation is rough; the exact p value is the one to report (the paragraph uses it).", apa::num(bc + cb, 0)) }
    } else {
        Diagnostic { id: "changes".into(), title: "Cases that changed".into(), state: DiagnosticState::NoConcernDetected, message: format!("{} cases changed category.", apa::num(bc + cb, 0)) }
    });
    b.diagnostics.push(not_assessed("pairing", "Pairing", "Each case must contribute one answer to each variable (the same people asked twice, or two raters judging the same cases)."));
    Ok(b)
}

// ---------------------------------------------------------------- Friedman

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FriedmanSpec {
    /// The measures, one variable each, at least two (usually three or more).
    pub variables: Vec<String>,
}

pub fn friedman(ds: &Dataset, spec: &FriedmanSpec) -> Result<ResultBundle> {
    if spec.variables.len() < 2 {
        return Err(Error::Invalid("Choose at least two variables (measures of the same cases).".into()));
    }
    for (i, a) in spec.variables.iter().enumerate() {
        if spec.variables[..i].iter().any(|b| ds.find(a) == ds.find(b)) {
            return Err(Error::Invalid(format!("{a} is listed twice.")));
        }
    }
    let labels: Vec<String> = spec.variables.iter().map(|n| ds.variable(n).map(|v| v.display_name().to_string())).collect::<Result<_>>()?;
    let cells: Vec<Vec<Slot<f64>>> = spec.variables.iter().map(|n| numeric(ds, n)).collect::<Result<_>>()?;
    let total = cells[0].len();
    let mut tally = Tally::default();
    let used: Vec<usize> = (0..total).filter(|&r| !cells.iter().zip(&spec.variables).any(|(c, n)| tally.check(&c[r], n))).collect();
    let columns: Vec<Vec<f64>> = cells.iter().map(|c| used.iter().map(|&r| c[r].value().expect("checked")).collect()).collect();
    let k = labels.len();
    let note = format!(
        "Friedman's rank test of whether {} measures of the same cases differ: each case's values are ranked among themselves (ties get average ranks) and the rank sums compared, with the correction for ties. Effect size: Kendall's W. Cases were used when every measure had a valid value (listwise). Computed by prismqda-stats {}.",
        k,
        super::STATS_VERSION
    );
    let mut b = common::bundle(FRIEDMAN, METHOD_VERSION, format!("Friedman Test: {}", labels.join(", ")), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, used.len()), note);
    set(&mut b, "k", k as f64);
    set(&mut b, "n", used.len() as f64);
    let fr = prismqda_stats::nonparametric::friedman(&columns);
    for (i, (l, c)) in labels.iter().zip(&columns).enumerate() {
        set_text(&mut b, format!("group.{}.label", i + 1), l.clone());
        if let Some(m) = median(c) {
            set(&mut b, format!("group.{}.median", i + 1), m);
        }
    }
    b.tables.push(Table {
        id: "ranks".into(),
        title: "Ranks".into(),
        columns: vec!["Measure".into(), "N".into(), "Median".into(), "Mean rank".into()],
        rows: labels.iter().zip(&columns).enumerate().map(|(i, (l, c))| vec![text(l), count(c.len()), opt(median(c), 2), opt(fr.as_ref().map(|f| f.mean_ranks[i]), 2)]).collect(),
        footnotes: Vec::new(),
    });
    let Some(fr) = fr else {
        fail(&mut b, if used.len() < 2 { format!("The test needs at least two cases with every measure; there {} {}.", if used.len() == 1 { "is" } else { "are" }, used.len()) } else { "Every case has the same value on every measure, so there is nothing to rank.".to_string() });
        return Ok(b);
    };
    for (key, v) in [("chi2", fr.chi2), ("df", fr.df), ("p", fr.p), ("W", fr.kendall_w)] {
        set(&mut b, key, v);
    }
    for (i, r) in fr.mean_ranks.iter().enumerate() {
        set(&mut b, format!("group.{}.mean_rank", i + 1), *r);
    }
    b.tables.push(Table {
        id: "test".into(),
        title: "Friedman test".into(),
        columns: vec!["N".into(), "Chi-square".into(), "df".into(), "p".into(), "Kendall's W".into()],
        rows: vec![vec![count(fr.n), num(fr.chi2, 3), num(fr.df, 0), p(fr.p), num(fr.kendall_w, 3)]],
        footnotes: vec!["The statistic is compared with the chi-square distribution.".into()],
    });
    b.diagnostics.push(size_diagnostic(fr.n, format!("{} case{} had every measure", fr.n, if fr.n == 1 { "" } else { "s" })));
    b.diagnostics.push(not_assessed("pairing", "Related measures", "The test assumes each row is one case measured under every condition, and that the cases are independent of each other."));
    Ok(b)
}

// ---------------------------------------------------------------- Cochran's Q

/// Cochran's Q: whether the share of "yes" differs across several yes-or-no measures of the same cases. Every
/// variable must use the same two codes; the higher code counts as "yes" unless `success` names one.
pub fn cochran(ds: &Dataset, spec: &FriedmanSpec) -> Result<ResultBundle> {
    if spec.variables.len() < 2 {
        return Err(Error::Invalid("Choose at least two variables (yes-or-no measures of the same cases).".into()));
    }
    let labels: Vec<String> = spec.variables.iter().map(|n| ds.variable(n).map(|v| v.display_name().to_string())).collect::<Result<_>>()?;
    let cells: Vec<Vec<Slot<f64>>> = spec.variables.iter().map(|n| numeric(ds, n)).collect::<Result<_>>()?;
    let total = cells[0].len();
    let mut tally = Tally::default();
    let used: Vec<usize> = (0..total).filter(|&r| !cells.iter().zip(&spec.variables).any(|(c, n)| tally.check(&c[r], n))).collect();
    let mut codes: Vec<f64> = used.iter().flat_map(|&r| cells.iter().map(move |c| c[r].value().expect("checked"))).collect();
    codes.sort_by(f64::total_cmp);
    codes.dedup();
    let k = labels.len();
    let note = format!("Cochran's Q test of whether the share of cases coded {} differs across {k} related yes-or-no measures. Cases were used when every measure had a valid value (listwise). Computed by PrismStatistics.", codes.last().map_or(String::new(), |c| apa::plain(*c)));
    let mut b = common::bundle(COCHRAN, METHOD_VERSION, format!("Cochran's Q: {}", labels.join(", ")), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, used.len()), note);
    set(&mut b, "k", k as f64);
    set(&mut b, "n", used.len() as f64);
    if codes.len() != 2 {
        fail(&mut b, format!("Cochran's Q needs the same two codes in every variable; these have {} different values.", codes.len()));
        return Ok(b);
    }
    let yes = codes[1];
    let v1 = ds.variable(&spec.variables[0])?;
    set_text(&mut b, "yes.label", v1.value_text(yes));
    let x: Vec<Vec<f64>> = used.iter().map(|&r| cells.iter().map(|c| f64::from(u8::from(c[r].value() == Some(yes)))).collect()).collect();
    let cols: Vec<f64> = (0..k).map(|j| x.iter().map(|r| r[j]).sum()).collect();
    let rows: Vec<f64> = x.iter().map(|r| r.iter().sum()).collect();
    let nn: f64 = rows.iter().sum();
    let kf = k as f64;
    let den = kf * nn - rows.iter().map(|r| r * r).sum::<f64>();
    for (i, l) in labels.iter().enumerate() {
        set_text(&mut b, format!("group.{}.label", i + 1), l.clone());
        set(&mut b, format!("group.{}.share", i + 1), cols[i] / used.len() as f64);
    }
    b.tables.push(Table { id: "shares".into(), title: format!("Cases coded {}", v1.value_text(yes)), columns: vec!["Measure".into(), "N".into(), "Count".into(), "Percent".into()], rows: labels.iter().zip(&cols).map(|(l, c)| vec![text(l), count(used.len()), count(*c as usize), num(100.0 * c / used.len() as f64, 1)]).collect(), footnotes: Vec::new() });
    if den <= 0.0 {
        fail(&mut b, "Every case gave the same answer on every measure, so there is nothing to test.");
        return Ok(b);
    }
    let q = (kf - 1.0) * (kf * cols.iter().map(|c| c * c).sum::<f64>() - nn * nn) / den;
    let df = kf - 1.0;
    let pv = prismqda_stats::dist::chisq_sf(q, df);
    for (key, v) in [("Q", q), ("df", df), ("p", pv)] {
        set(&mut b, key, v);
    }
    b.tables.push(Table { id: "test".into(), title: "Cochran's Q test".into(), columns: vec!["N".into(), "Cochran's Q".into(), "df".into(), "p".into()], rows: vec![vec![count(used.len()), num(q, 3), num(df, 0), p(pv)]], footnotes: vec!["Q is compared with the chi-square distribution.".into()] });
    b.diagnostics.push(size_diagnostic(used.len(), format!("{} cases had every measure", used.len())));
    Ok(b)
}
