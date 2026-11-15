//! Crosstabs (`desc.crosstabs`) with the chi-square test of independence (`cat.chisq_indep`), tasks T3-07 and T3-11.
//!
//! Defaults (05 §1, §3 row 17): cell percentages by row; chi-square on, without the continuity correction; effect size
//! Cramér's V (with phi for a two-by-two table). Categories are never merged automatically; small expected counts are
//! flagged instead. Numeric variables are grouped by code, text variables by their text. Computed by
//! `prismqda_stats::contingency`.

use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::apa;
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Cell, Diagnostic, DiagnosticState, ResultBundle, Table};

pub const METHOD_ID: &str = "desc.crosstabs";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Percent {
    Row,
    Column,
    Total,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub row: String,
    pub column: String,
    #[serde(default = "default_percentages")]
    pub percentages: Vec<Percent>,
    #[serde(default = "yes")]
    pub chi_square: bool,
    /// Yates' continuity correction, two-by-two tables only. Off by default (05 §3 row 17).
    #[serde(default)]
    pub continuity: bool,
    #[serde(default)]
    pub expected: bool,
    #[serde(default)]
    pub residuals: bool,
    /// Fisher's exact test, two-by-two tables only, with the conditional odds ratio. Off by default.
    #[serde(default)]
    pub fisher: bool,
    /// Relative risk and odds ratio with intervals (two-by-two tables).
    #[serde(default)]
    pub risk: bool,
    /// Gamma, Kendall's tau-b, Stuart's tau-c and Somers' d, for categories in a meaningful order.
    #[serde(default)]
    pub ordinal: bool,
    /// A layer variable: the Cochran-Mantel-Haenszel test and the Breslow-Day test across its categories
    /// (two-by-two tables in each layer).
    #[serde(default)]
    pub layer: Option<String>,
    /// A Bayes factor for association (Gunel and Dickey, prior concentration 1).
    #[serde(default)]
    pub bayes: bool,
    #[serde(default)]
    pub sampling: super::bayes::Sampling,
}

fn default_percentages() -> Vec<Percent> {
    vec![Percent::Row]
}

