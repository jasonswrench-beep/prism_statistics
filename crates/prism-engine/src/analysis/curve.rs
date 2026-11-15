//! Curve estimation (`regression.curve`, checklist A12): one numeric outcome against one predictor, fitted as each chosen
//! curve (linear, logarithmic, inverse, quadratic, cubic, power, exponential, S). Each curve is the least-squares fit of
//! the transformed outcome on the transformed predictor (R's `lm`), so the power, exponential and S curves are fitted on
//! ln(y) and their R squared is on that scale; R squared on the outcome's own scale is shown beside it. Cases are used
//! when both variables are valid. Computed by `prismqda_stats::curve`.

use prismqda_stats::curve::{fit, predict, CurveError, CurveFit, CurveKind};
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::chart::{self, Chart};
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Cell, ResultBundle, Table};

pub const METHOD_ID: &str = "regression.curve";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    /// One result per outcome.
    pub dependent: Vec<String>,
    pub independent: String,
    #[serde(default = "default_models")]
    pub models: Vec<CurveKind>,
    /// The scatterplot with every fitted curve.
    #[serde(default = "yes")]
    pub plot: bool,
    #[serde(default = "default_level")]
    pub level: f64,
}

fn default_models() -> Vec<CurveKind> {
    vec![CurveKind::Linear]
}

fn yes() -> bool {
    true
}

fn default_level() -> f64 {
    0.95
}

/// The curve's name: "Quadratic".
pub fn name(kind: CurveKind) -> &'static str {
    match kind {
        CurveKind::Linear => "Linear",
        CurveKind::Logarithmic => "Logarithmic",
        CurveKind::Inverse => "Inverse",
        CurveKind::Quadratic => "Quadratic",
        CurveKind::Cubic => "Cubic",
        CurveKind::Power => "Power",
        CurveKind::Exponential => "Exponential",
        CurveKind::SCurve => "S",
    }
}

/// The key written to results: "s_curve".
fn key(kind: CurveKind) -> String {
    serde_json::to_value(kind).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// The fitted equation in its own form, with the constants: "y = 2.10 + 0.53 ln(x)".
fn equation(kind: CurveKind, c: &[f64]) -> String {
    let n = |v: f64| crate::apa::num(v, 3);
    let signed = |v: f64| if v < 0.0 { format!("− {}", crate::apa::num(-v, 3)) } else { format!("+ {}", n(v)) };
    match kind {
        CurveKind::Linear => format!("y = {} {} x", n(c[0]), signed(c[1])),
        CurveKind::Logarithmic => format!("y = {} {} ln(x)", n(c[0]), signed(c[1])),
        CurveKind::Inverse => format!("y = {} {} / x", n(c[0]), signed(c[1])),
        CurveKind::Quadratic => format!("y = {} {} x {} x²", n(c[0]), signed(c[1]), signed(c[2])),
        CurveKind::Cubic => format!("y = {} {} x {} x² {} x³", n(c[0]), signed(c[1]), signed(c[2]), signed(c[3])),
        CurveKind::Power => format!("y = {} x^{}", n(c[0]), n(c[1])),
        CurveKind::Exponential => format!("y = {} e^({} x)", n(c[0]), n(c[1])),
        CurveKind::SCurve => format!("y = e^({} {} / x)", n(c[0]), signed(c[1])),
    }
}

/// The names of the regression's terms after the constant, on the fitted (transformed) scale.
fn term_names(kind: CurveKind) -> &'static [&'static str] {
    match kind {
        CurveKind::Linear | CurveKind::Exponential => &["x"],
        CurveKind::Logarithmic | CurveKind::Power => &["ln(x)"],
        CurveKind::Inverse | CurveKind::SCurve => &["1/x"],
        CurveKind::Quadratic => &["x", "x²"],
        CurveKind::Cubic => &["x", "x²", "x³"],
    }
}

