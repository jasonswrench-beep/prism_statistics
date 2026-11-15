//! Factorial ANOVA and ANCOVA (`anova.factorial`): one numeric outcome by two or three categorical factors, with every
//! interaction (or main effects only), optional covariates, Type III or Type II sums of squares, estimated marginal
//! means, post hoc comparisons, simple effects, and profile and residual plots.
//!
//! Defaults (05 §1, §3 row 8): the full factorial model with Type III sums of squares from sum-to-zero (effect) coding;
//! Type II on request (the usual choice for a main-effects model); partial eta squared; Brown-Forsythe's test of equal
//! spread; estimated marginal means with covariates at their means, each cell weighted equally; pairwise comparisons of
//! a main effect's means by Tukey (or Bonferroni or Holm); simple effects as F tests of one factor within each category
//! of another, using the full model's error term; with covariates, the homogeneity-of-slopes check. With interactions,
//! every combination of categories must have cases. Checked against R's lm, car::Anova and emmeans.

use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::apa;
use crate::chart::{self, Chart};
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Cell, Diagnostic, DiagnosticState, ResultBundle, Table};
use prismqda_stats::dist::{f_sf, t_quantile, t_two_sided};

pub const METHOD_ID: &str = "anova.factorial";
pub const METHOD_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SsType {
    #[default]
    #[serde(rename = "3")]
    Three,
    #[serde(rename = "2")]
    Two,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PostHoc {
    #[default]
    None,
    Tukey,
    Bonferroni,
    Holm,
}

fn yes() -> bool {
    true
}

fn level95() -> f64 {
    0.95
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub dependent: String,
    /// Two or three categorical factors.
    pub factors: Vec<String>,
    /// Numeric covariates (ANCOVA).
    #[serde(default)]
    pub covariates: Vec<String>,
    /// Include every interaction of the factors (false: main effects only).
    #[serde(default = "yes")]
    pub interactions: bool,
    #[serde(default)]
    pub ss_type: SsType,
    #[serde(default = "yes")]
    pub emmeans: bool,
    #[serde(default)]
    pub post_hoc: PostHoc,
    #[serde(default)]
    pub simple_effects: bool,
    #[serde(default)]
    pub plots: bool,
    #[serde(default = "level95")]
    pub level: f64,
}

/// One effect of the model: a covariate, or the factors it involves (indexes into the factor list).
struct Effect {
    covariate: Option<usize>,
    factors: Vec<usize>,
    name: String,
    key: String,
}

/// The covariates, then the factor effects: main effects, then two-way, then three-way interactions.
fn effects(cov_names: &[String], cov_keys: &[String], names: &[String], keys: &[String], interactions: bool) -> Vec<Effect> {
    let mut out: Vec<Effect> = cov_names.iter().zip(cov_keys).enumerate().map(|(i, (n, k))| Effect { covariate: Some(i), factors: Vec::new(), name: n.clone(), key: k.clone() }).collect();
    let m = names.len();
    let mut fx: Vec<Effect> = (1u32..(1 << m))
        .map(|mask| {
            let f: Vec<usize> = (0..m).filter(|i| mask & (1 << i) != 0).collect();
            Effect { covariate: None, name: f.iter().map(|&i| names[i].clone()).collect::<Vec<_>>().join(" × "), key: f.iter().map(|&i| keys[i].clone()).collect::<Vec<_>>().join(":"), factors: f }
        })
        .filter(|e| interactions || e.factors.len() == 1)
        .collect();
    fx.sort_by_key(|e| e.factors.len());
    out.extend(fx);
    out
}

/// The inverse of a symmetric positive definite matrix by Gauss-Jordan elimination with partial pivoting.
fn invert(a: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = a.len();
    let mut m: Vec<Vec<f64>> = a.iter().enumerate().map(|(i, r)| { let mut row = r.clone(); row.extend((0..n).map(|j| f64::from(u8::from(i == j)))); row }).collect();
    for c in 0..n {
        let piv = (c..n).max_by(|&i, &j| m[i][c].abs().total_cmp(&m[j][c].abs()))?;
        if m[piv][c].abs() < 1e-12 {
            return None;
        }
        m.swap(c, piv);
        let d = m[c][c];
        m[c].iter_mut().for_each(|v| *v /= d);
        for r in 0..n {
            if r != c {
                let f = m[r][c];
                if f != 0.0 {
                    let rowc = m[c].clone();
                    m[r].iter_mut().zip(&rowc).for_each(|(v, w)| *v -= f * w);
                }
            }
        }
    }
    Some(m.into_iter().map(|r| r[n..].to_vec()).collect())
}

fn quad(l: &[f64], v: &[Vec<f64>], r: &[f64]) -> f64 {
    l.iter().enumerate().map(|(i, a)| a * v[i].iter().zip(r).map(|(b, c)| b * c).sum::<f64>()).sum()
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    if !(2..=3).contains(&spec.factors.len()) {
        return Err(Error::Invalid("Choose two or three factors.".into()));
    }
    check_level(spec.level)?;
    let mut seen = vec![ds.find(&spec.dependent)];
    for f in spec.factors.iter().chain(&spec.covariates) {
        if seen.contains(&ds.find(f)) {
            return Err(Error::Invalid(format!("{f} is used more than once.")));
        }
        seen.push(ds.find(f));
    }
    let y_var = ds.variable(&spec.dependent)?;
    let y_label = y_var.display_name().to_string();
    let ys = numeric(ds, &spec.dependent)?;
    let fs: Vec<(Vec<Slot<usize>>, Vec<Level>)> = spec.factors.iter().map(|f| categories(ds, f)).collect::<Result<_>>()?;
    let cs: Vec<Vec<Slot<f64>>> = spec.covariates.iter().map(|c| numeric(ds, c)).collect::<Result<_>>()?;
    let f_labels: Vec<String> = spec.factors.iter().map(|f| ds.variable(f).map(|v| v.display_name().to_string())).collect::<Result<_>>()?;
    let c_labels: Vec<String> = spec.covariates.iter().map(|f| ds.variable(f).map(|v| v.display_name().to_string())).collect::<Result<_>>()?;
    let total = ys.len();
    let mut tally = Tally::default();
    let used: Vec<usize> = (0..total)
        .filter(|&r| !(tally.check(&ys[r], "dependent variable") || spec.factors.iter().zip(&fs).any(|(f, (s, _))| tally.check(&s[r], f)) || spec.covariates.iter().zip(&cs).any(|(c, x)| tally.check(&x[r], c))))
        .collect();
    let y: Vec<f64> = used.iter().map(|&r| ys[r].value().expect("checked")).collect();
    let cov_cols: Vec<Vec<f64>> = cs.iter().map(|x| used.iter().map(|&r| x[r].value().expect("checked")).collect()).collect();
    let cov_means: Vec<f64> = cov_cols.iter().map(|c| c.iter().sum::<f64>() / c.len().max(1) as f64).collect();
    // Each factor's categories among the cases used, renumbered 0..L.
    let mut levels: Vec<Vec<Level>> = Vec::new();
    let mut codes: Vec<Vec<usize>> = Vec::new();
    for (slots, lv) in &fs {
        let present: Vec<usize> = (0..lv.len()).filter(|&l| used.iter().any(|&r| slots[r].value() == Some(l))).collect();
        codes.push(used.iter().map(|&r| present.iter().position(|&p| Some(p) == slots[r].value()).expect("present")).collect());
        levels.push(present.iter().map(|&l| lv[l].clone()).collect());
    }
    let effs = effects(&c_labels, &spec.covariates, &f_labels, &spec.factors, spec.interactions);
    let sstype = if spec.ss_type == SsType::Two { "Type II" } else { "Type III" };
    let lv_text = level_text(spec.level);
    let note = format!(
        "{} of {y_label} by {}{}{}. {sstype} sums of squares{}. Effect size: partial eta squared (SS effect / (SS effect + SS error)). Equal spread checked with the Brown-Forsythe test (Levene's test from the cell medians). Cases were used when the outcome, every factor{} had valid values (listwise). Computed by PrismStatistics.",
        if spec.covariates.is_empty() { "Factorial ANOVA" } else { "Factorial ANCOVA" },
        apa::series(&f_labels),
        if spec.interactions { " with every interaction" } else { ", main effects only" },
        if c_labels.is_empty() { String::new() } else { format!(", adjusting for {}", apa::series(&c_labels)) },
        if spec.ss_type == SsType::Two { ": each effect after the others it doesn't contain" } else { " from sum-to-zero (effect) coding: each effect is tested after all the others" },
        if c_labels.is_empty() { "" } else { " and covariate" }
    );
    let title = format!("Factorial {}: {y_label} by {}", if spec.covariates.is_empty() { "ANOVA" } else { "ANCOVA" }, f_labels.join(" and "));
    let mut b = common::bundle(METHOD_ID, METHOD_VERSION, title, serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, used.len()), note);
    set_text(&mut b, "y.label", y_label.clone());
    set_text(&mut b, "ss_type", sstype);
    set(&mut b, "n", y.len() as f64);
    set(&mut b, "level", spec.level);
    set(&mut b, "factors", f_labels.len() as f64);
    set(&mut b, "covariates", c_labels.len() as f64);
    if !c_labels.is_empty() {
        set_text(&mut b, "covariate.labels", c_labels.join("\u{1f}"));
    }
    for (i, l) in f_labels.iter().enumerate() {
        set_text(&mut b, format!("factor.{}.label", i + 1), l.clone());
        set(&mut b, format!("factor.{}.levels", i + 1), levels[i].len() as f64);
    }
    if let Some(i) = levels.iter().position(|l| l.len() < 2) {
        fail(&mut b, format!("{} has {} categor{} with data among the cases used; each factor needs at least two.", f_labels[i], levels[i].len(), if levels[i].len() == 1 { "y" } else { "ies" }));
        return Ok(b);
    }

    // The cells: every combination of categories, in order, with its cases.
    let sizes: Vec<usize> = levels.iter().map(Vec::len).collect();
    let n_cells: usize = sizes.iter().product();
    let cell_of = |r: usize| -> usize { codes.iter().zip(&sizes).fold(0, |acc, (c, &s)| acc * s + c[r]) };
    let cell_levels = |c: usize| -> Vec<usize> {
        let mut rem = c;
        let mut out = vec![0; sizes.len()];
        for i in (0..sizes.len()).rev() {
            out[i] = rem % sizes[i];
            rem /= sizes[i];
        }
        out
    };
    let mut cells: Vec<Vec<f64>> = vec![Vec::new(); n_cells];
    for (r, v) in y.iter().enumerate() {
        cells[cell_of(r)].push(*v);
    }
    let cell_name = |c: usize| -> Vec<String> { cell_levels(c).iter().enumerate().map(|(i, &l)| levels[i][l].label.clone()).collect() };
    let mut desc_rows = Vec::new();
    for (c, vals) in cells.iter().enumerate() {
        let d = prismqda_stats::describe::descriptives(vals, 0.95);
        let mut row: Vec<Cell> = cell_name(c).into_iter().map(text).collect();
        row.extend([count(vals.len()), opt(d.mean, 3), opt(d.sd, 3)]);
        desc_rows.push(row);
        let key = format!("cell.{}", c + 1);
        set_text(&mut b, format!("{key}.label"), cell_name(c).join(", "));
        set(&mut b, format!("{key}.n"), vals.len() as f64);
        if let Some(m) = d.mean {
            set(&mut b, format!("{key}.mean"), m);
        }
        if let Some(s) = d.sd {
            set(&mut b, format!("{key}.sd"), s);
        }
    }
    let mut dcols = f_labels.clone();
    dcols.extend(["N".into(), "Mean".into(), "SD".into()]);
    b.tables.push(Table { id: "descriptives".into(), title: format!("Descriptive statistics for {y_label}"), columns: dcols, rows: desc_rows, footnotes: if c_labels.is_empty() { Vec::new() } else { vec!["Observed means, before adjusting for the covariates.".into()] } });
    let empty: Vec<String> = (0..n_cells).filter(|&c| cells[c].is_empty()).map(|c| cell_name(c).join(", ")).collect();
    if spec.interactions && !empty.is_empty() {
        fail(&mut b, format!("Some combinations of categories have no cases ({}). Interaction tests aren't defined for a design with empty cells; leave out the interactions, combine categories for a substantive reason, or analyze fewer factors.", empty.join("; ")));
        return Ok(b);
    }

    // Effect coding: a factor with L categories gets L − 1 columns; the last category is −1 in all of them.
    let code_row = |f: usize, level: usize| -> Vec<f64> { (0..sizes[f] - 1).map(|j| if level == j { 1.0 } else if level == sizes[f] - 1 { -1.0 } else { 0.0 }).collect() };
    // The design row for a case (or a cell, with covariates given): intercept, then each effect's columns.
    let design_row = |lv: &[usize], cov: &[f64]| -> (Vec<f64>, Vec<usize>) {
        let mut row = vec![1.0];
        let mut own = vec![usize::MAX];
        for (e, eff) in effs.iter().enumerate() {
            if let Some(ci) = eff.covariate {
                row.push(cov[ci]);
                own.push(e);
                continue;
            }
            let mut prod = vec![1.0];
            for &f in &eff.factors {
                let cr = code_row(f, lv[f]);
                prod = prod.iter().flat_map(|p| cr.iter().map(move |c| p * c)).collect();
            }
            for v in prod {
                row.push(v);
                own.push(e);
            }
        }
        (row, own)
    };
    let rows_x: Vec<Vec<f64>> = (0..y.len()).map(|r| { let lv: Vec<usize> = codes.iter().map(|c| c[r]).collect(); let cv: Vec<f64> = cov_cols.iter().map(|c| c[r]).collect(); design_row(&lv, &cv).0 }).collect();
    let owner: Vec<usize> = design_row(&vec![0; sizes.len()], &vec![0.0; cov_cols.len()]).1;
    let p_cols = rows_x[0].len();
    let columns: Vec<Vec<f64>> = (1..p_cols).map(|j| rows_x.iter().map(|r| r[j]).collect()).collect();
    let col_owner: Vec<usize> = owner[1..].to_vec();
    let full = match prismqda_stats::regression::linear(&y, &columns, spec.level) {
        Ok(f) => f,
        Err(_) => {
            fail(&mut b, format!("The model could not be fitted. It has {} parameters for {} cases; each cell needs cases and there must be more cases than parameters.", p_cols, y.len()));
            return Ok(b);
        }
    };
    if full.df_residual < 1.0 {
        fail(&mut b, "There are no degrees of freedom left for error: every cell has a single case.");
        return Ok(b);
    }
    let mse = full.rss / full.df_residual;
    let dfe = full.df_residual;
    set(&mut b, "error.ss", full.rss);
    set(&mut b, "error.df", dfe);
    set(&mut b, "error.ms", mse);
    let rss_without = |drop: &dyn Fn(usize) -> bool| -> f64 {
        let keep: Vec<Vec<f64>> = columns.iter().zip(&col_owner).filter(|(_, o)| !drop(**o)).map(|(c, _)| c.clone()).collect();
        if keep.is_empty() { full.tss } else { prismqda_stats::regression::linear(&y, &keep, 0.95).map(|f| f.rss).unwrap_or(f64::NAN) }
    };
    let model_ss = full.tss - full.rss;
    let model_df = columns.len() as f64;
    let mut rows = vec![{
        let fv = model_ss / model_df / mse;
        vec![text("Corrected model"), num(model_ss, 3), num(model_df, 0), num(model_ss / model_df, 3), num(fv, 3), p(f_sf(fv, model_df, dfe)), num(model_ss / (model_ss + full.rss), 3)]
    }];
    if spec.ss_type == SsType::Three {
        let t0 = full.terms[0].t;
        let int_ss = t0 * t0 * mse;
        rows.push(vec![text("Intercept"), num(int_ss, 3), num(1.0, 0), num(int_ss, 3), num(t0 * t0, 3), p(f_sf(t0 * t0, 1.0, dfe)), num(int_ss / (int_ss + full.rss), 3)]);
    }
    // An effect "contains" another when its factors include all of the other's (and it is a different effect).
    let contains = |big: &Effect, small: &Effect| small.covariate.is_none() && big.covariate.is_none() && big.factors.len() > small.factors.len() && small.factors.iter().all(|f| big.factors.contains(f));
    for (e, eff) in effs.iter().enumerate() {
        let df_e = col_owner.iter().filter(|o| **o == e).count() as f64;
        let ss = match spec.ss_type {
            SsType::Three => rss_without(&|o| o == e) - full.rss,
            SsType::Two => {
                let higher: Vec<usize> = (0..effs.len()).filter(|&k| contains(&effs[k], eff)).collect();
                rss_without(&|o| o == e || higher.contains(&o)) - rss_without(&|o| higher.contains(&o))
            }
        };
        let fv = ss / df_e / mse;
        let pv = f_sf(fv, df_e, dfe);
        let pes = ss / (ss + full.rss);
        let key = format!("effect.{}", e + 1);
        set_text(&mut b, format!("{key}.label"), eff.name.clone());
        set_text(&mut b, format!("{key}.key"), eff.key.clone());
        set(&mut b, format!("{key}.covariate"), if eff.covariate.is_some() { 1.0 } else { 0.0 });
        for (k, v) in [("ss", ss), ("df", df_e), ("F", fv), ("p", pv), ("pes", pes)] {
            set(&mut b, format!("{key}.{k}"), v);
        }
        rows.push(vec![text(&eff.name), num(ss, 3), num(df_e, 0), num(ss / df_e, 3), num(fv, 3), p(pv), num(pes, 3)]);
    }
    set(&mut b, "effects", effs.len() as f64);
    rows.push(vec![text("Error"), num(full.rss, 3), num(dfe, 0), num(mse, 3), Cell::Empty, Cell::Empty, Cell::Empty]);
    if spec.ss_type == SsType::Three {
        let sum_y2: f64 = y.iter().map(|v| v * v).sum();
        rows.push(vec![text("Total"), num(sum_y2, 3), num(y.len() as f64, 0), Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty]);
    }
    rows.push(vec![text("Corrected total"), num(full.tss, 3), num(y.len() as f64 - 1.0, 0), Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty]);
    set(&mut b, "r2", full.r_squared);
    set(&mut b, "adj_r2", full.adj_r_squared);
    b.tables.push(Table {
        id: "anova".into(),
        title: format!("Tests of between-subjects effects: {y_label}"),
        columns: vec!["Source".into(), format!("{sstype} sum of squares"), "df".into(), "Mean square".into(), "F".into(), "p".into(), "Partial η²".into()],
        rows,
        footnotes: vec![format!("R² = {}, adjusted R² = {}.", apa::num(full.r_squared, 3), apa::num(full.adj_r_squared, 3))],
    });

    // Estimated marginal means: each cell's prediction with covariates at their means, averaged with equal weights.
    let beta: Vec<f64> = full.terms.iter().map(|t| t.estimate).collect();
    let xtx: Vec<Vec<f64>> = (0..p_cols).map(|i| (0..p_cols).map(|j| rows_x.iter().map(|r| r[i] * r[j]).sum()).collect()).collect();
    let Some(xtx_inv) = invert(&xtx) else {
        fail(&mut b, "The model's design could not be inverted (a predictor is exactly predicted by the others).");
        return Ok(b);
    };
    let vcov: Vec<Vec<f64>> = xtx_inv.iter().map(|r| r.iter().map(|v| v * mse).collect()).collect();
    let cell_x: Vec<Vec<f64>> = (0..n_cells).map(|c| design_row(&cell_levels(c), &cov_means).0).collect();
    let avg = |pick: &dyn Fn(&[usize]) -> bool| -> Vec<f64> {
        let chosen: Vec<usize> = (0..n_cells).filter(|&c| pick(&cell_levels(c))).collect();
        (0..p_cols).map(|j| chosen.iter().map(|&c| cell_x[c][j]).sum::<f64>() / chosen.len() as f64).collect()
    };
    let est = |l: &[f64]| -> (f64, f64) { (l.iter().zip(&beta).map(|(a, c)| a * c).sum(), quad(l, &vcov, l).max(0.0).sqrt()) };
    let tcrit = t_quantile(1.0 - (1.0 - spec.level) / 2.0, dfe);
    let mut emm_rows = Vec::new();
    let mut emm_l: Vec<Vec<Vec<f64>>> = Vec::new();
    for (i, lvs) in levels.iter().enumerate() {
        let mut per = Vec::new();
        for (j, l) in lvs.iter().enumerate() {
            let lvec = avg(&|cl: &[usize]| cl[i] == j);
            let (m, se) = est(&lvec);
            let key = format!("factor.{}.level.{}", i + 1, j + 1);
            set(&mut b, format!("{key}.emm"), m);
            set(&mut b, format!("{key}.se"), se);
            set(&mut b, format!("{key}.ci.lower"), m - tcrit * se);
            set(&mut b, format!("{key}.ci.upper"), m + tcrit * se);
            set_text(&mut b, format!("{key}.label"), l.label.clone());
            emm_rows.push(vec![text(&f_labels[i]), text(&l.label), num(m, 3), num(se, 3), num(m - tcrit * se, 3), num(m + tcrit * se, 3)]);
            per.push(lvec);
        }
        emm_l.push(per);
    }
    if spec.emmeans {
        b.tables.push(Table {
            id: "marginal".into(),
            title: "Estimated marginal means".into(),
            columns: vec!["Factor".into(), "Category".into(), "Mean".into(), "SE".into(), format!("{lv_text} CI lower"), format!("{lv_text} CI upper")],
            rows: emm_rows,
            footnotes: vec![format!("Each cell counts equally whatever its size{}.", if c_labels.is_empty() { String::new() } else { format!("; {} held at {}", apa::series(&c_labels), if c_labels.len() == 1 { "its mean" } else { "their means" }) })],
        });
    }

    // Pairwise comparisons of each main effect's marginal means.
    if spec.post_hoc != PostHoc::None {
        let adj_name = match spec.post_hoc {
            PostHoc::Tukey => "Tukey",
            PostHoc::Bonferroni => "Bonferroni",
            PostHoc::Holm => "Holm",
            PostHoc::None => "",
        };
        let mut prow = Vec::new();
        let mut n_pairs = 0;
        for (i, lvs) in levels.iter().enumerate() {
            let k = lvs.len();
            let pairs: Vec<(usize, usize)> = (0..k).flat_map(|a| ((a + 1)..k).map(move |c| (a, c))).collect();
            let m = pairs.len();
            let mut raw = Vec::new();
            let mut stats = Vec::new();
            for &(a, c) in &pairs {
                let l: Vec<f64> = emm_l[i][a].iter().zip(&emm_l[i][c]).map(|(u, v)| u - v).collect();
                let (d, se) = est(&l);
                let t = d / se;
                raw.push(t_two_sided(t, dfe));
                stats.push((d, se, t));
            }
            let adj: Vec<f64> = match spec.post_hoc {
                PostHoc::Tukey => stats.iter().map(|s| prismqda_stats::studentized::ptukey_upper(s.2.abs() * 2f64.sqrt(), k as f64, dfe)).collect(),
                PostHoc::Bonferroni => raw.iter().map(|p| (p * m as f64).min(1.0)).collect(),
                PostHoc::Holm => prismqda_stats::adjust::p_adjust(&raw, prismqda_stats::adjust::Adjustment::Holm),
                PostHoc::None => raw.clone(),
            };
            let crit = match spec.post_hoc {
                PostHoc::Tukey => prismqda_stats::studentized::qtukey(spec.level, k as f64, dfe) / 2f64.sqrt(),
                _ => t_quantile(1.0 - (1.0 - spec.level) / (2.0 * m as f64), dfe),
            };
            for (pi, &(a, c)) in pairs.iter().enumerate() {
                n_pairs += 1;
                let (d, se, t) = stats[pi];
                let key = format!("pair.{n_pairs}");
                set_text(&mut b, format!("{key}.factor"), f_labels[i].clone());
                set_text(&mut b, format!("{key}.a"), lvs[a].label.clone());
                set_text(&mut b, format!("{key}.b"), lvs[c].label.clone());
                for (kk, v) in [("diff", d), ("se", se), ("t", t), ("p", adj[pi]), ("p.raw", raw[pi]), ("ci.lower", d - crit * se), ("ci.upper", d + crit * se)] {
                    set(&mut b, format!("{key}.{kk}"), v);
                }
                prow.push(vec![text(&f_labels[i]), text(&lvs[a].label), text(&lvs[c].label), num(d, 3), num(se, 3), num(t, 3), p(adj[pi]), num(d - crit * se, 3), num(d + crit * se, 3)]);
            }
        }
        set(&mut b, "pairs", n_pairs as f64);
        set_text(&mut b, "post_hoc", adj_name);
        b.tables.push(Table {
            id: "post_hoc".into(),
            title: format!("Pairwise comparisons of marginal means ({adj_name})"),
            columns: vec!["Factor".into(), "Category (I)".into(), "Category (J)".into(), "Difference (I − J)".into(), "SE".into(), "t".into(), format!("p ({adj_name})"), format!("{lv_text} CI lower"), format!("{lv_text} CI upper")],
            rows: prow,
            footnotes: vec![format!("t tests with {} degrees of freedom; p values and intervals adjusted within each factor ({adj_name}{}).", apa::df(dfe), if spec.post_hoc == PostHoc::Holm { "; intervals by Bonferroni" } else { "" })],
        });
    }

    // Simple effects: one factor's differences within each category of another (two-factor designs).
    if spec.simple_effects && spec.interactions && sizes.len() == 2 {
        let mut srow = Vec::new();
        let mut n_se = 0;
        for (fa, fb) in [(0usize, 1usize), (1, 0)] {
            for (bl, blevel) in levels[fb].iter().enumerate() {
                let ls: Vec<Vec<f64>> = (0..sizes[fa]).map(|al| avg(&|cl: &[usize]| cl[fa] == al && cl[fb] == bl)).collect();
                let c: Vec<Vec<f64>> = (0..sizes[fa] - 1).map(|al| ls[al].iter().zip(&ls[sizes[fa] - 1]).map(|(u, v)| u - v).collect()).collect();
                let q = c.len();
                let cb: Vec<f64> = c.iter().map(|r| r.iter().zip(&beta).map(|(a, w)| a * w).sum()).collect();
                let cvc: Vec<Vec<f64>> = c.iter().map(|ri| c.iter().map(|rj| quad(ri, &vcov, rj)).collect()).collect();
                let Some(inv) = invert(&cvc) else { continue };
                let fv = quad(&cb, &inv, &cb) / q as f64;
                let pv = f_sf(fv, q as f64, dfe);
                n_se += 1;
                let key = format!("simple.{n_se}");
                set_text(&mut b, format!("{key}.effect"), f_labels[fa].clone());
                set_text(&mut b, format!("{key}.within"), format!("{} = {}", f_labels[fb], blevel.label));
                set(&mut b, format!("{key}.F"), fv);
                set(&mut b, format!("{key}.df1"), q as f64);
                set(&mut b, format!("{key}.p"), pv);
                srow.push(vec![text(&f_labels[fa]), text(format!("{} = {}", f_labels[fb], blevel.label)), num(fv, 3), num(q as f64, 0), num(dfe, 0), p(pv)]);
            }
        }
        set(&mut b, "simple", n_se as f64);
        b.tables.push(Table { id: "simple".into(), title: "Simple effects".into(), columns: vec!["Effect of".into(), "Within".into(), "F".into(), "df1".into(), "df2".into(), "p".into()], rows: srow, footnotes: vec!["Each test compares the marginal means of one factor within one category of the other, using the full model's error term.".into()] });
    } else if spec.simple_effects {
        b.method_note.push_str(" Simple effects are given for two-factor designs with the interaction.");
    }

    // ANCOVA: is the covariate's slope the same in every group?
    if !c_labels.is_empty() {
        let mut extra = columns.clone();
        for cc in &cov_cols {
            for (f, _) in sizes.iter().enumerate() {
                for r0 in 0..sizes[f] - 1 {
                    extra.push((0..y.len()).map(|r| cc[r] * code_row(f, codes[f][r])[r0]).collect());
                }
            }
        }
        if let Ok(big) = prismqda_stats::regression::linear(&y, &extra, 0.95) {
            if let Some(ch) = prismqda_stats::regression::compare(&full, &big) {
                set(&mut b, "slopes.F", ch.f);
                set(&mut b, "slopes.p", ch.p);
                let detail = format!("Covariate-by-factor interactions: F({}, {}) = {}, p {}.", apa::df(ch.df1), apa::df(ch.df2), apa::num(ch.f, 2), apa::p_clause(ch.p));
                b.diagnostics.push(if ch.p < 0.05 {
                    Diagnostic { id: "slopes".into(), title: "Homogeneity of slopes".into(), state: DiagnosticState::ReviewNeeded, message: format!("{detail} The covariate's slope seems to differ between groups, so a single adjustment may mislead; consider modeling the interaction.") }
                } else {
                    Diagnostic { id: "slopes".into(), title: "Homogeneity of slopes".into(), state: DiagnosticState::NoConcernDetected, message: detail }
                });
            }
        }
    }

    // Plots: the profile of marginal means (first factor across, second as lines), and the residuals.
    if spec.plots {
        let series: Vec<(String, Vec<f64>)> = (0..sizes[1]).map(|bl| (levels[1][bl].label.clone(), (0..sizes[0]).map(|al| est(&avg(&|cl: &[usize]| cl[0] == al && cl[1] == bl)).0).collect())).collect();
        let cats: Vec<String> = levels[0].iter().map(|l| l.label.clone()).collect();
        let alt = format!("Profile plot of the estimated marginal means of {y_label}: {} across, a line for each category of {}.", f_labels[0], f_labels[1]);
        b.charts.push(Chart { id: "profile".into(), title: format!("Estimated marginal means of {y_label}"), alt: alt.clone(), svg: chart::profile(&cats, &series, &f_labels[0], &format!("Estimated mean {y_label}"), &f_labels[1], &alt) });
        let pts: Vec<(f64, f64)> = full.fitted.iter().copied().zip(full.residuals.iter().copied()).collect();
        let alt = "Residuals against predicted values, with a horizontal line at zero.".to_string();
        b.charts.push(Chart { id: "resid_fitted".into(), title: "Residuals against predicted values".into(), alt: alt.clone(), svg: chart::scatter(&pts, Some((0.0, 0.0)), "Predicted value", "Residual", &alt) });
        if let Some((qq, line)) = prismqda_stats::charts::normal_qq(&full.standardized_residuals) {
            let qp: Vec<(f64, f64)> = qq.iter().map(|q| (q.theoretical, q.sample)).collect();
            let alt = "Normal Q-Q plot of the standardized residuals.".to_string();
            b.charts.push(Chart { id: "resid_qq".into(), title: "Normal Q-Q plot of the standardized residuals".into(), alt: alt.clone(), svg: chart::scatter(&qp, Some((line.intercept, line.slope)), "Normal quantile", "Standardized residual", &alt) });
        }
    }

    // Brown-Forsythe: Levene's test from the cell medians, with the cells as groups.
    let smallest = cells.iter().map(Vec::len).filter(|n| *n > 0).min().unwrap_or(0);
    let nonempty: Vec<Vec<f64>> = cells.iter().filter(|c| !c.is_empty()).cloned().collect();
    match prismqda_stats::anova::one_way(&nonempty).and_then(|o| o.levene) {
        Some(ft) => {
            set(&mut b, "levene.F", ft.f);
            set(&mut b, "levene.p", ft.p);
            let detail = format!("Brown-Forsythe test across the {} cells: F({}, {}) = {}, p {}.", nonempty.len(), apa::df(ft.df1), apa::df(ft.df2), apa::num(ft.f, 2), apa::p_clause(ft.p));
            b.diagnostics.push(if ft.p < 0.05 {
                Diagnostic { id: "variance".into(), title: "Equal spread".into(), state: DiagnosticState::ReviewNeeded, message: format!("{detail} The cells seem to differ in spread; with unequal cell sizes the F tests can then be too liberal or too strict.") }
            } else {
                Diagnostic { id: "variance".into(), title: "Equal spread".into(), state: DiagnosticState::NoConcernDetected, message: detail }
            });
        }
        None => b.diagnostics.push(not_assessed("variance", "Equal spread", "The test of equal spread could not be computed (a cell has too few cases or no variation).")),
    }
    b.diagnostics.push(size_diagnostic(smallest, format!("The smallest cell has {smallest} case{}", if smallest == 1 { "" } else { "s" })));
    b.diagnostics.push(not_assessed("shape", "Shape and outliers", "Look at the distribution of the outcome in each cell (or of the residuals) for skew and outliers."));
    Ok(b)
}
