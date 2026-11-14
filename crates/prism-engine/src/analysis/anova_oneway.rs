//! One-way analysis of variance (`anova.oneway`, task T3-09).
//!
//! Defaults (05 §1): the conventional F test, or Welch's F when chosen (never picked from Levene's test); post hoc
//! comparisons none, Tukey (pooled variance) or Games-Howell (unequal variances); effect sizes omega squared and eta
//! squared, both named. Every category of the factor with data is a group, in code order. Computed by
//! `prismqda_stats::anova`.

use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::apa;
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Diagnostic, DiagnosticState, ResultBundle, Table};

pub const METHOD_ID: &str = "anova.oneway";
pub const METHOD_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Estimator {
    #[default]
    Conventional,
    Welch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PostHoc {
    #[default]
    None,
    Tukey,
    GamesHowell,
    Bonferroni,
    Holm,
    Scheffe,
}

/// How the p-values of several planned contrasts are adjusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContrastAdjust {
    #[default]
    None,
    Bonferroni,
    Holm,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    /// One result per dependent variable.
    pub dependent: Vec<String>,
    pub factor: String,
    #[serde(default)]
    pub estimator: Estimator,
    #[serde(default)]
    pub post_hoc: PostHoc,
    /// Planned contrasts: one coefficient per group, in code order, summing to zero.
    #[serde(default)]
    pub contrasts: Vec<Vec<f64>>,
    /// Linear, quadratic and cubic trend contrasts across the groups in code order (equally spaced).
    #[serde(default)]
    pub polynomial: bool,
    #[serde(default)]
    pub contrast_adjust: ContrastAdjust,
    /// A means plot with intervals, and the residual plots.
    #[serde(default)]
    pub plots: bool,
    /// A Bayes factor for the effect (BayesFactor's default prior unless a scale is given).
    #[serde(default)]
    pub bayes: bool,
    #[serde(default)]
    pub prior_scale: Option<f64>,
    #[serde(default = "default_level")]
    pub level: f64,
}

/// Orthonormal polynomial contrasts for `k` equally spaced groups (R's `contr.poly`), up to degree `max_degree`.
pub fn polynomial_contrasts(k: usize, max_degree: usize) -> Vec<Vec<f64>> {
    let x: Vec<f64> = (1..=k).map(|i| i as f64).collect();
    let mut basis: Vec<Vec<f64>> = vec![vec![1.0 / (k as f64).sqrt(); k]];
    for d in 1..=max_degree.min(k.saturating_sub(1)) {
        let mut v: Vec<f64> = x.iter().map(|xi| xi.powi(d as i32)).collect();
        for b in &basis {
            let dot: f64 = v.iter().zip(b).map(|(a, c)| a * c).sum();
            for (vi, bi) in v.iter_mut().zip(b) {
                *vi -= dot * bi;
            }
        }
        let norm = v.iter().map(|a| a * a).sum::<f64>().sqrt();
        basis.push(v.into_iter().map(|a| a / norm).collect());
    }
    basis.into_iter().skip(1).collect()
}

/// One planned contrast, tested on the pooled variance and without assuming equal variances.
#[derive(Debug, Clone, PartialEq)]
pub struct ContrastTest {
    pub estimate: f64,
    pub se: f64,
    pub t: f64,
    pub df: f64,
    pub p: f64,
}