fn why(e: CurveError) -> &'static str {
    match e {
        CurveError::TooFewCases => "there are too few cases for this curve",
        CurveError::NotFinite => "a value is not a finite number",
        CurveError::NeedsPositiveX => "it takes the logarithm of the predictor, which must be above zero in every case",
        CurveError::NeedsNonzeroX => "it divides by the predictor, which must not be zero in any case",
        CurveError::NeedsPositiveY => "it takes the logarithm of the outcome, which must be above zero in every case",
        CurveError::NoVariationInOutcome => "the outcome does not vary",
        CurveError::TooFewValues => "the predictor takes too few different values (a quadratic needs three, a cubic four)",
    }
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<Vec<ResultBundle>> {
    if spec.dependent.is_empty() {
        return Err(Error::Invalid("Choose at least one dependent variable.".into()));
    }
    if spec.models.is_empty() {
        return Err(Error::Invalid("Choose at least one curve to fit.".into()));
    }
    check_level(spec.level)?;
    spec.dependent.iter().map(|name| run_one(ds, spec, name)).collect()
}

fn run_one(ds: &Dataset, spec: &Spec, outcome: &str) -> Result<ResultBundle> {
    if ds.find(outcome) == ds.find(&spec.independent) {
        return Err(Error::Invalid("The independent variable can't also be a dependent variable.".into()));
    }
    let y_var = ds.variable(outcome)?;
    let x_var = ds.variable(&spec.independent)?;
    let (yl, xl) = (y_var.display_name().to_string(), x_var.display_name().to_string());
    let (ys, xs) = (numeric(ds, outcome)?, numeric(ds, &spec.independent)?);
    let mut tally = Tally::default();
    let (mut x, mut y) = (Vec::new(), Vec::new());
    for (yc, xc) in ys.iter().zip(&xs) {
        if tally.check(yc, "dependent variable") || tally.check(xc, "independent variable") {
            continue;
        }
        y.push(yc.value().expect("checked"));
        x.push(xc.value().expect("checked"));
    }
    let lv = level_text(spec.level);
    let mut models: Vec<CurveKind> = Vec::new();
    for m in &spec.models {
        if !models.contains(m) {
            models.push(*m);
        }
    }
    let note = format!(
        "Curve estimation: each curve is a least-squares regression of the outcome (on its natural logarithm for the power, exponential and S curves) on the transformed predictor, with {lv} confidence intervals for the coefficients. Cases were used when both variables had valid values. Computed by PrismStatistics."
    );
    let mut b = common::bundle(
        METHOD_ID,
        METHOD_VERSION,
        format!("Curve Estimation: {yl} by {xl}"),
        serde_json::to_value(spec).expect("a spec always serializes"),
        tally.accounting(ys.len(), y.len()),
        note,
    );
    set_text(&mut b, "y.label", yl.clone());
    set_text(&mut b, "x.label", xl.clone());
    set(&mut b, "n", y.len() as f64);
    set(&mut b, "level", spec.level);
    set(&mut b, "models", models.len() as f64);

    let fits: Vec<std::result::Result<CurveFit, CurveError>> = models.iter().map(|k| fit(*k, &x, &y, spec.level)).collect();
    let mut summary = Vec::new();
    let mut coefs = Vec::new();
    let mut failed = Vec::new();
    for (i, (kind, f)) in models.iter().zip(&fits).enumerate() {
        let k = format!("model.{}", i + 1);
        set_text(&mut b, format!("{k}.kind"), key(*kind));
        set_text(&mut b, format!("{k}.label"), name(*kind));
        let f = match f {
            Ok(f) => f,
            Err(e) => {
                failed.push(format!("{} ({})", name(*kind), why(*e)));
                let mut row = vec![text(name(*kind))];
                row.extend(std::iter::repeat_n(Cell::Empty, 10));
                summary.push(row);
                continue;
            }
        };
        let reg = &f.regression;
        for (field, v) in [
            ("r2", reg.r_squared),
            ("adj_r2", reg.adj_r_squared),
            ("F", reg.f.f),
            ("df1", reg.f.df1),
            ("df2", reg.f.df2),
            ("p", reg.f.p),
            ("r2_original", f.r_squared_original),
            ("rmse_original", f.rmse_original),
        ] {
            set(&mut b, format!("{k}.{field}"), v);
        }
        for (j, c) in f.constants.iter().enumerate() {
            set(&mut b, format!("{k}.b{j}"), *c);
        }
        set_text(&mut b, format!("{k}.equation"), equation(*kind, &f.constants));
        let mut row = vec![
            text(name(*kind)),
            num(reg.r_squared, 3),
            num(f.r_squared_original, 3),
            num(reg.f.f, 3),
            num(reg.f.df1, 0),
            num(reg.f.df2, 0),
            p(reg.f.p),
        ];
        for j in 0..4 {
            row.push(f.constants.get(j).map_or(Cell::Empty, |c| num(*c, 4)));
        }
        summary.push(row);
        let names = std::iter::once("(Constant)").chain(term_names(*kind).iter().copied());
        for (j, (t, tn)) in reg.terms.iter().zip(names).enumerate() {
            set(&mut b, format!("{k}.term.{j}.se"), t.se);
            set(&mut b, format!("{k}.term.{j}.p"), t.p);
            coefs.push(vec![
                if j == 0 { text(name(*kind)) } else { Cell::Empty },
                text(if j == 0 { "(Constant)".to_string() } else if kind.log_outcome() { format!("{tn} (on ln y)") } else { tn.to_string() }),
                num(t.estimate, 4),
                num(t.se, 4),
                num(t.t, 3),
                p(t.p),
                num(t.ci.0, 4),
                num(t.ci.1, 4),
            ]);
        }
    }
    let mut footnotes = vec![format!("The dependent variable is {yl}; the independent variable is {xl}.")];
    if models.iter().any(|m| m.log_outcome()) {
        footnotes.push("The power, exponential and S curves are fitted on ln(y): their R² and F test are on that scale. R² on the original scale is one minus the squared differences from the curve over the squared differences from the mean. For the power and exponential curves the constant is a = e^b0.".into());
    }
    if !failed.is_empty() {
        footnotes.push(format!("Not fitted: {}.", failed.join("; ")));
    }
    b.tables.push(Table {
        id: "summary".into(),
        title: "Model summary and parameter estimates".into(),
        columns: vec![
            "Equation".into(),
            "R²".into(),
            "R² (original scale)".into(),
            "F".into(),
            "df1".into(),
            "df2".into(),
            "p".into(),
            "Constant".into(),
            "b1".into(),
            "b2".into(),
            "b3".into(),
        ],
        rows: summary,
        footnotes,
    });
    if !coefs.is_empty() {
        b.tables.push(Table {
            id: "coefficients".into(),
            title: "Coefficients".into(),
            columns: vec!["Equation".into(), "Term".into(), "B".into(), "SE".into(), "t".into(), "p".into(), format!("{lv} CI lower"), format!("{lv} CI upper")],
            rows: coefs,
            footnotes: vec!["Coefficients are on the scale each curve is fitted on (ln y for the power, exponential and S curves).".into()],
        });
    }
    if fits.iter().all(|f| f.is_err()) {
        fail(&mut b, format!("No curve could be fitted: {}.", failed.join("; ")));
        return Ok(b);
    }

    if spec.plot {
        let (lo, hi) = x.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, c), v| (a.min(*v), c.max(*v)));
        let curves: Vec<(String, Vec<(f64, f64)>)> = models
            .iter()
            .zip(&fits)
            .filter_map(|(kind, f)| f.as_ref().ok().map(|f| (kind, f)))
            .map(|(kind, f)| {
                let coef: Vec<f64> = f.regression.terms.iter().map(|t| t.estimate).collect();
                let pts = (0..=160)
                    .map(|s| lo + (hi - lo) * s as f64 / 160.0)
                    .filter(|xv| !(matches!(kind, CurveKind::Inverse | CurveKind::SCurve) && *xv == 0.0))
                    .map(|xv| (xv, predict(*kind, &coef, xv)))
                    .collect();
                (name(*kind).to_string(), pts)
            })
            .collect();
        let pts: Vec<(f64, f64)> = x.iter().copied().zip(y.iter().copied()).collect();
        let alt = format!("Scatterplot of {yl} against {xl} with the fitted {} curves.", models.iter().map(|m| name(*m).to_lowercase()).collect::<Vec<_>>().join(", "));
        b.charts.push(Chart { id: "curves".into(), title: format!("{yl} by {xl}, with fitted curves"), alt: alt.clone(), svg: chart::scatter_curves(&pts, &curves, &xl, &yl, &alt) });
    }
    b.diagnostics.push(size_diagnostic(y.len(), format!("{} cases with both values", y.len())));
    b.diagnostics.push(not_assessed(
        "shape",
        "Choosing a curve",
        "A curve with more terms always fits at least as well. Prefer the simplest curve that follows the scatterplot, and one the theory behind the data supports.",
    ));
    Ok(b)
}
