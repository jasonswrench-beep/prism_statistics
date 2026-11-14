//! Survey design (checklist B13): estimates that account for sampling weights, strata and clusters, as R's survey
//! package computes them.
//!
//! * Survey estimates (`survey.estimates`): weighted means, totals, and the proportions of a categorical variable's
//!   categories.
//! * Survey regression (`survey.regression`): linear regression by weighted least squares with design-based standard
//!   errors (as svyglm).
//!
//! Variances are by linearization with the "ultimate cluster" estimator: each estimate's influence values are summed
//! within each cluster (PSU), and the PSU totals vary around their stratum mean, V = Σₕ nₕ/(nₕ−1) Σᵢ (zₕᵢ − z̄ₕ)². With no
//! clusters each case is its own PSU; with no strata there is one stratum. Sampling is treated as with replacement at
//! the first stage (no finite population correction). Tests use the design's degrees of freedom: PSUs minus strata.

use prismqda_stats::dist::{norm_quantile, t_quantile, t_two_sided};
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use super::linalg::{invert, Mat};
use super::regression::{self, Factor};
use crate::apa;
use crate::error::{Error, Result};
use crate::model::{Column, Dataset};
use crate::result::{ResultBundle, Table};

pub const ESTIMATES: &str = "survey.estimates";
pub const REGRESSION: &str = "survey.regression";
pub const METHOD_VERSION: u32 = 1;

