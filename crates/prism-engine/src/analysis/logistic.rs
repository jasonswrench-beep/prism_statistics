//! Binary logistic regression (`reg.logistic`).
//!
//! Defaults (05 §1): the outcome must have exactly two categories among the cases used; the one modeled (the "event")
//! is the second category (the higher code) unless another is chosen, and the output names it. Predictors enter
//! together (no stepwise selection); categorical predictors are dummy-coded against a reference category exactly as in
//! linear regression. Wald tests for the coefficients, with profile-likelihood intervals for the odds ratios (Wald
//! intervals where a profile end doesn't exist); a likelihood-ratio test of the
//! model and of each predictor (all of a categorical predictor's dummies at once); Nagelkerke's R² with Cox and Snell's
//! and McFadden's beside it; the Hosmer-Lemeshow test; a classification table at a cut-off of .5. Perfect separation is
//! reported as a failure, never as huge coefficients. Computed by `prismqda_stats::logistic` (IRLS, as R's glm).

use serde::{Deserialize, Serialize};

use super::common::{self, *};
use super::regression::{Block, Factor};
use crate::apa;
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Cell, Diagnostic, DiagnosticState, ResultBundle, Table};
use prismqda_stats::logistic::LogisticError;

pub const METHOD_ID: &str = "reg.logistic";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub dependent: String,
    /// The category modeled as the event, by code (or text). None: the second category.
    #[serde(default)]
    pub event: Option<String>,
    #[serde(default)]
    pub covariates: Vec<String>,
    #[serde(default)]
    pub factors: Vec<Factor>,
    #[serde(default = "default_level")]
    pub level: f64,
    /// Later blocks, each tested by the likelihood-ratio change.
    #[serde(default)]
    pub blocks: Vec<Block>,
    /// Two-way interactions between predictors in the model, entered as a final block.
    #[serde(default)]
    pub interactions: Vec<(String, String)>,
    #[serde(default)]
    pub center: bool,
    /// The ROC curve with the area under it and its DeLong interval.
    #[serde(default)]
    pub roc: bool,
}

