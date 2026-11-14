//! Bivariate correlation (`corr.bivariate`, task T3-13).
//!
//! Defaults (05 §1): Pearson's r (Spearman's rho and Kendall's tau-b on request), listwise deletion (pairwise on
//! request, with the N of each pair shown), two-sided tests, and a Fisher-z confidence interval for Pearson's r. A
//! constant variable gives an undefined correlation with the reason, never a number. Computed by
//! `prismqda_stats::correlation`.

use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Cell, ResultBundle, Table};

pub const METHOD_ID: &str = "corr.bivariate";
pub const METHOD_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Pearson,
    Spearman,
    Kendall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Missing {
    #[default]
    Listwise,
    Pairwise,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub variables: Vec<String>,
    /// One result per coefficient.
    #[serde(default = "default_kinds")]
    pub coefficients: Vec<Kind>,
    #[serde(default)]
    pub missing: Missing,
    /// Two-sided, or one-sided ("greater": a positive correlation).
    #[serde(default)]
    pub alternative: super::sided::Alternative,
    /// Adjustment of the p values for the number of pairs.
    #[serde(default)]
    pub adjust: Adjust,
    /// Bayes factors for Pearson correlations (stretched beta prior, width 1/3 unless given).
    #[serde(default)]
    pub bayes: bool,
    #[serde(default)]
    pub prior_scale: Option<f64>,
    #[serde(default = "default_level")]
    pub level: f64,
}

/// How the p values of the pairs are adjusted for their number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Adjust {
    #[default]
    None,
    Bonferroni,
    Holm,
}

impl Adjust {
    fn name(self) -> &'static str {
        match self {
            Adjust::None => "none",
            Adjust::Bonferroni => "Bonferroni",
            Adjust::Holm => "Holm",
        }
    }
}

fn default_kinds() -> Vec<Kind> {
    vec![Kind::Pearson]
}

fn default_level() -> f64 {
    0.95
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Pearson => "Pearson's r",
            Kind::Spearman => "Spearman's rho",
            Kind::Kendall => "Kendall's tau-b",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Kind::Pearson => "pearson",
            Kind::Spearman => "spearman",
            Kind::Kendall => "kendall",
        }
    }
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<Vec<ResultBundle>> {
    if spec.variables.len() < 2 {
        return Err(Error::Invalid("Choose at least two variables.".into()));
    }
    if spec.coefficients.is_empty() {
        return Err(Error::Invalid(
            "Choose at least one correlation coefficient.".into(),
        ));
    }
    for (i, a) in spec.variables.iter().enumerate() {
        if spec.variables[..i].iter().any(|b| ds.find(a) == ds.find(b)) {
            return Err(Error::Invalid(format!("{a} is listed twice.")));
        }
    }
    check_level(spec.level)?;
    let cells: Vec<Vec<Slot<f64>>> = spec
        .variables
        .iter()
        .map(|n| numeric(ds, n))
        .collect::<Result<_>>()?;
    spec.coefficients
        .iter()
        .map(|k| run_one(ds, spec, *k, &cells))
        .collect()
}

