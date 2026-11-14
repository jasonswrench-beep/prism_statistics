//! Latent class analysis (checklist C4e), as R's poLCA fits it: categorical indicators assumed independent within
//! each of R latent classes, estimated by EM from many random starts (the best log-likelihood kept), classes reported
//! largest first.
//!
//! Standard errors are poLCA's: the outer product of the casewise scores in the multinomial-logit parameterization,
//! carried to the class shares and response probabilities by the delta method. Fit is poLCA's G² and Pearson χ² over
//! the response patterns (unobserved patterns add their expected counts to χ²), with df = min(N, ∏K − 1) − npar, plus
//! AIC, BIC and the relative entropy of the classification.

#![allow(clippy::needless_range_loop)]

use prismqda_stats::dist::chisq_sf;
use prismqda_stats::rng::SplitMix64;
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use super::linalg::{invert, Mat};
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Diagnostic, DiagnosticState, ResultBundle, Table};

pub const LCA: &str = "lca.model";
pub const METHOD_VERSION: u32 = 1;

fn two() -> usize {
    2
}
fn starts() -> usize {
    30
}
fn seed() -> u64 {
    20_260_101
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub variables: Vec<String>,
    #[serde(default = "two")]
    pub classes: usize,
    /// Random starts of EM.
    #[serde(default = "starts")]
    pub starts: usize,
    #[serde(default = "seed")]
    pub seed: u64,
    /// Also fit 1 to `classes` classes and compare them.
    #[serde(default)]
    pub compare: bool,
}

/// Response patterns (category indices) and their counts.
struct Data {
    patterns: Vec<Vec<usize>>,
    counts: Vec<f64>,
    k: Vec<usize>,
    n: f64,
}

#[derive(Clone)]
struct Fit {
    /// Class shares.
    pi: Vec<f64>,
    /// probs[j][r][k].
    probs: Vec<Vec<Vec<f64>>>,
    ll: f64,
    iterations: usize,
}

/// Each pattern's joint probability with each class.
fn joint(d: &Data, f: &Fit) -> Vec<Vec<f64>> {
    let r = f.pi.len();
    d.patterns.iter().map(|y| (0..r).map(|c| f.pi[c] * y.iter().enumerate().map(|(j, &k)| f.probs[j][c][k]).product::<f64>()).collect()).collect()
}

fn em(d: &Data, mut f: Fit) -> Fit {
    let r = f.pi.len();
    let mut last = f64::NEG_INFINITY;
    for it in 0..20_000 {
        let jt = joint(d, &f);
        let ll: f64 = jt.iter().zip(&d.counts).map(|(row, c)| c * row.iter().sum::<f64>().ln()).sum();
        f.ll = ll;
        f.iterations = it;
        if (ll - last).abs() < 1e-13 * ll.abs().max(1.0) {
            break;
        }
        last = ll;
        let mut pi = vec![0.0; r];
        let mut probs: Vec<Vec<Vec<f64>>> = d.k.iter().map(|&k| vec![vec![0.0; k]; r]).collect();
        for ((y, row), c) in d.patterns.iter().zip(&jt).zip(&d.counts) {
            let s: f64 = row.iter().sum();
            for cl in 0..r {
                let h = c * row[cl] / s;
                pi[cl] += h;
                for (j, &k) in y.iter().enumerate() {
                    probs[j][cl][k] += h;
                }
            }
        }
        for cl in 0..r {
            for p in probs.iter_mut() {
                let t: f64 = p[cl].iter().sum();
                p[cl].iter_mut().for_each(|x| *x /= t);
            }
            pi[cl] /= d.n;
        }
        f.pi = pi;
        f.probs = probs;
    }
    f
}