/// The contrast `c` of the group means, with the pooled-variance test and the Welch (Satterthwaite) test.
fn contrast_tests(a: &prismqda_stats::anova::OneWay, c: &[f64]) -> (ContrastTest, Option<ContrastTest>) {
    use prismqda_stats::dist::t_two_sided;
    let estimate: f64 = a.groups.iter().zip(c).map(|(g, ci)| ci * g.mean).sum();
    let se = (a.ms_within * a.groups.iter().zip(c).map(|(g, ci)| ci * ci / g.n as f64).sum::<f64>()).sqrt();
    let t = estimate / se;
    let pooled = ContrastTest { estimate, se, t, df: a.df_within, p: t_two_sided(t, a.df_within) };
    let parts: Option<Vec<(f64, f64)>> = a
        .groups
        .iter()
        .zip(c)
        .map(|(g, ci)| g.sd.filter(|_| g.n > 1).map(|s| (ci * ci * s * s / g.n as f64, (g.n - 1) as f64)))
        .collect();
    let welch = parts.and_then(|parts| {
        let var: f64 = parts.iter().map(|p| p.0).sum();
        let denom: f64 = parts.iter().filter(|p| p.0 > 0.0).map(|p| p.0 * p.0 / p.1).sum();
        if var <= 0.0 || denom <= 0.0 {
            return None;
        }
        let se = var.sqrt();
        let df = var * var / denom;
        let t = estimate / se;
        Some(ContrastTest { estimate, se, t, df, p: t_two_sided(t, df) })
    });
    (pooled, welch)
}

fn default_level() -> f64 {
    0.95
}

impl Default for Spec {
    fn default() -> Self {
        Spec {
            dependent: Vec::new(),
            factor: String::new(),
            estimator: Estimator::default(),
            post_hoc: PostHoc::default(),
            contrasts: Vec::new(),
            polynomial: false,
            contrast_adjust: ContrastAdjust::default(),
            plots: false,
            bayes: false,
            prior_scale: None,
            level: default_level(),
        }
    }
}

/// The values of an outcome split by the categories of a factor, with the categories that have data.
pub(crate) struct Split {
    pub groups: Vec<Vec<f64>>,
    pub levels: Vec<Level>,
    pub tally: Tally,
    pub total: usize,
}

