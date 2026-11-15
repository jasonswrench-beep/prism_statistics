//! Reliability analysis (`scale.reliability`): Cronbach's alpha for a set of items scored in the same direction.
//!
//! Defaults (05 §1): listwise deletion across the items; Cronbach's alpha with Feldt's confidence interval, the
//! standardized alpha (from the mean inter-item correlation), and for each item its mean, SD, corrected item-total
//! correlation and the alpha if it were left out. Items worded the other way round can be reverse-scored here
//! ((scale minimum + scale maximum) − value) without changing the data; the reversal is stated in the output. An item
//! with a negative corrected item-total correlation is flagged, never reversed automatically.

use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::apa;
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Diagnostic, DiagnosticState, ResultBundle, Table};

pub const METHOD_ID: &str = "scale.reliability";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub items: Vec<String>,
    /// Items to reverse-score for this analysis only.
    #[serde(default)]
    pub reverse: Vec<String>,
    /// The scale's lowest and highest possible scores, used to reverse items. When left out, the lowest and highest
    /// values found among the items are used (and the output says so).
    #[serde(default)]
    pub scale_min: Option<f64>,
    #[serde(default)]
    pub scale_max: Option<f64>,
    #[serde(default = "default_level")]
    pub level: f64,
}

fn default_level() -> f64 {
    0.95
}

/// Cronbach's alpha of the columns (items), or NaN when the total has no variance.
pub fn alpha(items: &[Vec<f64>]) -> f64 {
    let k = items.len() as f64;
    let n = items.first().map_or(0, Vec::len);
    let item_var: f64 = items.iter().map(|c| prismqda_stats::describe::variance(c).unwrap_or(f64::NAN)).sum();
    let totals: Vec<f64> = (0..n).map(|r| items.iter().map(|c| c[r]).sum()).collect();
    let total_var = prismqda_stats::describe::variance(&totals).unwrap_or(f64::NAN);
    if total_var.is_nan() || total_var <= 0.0 {
        return f64::NAN;
    }
    k / (k - 1.0) * (1.0 - item_var / total_var)
}

