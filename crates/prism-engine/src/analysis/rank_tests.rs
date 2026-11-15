//! The rank tests (task T3-12): Mann-Whitney U (`np.mann_whitney`), Wilcoxon signed-rank (`np.wilcoxon`) and
//! Kruskal-Wallis (`np.kruskal`) with Dunn's comparisons.
//!
//! Defaults (05 §1): the exact p value when the samples are small and there are no ties, otherwise the normal
//! approximation with the continuity and tie corrections (R's choices); effect size the rank-biserial correlation, or
//! epsilon squared for Kruskal-Wallis; zero differences dropped (Wilcoxon's method); post hoc none or Dunn with Holm's
//! adjustment. Computed by `prismqda_stats::nonparametric`.

use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Cell, ResultBundle, Table};

pub const MANN_WHITNEY: &str = "np.mann_whitney";
pub const WILCOXON: &str = "np.wilcoxon";
pub const KRUSKAL: &str = "np.kruskal";
pub const METHOD_VERSION: u32 = 2;

// ---------------------------------------------------------------- Mann-Whitney

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MannWhitneySpec {
    pub test_variables: Vec<String>,
    pub grouping_variable: String,
    /// The two group codes, by code (never by label or position).
    pub groups: (f64, f64),
    /// Two-sided, or one-sided ("greater": the first group tends to be higher).
    #[serde(default)]
    pub alternative: super::sided::Alternative,
}

pub fn mann_whitney(ds: &Dataset, spec: &MannWhitneySpec) -> Result<Vec<ResultBundle>> {
    if spec.test_variables.is_empty() {
        return Err(Error::Invalid("Choose at least one test variable.".into()));
    }
    if spec.groups.0 == spec.groups.1 {
        return Err(Error::Invalid(
            "The two groups must have different codes.".into(),
        ));
    }
    spec.test_variables
        .iter()
        .map(|name| mann_whitney_one(ds, spec, name))
        .collect()
}