fn run_one(
    ds: &Dataset,
    spec: &Spec,
    kind: Kind,
    cells: &[Vec<Slot<f64>>],
) -> Result<ResultBundle> {
    let labels: Vec<String> = spec
        .variables
        .iter()
        .map(|n| ds.variable(n).map(|v| v.display_name().to_string()))
        .collect::<Result<_>>()?;
    let total = cells[0].len();
    // Listwise: the cases with every variable valid. The tally names the first variable that is missing.
    let mut tally = Tally::default();
    let complete: Vec<bool> = (0..total)
        .map(|r| {
            for (i, c) in cells.iter().enumerate() {
                if !matches!(c[r], Slot::Valid(_)) {
                    tally.add(format!(
                        "{} {}",
                        spec.variables[i],
                        if c[r] == Slot::Empty {
                            "empty"
                        } else {
                            "user-missing"
                        }
                    ));
                    return false;
                }
            }
            true
        })
        .collect();
    let listwise_n = complete.iter().filter(|c| **c).count();
    let cases = match spec.missing {
        Missing::Listwise => tally.accounting(total, listwise_n),
        // With pairwise deletion each pair has its own N, shown in the table; the count here is the cases with any pair.
        Missing::Pairwise => {
            let any = (0..total)
                .filter(|&r| {
                    cells
                        .iter()
                        .filter(|c| matches!(c[r], Slot::Valid(_)))
                        .count()
                        >= 2
                })
                .count();
            let mut t = Tally::default();
            for _ in 0..(total - any) {
                t.add("fewer than two variables with values");
            }
            t.accounting(total, any)
        }
    };
    let lv = level_text(spec.level);
    let note = format!(
        "{}, {} tests{}{}. Missing values: {}. Computed by prismqda-stats {}.",
        kind.name(),
        spec.alternative.words(),
        match spec.adjust {
            Adjust::None => String::new(),
            a => format!(", p values adjusted for the number of pairs by {}'s method", a.name()),
        },
        match kind {
            Kind::Pearson => format!(", {lv} confidence intervals from Fisher's z transformation"),
            Kind::Spearman => ", with ties given average ranks; the p value is exact (Edgeworth series for 10 or more pairs) when there are no ties and the t approximation otherwise".into(),
            Kind::Kendall => ", which corrects for ties; the p value is exact below 50 pairs without ties and the normal approximation otherwise".into(),
        },
        match spec.missing {
            Missing::Listwise => "listwise (only cases with every variable valid are used, so every coefficient uses the same cases)",
            Missing::Pairwise => "pairwise (each pair of variables uses every case valid on both, so N can differ between pairs)",
        },
        super::STATS_VERSION
    );
    let title = format!("Correlations ({})", kind.name());
    let mut b = common::bundle(
        METHOD_ID,
        METHOD_VERSION,
        title,
        serde_json::to_value(spec).expect("a spec always serializes"),
        cases,
        note,
    );
    set_text(&mut b, "coefficient", kind.key());
    set_text(&mut b, "alternative", spec.alternative.key());
    set_text(&mut b, "adjust", spec.adjust.name());
    set(&mut b, "level", spec.level);
    let k = spec.variables.len();
    for (i, l) in labels.iter().enumerate() {
        set_text(&mut b, format!("var.{}.label", i + 1), l.clone());
    }

    // Every pair once.
    let mut bf_rows = Vec::new();
    let mut results = vec![vec![None; k]; k];
    let mut pair_rows = Vec::new();
    let mut undefined = Vec::new();
    for i in 0..k {
        for j in (i + 1)..k {
            let (x, y): (Vec<f64>, Vec<f64>) = (0..total)
                .filter(|&r| match spec.missing {
                    Missing::Listwise => complete[r],
                    Missing::Pairwise => {
                        matches!((cells[i][r], cells[j][r]), (Slot::Valid(_), Slot::Valid(_)))
                    }
                })
                .map(|r| {
                    (
                        cells[i][r].value().expect("valid"),
                        cells[j][r].value().expect("valid"),
                    )
                })
                .unzip();
            let mut c = match kind {
                Kind::Pearson => prismqda_stats::correlation::pearson(&x, &y, spec.level),
                Kind::Spearman => prismqda_stats::correlation::spearman(&x, &y, spec.level),
                Kind::Kendall => prismqda_stats::correlation::kendall(&x, &y, spec.level),
            };
            if let Some(c) = c.as_mut().filter(|_| !spec.alternative.is_two_sided()) {
                one_sided(c, spec.alternative);
            }
            let key = format!("pair.{}.{}", i + 1, j + 1);
            if let (true, Kind::Pearson, Some(cc)) = (spec.bayes, kind, c.as_ref()) {
                let kappa = spec.prior_scale.unwrap_or(1.0 / 3.0);
                let bf = super::bayes::correlation_log_bf(cc.n as f64, cc.r, kappa).exp();
                set(&mut b, format!("{key}.bf10"), bf);
                bf_rows.push(vec![text(format!("{} & {}", labels[i], labels[j])), num(cc.r, 3), num(bf, 3), num(1.0 / bf, 3), text(super::bayes::describe(bf))]);
            }
            set(&mut b, format!("{key}.n"), x.len() as f64);
            match &c {
                Some(c) => {
                    set(&mut b, format!("{key}.r"), c.r);
                    set(&mut b, format!("{key}.p"), c.p);
                    if let Some(df) = c.df {
                        set(&mut b, format!("{key}.df"), df);
                    }
                    if let Some((lo, hi)) = c.ci {
                        set(&mut b, format!("{key}.ci.lower"), lo);
                        set(&mut b, format!("{key}.ci.upper"), hi);
                    }
                    let mut row = vec![
                        text(format!("{} & {}", labels[i], labels[j])),
                        count(c.n),
                        num(c.r, 3),
                    ];
                    if kind == Kind::Pearson {
                        row.push(opt(c.ci.map(|v| v.0), 3));
                        row.push(opt(c.ci.map(|v| v.1), 3));
                    }
                    row.push(p(c.p));
                    pair_rows.push(row);
                }
                None => {
                    undefined.push(format!("{} & {}", labels[i], labels[j]));
                    let mut row = vec![
                        text(format!("{} & {}", labels[i], labels[j])),
                        count(x.len()),
                        Cell::Empty,
                    ];
                    if kind == Kind::Pearson {
                        row.extend([Cell::Empty, Cell::Empty]);
                    }
                    row.push(Cell::Empty);
                    pair_rows.push(row);
                }
            }
            results[i][j] = Some((c, x.len()));
        }
    }

    // Adjusted p values across the pairs that were computed.
    let adjusted: Vec<Vec<Option<f64>>> = {
        let flat: Vec<f64> = results
            .iter()
            .enumerate()
            .flat_map(|(i, row)| row.iter().skip(i + 1))
            .map(|cell| cell.as_ref().and_then(|(c, _)| c.as_ref()).map_or(f64::NAN, |c| c.p))
            .collect();
        let method = match spec.adjust {
            Adjust::None => prismqda_stats::adjust::Adjustment::None,
            Adjust::Bonferroni => prismqda_stats::adjust::Adjustment::Bonferroni,
            Adjust::Holm => prismqda_stats::adjust::Adjustment::Holm,
        };
        let adj = prismqda_stats::adjust::p_adjust(&flat, method);
        let mut out = vec![vec![None; k]; k];
        let mut idx = 0;
        for (i, row) in out.iter_mut().enumerate() {
            for cell in row.iter_mut().skip(i + 1) {
                *cell = adj[idx].is_finite().then_some(adj[idx]);
                idx += 1;
            }
        }
        out
    };
    if spec.adjust != Adjust::None {
        let pairs = adjusted.iter().enumerate().flat_map(|(i, row)| row.iter().enumerate().skip(i + 1).map(move |(j, v)| (i, j, *v)));
        for ((i, j, v), row) in pairs.zip(pair_rows.iter_mut()) {
            if let Some(v) = v {
                set(&mut b, format!("pair.{}.{}.p_adj", i + 1, j + 1), v);
            }
            row.push(v.map_or(Cell::Empty, p));
        }
    }
    let p_label = match spec.adjust {
        Adjust::None => format!("p ({})", spec.alternative.words()),
        a => format!("p ({}, {}-adjusted)", spec.alternative.words(), a.name()),
    };

    let adjusted_at = |i: usize, j: usize| if i < j { adjusted[i][j] } else { adjusted[j][i] };

    // The matrix, with r, p and N under each variable, as menu-driven packages show it.
    let sym = match kind {
        Kind::Pearson => "r",
        Kind::Spearman => "rho",
        Kind::Kendall => "tau-b",
    };
    let mut columns = vec![String::new(), String::new()];
    columns.extend(labels.iter().cloned());
    let mut rows = Vec::new();
    for i in 0..k {
        let get = |j: usize| {
            if i < j {
                results[i][j].as_ref()
            } else {
                results[j][i].as_ref()
            }
        };
        let mut r_row = vec![text(&labels[i]), text(sym)];
        let mut p_row = vec![Cell::Empty, text(p_label.clone())];
        let mut n_row = vec![Cell::Empty, text("N")];
        for j in 0..k {
            if i == j {
                r_row.push(num(1.0, 3));
                p_row.push(Cell::Empty);
                n_row.push(Cell::Empty);
                continue;
            }
            let (c, n) = get(j).expect("every pair was computed");
            r_row.push(c.as_ref().map_or(Cell::Empty, |c| num(c.r, 3)));
            let shown = if spec.adjust == Adjust::None { c.as_ref().map(|c| c.p) } else { adjusted_at(i, j) };
            p_row.push(shown.map_or(Cell::Empty, p));
            n_row.push(count(*n));
        }
        rows.push(r_row);
        rows.push(p_row);
        rows.push(n_row);
    }
    let mut foot = Vec::new();
    if !undefined.is_empty() {
        foot.push(format!("Not computed for {}: a variable has no variation among the cases used, or there are fewer than three pairs.", undefined.join("; ")));
    }
    b.tables.push(Table {
        id: "matrix".into(),
        title: "Correlation matrix".into(),
        columns,
        rows,
        footnotes: foot.clone(),
    });
    let mut pcols = vec!["Pair".into(), "N".into(), sym.into()];
    if kind == Kind::Pearson {
        pcols.push(format!("{lv} CI lower"));
        pcols.push(format!("{lv} CI upper"));
    }
    pcols.push(format!("p ({})", spec.alternative.words()));
    if spec.adjust != Adjust::None {
        pcols.push(format!("p ({}-adjusted)", spec.adjust.name()));
    }
    b.tables.push(Table {
        id: "pairs".into(),
        title: "Pairs".into(),
        columns: pcols,
        rows: pair_rows,
        footnotes: foot,
    });

    if !bf_rows.is_empty() {
        let kappa = spec.prior_scale.unwrap_or(1.0 / 3.0);
        set_text(&mut b, "bf.prior", format!("stretched beta, width {}", crate::apa::num(kappa, 3)));
        b.tables.push(Table {
            id: "bayes".into(),
            title: "Bayes factors".into(),
            columns: vec!["Pair".into(), "r".into(), "BF₁₀".into(), "BF₀₁".into(), "Evidence".into()],
            rows: bf_rows,
            footnotes: vec![format!("Prior on the correlation: a beta distribution stretched over −1 to 1 with width {} (R's BayesFactor default).", crate::apa::num(kappa, 3))],
        });
    }
    let smallest = results
        .iter()
        .flatten()
        .flatten()
        .map(|(_, n)| *n)
        .min()
        .unwrap_or(0);
    if undefined.len() == k * (k - 1) / 2 {
        fail(&mut b, format!("No correlation could be computed. Each pair needs at least three cases with both values ({smallest} here) and some variation in both variables."));
    }
    b.diagnostics.push(size_diagnostic(
        smallest,
        format!("The smallest N for a pair is {smallest}"),
    ));
    b.diagnostics.push(not_assessed(
        "shape",
        "Linearity and outliers",
        match kind {
            Kind::Pearson => "Pearson's r measures straight-line association and is pulled by outliers. Look at the scatterplot of each pair.",
            _ => "Rank correlations measure whether one variable tends to rise with the other. Look at the scatterplot of each pair.",
        },
    ));
    Ok(b)
}

/// A correlation's test made one-sided ("greater": the correlation is positive), as R's `cor.test` does it.
fn one_sided(c: &mut prismqda_stats::correlation::Correlation, alt: super::sided::Alternative) {
    use super::sided;
    use prismqda_stats::correlation::CorrelationKind;
    c.p = match (c.kind, c.statistic, c.df) {
        (CorrelationKind::Spearman, _, _) if c.exact || c.edgeworth => sided::spearman_exact_p(c.n, c.r, alt),
        (CorrelationKind::Kendall, _, _) if c.exact => sided::kendall_exact_p(c.n, c.r, alt),
        (CorrelationKind::Kendall, Some(z), _) => sided::z_p(z, c.p, alt),
        (_, Some(t), Some(df)) => sided::t_p(t, df, c.p, alt),
        // A perfect Pearson correlation: the two-sided p is already 0; the opposite direction is 1.
        _ => {
            if (c.r > 0.0) == (alt == sided::Alternative::Greater) {
                c.p
            } else {
                1.0
            }
        }
    };
    if c.kind == CorrelationKind::Pearson {
        c.ci = sided::pearson_interval(c.r, c.n, c.level, alt);
    }
}