fn default_level() -> f64 {
    0.95
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    check_level(spec.level)?;
    let y_var = ds.variable(&spec.dependent)?;
    let y_label = y_var.display_name().to_string();
    let (ys, ylev) = categories(ds, &spec.dependent)?;
    // The design is built as for linear regression, with the outcome's category numbers in place of values.
    let reg = super::regression::Spec { dependent: spec.dependent.clone(), covariates: spec.covariates.clone(), factors: spec.factors.clone(), level: spec.level, blocks: spec.blocks.clone(), interactions: spec.interactions.clone(), center: spec.center, plots: false, robust: Default::default(), bayes: Default::default(), prior_scale: Default::default() };
    let ycells: Vec<Slot<f64>> = ys.iter().map(|s| match s { Slot::Valid(i) => Slot::Valid(*i as f64), Slot::Empty => Slot::Empty, Slot::UserMissing => Slot::UserMissing }).collect();
    let pr = super::regression::prepare_with(ds, &reg, ycells)?;
    let (used, tally, total) = (pr.used, pr.tally, pr.total);
    let present: Vec<usize> = (0..ylev.len()).filter(|&l| pr.y.iter().any(|v| *v as usize == l)).collect();
    let pred_labels = pr.pred_labels.clone();
    let mut b = common::bundle(METHOD_ID, METHOD_VERSION, format!("Binary Logistic Regression: {y_label}"), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, used.len()), String::new());
    set_text(&mut b, "y.label", y_label.clone());
    set_text(&mut b, "predictors", pred_labels.join("\u{1f}"));
    set(&mut b, "level", spec.level);
    set(&mut b, "n", used.len() as f64);
    if present.len() != 2 {
        b.method_note = format!("Binary logistic regression. Computed by prismqda-stats {}.", super::STATS_VERSION);
        fail(&mut b, format!("The outcome must have exactly two categories among the cases used; {y_label} has {}.", present.len()));
        return Ok(b);
    }
    let event = match &spec.event {
        None => present[1],
        Some(key) => present.iter().copied().find(|&l| ylev[l].key() == *key || ylev[l].code.is_some_and(|c| key.parse::<f64>().ok() == Some(c))).ok_or_else(|| Error::Invalid(format!("The event category {key} of {y_label} has no cases in the model.")))?,
    };
    let other = if present[0] == event { present[1] } else { present[0] };
    let (ev, nonev) = (ylev[event].label.clone(), ylev[other].label.clone());
    let y: Vec<f64> = pr.y.iter().map(|v| if *v as usize == event { 1.0 } else { 0.0 }).collect();
    let (columns, names, owner, references, block_ends, centered) = (pr.columns, pr.names, pr.owner, pr.references, pr.block_ends, pr.centered);
    let lv = level_text(spec.level);
    b.method_note = format!(
        "Binary logistic regression of {y_label} = {ev} (against {nonev}), fitted by maximum likelihood (iteratively reweighted least squares). {}{}{}Coefficients are tested with Wald's z; odds ratios are e to the coefficient, with {lv} profile-likelihood intervals (a Wald limit is used, and marked, where the likelihood doesn't fall far enough for a profile limit). The likelihood-ratio tests compare the model with one that leaves out the predictor (all its dummies together). R² measures are Nagelkerke's, Cox and Snell's and McFadden's; the Hosmer-Lemeshow test uses 10 groups. Cases were used when every variable in the model had a valid value (listwise). Computed by prismqda-stats {}.",
        if block_ends.len() > 1 { format!("Predictors were entered in {} blocks, each tested by the likelihood-ratio change. ", block_ends.len()) } else { "All predictors entered together. ".to_string() },
        if centered.is_empty() { String::new() } else { format!("{} {} mean-centered before forming the interaction terms. ", crate::apa::series(&centered), if centered.len() == 1 { "was" } else { "were" }) },
        if references.is_empty() { String::new() } else { format!("Categorical predictors are dummy-coded against a reference category ({}). ", references.join("; ")) },
        super::STATS_VERSION
    );
    set(&mut b, "blocks", block_ends.len() as f64);
    set_text(&mut b, "event.label", ev.clone());
    set_text(&mut b, "nonevent.label", nonev.clone());
    let fit = match prismqda_stats::logistic::logistic(&y, &columns, spec.level, true) {
        Ok(f) => f,
        Err(e) => {
            fail(&mut b, match e {
                LogisticError::TooFewCases => format!("There are {} cases for {} coefficients; the model needs more cases than coefficients.", y.len(), names.len() + 1),
                LogisticError::NoVariationInOutcome => format!("Every case used has the same outcome ({}).", if y.iter().all(|v| *v == 1.0) { &ev } else { &nonev }),
                LogisticError::Collinear(i) => format!("{} is (almost) exactly predicted by the other predictors, so its effect can't be separated from theirs. Remove it or one of the predictors it depends on.", names.get(i).map_or("A predictor", String::as_str)),
                LogisticError::Separation => "The predictors separate the two outcomes (almost) perfectly, so the coefficients have no finite estimate. This often happens with a category in which every case has the same outcome, or with very few events; combine categories for a substantive reason or use fewer predictors.".to_string(),
                LogisticError::NotBinary | LogisticError::NotFinite => "A value is not a finite number.".to_string(),
            });
            return Ok(b);
        }
    };
    for (k, v) in [("events", fit.events as f64), ("nonevents", fit.non_events as f64), ("chi2", fit.lr.chi2), ("df", fit.lr.df), ("p", fit.lr.p), ("deviance", fit.deviance), ("null_deviance", fit.null_deviance), ("nagelkerke", fit.nagelkerke), ("cox_snell", fit.cox_snell), ("mcfadden", fit.mcfadden), ("aic", fit.aic), ("auc", fit.auc), ("accuracy", fit.classification.accuracy)] {
        set(&mut b, k, v);
    }
    b.tables.push(Table {
        id: "outcome".into(),
        title: "Outcome".into(),
        columns: vec![y_label.clone(), "N".into(), "Coded".into()],
        rows: vec![vec![text(&nonev), count(fit.non_events), count(0)], vec![text(&ev), count(fit.events), count(1)]],
        footnotes: vec![format!("The model predicts the probability of {ev}.")],
    });
    b.tables.push(Table {
        id: "omnibus".into(),
        title: "Test of the model".into(),
        columns: vec!["Test".into(), "Chi-square".into(), "df".into(), "p".into()],
        rows: vec![vec![text("Likelihood ratio (model vs intercept only)"), num(fit.lr.chi2, 3), num(fit.lr.df, 0), p(fit.lr.p)]],
        footnotes: Vec::new(),
    });
    b.tables.push(Table {
        id: "summary".into(),
        title: "Model summary".into(),
        columns: vec!["−2 log likelihood".into(), "Cox & Snell R²".into(), "Nagelkerke R²".into(), "McFadden R²".into(), "AIC".into(), "AUC".into()],
        rows: vec![vec![num(fit.deviance, 3), num(fit.cox_snell, 3), num(fit.nagelkerke, 3), num(fit.mcfadden, 3), num(fit.aic, 3), num(fit.auc, 3)]],
        footnotes: vec!["AUC is the area under the ROC curve: the chance that a case with the event gets a higher predicted probability than one without.".into()],
    });
    let mut rows = Vec::new();
    let mut wald_used: Vec<String> = Vec::new();
    for (i, t) in fit.terms.iter().enumerate() {
        let name = if i == 0 { "(Intercept)".to_string() } else { names[i - 1].clone() };
        let key = if i == 0 { "b0".to_string() } else { format!("term.{i}") };
        if i > 0 {
            set_text(&mut b, format!("{key}.label"), name.clone());
            set(&mut b, format!("{key}.owner"), owner[i - 1] as f64);
        }
        // The profile-likelihood interval of the odds ratio (none for the intercept), falling back to Wald for a missing end.
        let (plo, phi) = t.or_profile_ci.unwrap_or((None, None));
        if i > 0 && (plo.is_none() || phi.is_none()) {
            wald_used.push(name.clone());
        }
        let (lo, hi) = (plo.unwrap_or(t.or_ci.0), phi.unwrap_or(t.or_ci.1));
        for (k, v) in [("b", t.estimate), ("se", t.se), ("wald", t.wald), ("p", t.p), ("or", t.odds_ratio), ("or.lower", lo), ("or.upper", hi), ("or.wald.lower", t.or_ci.0), ("or.wald.upper", t.or_ci.1)] {
            set(&mut b, format!("{key}.{k}"), v);
        }
        // The intercept's odds ratio is the odds at zero on every predictor; its interval says little, so it isn't shown.
        let (lo_cell, hi_cell) = if i == 0 { (Cell::Empty, Cell::Empty) } else { (num(lo, 3), num(hi, 3)) };
        rows.push(vec![text(name), num(t.estimate, 3), num(t.se, 3), num(t.wald, 3), num(1.0, 0), p(t.p), num(t.odds_ratio, 3), lo_cell, hi_cell]);
    }
    b.tables.push(Table {
        id: "coefficients".into(),
        title: "Coefficients".into(),
        columns: vec!["Term".into(), "B".into(), "SE".into(), "Wald".into(), "df".into(), "p".into(), "Odds ratio".into(), format!("{lv} CI lower"), format!("{lv} CI upper")],
        rows,
        footnotes: {
            let mut f = vec![format!("B is the change in the log odds of {ev} for one unit more of the predictor (or for the category against the reference); the odds ratio is e to B. Intervals are profile-likelihood intervals.")];
            if !wald_used.is_empty() {
                f.push(format!("A profile limit doesn't exist for {}, so the Wald limit is shown there.", apa::series(&wald_used)));
            }
            f
        },
    });
    let mut lr_rows = Vec::new();
    for (pi, label) in pred_labels.iter().enumerate() {
        let without: Vec<usize> = owner.iter().enumerate().filter(|(_, o)| **o == pi).map(|(c, _)| c).collect();
        if let Some(t) = prismqda_stats::logistic::drop_test(&y, &columns, fit.deviance, &without) {
            set(&mut b, format!("pred.{}.chi2", pi + 1), t.chi2);
            set(&mut b, format!("pred.{}.df", pi + 1), t.df);
            set(&mut b, format!("pred.{}.p", pi + 1), t.p);
            lr_rows.push(vec![text(label), num(t.chi2, 3), num(t.df, 0), p(t.p)]);
        } else {
            lr_rows.push(vec![text(label), Cell::Empty, Cell::Empty, Cell::Empty]);
        }
    }
    b.tables.push(Table { id: "lr_tests".into(), title: "Likelihood-ratio tests of each predictor".into(), columns: vec!["Predictor".into(), "Chi-square".into(), "df".into(), "p".into()], rows: lr_rows, footnotes: vec!["Each predictor is tested by leaving it out of the model (a categorical predictor with all its categories at once).".into()] });
    if block_ends.len() > 1 {
        let mut rows = Vec::new();
        let mut prev: Option<prismqda_stats::logistic::Logistic> = None;
        for (k, &end) in block_ends.iter().enumerate() {
            let m = if end == columns.len() { Some(fit.clone()) } else { prismqda_stats::logistic::logistic(&y, &columns[..end], spec.level, false).ok() };
            let Some(m) = m else { continue };
            let lo = if k == 0 { 0 } else { block_ends[k - 1] };
            let mut added: Vec<String> = owner[lo..end].iter().map(|o| pred_labels[*o].clone()).collect();
            added.dedup();
            let (chi, d, pv) = match &prev {
                None => (m.lr.chi2, m.lr.df, m.lr.p),
                Some(pm) => prismqda_stats::logistic::compare(pm, &m).map_or((f64::NAN, f64::NAN, f64::NAN), |c| (c.chi2, c.df, c.p)),
            };
            let key = format!("step.{}", k + 1);
            for (kk, v) in [("chi2", chi), ("df", d), ("p", pv), ("deviance", m.deviance), ("nagelkerke", m.nagelkerke)] {
                set(&mut b, format!("{key}.{kk}"), v);
            }
            set_text(&mut b, format!("{key}.added"), added.join("\u{1f}"));
            rows.push(vec![count(k + 1), text(added.join(", ")), num(m.deviance, 3), num(chi, 3), num(d, 0), p(pv), num(m.nagelkerke, 3)]);
            prev = Some(m);
        }
        b.tables.push(Table { id: "change".into(), title: "Model change".into(), columns: vec!["Model".into(), "Added".into(), "−2 log likelihood".into(), "Chi-square change".into(), "df".into(), "p".into(), "Nagelkerke R²".into()], rows, footnotes: vec!["Each model adds its block to the one before; the change in −2 log likelihood is tested against the previous model. The tables below are for the full model.".into()] });
    }
    if spec.roc {
        let (pos, neg): (Vec<f64>, Vec<f64>) = { let mut a = Vec::new(); let mut c = Vec::new(); for (yy, pp) in y.iter().zip(&fit.fitted) { if *yy == 1.0 { a.push(*pp) } else { c.push(*pp) } } (a, c) };
        if let Some((auc, se)) = delong(&pos, &neg) {
            let z = prismqda_stats::dist::norm_quantile(1.0 - (1.0 - spec.level) / 2.0);
            let (lo, hi) = ((auc - z * se).max(0.0), (auc + z * se).min(1.0));
            set(&mut b, "roc.auc", auc);
            set(&mut b, "roc.se", se);
            set(&mut b, "roc.lower", lo);
            set(&mut b, "roc.upper", hi);
            let pts = roc_points(&pos, &neg);
            let alt = format!("ROC curve of the predicted probability of {ev}: sensitivity against 1 − specificity, with the diagonal of chance. AUC = {}.", crate::apa::num(auc, 3));
            b.charts.push(crate::chart::Chart { id: "roc".into(), title: "ROC curve".into(), alt: alt.clone(), svg: crate::chart::line_xy(&pts, true, "1 − specificity", "Sensitivity", &alt) });
            b.tables.push(Table { id: "roc".into(), title: "Area under the ROC curve".into(), columns: vec!["AUC".into(), "SE".into(), format!("{lv} CI lower"), format!("{lv} CI upper")], rows: vec![vec![num(auc, 3), num(se, 3), num(lo, 3), num(hi, 3)]], footnotes: vec!["The interval uses DeLong's standard error.".into()] });
            b.method_note.push_str(" ROC curve of the predicted probabilities, with the AUC's DeLong interval.");
        }
    }
    let c = &fit.classification;
    let pct = |a: usize, base: usize| if base == 0 { Cell::Empty } else { num(100.0 * a as f64 / base as f64, 1) };
    b.tables.push(Table {
        id: "classification".into(),
        title: "Classification table".into(),
        columns: vec!["Observed".into(), format!("Predicted {nonev}"), format!("Predicted {ev}"), "Percent correct".into()],
        rows: vec![
            vec![text(&nonev), count(c.true_negative), count(c.false_positive), pct(c.true_negative, c.true_negative + c.false_positive)],
            vec![text(&ev), count(c.false_negative), count(c.true_positive), pct(c.true_positive, c.true_positive + c.false_negative)],
            vec![text("Overall"), Cell::Empty, Cell::Empty, num(100.0 * c.accuracy, 1)],
        ],
        footnotes: vec!["A case is predicted to have the event when its predicted probability is .5 or more.".into()],
    });
    let epv = fit.events.min(fit.non_events) as f64 / names.len() as f64;
    set(&mut b, "epv", epv);
    b.diagnostics.push(if epv < 10.0 {
        Diagnostic { id: "events".into(), title: "Events per predictor".into(), state: DiagnosticState::ReviewNeeded, message: format!("The rarer outcome has {} cases for {} predictor terms ({} per term). With fewer than 10 per term, coefficients can be biased away from zero and their intervals unreliable.", fit.events.min(fit.non_events), names.len(), apa::num(epv, 1)) }
    } else {
        Diagnostic { id: "events".into(), title: "Events per predictor".into(), state: DiagnosticState::NoConcernDetected, message: format!("The rarer outcome has {} cases for {} predictor terms.", fit.events.min(fit.non_events), names.len()) }
    });
    if let Some(h) = &fit.hosmer_lemeshow {
        set(&mut b, "hl.chi2", h.chi2);
        set(&mut b, "hl.df", h.df);
        set(&mut b, "hl.p", h.p);
    }
    b.diagnostics.push(match &fit.hosmer_lemeshow {
        Some(h) => {
            let detail = format!("Hosmer-Lemeshow χ²({}) = {}, p {}.", apa::df(h.df), apa::num(h.chi2, 2), apa::p_clause(h.p));
            if h.p < 0.05 {
                Diagnostic { id: "fit".into(), title: "Calibration".into(), state: DiagnosticState::ReviewNeeded, message: format!("{detail} The predicted probabilities don't match the observed shares well in some groups of cases; a curved relationship or a missing interaction may be the reason.") }
            } else {
                Diagnostic { id: "fit".into(), title: "Calibration".into(), state: DiagnosticState::NoConcernDetected, message: detail }
            }
        }
        None => not_assessed("fit", "Calibration", "The Hosmer-Lemeshow test could not be computed (too few distinct predicted probabilities)."),
    });
    let big = fit.cooks_distance.iter().filter(|d| **d > 1.0).count();
    let worst = fit.cooks_distance.iter().copied().fold(0.0, f64::max);
    b.diagnostics.push(if big > 0 {
        Diagnostic { id: "influence".into(), title: "Influential cases".into(), state: DiagnosticState::ReviewNeeded, message: format!("{big} case{} {} a Cook's distance above 1 (largest {}). Check such cases for an error before relying on the coefficients.", if big == 1 { "" } else { "s" }, if big == 1 { "has" } else { "have" }, apa::num(worst, 2)) }
    } else {
        Diagnostic { id: "influence".into(), title: "Influential cases".into(), state: DiagnosticState::NoConcernDetected, message: format!("No case has a Cook's distance above 1 (largest {}).", apa::num(worst, 2)) }
    });
    b.diagnostics.push(not_assessed("linearity", "Linearity in the log odds", "Each numeric predictor is assumed to change the log odds in a straight line. Grouping a predictor into ranges and looking at the share with the event in each is a quick check."));
    Ok(b)
}