fn mann_whitney_one(ds: &Dataset, spec: &MannWhitneySpec, name: &str) -> Result<ResultBundle> {
    let y_var = ds.variable(name)?;
    let g_var = ds.variable(&spec.grouping_variable)?;
    let gs = numeric(ds, &spec.grouping_variable)?;
    let ys = numeric(ds, name)?;
    let (c1, c2) = spec.groups;
    let mut tally = Tally::default();
    let (mut x1, mut x2) = (Vec::new(), Vec::new());
    for (g, y) in gs.iter().zip(&ys) {
        if tally.check(g, "grouping variable") {
            continue;
        }
        let gv = g.value().expect("checked");
        if gv != c1 && gv != c2 {
            tally.add("in neither compared group");
            continue;
        }
        if tally.check(y, "test variable") {
            continue;
        }
        if gv == c1 { &mut x1 } else { &mut x2 }.push(y.value().expect("checked"));
    }
    let (l1, l2) = (g_var.value_text(c1), g_var.value_text(c2));
    let note = format!(
        "Mann-Whitney U test (Wilcoxon rank-sum test), {}. The p value is exact when both groups have fewer than 50 values and there are no ties; otherwise it uses the normal approximation with continuity and tie corrections. Effect size: the rank-biserial correlation ({l1} relative to {l2}). Groups were chosen by code. Computed by PrismStatistics.",
        spec.alternative.words()
    );
    let mut b = common::bundle(
        MANN_WHITNEY,
        METHOD_VERSION,
        format!("Mann-Whitney U Test: {}", y_var.display_name()),
        serde_json::to_value(spec).expect("a spec always serializes"),
        tally.accounting(ys.len(), x1.len() + x2.len()),
        note,
    );
    set_text(&mut b, "y.label", y_var.display_name());
    set_text(&mut b, "g1.label", l1.clone());
    set_text(&mut b, "g2.label", l2.clone());
    set(&mut b, "g1.n", x1.len() as f64);
    set(&mut b, "g2.n", x2.len() as f64);
    set_text(&mut b, "alternative", spec.alternative.key());
    let mut m = prismqda_stats::nonparametric::mann_whitney(&x1, &x2);
    if let Some(m) = m.as_mut().filter(|_| !spec.alternative.is_two_sided()) {
        if let Some((pv, z)) = super::sided::mann_whitney_p(&x1, &x2, spec.alternative) {
            m.p = pv;
            m.z = z;
        }
    }
    let (md1, md2) = (median(&x1), median(&x2));
    let rank_row = |l: &str, x: &[f64], md: Option<f64>, mr: Option<f64>| {
        vec![
            text(l),
            count(x.len()),
            opt(md, 2),
            opt(mr, 2),
            opt(mr.map(|r| r * x.len() as f64), 2),
        ]
    };
    b.tables.push(Table {
        id: "ranks".into(),
        title: "Ranks".into(),
        columns: vec![
            "Group".into(),
            "N".into(),
            "Median".into(),
            "Mean rank".into(),
            "Sum of ranks".into(),
        ],
        rows: vec![
            rank_row(&l1, &x1, md1, m.as_ref().map(|m| m.mean_rank_1)),
            rank_row(&l2, &x2, md2, m.as_ref().map(|m| m.mean_rank_2)),
        ],
        footnotes: Vec::new(),
    });
    let Some(m) = m else {
        fail(
            &mut b,
            if x1.is_empty() || x2.is_empty() {
                format!(
                    "Each group needs at least one valid value. {l1}: {}, {l2}: {}.",
                    x1.len(),
                    x2.len()
                )
            } else {
                "Every value is the same, so the groups can't be ranked.".to_string()
            },
        );
        return Ok(b);
    };
    for (k, v) in [("g1.median", md1), ("g2.median", md2)] {
        if let Some(v) = v {
            set(&mut b, k, v);
        }
    }
    for (k, v) in [
        ("U", m.u),
        ("p", m.p),
        ("r", m.rank_biserial),
        ("ps", m.probability_of_superiority),
        ("g1.mean_rank", m.mean_rank_1),
        ("g2.mean_rank", m.mean_rank_2),
    ] {
        set(&mut b, k, v);
    }
    set(&mut b, "exact", if m.exact { 1.0 } else { 0.0 });
    if let Some(z) = m.z {
        set(&mut b, "z", z);
    }
    let nn = (x1.len() * x2.len()) as f64;
    b.tables.push(Table {
        id: "test".into(),
        title: "Mann-Whitney U test".into(),
        columns: vec!["Test variable".into(), "U".into(), "z".into(), format!("p ({})", spec.alternative.words()), "p method".into()],
        rows: vec![vec![text(y_var.display_name()), num(m.u, 1), opt(m.z, 3), p(m.p), text(if m.exact { "Exact" } else { "Normal approximation" })]],
        footnotes: vec![format!("U counts the pairs of cases, one from each group, in which the {l1} case has the higher value (ties count half). For {l2} relative to {l1} it is {}.", crate::apa::num(nn - m.u, 1))],
    });
    b.tables.push(Table {
        id: "effect".into(),
        title: "Effect size".into(),
        columns: vec!["Measure".into(), "Estimate".into()],
        rows: vec![
            vec![text("Rank-biserial correlation"), num(m.rank_biserial, 3)],
            vec![
                text(format!("Probability the {l1} case is higher, in a pair with one case from each group")),
                num(m.probability_of_superiority, 3),
            ],
        ],
        footnotes: vec![format!("Positive values mean {l1} tends to be higher.")],
    });
    let smallest = x1.len().min(x2.len());
    b.diagnostics.push(size_diagnostic(
        smallest,
        format!("{l1}: {}, {l2}: {}", x1.len(), x2.len()),
    ));
    b.diagnostics.push(not_assessed("shape", "Shape of the groups", "The test compares whole distributions. Read it as a difference in medians only if the two groups have a similar shape and spread; look at a boxplot of each."));
    Ok(b)
}

// ---------------------------------------------------------------- Wilcoxon signed-rank

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WilcoxonSpec {
    /// Each pair is (first, second); differences are first minus second.
    pub pairs: Vec<(String, String)>,
    /// Two-sided, or one-sided ("greater": the first measure tends to be higher).
    #[serde(default)]
    pub alternative: super::sided::Alternative,
}

pub fn wilcoxon(ds: &Dataset, spec: &WilcoxonSpec) -> Result<Vec<ResultBundle>> {
    if spec.pairs.is_empty() {
        return Err(Error::Invalid(
            "Choose at least one pair of variables.".into(),
        ));
    }
    spec.pairs
        .iter()
        .map(|(a, c)| wilcoxon_one(ds, spec, a, c))
        .collect()
}

