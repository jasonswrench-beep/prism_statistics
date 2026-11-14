//! Predictive modeling with cross-validation (checklist D-3): penalized linear and logistic regression as R's glmnet
//! (cv.glmnet) fits them.
//!
//! The elastic-net objective, on the standardized predictors (1/N variances) with an unpenalized intercept, is
//! (1/2N) Σ (y − η)² (Gaussian) or −(1/N) Σ [yη − log(1 + e^η)] (binomial), plus λ Σⱼ [(1 − α)/2 βⱼ² + α|βⱼ|]: ridge
//! is α = 0, the lasso α = 1. As glmnet, which standardizes a Gaussian outcome internally, the ridge term of a
//! Gaussian model is divided by the outcome's standard deviation. It is solved by coordinate descent (with Newton steps for the binomial) to full
//! precision at each λ of glmnet's sequence: 100 values from λ_max = max|x̃ⱼ'(y − ȳ)|/(N max(α, 0.001)) down to
//! λ_max × 10⁻⁴ (10⁻² when there are more predictors than cases), stopping early as glmnet does when the deviance
//! explained stops changing (by less than 10⁻⁵ of itself) or passes 0.999. Coefficients are reported on the original
//! scale.
//!
//! k-fold cross-validation refits the path on each training set at the same λ values; the CV error (mean squared
//! error, or binomial deviance with probabilities kept within [10⁻⁵, 1 − 10⁻⁵]) is the fold-size-weighted mean over
//! folds, with standard error √(weighted variance / (k − 1)). λ_min minimizes it; λ_1se is the largest λ within one
//! standard error of that minimum. Folds come from a fold variable, or are drawn at random (seeded), and can be
//! repeated; out-of-fold predictions at the chosen λ give the cross-validated performance.

#![allow(clippy::needless_range_loop)]

use prismqda_stats::rng::SplitMix64;
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use super::regression::{self, Factor};
use crate::apa;
use crate::chart;
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Diagnostic, DiagnosticState, ResultBundle, Table};

