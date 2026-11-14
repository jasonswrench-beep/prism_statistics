//! Item response theory (checklist C4f), as R's mirt fits unidimensional models: the Rasch model, the two-parameter
//! logistic model and Samejima's graded response model, by marginal maximum likelihood.
//!
//! The latent trait is integrated over mirt's fixed grid of 61 points from −6 to 6 with normal weights (normalized to
//! sum to 1), so the same function is maximized; estimation is Newton's method with an analytic gradient and the
//! Hessian from it by central differences, which also gives the observed-information standard errors (mirt's Oakes
//! standard errors are the same quantity). Items are in slope-intercept form, P(Y ≥ k | θ) = 1 / (1 + exp(−(aθ + d_k))),
//! with the IRT form b_k = −d_k / a and its standard errors by the delta method. A two-category item in the graded
//! model is the 2PL. In the Rasch model every slope is 1 and the variance of θ is estimated.

use prismqda_stats::dist::norm_two_sided;
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use super::linalg::{cholesky, invert, Mat};
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Diagnostic, DiagnosticState, ResultBundle, Table};

pub const IRT: &str = "irt.model";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ItemModel {
    Rasch,
    /// The 2PL for two-category items; the graded response model for more.
    #[default]
    Graded,
}

fn level95() -> f64 {
    0.95
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub items: Vec<String>,
    #[serde(default)]
    pub model: ItemModel,
    #[serde(default = "level95")]
    pub level: f64,
}

const QUAD: usize = 61;

struct Data {
    /// Response patterns (category indices) and counts.
    patterns: Vec<Vec<usize>>,
    counts: Vec<f64>,
    k: Vec<usize>,
}

/// Parameter layout: for each item its slope (unless Rasch) then its K − 1 intercepts; then the variance (Rasch).
struct Layout {
    a: Vec<Option<usize>>,
    d: Vec<Vec<usize>>,
    var: Option<usize>,
    q: usize,
}

fn layout(k: &[usize], rasch: bool) -> Layout {
    let mut q = 0;
    let (mut a, mut d) = (Vec::new(), Vec::new());
    for &kk in k {
        if rasch {
            a.push(None);
        } else {
            a.push(Some(q));
            q += 1;
        }
        d.push((q..q + kk - 1).collect());
        q += kk - 1;
    }
    let var = rasch.then(|| {
        q += 1;
        q - 1
    });
    Layout { a, d, var, q }
}

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

/// The grid and its normalized weights for a trait variance.
fn grid(var: f64) -> (Vec<f64>, Vec<f64>) {
    let th: Vec<f64> = (0..QUAD).map(|i| -6.0 + 12.0 * i as f64 / (QUAD - 1) as f64).collect();
    let w: Vec<f64> = th.iter().map(|t| (-t * t / (2.0 * var)).exp()).collect();
    let s: f64 = w.iter().sum();
    (th, w.into_iter().map(|x| x / s).collect())
}

/// P*(k) = P(Y ≥ k) for k = 0..=K (1 at 0, 0 at K).
fn cum(a: f64, d: &[f64], t: f64) -> Vec<f64> {
    let mut v = vec![1.0];
    v.extend(d.iter().map(|dk| sigmoid(a * t + dk)));
    v.push(0.0);
    v
}

fn slope(lay: &Layout, j: usize, p: &[f64]) -> f64 {
    lay.a[j].map_or(1.0, |i| p[i])
}