fn wilcoxon_one(
    ds: &Dataset,
    spec: &WilcoxonSpec,
    first: &str,
    second: &str,
) -> Result<ResultBundle> {
    let (l1, l2) = (
        ds.variable(first)?.display_name().to_string(),
        ds.variable(second)?.display_name().to_string(),
    );
    let (x, y, tally, total) = super::ttest_paired::matched(ds, first, second)?;
    let note = format!(
        "Wilcoxon signed-rank test of the differences ({l1} minus {l2}), {}. Zero differences are dropped (Wilcoxon's method). The p value is exact below 50 pairs when there are no ties or zero differences; otherwise it uses the normal approximation with continuity and tie corrections. Effect size: the matched-pairs rank-biserial correlation. Computed by PrismStatistics.",
        spec.alternative.words()
    );
    let mut b = common::bundle(
        WILCOXON,
        METHOD_VERSION,
        format!("Wilcoxon Signed-Rank Test: {l1} and {l2}"),
        serde_json::to_value(spec).expect("a spec always serializes"),
        tally.accounting(total, x.len()),
        note,
    );
    set_text(&mut b, "x1.label", l1.clone());
    set_text(&mut b, "x2.label", l2.clone());
    set(&mut b, "n", x.len() as f64);
    let d: Vec<f64> = x.iter().zip(&y).map(|(a, c)| a - c).collect();
    let (neg, pos) = (
        d.iter().filter(|v| **v < 0.0).count(),
        d.iter().filter(|v| **v > 0.0).count(),
    );
    let ties = d.len() - neg - pos;
    set_text(&mut b, "alternative", spec.alternative.key());
    let mut w = prismqda_stats::nonparametric::wilcoxon(&x, Some(&y), 0.0);
    if let Some(w) = w.as_mut().filter(|_| !spec.alternative.is_two_sided()) {
        if let Some((pv, z)) = super::sided::wilcoxon_p(&d, spec.alternative) {
            w.p = pv;
            w.z = z;
        }
    }
    // Rank sums of the nonzero differences, for the table.
    let nz: Vec<f64> = d.iter().copied().filter(|v| *v != 0.0).collect();
    let ranks = prismqda_stats::rank::rank_average(&nz.iter().map(|v| v.abs()).collect::<Vec<_>>());
    let (sum_neg, sum_pos): (f64, f64) =
        nz.iter().zip(&ranks).fold((0.0, 0.0), |(sn, sp), (v, r)| {
            if *v < 0.0 {
                (sn + r, sp)
            } else {
                (sn, sp + r)
            }
        });
    let mean_rank = |s: f64, n: usize| {
        if n == 0 {
            Cell::Empty
        } else {
            num(s / n as f64, 2)
        }
    };
    b.tables.push(Table {
        id: "ranks".into(),
        title: "Ranks".into(),
        columns: vec![
            "Differences".into(),
            "N".into(),
            "Mean rank".into(),
            "Sum of ranks".into(),
        ],
        rows: vec![
            vec![
                text(format!("Negative ({l1} < {l2})")),
                count(neg),
                mean_rank(sum_neg, neg),
                num(sum_neg, 2),
            ],
            vec![
                text(format!("Positive ({l1} > {l2})")),
                count(pos),
                mean_rank(sum_pos, pos),
                num(sum_pos, 2),
            ],
            vec![
                text("Ties (dropped)"),
                count(ties),
                Cell::Empty,
                Cell::Empty,
            ],
            vec![text("Total"), count(d.len()), Cell::Empty, Cell::Empty],
        ],
        footnotes: Vec::new(),
    });
    for (k, v) in [("neg", neg), ("pos", pos), ("ties", ties)] {
        set(&mut b, k, v as f64);
    }
    for (k, v) in [
        ("x1.median", median(&x)),
        ("x2.median", median(&y)),
        ("diff.median", median(&d)),
    ] {
        if let Some(v) = v {
            set(&mut b, k, v);
        }
    }
    let Some(w) = w else {
        fail(
            &mut b,
            if d.is_empty() {
                "There are no cases with both values.".to_string()
            } else {
                "Every difference is zero, so there is nothing to rank.".to_string()
            },
        );
        return Ok(b);
    };
    for (k, v) in [("V", w.v), ("p", w.p), ("r", w.rank_biserial)] {
        set(&mut b, k, v);
    }
    set(&mut b, "exact", if w.exact { 1.0 } else { 0.0 });
    if let Some(z) = w.z {
        set(&mut b, "z", z);
    }
    b.tables.push(Table {
        id: "test".into(),
        title: "Wilcoxon signed-rank test".into(),
        columns: vec![
            "Pair".into(),
            "V".into(),
            "z".into(),
            format!("p ({})", spec.alternative.words()),
            "p method".into(),
        ],
        rows: vec![vec![
            text(format!("{l1} − {l2}")),
            num(w.v, 1),
            opt(w.z, 3),
            p(w.p),
            text(if w.exact {
                "Exact"
            } else {
                "Normal approximation"
            }),
        ]],
        footnotes: vec!["V is the sum of the ranks of the positive differences.".into()],
    });
    b.tables.push(Table {
        id: "effect".into(),
        title: "Effect size".into(),
        columns: vec!["Measure".into(), "Estimate".into()],
        rows: vec![vec![
            text("Matched-pairs rank-biserial correlation"),
            num(w.rank_biserial, 3),
        ]],
        footnotes: vec![format!(
            "Positive values mean {l1} tends to be higher than {l2}."
        )],
    });
    b.diagnostics.push(size_diagnostic(
        w.n,
        format!("{} pairs with a nonzero difference", w.n),
    ));
    b.diagnostics.push(not_assessed("shape", "Shape of the differences", "The test assumes the differences are spread symmetrically around their center. Look at a histogram of the differences."));
    Ok(b)
}

