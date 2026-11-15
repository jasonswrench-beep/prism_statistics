//! Linear regression (`reg.linear`, task T3-15).
//!
//! Defaults (05 §1, §3 rows 6 and 7): ordinary least squares with an intercept; categorical predictors dummy-coded with
//! the reference category chosen by **code** (the lowest code unless another is chosen) and named in the coefficient
//! table; 95% intervals; listwise deletion across every variable in the model; no stepwise selection. A predictor that is
//! a combination of others is named and the model is refused rather than a coefficient silently dropped. Computed by
//! `prismqda_stats::regression::linear` (a QR decomposition, not the normal equations).

use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::apa;
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Cell, Diagnostic, DiagnosticState, ResultBundle, Table};
use prismqda_stats::regression::RegressionError;

pub const METHOD_ID: &str = "reg.linear";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Factor {
    pub name: String,
    /// The reference category's code (or text, for a text variable). None: the first category.
    #[serde(default)]
    pub reference: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub dependent: String,
    /// Numeric predictors, entered as they are.
    #[serde(default)]
    pub covariates: Vec<String>,
    /// Categorical predictors, dummy-coded against a reference category.
    #[serde(default)]
    pub factors: Vec<Factor>,
    #[serde(default = "default_level")]
    pub level: f64,
    /// Further blocks entered in order after the first (hierarchical regression), each tested by its R² change.
    #[serde(default)]
    pub blocks: Vec<Block>,
    /// Two-way interactions between predictors already in the model, entered in a final block of their own.
    #[serde(default)]
    pub interactions: Vec<(String, String)>,
    /// Mean-center the numeric predictors that appear in an interaction.
    #[serde(default)]
    pub center: bool,
    /// Residual plots: residuals against fitted values, and a normal Q-Q plot and histogram of the standardized residuals.
    #[serde(default)]
    pub plots: bool,
    /// A Bayes factor for the model against the intercept-only one (Zellner-Siow prior; numeric predictors only).
    #[serde(default)]
    pub bayes: bool,
    #[serde(default)]
    pub prior_scale: Option<f64>,
    /// Heteroskedasticity-consistent standard errors in place of the usual ones.
    #[serde(default)]
    pub robust: Option<Robust>,
}

/// Which heteroskedasticity-consistent covariance (R's sandwich::vcovHC types).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Robust {
    Hc0,
    Hc1,
    Hc2,
    Hc3,
}

impl Robust {
    fn name(self) -> &'static str {
        match self {
            Robust::Hc0 => "HC0",
            Robust::Hc1 => "HC1",
            Robust::Hc2 => "HC2",
            Robust::Hc3 => "HC3",
        }
    }
}

/// The sandwich covariance (XᵀX)⁻¹ Xᵀ diag(ωᵢ) X (XᵀX)⁻¹ of OLS coefficients, intercept first.
pub fn sandwich(columns: &[Vec<f64>], residuals: &[f64], leverage: &[f64], kind: Robust) -> Option<Vec<Vec<f64>>> {
    use super::linalg::{invert, mul, transpose};
    let n = residuals.len();
    let x: Vec<Vec<f64>> = (0..n).map(|i| std::iter::once(1.0).chain(columns.iter().map(|c| c[i])).collect()).collect();
    let k = x[0].len();
    let xt = transpose(&x);
    let bread = invert(&mul(&xt, &x))?;
    let omega: Vec<f64> = residuals
        .iter()
        .zip(leverage)
        .map(|(e, h)| match kind {
            Robust::Hc0 => e * e,
            Robust::Hc1 => e * e * n as f64 / (n - k) as f64,
            Robust::Hc2 => e * e / (1.0 - h),
            Robust::Hc3 => e * e / ((1.0 - h) * (1.0 - h)),
        })
        .collect();
    let meat: Vec<Vec<f64>> = (0..k).map(|a| (0..k).map(|c| (0..n).map(|i| x[i][a] * omega[i] * x[i][c]).sum()).collect()).collect();
    Some(mul(&bread, &mul(&meat, &bread)))
}