/// The log-likelihood and its gradient.
fn loglik(d: &Data, lay: &Layout, p: &[f64], grad: bool) -> Option<(f64, Vec<f64>)> {
    let var = lay.var.map_or(1.0, |i| p[i]);
    if var <= 0.0 {
        return None;
    }
    let (th, pw) = grid(var);
    let m2: f64 = th.iter().zip(&pw).map(|(t, w)| t * t * w).sum();
    let items = d.k.len();
    // Category probabilities per item, grid point and category.
    let probs: Vec<Vec<Vec<f64>>> = (0..items)
        .map(|j| {
            let dd: Vec<f64> = lay.d[j].iter().map(|&i| p[i]).collect();
            th.iter().map(|&t| {
                let c = cum(slope(lay, j, p), &dd, t);
                (0..d.k[j]).map(|k| (c[k] - c[k + 1]).max(1e-300)).collect()
            }).collect()
        })
        .collect();
    let mut ll = 0.0;
    let mut g = vec![0.0; lay.q];
    for (y, n) in d.patterns.iter().zip(&d.counts) {
        let joint: Vec<f64> = (0..QUAD).map(|q| pw[q] * y.iter().enumerate().map(|(j, &k)| probs[j][q][k]).product::<f64>()).collect();
        let l: f64 = joint.iter().sum();
        if l.is_nan() || l <= 0.0 {
            return None;
        }
        ll += n * l.ln();
        if !grad {
            continue;
        }
        for q in 0..QUAD {
            let w = n * joint[q] / l;
            if w == 0.0 {
                continue;
            }
            let t = th[q];
            for (j, &k) in y.iter().enumerate() {
                let dd: Vec<f64> = lay.d[j].iter().map(|&i| p[i]).collect();
                let c = cum(slope(lay, j, p), &dd, t);
                let pk = probs[j][q][k];
                let dlow = |kk: usize| if kk >= 1 && kk < d.k[j] { c[kk] * (1.0 - c[kk]) } else { 0.0 };
                let (up, lo) = (dlow(k), dlow(k + 1));
                if k >= 1 {
                    g[lay.d[j][k - 1]] += w * up / pk;
                }
                if k + 1 < d.k[j] {
                    g[lay.d[j][k]] -= w * lo / pk;
                }
                if let Some(ai) = lay.a[j] {
                    g[ai] += w * (up - lo) * t / pk;
                }
            }
            if let Some(vi) = lay.var {
                g[vi] += w * (t * t - m2) / (2.0 * var * var);
            }
        }
    }
    Some((ll, g))
}

fn hessian(d: &Data, lay: &Layout, p: &[f64]) -> Option<Mat> {
    let q = p.len();
    let mut h = vec![vec![0.0; q]; q];
    for k in 0..q {
        let step = 1e-5 * p[k].abs().max(1.0);
        let (mut up, mut dn) = (p.to_vec(), p.to_vec());
        up[k] += step;
        dn[k] -= step;
        let (_, gu) = loglik(d, lay, &up, true)?;
        let (_, gd) = loglik(d, lay, &dn, true)?;
        for j in 0..q {
            h[j][k] = (gu[j] - gd[j]) / (2.0 * step);
        }
    }
    Some((0..q).map(|i| (0..q).map(|j| 0.5 * (h[i][j] + h[j][i])).collect()).collect())
}