pub(crate) fn split(ds: &Dataset, outcome: &str, factor: &str) -> Result<Split> {
    if ds.find(outcome) == ds.find(factor) {
        return Err(Error::Invalid(
            "The factor can't also be a dependent variable.".into(),
        ));
    }
    let (slots, levels) = categories(ds, factor)?;
    let ys = numeric(ds, outcome)?;
    let mut tally = Tally::default();
    let mut groups = vec![Vec::new(); levels.len()];
    for (g, y) in slots.iter().zip(&ys) {
        if tally.check(g, "factor") || tally.check(y, "dependent variable") {
            continue;
        }
        groups[g.value().expect("checked")].push(y.value().expect("checked"));
    }
    // A category whose cases all lack the outcome is not a group.
    let keep: Vec<bool> = groups.iter().map(|g| !g.is_empty()).collect();
    let levels = levels
        .into_iter()
        .zip(&keep)
        .filter(|(_, k)| **k)
        .map(|(l, _)| l)
        .collect();
    let groups = groups.into_iter().filter(|g| !g.is_empty()).collect();
    Ok(Split {
        groups,
        levels,
        tally,
        total: ys.len(),
    })
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<Vec<ResultBundle>> {
    if spec.dependent.is_empty() {
        return Err(Error::Invalid(
            "Choose at least one dependent variable.".into(),
        ));
    }
    check_level(spec.level)?;
    spec.dependent
        .iter()
        .map(|name| run_one(ds, spec, name))
        .collect()
}

fn run_one(ds: &Dataset, spec: &Spec, name: &str) -> Result<ResultBundle> {
    let y_var = ds.variable(name)?;
    let f_var = ds.variable(&spec.factor)?;
    let Split {
        groups,
        levels,
        tally,
        total,
    } = split(ds, name, &spec.factor)?;
    let analyzed: usize = groups.iter().map(Vec::len).sum();
    let lv = level_text(spec.level);
    let test_name = match spec.estimator {
        Estimator::Conventional => "the conventional F test (equal variances assumed)",
        Estimator::Welch => "Welch's F test (equal variances not assumed)",
    };
    let post_name = match spec.post_hoc {
        PostHoc::None => None,
        PostHoc::Tukey => {
            Some("Tukey's HSD (Tukey-Kramer for unequal group sizes), on the pooled variance")
        }
        PostHoc::GamesHowell => Some("Games-Howell, which does not assume equal variances"),
        PostHoc::Bonferroni => Some("t tests on the pooled variance with Bonferroni-adjusted p values and intervals"),
        PostHoc::Holm => Some("t tests on the pooled variance with Holm-adjusted p values (the intervals are not adjusted)"),
        PostHoc::Scheffe => Some("Scheffé's test, on the pooled variance"),
    };
    let note = format!(
        "One-way analysis of variance with {test_name}. Effect sizes: omega squared (less biased) and eta squared (SS between / SS total). Levene's test uses deviations from the group medians (Brown-Forsythe). {}Groups are the categories of {} with data. Cases were used when both the factor and the dependent variable had valid values. Computed by prismqda-stats {}.",
        post_name.map_or(String::new(), |p| format!("Pairs of groups compared by {p}, with {lv} simultaneous intervals. ")),
        f_var.display_name(),
        super::STATS_VERSION
    );
    let mut b = common::bundle(
        METHOD_ID,
        METHOD_VERSION,
        format!(
            "One-Way ANOVA: {} by {}",
            y_var.display_name(),
            f_var.display_name()
        ),
        serde_json::to_value(spec).expect("a spec always serializes"),
        tally.accounting(total, analyzed),
        note,
    );
    set_text(&mut b, "y.label", y_var.display_name());
    set_text(&mut b, "factor.label", f_var.display_name());
    set_text(
        &mut b,
        "test",
        if spec.estimator == Estimator::Welch {
            "welch"
        } else {
            "conventional"
        },
    );
    set(&mut b, "level", spec.level);
    set(&mut b, "k", groups.len() as f64);
    set(&mut b, "n", analyzed as f64);
    for (i, (l, g)) in levels.iter().zip(&groups).enumerate() {
        set_text(&mut b, format!("group.{}.label", i + 1), l.label.clone());
        set(&mut b, format!("group.{}.n", i + 1), g.len() as f64);
    }
    b.tables.push(Table {
        id: "groups".into(),
        title: "Descriptive statistics".into(),
        columns: vec![
            f_var.display_name().into(),
            "N".into(),
            "Mean".into(),
            "SD".into(),
            "SE of mean".into(),
        ],
        rows: levels
            .iter()
            .zip(&groups)
            .map(|(l, g)| summary_row(&l.label, g))
            .collect(),
        footnotes: Vec::new(),
    });
    if groups.len() < 2 {
        fail(
            &mut b,
            format!(
                "The analysis needs at least two groups with data; {} has {}.",
                f_var.display_name(),
                groups.len()
            ),
        );
        return Ok(b);
    }
    let Some(a) = prismqda_stats::anova::one_way(&groups) else {
        fail(
            &mut b,
            if analyzed <= groups.len() {
                "There are too few cases: every group has a single value, so there is no variation within groups to compare with.".to_string()
            } else {
                "No group has any variation within it, so the F statistic is undefined.".to_string()
            },
        );
        return Ok(b);
    };
    for (i, g) in a.groups.iter().enumerate() {
        set(&mut b, format!("group.{}.mean", i + 1), g.mean);
        set(
            &mut b,
            format!("group.{}.sd", i + 1),
            g.sd.unwrap_or(f64::NAN),
        );
    }
    for (k, v) in [
        ("ss.between", a.ss_between),
        ("ss.within", a.ss_within),
        ("df.between", a.df_between),
        ("df.within", a.df_within),
        ("F", a.test.f),
        ("df1", a.test.df1),
        ("df2", a.test.df2),
        ("p", a.test.p),
        ("eta2", a.eta_squared),
        ("omega2", a.omega_squared),
    ] {
        set(&mut b, k, v);
    }
    b.tables.push(Table {
        id: "anova".into(),
        title: "Analysis of variance".into(),
        columns: vec![
            "Source".into(),
            "Sum of squares".into(),
            "df".into(),
            "Mean square".into(),
            "F".into(),
            "p".into(),
        ],
        rows: vec![
            vec![
                text("Between groups"),
                num(a.ss_between, 3),
                num(a.df_between, 0),
                num(a.ms_between, 3),
                num(a.test.f, 3),
                p(a.test.p),
            ],
            vec![
                text("Within groups"),
                num(a.ss_within, 3),
                num(a.df_within, 0),
                num(a.ms_within, 3),
                Cell::Empty,
                Cell::Empty,
            ],
            vec![
                text("Total"),
                num(a.ss_total, 3),
                num(a.df_between + a.df_within, 0),
                Cell::Empty,
                Cell::Empty,
                Cell::Empty,
            ],
        ],
        footnotes: Vec::new(),
    });
    match (&a.welch, spec.estimator) {
        (Some(w), _) => {
            set(&mut b, "welch.F", w.f);
            set(&mut b, "welch.df1", w.df1);
            set(&mut b, "welch.df2", w.df2);
            set(&mut b, "welch.p", w.p);
            b.tables.push(Table {
                id: "welch".into(),
                title: "Welch's test (equal variances not assumed)".into(),
                columns: vec![
                    "Test".into(),
                    "F".into(),
                    "df1".into(),
                    "df2".into(),
                    "p".into(),
                ],
                rows: vec![vec![
                    text("Welch"),
                    num(w.f, 3),
                    num(w.df1, 0),
                    num(w.df2, 2),
                    p(w.p),
                ]],
                footnotes: if spec.estimator == Estimator::Conventional {
                    vec![
                        "Shown for reference; the conventional F test above is the one chosen."
                            .into(),
                    ]
                } else {
                    Vec::new()
                },
            });
        }
        (None, Estimator::Welch) => {
            fail(
                &mut b,
                "Welch's test needs at least two values and some variation in every group.",
            );
            return Ok(b);
        }
        (None, Estimator::Conventional) => {}
    }
    if spec.bayes {
        let r = spec.prior_scale.unwrap_or(0.5);
        if let Some(lbf) = super::bayes::anova_log_bf(&groups, r) {
            super::bayes::add(&mut b, lbf, &format!("g prior on the fixed effects, scale {}", apa::num(r, 3)));
        }
    }
    b.tables.push(Table {
        id: "effect".into(),
        title: "Effect size".into(),
        columns: vec!["Measure".into(), "Estimate".into()],
        rows: vec![
            vec![text("Omega squared (ω²)"), num(a.omega_squared, 3)],
            vec![text("Eta squared (η²)"), num(a.eta_squared, 3)],
        ],
        footnotes: vec!["Both come from the conventional sums of squares.".into()],
    });
    if let Some(post) = spec.post_hoc.into_stats() {
        let pairs = prismqda_stats::anova::compare_pairs(&a, post, spec.level);
        let mut rows = Vec::new();
        for (i, pr) in pairs.iter().enumerate() {
            let key = format!("pair.{}", i + 1);
            set_text(&mut b, format!("{key}.a"), levels[pr.a].label.clone());
            set_text(&mut b, format!("{key}.b"), levels[pr.b].label.clone());
            set(&mut b, format!("{key}.diff"), pr.difference);
            set(&mut b, format!("{key}.p"), pr.p_adjusted);
            rows.push(vec![
                text(&levels[pr.a].label),
                text(&levels[pr.b].label),
                num(pr.difference, 3),
                num(pr.se, 3),
                p(pr.p_adjusted),
                num(pr.ci.0, 3),
                num(pr.ci.1, 3),
            ]);
        }
        let title = match spec.post_hoc {
            PostHoc::Tukey => "Tukey HSD",
            PostHoc::GamesHowell => "Games-Howell",
            PostHoc::Bonferroni => "Bonferroni",
            PostHoc::Holm => "Holm",
            _ => "Scheffé",
        };
        set_text(&mut b, "post_hoc", title);
        b.tables.push(Table {
            id: "post_hoc".into(),
            title: format!("Multiple comparisons ({title})"),
            columns: vec![
                "Group (I)".into(),
                "Group (J)".into(),
                "Mean difference (I − J)".into(),
                "SE".into(),
                "p (adjusted)".into(),
                format!("{lv} CI lower"),
                format!("{lv} CI upper"),
            ],
            rows,
            footnotes: vec![if spec.post_hoc == PostHoc::Holm {
                format!("p values are adjusted for the {} comparisons by Holm's method; the intervals are not adjusted.", pairs.len())
            } else {
                format!("p values and intervals are adjusted for the {} comparisons.", pairs.len())
            }],
        });
    }

    contrasts(&mut b, spec, &a, &levels)?;
    if spec.plots {
        plots(&mut b, spec, &a, &groups, &levels, y_var.display_name(), f_var.display_name());
    }

    let smallest = groups.iter().map(Vec::len).min().unwrap_or(0);
    b.diagnostics.push(size_diagnostic(
        smallest,
        format!(
            "The smallest group has {smallest} case{}",
            if smallest == 1 { "" } else { "s" }
        ),
    ));
    b.diagnostics.push(match (&a.levene, spec.estimator) {
        (Some(l), Estimator::Conventional) if l.p < 0.05 => Diagnostic {
            id: "variances".into(),
            title: "Unequal spread".into(),
            state: DiagnosticState::ReviewNeeded,
            message: format!("Levene's test suggests the groups differ in spread, F({}, {}) = {}, p {}. The conventional F test assumes equal spread; Welch's test does not and is one click away.", apa::df(l.df1), apa::df(l.df2), apa::num(l.f, 2), apa::p_clause(l.p)),
        },
        (Some(l), _) => Diagnostic {
            id: "variances".into(),
            title: "Unequal spread".into(),
            state: DiagnosticState::NoConcernDetected,
            message: format!(
                "Levene's test: F({}, {}) = {}, p {}.{}",
                apa::df(l.df1),
                apa::df(l.df2),
                apa::num(l.f, 2),
                apa::p_clause(l.p),
                if spec.estimator == Estimator::Welch { " Welch's test doesn't assume equal spread." } else { "" }
            ),
        },
        (None, _) => Diagnostic { id: "variances".into(), title: "Unequal spread".into(), state: DiagnosticState::NotAssessed, message: "Levene's test needs at least two values in a group.".into() },
    });
    if let Some(l) = &a.levene {
        set(&mut b, "levene.F", l.f);
        set(&mut b, "levene.p", l.p);
    }
    b.diagnostics.push(not_assessed(
        "shape",
        "Shape and outliers",
        "Look at a boxplot of each group.",
    ));
    Ok(b)
}

use crate::result::Cell;

impl PostHoc {
    fn into_stats(self) -> Option<prismqda_stats::anova::PostHoc> {
        match self {
            PostHoc::None => None,
            PostHoc::Tukey => Some(prismqda_stats::anova::PostHoc::Tukey),
            PostHoc::GamesHowell => Some(prismqda_stats::anova::PostHoc::GamesHowell),
            PostHoc::Bonferroni => Some(prismqda_stats::anova::PostHoc::Adjusted(prismqda_stats::adjust::Adjustment::Bonferroni)),
            PostHoc::Holm => Some(prismqda_stats::anova::PostHoc::Adjusted(prismqda_stats::adjust::Adjustment::Holm)),
            PostHoc::Scheffe => Some(prismqda_stats::anova::PostHoc::Scheffe),
        }
    }
}

/// Planned contrasts (the user's and the polynomial trends), each tested on the pooled variance and without assuming
/// equal variances; the row that matches the chosen F test carries the fields.
fn contrasts(b: &mut ResultBundle, spec: &Spec, a: &prismqda_stats::anova::OneWay, levels: &[Level]) -> Result<()> {
    use prismqda_stats::adjust::{p_adjust, Adjustment};
    use prismqda_stats::dist::t_quantile;
    let k = a.groups.len();
    let mut list: Vec<(String, Vec<f64>)> = Vec::new();
    for (i, c) in spec.contrasts.iter().enumerate() {
        if c.len() != k {
            return Err(Error::Invalid(format!("Contrast {} has {} coefficients; the factor has {k} groups with data, so it needs {k}.", i + 1, c.len())));
        }
        if c.iter().sum::<f64>().abs() > 1e-8 || c.iter().all(|x| *x == 0.0) {
            return Err(Error::Invalid(format!("The coefficients of contrast {} must add up to zero and not all be zero.", i + 1)));
        }
        let coef = c.iter().map(|x| short_number(*x)).collect::<Vec<_>>().join(", ");
        list.push((format!("Contrast {} ({coef})", i + 1), c.clone()));
    }
    if spec.polynomial {
        for (d, c) in polynomial_contrasts(k, 3).into_iter().enumerate() {
            list.push((["Linear trend", "Quadratic trend", "Cubic trend"][d].to_string(), c));
        }
    }
    if list.is_empty() {
        return Ok(());
    }
    let tests: Vec<(ContrastTest, Option<ContrastTest>)> = list.iter().map(|(_, c)| contrast_tests(a, c)).collect();
    let adj = match spec.contrast_adjust {
        ContrastAdjust::None => Adjustment::None,
        ContrastAdjust::Bonferroni => Adjustment::Bonferroni,
        ContrastAdjust::Holm => Adjustment::Holm,
    };
    let pooled_adj = p_adjust(&tests.iter().map(|t| t.0.p).collect::<Vec<_>>(), adj);
    let welch_adj = p_adjust(&tests.iter().map(|t| t.1.as_ref().map_or(f64::NAN, |w| w.p)).collect::<Vec<_>>(), adj);
    let m = list.len() as f64;
    let alpha = if spec.contrast_adjust == ContrastAdjust::Bonferroni { (1.0 - spec.level) / m } else { 1.0 - spec.level };
    let welch_chosen = spec.estimator == Estimator::Welch;
    let lv = level_text(spec.level);
    let mut rows = Vec::new();
    set(b, "contrasts", m);
    set_text(
        b,
        "contrast_adjust",
        match spec.contrast_adjust {
            ContrastAdjust::None => "none",
            ContrastAdjust::Bonferroni => "Bonferroni",
            ContrastAdjust::Holm => "Holm",
        },
    );
    for (i, ((label, c), (pooled, welch))) in list.iter().zip(&tests).enumerate() {
        let key = format!("contrast.{}", i + 1);
        set_text(b, format!("{key}.label"), label.clone());
        set_text(b, format!("{key}.coefficients"), c.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(","));
        for (equal, test, padj) in [(true, Some(pooled), pooled_adj[i]), (false, welch.as_ref(), welch_adj[i])] {
            let variant = if equal { "Equal variances assumed" } else { "Equal variances not assumed" };
            let Some(t) = test else {
                rows.push(vec![text(label), text(variant), Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty]);
                continue;
            };
            let q = t_quantile(1.0 - alpha / 2.0, t.df);
            let (lo, hi) = (t.estimate - q * t.se, t.estimate + q * t.se);
            if equal != welch_chosen {
                for (f, v) in [("est", t.estimate), ("se", t.se), ("t", t.t), ("df", t.df), ("p", t.p), ("p_adj", padj), ("lower", lo), ("upper", hi)] {
                    set(b, format!("{key}.{f}"), v);
                }
            }
            rows.push(vec![
                text(label),
                text(variant),
                num(t.estimate, 3),
                num(t.se, 3),
                num(t.t, 3),
                num(t.df, if equal { 0 } else { 2 }),
                p(t.p),
                if adj == Adjustment::None { Cell::Empty } else { p(padj) },
                num(lo, 3),
                num(hi, 3),
            ]);
        }
    }
    let mut footnotes = vec![format!(
        "Coefficients apply to the groups in this order: {}.",
        levels.iter().map(|l| l.label.as_str()).collect::<Vec<_>>().join(", ")
    )];
    if spec.polynomial {
        footnotes.push("Trend contrasts treat the groups as equally spaced, in code order (orthonormal polynomial coefficients).".into());
    }
    footnotes.push(match spec.contrast_adjust {
        ContrastAdjust::None => "p values are not adjusted for the number of contrasts.".into(),
        ContrastAdjust::Bonferroni => format!("Adjusted p values and the intervals use Bonferroni's method for the {} contrasts.", list.len()),
        ContrastAdjust::Holm => format!("Adjusted p values use Holm's method for the {} contrasts; the intervals are not adjusted.", list.len()),
    });
    b.tables.push(Table {
        id: "contrasts".into(),
        title: "Planned contrasts".into(),
        columns: vec![
            "Contrast".into(),
            "Variances".into(),
            "Estimate".into(),
            "SE".into(),
            "t".into(),
            "df".into(),
            "p".into(),
            "p (adjusted)".into(),
            format!("{lv} CI lower"),
            format!("{lv} CI upper"),
        ],
        rows,
        footnotes,
    });
    Ok(())
}

/// A coefficient as typed: 1, -0.5, 0.333.
fn short_number(x: f64) -> String {
    let s = format!("{:.3}", x);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.replace('-', "−") }
}