fn pearson(x: &[f64], y: &[f64]) -> f64 {
    prismqda_stats::correlation::pearson(x, y, 0.95).map_or(f64::NAN, |c| c.r)
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    if spec.items.len() < 2 {
        return Err(Error::Invalid("Choose at least two items.".into()));
    }
    for (i, a) in spec.items.iter().enumerate() {
        if spec.items[..i].iter().any(|b| ds.find(a) == ds.find(b)) {
            return Err(Error::Invalid(format!("{a} is listed twice.")));
        }
    }
    for r in &spec.reverse {
        if !spec.items.iter().any(|i| ds.find(i) == ds.find(r)) {
            return Err(Error::Invalid(format!("{r} is marked to be reversed but is not one of the items.")));
        }
    }
    if let (Some(lo), Some(hi)) = (spec.scale_min, spec.scale_max) {
        if lo >= hi {
            return Err(Error::Invalid("The scale's lowest score must be below its highest.".into()));
        }
    }
    check_level(spec.level)?;
    let labels: Vec<String> = spec.items.iter().map(|n| ds.variable(n).map(|v| v.display_name().to_string())).collect::<Result<_>>()?;
    let cells: Vec<Vec<Slot<f64>>> = spec.items.iter().map(|n| numeric(ds, n)).collect::<Result<_>>()?;
    let total = cells[0].len();
    let mut tally = Tally::default();
    let used: Vec<usize> = (0..total).filter(|&r| !cells.iter().zip(&spec.items).any(|(c, n)| tally.check(&c[r], n))).collect();
    let mut cols: Vec<Vec<f64>> = cells.iter().map(|c| used.iter().map(|&r| c[r].value().expect("checked")).collect()).collect();
    let reversed: Vec<bool> = spec.items.iter().map(|i| spec.reverse.iter().any(|r| ds.find(r) == ds.find(i))).collect();
    let observed = cols.iter().flatten().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
    let (lo, hi) = (spec.scale_min.unwrap_or(observed.0), spec.scale_max.unwrap_or(observed.1));
    for (c, rev) in cols.iter_mut().zip(&reversed) {
        if *rev {
            c.iter_mut().for_each(|v| *v = lo + hi - *v);
        }
    }
    let k = cols.len();
    let n = used.len();
    let rev_names: Vec<String> = labels.iter().zip(&reversed).filter(|(_, r)| **r).map(|(l, _)| l.clone()).collect();
    let lv = level_text(spec.level);
    let note = format!(
        "Cronbach's alpha for {k} items, with a {lv} confidence interval by Feldt's method (from the F distribution with N − 1 and (N − 1)(k − 1) degrees of freedom). Standardized alpha uses the mean inter-item correlation. The corrected item-total correlation is each item's correlation with the sum of the other items. {}Cases were used when every item had a valid value (listwise). Computed by PrismStatistics.",
        if rev_names.is_empty() {
            String::new()
        } else {
            format!(
                "{} {} reverse-scored for this analysis as ({} + {}) minus the value{}. ",
                apa::series(&rev_names),
                if rev_names.len() == 1 { "was" } else { "were" },
                apa::plain(lo),
                apa::plain(hi),
                if spec.scale_min.is_none() || spec.scale_max.is_none() { ", using the lowest and highest values found among the items" } else { "" }
            )
        }
    );
    let mut b = common::bundle(METHOD_ID, METHOD_VERSION, format!("Reliability: {} items", k), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, n), note);
    set(&mut b, "k", k as f64);
    set(&mut b, "n", n as f64);
    set(&mut b, "level", spec.level);
    set_text(&mut b, "items", labels.join("\u{1f}"));
    if !rev_names.is_empty() {
        set_text(&mut b, "reversed", rev_names.join("\u{1f}"));
    }
    if n < 3 {
        fail(&mut b, format!("Reliability needs at least three cases with every item; there {} {n}.", if n == 1 { "is" } else { "are" }));
        return Ok(b);
    }
    let a = alpha(&cols);
    if !a.is_finite() {
        fail(&mut b, "The scale total has no variation among the cases used, so alpha can't be computed.");
        return Ok(b);
    }
    // The mean of the correlations between different items.
    let mut rs = Vec::new();
    for i in 0..k {
        for j in (i + 1)..k {
            rs.push(pearson(&cols[i], &cols[j]));
        }
    }
    let rbar = rs.iter().sum::<f64>() / rs.len() as f64;
    let std_alpha = k as f64 * rbar / (1.0 + (k as f64 - 1.0) * rbar);
    let (d1, d2) = ((n - 1) as f64, ((n - 1) * (k - 1)) as f64);
    let tail = (1.0 - spec.level) / 2.0;
    let ci_lo = 1.0 - (1.0 - a) * prismqda_stats::dist::f_quantile(1.0 - tail, d1, d2);
    let ci_hi = 1.0 - (1.0 - a) * prismqda_stats::dist::f_quantile(tail, d1, d2);
    let totals: Vec<f64> = (0..n).map(|r| cols.iter().map(|c| c[r]).sum()).collect();
    let td = prismqda_stats::describe::descriptives(&totals, 0.95);
    // McDonald's omega total from a one-factor maximum likelihood model, rescaled to the items' own variances (what a
    // one-factor CFA gives): (sum of loadings)^2 / ((sum of loadings)^2 + sum of unique variances).
    let omega = if k >= 3 {
        let r = super::factor::correlations(&cols);
        super::linalg::invert(&r).map(|rinv| {
            let (load, psi, _) = super::factor::ml(&r, 1, &rinv);
            let sds: Vec<f64> = cols.iter().map(|c| {
                let m = c.iter().sum::<f64>() / n as f64;
                (c.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (n as f64 - 1.0)).sqrt()
            }).collect();
            let sum_l: f64 = load.iter().zip(&sds).map(|(l, s)| l[0] * s).sum();
            let sum_u: f64 = psi.iter().zip(&sds).map(|(u, s)| u * s * s).sum();
            sum_l * sum_l / (sum_l * sum_l + sum_u)
        })
    } else {
        None
    };
    if let Some(w) = omega {
        set(&mut b, "omega", w);
    }
    for (key, v) in [("alpha", a), ("alpha.ci.lower", ci_lo), ("alpha.ci.upper", ci_hi), ("alpha.std", std_alpha), ("r.mean", rbar), ("total.mean", td.mean.unwrap_or(f64::NAN)), ("total.sd", td.sd.unwrap_or(f64::NAN))] {
        set(&mut b, key, v);
    }
    b.tables.push(Table {
        id: "alpha".into(),
        title: "Reliability".into(),
        columns: vec!["Cronbach's α".into(), format!("{lv} CI lower"), format!("{lv} CI upper"), "Standardized α".into(), "McDonald's ω".into(), "Mean inter-item r".into(), "Items".into(), "N".into()],
        rows: vec![vec![num(a, 3), num(ci_lo, 3), num(ci_hi, 3), num(std_alpha, 3), opt(omega, 3), num(rbar, 3), count(k), count(n)]],
        footnotes: vec!["McDonald's omega (total) comes from a one-factor maximum likelihood model; unlike alpha it doesn't assume the items measure the factor equally well. It needs at least three items.".into()],
    });
    let mut rows = Vec::new();
    let mut negative = Vec::new();
    for i in 0..k {
        let rest: Vec<f64> = (0..n).map(|r| totals[r] - cols[i][r]).collect();
        let rit = pearson(&cols[i], &rest);
        let others: Vec<Vec<f64>> = cols.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, c)| c.clone()).collect();
        let a_del = if k > 2 { alpha(&others) } else { f64::NAN };
        let d = prismqda_stats::describe::descriptives(&cols[i], 0.95);
        let key = format!("item.{}", i + 1);
        set_text(&mut b, format!("{key}.label"), labels[i].clone());
        set(&mut b, format!("{key}.r_it"), rit);
        if a_del.is_finite() {
            set(&mut b, format!("{key}.alpha_deleted"), a_del);
        }
        if rit < 0.0 {
            negative.push(labels[i].clone());
        }
        let name = if reversed[i] { format!("{} (reversed)", labels[i]) } else { labels[i].clone() };
        rows.push(vec![text(name), opt(d.mean, 3), opt(d.sd, 3), num(rit, 3), num(a_del, 3)]);
    }
    b.tables.push(Table {
        id: "items".into(),
        title: "Item statistics".into(),
        columns: vec!["Item".into(), "Mean".into(), "SD".into(), "Corrected item-total r".into(), "α if item deleted".into()],
        rows,
        footnotes: if k == 2 { vec!["With two items, alpha if an item is deleted is not defined.".into()] } else { Vec::new() },
    });
    b.diagnostics.push(if negative.is_empty() {
        Diagnostic { id: "direction".into(), title: "Item direction".into(), state: DiagnosticState::NoConcernDetected, message: "Every item correlates positively with the rest of the scale.".into() }
    } else {
        Diagnostic { id: "direction".into(), title: "Item direction".into(), state: DiagnosticState::ReviewNeeded, message: format!("{} {} negatively with the rest of the scale. An item worded the other way round must be reverse-scored first (an option in this dialog); otherwise it lowers alpha and the scale total mixes opposite meanings.", apa::series(&negative), if negative.len() == 1 { "correlates" } else { "correlate" }) }
    });
    b.diagnostics.push(size_diagnostic(n, format!("{n} cases had every item")));
    b.diagnostics.push(not_assessed("dimensionality", "One dimension", "Alpha assumes the items measure one thing, about equally well. A high alpha does not show that; check the items' content or a factor analysis."));
    Ok(b)
}
