//! Bootstrap intervals (`bootstrap`, checklist A13): percentile and BCa intervals for means, medians, 5% trimmed means and
//! standard deviations of variables, for the correlations among variables, and for the coefficients of a linear
//! regression. Cases are drawn with replacement from a seeded generator, so the same seed gives the same intervals on every
//! computer. Computed by `prismqda_stats::bootstrap`, whose interval arithmetic follows R's `boot` package.

use prismqda_stats::bootstrap::{bootstrap, bootstrap_many, BootstrapInterval, BootstrapSpec};
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Cell, ResultBundle, Table};

pub const METHOD_ID: &str = "bootstrap";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Statistic {
    #[default]
    Mean,
    Median,
    TrimmedMean,
    Sd,
    Correlation,
    Regression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Coefficient {
    #[default]
    Pearson,
    Spearman,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    #[serde(default)]
    pub statistic: Statistic,
    /// The variables described or correlated (or, for a regression, the predictors).
    pub variables: Vec<String>,
    /// The outcome of a regression.
    #[serde(default)]
    pub dependent: Option<String>,
    #[serde(default)]
    pub coefficient: Coefficient,
    #[serde(default = "default_resamples")]
    pub resamples: usize,
    #[serde(default = "default_seed")]
    pub seed: u64,
    #[serde(default = "default_level")]
    pub level: f64,
}

fn default_resamples() -> usize {
    2000
}

fn default_seed() -> u64 {
    20_260_101
}

fn default_level() -> f64 {
    0.95
}

fn mean(x: &[f64]) -> f64 {
    x.iter().sum::<f64>() / x.len() as f64
}

fn median_of(mut x: Vec<f64>) -> f64 {
    x.sort_by(|a, b| a.total_cmp(b));
    let n = x.len();
    if n % 2 == 1 { x[n / 2] } else { (x[n / 2 - 1] + x[n / 2]) / 2.0 }
}

/// R's `mean(x, trim = 0.05)`: drop floor(0.05 n) values from each end.
fn trimmed_of(mut x: Vec<f64>) -> f64 {
    x.sort_by(|a, b| a.total_cmp(b));
    let g = (0.05 * x.len() as f64).floor() as usize;
    mean(&x[g..x.len() - g])
}

fn sd_of(x: &[f64]) -> Option<f64> {
    if x.len() < 2 {
        return None;
    }
    let m = mean(x);
    Some((x.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (x.len() - 1) as f64).sqrt())
}

fn pearson(x: &[f64], y: &[f64]) -> Option<f64> {
    let (mx, my) = (mean(x), mean(y));
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (a, b) in x.iter().zip(y) {
        sxy += (a - mx) * (b - my);
        sxx += (a - mx).powi(2);
        syy += (b - my).powi(2);
    }
    (sxx > 0.0 && syy > 0.0).then(|| sxy / (sxx * syy).sqrt())
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    if spec.variables.is_empty() {
        return Err(Error::Invalid("Choose at least one variable.".into()));
    }
    if spec.statistic == Statistic::Correlation && spec.variables.len() < 2 {
        return Err(Error::Invalid("Choose at least two variables to correlate.".into()));
    }
    if spec.statistic == Statistic::Regression && spec.dependent.is_none() {
        return Err(Error::Invalid("Choose the dependent variable of the regression.".into()));
    }
    if !(100..=100_000).contains(&spec.resamples) {
        return Err(Error::Invalid("The number of resamples must be between 100 and 100,000.".into()));
    }
    check_level(spec.level)?;
    let boot = BootstrapSpec { resamples: spec.resamples, seed: spec.seed, level: spec.level };
    let lv = level_text(spec.level);
    let label = |n: &str| ds.variable(n).map(|v| v.display_name().to_string());
    let labels: Vec<String> = spec.variables.iter().map(|n| label(n)).collect::<Result<_>>()?;
    let what = match spec.statistic {
        Statistic::Mean => "means",
        Statistic::Median => "medians",
        Statistic::TrimmedMean => "5% trimmed means",
        Statistic::Sd => "standard deviations",
        Statistic::Correlation => match spec.coefficient {
            Coefficient::Pearson => "Pearson correlations",
            Coefficient::Spearman => "Spearman correlations",
        },
        Statistic::Regression => "regression coefficients",
    };
    let note = format!(
        "Bootstrap of the {what}: {} resamples of the cases drawn with replacement (seed {}), with {lv} percentile and bias-corrected and accelerated (BCa) intervals; BCa uses the jackknife for its acceleration. {}Computed by PrismStatistics.",
        spec.resamples,
        spec.seed,
        match spec.statistic {
            Statistic::Mean | Statistic::Median | Statistic::TrimmedMean | Statistic::Sd => "Each variable uses its own valid values. ",
            _ => "Cases were used when every variable had a valid value. ",
        }
    );

    // The rows: a name, the interval, and a field prefix.
    let mut rows: Vec<(String, String, Option<BootstrapInterval>)> = Vec::new();
    let cases;
    match spec.statistic {
        Statistic::Mean | Statistic::Median | Statistic::TrimmedMean | Statistic::Sd => {
            let mut total = 0;
            let mut tally = Tally::default();
            let mut analyzed = 0;
            for (i, (name, l)) in spec.variables.iter().zip(&labels).enumerate() {
                let cells = numeric(ds, name)?;
                total = cells.len();
                let x: Vec<f64> = cells.iter().filter(|c| !tally.check(*c, name)).filter_map(Slot::value).collect();
                analyzed = analyzed.max(x.len());
                let stat = spec.statistic;
                let f = |idx: &[usize]| -> Option<f64> {
                    let v: Vec<f64> = idx.iter().map(|&j| x[j]).collect();
                    match stat {
                        Statistic::Mean => Some(mean(&v)),
                        Statistic::Median => Some(median_of(v)),
                        Statistic::TrimmedMean => Some(trimmed_of(v)),
                        _ => sd_of(&v),
                    }
                };
                rows.push((l.clone(), format!("var.{}", i + 1), bootstrap(x.len(), &boot, f)));
            }
            cases = tally.accounting(total, analyzed);
        }
        Statistic::Correlation | Statistic::Regression => {
            let names: Vec<&String> = spec.dependent.iter().chain(spec.variables.iter()).collect();
            for (i, a) in names.iter().enumerate() {
                if names[..i].iter().any(|b| ds.find(a) == ds.find(b)) {
                    return Err(Error::Invalid(format!("{a} is used more than once.")));
                }
            }
            let cells: Vec<Vec<Slot<f64>>> = names.iter().map(|n| numeric(ds, n)).collect::<Result<_>>()?;
            let total = cells[0].len();
            let mut tally = Tally::default();
            let used: Vec<usize> = (0..total).filter(|&r| !cells.iter().zip(&names).any(|(c, n)| tally.check(&c[r], n))).collect();
            let cols: Vec<Vec<f64>> = cells.iter().map(|c| used.iter().map(|&r| c[r].value().expect("checked")).collect()).collect();
            let n = used.len();
            cases = tally.accounting(total, n);
            if spec.statistic == Statistic::Correlation {
                let cols: Vec<Vec<f64>> = if spec.coefficient == Coefficient::Spearman { cols.iter().map(|c| prismqda_stats::rank::rank_average(c)).collect() } else { cols };
                let spearman = spec.coefficient == Coefficient::Spearman;
                let pairs: Vec<(usize, usize)> = (0..cols.len()).flat_map(|i| ((i + 1)..cols.len()).map(move |j| (i, j))).collect();
                let f = |idx: &[usize]| -> Option<Vec<f64>> {
                    pairs
                        .iter()
                        .map(|&(i, j)| {
                            let x: Vec<f64> = idx.iter().map(|&r| cols[i][r]).collect();
                            let y: Vec<f64> = idx.iter().map(|&r| cols[j][r]).collect();
                            // Spearman's rho of a resample re-ranks the values drawn.
                            if spearman {
                                pearson(&prismqda_stats::rank::rank_average(&x), &prismqda_stats::rank::rank_average(&y))
                            } else {
                                pearson(&x, &y)
                            }
                        })
                        .collect()
                };
                let res = bootstrap_many(n, pairs.len(), &boot, f);
                for ((i, j), r) in pairs.iter().zip(res) {
                    rows.push((format!("{} & {}", labels[*i], labels[*j]), format!("pair.{}.{}", i + 1, j + 1), r));
                }
            } else {
                let k = cols.len() - 1;
                let f = |idx: &[usize]| -> Option<Vec<f64>> {
                    let y: Vec<f64> = idx.iter().map(|&r| cols[0][r]).collect();
                    let xs: Vec<Vec<f64>> = cols[1..].iter().map(|c| idx.iter().map(|&r| c[r]).collect()).collect();
                    prismqda_stats::regression::linear(&y, &xs, 0.95).ok().map(|m| m.terms.iter().map(|t| t.estimate).collect())
                };
                let res = bootstrap_many(n, k + 1, &boot, f);
                for (t, r) in res.into_iter().enumerate() {
                    let name = if t == 0 { "(Constant)".to_string() } else { labels[t - 1].clone() };
                    rows.push((name, format!("term.{t}"), r));
                }
            }
        }
    }

    let title = match (spec.statistic, &spec.dependent) {
        (Statistic::Regression, Some(d)) => format!("Bootstrap: Regression of {}", label(d)?),
        _ => format!("Bootstrap: {}", what[..1].to_uppercase() + &what[1..]),
    };
    let mut b = common::bundle(METHOD_ID, METHOD_VERSION, title, serde_json::to_value(spec).expect("a spec always serializes"), cases, note);
    set_text(&mut b, "statistic", serde_json::to_value(spec.statistic).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default());
    set_text(&mut b, "what", what);
    set(&mut b, "level", spec.level);
    set(&mut b, "resamples", spec.resamples as f64);
    set(&mut b, "seed", spec.seed as f64);
    set(&mut b, "rows", rows.len() as f64);
    if let Some(d) = &spec.dependent {
        set_text(&mut b, "y.label", label(d)?);
    }
    let mut table = Vec::new();
    let mut failed = Vec::new();
    for (i, (name, key, r)) in rows.iter().enumerate() {
        set_text(&mut b, format!("row.{}.label", i + 1), name.clone());
        set_text(&mut b, format!("row.{}.key", i + 1), key.clone());
        let Some(r) = r else {
            failed.push(name.clone());
            let mut row = vec![text(name)];
            row.extend(std::iter::repeat_n(Cell::Empty, 8));
            table.push(row);
            continue;
        };
        let k = format!("row.{}", i + 1);
        for (f, v) in [("estimate", r.estimate), ("bias", r.bias), ("se", r.se), ("pct.lower", r.percentile.0), ("pct.upper", r.percentile.1), ("used", r.used as f64)] {
            set(&mut b, format!("{k}.{f}"), v);
        }
        if let Some((lo, hi)) = r.bca {
            set(&mut b, format!("{k}.bca.lower"), lo);
            set(&mut b, format!("{k}.bca.upper"), hi);
        }
        table.push(vec![
            text(name),
            num(r.estimate, 3),
            num(r.bias, 3),
            num(r.se, 3),
            num(r.percentile.0, 3),
            num(r.percentile.1, 3),
            opt(r.bca.map(|v| v.0), 3),
            opt(r.bca.map(|v| v.1), 3),
            count(r.used),
        ]);
    }
    let mut footnotes = vec![format!("Bias is the mean of the resampled values minus the estimate; SE is their standard deviation. Seed {} repeats these values exactly.", spec.seed)];
    if !failed.is_empty() {
        footnotes.push(format!("Not computed for {}: the statistic is undefined for these cases or for most resamples.", failed.join("; ")));
    }
    b.tables.push(Table {
        id: "bootstrap".into(),
        title: format!("Bootstrap {what}"),
        columns: vec![
            if spec.statistic == Statistic::Regression { "Term".into() } else if spec.statistic == Statistic::Correlation { "Pair".into() } else { "Variable".into() },
            "Estimate".into(),
            "Bias".into(),
            "SE".into(),
            format!("{lv} percentile lower"),
            format!("{lv} percentile upper"),
            format!("{lv} BCa lower"),
            format!("{lv} BCa upper"),
            "Resamples used".into(),
        ],
        rows: table,
        footnotes,
    });
    if failed.len() == rows.len() {
        fail(&mut b, "No bootstrap interval could be computed: the statistic is undefined for these cases or for most resamples (each needs at least three cases).");
        return Ok(b);
    }
    b.diagnostics.push(not_assessed(
        "resamples",
        "Number of resamples",
        &format!("{} resamples were drawn. For intervals stable to two decimals, 2,000 or more is usual; run again with another seed to see how much they move.", spec.resamples),
    ));
    Ok(b)
}