/// The area under the ROC curve (the chance a positive outranks a negative, ties half) and DeLong's standard error.
pub fn delong(pos: &[f64], neg: &[f64]) -> Option<(f64, f64)> {
    let (m, n) = (pos.len(), neg.len());
    if m < 2 || n < 2 {
        return None;
    }
    let psi = |x: f64, y: f64| if x > y { 1.0 } else if x == y { 0.5 } else { 0.0 };
    let v10: Vec<f64> = pos.iter().map(|x| neg.iter().map(|y| psi(*x, *y)).sum::<f64>() / n as f64).collect();
    let v01: Vec<f64> = neg.iter().map(|y| pos.iter().map(|x| psi(*x, *y)).sum::<f64>() / m as f64).collect();
    let auc = v10.iter().sum::<f64>() / m as f64;
    let var = |v: &[f64]| { let mu = v.iter().sum::<f64>() / v.len() as f64; v.iter().map(|a| (a - mu).powi(2)).sum::<f64>() / (v.len() - 1) as f64 };
    Some((auc, (var(&v10) / m as f64 + var(&v01) / n as f64).sqrt()))
}

/// The ROC curve's points (1 − specificity, sensitivity), from the strictest threshold to the most lenient.
fn roc_points(pos: &[f64], neg: &[f64]) -> Vec<(f64, f64)> {
    let mut t: Vec<f64> = pos.iter().chain(neg).copied().collect();
    t.sort_by(|a, b| b.total_cmp(a));
    t.dedup();
    let mut pts = vec![(0.0, 0.0)];
    for th in t {
        let tp = pos.iter().filter(|p| **p >= th).count() as f64 / pos.len() as f64;
        let fp = neg.iter().filter(|p| **p >= th).count() as f64 / neg.len() as f64;
        pts.push((fp, tp));
    }
    pts
}