fn default_level() -> f64 {
    0.95
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Design {
    pub weight: String,
    #[serde(default)]
    pub strata: Option<String>,
    #[serde(default)]
    pub cluster: Option<String>,
}

/// Each case's weight, stratum and PSU (as category indices), for the cases with them valid.
struct DesignRows {
    rows: Vec<usize>,
    w: Vec<f64>,
    stratum: Vec<usize>,
    psu: Vec<usize>,
    n_strata: usize,
    n_psu: usize,
}

fn design_rows(ds: &Dataset, d: &Design, tally: &mut Tally, candidates: &[usize]) -> Result<DesignRows> {
    let wc = numeric(ds, &d.weight)?;
    let st = d.strata.as_ref().map(|s| categories(ds, s)).transpose()?;
    let cl = d.cluster.as_ref().map(|c| categories(ds, c)).transpose()?;
    let (mut rows, mut w, mut stratum, mut psu_raw) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for &r in candidates {
        if tally.check(&wc[r], "weight") {
            continue;
        }
        let wv = wc[r].value().expect("checked");
        if wv <= 0.0 {
            tally.add("weight zero or below");
            continue;
        }
        let s = match &st {
            Some((slots, _)) => {
                if tally.check(&slots[r], "stratum") {
                    continue;
                }
                slots[r].value().expect("checked")
            }
            None => 0,
        };
        let c = match &cl {
            Some((slots, _)) => {
                if tally.check(&slots[r], "cluster") {
                    continue;
                }
                Some(slots[r].value().expect("checked"))
            }
            None => None,
        };
        rows.push(r);
        w.push(wv);
        stratum.push(s);
        psu_raw.push(c.unwrap_or(rows.len() - 1 + 1_000_000));
    }
    // PSUs are nested in strata: a PSU is the pair (stratum, cluster).
    let mut keys: Vec<(usize, usize)> = stratum.iter().copied().zip(psu_raw.iter().copied()).collect();
    let mut uniq = keys.clone();
    uniq.sort_unstable();
    uniq.dedup();
    let psu: Vec<usize> = keys.iter_mut().map(|k| uniq.binary_search(k).expect("present")).collect();
    let mut strata_used = stratum.clone();
    strata_used.sort_unstable();
    strata_used.dedup();
    Ok(DesignRows { n_strata: strata_used.len(), n_psu: uniq.len(), rows, w, stratum, psu })
}

/// The ultimate-cluster covariance of estimators with influence values `z` (cases × parameters).
fn ultimate_cluster(z: &[Vec<f64>], d: &DesignRows) -> Option<Mat> {
    let k = z[0].len();
    // PSU totals and each PSU's stratum.
    let mut tot = vec![vec![0.0; k]; d.n_psu];
    let mut psu_stratum = vec![0usize; d.n_psu];
    for (i, zi) in z.iter().enumerate() {
        for (a, v) in zi.iter().enumerate() {
            tot[d.psu[i]][a] += v;
        }
        psu_stratum[d.psu[i]] = d.stratum[i];
    }
    let mut strata: Vec<usize> = psu_stratum.clone();
    strata.sort_unstable();
    strata.dedup();
    let mut v = vec![vec![0.0; k]; k];
    for s in strata {
        let members: Vec<usize> = (0..d.n_psu).filter(|&p| psu_stratum[p] == s).collect();
        let nh = members.len() as f64;
        if nh < 2.0 {
            return None;
        }
        let mean: Vec<f64> = (0..k).map(|a| members.iter().map(|&p| tot[p][a]).sum::<f64>() / nh).collect();
        for &p in &members {
            for a in 0..k {
                for c in 0..k {
                    v[a][c] += nh / (nh - 1.0) * (tot[p][a] - mean[a]) * (tot[p][c] - mean[c]);
                }
            }
        }
    }
    Some(v)
}

fn describe_design(ds: &Dataset, d: &Design, dr: &DesignRows) -> Result<String> {
    Ok(format!(
        "Weights: {}{}{}; {} PSUs in {} strat{} (design df = {}).",
        ds.variable(&d.weight)?.display_name(),
        d.strata.as_ref().map(|s| ds.variable(s).map(|v| format!("; strata: {}", v.display_name()))).transpose()?.unwrap_or_default(),
        d.cluster.as_ref().map(|c| ds.variable(c).map(|v| format!("; clusters (PSUs): {}", v.display_name()))).transpose()?.unwrap_or_default(),
        dr.n_psu,
        dr.n_strata,
        if dr.n_strata == 1 { "um" } else { "a" },
        dr.n_psu - dr.n_strata
    ))
}

const LONELY: &str = "A stratum has only one PSU, so its variance can't be estimated (R's survey package stops here too). Merge it with a similar stratum.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Statistic {
    #[default]
    Mean,
    Total,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EstimatesSpec {
    pub variables: Vec<String>,
    #[serde(flatten)]
    pub design: Design,
    #[serde(default)]
    pub statistic: Statistic,
    #[serde(default = "default_level")]
    pub level: f64,
}

pub fn estimates(ds: &Dataset, spec: &EstimatesSpec) -> Result<ResultBundle> {
    if spec.variables.is_empty() {
        return Err(Error::Invalid("Choose at least one variable.".into()));
    }
    check_level(spec.level)?;
    let total = ds.n_rows();
    let mut tally = Tally::default();
    let all: Vec<usize> = (0..total).collect();
    let dr = design_rows(ds, &spec.design, &mut tally, &all)?;
    let lv = level_text(spec.level);
    let what = if spec.statistic == Statistic::Mean { "means (and proportions)" } else { "totals" };
    let design_text = describe_design(ds, &spec.design, &dr)?;
    let note = format!("Design-based {what} with linearization standard errors (ultimate-cluster variance, with replacement at the first stage), as R's survey package; {lv} intervals use the normal distribution, as its confint. {design_text} Each variable uses the cases where it is valid. Computed by PrismStatistics.");
    let mut b = common::bundle(ESTIMATES, METHOD_VERSION, format!("Survey Estimates: {}", spec.variables.join(", ")), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, dr.rows.len()), note);
    set(&mut b, "level", spec.level);
    set(&mut b, "psu", dr.n_psu as f64);
    set(&mut b, "strata", dr.n_strata as f64);
    let z = norm_quantile(1.0 - (1.0 - spec.level) / 2.0);
    let mut rows = Vec::new();
    let mut k = 0;
    for name in &spec.variables {
        let var = ds.variable(name)?;
        let lab = var.display_name().to_string();
        let i = ds.find(name).expect("found");
        // Indicators for each category of a categorical variable; the value itself otherwise.
        let numeric_var = matches!(ds.columns[i], Column::Numeric(_)) && var.value_labels.is_empty();
        let (cols, names): (Vec<Vec<Option<f64>>>, Vec<String>) = if numeric_var {
            let c = numeric(ds, name)?;
            (vec![dr.rows.iter().map(|&r| c[r].value()).collect()], vec![lab.clone()])
        } else {
            let (slots, levels) = categories(ds, name)?;
            let cols = (0..levels.len()).map(|l| dr.rows.iter().map(|&r| slots[r].value().map(|v| if v == l { 1.0 } else { 0.0 })).collect()).collect();
            (cols, levels.iter().map(|l| format!("{lab}: {}", l.label)).collect())
        };
        // Cases where this variable is valid; a mean's influence is zero elsewhere (as survey's na.rm subsetting).
        let valid: Vec<bool> = cols[0].iter().map(Option::is_some).collect();
        let sw: f64 = dr.w.iter().zip(&valid).filter(|(_, v)| **v).map(|(w, _)| w).sum();
        for (col, nm) in cols.iter().zip(&names) {
            let y: Vec<f64> = col.iter().map(|v| v.unwrap_or(0.0)).collect();
            let est = if spec.statistic == Statistic::Mean {
                dr.w.iter().zip(&y).zip(&valid).filter(|(_, v)| **v).map(|((w, yv), _)| w * yv).sum::<f64>() / sw
            } else {
                dr.w.iter().zip(&y).zip(&valid).filter(|(_, v)| **v).map(|((w, yv), _)| w * yv).sum::<f64>()
            };
            let infl: Vec<Vec<f64>> = (0..dr.rows.len())
                .map(|i| vec![if !valid[i] { 0.0 } else if spec.statistic == Statistic::Mean { dr.w[i] * (y[i] - est) / sw } else { dr.w[i] * y[i] }])
                .collect();
            let Some(v) = ultimate_cluster(&infl, &dr) else {
                fail(&mut b, LONELY);
                return Ok(b);
            };
            let se = v[0][0].sqrt();
            k += 1;
            let key = format!("est.{k}");
            set_text(&mut b, format!("{key}.label"), nm.clone());
            for (f, val) in [("value", est), ("se", se), ("lower", est - z * se), ("upper", est + z * se)] {
                set(&mut b, format!("{key}.{f}"), val);
            }
            rows.push(vec![text(nm), num(est, 4), num(se, 4), num(est - z * se, 4), num(est + z * se, 4), count(valid.iter().filter(|v| **v).count())]);
        }
    }
    set(&mut b, "count", k as f64);
    b.tables.push(Table {
        id: "estimates".into(),
        title: if spec.statistic == Statistic::Mean { "Weighted means and proportions".into() } else { "Weighted totals".into() },
        columns: vec!["".into(), if spec.statistic == Statistic::Mean { "Estimate".into() } else { "Total".into() }, "SE".into(), format!("{lv} CI lower"), format!("{lv} CI upper"), "Cases".into()],
        rows,
        footnotes: vec![design_text],
    });
    Ok(b)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegressionSpec {
    pub dependent: String,
    #[serde(default)]
    pub covariates: Vec<String>,
    #[serde(default)]
    pub factors: Vec<Factor>,
    #[serde(flatten)]
    pub design: Design,
    #[serde(default = "default_level")]
    pub level: f64,
}

pub fn regression(ds: &Dataset, spec: &RegressionSpec) -> Result<ResultBundle> {
    check_level(spec.level)?;
    let reg = regression::Spec { dependent: spec.dependent.clone(), covariates: spec.covariates.clone(), factors: spec.factors.clone(), level: spec.level, blocks: Vec::new(), interactions: Vec::new(), center: false, plots: false, robust: None, bayes: false, prior_scale: None };
    let pr = regression::prepare(ds, &reg)?;
    let mut tally = pr.tally;
    let dr = design_rows(ds, &spec.design, &mut tally, &pr.used)?;
    let pos: Vec<usize> = dr.rows.iter().map(|r| pr.used.iter().position(|u| u == r).expect("used")).collect();
    let y: Vec<f64> = pos.iter().map(|&i| pr.y[i]).collect();
    let x: Vec<Vec<f64>> = pos.iter().map(|&i| std::iter::once(1.0).chain(pr.columns.iter().map(|c| c[i])).collect()).collect();
    let n = y.len();
    let p = x.first().map_or(1, Vec::len);
    let lv = level_text(spec.level);
    let design_text = describe_design(ds, &spec.design, &dr)?;
    let df = (dr.n_psu as f64 - dr.n_strata as f64) - p as f64 + 1.0;
    let note = format!("Survey-weighted linear regression (weighted least squares) with design-based standard errors by linearization, as R's svyglm; t tests and {lv} intervals on the design's residual degrees of freedom ({}). {design_text} Computed by PrismStatistics.", apa::df(df));
    let mut b = common::bundle(REGRESSION, METHOD_VERSION, format!("Survey Regression: {}", pr.y_label), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(pr.total, n), note);
    set_text(&mut b, "y.label", pr.y_label.clone());
    set(&mut b, "n", n as f64);
    set(&mut b, "df", df);
    set(&mut b, "level", spec.level);
    if n <= p || df < 1.0 {
        fail(&mut b, "There are too few cases or PSUs for this many predictors.");
        return Ok(b);
    }
    let mut xtwx = vec![vec![0.0; p]; p];
    let mut xtwy = vec![0.0; p];
    for i in 0..n {
        for a in 0..p {
            xtwy[a] += dr.w[i] * x[i][a] * y[i];
            for c in 0..p {
                xtwx[a][c] += dr.w[i] * x[i][a] * x[i][c];
            }
        }
    }
    let Some(inv) = invert(&xtwx) else {
        fail(&mut b, "The predictors are collinear.");
        return Ok(b);
    };
    let beta: Vec<f64> = inv.iter().map(|r| r.iter().zip(&xtwy).map(|(a, c)| a * c).sum()).collect();
    let infl: Vec<Vec<f64>> = (0..n)
        .map(|i| {
            let e = y[i] - x[i].iter().zip(&beta).map(|(a, c)| a * c).sum::<f64>();
            (0..p).map(|a| (0..p).map(|c| inv[a][c] * x[i][c]).sum::<f64>() * dr.w[i] * e).collect()
        })
        .collect();
    let Some(v) = ultimate_cluster(&infl, &dr) else {
        fail(&mut b, LONELY);
        return Ok(b);
    };
    let q = t_quantile(1.0 - (1.0 - spec.level) / 2.0, df);
    let mut rows = Vec::new();
    for j in 0..p {
        let se = v[j][j].sqrt();
        let t = beta[j] / se;
        let pv = t_two_sided(t, df);
        let name = if j == 0 { "(Intercept)".to_string() } else { pr.names[j - 1].clone() };
        let key = if j == 0 { "b0".to_string() } else { format!("term.{j}") };
        if j > 0 {
            set_text(&mut b, format!("{key}.label"), name.clone());
        }
        for (f, val) in [("b", beta[j]), ("se", se), ("t", t), ("p", pv), ("lower", beta[j] - q * se), ("upper", beta[j] + q * se)] {
            set(&mut b, format!("{key}.{f}"), val);
        }
        rows.push(vec![text(name), num(beta[j], 4), num(se, 4), num(t, 3), common::p(pv), num(beta[j] - q * se, 4), num(beta[j] + q * se, 4)]);
    }
    b.tables.push(Table { id: "coefficients".into(), title: "Coefficients".into(), columns: vec!["Term".into(), "B".into(), "SE (design-based)".into(), "t".into(), "p".into(), format!("{lv} CI lower"), format!("{lv} CI upper")], rows, footnotes: vec![design_text] });
    Ok(b)
}
