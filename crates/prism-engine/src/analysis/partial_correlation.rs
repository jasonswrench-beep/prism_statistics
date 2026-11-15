//! Partial correlation (`corr.partial`): Pearson correlations between pairs of variables with the linear effect of one or
//! more control variables taken out of both.
//!
//! Defaults (05 §1): listwise deletion across every variable (variables and controls), two-sided t tests with
//! N − 2 − (number of controls) degrees of freedom, Fisher-z intervals, and the zero-order correlations of the same cases
//! beside them so the change is visible. Computed by `prismqda_stats::correlation::partial_correlation`.

use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Cell, ResultBundle, Table};

pub const METHOD_ID: &str = "corr.partial";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub variables: Vec<String>,
    pub controls: Vec<String>,
    /// Also show the ordinary correlations of the same cases.
    #[serde(default = "yes")]
    pub zero_order: bool,
    #[serde(default = "default_level")]
    pub level: f64,
}

fn yes() -> bool {
    true
}

fn default_level() -> f64 {
    0.95
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    if spec.variables.len() < 2 {
        return Err(Error::Invalid("Choose at least two variables to correlate.".into()));
    }
    if spec.controls.is_empty() {
        return Err(Error::Invalid("Choose at least one variable to control for.".into()));
    }
    let all: Vec<&String> = spec.variables.iter().chain(&spec.controls).collect();
    for (i, a) in all.iter().enumerate() {
        if all[..i].iter().any(|b| ds.find(a) == ds.find(b)) {
            return Err(Error::Invalid(format!("{a} is used more than once.")));
        }
    }
    check_level(spec.level)?;
    let label = |n: &String| ds.variable(n).map(|v| v.display_name().to_string());
    let labels: Vec<String> = spec.variables.iter().map(label).collect::<Result<_>>()?;
    let control_labels: Vec<String> = spec.controls.iter().map(label).collect::<Result<_>>()?;
    let cells: Vec<Vec<Slot<f64>>> = all.iter().map(|n| numeric(ds, n)).collect::<Result<_>>()?;
    let total = cells[0].len();
    let mut tally = Tally::default();
    let used: Vec<usize> = (0..total).filter(|&r| !cells.iter().zip(&all).any(|(c, n)| tally.check(&c[r], n))).collect();
    let col = |i: usize| -> Vec<f64> { used.iter().map(|&r| cells[i][r].value().expect("checked")).collect() };
    let k = labels.len();
    let controls: Vec<Vec<f64>> = (k..all.len()).map(col).collect();
    let lv = level_text(spec.level);
    let note = format!(
        "Partial Pearson correlations controlling for {}: each variable's linear relationship with the controls is removed (least-squares residuals) and the residuals are correlated. Two-sided t tests with N − 2 − {} degrees of freedom; {lv} confidence intervals from Fisher's z with N − 3 − {} in the standard error. Cases were used when every variable and control had a valid value (listwise). Computed by PrismStatistics.",
        common_series(&control_labels),
        controls.len(),
        controls.len()
    );
    let mut b = common::bundle(METHOD_ID, METHOD_VERSION, format!("Partial Correlations controlling for {}", control_labels.join(", ")), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, used.len()), note);
    set(&mut b, "level", spec.level);
    set(&mut b, "n", used.len() as f64);
    set_text(&mut b, "controls", control_labels.join("\u{1f}"));
    for (i, l) in labels.iter().enumerate() {
        set_text(&mut b, format!("var.{}.label", i + 1), l.clone());
    }
    let mut pair_rows = Vec::new();
    let mut matrix = vec![vec![None; k]; k];
    let mut any = false;
    for i in 0..k {
        for j in (i + 1)..k {
            let (x, y) = (col(i), col(j));
            let pc = prismqda_stats::correlation::partial_correlation(&x, &y, &controls, spec.level);
            let zero = prismqda_stats::correlation::pearson(&x, &y, spec.level);
            let key = format!("pair.{}.{}", i + 1, j + 1);
            let mut row = vec![text(format!("{} & {}", labels[i], labels[j]))];
            if spec.zero_order {
                row.push(zero.as_ref().map_or(Cell::Empty, |z| num(z.r, 3)));
                if let Some(z) = &zero {
                    set(&mut b, format!("{key}.r0"), z.r);
                }
            }
            match &pc {
                Some(c) => {
                    any = true;
                    set(&mut b, format!("{key}.r"), c.r);
                    set(&mut b, format!("{key}.df"), c.df);
                    set(&mut b, format!("{key}.t"), c.t);
                    set(&mut b, format!("{key}.p"), c.p);
                    if let Some((lo, hi)) = c.ci {
                        set(&mut b, format!("{key}.ci.lower"), lo);
                        set(&mut b, format!("{key}.ci.upper"), hi);
                    }
                    row.extend([num(c.r, 3), opt(c.ci.map(|v| v.0), 3), opt(c.ci.map(|v| v.1), 3), num(c.df, 0), p(c.p)]);
                }
                None => row.extend([Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty]),
            }
            pair_rows.push(row);
            matrix[i][j] = pc.map(|c| (c.r, c.p, c.df));
        }
    }
    let mut columns = vec![String::new(), String::new()];
    columns.extend(labels.iter().cloned());
    let mut rows = Vec::new();
    for i in 0..k {
        let get = |j: usize| if i < j { matrix[i][j] } else { matrix[j][i] };
        let mut r_row = vec![text(&labels[i]), text("r")];
        let mut p_row = vec![Cell::Empty, text("p (two-sided)")];
        let mut d_row = vec![Cell::Empty, text("df")];
        for j in 0..k {
            if i == j {
                r_row.push(num(1.0, 3));
                p_row.push(Cell::Empty);
                d_row.push(Cell::Empty);
                continue;
            }
            let v = get(j);
            r_row.push(v.map_or(Cell::Empty, |v| num(v.0, 3)));
            p_row.push(v.map_or(Cell::Empty, |v| p(v.1)));
            d_row.push(v.map_or(Cell::Empty, |v| num(v.2, 0)));
        }
        rows.extend([r_row, p_row, d_row]);
    }
    b.tables.push(Table { id: "matrix".into(), title: format!("Partial correlations (controlling for {})", control_labels.join(", ")), columns, rows, footnotes: vec![format!("N = {} for every pair.", used.len())] });
    let mut pcols = vec!["Pair".to_string()];
    if spec.zero_order {
        pcols.push("Zero-order r".into());
    }
    pcols.extend(["Partial r".into(), format!("{lv} CI lower"), format!("{lv} CI upper"), "df".into(), "p (two-sided)".into()]);
    b.tables.push(Table { id: "pairs".into(), title: "Pairs".into(), columns: pcols, rows: pair_rows, footnotes: if spec.zero_order { vec!["The zero-order r is the ordinary Pearson correlation of the same cases, before the controls are taken out.".into()] } else { Vec::new() } });
    if !any {
        fail(&mut b, format!("No partial correlation could be computed. With {} control{} each pair needs at least {} cases with every value ({} here), some variation in every variable, and controls that are not predicted exactly by each other.", controls.len(), if controls.len() == 1 { "" } else { "s" }, controls.len() + 4, used.len()));
    }
    b.diagnostics.push(size_diagnostic(used.len(), format!("{} cases had every value", used.len())));
    b.diagnostics.push(not_assessed("shape", "Linearity and outliers", "The controls are removed by straight-line regression, and Pearson's r is pulled by outliers. Look at scatterplots of the variables against each control."));
    Ok(b)
}

fn common_series(items: &[String]) -> String {
    crate::apa::series(items)
}