fn yes() -> bool {
    true
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    if ds.find(&spec.row) == ds.find(&spec.column) {
        return Err(Error::Invalid(
            "Choose two different variables for the rows and the columns.".into(),
        ));
    }
    let (rv, cv) = (ds.variable(&spec.row)?, ds.variable(&spec.column)?);
    let (rl, cl) = (rv.display_name().to_string(), cv.display_name().to_string());
    let (rs, rlev) = categories(ds, &spec.row)?;
    let (cs, clev) = categories(ds, &spec.column)?;
    let layer = match &spec.layer {
        Some(l) => {
            if ds.find(l) == ds.find(&spec.row) || ds.find(l) == ds.find(&spec.column) {
                return Err(Error::Invalid("The layer variable must differ from the row and column variables.".into()));
            }
            Some((ds.variable(l)?.display_name().to_string(), categories(ds, l)?))
        }
        None => None,
    };
    let mut tally = Tally::default();
    let mut counts = vec![vec![0usize; clev.len()]; rlev.len()];
    let mut used_cases: Vec<usize> = Vec::new();
    for (i, (r, c)) in rs.iter().zip(&cs).enumerate() {
        if tally.check(r, "row variable") || tally.check(c, "column variable") {
            continue;
        }
        if let Some((_, (ls, _))) = &layer {
            if tally.check(&ls[i], "layer variable") {
                continue;
            }
        }
        counts[r.value().expect("checked")][c.value().expect("checked")] += 1;
        used_cases.push(i);
    }
    // Categories with no cases once the other variable's missing values are left out are dropped (no empty rows).
    let keep_r: Vec<usize> = (0..rlev.len())
        .filter(|&i| counts[i].iter().sum::<usize>() > 0)
        .collect();
    let keep_c: Vec<usize> = (0..clev.len())
        .filter(|&j| counts.iter().map(|r| r[j]).sum::<usize>() > 0)
        .collect();
    let rlev: Vec<Level> = keep_r.iter().map(|&i| rlev[i].clone()).collect();
    let clev: Vec<Level> = keep_c.iter().map(|&j| clev[j].clone()).collect();
    let counts: Vec<Vec<usize>> = keep_r
        .iter()
        .map(|&i| keep_c.iter().map(|&j| counts[i][j]).collect())
        .collect();
    let n: usize = counts.iter().flatten().sum();
    let row_tot: Vec<usize> = counts.iter().map(|r| r.iter().sum()).collect();
    let col_tot: Vec<usize> = (0..clev.len())
        .map(|j| counts.iter().map(|r| r[j]).sum())
        .collect();

    let note = format!(
        "Counts of cases by {rl} (rows) and {cl} (columns). {}Categories are never merged. Cases were used when both variables had valid values. Computed by PrismStatistics.",
        if spec.chi_square {
            format!(
                "Pearson's chi-square test of independence{}; effect size Cramér's V{}. ",
                if spec.continuity { " with Yates' continuity correction (two-by-two tables)" } else { ", without a continuity correction" },
                if rlev.len() == 2 && clev.len() == 2 { " and phi" } else { "" }
            )
        } else {
            String::new()
        }
    );
    let mut b = common::bundle(
        METHOD_ID,
        METHOD_VERSION,
        format!("Crosstabs: {rl} by {cl}"),
        serde_json::to_value(spec).expect("a spec always serializes"),
        tally.accounting(rs.len(), n),
        note,
    );
    set_text(&mut b, "row.label", rl.clone());
    set_text(&mut b, "col.label", cl.clone());
    set(&mut b, "n", n as f64);
    for (i, l) in rlev.iter().enumerate() {
        for (j, m) in clev.iter().enumerate() {
            set(
                &mut b,
                format!("count.{}.{}", l.key(), m.key()),
                counts[i][j] as f64,
            );
        }
    }

    // The table itself: for each row category, the counts and then the chosen percentages, as menu-driven packages lay it out.
    let pct = |a: usize, base: usize| {
        if base == 0 {
            Cell::Empty
        } else {
            num(100.0 * a as f64 / base as f64, 1)
        }
    };
    let mut columns = vec![rl.clone(), String::new()];
    columns.extend(clev.iter().map(|l| l.label.clone()));
    columns.push("Total".into());
    let mut rows = Vec::new();
    let chi = if spec.chi_square && rlev.len() >= 2 && clev.len() >= 2 {
        prismqda_stats::contingency::chi_square(
            &counts
                .iter()
                .map(|r| r.iter().map(|&c| c as f64).collect())
                .collect::<Vec<Vec<f64>>>(),
        )
    } else {
        None
    };
    for (i, l) in rlev.iter().enumerate() {
        let mut row = vec![text(&l.label), text("Count")];
        row.extend(counts[i].iter().map(|&c| count(c)));
        row.push(count(row_tot[i]));
        rows.push(row);
        if spec.expected {
            if let Some(c) = &chi {
                let mut row = vec![Cell::Empty, text("Expected count")];
                row.extend(c.expected[i].iter().map(|&e| num(e, 1)));
                row.push(Cell::Empty);
                rows.push(row);
            }
        }
        for which in &spec.percentages {
            let mut row = vec![Cell::Empty];
            match which {
                Percent::Row => {
                    row.push(text(format!("% within {rl}")));
                    row.extend(counts[i].iter().map(|&c| pct(c, row_tot[i])));
                    row.push(pct(row_tot[i], row_tot[i]));
                }
                Percent::Column => {
                    row.push(text(format!("% within {cl}")));
                    row.extend(counts[i].iter().zip(&col_tot).map(|(&c, &t)| pct(c, t)));
                    row.push(pct(row_tot[i], n));
                }
                Percent::Total => {
                    row.push(text("% of total"));
                    row.extend(counts[i].iter().map(|&c| pct(c, n)));
                    row.push(pct(row_tot[i], n));
                }
            }
            rows.push(row);
        }
        if spec.residuals {
            if let Some(c) = &chi {
                let mut row = vec![Cell::Empty, text("Adjusted residual")];
                row.extend(c.adjusted_residuals[i].iter().map(|&e| num(e, 2)));
                row.push(Cell::Empty);
                rows.push(row);
            }
        }
    }
    let mut total = vec![text("Total"), text("Count")];
    total.extend(col_tot.iter().map(|&c| count(c)));
    total.push(count(n));
    rows.push(total);
    if spec.percentages.iter().any(|p| *p != Percent::Row) {
        let mut row = vec![Cell::Empty, text("% of total")];
        row.extend(col_tot.iter().map(|&c| pct(c, n)));
        row.push(pct(n, n));
        rows.push(row);
    }
    b.tables.push(Table { id: "crosstab".into(), title: format!("{rl} × {cl}"), columns, rows, footnotes: if spec.residuals { vec!["Adjusted residuals are about standard normal when the variables are independent; beyond ±1.96 is unusual.".into()] } else { Vec::new() } });
    if spec.bayes && counts.len() >= 2 && counts[0].len() >= 2 {
        let y: Vec<Vec<f64>> = counts.iter().map(|r| r.iter().map(|&c| c as f64).collect()).collect();
        let lbf = super::bayes::contingency_log_bf(&y, spec.sampling, 1.0);
        let design = match spec.sampling {
            super::bayes::Sampling::JointMultinomial => "joint multinomial sampling (only the total fixed)",
            super::bayes::Sampling::Rows => "independent multinomial sampling (row totals fixed)",
            super::bayes::Sampling::Columns => "independent multinomial sampling (column totals fixed)",
            super::bayes::Sampling::Poisson => "Poisson sampling (nothing fixed)",
        };
        super::bayes::add(&mut b, lbf, &format!("Gunel-Dickey, {design}, concentration 1"));
    }

    let two_by_two = rlev.len() == 2 && clev.len() == 2;
    let fisher = if spec.fisher && two_by_two {
        let c = |i: usize, j: usize| counts[i][j] as u64;
        prismqda_stats::contingency::fisher_exact(c(0, 0), c(0, 1), c(1, 0), c(1, 1), 0.95)
    } else {
        None
    };
    if let Some(f) = &fisher {
        set(&mut b, "fisher.p", f.p);
        set(&mut b, "fisher.or", f.odds_ratio);
        set(&mut b, "fisher.or.lower", f.ci.0);
        set(&mut b, "fisher.or.upper", f.ci.1);
        b.method_note.push_str(" Fisher's exact test: two-sided p value (the total probability of the tables no more likely than the one observed), with the conditional maximum-likelihood odds ratio and its exact 95% interval.");
    } else if spec.fisher {
        b.method_note.push_str(" Fisher's exact test was asked for, but it is offered for two-by-two tables only.");
    }
    let fisher_rows = |f: &prismqda_stats::contingency::Fisher| {
        vec![
            vec![text("Fisher's exact test"), Cell::Empty, Cell::Empty, p(f.p)],
        ]
    };
    let odds_row = |f: &prismqda_stats::contingency::Fisher| {
        vec![
            text(format!("Odds ratio, {} vs {} (95% CI {} to {})", rlev[0].label, rlev[1].label, apa::num(f.ci.0, 2), if f.ci.1.is_finite() { apa::num(f.ci.1, 2) } else { "∞".into() })),
            num(f.odds_ratio, 3),
        ]
    };
    if spec.risk {
        if two_by_two {
            risk_table(&mut b, &counts, &rlev, &clev, &rl, &cl);
        } else {
            b.method_note.push_str(" Relative risk and the odds ratio were asked for, but they are given for two-by-two tables only.");
        }
    }
    if spec.ordinal {
        ordinal_table(&mut b, &counts);
    }
    if let Some((ll, (ls, llev))) = &layer {
        // Each layer's two-by-two table, over the row and column categories kept above.
        let mut tables: Vec<(String, [[f64; 2]; 2])> = Vec::new();
        for (k, lv) in llev.iter().enumerate() {
            let mut t = [[0.0; 2]; 2];
            let mut any = false;
            for &i in &used_cases {
                if ls[i].value() != Some(k) {
                    continue;
                }
                let (Some(r), Some(c)) = (rs[i].value(), cs[i].value()) else { continue };
                let (Some(ri), Some(ci)) = (keep_r.iter().position(|x| *x == r), keep_c.iter().position(|x| *x == c)) else { continue };
                if ri < 2 && ci < 2 {
                    t[ri][ci] += 1.0;
                    any = true;
                }
            }
            if any {
                tables.push((lv.label.clone(), t));
            }
        }
        if two_by_two {
            cmh_tables(&mut b, &tables, ll);
        } else {
            b.method_note.push_str(" The Cochran-Mantel-Haenszel test was asked for, but it needs a two-by-two table in each layer.");
        }
    }
    if !spec.chi_square {
        if let Some(f) = &fisher {
            b.tables.push(Table { id: "fisher".into(), title: "Fisher's exact test".into(), columns: vec!["Test".into(), "Value".into(), "df".into(), "p (two-sided)".into()], rows: fisher_rows(f), footnotes: Vec::new() });
            b.tables.push(Table { id: "effect".into(), title: "Effect size".into(), columns: vec!["Measure".into(), "Value".into()], rows: vec![odds_row(f)], footnotes: vec!["The conditional maximum-likelihood estimate of the odds ratio.".into()] });
        }
        return Ok(b);
    }
    if rlev.len() < 2 || clev.len() < 2 {
        fail(&mut b, format!("The chi-square test needs at least two categories with data in each variable ({rl}: {}, {cl}: {}).", rlev.len(), clev.len()));
        return Ok(b);
    }
    let Some(c) = chi else {
        fail(
            &mut b,
            "The chi-square test could not be computed for this table.",
        );
        return Ok(b);
    };
    let two = c.rows == 2 && c.cols == 2;
    let (stat, pval) = match (spec.continuity, c.yates) {
        (true, Some((s, pv))) => (s, pv),
        _ => (c.chi2, c.p),
    };
    for (k, v) in [
        ("chi2", stat),
        ("df", c.df),
        ("p", pval),
        ("V", c.cramers_v),
        ("g2", c.g2),
        ("g2.p", c.g2_p),
        ("min_expected", c.smallest_expected),
        ("share_below_5", c.share_expected_below_5),
    ] {
        set(&mut b, k, v);
    }
    set(&mut b, "rows", c.rows as f64);
    set(&mut b, "cols", c.cols as f64);
    if let Some(phi) = c.phi {
        set(&mut b, "phi", phi);
    }
    let mut test_rows = vec![vec![
        text("Pearson chi-square"),
        num(c.chi2, 3),
        num(c.df, 0),
        p(c.p),
    ]];
    if two && spec.continuity {
        if let Some((s, pv)) = c.yates {
            test_rows.push(vec![
                text("Continuity correction (Yates)"),
                num(s, 3),
                num(c.df, 0),
                p(pv),
            ]);
        }
    }
    test_rows.push(vec![
        text("Likelihood ratio (G²)"),
        num(c.g2, 3),
        num(c.df, 0),
        p(c.g2_p),
    ]);
    if let Some(f) = &fisher {
        test_rows.extend(fisher_rows(f));
    }
    test_rows.push(vec![
        text("N of valid cases"),
        count(n),
        Cell::Empty,
        Cell::Empty,
    ]);
    b.tables.push(Table {
        id: "chi_square".into(),
        title: "Chi-square tests".into(),
        columns: vec!["Test".into(), "Value".into(), "df".into(), "p (two-sided)".into()],
        rows: test_rows,
        footnotes: vec![format!(
            "{} of {} cells ({}%) have an expected count below 5. The smallest expected count is {}.",
            (c.share_expected_below_5 * (c.rows * c.cols) as f64).round() as usize,
            c.rows * c.cols,
            apa::num(c.share_expected_below_5 * 100.0, 1),
            apa::num(c.smallest_expected, 2)
        )],
    });
    let mut eff = Vec::new();
    if let Some(phi) = c.phi {
        eff.push(vec![text("Phi (φ)"), num(phi, 3)]);
    }
    eff.push(vec![text("Cramér's V"), num(c.cramers_v, 3)]);
    if let Some(f) = &fisher {
        eff.push(odds_row(f));
    }
    eff.push(vec![
        text("Contingency coefficient"),
        num(c.contingency_c, 3),
    ]);
    b.tables.push(Table {
        id: "effect".into(),
        title: "Effect size".into(),
        columns: vec!["Measure".into(), "Value".into()],
        rows: eff,
        footnotes: Vec::new(),
    });
    b.diagnostics.push(if c.smallest_expected < 1.0 || c.share_expected_below_5 > 0.2 {
        Diagnostic {
            id: "expected".into(),
            title: "Expected counts".into(),
            state: DiagnosticState::ReviewNeeded,
            message: format!(
                "{}% of cells have an expected count below 5 and the smallest is {}. The chi-square p value may be inaccurate with expected counts this small. {}",
                apa::num(c.share_expected_below_5 * 100.0, 0),
                apa::num(c.smallest_expected, 2),
                if two && fisher.is_some() { "Fisher's exact test, shown above, does not depend on large expected counts." } else if two { "Fisher's exact test (an option in this dialog) is the usual alternative for a two-by-two table." } else { "Consider whether categories can be combined for a substantive reason (PrismStatistics never merges them for you)." }
            ),
        }
    } else {
        Diagnostic { id: "expected".into(), title: "Expected counts".into(), state: DiagnosticState::NoConcernDetected, message: format!("Every expected count is at least 1 and no more than 20% are below 5 (smallest: {}).", apa::num(c.smallest_expected, 2)) }
    });
    b.diagnostics.push(not_assessed("independence", "Independent observations", "The test assumes each case is counted once and cases don't influence each other. Repeated measurements of the same people need a different test."));
    Ok(b)
}