/// The best of several random starts, classes ordered by size.
fn estimate(d: &Data, r: usize, starts: usize, seed: u64) -> Option<(Fit, Vec<f64>)> {
    let mut rng = SplitMix64::new(seed);
    let mut best: Option<Fit> = None;
    let mut lls = Vec::new();
    for _ in 0..starts.max(1) {
        let probs: Vec<Vec<Vec<f64>>> = d
            .k
            .iter()
            .map(|&k| {
                (0..r)
                    .map(|_| {
                        let v: Vec<f64> = (0..k).map(|_| 0.05 + (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64).collect();
                        let t: f64 = v.iter().sum();
                        v.into_iter().map(|x| x / t).collect()
                    })
                    .collect()
            })
            .collect();
        let f = em(d, Fit { pi: vec![1.0 / r as f64; r], probs, ll: f64::NEG_INFINITY, iterations: 0 });
        lls.push(f.ll);
        if f.ll.is_finite() && best.as_ref().is_none_or(|b| f.ll > b.ll + 1e-9) {
            best = Some(f);
        }
    }
    let mut b = best?;
    let mut order: Vec<usize> = (0..r).collect();
    order.sort_by(|&a, &c| b.pi[c].total_cmp(&b.pi[a]));
    b.pi = order.iter().map(|&c| b.pi[c]).collect();
    b.probs = b.probs.iter().map(|p| order.iter().map(|&c| p[c].clone()).collect()).collect();
    Some((b, lls))
}

/// poLCA's standard errors of the class shares and of every response probability.
/// Standard errors of the class shares, and of each item's response probabilities by class and category.
type Errors = (Vec<f64>, Vec<Vec<Vec<f64>>>);

fn standard_errors(d: &Data, f: &Fit) -> Option<Errors> {
    let r = f.pi.len();
    let nlo: usize = r * d.k.iter().map(|k| k - 1).sum::<usize>();
    let m = nlo + r - 1;
    let jt = joint(d, f);
    let mut info = vec![vec![0.0; m]; m];
    for ((y, row), c) in d.patterns.iter().zip(&jt).zip(&d.counts) {
        let s: f64 = row.iter().sum();
        let h: Vec<f64> = row.iter().map(|x| x / s).collect();
        let mut sc = Vec::with_capacity(m);
        for cl in 0..r {
            for (j, &k) in d.k.iter().enumerate() {
                for kk in 1..k {
                    sc.push(h[cl] * (f64::from(u8::from(y[j] == kk)) - f.probs[j][cl][kk]));
                }
            }
        }
        for cl in 1..r {
            sc.push(h[cl] - f.pi[cl]);
        }
        for a in 0..m {
            for b in 0..m {
                info[a][b] += c * sc[a] * sc[b];
            }
        }
    }
    let vce = invert(&info).unwrap_or_else(|| super::linalg::pseudo_inverse_sym(&info));
    let mut se_probs: Vec<Vec<Vec<f64>>> = d.k.iter().map(|&k| vec![vec![0.0; k]; r]).collect();
    let mut cpos = 0;
    // Each (class, item) block depends only on its own logits.
    for cl in 0..r {
        for (j, &k) in d.k.iter().enumerate() {
            let p = &f.probs[j][cl];
            let jac: Mat = (0..k).map(|a| (1..k).map(|b| if a == b { p[a] * (1.0 - p[a]) } else { -p[a] * p[b] }).collect()).collect();
            for a in 0..k {
                let mut v = 0.0;
                for x in 0..k - 1 {
                    for y in 0..k - 1 {
                        v += jac[a][x] * vce[cpos + x][cpos + y] * jac[a][y];
                    }
                }
                se_probs[j][cl][a] = v.max(0.0).sqrt();
            }
            cpos += k - 1;
        }
    }
    let mut se_pi = vec![0.0; r];
    if r > 1 {
        let ptp = |a: usize, b: usize| if a == b { f.pi[a] * (1.0 - f.pi[a]) } else { -f.pi[a] * f.pi[b] };
        for a in 0..r {
            let mut v = 0.0;
            for x in 1..r {
                for y in 1..r {
                    v += ptp(a, x) * vce[nlo + x - 1][nlo + y - 1] * ptp(a, y);
                }
            }
            se_pi[a] = v.max(0.0).sqrt();
        }
    }
    Some((se_pi, se_probs))
}

struct Summary {
    npar: f64,
    aic: f64,
    bic: f64,
    gsq: f64,
    chisq: f64,
    df: f64,
    entropy: f64,
    modal: Vec<f64>,
}

fn summarize(d: &Data, f: &Fit) -> Summary {
    let r = f.pi.len();
    let npar = (r * d.k.iter().map(|k| k - 1).sum::<usize>() + r - 1) as f64;
    let jt = joint(d, f);
    let (mut gsq, mut chisq, mut fitted, mut ent) = (0.0, 0.0, 0.0, 0.0);
    let mut modal = vec![0.0; r];
    for (row, c) in jt.iter().zip(&d.counts) {
        let s: f64 = row.iter().sum();
        let e = d.n * s;
        gsq += 2.0 * c * (c / e).ln();
        chisq += (c - e).powi(2) / e;
        fitted += e;
        let mut top = 0;
        for cl in 0..r {
            let h = row[cl] / s;
            if h > 0.0 {
                ent -= c * h * h.ln();
            }
            if row[cl] > row[top] {
                top = cl;
            }
        }
        modal[top] += c;
    }
    chisq += d.n - fitted;
    let cells: f64 = d.k.iter().map(|&k| k as f64).product();
    let df = d.n.min(cells - 1.0) - npar;
    Summary { npar, aic: -2.0 * f.ll + 2.0 * npar, bic: -2.0 * f.ll + d.n.ln() * npar, gsq, chisq, df, entropy: if r > 1 { 1.0 - ent / (d.n * (r as f64).ln()) } else { 1.0 }, modal }
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    if spec.variables.len() < 2 {
        return Err(Error::Invalid("Choose at least two categorical indicators.".into()));
    }
    if !(1..=20).contains(&spec.classes) {
        return Err(Error::Invalid("Use 1 to 20 classes.".into()));
    }
    let cats: Vec<(Vec<Slot<usize>>, Vec<Level>)> = spec.variables.iter().map(|v| categories(ds, v)).collect::<Result<_>>()?;
    let labels: Vec<String> = spec.variables.iter().map(|v| ds.variable(v).map(|x| x.display_name().to_string())).collect::<Result<_>>()?;
    let total = ds.n_rows();
    let mut tally = Tally::default();
    let mut patterns: Vec<Vec<usize>> = Vec::new();
    let mut counts: Vec<f64> = Vec::new();
    'case: for r in 0..total {
        let mut y = Vec::with_capacity(cats.len());
        for ((slots, _), name) in cats.iter().zip(&spec.variables) {
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
    let d = Data { n: counts.iter().sum(), patterns, counts, k: k.clone() };
    let r = spec.classes;
    let note = format!(
        "Latent class analysis as R's poLCA: indicators independent within classes, estimated by EM from {} random starts (seed {}), the best log-likelihood kept; classes are numbered by size. Standard errors from the outer product of the casewise scores (multinomial-logit parameterization) by the delta method. G² and Pearson χ² over the response patterns with df = min(N, ∏K − 1) − npar; relative entropy 1 − Σ(−h log h)/(N log R). Computed by PrismStatistics.",
        spec.starts, spec.seed
    );
    let mut b = common::bundle(LCA, METHOD_VERSION, format!("Latent Class Analysis: {r} Classes"), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, d.n as usize), note);
    set(&mut b, "n", d.n);
    set(&mut b, "classes", r as f64);
    if d.n < 10.0 {
        fail(&mut b, "There are too few complete cases.");
        return Ok(b);
    }
    if k.iter().any(|&x| x < 2) {
        fail(&mut b, "Every indicator needs at least two categories in the cases analyzed.");
        return Ok(b);
    }
    let Some((f, lls)) = estimate(&d, r, spec.starts, spec.seed) else {
        fail(&mut b, "EM did not reach a finite log-likelihood.");
        return Ok(b);
    };
    let s = summarize(&d, &f);
    let ses = standard_errors(&d, &f);
    for (key, v) in [("llik", f.ll), ("npar", s.npar), ("aic", s.aic), ("bic", s.bic), ("Gsq", s.gsq), ("Chisq", s.chisq), ("resid.df", s.df), ("entropy", s.entropy)] {
        set(&mut b, key, v);
    }
    let p_of = |x: f64| if s.df > 0.0 { chisq_sf(x, s.df) } else { f64::NAN };
    set(&mut b, "Gsq.p", p_of(s.gsq));
    set(&mut b, "Chisq.p", p_of(s.chisq));
    let hits = lls.iter().filter(|l| (*l - f.ll).abs() < 1e-6 * f.ll.abs().max(1.0)).count();
    set(&mut b, "starts.best", hits as f64);
    b.tables.push(Table {
        id: "fit".into(),
        title: "Model fit".into(),
        columns: vec!["Measure".into(), "Value".into()],
        rows: vec![
            vec![text("Log-likelihood"), num(f.ll, 2)],
            vec![text("Free parameters"), count(s.npar as usize)],
            vec![text("AIC"), num(s.aic, 2)],
            vec![text("BIC"), num(s.bic, 2)],
            vec![text("G² (likelihood ratio)"), num(s.gsq, 2)],
            vec![text("Pearson χ²"), num(s.chisq, 2)],
            vec![text("Residual df"), num(s.df, 0)],
            vec![text("Relative entropy"), text(crate::apa::no_leading_zero(s.entropy, 3))],
            vec![text("Cases"), count(d.n as usize)],
        ],
        footnotes: vec![format!("{hits} of {} random starts reached the best log-likelihood. With sparse tables G² and χ² do not follow a χ² distribution, so compare models by BIC instead.", spec.starts.max(1))],
    });
    let mut class_rows = Vec::new();
    for c in 0..r {
        set(&mut b, format!("P.{}", c + 1), f.pi[c]);
        if let Some((sp, _)) = &ses {
            set(&mut b, format!("P.se.{}", c + 1), sp[c]);
        }
        set(&mut b, format!("modal.{}", c + 1), s.modal[c]);
        class_rows.push(vec![text(format!("Class {}", c + 1)), num(f.pi[c], 3), ses.as_ref().map_or(text(""), |(sp, _)| num(sp[c], 3)), count(s.modal[c] as usize)]);
    }
    b.tables.push(Table { id: "classes".into(), title: "Class sizes".into(), columns: vec!["Class".into(), "Share".into(), "SE".into(), "Cases assigned".into()], rows: class_rows, footnotes: vec!["Cases are assigned to their most probable class.".into()] });
    let mut prob_rows = Vec::new();
    for (j, (_, levels)) in cats.iter().enumerate() {
        for (kk, lv) in levels.iter().enumerate() {
            let mut row = vec![text(labels[j].clone()), text(lv.label.clone())];
            for c in 0..r {
                let key = format!("prob.{}.{}.{}", spec.variables[j], c + 1, kk + 1);
                set(&mut b, key.clone(), f.probs[j][c][kk]);
                let se = ses.as_ref().map(|(_, sp)| sp[j][c][kk]);
                if let Some(se) = se {
                    set(&mut b, format!("prob.se.{}.{}.{}", spec.variables[j], c + 1, kk + 1), se);
                }
                row.push(text(format!("{}{}", crate::apa::no_leading_zero(f.probs[j][c][kk], 3), se.map(|x| format!(" ({})", crate::apa::no_leading_zero(x, 3))).unwrap_or_default())));
            }
            prob_rows.push(row);
        }
    }
    let mut cols = vec!["Indicator".to_string(), "Category".into()];
    cols.extend((1..=r).map(|c| format!("Class {c}")));
    b.tables.push(Table { id: "probs".into(), title: "Item response probabilities".into(), columns: cols, rows: prob_rows, footnotes: vec!["Probability of each category within each class, with its standard error in parentheses.".into()] });
    if spec.compare && r > 1 {
        let mut rows = Vec::new();
        for rr in 1..=r {
            let fit = if rr == r { Some(f.clone()) } else { estimate(&d, rr, spec.starts, spec.seed).map(|x| x.0) };
            if let Some(fit) = fit {
                let sm = summarize(&d, &fit);
                set(&mut b, format!("compare.{rr}.bic"), sm.bic);
                rows.push(vec![count(rr), num(fit.ll, 2), count(sm.npar as usize), num(sm.aic, 2), num(sm.bic, 2), if rr > 1 { text(crate::apa::no_leading_zero(sm.entropy, 3)) } else { text("") }]);
            }
        }
        b.tables.push(Table { id: "compare".into(), title: "Number of classes".into(), columns: vec!["Classes".into(), "Log-likelihood".into(), "Parameters".into(), "AIC".into(), "BIC".into(), "Entropy".into()], rows, footnotes: vec!["Lower BIC is better; the model with the lowest BIC is the usual choice, with interpretability.".into()] });
    }
    b.diagnostics.push(if hits >= 2 || spec.starts <= 1 {
        Diagnostic { id: "starts".into(), title: "Local maxima".into(), state: DiagnosticState::NoConcernDetected, message: format!("The best log-likelihood was reached from {hits} of {} starts.", spec.starts) }
    } else {
        Diagnostic { id: "starts".into(), title: "Local maxima".into(), state: DiagnosticState::ReviewNeeded, message: "Only one random start reached the best log-likelihood; run again with more starts to make sure it is the maximum.".into() }
    });
    if ses.is_none() {
        b.diagnostics.push(Diagnostic { id: "se".into(), title: "Standard errors".into(), state: DiagnosticState::ReviewNeeded, message: "The information matrix is singular (some probabilities are at 0 or 1), so standard errors are not shown.".into() });
    }
    let boundary = f.probs.iter().flatten().flatten().any(|p| *p < 1e-6);
    if boundary {
        b.diagnostics.push(Diagnostic { id: "boundary".into(), title: "Boundary estimates".into(), state: DiagnosticState::ReviewNeeded, message: "Some response probabilities are at 0 or 1; their standard errors are not reliable.".into() });
    }
    b.diagnostics.push(common::size_diagnostic(d.n as usize, format!("{} complete cases", d.n as usize)));
    Ok(b)
}