/// A later block of predictors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Block {
    #[serde(default)]
    pub covariates: Vec<String>,
    #[serde(default)]
    pub factors: Vec<Factor>,
}

fn default_level() -> f64 {
    0.95
}

/// What a model needs before fitting: the cases used, the outcome, the design columns in block order, and labels.
pub struct Prepared {
    pub used: Vec<usize>,
    pub total: usize,
    pub tally: Tally,
    pub y: Vec<f64>,
    pub y_label: String,
    pub columns: Vec<Vec<f64>>,
    pub names: Vec<String>,
    pub owner: Vec<usize>,
    pub references: Vec<String>,
    /// The display names of the predictors (one per variable or interaction), in order.
    pub pred_labels: Vec<String>,
    /// For each block, how many design columns the models up to and including it have.
    pub block_ends: Vec<usize>,
    pub centered: Vec<String>,
}

pub fn prepare(ds: &Dataset, spec: &Spec) -> Result<Prepared> {
    prepare_with(ds, spec, numeric(ds, &spec.dependent)?)
}

/// As [`prepare`], with the outcome's cells supplied (logistic regression passes category numbers).
pub fn prepare_with(ds: &Dataset, spec: &Spec, ys: Vec<Slot<f64>>) -> Result<Prepared> {
    let mut all_blocks = vec![Block { covariates: spec.covariates.clone(), factors: spec.factors.clone() }];
    all_blocks.extend(spec.blocks.iter().filter(|b| !b.covariates.is_empty() || !b.factors.is_empty()).cloned());
    if all_blocks.iter().all(|b| b.covariates.is_empty() && b.factors.is_empty()) {
        return Err(Error::Invalid("Choose at least one predictor.".into()));
    }
    let mut seen = vec![ds.find(&spec.dependent)];
    for n in all_blocks.iter().flat_map(|b| b.covariates.iter().chain(b.factors.iter().map(|f| &f.name))) {
        let i = ds.find(n);
        if seen.contains(&i) {
            return Err(Error::Invalid(format!("{n} is used more than once in the model.")));
        }
        seen.push(i);
    }
    let in_model = |n: &str| all_blocks.iter().any(|b| b.covariates.iter().any(|c| ds.find(c) == ds.find(n)) || b.factors.iter().any(|f| ds.find(&f.name) == ds.find(n)));
    for (a, c) in &spec.interactions {
        if !in_model(a) || !in_model(c) {
            return Err(Error::Invalid(format!("Both parts of an interaction must be predictors in the model ({a} × {c}).")));
        }
        if ds.find(a) == ds.find(c) {
            return Err(Error::Invalid("An interaction needs two different predictors.".into()));
        }
    }
    let y_var = ds.variable(&spec.dependent)?;
    let covs: Vec<String> = all_blocks.iter().flat_map(|b| b.covariates.clone()).collect();
    let facs: Vec<Factor> = all_blocks.iter().flat_map(|b| b.factors.clone()).collect();
    let xs: Vec<Vec<Slot<f64>>> = covs.iter().map(|n| numeric(ds, n)).collect::<Result<_>>()?;
    let fs: Vec<(Vec<Slot<usize>>, Vec<Level>)> = facs.iter().map(|f| categories(ds, &f.name)).collect::<Result<_>>()?;
    let total = ys.len();
    let mut tally = Tally::default();
    let used: Vec<usize> = (0..total)
        .filter(|&r| !(tally.check(&ys[r], "dependent variable") || covs.iter().zip(&xs).any(|(n, x)| tally.check(&x[r], n)) || facs.iter().zip(&fs).any(|(f, (s, _))| tally.check(&s[r], &f.name))))
        .collect();
    let y: Vec<f64> = used.iter().map(|&r| ys[r].value().expect("checked")).collect();
    let in_interaction = |n: &str| spec.interactions.iter().any(|(a, c)| ds.find(a) == ds.find(n) || ds.find(c) == ds.find(n));
    let mut centered = Vec::new();
    // Each block's design, in order; covariates in an interaction are centered on the cases used when asked.
    let (mut columns, mut names, mut owner, mut references, mut pred_labels, mut block_ends) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut per_pred: Vec<(String, Vec<Vec<f64>>, Vec<String>)> = Vec::new();
    let (mut ci, mut fi) = (0, 0);
    for blk in &all_blocks {
        let bx = &xs[ci..ci + blk.covariates.len()];
        let bf = &fs[fi..fi + blk.factors.len()];
        ci += blk.covariates.len();
        fi += blk.factors.len();
        let mut d = design(ds, &blk.covariates, &blk.factors, bx, bf, &used)?;
        for (j, n) in blk.covariates.iter().enumerate() {
            if spec.center && in_interaction(n) {
                let m = d.columns[j].iter().sum::<f64>() / d.columns[j].len().max(1) as f64;
                d.columns[j].iter_mut().for_each(|v| *v -= m);
                centered.push(ds.variable(n)?.display_name().to_string());
            }
        }
        let base = pred_labels.len();
        for (k, n) in blk.covariates.iter().chain(blk.factors.iter().map(|f| &f.name)).enumerate() {
            let cols: Vec<usize> = d.owner.iter().enumerate().filter(|(_, o)| **o == k).map(|(c, _)| c).collect();
            per_pred.push((n.clone(), cols.iter().map(|&c| d.columns[c].clone()).collect(), cols.iter().map(|&c| d.names[c].clone()).collect()));
            pred_labels.push(ds.variable(n)?.display_name().to_string());
        }
        columns.extend(d.columns);
        names.extend(d.names);
        owner.extend(d.owner.into_iter().map(|o| o + base));
        references.extend(d.references);
        block_ends.push(columns.len());
    }
    if !spec.interactions.is_empty() {
        for (a, c) in &spec.interactions {
            let pa = per_pred.iter().find(|p| ds.find(&p.0) == ds.find(a)).expect("checked above");
            let pc = per_pred.iter().find(|p| ds.find(&p.0) == ds.find(c)).expect("checked above");
            let (la, lc) = (ds.variable(a)?.display_name().to_string(), ds.variable(c)?.display_name().to_string());
            let idx = pred_labels.len();
            for (ca, na) in pa.1.iter().zip(&pa.2) {
                for (cc, nc) in pc.1.iter().zip(&pc.2) {
                    columns.push(ca.iter().zip(cc).map(|(u, v)| u * v).collect());
                    names.push(format!("{na} × {nc}"));
                    owner.push(idx);
                }
            }
            pred_labels.push(format!("{la} × {lc}"));
        }
        block_ends.push(columns.len());
    }
    centered.dedup();
    Ok(Prepared { used, total, tally, y, y_label: y_var.display_name().to_string(), columns, names, owner, references, pred_labels, block_ends, centered })
}