fn maximize(d: &Data, lay: &Layout, mut p: Vec<f64>) -> Option<(Vec<f64>, f64, usize, bool)> {
    let (mut ll, _) = loglik(d, lay, &p, false)?;
    let q = p.len();
    let mut converged = false;
    let mut iterations = 0;
    for it in 0..500 {
        iterations = it + 1;
        let (_, g) = loglik(d, lay, &p, true)?;
        if g.iter().all(|v| v.abs() < 1e-9) {
            converged = true;
            break;
        }
        let h = hessian(d, lay, &p)?;
        let mut ridge = 0.0;
        let mut step = None;
        for _ in 0..40 {
            let m: Mat = (0..q).map(|i| (0..q).map(|j| -h[i][j] + if i == j { ridge } else { 0.0 }).collect()).collect();
            if cholesky(&m).is_some() {
                let mi = invert(&m)?;
                step = Some(mi.iter().map(|r| r.iter().zip(&g).map(|(a, b)| a * b).sum::<f64>()).collect::<Vec<f64>>());
                break;
            }
            ridge = if ridge == 0.0 { 1e-6 * (0..q).map(|i| h[i][i].abs()).fold(1.0, f64::max) } else { ridge * 10.0 };
        }
        let step = step?;
        let mut alpha = 1.0;
        let mut moved = false;
        while alpha > 1e-12 {
            let cand: Vec<f64> = p.iter().zip(&step).map(|(t, s)| t + alpha * s).collect();
            if let Some((lc, _)) = loglik(d, lay, &cand, false) {
                if lc >= ll - 1e-12 * ll.abs() {
                    converged = step.iter().all(|s| (alpha * s).abs() < 1e-11);
                    p = cand;
                    ll = lc;
                    moved = true;
                    break;
                }
            }
            alpha /= 2.0;
        }
        if !moved || converged {
            converged = converged || g.iter().all(|v| v.abs() < 1e-6);
            break;
        }
    }
    Some((p, ll, iterations, converged))
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    check_level(spec.level)?;
    if spec.items.len() < 2 {
        return Err(Error::Invalid("Choose at least two items.".into()));
    }
    let cats: Vec<(Vec<Slot<usize>>, Vec<Level>)> = spec.items.iter().map(|v| categories(ds, v)).collect::<Result<_>>()?;
    let rasch = spec.model == ItemModel::Rasch;
    if rasch && cats.iter().any(|(_, l)| l.len() != 2) {
        return Err(Error::Invalid("The Rasch model here is for two-category items; use the graded response model for items with more categories.".into()));
    }
    let total = ds.n_rows();
    let mut tally = Tally::default();
    let (mut patterns, mut counts): (Vec<Vec<usize>>, Vec<f64>) = (Vec::new(), Vec::new());
    'case: for r in 0..total {
        let mut y = Vec::with_capacity(cats.len());
        for ((slots, _), name) in cats.iter().zip(&spec.items) {
            if tally.check(&slots[r], name) {
                continue 'case;
            }
            y.push(slots[r].value().expect("checked"));
        }
        match patterns.iter().position(|p| *p == y) {
            Some(i) => counts[i] += 1.0,
            None => {
                patterns.push(y);
                counts.push(1.0);
            }
        }
    }
    let k: Vec<usize> = cats.iter().map(|(_, l)| l.len()).collect();
    let n: f64 = counts.iter().sum();
    let d = Data { patterns, counts, k: k.clone() };
    let lay = layout(&k, rasch);
    let what = match (rasch, k.iter().all(|&x| x == 2)) {
        (true, _) => "Rasch model",
        (false, true) => "two-parameter logistic (2PL) model",
        (false, false) => "graded response model",
    };
    let lv = level_text(spec.level);
    let note = format!(
        "Unidimensional {what} by marginal maximum likelihood, as R's mirt: θ integrated over 61 points from −6 to 6 with normal weights; Newton's method; standard errors from the observed information (as mirt's Oakes standard errors); z tests and {lv} Wald intervals. Slope-intercept form P(Y ≥ k) = 1/(1 + exp(−(aθ + d_k))) and IRT form b_k = −d_k/a (delta-method standard errors){}. Computed by PrismStatistics.",
        if rasch { "; slopes fixed at 1 and the variance of θ estimated" } else { "; θ standard normal" }
    );
    let title = match what {
        "Rasch model" => "Rasch Model",
        "graded response model" => "Graded Response Model",
        _ => "Two-Parameter Logistic Model",
    };
    let mut b = common::bundle(IRT, METHOD_VERSION, title.into(), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, n as usize), note);
    set(&mut b, "n", n);
    if n < 10.0 {
        fail(&mut b, "There are too few complete cases.");
        return Ok(b);
    }
    if k.iter().any(|&x| x < 2) {
        fail(&mut b, "Every item needs at least two categories in the cases analyzed.");
        return Ok(b);
    }
    // Starting values: intercepts from the cumulative proportions, slopes 1, variance 1.
    let mut p0 = vec![1.0; lay.q];
    for j in 0..k.len() {
        let mut freq = vec![0.0; k[j]];
        for (y, c) in d.patterns.iter().zip(&d.counts) {
            freq[y[j]] += c;
        }
        for kk in 1..k[j] {
            let above: f64 = freq[kk..].iter().sum::<f64>() / n;
            let pr = above.clamp(0.01, 0.99);
            p0[lay.d[j][kk - 1]] = 1.7 * (pr / (1.0 - pr)).ln();
        }
    }
    let Some((p, ll, iterations, converged)) = maximize(&d, &lay, p0) else {
        fail(&mut b, "The model could not be estimated (an item may have an empty category or separate the cases perfectly).");
        return Ok(b);
    };
    let Some(h) = hessian(&d, &lay, &p) else {
        fail(&mut b, "The information matrix could not be computed.");
        return Ok(b);
    };
    let Some(v) = invert(&h.iter().map(|r| r.iter().map(|x| -x).collect()).collect()) else {
        fail(&mut b, "The information matrix is singular, so standard errors can't be computed.");
        return Ok(b);
    };
    let npar = lay.q as f64;
    for (key, val) in [("logl", ll), ("aic", -2.0 * ll + 2.0 * npar), ("bic", -2.0 * ll + n.ln() * npar), ("npar", npar)] {
        set(&mut b, key, val);
    }
    let se = |i: usize| v[i][i].max(0.0).sqrt();
    let mut rows = Vec::new();
    for (j, name) in spec.items.iter().enumerate() {
        let a = slope(&lay, j, &p);
        set(&mut b, format!("a1.{name}"), a);
        let mut row = vec![text(ds.variable(name)?.display_name().to_string()), num(a, 3), lay.a[j].map_or(text(""), |ai| num(se(ai), 3))];
        if let Some(ai) = lay.a[j] {
            set(&mut b, format!("a1.se.{name}"), se(ai));
        }
        let mut bs = Vec::new();
        for (kk, &di) in lay.d[j].iter().enumerate() {
            let suffix = if k[j] == 2 { String::new() } else { (kk + 1).to_string() };
            let dv = p[di];
            set(&mut b, format!("d{suffix}.{name}"), dv);
            set(&mut b, format!("d{suffix}.se.{name}"), se(di));
            let bv = -dv / a;
            // Delta method: ∂b/∂d = −1/a, ∂b/∂a = d/a².
            let var_b = match lay.a[j] {
                Some(ai) => v[di][di] / (a * a) + (dv / (a * a)).powi(2) * v[ai][ai] - 2.0 * (1.0 / a) * (dv / (a * a)) * v[di][ai],
                None => v[di][di],
            };
            let seb = var_b.max(0.0).sqrt();
            set(&mut b, format!("b{suffix}.{name}"), bv);
            set(&mut b, format!("b{suffix}.se.{name}"), seb);
            set(&mut b, format!("b{suffix}.p.{name}"), norm_two_sided(bv / seb));
            bs.push(format!("{} ({})", crate::apa::num(bv, 3), crate::apa::num(seb, 3)));
        }
        row.push(text(bs.join("; ")));
        rows.push(row);
    }
    if let Some(vi) = lay.var {
        set(&mut b, "var", p[vi]);
        set(&mut b, "var.se", se(vi));
    } else {
        set(&mut b, "var", 1.0);
    }
    b.tables.push(Table {
        id: "fit".into(),
        title: "Model fit".into(),
        columns: vec!["Measure".into(), "Value".into()],
        rows: vec![
            vec![text("Log-likelihood"), num(ll, 2)],
            vec![text("Free parameters"), count(lay.q)],
            vec![text("AIC"), num(-2.0 * ll + 2.0 * npar, 2)],
            vec![text("BIC"), num(-2.0 * ll + n.ln() * npar, 2)],
            vec![text("Cases"), count(n as usize)],
        ]
        .into_iter()
        .chain(lay.var.map(|vi| vec![text("Variance of θ (SE)"), text(format!("{} ({})", crate::apa::num(p[vi], 3), crate::apa::num(se(vi), 3)))]))
        .collect(),
        footnotes: Vec::new(),
    });
    b.tables.push(Table {
        id: "items".into(),
        title: "Item parameters".into(),
        columns: vec!["Item".into(), "Discrimination a".into(), "SE".into(), if k.iter().all(|&x| x == 2) { "Difficulty b (SE)".into() } else { "Thresholds b₁, b₂, … (SE)".into() }],
        rows,
        footnotes: vec![if rasch { "In the Rasch model every discrimination is fixed at 1.".into() } else { "b is the trait level at which a response at or above each category becomes more likely than not; a is the slope at that point.".into() }],
    });
    // Curves: expected item scores, and the test information.
    let pts: Vec<f64> = (0..=80).map(|i| -4.0 + 0.1 * i as f64).collect();
    let mut item_curves = Vec::new();
    let mut info = vec![0.0; pts.len()];
    for (j, name) in spec.items.iter().enumerate() {
        let a = slope(&lay, j, &p);
        let dd: Vec<f64> = lay.d[j].iter().map(|&i| p[i]).collect();
        let mut curve = Vec::new();
        for (q, &t) in pts.iter().enumerate() {
            let c = cum(a, &dd, t);
            curve.push((t, c[1..k[j]].iter().sum::<f64>()));
            for kk in 0..k[j] {
                let pk = (c[kk] - c[kk + 1]).max(1e-300);
                let dp = c[kk] * (1.0 - c[kk]) - c[kk + 1] * (1.0 - c[kk + 1]);
                info[q] += a * a * dp * dp / pk;
            }
        }
        item_curves.push((ds.variable(name)?.display_name().to_string(), curve));
    }
    let alt_items = format!("Expected score on each of {} items across the trait, from −4 to 4.", spec.items.len());
    b.charts.push(crate::chart::Chart { id: "icc".into(), title: if k.iter().all(|&x| x == 2) { "Item characteristic curves".into() } else { "Expected item scores".into() }, svg: crate::chart::scatter_curves(&[], &item_curves, "Trait (θ)", if k.iter().all(|&x| x == 2) { "P(correct)" } else { "Expected score" }, &alt_items), alt: alt_items });
    let peak = pts.iter().zip(&info).fold((0.0, 0.0), |m, (t, i)| if *i > m.1 { (*t, *i) } else { m });
    let alt_info = format!("Test information across the trait, highest ({}) at θ = {}.", crate::apa::num(peak.1, 2), crate::apa::num(peak.0, 1));
    b.charts.push(crate::chart::Chart { id: "information".into(), title: "Test information".into(), svg: crate::chart::scatter_curves(&[], &[("Information".to_string(), pts.iter().copied().zip(info.iter().copied()).collect())], "Trait (θ)", "Information", &alt_info), alt: alt_info });
    set(&mut b, "info.peak", peak.0);
    b.diagnostics.push(if converged {
        Diagnostic { id: "convergence".into(), title: "Convergence".into(), state: DiagnosticState::NoConcernDetected, message: format!("The estimates converged in {iterations} iterations.") }
    } else {
        Diagnostic { id: "convergence".into(), title: "Convergence".into(), state: DiagnosticState::ReviewNeeded, message: "The estimation stopped before converging.".into() }
    });
    b.diagnostics.push(common::not_assessed("fit", "Item and model fit", "Limited-information fit (M2) and item fit statistics are not computed yet."));
    b.diagnostics.push(common::size_diagnostic(n as usize, format!("{} complete cases", n as usize)));
    Ok(b)
}