/// The means plot with intervals, residuals against the group means, and the normal Q-Q plot of standardized residuals.
fn plots(b: &mut ResultBundle, spec: &Spec, a: &prismqda_stats::anova::OneWay, groups: &[Vec<f64>], levels: &[Level], y_label: &str, f_label: &str) {
    use crate::chart::{self, Chart};
    use prismqda_stats::dist::t_quantile;
    let lv = level_text(spec.level);
    let cats: Vec<(String, f64)> = levels.iter().zip(&a.groups).map(|(l, g)| (l.label.clone(), g.mean)).collect();
    let errs: Vec<(f64, f64)> = a
        .groups
        .iter()
        .map(|g| match g.sd {
            Some(s) if g.n > 1 => {
                let h = t_quantile(1.0 - (1.0 - spec.level) / 2.0, (g.n - 1) as f64) * s / (g.n as f64).sqrt();
                (g.mean - h, g.mean + h)
            }
            _ => (f64::NAN, f64::NAN),
        })
        .collect();
    let alt = format!("Mean {y_label} for each category of {f_label}, with {lv} confidence intervals.");
    b.charts.push(Chart { id: "means".into(), title: format!("Means of {y_label} with {lv} confidence intervals"), alt: alt.clone(), svg: chart::bars(&cats, Some(&errs), f_label, &format!("Mean {y_label}"), &alt) });
    let mut pts = Vec::new();
    let mut std_res = Vec::new();
    for (g, s) in groups.iter().zip(&a.groups) {
        let lev = 1.0 - 1.0 / s.n as f64;
        for y in g {
            let r = y - s.mean;
            pts.push((s.mean, r));
            if lev > 0.0 {
                std_res.push(r / (a.ms_within * lev).sqrt());
            }
        }
    }
    let alt = "Residuals against the group means, with a horizontal line at zero.".to_string();
    b.charts.push(Chart { id: "resid_fitted".into(), title: "Residuals against predicted values".into(), alt: alt.clone(), svg: chart::scatter(&pts, Some((0.0, 0.0)), "Predicted value (group mean)", "Residual", &alt) });
    if let Some((qq, line)) = prismqda_stats::charts::normal_qq(&std_res) {
        let qp: Vec<(f64, f64)> = qq.iter().map(|q| (q.theoretical, q.sample)).collect();
        let alt = "Normal Q-Q plot of the standardized residuals.".to_string();
        b.charts.push(Chart { id: "resid_qq".into(), title: "Normal Q-Q plot of the standardized residuals".into(), alt: alt.clone(), svg: chart::scatter(&qp, Some((line.intercept, line.slope)), "Normal quantile", "Standardized residual", &alt) });
    }
}