/// For saving: each case used, with its predicted value, residual, standardized residual, Cook's distance and leverage.
pub fn case_values(ds: &Dataset, spec: &Spec) -> Result<Vec<(usize, [f64; 5])>> {
    let pr = prepare(ds, spec)?;
    let fit = prismqda_stats::regression::linear(&pr.y, &pr.columns, spec.level).map_err(|_| Error::Invalid("The model could not be fitted, so there is nothing to save.".into()))?;
    Ok(pr.used.iter().enumerate().map(|(i, &r)| (r, [fit.fitted[i], fit.residuals[i], fit.standardized_residuals[i], fit.cooks_distance[i], fit.leverage[i]])).collect())
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    check_level(spec.level)?;
    let Prepared { used, total, tally, y, y_label, columns, names, owner, references, pred_labels, block_ends, centered } = prepare(ds, spec)?;
    let lv = level_text(spec.level);
    let note = format!(
        "Ordinary least squares with an intercept, fitted by QR decomposition. {}{}{}{lv} confidence intervals. Standardized coefficients (β) use the SDs of the predictor column and the outcome. Equal error variance is checked with Koenker's studentized Breusch-Pagan test. Cases were used when every variable in the model had a valid value (listwise). Computed by PrismStatistics.",
        if references.is_empty() { String::new() } else { format!("Categorical predictors are dummy-coded against a reference category ({}). ", references.join("; ")) },
        if block_ends.len() > 1 { format!("Predictors were entered in {} blocks, each tested by the F test of its R² change{}. ", block_ends.len(), if spec.interactions.is_empty() { "" } else { " (the interactions form the last block)" }) } else { String::new() },
        if centered.is_empty() { String::new() } else { format!("{} {} mean-centered before forming the interaction terms. ", crate::apa::series(&centered), if centered.len() == 1 { "was" } else { "were" }) }
    );
    let mut b = common::bundle(METHOD_ID, METHOD_VERSION, format!("Linear Regression: {y_label}"), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, used.len()), note);
    let y_var_name = y_label.clone();
    set_text(&mut b, "y.label", y_label.clone());
    set_text(&mut b, "predictors", pred_labels.join("\u{1f}"));
    set(&mut b, "level", spec.level);
    set(&mut b, "n", y.len() as f64);
    set(&mut b, "terms", names.len() as f64);
    set(&mut b, "blocks", block_ends.len() as f64);

    let fit = match prismqda_stats::regression::linear(&y, &columns, spec.level) {
        Ok(f) => f,
        Err(e) => {
            fail(
                &mut b,
                match e {
                    RegressionError::TooFewCases => format!("There are {} cases for {} coefficients; the model needs more cases than coefficients.", y.len(), names.len() + 1),
                    RegressionError::NoVariationInOutcome => format!("{y_var_name} has the same value for every case used."),
                    RegressionError::Collinear(i) => format!("{} is (almost) exactly predicted by the other predictors, so its effect can't be separated from theirs. Remove it or one of the predictors it depends on.", names.get(i).map_or("A predictor", String::as_str)),
                    RegressionError::NotFinite => "A value is not a finite number.".to_string(),
                },
            );
            return Ok(b);
        }
    };
    for (k, v) in [
        ("r2", fit.r_squared),
        ("adj_r2", fit.adj_r_squared),
        ("F", fit.f.f),
        ("df1", fit.f.df1),
        ("df2", fit.f.df2),
        ("p", fit.f.p),
        ("sigma", fit.sigma),
    ] {
        set(&mut b, k, v);
    }
    if block_ends.len() > 1 {
        let mut rows = Vec::new();
        let mut prev: Option<prismqda_stats::regression::Regression> = None;
        for (k, &end) in block_ends.iter().enumerate() {
            let m = if end == columns.len() { Some(fit.clone()) } else { prismqda_stats::regression::linear(&y, &columns[..end], spec.level).ok() };
            let Some(m) = m else { continue };
            let added: Vec<String> = { let lo = if k == 0 { 0 } else { block_ends[k - 1] }; let mut v: Vec<String> = owner[lo..end].iter().map(|o| pred_labels[*o].clone()).collect(); v.dedup(); v };
            let (dr2, fch, d1, d2, pch) = match &prev {
                None => (m.r_squared, m.f.f, m.f.df1, m.f.df2, m.f.p),
                Some(pm) => prismqda_stats::regression::compare(pm, &m).map_or((f64::NAN, f64::NAN, f64::NAN, f64::NAN, f64::NAN), |c| (c.r2_change, c.f, c.df1, c.df2, c.p)),
            };
            let key = format!("step.{}", k + 1);
            for (kk, v) in [("r2", m.r_squared), ("adj_r2", m.adj_r_squared), ("dr2", dr2), ("F", fch), ("df1", d1), ("df2", d2), ("p", pch)] {
                set(&mut b, format!("{key}.{kk}"), v);
            }
            set_text(&mut b, format!("{key}.added"), added.join("\u{1f}"));
            rows.push(vec![count(k + 1), text(added.join(", ")), num(m.r_squared, 3), num(m.adj_r_squared, 3), num(dr2, 3), num(fch, 3), num(d1, 0), num(d2, 0), p(pch)]);
            prev = Some(m);
        }
        b.tables.push(Table { id: "change".into(), title: "Model change".into(), columns: vec!["Model".into(), "Added".into(), "R²".into(), "Adjusted R²".into(), "R² change".into(), "F change".into(), "df1".into(), "df2".into(), "p".into()], rows, footnotes: vec!["Each model adds its block to the one before; the change is tested against the previous model. The tables below are for the full model.".into()] });
    }
    if spec.plots {
        let pts: Vec<(f64, f64)> = fit.fitted.iter().copied().zip(fit.residuals.iter().copied()).collect();
        let alt = format!("Residuals of {y_label} against the predicted values for {} cases, with a horizontal line at zero.", fit.n);
        b.charts.push(crate::chart::Chart { id: "resid_fitted".into(), title: "Residuals against predicted values".into(), alt: alt.clone(), svg: crate::chart::scatter(&pts, Some((0.0, 0.0)), "Predicted value", "Residual", &alt) });
        if let Some((qq, line)) = prismqda_stats::charts::normal_qq(&fit.standardized_residuals) {
            let qp: Vec<(f64, f64)> = qq.iter().map(|q| (q.theoretical, q.sample)).collect();
            let alt = "Normal Q-Q plot of the standardized residuals, with the reference line through the quartiles.".to_string();
            b.charts.push(crate::chart::Chart { id: "resid_qq".into(), title: "Normal Q-Q plot of the standardized residuals".into(), alt: alt.clone(), svg: crate::chart::scatter(&qp, Some((line.intercept, line.slope)), "Normal quantile", "Standardized residual", &alt) });
        }
        let bins: Vec<(f64, f64, usize)> = prismqda_stats::charts::histogram(&fit.standardized_residuals).iter().map(|x| (x.lo, x.hi, x.count)).collect();
        if !bins.is_empty() {
            let alt = "Histogram of the standardized residuals, with a standard normal curve.".to_string();
            b.charts.push(crate::chart::Chart { id: "resid_hist".into(), title: "Distribution of the standardized residuals".into(), alt: alt.clone(), svg: crate::chart::histogram(&bins, Some((0.0, 1.0, fit.n)), "Standardized residual", &alt) });
        }
        b.method_note.push_str(" Residual plots: residuals against predicted values, and the standardized residuals' Q-Q plot and histogram.");
    }
    b.tables.push(Table {
        id: "summary".into(),
        title: "Model summary".into(),
        columns: vec![
            "R".into(),
            "R²".into(),
            "Adjusted R²".into(),
            "SE of the estimate".into(),
            "N".into(),
        ],
        rows: vec![vec![
            num(fit.r_squared.sqrt(), 3),
            num(fit.r_squared, 3),
            num(fit.adj_r_squared, 3),
            num(fit.sigma, 3),
            count(fit.n),
        ]],
        footnotes: vec![format!("Predictors: {}.", pred_labels.join(", "))],
    });
    let ss_reg = fit.tss - fit.rss;
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
                text("Regression"),
                num(ss_reg, 3),
                num(fit.f.df1, 0),
                num(ss_reg / fit.f.df1, 3),
                num(fit.f.f, 3),
                p(fit.f.p),
            ],
            vec![
                text("Residual"),
                num(fit.rss, 3),
                num(fit.df_residual, 0),
                num(fit.rss / fit.df_residual, 3),
                Cell::Empty,
                Cell::Empty,
            ],
            vec![
                text("Total"),
                num(fit.tss, 3),
                num(fit.f.df1 + fit.df_residual, 0),
                Cell::Empty,
                Cell::Empty,
                Cell::Empty,
            ],
        ],
        footnotes: Vec::new(),
    });
    // Heteroskedasticity-consistent standard errors, when chosen: t tests on the residual df (as lmtest::coeftest).
    let robust_v = spec.robust.and_then(|kind| sandwich(&columns, &fit.residuals, &fit.leverage, kind));
    if let (Some(kind), Some(v)) = (spec.robust, &robust_v) {
        set_text(&mut b, "robust", kind.name());
        // The robust Wald test that every slope is zero.
        let k = v.len();
        if k > 1 {
            let sub: Vec<Vec<f64>> = v[1..].iter().map(|r| r[1..].to_vec()).collect();
            if let Some(inv) = super::linalg::invert(&sub) {
                let bs: Vec<f64> = fit.terms[1..].iter().map(|t| t.estimate).collect();
                let w: f64 = (0..k - 1).map(|a| (0..k - 1).map(|c| bs[a] * inv[a][c] * bs[c]).sum::<f64>()).sum();
                let fstat = w / (k - 1) as f64;
                set(&mut b, "robust.F", fstat);
                set(&mut b, "robust.df1", (k - 1) as f64);
                set(&mut b, "robust.p", prismqda_stats::dist::f_sf(fstat, (k - 1) as f64, fit.df_residual));
            }
        }
    }
    if spec.bayes {
        if spec.factors.is_empty() && spec.blocks.iter().all(|bl| bl.factors.is_empty()) {
            let r = spec.prior_scale.unwrap_or(std::f64::consts::SQRT_2 / 4.0);
            super::bayes::add(&mut b, super::bayes::regression_log_bf(fit.n as f64, names.len() as f64, fit.r_squared, r), &format!("Zellner-Siow on the coefficients, scale {}", apa::num(r, 3)));
        } else {
            b.method_note.push_str(" The Bayes factor is available for numeric predictors only.");
        }
    }
    let tq = prismqda_stats::dist::t_quantile(1.0 - (1.0 - spec.level) / 2.0, fit.df_residual);
    let mut rows = Vec::new();
    for (i, t) in fit.terms.iter().enumerate() {
        let mut t = t.clone();
        if let Some(v) = &robust_v {
            t.se = v[i][i].sqrt();
            t.t = t.estimate / t.se;
            t.p = prismqda_stats::dist::t_two_sided(t.t, fit.df_residual);
            t.ci = (t.estimate - tq * t.se, t.estimate + tq * t.se);
        }
        let t = &t;
        let name = if i == 0 {
            "(Intercept)".to_string()
        } else {
            names[i - 1].clone()
        };
        let key = if i == 0 {
            "b0".to_string()
        } else {
            format!("term.{i}")
        };
        if i > 0 {
            set_text(&mut b, format!("{key}.label"), name.clone());
            set(&mut b, format!("{key}.owner"), owner[i - 1] as f64);
        }
        for (k, v) in [
            ("b", t.estimate),
            ("se", t.se),
            ("t", t.t),
            ("p", t.p),
            ("ci.lower", t.ci.0),
            ("ci.upper", t.ci.1),
        ] {
            set(&mut b, format!("{key}.{k}"), v);
        }
        if let Some(beta) = t.beta {
            set(&mut b, format!("{key}.beta"), beta);
        }
        if let Some(v) = t.vif {
            set(&mut b, format!("{key}.vif"), v);
        }
        rows.push(vec![
            text(name),
            num(t.estimate, 3),
            num(t.se, 3),
            opt(t.beta, 3),
            num(t.t, 3),
            p(t.p),
            num(t.ci.0, 3),
            num(t.ci.1, 3),
            opt(t.vif, 2),
        ]);
    }
    b.tables.push(Table {
        id: "coefficients".into(),
        title: "Coefficients".into(),
        columns: vec!["Term".into(), "B".into(), "SE".into(), "β".into(), "t".into(), "p".into(), format!("{lv} CI lower"), format!("{lv} CI upper"), "VIF".into()],
        rows,
        footnotes: {
            let mut f = vec![format!("t tests have {} degrees of freedom. B is the unstandardized coefficient; β the standardized one.", apa::df(fit.df_residual))];
            if let Some(kind) = spec.robust.filter(|_| robust_v.is_some()) {
                f.push(format!(
                    "Standard errors, t, p and intervals are heteroskedasticity-consistent ({}){}.",
                    kind.name(),
                    b.number("robust.F").map_or(String::new(), |fv| format!("; the robust Wald test of every slope gives F({}, {}) = {}, p {}", apa::df(b.number("robust.df1").unwrap_or(f64::NAN)), apa::df(fit.df_residual), apa::num(fv, 2), apa::p_clause(b.number("robust.p").unwrap_or(f64::NAN))))
                ));
            }
            f
        },
    });

    let ratio = fit.n as f64 / names.len() as f64;
    b.diagnostics.push(if ratio < 10.0 {
        Diagnostic { id: "size".into(), title: "Cases per predictor".into(), state: DiagnosticState::ReviewNeeded, message: format!("There are {} cases for {} predictor terms ({} per term). With fewer than 10 per term, estimates can be unstable and R² overstates how well the model would predict new cases.", fit.n, names.len(), apa::num(ratio, 1)) }
    } else {
        Diagnostic { id: "size".into(), title: "Cases per predictor".into(), state: DiagnosticState::NoConcernDetected, message: format!("{} cases for {} predictor terms.", fit.n, names.len()) }
    });
    let high_vif: Vec<String> = fit
        .terms
        .iter()
        .skip(1)
        .zip(&names)
        .filter(|(t, _)| t.vif.is_some_and(|v| v > 5.0))
        .map(|(t, n)| format!("{n} ({})", apa::num(t.vif.unwrap_or(f64::NAN), 1)))
        .collect();
    b.diagnostics.push(if high_vif.is_empty() {
        Diagnostic { id: "collinearity".into(), title: "Collinearity".into(), state: DiagnosticState::NoConcernDetected, message: "Every variance inflation factor is 5 or below.".into() }
    } else {
        Diagnostic { id: "collinearity".into(), title: "Collinearity".into(), state: DiagnosticState::ReviewNeeded, message: format!("Variance inflation factors above 5: {}. These predictors overlap heavily with the others, so their coefficients have wide intervals and can change a lot from sample to sample.", high_vif.join(", ")) }
    });
    if let Some(bp) = &fit.breusch_pagan {
        set(&mut b, "bp", bp.statistic);
        set(&mut b, "bp.p", bp.p);
    }
    b.diagnostics.push(match &fit.breusch_pagan {
        Some(bp) => {
            let detail = format!("Breusch-Pagan (Koenker) χ²({}) = {}, p {}.", apa::df(bp.df), apa::num(bp.statistic, 2), apa::p_clause(bp.p));
            if bp.p < 0.05 {
                Diagnostic { id: "variance".into(), title: "Equal error variance".into(), state: DiagnosticState::ReviewNeeded, message: format!("{detail} The spread of the residuals seems to change with the predictors, so the standard errors and p values may be off. Look at the plot of residuals against fitted values.") }
            } else {
                Diagnostic { id: "variance".into(), title: "Equal error variance".into(), state: DiagnosticState::NoConcernDetected, message: detail }
            }
        }
        None => Diagnostic { id: "variance".into(), title: "Equal error variance".into(), state: DiagnosticState::NotAssessed, message: "The Breusch-Pagan test could not be computed.".into() },
    });
    let n = fit.n as f64;
    let influential = fit.cooks_distance.iter().filter(|d| **d > 4.0 / n).count();
    let big = fit.cooks_distance.iter().filter(|d| **d > 1.0).count();
    let worst = fit.cooks_distance.iter().copied().fold(0.0, f64::max);
    set(&mut b, "cooks.max", worst);
    b.diagnostics.push(if big > 0 {
        Diagnostic { id: "influence".into(), title: "Influential cases".into(), state: DiagnosticState::ReviewNeeded, message: format!("{big} case{} {} a Cook's distance above 1 (largest {}). Such a case changes the fitted model a lot; check it for an error before relying on the coefficients.", if big == 1 { "" } else { "s" }, if big == 1 { "has" } else { "have" }, apa::num(worst, 2)) }
    } else {
        Diagnostic { id: "influence".into(), title: "Influential cases".into(), state: DiagnosticState::NoConcernDetected, message: format!("No case has a Cook's distance above 1 (largest {}). {influential} case{} above 4/N, which is worth a look but not unusual.", apa::num(worst, 2), if influential == 1 { " is" } else { "s are" }) }
    });
    b.diagnostics.push(not_assessed("residuals", "Shape of the residuals", "Look at a histogram of the residuals and the plot of residuals against fitted values for curvature or outliers."));
    Ok(b)
}