pub const PREDICT: &str = "predict.penalized";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    #[default]
    Continuous,
    Binary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Penalty {
    Ridge,
    #[default]
    Lasso,
    Elastic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Choice {
    Min,
    #[default]
    #[serde(rename = "1se")]
    OneSe,
}

fn half() -> f64 {
    0.5
}
fn ten() -> usize {
    10
}
fn one() -> usize {
    1
}
fn seed() -> u64 {
    20_260_101
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub dependent: String,
    #[serde(default)]
    pub covariates: Vec<String>,
    #[serde(default)]
    pub factors: Vec<Factor>,
    #[serde(default)]
    pub outcome: Outcome,
    #[serde(default)]
    pub penalty: Penalty,
    /// The elastic-net mixing parameter (elastic only).
    #[serde(default = "half")]
    pub alpha: f64,
    #[serde(default = "ten")]
    pub folds: usize,
    #[serde(default = "one")]
    pub repeats: usize,
    #[serde(default = "seed")]
    pub seed: u64,
    /// A variable holding each case's fold (instead of random folds).
    #[serde(default)]
    pub fold_variable: Option<String>,
    /// Which λ to report: the minimum CV error, or the largest within one standard error of it.
    #[serde(default)]
    pub choice: Choice,
}

/// Standardized design: columns centered and scaled by their 1/N standard deviations.
struct Design {
    z: Vec<Vec<f64>>,
    mean: Vec<f64>,
    sd: Vec<f64>,
}

fn standardize(x: &[Vec<f64>], rows: &[usize]) -> Design {
    let n = rows.len() as f64;
    let mut z = Vec::with_capacity(x.len());
    let (mut mean, mut sd) = (Vec::new(), Vec::new());
    for c in x {
        let m = rows.iter().map(|&i| c[i]).sum::<f64>() / n;
        let s = (rows.iter().map(|&i| (c[i] - m).powi(2)).sum::<f64>() / n).sqrt();
        let s = if s > 0.0 { s } else { 1.0 };
        z.push(rows.iter().map(|&i| (c[i] - m) / s).collect());
        mean.push(m);
        sd.push(s);
    }
    Design { z, mean, sd }
}

fn soft(v: f64, t: f64) -> f64 {
    if v > t {
        v - t
    } else if v < -t {
        v + t
    } else {
        0.0
    }
}

/// One λ: coefficients on the standardized scale (intercept first), warm-started.
fn solve(z: &[Vec<f64>], y: &[f64], binary: bool, lambda: f64, alpha: f64, start: &[f64]) -> Vec<f64> {
    let n = y.len();
    let p = z.len();
    let nf = n as f64;
    let q = p + 1;
    let mut b = start.to_vec();
    // For a Gaussian outcome glmnet works with y scaled by its (1/N) standard deviation, which leaves the lasso term
    // as documented but divides the ridge term by that standard deviation.
    let ys = if binary {
        1.0
    } else {
        let m = y.iter().sum::<f64>() / nf;
        (y.iter().map(|v| (v - m).powi(2)).sum::<f64>() / nf).sqrt().max(1e-300)
    };
    let (l1, l2) = (lambda * alpha, lambda * (1.0 - alpha) / ys);
    let col = |j: usize, i: usize| if j == 0 { 1.0 } else { z[j - 1][i] };
    let eta = |b: &[f64], i: usize| (0..q).map(|j| col(j, i) * b[j]).sum::<f64>();
    for _outer in 0..200 {
        // Working response and weights (Gaussian: the data themselves, weight 1).
        let (w, zr): (Vec<f64>, Vec<f64>) = if binary {
            (0..n)
                .map(|i| {
                    let e = eta(&b, i);
                    let pr = 1.0 / (1.0 + (-e).exp());
                    let w = (pr * (1.0 - pr)).max(1e-5);
                    (w, e + (y[i] - pr) / w)
                })
                .unzip()
        } else {
            (vec![1.0; n], y.to_vec())
        };
        // The weighted Gram matrix and cross-products (column 0 is the intercept, unpenalized).
        let mut g = vec![vec![0.0; q]; q];
        let mut c = vec![0.0; q];
        for i in 0..n {
            for a in 0..q {
                let xa = col(a, i) * w[i];
                c[a] += xa * zr[i] / nf;
                for d in a..q {
                    g[a][d] += xa * col(d, i) / nf;
                }
            }
        }
        for a in 0..q {
            for d in 0..a {
                g[a][d] = g[d][a];
            }
        }
        let before = b.clone();
        let grad = |b: &[f64], j: usize| c[j] - (0..q).filter(|&k| k != j).map(|k| g[j][k] * b[k]).sum::<f64>();
        for _round in 0..1000 {
            // Coordinate descent sweeps.
            for _ in 0..200 {
                let mut change: f64 = 0.0;
                for j in 0..q {
                    let old = b[j];
                    let new = if j == 0 { grad(&b, 0) / g[0][0] } else { soft(grad(&b, j), l1) / (g[j][j] + l2) };
                    change = change.max(g[j][j] * (new - old).powi(2));
                    b[j] = new;
                }
                if change < 1e-28 {
                    break;
                }
            }
            // Polish: solve the optimality equations on the active set exactly, then check them everywhere.
            let act: Vec<usize> = (0..q).filter(|&j| j == 0 || b[j] != 0.0).collect();
            let m: Vec<Vec<f64>> = act.iter().map(|&a| act.iter().map(|&d| g[a][d] + if a == d && a != 0 { l2 } else { 0.0 }).collect()).collect();
            let rhs: Vec<f64> = act.iter().map(|&a| c[a] - if a == 0 { 0.0 } else { l1 * b[a].signum() }).collect();
            if let Some(inv) = super::linalg::invert(&m) {
                let sol: Vec<f64> = (0..act.len()).map(|r| (0..act.len()).map(|s| inv[r][s] * rhs[s]).sum()).collect();
                let mut trial = b.clone();
                for (k, &a) in act.iter().enumerate() {
                    trial[a] = sol[k];
                }
                let signs_ok = act.iter().all(|&a| a == 0 || trial[a].signum() == b[a].signum());
                let kkt_ok = (1..q).filter(|j| !act.contains(j)).all(|j| {
                    let r = c[j] - (0..q).map(|k| g[j][k] * trial[k]).sum::<f64>();
                    r.abs() <= l1 * (1.0 + 1e-12)
                });
                if signs_ok && kkt_ok {
                    b = trial;
                    break;
                }
            }
        }
        if !binary || b.iter().zip(&before).all(|(a, c)| (a - c).abs() < 1e-13 * a.abs().max(1.0)) {
            break;
        }
    }
    b
}

/// Deviance of predictions (Gaussian: residual sum of squares; binomial: −2 log-likelihood).
fn deviance(y: &[f64], eta: &[f64], binary: bool) -> f64 {
    y.iter()
        .zip(eta)
        .map(|(y, e)| {
            if binary {
                -2.0 * (y * e - (1.0 + e.exp()).ln())
            } else {
                (y - e).powi(2)
            }
        })
        .sum()
}

/// The λ sequence for these data, as glmnet makes it.
fn lambda_sequence(z: &[Vec<f64>], y: &[f64], alpha: f64) -> Vec<f64> {
    let n = y.len() as f64;
    let ybar = y.iter().sum::<f64>() / n;
    let lmax = z.iter().map(|c| c.iter().zip(y).map(|(a, b)| a * (b - ybar)).sum::<f64>().abs()).fold(0.0, f64::max) / (n * alpha.max(1e-3));
    let ratio: f64 = if y.len() < z.len() { 1e-2 } else { 1e-4 };
    (0..100).map(|k| lmax * ratio.powf(k as f64 / 99.0)).collect()
}

/// The path over a λ sequence; with `stop`, glmnet's early stopping. Returns standardized coefficients per λ.
fn path(z: &[Vec<f64>], y: &[f64], binary: bool, alpha: f64, lambdas: &[f64], stop: bool) -> Vec<Vec<f64>> {
    let n = y.len();
    let p = z.len();
    let ybar = y.iter().sum::<f64>() / n as f64;
    let mut start = vec![0.0; p + 1];
    start[0] = if binary { (ybar / (1.0 - ybar)).ln() } else { ybar };
    let null = deviance(y, &vec![start[0]; n], binary);
    let mut out: Vec<Vec<f64>> = Vec::new();
    let mut last_ratio = 0.0;
    for (m, &lam) in lambdas.iter().enumerate() {
        // As glmnet, the first point of its own sequence is the null model (it is fitted at λ = ∞).
        let b = if stop && m == 0 { start.clone() } else { solve(z, y, binary, lam, alpha, &start) };
        let eta: Vec<f64> = (0..n).map(|i| b[0] + (0..p).map(|j| z[j][i] * b[j + 1]).sum::<f64>()).collect();
        let ratio = 1.0 - deviance(y, &eta, binary) / null;
        out.push(b.clone());
        start = b;
        // glmnet's stopping rule: the deviance explained changed by less than 10⁻⁵ (relative for the Gaussian, absolute
        // for the binomial) or passed 0.999, checked from the fifth λ on.
        let small = if binary { ratio - last_ratio < 1e-5 } else { ratio - last_ratio < 1e-5 * ratio };
        if stop && m + 1 >= 5 && (small || ratio > 0.999) {
            break;
        }
        last_ratio = ratio;
    }
    out
}

/// Coefficients at λ from a path over a decreasing λ grid, by linear interpolation in λ (glmnet's lambda.interp):
/// values beyond the grid take its end.
fn interpolate(grid: &[f64], coefs: &[Vec<f64>], lam: f64) -> Vec<f64> {
    let k = grid.len();
    if k == 1 || lam >= grid[0] {
        return coefs[0].clone();
    }
    if lam <= grid[k - 1] {
        return coefs[k - 1].clone();
    }
    let r = (1..k).find(|&r| grid[r] <= lam).expect("inside the grid");
    let l = r - 1;
    let f = (lam - grid[r]) / (grid[l] - grid[r]);
    coefs[l].iter().zip(&coefs[r]).map(|(a, c)| f * a + (1.0 - f) * c).collect()
}

/// Original-scale coefficients from standardized ones.
fn unscale(b: &[f64], d: &Design) -> Vec<f64> {
    let slopes: Vec<f64> = (0..d.sd.len()).map(|j| b[j + 1] / d.sd[j]).collect();
    let b0 = b[0] - slopes.iter().zip(&d.mean).map(|(s, m)| s * m).sum::<f64>();
    std::iter::once(b0).chain(slopes).collect()
}

fn predict(beta: &[f64], x: &[Vec<f64>], i: usize) -> f64 {
    beta[0] + (0..x.len()).map(|j| beta[j + 1] * x[j][i]).sum::<f64>()
}

/// The CV loss of one case: squared error, or binomial deviance with the probability kept within [1e-5, 1 − 1e-5].
fn loss(y: f64, eta: f64, binary: bool) -> f64 {
    if binary {
        let p = (1.0 / (1.0 + (-eta).exp())).clamp(1e-5, 1.0 - 1e-5);
        -2.0 * (y * p.ln() + (1.0 - y) * (1.0 - p).ln())
    } else {
        (y - eta).powi(2)
    }
}

/// AUC by the Mann-Whitney statistic (ties count half).
fn auc(y: &[f64], score: &[f64]) -> f64 {
    let mut idx: Vec<usize> = (0..y.len()).collect();
    idx.sort_by(|&a, &b| score[a].total_cmp(&score[b]));
    let mut ranks = vec![0.0; y.len()];
    let mut i = 0;
    while i < idx.len() {
        let mut j = i;
        while j + 1 < idx.len() && score[idx[j + 1]] == score[idx[i]] {
            j += 1;
        }
        let r = (i + j) as f64 / 2.0 + 1.0;
        for k in i..=j {
            ranks[idx[k]] = r;
        }
        i = j + 1;
    }
    let n1 = y.iter().filter(|v| **v == 1.0).count() as f64;
    let n0 = y.len() as f64 - n1;
    let r1: f64 = (0..y.len()).filter(|&k| y[k] == 1.0).map(|k| ranks[k]).sum();
    (r1 - n1 * (n1 + 1.0) / 2.0) / (n1 * n0)
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    let binary = spec.outcome == Outcome::Binary;
    let reg = regression::Spec { dependent: spec.dependent.clone(), covariates: spec.covariates.clone(), factors: spec.factors.clone(), level: 0.95, blocks: Vec::new(), interactions: Vec::new(), center: false, plots: false, robust: Default::default(), bayes: Default::default(), prior_scale: Default::default() };
    let mut event = None;
    let mut pr = if binary {
        let (ys, levels) = categories(ds, &spec.dependent)?;
        if levels.len() != 2 {
            return Err(Error::Invalid(format!("{} needs exactly two categories for a binary outcome (it has {}).", ds.variable(&spec.dependent)?.display_name(), levels.len())));
        }
        event = Some(levels[1].label.clone());
        regression::prepare_with(ds, &reg, ys.iter().map(|s| match s { Slot::Valid(i) => Slot::Valid(*i as f64), Slot::Empty => Slot::Empty, Slot::UserMissing => Slot::UserMissing }).collect())?
    } else {
        regression::prepare(ds, &reg)?
    };
    // Folds from a fold variable, if given (cases without one are dropped).
    let given: Option<Vec<Slot<f64>>> = spec.fold_variable.as_ref().map(|f| numeric(ds, f)).transpose()?;
    if let Some(g) = &given {
        let keep: Vec<usize> = (0..pr.used.len()).filter(|&i| g[pr.used[i]].value().is_some()).collect();
        for _ in 0..pr.used.len() - keep.len() {
            pr.tally.add("no fold");
        }
        pr.y = keep.iter().map(|&i| pr.y[i]).collect();
        pr.columns = pr.columns.iter().map(|c| keep.iter().map(|&i| c[i]).collect()).collect();
        pr.used = keep.iter().map(|&i| pr.used[i]).collect();
    }
    let n = pr.y.len();
    let p = pr.columns.len();
    let alpha = match spec.penalty {
        Penalty::Ridge => 0.0,
        Penalty::Lasso => 1.0,
        Penalty::Elastic => spec.alpha,
    };
    let pname = match spec.penalty {
        Penalty::Ridge => "ridge".to_string(),
        Penalty::Lasso => "lasso".to_string(),
        Penalty::Elastic => format!("elastic net (α = {})", apa::num(alpha, 2)),
    };
    let note = format!(
        "Penalized {} regression ({pname}) as R's glmnet and cv.glmnet: standardized predictors, glmnet's λ sequence, coordinate descent to full precision; {}-fold cross-validation{} of the {}; λ chosen by {}. Coefficients on the original scale. Computed by PrismStatistics.",
        if binary { "logistic" } else { "linear" },
        spec.folds,
        if spec.repeats > 1 { format!(" repeated {} times", spec.repeats) } else { String::new() },
        if binary { "binomial deviance" } else { "mean squared error" },
        if spec.choice == Choice::Min { "the minimum CV error" } else { "the one-standard-error rule" }
    );
    let mut b = common::bundle(PREDICT, METHOD_VERSION, format!("Penalized {} Regression: {}", if binary { "Logistic" } else { "Linear" }, pr.y_label), serde_json::to_value(spec).expect("a spec always serializes"), pr.tally.clone().accounting(pr.total, n), note);
    set_text(&mut b, "y.label", pr.y_label.clone());
    set(&mut b, "n", n as f64);
    set(&mut b, "alpha", alpha);
    if let Some(e) = &event {
        set_text(&mut b, "event", e.clone());
    }
    if !(0.0..=1.0).contains(&alpha) {
        fail(&mut b, "The mixing parameter α must be between 0 and 1.");
        return Ok(b);
    }
    if p < 1 {
        fail(&mut b, "Add at least one predictor.");
        return Ok(b);
    }
    // Folds: (repeat, fold of each case).
    let fold_sets: Vec<(usize, Vec<usize>)> = if let Some(g) = &given {
        let raw: Vec<f64> = pr.used.iter().map(|&r| g[r].value().expect("kept")).collect();
        let mut u = raw.clone();
        u.sort_by(f64::total_cmp);
        u.dedup();
        vec![(u.len(), raw.iter().map(|v| u.iter().position(|x| x == v).expect("listed")).collect())]
    } else {
        let mut rng = SplitMix64::new(spec.seed);
        (0..spec.repeats.max(1))
            .map(|_| {
                let mut idx: Vec<usize> = (0..n).collect();
                for i in (1..n).rev() {
                    let j = rng.below(i + 1);
                    idx.swap(i, j);
                }
                let mut f = vec![0; n];
                for (pos, &i) in idx.iter().enumerate() {
                    f[i] = pos % spec.folds;
                }
                (spec.folds, f)
            })
            .collect()
    };
    let k = fold_sets[0].0;
    if k < 3 || k > n {
        fail(&mut b, format!("Use 3 to {n} folds."));
        return Ok(b);
    }
    if binary {
        let ones = pr.y.iter().filter(|v| **v == 1.0).count();
        if ones < 2 || n - ones < 2 {
            fail(&mut b, "Each outcome category needs at least two cases.");
            return Ok(b);
        }
    }
    let all: Vec<usize> = (0..n).collect();
    let d = standardize(&pr.columns, &all);
    let lambdas_full = lambda_sequence(&d.z, &pr.y, alpha);
    let fit_path = path(&d.z, &pr.y, binary, alpha, &lambdas_full, true);
    let nl = fit_path.len();
    let lambdas = &lambdas_full[..nl];
    let coefs: Vec<Vec<f64>> = fit_path.iter().map(|bs| unscale(bs, &d)).collect();
    // Cross-validation.
    let mut fold_err: Vec<Vec<f64>> = Vec::new();
    let mut fold_w: Vec<f64> = Vec::new();
    let mut oof = vec![vec![0.0; nl]; n];
    let reps = fold_sets.len() as f64;
    for (kk, f) in &fold_sets {
        for fold in 0..*kk {
            let train: Vec<usize> = (0..n).filter(|&i| f[i] != fold).collect();
            let test: Vec<usize> = (0..n).filter(|&i| f[i] == fold).collect();
            if test.is_empty() {
                continue;
            }
            let dt = standardize(&pr.columns, &train);
            let yt: Vec<f64> = train.iter().map(|&i| pr.y[i]).collect();
            // As cv.glmnet: the fold's own λ sequence (with early stopping), its coefficients then taken at the
            // full-data λ values by linear interpolation in λ (glmnet's lambda.interp).
            let fl_full = lambda_sequence(&dt.z, &yt, alpha);
            let bp = path(&dt.z, &yt, binary, alpha, &fl_full, true);
            let fl = &fl_full[..bp.len()];
            let fcoef: Vec<Vec<f64>> = bp.iter().map(|bs| unscale(bs, &dt)).collect();
            let mut errs = vec![0.0; nl];
            for (l, &lam) in lambdas.iter().enumerate() {
                let beta = interpolate(fl, &fcoef, lam);
                for &i in &test {
                    let e = predict(&beta, &pr.columns, i);
                    errs[l] += loss(pr.y[i], e, binary);
                    oof[i][l] += e / reps;
                }
            }
            fold_err.push(errs.iter().map(|e| e / test.len() as f64).collect());
            fold_w.push(test.len() as f64);
        }
    }
    let sw: f64 = fold_w.iter().sum();
    let nfold = fold_err.len() as f64;
    let cvm: Vec<f64> = (0..nl).map(|l| fold_err.iter().zip(&fold_w).map(|(e, w)| e[l] * w).sum::<f64>() / sw).collect();
    let cvsd: Vec<f64> = (0..nl).map(|l| (fold_err.iter().zip(&fold_w).map(|(e, w)| w * (e[l] - cvm[l]).powi(2)).sum::<f64>() / sw / (nfold - 1.0)).sqrt()).collect();
    let imin = (0..nl).fold(0, |a, l| if cvm[l] < cvm[a] { l } else { a });
    let i1se = (0..nl).find(|&l| cvm[l] <= cvm[imin] + cvsd[imin]).unwrap_or(imin);
    let chosen = if spec.choice == Choice::Min { imin } else { i1se };
    for (key, l) in [("min", imin), ("1se", i1se)] {
        set(&mut b, format!("lambda.{key}"), lambdas[l]);
        set(&mut b, format!("cvm.{key}"), cvm[l]);
        set(&mut b, format!("cvsd.{key}"), cvsd[l]);
        set(&mut b, format!("nonzero.{key}"), coefs[l][1..].iter().filter(|c| **c != 0.0).count() as f64);
        for j in 0..=p {
            set(&mut b, format!("coef.{key}.{j}"), coefs[l][j]);
        }
    }
    set(&mut b, "lambda.count", nl as f64);
    for l in 0..nl {
        set(&mut b, format!("path.{}.lambda", l + 1), lambdas[l]);
        set(&mut b, format!("path.{}.cvm", l + 1), cvm[l]);
        set(&mut b, format!("path.{}.cvsd", l + 1), cvsd[l]);
    }
    for j in 0..=p {
        for l in [0, nl / 2, nl - 1] {
            set(&mut b, format!("path.{}.coef.{j}", l + 1), coefs[l][j]);
        }
    }
    // Tables: λ choice, coefficients, performance.
    let err_name = if binary { "Binomial deviance" } else { "Mean squared error" };
    b.tables.push(Table {
        id: "lambda".into(),
        title: "Choice of λ".into(),
        columns: vec!["".into(), "λ".into(), "log λ".into(), format!("CV {}", err_name.to_lowercase()), "SE".into(), "Nonzero coefficients".into()],
        rows: [("Minimum CV error (λ_min)", imin), ("One-SE rule (λ_1se)", i1se)]
            .iter()
            .map(|(label, l)| vec![text(*label), num(lambdas[*l], 5), num(lambdas[*l].ln(), 3), num(cvm[*l], 4), num(cvsd[*l], 4), count(coefs[*l][1..].iter().filter(|c| **c != 0.0).count())])
            .collect(),
        footnotes: vec![format!("{k}-fold cross-validation{} over {nl} values of λ; the model below uses {}.", if fold_sets.len() > 1 { format!(", repeated {} times", fold_sets.len()) } else { String::new() }, if spec.choice == Choice::Min { "λ_min" } else { "λ_1se, the most regularized model within one standard error of the best" })],
    });
    let names: Vec<String> = std::iter::once("(Intercept)".to_string()).chain(pr.names.iter().cloned()).collect();
    let mut crow = Vec::new();
    for j in 0..=p {
        let c = coefs[chosen][j];
        let std = if j == 0 { f64::NAN } else { fit_path[chosen][j] };
        set_text(&mut b, format!("term.{j}"), names[j].clone());
        crow.push(vec![text(names[j].clone()), if c == 0.0 && j > 0 { text("·") } else { num(c, 4) }, if j == 0 || std == 0.0 { text("") } else { num(std, 4) }, if binary && j > 0 && c != 0.0 { num(c.exp(), 3) } else { text("") }]);
    }
    b.tables.push(Table {
        id: "coefficients".into(),
        title: format!("Coefficients at {}", if spec.choice == Choice::Min { "λ_min" } else { "λ_1se" }),
        columns: if binary { vec!["Term".into(), "Coefficient".into(), "Standardized".into(), "Odds ratio".into()] } else { vec!["Term".into(), "Coefficient".into(), "Standardized".into(), "".into()] },
        rows: crow,
        footnotes: vec!["A dot marks a coefficient the penalty set to zero. Penalized coefficients are shrunk toward zero and have no standard errors or p values; their job is prediction.".into(), if !pr.references.is_empty() { format!("Reference categories: {}.", pr.references.join("; ")) } else { String::new() }].into_iter().filter(|s| !s.is_empty()).collect(),
    });
    // Cross-validated performance of the chosen λ (out-of-fold predictions).
    let pred: Vec<f64> = (0..n).map(|i| oof[i][chosen]).collect();
    let train_eta: Vec<f64> = (0..n).map(|i| predict(&coefs[chosen], &pr.columns, i)).collect();
    let mut perf = Vec::new();
    let mut metrics = |label: &str, key: &str, cv: f64, tr: f64, dec: u8, b: &mut ResultBundle| {
        set(b, format!("cv.{key}"), cv);
        set(b, format!("train.{key}"), tr);
        perf.push(vec![text(label), num(cv, dec), num(tr, dec)]);
    };
    if binary {
        let prob = |e: f64| 1.0 / (1.0 + (-e).exp());
        let acc = |eta: &[f64]| (0..n).filter(|&i| (prob(eta[i]) >= 0.5) == (pr.y[i] == 1.0)).count() as f64 / n as f64;
        let brier = |eta: &[f64]| (0..n).map(|i| (prob(eta[i]) - pr.y[i]).powi(2)).sum::<f64>() / n as f64;
        metrics("AUC", "auc", auc(&pr.y, &pred), auc(&pr.y, &train_eta), 3, &mut b);
        metrics("Accuracy (cutoff .5)", "accuracy", acc(&pred), acc(&train_eta), 3, &mut b);
        metrics("Brier score", "brier", brier(&pred), brier(&train_eta), 4, &mut b);
    } else {
        let ybar = pr.y.iter().sum::<f64>() / n as f64;
        let sst: f64 = pr.y.iter().map(|y| (y - ybar).powi(2)).sum();
        let sse = |eta: &[f64]| (0..n).map(|i| (pr.y[i] - eta[i]).powi(2)).sum::<f64>();
        let mae = |eta: &[f64]| (0..n).map(|i| (pr.y[i] - eta[i]).abs()).sum::<f64>() / n as f64;
        metrics("RMSE", "rmse", (sse(&pred) / n as f64).sqrt(), (sse(&train_eta) / n as f64).sqrt(), 4, &mut b);
        metrics("MAE", "mae", mae(&pred), mae(&train_eta), 4, &mut b);
        metrics("R²", "r2", 1.0 - sse(&pred) / sst, 1.0 - sse(&train_eta) / sst, 3, &mut b);
    }
    b.tables.push(Table { id: "performance".into(), title: "Predictive performance".into(), columns: vec!["Measure".into(), "Cross-validated".into(), "In the training data".into()], rows: perf, footnotes: vec!["Cross-validated values use each case's prediction from the model fitted without its fold (averaged over repeats); training values are optimistic.".into()] });
    // Charts.
    let logl: Vec<f64> = lambdas.iter().map(|l| l.ln()).collect();
    let curves = [
        (err_name.to_string(), logl.iter().zip(&cvm).map(|(a, c)| (*a, *c)).collect::<Vec<_>>()),
        ("± 1 SE".to_string(), logl.iter().zip(cvm.iter().zip(&cvsd)).map(|(a, (c, s))| (*a, c + s)).collect()),
        ("".to_string(), logl.iter().zip(cvm.iter().zip(&cvsd)).map(|(a, (c, s))| (*a, c - s)).collect()),
    ];
    let alt = format!("Cross-validated {} against log λ; it is lowest at log λ = {} and the one-SE choice is log λ = {}.", err_name.to_lowercase(), apa::num(logl[imin], 2), apa::num(logl[i1se], 2));
    b.charts.push(chart::Chart { id: "cv".into(), title: "Cross-validation curve".into(), svg: chart::scatter_curves(&[], &curves, "log λ", &format!("CV {}", err_name.to_lowercase()), &alt), alt });
    let paths: Vec<(String, Vec<(f64, f64)>)> = (1..=p).map(|j| (names[j].clone(), logl.iter().zip(&fit_path).map(|(a, bs)| (*a, bs[j])).collect())).collect();
    let alt2 = format!("Standardized coefficient paths of {p} predictors against log λ: each shrinks toward zero as λ grows.");
    b.charts.push(chart::Chart { id: "path".into(), title: "Coefficient paths".into(), svg: chart::scatter_curves(&[], &paths, "log λ", "Standardized coefficient", &alt2), alt: alt2 });
    if chosen == 0 {
        b.diagnostics.push(Diagnostic { id: "null".into(), title: "No predictive signal".into(), state: DiagnosticState::ReviewNeeded, message: "The chosen λ removes every predictor: by cross-validation, none of them improves prediction over the mean.".into() });
    }
    if n < 10 * k {
        b.diagnostics.push(Diagnostic { id: "folds".into(), title: "Fold size".into(), state: DiagnosticState::ReviewNeeded, message: format!("With {n} cases, each of the {k} folds holds about {} cases, so the CV error is noisy; repeating the cross-validation steadies it.", n / k) });
    }
    b.diagnostics.push(common::size_diagnostic(n, format!("{n} cases")));
    Ok(b)
}