/// Relative risk (each column as the outcome) and the odds ratio of a two-by-two table, with Wald intervals on the log
/// scale (Woolf's for the odds ratio). Counts of zero leave the estimate undefined rather than adding 0.5.
fn risk_table(b: &mut ResultBundle, counts: &[Vec<usize>], rlev: &[Level], clev: &[Level], rl: &str, cl: &str) {
    let (a, bb, c, d) = (counts[0][0] as f64, counts[0][1] as f64, counts[1][0] as f64, counts[1][1] as f64);
    let z = prismqda_stats::dist::norm_quantile(0.975);
    let est = |e: f64, se: f64| if e.is_finite() && e > 0.0 && se.is_finite() { (e, (e.ln() - z * se).exp(), (e.ln() + z * se).exp()) } else { (f64::NAN, f64::NAN, f64::NAN) };
    let or = est(a * d / (bb * c), (1.0 / a + 1.0 / bb + 1.0 / c + 1.0 / d).sqrt());
    let rr1 = est((a / (a + bb)) / (c / (c + d)), (1.0 / a - 1.0 / (a + bb) + 1.0 / c - 1.0 / (c + d)).sqrt());
    let rr2 = est((bb / (a + bb)) / (d / (c + d)), (1.0 / bb - 1.0 / (a + bb) + 1.0 / d - 1.0 / (c + d)).sqrt());
    for (k, v) in [("or", or), ("rr1", rr1), ("rr2", rr2)] {
        set(b, format!("risk.{k}"), v.0);
        set(b, format!("risk.{k}.lower"), v.1);
        set(b, format!("risk.{k}.upper"), v.2);
    }
    let row = |name: String, v: (f64, f64, f64)| vec![text(name), num(v.0, 3), num(v.1, 3), num(v.2, 3)];
    b.tables.push(Table {
        id: "risk".into(),
        title: "Risk estimates".into(),
        columns: vec!["Measure".into(), "Value".into(), "95% CI lower".into(), "95% CI upper".into()],
        rows: vec![
            row(format!("Odds ratio ({rl}: {} vs {})", rlev[0].label, rlev[1].label), or),
            row(format!("Relative risk of {cl} = {} ({} vs {})", clev[0].label, rlev[0].label, rlev[1].label), rr1),
            row(format!("Relative risk of {cl} = {} ({} vs {})", clev[1].label, rlev[0].label, rlev[1].label), rr2),
        ],
        footnotes: vec!["Intervals are on the log scale (Woolf's for the odds ratio). A cell count of zero leaves a measure undefined.".into()],
    });
    b.method_note.push_str(" Risk estimates: the sample odds ratio and relative risks with 95% log-scale (Wald) intervals.");
}