/// The design columns of a model and what each stands for.
pub(crate) struct Design {
    pub columns: Vec<Vec<f64>>,
    /// The name of each column, as the coefficient table shows it.
    pub names: Vec<String>,
    /// Which predictor (covariates first, then factors) each column belongs to.
    pub owner: Vec<usize>,
    /// "Factor: reference category", one for each factor.
    pub references: Vec<String>,
}

/// The design columns in the order given: covariates as they are, then each factor's dummies against its reference
/// category (by code; the first category present among the cases used unless another is chosen).
pub(crate) fn design(ds: &Dataset, covariates: &[String], factors: &[Factor], xs: &[Vec<Slot<f64>>], fs: &[(Vec<Slot<usize>>, Vec<Level>)], used: &[usize]) -> Result<Design> {
    // The design columns and their names, in the order given: covariates, then each factor's dummies.
    let mut columns: Vec<Vec<f64>> = xs
        .iter()
        .map(|x| {
            used.iter()
                .map(|&r| x[r].value().expect("checked"))
                .collect()
        })
        .collect();
    let mut names: Vec<String> = covariates
        .iter()
        .map(|n| ds.variable(n).map(|v| v.display_name().to_string()))
        .collect::<Result<_>>()?;
    let mut owner: Vec<usize> = (0..covariates.len()).collect();
    let mut references = Vec::new();
    for (fi, (f, (slots, levels))) in factors.iter().zip(fs).enumerate() {
        let fname = ds.variable(&f.name)?.display_name().to_string();
        // Only categories present among the cases used.
        let present: Vec<usize> = (0..levels.len())
            .filter(|&l| used.iter().any(|&r| slots[r].value() == Some(l)))
            .collect();
        let reference = match &f.reference {
            None => *present.first().ok_or_else(|| {
                Error::Invalid(format!("{fname} has no categories among the cases used."))
            })?,
            Some(key) => present
                .iter()
                .copied()
                .find(|&l| {
                    levels[l].key() == *key
                        || levels[l]
                            .code
                            .is_some_and(|c| key.parse::<f64>().ok() == Some(c))
                })
                .ok_or_else(|| {
                    Error::Invalid(format!(
                        "The reference category {key} of {fname} has no cases in the model."
                    ))
                })?,
        };
        references.push(format!("{fname}: {}", levels[reference].label));
        for &l in present.iter().filter(|&&l| l != reference) {
            columns.push(
                used.iter()
                    .map(|&r| {
                        if slots[r].value() == Some(l) {
                            1.0
                        } else {
                            0.0
                        }
                    })
                    .collect(),
            );
            names.push(format!(
                "{fname}: {} (vs {})",
                levels[l].label, levels[reference].label
            ));
            owner.push(covariates.len() + fi);
        }
    }
    Ok(Design { columns, names, owner, references })
}