// ---------------------------------------------------------------- Kruskal-Wallis

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum KruskalPostHoc {
    #[default]
    None,
    /// Dunn's test with Holm's adjustment.
    Dunn,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KruskalSpec {
    pub dependent: Vec<String>,
    pub factor: String,
    #[serde(default)]
    pub post_hoc: KruskalPostHoc,
    /// How Dunn's p values are adjusted (Holm's method unless chosen otherwise).
    #[serde(default)]
    pub dunn_adjust: DunnAdjust,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DunnAdjust {
    #[default]
    Holm,
    Bonferroni,
    None,
}

impl DunnAdjust {
    fn name(self) -> &'static str {
        match self {
            DunnAdjust::Holm => "Holm",
            DunnAdjust::Bonferroni => "Bonferroni",
            DunnAdjust::None => "none",
        }
    }
}

pub fn kruskal(ds: &Dataset, spec: &KruskalSpec) -> Result<Vec<ResultBundle>> {
    if spec.dependent.is_empty() {
        return Err(Error::Invalid(
            "Choose at least one dependent variable.".into(),
        ));
    }
    spec.dependent
        .iter()
        .map(|name| kruskal_one(ds, spec, name))
        .collect()
}

fn kruskal_one(ds: &Dataset, spec: &KruskalSpec, name: &str) -> Result<ResultBundle> {
    let y_var = ds.variable(name)?;
    let f_var = ds.variable(&spec.factor)?;
    let super::anova_oneway::Split {
        groups,
        levels,
        tally,
        total,
    } = super::anova_oneway::split(ds, name, &spec.factor)?;
    let analyzed: usize = groups.iter().map(Vec::len).sum();
    let note = format!(
        "Kruskal-Wallis rank-sum test with the correction for ties. Effect size: epsilon squared (H / (N − 1)). {}Groups are the categories of {} with data. Computed by PrismStatistics.",
        if spec.post_hoc == KruskalPostHoc::Dunn {
            match spec.dunn_adjust {
                DunnAdjust::None => "Pairs of groups compared by Dunn's test (with the tie correction), p values not adjusted. ".to_string(),
                a => format!("Pairs of groups compared by Dunn's test (with the tie correction), p values adjusted by {}'s method. ", a.name()),
            }
        } else {
            String::new()
        },
        f_var.display_name()
    );
    let mut b = common::bundle(
        KRUSKAL,
        METHOD_VERSION,
        format!(
            "Kruskal-Wallis Test: {} by {}",
            y_var.display_name(),
            f_var.display_name()
        ),
        serde_json::to_value(spec).expect("a spec always serializes"),
        tally.accounting(total, analyzed),
        note,
    );
    set_text(&mut b, "y.label", y_var.display_name());
    set_text(&mut b, "factor.label", f_var.display_name());
    set(&mut b, "k", groups.len() as f64);
    set(&mut b, "n", analyzed as f64);
    let kw = if groups.len() >= 2 {
        prismqda_stats::nonparametric::kruskal_wallis(&groups)
    } else {
        None
    };
    b.tables.push(Table {
        id: "ranks".into(),
        title: "Ranks".into(),
        columns: vec![
            f_var.display_name().into(),
            "N".into(),
            "Median".into(),
            "Mean rank".into(),
        ],
        rows: levels
            .iter()
            .zip(&groups)
            .enumerate()
            .map(|(i, (l, g))| {
                vec![
                    text(&l.label),
                    count(g.len()),
                    opt(median(g), 2),
                    opt(kw.as_ref().map(|k| k.groups[i].mean_rank), 2),
                ]
            })
            .collect(),
        footnotes: Vec::new(),
    });
    for (i, (l, g)) in levels.iter().zip(&groups).enumerate() {
        set_text(&mut b, format!("group.{}.label", i + 1), l.label.clone());
        set(&mut b, format!("group.{}.n", i + 1), g.len() as f64);
        if let Some(m) = median(g) {
            set(&mut b, format!("group.{}.median", i + 1), m);
        }
    }
    let Some(kw) = kw else {
        fail(
            &mut b,
            if groups.len() < 2 {
                format!(
                    "The test needs at least two groups with data; {} has {}.",
                    f_var.display_name(),
                    groups.len()
                )
            } else {
                "Every value is the same, so there is nothing to rank.".to_string()
            },
        );
        return Ok(b);
    };
    for (k, v) in [
        ("H", kw.h),
        ("df", kw.df),
        ("p", kw.p),
        ("epsilon2", kw.epsilon_squared),
        ("eta2_h", kw.eta_squared_h),
    ] {
        set(&mut b, k, v);
    }
    b.tables.push(Table {
        id: "test".into(),
        title: "Kruskal-Wallis test".into(),
        columns: vec![
            "Dependent variable".into(),
            "H".into(),
            "df".into(),
            "p".into(),
            "Epsilon squared (ε²)".into(),
        ],
        rows: vec![vec![
            text(y_var.display_name()),
            num(kw.h, 3),
            num(kw.df, 0),
            p(kw.p),
            num(kw.epsilon_squared, 3),
        ]],
        footnotes: vec!["H is compared with the chi-square distribution.".into()],
    });
    if spec.post_hoc == KruskalPostHoc::Dunn {
        let pairs =
            prismqda_stats::nonparametric::dunn(&kw, match spec.dunn_adjust {
                DunnAdjust::Holm => prismqda_stats::adjust::Adjustment::Holm,
                DunnAdjust::Bonferroni => prismqda_stats::adjust::Adjustment::Bonferroni,
                DunnAdjust::None => prismqda_stats::adjust::Adjustment::None,
            });
        set_text(&mut b, "dunn_adjust", spec.dunn_adjust.name());
        let mut rows = Vec::new();
        for (i, d) in pairs.iter().enumerate() {
            let key = format!("pair.{}", i + 1);
            set_text(&mut b, format!("{key}.a"), levels[d.a].label.clone());
            set_text(&mut b, format!("{key}.b"), levels[d.b].label.clone());
            set(&mut b, format!("{key}.z"), d.z);
            set(&mut b, format!("{key}.p"), d.p_adjusted);
            rows.push(vec![
                text(&levels[d.a].label),
                text(&levels[d.b].label),
                num(d.rank_difference, 2),
                num(d.z, 3),
                p(d.p),
                p(d.p_adjusted),
            ]);
        }
        b.tables.push(Table {
            id: "post_hoc".into(),
            title: "Pairwise comparisons (Dunn)".into(),
            columns: vec![
                "Group (I)".into(),
                "Group (J)".into(),
                "Mean rank difference (I − J)".into(),
                "z".into(),
                "p".into(),
                format!("p ({})", if spec.dunn_adjust == DunnAdjust::None { "unadjusted" } else { spec.dunn_adjust.name() }),
            ],
            rows,
            footnotes: vec![match spec.dunn_adjust {
                DunnAdjust::None => format!("p values are not adjusted for the {} comparisons.", pairs.len()),
                a => format!("{}'s adjustment for the {} comparisons.", a.name(), pairs.len()),
            }],
        });
    }
    let smallest = groups.iter().map(Vec::len).min().unwrap_or(0);
    b.diagnostics.push(size_diagnostic(
        smallest,
        format!(
            "The smallest group has {smallest} case{}",
            if smallest == 1 { "" } else { "s" }
        ),
    ));
    b.diagnostics.push(not_assessed("shape", "Shape of the groups", "The test compares whole distributions. Read it as a difference in medians only if the groups have a similar shape and spread; look at a boxplot of each."));
    Ok(b)
}