/// Ordinal measures of association, for row and column categories in a meaningful order.
fn ordinal_table(b: &mut ResultBundle, counts: &[Vec<usize>]) {
    let t: Vec<Vec<f64>> = counts.iter().map(|r| r.iter().map(|c| *c as f64).collect()).collect();
    let Some(o) = prismqda_stats::contingency::ordinal_association(&t) else {
        b.method_note.push_str(" Ordinal measures were asked for but could not be computed (every pair of cases is tied).");
        return;
    };
    let z = prismqda_stats::dist::norm_quantile(0.975);
    let mut rows = Vec::new();
    for (key, name, m) in [("gamma", "Gamma", o.gamma), ("tau_b", "Kendall's tau-b", o.tau_b), ("tau_c", "Stuart's tau-c", o.tau_c), ("somers_c", "Somers' d (column dependent)", o.somers_columns_dependent), ("somers_r", "Somers' d (row dependent)", o.somers_rows_dependent)] {
        set(b, format!("ord.{key}"), m.estimate);
        set(b, format!("ord.{key}.ase"), m.ase);
        set(b, format!("ord.{key}.p"), m.p);
        rows.push(vec![text(name), num(m.estimate, 3), num(m.ase, 3), num(m.estimate - z * m.ase, 3), num(m.estimate + z * m.ase, 3), num(m.z, 3), p(m.p)]);
    }
    rows.push(vec![text("Somers' d (symmetric)"), num(o.somers_symmetric, 3), Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty]);
    b.tables.push(Table {
        id: "ordinal".into(),
        title: "Ordinal measures of association".into(),
        columns: vec!["Measure".into(), "Value".into(), "ASE".into(), "95% CI lower".into(), "95% CI upper".into(), "z".into(), "p".into()],
        rows,
        footnotes: vec![format!("Based on {} concordant and {} discordant pairs. The test uses the standard error under the hypothesis of no association; the interval uses the asymptotic standard error. Rows and columns are taken in code order.", apa::num(o.concordant, 0), apa::num(o.discordant, 0))],
    });
    b.method_note.push_str(" Ordinal measures (Agresti, 2002): gamma, tau-b, tau-c and Somers' d with asymptotic standard errors.");
}

/// The Cochran-Mantel-Haenszel test of conditional independence across layers, the Mantel-Haenszel common odds ratio
/// with the Robins-Breslow-Greenland interval, and the Breslow-Day test of a common odds ratio.
fn cmh_tables(b: &mut ResultBundle, tables: &[(String, [[f64; 2]; 2])], ll: &str) {
    let (mut num_a, mut sum_e, mut sum_v) = (0.0, 0.0, 0.0);
    let (mut r_sum, mut s_sum, mut pr, mut ps_qr, mut qs) = (0.0, 0.0, 0.0, 0.0, 0.0);
    let mut usable = Vec::new();
    for (name, t) in tables {
        let (a, bb, c, d) = (t[0][0], t[0][1], t[1][0], t[1][1]);
        let n = a + bb + c + d;
        if n < 2.0 {
            continue;
        }
        let (n1, n2, m1, m2) = (a + bb, c + d, a + c, bb + d);
        num_a += a;
        sum_e += n1 * m1 / n;
        sum_v += n1 * n2 * m1 * m2 / (n * n * (n - 1.0));
        let (r, s) = (a * d / n, bb * c / n);
        let (pp, q) = ((a + d) / n, (bb + c) / n);
        r_sum += r;
        s_sum += s;
        pr += pp * r;
        ps_qr += pp * s + q * r;
        qs += q * s;
        usable.push((name.clone(), *t));
    }
    if usable.len() < 2 || sum_v <= 0.0 {
        b.method_note.push_str(&format!(" The Cochran-Mantel-Haenszel test needs at least two layers of {ll} with data in each; it was not computed."));
        return;
    }
    let chi2 = (num_a - sum_e).powi(2) / sum_v;
    let pv = prismqda_stats::dist::chisq_sf(chi2, 1.0);
    let or = r_sum / s_sum;
    let var_log = pr / (2.0 * r_sum * r_sum) + ps_qr / (2.0 * r_sum * s_sum) + qs / (2.0 * s_sum * s_sum);
    let z = prismqda_stats::dist::norm_quantile(0.975);
    let (lo, hi) = ((or.ln() - z * var_log.sqrt()).exp(), (or.ln() + z * var_log.sqrt()).exp());
    // Breslow-Day: in each layer, the count in the first cell that the common odds ratio predicts given the margins.
    let mut bd = 0.0;
    for (_, t) in &usable {
        let (a, bb, c, d) = (t[0][0], t[0][1], t[1][0], t[1][1]);
        let (n1, m1, n) = (a + bb, a + c, a + bb + c + d);
        // Solve (e)(n - n1 - m1 + e) = or (n1 - e)(m1 - e) for e within the feasible range.
        let (qa, qb, qc) = (1.0 - or, n - n1 - m1 + or * (n1 + m1), -or * n1 * m1);
        let lo_e = (n1 + m1 - n).max(0.0);
        let hi_e = n1.min(m1);
        let e = if qa.abs() < 1e-12 { -qc / qb } else {
            let disc = (qb * qb - 4.0 * qa * qc).max(0.0).sqrt();
            let (r1, r2) = ((-qb + disc) / (2.0 * qa), (-qb - disc) / (2.0 * qa));
            if r1 >= lo_e - 1e-9 && r1 <= hi_e + 1e-9 { r1 } else { r2 }
        };
        let v = 1.0 / (1.0 / e + 1.0 / (n1 - e) + 1.0 / (m1 - e) + 1.0 / (n - n1 - m1 + e));
        if v.is_finite() && v > 0.0 {
            bd += (a - e).powi(2) / v;
        }
    }
    let bd_df = (usable.len() - 1) as f64;
    let bd_p = prismqda_stats::dist::chisq_sf(bd, bd_df);
    for (k, v) in [("cmh.chi2", chi2), ("cmh.p", pv), ("cmh.or", or), ("cmh.or.lower", lo), ("cmh.or.upper", hi), ("bd.chi2", bd), ("bd.df", bd_df), ("bd.p", bd_p), ("cmh.layers", usable.len() as f64)] {
        set(b, k, v);
    }
    set_text(b, "layer.label", ll);
    b.tables.push(Table {
        id: "layers".into(),
        title: format!("Two-by-two table in each layer of {ll}"),
        columns: vec![ll.to_string(), "Cell 1,1".into(), "Cell 1,2".into(), "Cell 2,1".into(), "Cell 2,2".into(), "Odds ratio".into()],
        rows: usable.iter().map(|(n, t)| vec![text(n), count(t[0][0] as usize), count(t[0][1] as usize), count(t[1][0] as usize), count(t[1][1] as usize), num(t[0][0] * t[1][1] / (t[0][1] * t[1][0]), 3)]).collect(),
        footnotes: Vec::new(),
    });
    b.tables.push(Table {
        id: "cmh".into(),
        title: format!("Tests across the layers of {ll}"),
        columns: vec!["Test".into(), "Chi-square".into(), "df".into(), "p".into()],
        rows: vec![
            vec![text("Cochran-Mantel-Haenszel (conditional independence)"), num(chi2, 3), num(1.0, 0), p(pv)],
            vec![text("Breslow-Day (equal odds ratios across layers)"), num(bd, 3), num(bd_df, 0), p(bd_p)],
        ],
        footnotes: vec![format!("Mantel-Haenszel common odds ratio = {}, 95% CI [{}, {}] (Robins-Breslow-Greenland). The CMH statistic has no continuity correction.", apa::num(or, 3), apa::num(lo, 3), apa::num(hi, 3))],
    });
    b.method_note.push_str(&format!(" Across the layers of {ll}: the Cochran-Mantel-Haenszel test without a continuity correction, the Mantel-Haenszel common odds ratio with the Robins-Breslow-Greenland interval, and the Breslow-Day test of homogeneity."));
}
