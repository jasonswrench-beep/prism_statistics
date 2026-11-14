//! Structural equation modeling (checklist Part C), covariance-based, by maximum likelihood.
//!
//! A model is written in lavaan's syntax (see [`syntax`]). The engine builds lavaan's parameter table with its defaults
//! ([`model`]), estimates by maximum likelihood ([`estimate`]), and reports lavaan's fit indices, the unstandardized
//! and completely standardized ("std.all") solutions with delta-method standard errors, R² for every endogenous
//! variable, and modification indices.

pub mod boot;
pub mod diagram;
pub mod estimate;
pub mod invariance;
pub mod mcmc;
pub mod model;
pub mod ordinal;
pub mod robust;
pub mod syntax;
pub mod twolevel;

use prismqda_stats::dist::{chisq_sf, norm_quantile, norm_two_sided};
use serde::{Deserialize, Serialize};

use self::estimate::{Block, FitError};
use self::model::{Matrix, Model, Options, Param, Place};
use self::robust::H1;
use self::syntax::Expr;
use super::common::{self, *};
use super::linalg::{invert, Mat};
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Diagnostic, DiagnosticState, ResultBundle, Table};

pub const SEM: &str = "sem.model";
pub const METHOD_VERSION: u32 = 1;

fn default_level() -> f64 {
    0.95
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    /// The model in lavaan's syntax.
    pub model: String,
    /// Identify factors by fixing their variances at 1 (instead of fixing each first loading at 1).
    #[serde(default)]
    pub std_lv: bool,
    /// Estimate intercepts and means.
    #[serde(default)]
    pub meanstructure: bool,
    /// Treat observed predictors that are not modeled as fixed (their moments at the sample values), as lavaan does.
    #[serde(default = "yes")]
    pub fixed_x: bool,
    #[serde(default = "default_level")]
    pub level: f64,
    #[serde(default = "yes")]
    pub modindices: bool,
    #[serde(default)]
    pub estimator: Estimator,
    /// MLM only: the information the Satorra-Bentler correction uses.
    #[serde(default)]
    pub h1_information: H1Information,
    #[serde(default)]
    pub missing: Missing,
    /// A grouping variable for a multigroup model (groups in order of first appearance, as lavaan orders them).
    #[serde(default)]
    pub group: Option<String>,
    /// Parameter kinds held equal across groups (lavaan's group.equal).
    #[serde(default)]
    pub group_equal: Vec<String>,
    /// Parameters exempt from `group_equal` (lavaan's group.partial), such as "x3 ~1".
    #[serde(default)]
    pub group_partial: Vec<String>,
    /// Composite reliability, AVE and HTMT for factors measured by observed indicators.
    #[serde(default = "yes")]
    pub reliability: bool,
    /// Bootstrap resamples for standard errors and intervals (0: none).
    #[serde(default)]
    pub bootstrap: usize,
    /// Bias-corrected percentile intervals instead of plain percentile ones.
    #[serde(default)]
    pub bias_corrected: bool,
    #[serde(default = "default_seed")]
    pub seed: u64,
    /// With WLSMV, the ordinal variables (every observed variable when empty).
    #[serde(default)]
    pub ordered: Vec<String>,
    /// Positions of the path diagram's variables (from the diagram editor); others are laid out automatically.
    #[serde(default)]
    pub layout: std::collections::BTreeMap<String, [f64; 2]>,
    /// A latent growth model, as lavaan's growth(): observed intercepts fixed at 0, factor means estimated.
    #[serde(default)]
    pub growth: bool,
    /// The cluster variable of a two-level model (one written with level: 1 and level: 2).
    #[serde(default)]
    pub cluster: Option<String>,
    /// Bayesian estimation: chains, warm-up and sampled draws per chain (blavaan's defaults).
    #[serde(default = "three")]
    pub chains: usize,
    #[serde(default = "five_hundred")]
    pub burnin: usize,
    #[serde(default = "thousand")]
    pub samples: usize,
}

fn three() -> usize {
    3
}
fn five_hundred() -> usize {
    500
}
fn thousand() -> usize {
    1000
}

fn default_seed() -> u64 {
    20_260_101
}

/// ML, or ML with robust standard errors and a scaled test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Estimator {
    #[default]
    Ml,
    /// Satorra-Bentler: robust standard errors from the expected information, scaled χ².
    Mlm,
    /// Yuan-Bentler (the variant lavaan uses for MLR): robust (Huber-White) standard errors from the observed information, scaled χ².
    Mlr,
    /// Ordinal indicators: polychoric correlations, diagonally weighted least squares, robust standard errors and the
    /// mean- and variance-adjusted χ² (lavaan's default for ordered variables).
    Wlsmv,
    /// Bayesian estimation by MCMC with blavaan's default priors.
    Bayes,
}

/// Where the normal-theory weight of the Satorra-Bentler (MLM) correction is evaluated: at the model-implied moments
/// (lavaan's default) or at the sample moments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum H1Information {
    #[default]
    Structured,
    Unstructured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Missing {
    #[default]
    Listwise,
    /// Full-information maximum likelihood: every case with any value, under missing at random.
    Fiml,
}

impl Default for Spec {
    fn default() -> Self {
        Spec { model: String::new(), std_lv: false, meanstructure: false, fixed_x: true, level: 0.95, modindices: true, estimator: Estimator::Ml, h1_information: H1Information::Structured, missing: Missing::Listwise, group: None, group_equal: Vec::new(), group_partial: Vec::new(), reliability: true, bootstrap: 0, bias_corrected: false, seed: default_seed(), ordered: Vec::new(), layout: Default::default(), growth: false, cluster: None, chains: 3, burnin: 500, samples: 1000 }
    }
}

/// Covariance (divisor n) and means of complete rows.
fn moments(rows: &[Vec<f64>]) -> (Mat, Vec<f64>) {
    let n = rows.len() as f64;
    let p = rows.first().map_or(0, Vec::len);
    let mean: Vec<f64> = (0..p).map(|j| rows.iter().map(|r| r[j]).sum::<f64>() / n).collect();
    let mut cov = vec![vec![0.0; p]; p];
    for r in rows {
        for i in 0..p {
            for j in 0..p {
                cov[i][j] += (r[i] - mean[i]) * (r[j] - mean[j]) / n;
            }
        }
    }
    (cov, mean)
}

fn log_det(m: &Mat) -> Option<f64> {
    let l = super::linalg::cholesky(m)?;
    Some(2.0 * l.iter().enumerate().map(|(i, r)| r[i].ln()).sum::<f64>())
}

/// Writes an expression back as text.
pub(crate) fn show(e: &Expr) -> String {
    fn prec(e: &Expr) -> u8 {
        match e {
            Expr::Bin('+' | '-', ..) => 1,
            Expr::Bin('*' | '/', ..) => 2,
            Expr::Neg(_) => 3,
            Expr::Bin(..) => 4,
            _ => 5,
        }
    }
    fn wrap(e: &Expr, min: u8) -> String {
        if prec(e) < min {
            format!("({})", show(e))
        } else {
            show(e)
        }
    }
    match e {
        Expr::Num(v) => crate::apa::num(*v, if v.fract() == 0.0 { 0 } else { 3 }),
        Expr::Name(n) => n.clone(),
        Expr::Neg(x) => format!("-{}", wrap(x, 3)),
        Expr::Call(f, x) => format!("{f}({})", show(x)),
        Expr::Bin(op @ ('+' | '-'), a, b) => format!("{} {op} {}", wrap(a, 1), wrap(b, 2)),
        Expr::Bin(op, a, b) => format!("{}{op}{}", wrap(a, prec(e)), wrap(b, prec(e) + 1)),
    }
}

/// The baseline (independence) model: free variances (and means), with fixed.x moments still fixed.
fn baseline_of(model: &Model) -> Model {
    let mut b = Model { obs: model.obs.clone(), lat: Vec::new(), groups: model.groups, params: Vec::new(), nfree: 0, defined: Vec::new(), meanstructure: model.meanstructure, xvars: model.xvars.clone(), ordinal: Vec::new() };
    let p = model.p();
    // As lavaan, the observed predictors (they predict and are not predicted) keep their covariances in the baseline:
    // fixed at the sample values with fixed.x, free without.
    let exo: Vec<usize> = (0..p)
        .filter(|&k| {
            let a = |f: &dyn Fn(&Param) -> bool| model.params.iter().any(|q| q.mat == Matrix::A && f(q));
            a(&|q| q.j == k) && !a(&|q| q.i == k)
        })
        .collect();
    for g in 0..model.groups {
        for i in 0..p {
            for j in i..p {
                let x = model.xvars.contains(&i) && model.xvars.contains(&j);
                if i != j && !x && !(exo.contains(&i) && exo.contains(&j)) {
                    continue;
                }
                let free = if x {
                    None
                } else {
                    b.nfree += 1;
                    Some(b.nfree - 1)
                };
                b.params.push(Param { group: g, lhs: model.obs[i].clone(), op: "~~", rhs: model.obs[j].clone(), label: None, free, value: 0.0, start_given: false, user: false, exo: x, mat: Matrix::S, i, j });
            }
            if model.meanstructure {
                let x = model.xvars.contains(&i);
                let free = if x {
                    None
                } else {
                    b.nfree += 1;
                    Some(b.nfree - 1)
                };
                b.params.push(Param { group: g, lhs: model.obs[i].clone(), op: "~1", rhs: String::new(), label: None, free, value: 0.0, start_given: false, user: false, exo: x, mat: Matrix::M, i, j: 0 });
            }
        }
    }
    b
}

/// The value of every labeled parameter (and defined parameter) from per-parameter values.
fn label_value<'a>(model: &'a Model, values: &'a [f64], defined: &'a [f64]) -> impl Fn(&str) -> Option<f64> + 'a {
    move |n: &str| {
        model.params.iter().position(|p| p.label.as_deref() == Some(n)).map(|k| values[k]).or_else(|| model.defined.iter().position(|(d, _)| d == n).and_then(|k| defined.get(k).copied()))
    }
}

/// Each parameter's value at θ, followed by each defined parameter's.
fn unstandardized(model: &Model, th: &[f64]) -> Vec<f64> {
    let im = if model.ordinal.is_empty() { None } else { model.implied_full(0, th) };
    let mut v: Vec<f64> = model
        .params
        .iter()
        .map(|p| match &im {
            // Delta parameterization: residual variances of ordinal variables follow from the other parameters.
            Some(im) if p.mat == Matrix::S && p.i == p.j && model.ordinal.contains(&p.i) => im.s[p.i][p.i],
            _ => p.free.map_or(p.value, |k| th[k]),
        })
        .collect();
    let mut defs: Vec<f64> = Vec::new();
    for (_, e) in &model.defined {
        let x = e.eval(&label_value(model, &v, &defs)).unwrap_or(f64::NAN);
        defs.push(x);
    }
    v.extend(defs);
    v
}

/// The completely standardized solution ("std.all") at θ, with defined parameters evaluated on standardized values.
fn standardized(model: &Model, th: &[f64]) -> Vec<f64> {
    let imp: Vec<_> = (0..model.groups).map(|g| model.implied_full(g, th)).collect();
    let mut v: Vec<f64> = model
        .params
        .iter()
        .map(|p| {
            let Some(im) = &imp[p.group] else { return f64::NAN };
            let val = if p.mat == Matrix::S && p.i == p.j && model.ordinal.contains(&p.i) { im.s[p.i][p.i] } else { p.free.map_or(p.value, |k| th[k]) };
            let sd = |i: usize| im.full[i][i].sqrt();
            match p.mat {
                Matrix::A => val * sd(p.j) / sd(p.i),
                Matrix::S if p.i == p.j => val / im.full[p.i][p.i],
                Matrix::S => val / (im.s[p.i][p.i] * im.s[p.j][p.j]).sqrt(),
                Matrix::M | Matrix::T => val / sd(p.i),
            }
        })
        .collect();
    let mut defs: Vec<f64> = Vec::new();
    for (_, e) in &model.defined {
        let x = e.eval(&label_value(model, &v, &defs)).unwrap_or(f64::NAN);
        defs.push(x);
    }
    v.extend(defs);
    v
}

/// Delta-method standard errors of f(θ): diag(J V Jᵀ), J by central differences.
fn delta_se(f: &dyn Fn(&[f64]) -> Vec<f64>, th: &[f64], v: &Mat) -> Vec<f64> {
    let base = f(th);
    let q = th.len();
    let mut jac = vec![vec![0.0; q]; base.len()];
    for k in 0..q {
        let h = 1e-6 * th[k].abs().max(1.0);
        let (mut up, mut dn) = (th.to_vec(), th.to_vec());
        up[k] += h;
        dn[k] -= h;
        let (fu, fd) = (f(&up), f(&dn));
        for (r, row) in jac.iter_mut().enumerate() {
            row[k] = (fu[r] - fd[r]) / (2.0 * h);
        }
    }
    jac.iter()
        .map(|j| {
            let mut s = 0.0;
            for a in 0..q {
                for c in 0..q {
                    s += j[a] * v[a][c] * j[c];
                }
            }
            s.max(0.0).sqrt()
        })
        .collect()
}

pub struct Prepared {
    pub model: Model,
    pub blocks: Vec<Block>,
    pub h1: H1,
    pub total: usize,
    pub tally: Tally,
    /// Full-information ML with at least one incomplete case.
    pub fiml: bool,
    /// The group labels of a multigroup model (empty with one group).
    pub group_labels: Vec<String>,
    /// WLSMV's sample statistics.
    pub ordinal: Option<ordinal::Sample>,
}

/// Reads the model and the cases: complete cases, or (FIML) every case with a value, in blocks by missing-data pattern.
pub fn prepare(ds: &Dataset, spec: &Spec) -> Result<Prepared> {
    let statements = syntax::parse(&spec.model)?;
    let use_fiml = spec.missing == Missing::Fiml;
    if spec.estimator == Estimator::Wlsmv {
        return prepare_ordinal(ds, spec, statements);
    }
    if use_fiml && spec.estimator == Estimator::Mlm {
        return Err(Error::Invalid("The Satorra-Bentler correction (MLM) needs complete data; with missing values use MLR (robust) with full-information ML.".into()));
    }
    let opts = Options { std_lv: spec.std_lv, meanstructure: spec.meanstructure || use_fiml, fixed_x: spec.fixed_x, growth: spec.growth };
    // Groups in order of first appearance.
    let (group_of, group_labels): (Vec<Option<usize>>, Vec<String>) = match &spec.group {
        Some(gname) => {
            let (slots, levels) = categories(ds, gname)?;
            let mut order: Vec<usize> = Vec::new();
            for s in &slots {
                if let Some(v) = s.value() {
                    if !order.contains(&v) {
                        order.push(v);
                    }
                }
            }
            if order.len() < 2 {
                return Err(Error::Invalid(format!("The grouping variable {gname} has fewer than two groups.")));
            }
            (slots.iter().map(|s| s.value().map(|v| order.iter().position(|o| *o == v).expect("seen"))).collect(), order.iter().map(|&v| levels[v].label.clone()).collect())
        }
        None => (vec![Some(0); ds.n_rows()], Vec::new()),
    };
    let groups = group_labels.len().max(1);
    let mut model = Model::build(&statements, &|n| ds.find(n).is_some() && spec.group.as_deref() != Some(n), &opts, groups)?;
    if model.obs.is_empty() {
        return Err(Error::Invalid("The model has no observed variables.".into()));
    }
    model.equate(&spec.group_equal, &spec.group_partial, spec.std_lv)?;
    let cols: Vec<Vec<Slot<f64>>> = model.obs.iter().map(|v| numeric(ds, v)).collect::<Result<_>>()?;
    let total = ds.n_rows();
    let p = model.p();
    let mut tally = Tally::default();
    let mut rows: Vec<Vec<Option<f64>>> = Vec::new();
    let mut row_group: Vec<usize> = Vec::new();
    'case: for r in 0..total {
        let Some(g) = group_of[r] else {
            tally.add("group missing");
            continue;
        };
        row_group.push(g);
        if use_fiml {
            // Fixed predictors must be observed (as lavaan requires); the other variables may be missing.
            for (k, c) in cols.iter().enumerate() {
                if model.xvars.contains(&k) && tally.check(&c[r], &model.obs[k]) {
                    continue 'case;
                }
            }
            let row: Vec<Option<f64>> = cols.iter().map(|c| c[r].value()).collect();
            if row.iter().all(Option::is_none) {
                tally.add("every model variable missing");
                row_group.pop();
                continue;
            }
            rows.push(row);
        } else {
            let mut row = Vec::with_capacity(p);
            for (c, name) in cols.iter().zip(&model.obs) {
                if tally.check(&c[r], name) {
                    row_group.pop();
                    continue 'case;
                }
                row.push(c[r].value());
            }
            rows.push(row);
        }
    }
    if rows.len() < 2 {
        return Err(Error::Invalid("Fewer than two cases have values for the model's variables.".into()));
    }
    let mut patterns: Vec<(usize, Vec<bool>, Vec<Vec<f64>>)> = Vec::new();
    for (row, &g) in rows.iter().zip(&row_group) {
        let pat: Vec<bool> = row.iter().map(Option::is_some).collect();
        let vals: Vec<f64> = row.iter().flatten().copied().collect();
        match patterns.iter_mut().find(|(h, q, _)| *h == g && *q == pat) {
            Some((_, _, v)) => v.push(vals),
            None => patterns.push((g, pat, vec![vals])),
        }
    }
    patterns.sort_by_key(|(g, _, _)| *g);
    let fiml = patterns.iter().any(|(_, pat, _)| pat.iter().any(|o| !o));
    let blocks: Vec<Block> = patterns
        .into_iter()
        .map(|(g, pat, vals)| {
            let (cov, mean) = moments(&vals);
            Block { group: g, vars: (0..p).filter(|&j| pat[j]).collect(), n: vals.len() as f64, cov, mean, rows: vals }
        })
        .collect();
    let mut h1 = H1 { mean: Vec::new(), cov: Vec::new() };
    for g in 0..groups {
        let label = group_labels.get(g).map(String::as_str).unwrap_or("the sample");
        let ng: usize = blocks.iter().filter(|b| b.group == g).map(|b| b.rows.len()).sum();
        if ng < 2 {
            return Err(Error::Invalid(format!("Fewer than two cases in group {label} have values for the model's variables.")));
        }
        if fiml {
            let grows: Vec<Vec<Option<f64>>> = rows.iter().zip(&row_group).filter(|(_, h)| **h == g).map(|(r, _)| r.clone()).collect();
            let (m, c) = super::missing::em(&grows, p).ok_or_else(|| Error::Invalid(format!("The saturated model could not be estimated from the incomplete data in {label} (EM failed): some pair of variables may never be observed together.")))?;
            h1.mean.push(m);
            h1.cov.push(c);
        } else {
            let b = blocks.iter().find(|b| b.group == g).expect("a block per group");
            h1.mean.push(b.mean.clone());
            h1.cov.push(b.cov.clone());
        }
    }
    estimate::start(&mut model, &h1.cov, &h1.mean);
    Ok(Prepared { model, blocks, h1, total, tally, fiml, group_labels, ordinal: None })
}

/// WLSMV: ordinal variables, their thresholds and polychoric correlations (complete cases, one group).
fn prepare_ordinal(ds: &Dataset, spec: &Spec, statements: Vec<syntax::Statement>) -> Result<Prepared> {
    if spec.group.is_some() || spec.missing == Missing::Fiml || spec.bootstrap > 0 {
        return Err(Error::Invalid("WLSMV here fits one group to complete cases, without the bootstrap.".into()));
    }
    let opts = Options { std_lv: spec.std_lv, meanstructure: false, fixed_x: false, growth: false };
    let mut model = Model::build(&statements, &|n| ds.find(n).is_some(), &opts, 1)?;
    for o in &model.obs {
        if !spec.ordered.is_empty() && !spec.ordered.contains(o) {
            return Err(Error::Invalid(format!("With WLSMV every observed variable must be ordinal; {o} is not in the ordered list.")));
        }
    }
    let p = model.p();
    for i in 0..p {
        if model.params.iter().any(|q| q.mat == Matrix::A && q.j == i) {
            return Err(Error::Invalid(format!("With WLSMV the ordinal variables must be outcomes (indicators or dependent variables); {} predicts another variable.", model.obs[i])));
        }
    }
    if model.params.iter().any(|q| q.op == "~1") {
        return Err(Error::Invalid("With WLSMV the means of the latent responses are fixed at 0 and thresholds are estimated instead; remove the ~1 lines.".into()));
    }
    let cols: Vec<Vec<Slot<f64>>> = model.obs.iter().map(|v| numeric(ds, v)).collect::<Result<_>>()?;
    let total = ds.n_rows();
    let mut tally = Tally::default();
    let mut rows: Vec<Vec<f64>> = Vec::new();
    'case: for r in 0..total {
        let mut row = Vec::with_capacity(p);
        for (c, name) in cols.iter().zip(&model.obs) {
            if tally.check(&c[r], name) {
                continue 'case;
            }
            row.push(c[r].value().expect("checked"));
        }
        rows.push(row);
    }
    if rows.len() < 10 {
        return Err(Error::Invalid("Fewer than ten complete cases: too few for polychoric correlations.".into()));
    }
    let mut cats: Vec<Vec<usize>> = Vec::new();
    let mut k = Vec::new();
    for (j, name) in model.obs.iter().enumerate() {
        let mut vals: Vec<f64> = rows.iter().map(|r| r[j]).collect();
        vals.sort_by(f64::total_cmp);
        vals.dedup();
        if vals.len() < 2 || vals.len() > 20 {
            return Err(Error::Invalid(format!("{name} has {} different values; an ordinal variable needs 2 to 20 ordered categories.", vals.len())));
        }
        cats.push(rows.iter().map(|r| vals.iter().position(|v| *v == r[j]).expect("present")).collect());
        k.push(vals.len());
    }
    let smp = ordinal::sample(&cats, &k).ok_or_else(|| Error::Invalid("The polychoric correlations' covariance matrix could not be computed (a pair of variables may have empty rows or columns).".into()))?;
    // Thresholds are parameters; residual variances follow from the delta parameterization.
    model.ordinal = (0..p).collect();
    for q in model.params.iter_mut() {
        if q.mat == Matrix::S && q.i == q.j && q.i < p {
            q.free = Some(usize::MAX);
        }
    }
    let mut next = 0;
    let mut map: Vec<(usize, usize)> = Vec::new();
    for q in model.params.iter_mut() {
        match q.free {
            Some(usize::MAX) => q.free = None,
            Some(old) => {
                let new = match map.iter().find(|(o, _)| *o == old) {
                    Some((_, n)) => *n,
                    None => {
                        map.push((old, next));
                        next += 1;
                        next - 1
                    }
                };
                q.free = Some(new);
            }
            None => {}
        }
    }
    for (i, tau) in smp.tau.iter().enumerate() {
        for (t, &v) in tau.iter().enumerate() {
            model.params.push(Param { group: 0, lhs: model.obs[i].clone(), op: "|", rhs: format!("t{}", t + 1), label: None, free: Some(next), value: v, start_given: true, user: false, exo: false, mat: Matrix::T, i, j: t });
            next += 1;
        }
    }
    model.nfree = next;
    estimate::start(&mut model, std::slice::from_ref(&smp.r), &[vec![0.0; p]]);
    let n = rows.len() as f64;
    let blocks = vec![Block { group: 0, vars: (0..p).collect(), n, cov: smp.r.clone(), mean: vec![0.0; p], rows: Vec::new() }];
    let h1 = H1 { mean: vec![vec![0.0; p]], cov: vec![smp.r.clone()] };
    Ok(Prepared { model, blocks, h1, total, tally, fiml: false, group_labels: Vec::new(), ordinal: Some(smp) })
}

/// WLSMV's fit and its measures.
fn core_ordinal(pr: &Prepared, smp: &ordinal::Sample) -> std::result::Result<Core, &'static str> {
    let d = ordinal::fit(&pr.model, smp).ok_or("The model could not be estimated by DWLS (a weight matrix or Δ'WΔ is singular); check that the model is identified.")?;
    let (bchisq, bdf, ba, bb) = ordinal::baseline(smp).ok_or("The baseline model could not be computed.")?;
    let mut model = pr.model.clone();
    model.set_theta(&d.fit.theta);
    let n = smp.n;
    let df = d.df;
    let t = d.a * d.chisq + d.b;
    let tb = ba * bchisq + bb;
    // RMSEA with N − 1, as lavaan for least-squares estimators.
    let nr = n - 1.0;
    let rm = |x2: f64| if df > 0.0 { ((x2 - df) / (nr * df)).max(0.0).sqrt() } else { 0.0 };
    let rmsea_ci = |x2: f64| -> (f64, f64) { if df > 0.0 { (((estimate::ncp_for(x2, df, 0.95)) / (nr * df)).sqrt(), ((estimate::ncp_for(x2, df, 0.05)) / (nr * df)).sqrt()) } else { (0.0, 0.0) } };
    let (lo, hi) = rmsea_ci(d.chisq);
    // SRMR from the correlation residuals (off the diagonal).
    let im = model.implied_full(0, &d.fit.theta).ok_or("The implied correlations could not be computed.")?;
    let p = model.p();
    let mut ss = 0.0;
    for i in 0..p {
        for j in 0..i {
            ss += (smp.r[i][j] - im.full[i][j]).powi(2);
        }
    }
    let srmr = (ss / (p * (p + 1) / 2) as f64).sqrt();
    let m = Measures {
        chisq: d.chisq,
        df,
        pvalue: if df > 0.0 { chisq_sf(d.chisq, df) } else { f64::NAN },
        base_chisq: bchisq,
        base_df: bdf,
        cfi: cfi_of(d.chisq, df, bchisq, bdf),
        tli: tli_of(d.chisq, df, bchisq, bdf),
        rmsea: rm(d.chisq),
        rmsea_lo: lo,
        rmsea_hi: hi,
        rmsea_p: if df > 0.0 { 1.0 - estimate::pnchisq(d.chisq, df, 0.05f64.powi(2) * nr * df) } else { f64::NAN },
        srmr,
        logl: f64::NAN,
        logl_sat: f64::NAN,
        aic: f64::NAN,
        bic: f64::NAN,
        scaled: (df > 0.0).then(|| Scaled {
            c: 1.0 / d.a,
            chisq: t,
            pvalue: chisq_sf(t, df),
            base_c: 1.0 / ba,
            base_chisq: tb,
            cfi: cfi_of(t, df, tb, bdf),
            tli: tli_of(t, df, tb, bdf),
            rmsea: rm(t),
            cfi_robust: f64::NAN,
            tli_robust: f64::NAN,
            rmsea_robust: f64::NAN,
            rmsea_robust_lo: f64::NAN,
            rmsea_robust_hi: f64::NAN,
        }),
    };
    Ok(Core { model, fit: d.fit, v: d.v, scaling: Some(1.0 / d.a), m })
}

/// The degrees of freedom: sample moments (less the fixed.x ones) minus free parameters.
fn degrees(model: &Model) -> (f64, f64) {
    let p = model.p() as f64;
    let nx = model.xvars.len() as f64;
    let ms = f64::from(u8::from(model.meanstructure));
    let g = model.groups as f64;
    let moments = g * (p * (p + 1.0) / 2.0 + ms * p) - g * (nx * (nx + 1.0) / 2.0 + ms * nx);
    (moments, moments - model.nfree as f64)
}

/// The log-likelihood of the blocks at the moments of each group.
fn loglik(blocks: &[Block], mean: &[Vec<f64>], cov: &[Mat]) -> Option<f64> {
    let mut l = 0.0;
    for b in blocks {
        let sig: Mat = b.vars.iter().map(|&i| b.vars.iter().map(|&j| cov[b.group][i][j]).collect()).collect();
        let w = invert(&sig)?;
        let pb = b.vars.len();
        let d: Vec<f64> = b.vars.iter().enumerate().map(|(k, &v)| b.mean[k] - mean[b.group][v]).collect();
        let mut tr = 0.0;
        let mut q = 0.0;
        for i in 0..pb {
            for j in 0..pb {
                tr += w[i][j] * b.cov[j][i];
                q += d[i] * w[i][j] * d[j];
            }
        }
        l += -b.n / 2.0 * (pb as f64 * 1.837_877_066_409_345_3 + log_det(&sig)? + tr + q);
    }
    Some(l)
}

/// The covariance of the estimates, and for MLM and MLR the scaling factor of the test.
fn uncertainty(model: &Model, blocks: &[Block], h1: &H1, fit: &estimate::Fit, estimator: Estimator, fiml: bool, info: H1Information) -> Option<(Mat, Option<f64>)> {
    let n = estimate::total_n(blocks);
    let (_, df) = degrees(model);
    match estimator {
        Estimator::Ml if !fiml => Some((estimate::vcov(fit, n)?, None)),
        Estimator::Ml => {
            let a = robust::observed_info(model, blocks, &fit.theta)?;
            let ai = invert(&a)?;
            Some((ai.iter().map(|r| r.iter().map(|v| v / n).collect()).collect(), None))
        }
        Estimator::Mlm => {
            let unstructured = info == H1Information::Unstructured;
            let (e_sample, meat, tr_wg) = robust::satorra_bentler(model, blocks, &fit.theta, unstructured)?;
            let e: Mat = if unstructured { e_sample } else { fit.h.iter().map(|r| r.iter().map(|v| v / 2.0).collect()).collect() };
            let v = robust::sandwich(&e, &meat, n)?;
            let c = (tr_wg - robust::trace_inv(&e, &meat)?) / df;
            Some((v, Some(c)))
        }
        Estimator::Wlsmv | Estimator::Bayes => None,
        Estimator::Mlr => {
            let a = robust::observed_info(model, blocks, &fit.theta)?;
            let b0 = robust::first_order(model, blocks, &fit.theta)?;
            let v = robust::sandwich(&a, &b0, n)?;
            let c = (robust::h1_trace(model, blocks, h1, fiml)? - robust::trace_inv(&a, &b0)?) / df;
            Some((v, Some(c)))
        }
    }
}

struct Measures {
    chisq: f64,
    df: f64,
    pvalue: f64,
    base_chisq: f64,
    base_df: f64,
    cfi: f64,
    tli: f64,
    rmsea: f64,
    rmsea_lo: f64,
    rmsea_hi: f64,
    rmsea_p: f64,
    srmr: f64,
    logl: f64,
    logl_sat: f64,
    aic: f64,
    bic: f64,
    /// MLM or MLR: the scaled test and the scaled and robust indices.
    scaled: Option<Scaled>,
}

struct Scaled {
    c: f64,
    chisq: f64,
    pvalue: f64,
    base_c: f64,
    base_chisq: f64,
    cfi: f64,
    tli: f64,
    rmsea: f64,
    cfi_robust: f64,
    tli_robust: f64,
    rmsea_robust: f64,
    rmsea_robust_lo: f64,
    rmsea_robust_hi: f64,
}

fn cfi_of(x2: f64, df: f64, x2b: f64, dfb: f64) -> f64 {
    let t1 = (x2 - df).max(0.0);
    let t2 = (x2 - df).max(x2b - dfb).max(0.0);
    if t2 > 0.0 {
        1.0 - t1 / t2
    } else {
        1.0
    }
}

fn tli_of(x2: f64, df: f64, x2b: f64, dfb: f64) -> f64 {
    let (t1, t2) = ((x2 - df) * dfb, (x2b - dfb) * df);
    if df > 0.0 && t2.abs() > 0.0 {
        1.0 - t1 / t2
    } else {
        1.0
    }
}

fn measures(pr: &Prepared, th: &[f64], f: f64, scaling: Option<f64>, estimator: Estimator, info: H1Information) -> Option<Measures> {
    let (model, blocks, h1, fiml) = (&pr.model, &pr.blocks[..], &pr.h1, pr.fiml);
    let n = estimate::total_n(blocks);
    let g = model.groups as f64;
    let (moments, df) = degrees(model);
    let logl = -n / 2.0 * f;
    let logl_sat = loglik(blocks, &h1.mean, &h1.cov)?;
    let chisq = (2.0 * (logl_sat - logl)).max(0.0);
    // With fixed.x, lavaan reports log-likelihoods conditional on the fixed predictors: the predictors' own (saturated)
    // log-likelihood is taken out.
    let mut logl_x = 0.0;
    if !model.xvars.is_empty() {
        for gi in 0..model.groups {
            let ng: f64 = blocks.iter().filter(|b| b.group == gi).map(|b| b.n).sum();
            let sxx: Mat = model.xvars.iter().map(|&i| model.xvars.iter().map(|&j| h1.cov[gi][i][j]).collect()).collect();
            let px = model.xvars.len() as f64;
            logl_x += -ng / 2.0 * (px * 1.837_877_066_409_345_3 + log_det(&sxx)? + px);
        }
    }
    let mut base = baseline_of(model);
    estimate::start(&mut base, &h1.cov, &h1.mean);
    let bf = estimate::fit(&base, blocks).ok()?;
    let base_chisq = (2.0 * (logl_sat + n / 2.0 * bf.f)).max(0.0);
    let base_df = moments - base.nfree as f64;
    let (logl, logl_sat) = (logl - logl_x, logl_sat - logl_x);
    let cfi = cfi_of(chisq, df, base_chisq, base_df);
    let tli = tli_of(chisq, df, base_chisq, base_df);
    let rm = |lambda: f64| if df > 0.0 { (lambda / (n * df)).max(0.0).sqrt() * g.sqrt() } else { 0.0 };
    let rmsea = rm((chisq - df).max(0.0));
    let (rmsea_lo, rmsea_hi, rmsea_p) = if df > 0.0 {
        (rm(estimate::ncp_for(chisq, df, 0.95)), rm(estimate::ncp_for(chisq, df, 0.05)), 1.0 - estimate::pnchisq(chisq, df, 0.05f64.powi(2) * n * df / g))
    } else {
        (0.0, 0.0, f64::NAN)
    };
    // SRMR (Bentler): residuals from the unrestricted moments, scaled by their standard deviations, over the lower
    // triangle (and the means).
    let mut srmr = 0.0;
    for gi in 0..model.groups {
        let im = model.implied_full(gi, th)?;
        let (sc, sm) = (&h1.cov[gi], &h1.mean[gi]);
        let p = model.p();
        let (mut ss, mut count) = (0.0, 0.0);
        for i in 0..p {
            for j in 0..=i {
                let r = (sc[i][j] - im.full[i][j]) / (sc[i][i] * sc[j][j]).sqrt();
                ss += r * r;
                count += 1.0;
            }
            if model.meanstructure {
                let r = (sm[i] - im.mean[i]) / sc[i][i].sqrt();
                ss += r * r;
                count += 1.0;
            }
        }
        let ng: f64 = blocks.iter().filter(|b| b.group == gi).map(|b| b.n).sum();
        srmr += ng / n * (ss / count).sqrt();
    }
    let scaled = match scaling {
        Some(c) if df > 0.0 => {
            let (_, base_c) = uncertainty(&base, blocks, h1, &bf, estimator, fiml, info)?;
            let base_c = base_c?;
            let (t, tb) = (chisq / c, base_chisq / base_c);
            let rr = |lambda: f64| (c * lambda / (n * df)).max(0.0).sqrt() * g.sqrt();
            Some(Scaled {
                c,
                chisq: t,
                pvalue: chisq_sf(t, df),
                base_c,
                base_chisq: tb,
                cfi: cfi_of(t, df, tb, base_df),
                tli: tli_of(t, df, tb, base_df),
                rmsea: rm((t - df).max(0.0)),
                cfi_robust: cfi_of(chisq, c * df, base_chisq, base_c * base_df),
                tli_robust: 1.0 - (chisq - c * df) * base_df / ((base_chisq - base_c * base_df) * df),
                rmsea_robust: ((chisq / n) / df - c / n).max(0.0).sqrt() * g.sqrt(),
                rmsea_robust_lo: rr(estimate::ncp_for(t, df, 0.95)),
                rmsea_robust_hi: rr(estimate::ncp_for(t, df, 0.05)),
            })
        }
        _ => None,
    };
    let k = model.nfree as f64;
    Some(Measures {
        chisq,
        df,
        pvalue: if df > 0.0 { chisq_sf(chisq, df) } else { f64::NAN },
        base_chisq,
        base_df,
        cfi,
        tli,
        rmsea,
        rmsea_lo,
        rmsea_hi,
        rmsea_p,
        srmr,
        logl,
        logl_sat,
        aic: -2.0 * logl + 2.0 * k,
        bic: -2.0 * logl + k * n.ln(),
        scaled,
    })
}

/// Candidate parameters for modification indices: loadings not in the model, and residual covariances not in it.
fn candidates(model: &Model) -> Vec<(String, Place)> {
    let p = model.p();
    let mut out = Vec::new();
    for g in 0..model.groups {
        let present = |mat: Matrix, i: usize, j: usize| model.params.iter().any(|q| q.group == g && q.mat == mat && ((q.i == i && q.j == j) || (mat == Matrix::S && q.i == j && q.j == i)));
        let measured: Vec<usize> = (0..model.lat.len()).map(|l| p + l).filter(|&f| model.params.iter().any(|q| q.op == "=~" && q.j == f)).collect();
        for &f in &measured {
            for i in 0..p {
                if model.xvars.contains(&i) || present(Matrix::A, i, f) {
                    continue;
                }
                // Only variables that already indicate some factor.
                if !model.params.iter().any(|q| q.op == "=~" && q.i == i) {
                    continue;
                }
                out.push((format!("{} =~ {}", model.name_of(f), model.obs[i]), Place { group: g, mat: Matrix::A, i, j: f }));
            }
        }
        for i in 0..p {
            for j in i + 1..p {
                if model.xvars.contains(&i) || model.xvars.contains(&j) || present(Matrix::S, i, j) {
                    continue;
                }
                out.push((format!("{} ~~ {}", model.obs[i], model.obs[j]), Place { group: g, mat: Matrix::S, i, j }));
            }
        }
    }
    out
}

/// A fitted model: estimates, their covariance, the test's scaling factor (MLM, MLR) and the fit measures.
pub struct Core {
    pub model: Model,
    pub fit: estimate::Fit,
    pub v: Mat,
    pub scaling: Option<f64>,
    m: Measures,
}

/// Fits a prepared model, or says why it can't be fitted.
pub fn core(pr: &Prepared, spec: &Spec) -> std::result::Result<Core, &'static str> {
    if let Some(smp) = &pr.ordinal {
        return core_ordinal(pr, smp);
    }
    let n = estimate::total_n(&pr.blocks);
    if n < 2.0 || (n as usize) <= pr.model.p() {
        return Err("There are too few complete cases for this many variables.");
    }
    let fit = estimate::fit(&pr.model, &pr.blocks).map_err(|e| match e {
        FitError::Start => "The model could not be estimated: the starting values give an impossible (not positive definite) covariance matrix. Give starting values with start(), or check the model.",
        FitError::NotIdentified => "The model is not identified: some parameters can't be estimated from the data. Fix a loading or a variance of each factor, or remove parameters.",
    })?;
    let (v, scaling) = uncertainty(&pr.model, &pr.blocks, &pr.h1, &fit, spec.estimator, pr.fiml, spec.h1_information).ok_or("The information matrix is singular, so the model is not identified.")?;
    let mut model = pr.model.clone();
    model.set_theta(&fit.theta);
    let m = measures(pr, &fit.theta, fit.f, scaling, spec.estimator, spec.h1_information).ok_or("The fit measures could not be computed: the sample covariance matrix is singular (a variable is constant or a linear combination of others).")?;
    Ok(Core { model, fit, v, scaling, m })
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    check_level(spec.level)?;
    if twolevel::is_two_level(&spec.model) {
        return twolevel::run(ds, spec);
    }
    if spec.estimator == Estimator::Bayes {
        return mcmc::run(ds, spec);
    }
    if spec.bootstrap > 0 && !(100..=100_000).contains(&spec.bootstrap) {
        return Err(Error::Invalid("Use between 100 and 100,000 bootstrap resamples.".into()));
    }
    let pr = prepare(ds, spec)?;
    let model = pr.model.clone();
    let blocks = pr.blocks.clone();
    let n = estimate::total_n(&blocks);
    let lv = level_text(spec.level);
    let estimation = match (spec.estimator, pr.fiml) {
        (Estimator::Mlm, _) if spec.h1_information == H1Information::Unstructured => "maximum likelihood with Satorra-Bentler robust standard errors (from the expected information) and the Satorra-Bentler scaled χ², with the weight at the sample moments (unstructured)",
        (Estimator::Ml, false) => "maximum likelihood, with standard errors from the expected information",
        (Estimator::Ml, true) => "full-information maximum likelihood (every case with any value, assuming the values are missing at random), with standard errors from the observed information",
        (Estimator::Mlm, _) => "maximum likelihood with Satorra-Bentler robust standard errors (from the expected information) and the Satorra-Bentler scaled χ²",
        (Estimator::Mlr, false) => "maximum likelihood with robust (Huber-White) standard errors from the observed information and the Yuan-Bentler scaled χ², as lavaan computes it for MLR",
        (Estimator::Mlr, true) => "full-information maximum likelihood (assuming missing at random) with robust (Huber-White) standard errors from the observed information and the Yuan-Bentler scaled χ², as lavaan computes it for MLR",
        (Estimator::Bayes, _) => "Bayesian MCMC",
        (Estimator::Wlsmv, _) => "diagonally weighted least squares on polychoric correlations and thresholds (WLSMV: thresholds from the margins and correlations by two-step ML, their asymptotic covariance as Muthén 1984, the delta parameterization), with robust standard errors and the mean- and variance-adjusted (scaled and shifted) χ²",
    };
    let robust_note = match (spec.estimator, pr.fiml) {
        (Estimator::Ml, _) => "",
        (Estimator::Wlsmv, _) => " Scaled CFI, TLI and RMSEA use the scaled-and-shifted χ²; RMSEA uses N − 1 and SRMR the correlation residuals, as lavaan.",
        (_, false) => " Scaled indices use the scaled χ²; robust CFI, TLI and RMSEA are those of Brosseau-Liard and Savalei.",
        (_, true) => " Scaled CFI, TLI and RMSEA use the scaled χ²; the FIML-corrected robust indices are not computed.",
    };
    let note = format!(
        "Covariance-based structural equation model estimated by {estimation}, with lavaan's defaults ({}{}{}); z tests and {lv} Wald intervals. Completely standardized estimates (std.all) have delta-method standard errors. Fit: the χ² test and CFI, TLI, RMSEA (with a 90% interval), and SRMR (Bentler).{robust_note} Computed by PrismStatistics.",
        if spec.std_lv { "factor variances fixed at 1" } else { "the first loading of each factor fixed at 1" },
        if spec.fixed_x && !model.xvars.is_empty() { "; observed predictors fixed at their sample moments" } else { "" },
        if spec.growth { "; a latent growth model as lavaan's growth(): observed intercepts fixed at 0 and the growth factors' means estimated" } else if model.meanstructure { "; with a mean structure" } else { "" },
    );
    let title = if spec.growth {
        "Latent Growth Model".to_string()
    } else if model.lat.is_empty() { "Path Analysis".to_string() } else if model.params.iter().any(|p| p.op == "~") { "Structural Equation Model".to_string() } else { "Confirmatory Factor Analysis".to_string() };
    let mut b = common::bundle(SEM, METHOD_VERSION, title, serde_json::to_value(spec).expect("a spec always serializes"), pr.tally.clone().accounting(pr.total, n as usize), note);
    set(&mut b, "n", n);
    set(&mut b, "npar", model.nfree as f64);
    set_text(&mut b, "estimator", match spec.estimator {
        Estimator::Ml => "ML",
        Estimator::Mlm => "MLM",
        Estimator::Mlr => "MLR",
        Estimator::Wlsmv => "WLSMV",
        Estimator::Bayes => "Bayes",
    });
    set(&mut b, "fiml", f64::from(u8::from(pr.fiml)));
    set(&mut b, "level", spec.level);
    let c = match core(&pr, spec) {
        Ok(c) => c,
        Err(why) => {
            fail(&mut b, why);
            return Ok(b);
        }
    };
    let (fit, v, model, m) = (c.fit, c.v, c.model, c.m);
    let th = fit.theta.clone();
    let multi = model.groups > 1;
    let glabel = |g: usize| pr.group_labels.get(g).cloned().unwrap_or_default();
    if multi {
        set(&mut b, "groups", model.groups as f64);
        for (g, l) in pr.group_labels.iter().enumerate() {
            set_text(&mut b, format!("group.{}.label", g + 1), l.clone());
            set(&mut b, format!("group.{}.n", g + 1), blocks.iter().filter(|x| x.group == g).map(|x| x.n).sum::<f64>());
        }
    }
    let est = unstandardized(&model, &th);
    let mut se_all = delta_se(&|t| unstandardized(&model, t), &th, &v);
    let std = standardized(&model, &th);
    // With the bootstrap: each value's standard error and interval from its resampled values, and the standardized
    // solution's delta-method standard errors from the bootstrap covariance of the estimates, as lavaan.
    let mut intervals: Vec<Option<(f64, f64)>> = vec![None; est.len()];
    let mut v = v;
    if spec.bootstrap > 0 {
        let bt = boot::resample(&model, &blocks, spec.bootstrap, spec.seed, &|t| unstandardized(&model, t));
        set(&mut b, "boot.resamples", spec.bootstrap as f64);
        set(&mut b, "boot.used", bt.values.len() as f64);
        set(&mut b, "boot.failed", bt.failed as f64);
        set(&mut b, "boot.seed", spec.seed as f64);
        if bt.values.len() < 20 {
            fail(&mut b, "Fewer than 20 bootstrap resamples could be fitted, so the bootstrap can't be used.");
            return Ok(b);
        }
        for k in 0..est.len() {
            let reps: Vec<f64> = bt.values.iter().map(|r| r[k]).collect();
            if let Some((sd, lo, hi)) = boot::summarize(est[k], &reps, spec.level, spec.bias_corrected) {
                se_all[k] = sd;
                intervals[k] = Some((lo, hi));
            }
        }
        v = boot::covariance(&bt.thetas);
        b.method_note = b.method_note.replace(
            " Computed by PrismStatistics.",
            &format!(
                " Standard errors, z tests and {lv} {} intervals are from {} bootstrap resamples of the cases{} (seed {}; {} could not be fitted and were dropped); standardized standard errors use the bootstrap covariance. Computed by PrismStatistics.",
                if spec.bias_corrected { "bias-corrected percentile" } else { "percentile" },
                spec.bootstrap,
                if multi { " within groups" } else { "" },
                spec.seed,
                bt.failed
            ),
        );
    }
    let std_se = delta_se(&|t| standardized(&model, t), &th, &v);
    let z = norm_quantile(1.0 - (1.0 - spec.level) / 2.0);

    // Every parameter, then the defined ones.
    let mut names: Vec<String> = model.params.iter().map(Param::name).collect();
    names.extend(model.defined.iter().map(|(d, e)| format!("{d} := {}", show(e))));
    let free: Vec<bool> = model.params.iter().map(|p| p.free.is_some()).chain(model.defined.iter().map(|_| true)).collect();
    let mut sections: Vec<(&str, &str, Vec<Vec<crate::result::Cell>>)> = vec![("loadings", "Factor loadings", vec![]), ("regressions", "Regressions", vec![]), ("covariances", "Covariances", vec![]), ("variances", "Variances", vec![]), ("intercepts", "Intercepts and means", vec![]), ("thresholds", "Thresholds", vec![]), ("defined", "Defined parameters", vec![])];
    for (k, name) in names.iter().enumerate() {
        let key = format!("par.{}", k + 1);
        set_text(&mut b, format!("{key}.label"), name.clone());
        set(&mut b, format!("{key}.est"), est[k]);
        set(&mut b, format!("{key}.std"), std[k]);
        let has_se = free[k] && se_all[k] > 0.0;
        let mut row = vec![text(name.clone())];
        if multi {
            let g = if k < model.params.len() { glabel(model.params[k].group) } else { String::new() };
            if !g.is_empty() {
                set_text(&mut b, format!("{key}.group"), g.clone());
            }
            row.push(text(g));
        }
        row.push(num(est[k], 3));
        if has_se {
            let (se, zv) = (se_all[k], est[k] / se_all[k]);
            let pv = norm_two_sided(zv);
            let (lo, hi) = intervals[k].unwrap_or((est[k] - z * se, est[k] + z * se));
            for (f, val) in [("se", se), ("z", zv), ("p", pv), ("lower", lo), ("upper", hi)] {
                set(&mut b, format!("{key}.{f}"), val);
            }
            row.extend([num(se, 3), num(zv, 2), common::p(pv), num(lo, 3), num(hi, 3)]);
        } else {
            row.extend([text(""), text(""), text(""), text(""), text("")]);
        }
        if std_se[k] > 1e-12 {
            set(&mut b, format!("{key}.stdse"), std_se[k]);
        }
        row.push(if std[k].is_finite() { num(std[k], 3) } else { text("") });
        let sec = if k >= model.params.len() {
            6
        } else {
            let p = &model.params[k];
            match (p.op, p.i == p.j) {
                ("=~", _) => 0,
                ("~", _) => 1,
                ("~~", false) => 2,
                ("~~", true) => 3,
                ("|", _) => 5,
                _ => 4,
            }
        };
        sections[sec].2.push(row);
    }
    set(&mut b, "par.count", names.len() as f64);
    let cols = |first: &str| {
        let mut c = vec![first.to_string()];
        if multi {
            c.push("Group".into());
        }
        c.extend(["Estimate".to_string(), "SE".into(), "z".into(), "p".into(), format!("{lv} CI lower"), format!("{lv} CI upper"), "Standardized".into()]);
        c
    };
    let mut param_tables = Vec::new();
    for (id, title, rows) in sections {
        if rows.is_empty() {
            continue;
        }
        let mut footnotes = Vec::new();
        if id == "loadings" && !spec.std_lv {
            footnotes.push("Loadings without a standard error are fixed (at 1, the first indicator of each factor) to set the factor's scale.".to_string());
        }
        if id == "variances" && !model.xvars.is_empty() {
            footnotes.push("Variances of observed predictors are fixed at their sample values.".to_string());
        }
        if id == "variances" {
            footnotes.push("Variances of endogenous variables are residual variances.".to_string());
        }
        param_tables.push(Table { id: id.into(), title: title.into(), columns: cols("Parameter"), rows, footnotes });
    }

    // Fit.
    for (k, val) in [
        ("fit.chisq", m.chisq),
        ("fit.df", m.df),
        ("fit.pvalue", m.pvalue),
        ("fit.baseline.chisq", m.base_chisq),
        ("fit.baseline.df", m.base_df),
        ("fit.cfi", m.cfi),
        ("fit.tli", m.tli),
        ("fit.rmsea", m.rmsea),
        ("fit.rmsea.lower", m.rmsea_lo),
        ("fit.rmsea.upper", m.rmsea_hi),
        ("fit.rmsea.pvalue", m.rmsea_p),
        ("fit.srmr", m.srmr),
        ("fit.logl", m.logl),
        ("fit.unrestricted.logl", m.logl_sat),
        ("fit.aic", m.aic),
        ("fit.bic", m.bic),
    ] {
        set(&mut b, k, val);
    }
    if let Some(sc) = &m.scaled {
        let robust_ok = !pr.fiml;
        for (k, val) in [
            ("fit.chisq.scaled", sc.chisq),
            ("fit.chisq.scaling.factor", sc.c),
            ("fit.pvalue.scaled", sc.pvalue),
            ("fit.baseline.chisq.scaled", sc.base_chisq),
            ("fit.baseline.chisq.scaling.factor", sc.base_c),
            ("fit.cfi.scaled", sc.cfi),
            ("fit.tli.scaled", sc.tli),
            ("fit.rmsea.scaled", sc.rmsea),
            ("fit.cfi.robust", sc.cfi_robust),
            ("fit.tli.robust", sc.tli_robust),
            ("fit.rmsea.robust", sc.rmsea_robust),
            ("fit.rmsea.ci.lower.robust", sc.rmsea_robust_lo),
            ("fit.rmsea.ci.upper.robust", sc.rmsea_robust_hi),
        ] {
            if robust_ok || !k.ends_with(".robust") {
                set(&mut b, k, val);
            }
        }
    }
    let small = |v: f64| crate::apa::no_leading_zero(v, 3);
    let mut fit_rows = vec![
        vec![text("χ²"), num(m.chisq, 2)],
        vec![text("df"), num(m.df, 0)],
        vec![text("p"), if m.df > 0.0 { common::p(m.pvalue) } else { text("") }],
        vec![text("CFI"), text(small(m.cfi))],
        vec![text("TLI"), text(small(m.tli))],
        vec![text("RMSEA [90% CI]"), text(format!("{} [{}, {}]", small(m.rmsea), small(m.rmsea_lo), small(m.rmsea_hi)))],
        vec![text("p (RMSEA ≤ .05)"), if m.df > 0.0 { common::p(m.rmsea_p) } else { text("") }],
        vec![text("SRMR"), text(small(m.srmr))],
        vec![text("AIC"), num(m.aic, 2)],
        vec![text("BIC"), num(m.bic, 2)],
        vec![text("Log-likelihood"), num(m.logl, 2)],
    ];
    // Least squares has no likelihood.
    fit_rows.retain(|r| !matches!(&r[1], crate::result::Cell::Number { value, .. } if value.is_nan()));
    if let Some(sc) = &m.scaled {
        let name = match spec.estimator {
            Estimator::Mlm => "Satorra-Bentler",
            Estimator::Wlsmv => "mean- and variance-adjusted",
            _ => "Yuan-Bentler",
        };
        fit_rows.push(vec![text(format!("Scaled χ² ({name})")), num(sc.chisq, 2)]);
        fit_rows.push(vec![text("Scaling factor"), num(sc.c, 3)]);
        fit_rows.push(vec![text("p (scaled)"), common::p(sc.pvalue)]);
        if pr.fiml || pr.ordinal.is_some() {
            fit_rows.push(vec![text("Scaled CFI"), text(small(sc.cfi))]);
            fit_rows.push(vec![text("Scaled TLI"), text(small(sc.tli))]);
            fit_rows.push(vec![text("Scaled RMSEA"), text(small(sc.rmsea))]);
        } else {
            fit_rows.push(vec![text("Robust CFI"), text(small(sc.cfi_robust))]);
            fit_rows.push(vec![text("Robust TLI"), text(small(sc.tli_robust))]);
            fit_rows.push(vec![text("Robust RMSEA [90% CI]"), text(format!("{} [{}, {}]", small(sc.rmsea_robust), small(sc.rmsea_robust_lo), small(sc.rmsea_robust_hi)))]);
        }
    }
    fit_rows.push(vec![text("Free parameters"), count(model.nfree)]);
    fit_rows.push(vec![text("Cases"), count(n as usize)]);
    if pr.fiml {
        fit_rows.push(vec![text("Missing-data patterns"), count(blocks.len())]);
    }
    let fit_note = format!("Baseline (independence) model: χ²({}) = {}.", crate::apa::num(m.base_df, 0), crate::apa::num(m.base_chisq, 2));
    b.tables.push(Table { id: "fit".into(), title: "Model fit".into(), columns: vec!["Measure".into(), "Value".into()], rows: fit_rows, footnotes: vec![fit_note] });
    b.tables.extend(param_tables);

    // R² of endogenous variables.
    let mut r2_rows = Vec::new();
    for g in 0..model.groups {
        let Some(im) = model.implied_full(g, &th) else { continue };
        for i in 0..model.t() {
            if !model.params.iter().any(|p| p.group == g && p.mat == Matrix::A && p.i == i) {
                continue;
            }
            let r2 = 1.0 - im.s[i][i] / im.full[i][i];
            if multi {
                set(&mut b, format!("r2.{}:{}", glabel(g), model.name_of(i)), r2);
                r2_rows.push(vec![text(model.name_of(i)), text(glabel(g)), num(r2, 3)]);
            } else {
                set(&mut b, format!("r2.{}", model.name_of(i)), r2);
                r2_rows.push(vec![text(model.name_of(i)), num(r2, 3)]);
            }
        }
    }
    if !r2_rows.is_empty() {
        let columns = if multi { vec!["Variable".into(), "Group".into(), "R²".into()] } else { vec!["Variable".into(), "R²".into()] };
        b.tables.push(Table { id: "r2".into(), title: "Variance explained (R²)".into(), columns, rows: r2_rows, footnotes: Vec::new() });
    }

    // Reliability and validity: composite reliability (omega) and AVE from the observed variances, and the
    // heterotrait-monotrait ratio, as semTools computes them (compRelSEM, AVE and htmt with their defaults).
    if spec.reliability && !multi && pr.ordinal.is_none() {
        if let Some(im) = model.implied_full(0, &th) {
            let s = &pr.h1.cov[0];
            let p = model.p();
            let mut sets: Vec<(String, Vec<usize>)> = Vec::new();
            let mut rel_rows = Vec::new();
            for (li, name) in model.lat.iter().enumerate() {
                let f = p + li;
                let ind: Vec<(usize, f64)> = model.params.iter().filter(|q| q.op == "=~" && q.j == f && q.i < p).map(|q| (q.i, q.value)).collect();
                if ind.len() < 2 {
                    continue;
                }
                let vf = im.full[f][f];
                let sum_l: f64 = ind.iter().map(|x| x.1).sum();
                let total: f64 = ind.iter().map(|&(i, _)| ind.iter().map(|&(j, _)| s[i][j]).sum::<f64>()).sum();
                let omega = sum_l * sum_l * vf / total;
                let ave = ind.iter().map(|x| x.1 * x.1).sum::<f64>() * vf / ind.iter().map(|&(i, _)| s[i][i]).sum::<f64>();
                set(&mut b, format!("rel.omega.{name}"), omega);
                set(&mut b, format!("rel.ave.{name}"), ave);
                rel_rows.push(vec![text(name.clone()), num(omega, 3), num(ave, 3), num(ave.sqrt(), 3)]);
                sets.push((name.clone(), ind.iter().map(|x| x.0).collect()));
            }
            if !rel_rows.is_empty() {
                b.tables.push(Table {
                    id: "reliability".into(),
                    title: "Composite reliability and convergent validity".into(),
                    columns: vec!["Factor".into(), "Composite reliability (ω)".into(), "AVE".into(), "√AVE".into()],
                    rows: rel_rows,
                    footnotes: vec!["ω = (Σλ)²·Var(factor) / the variance of the sum of the indicators (observed); AVE (average variance extracted) = Σλ²·Var(factor) / the sum of the indicators' observed variances, as semTools' compRelSEM and AVE. Common guidelines: ω ≥ .70 and AVE ≥ .50 (Fornell & Larcker, 1981); discriminant validity when √AVE exceeds the factor's correlations with other factors.".into()],
                });
            }
            if sets.len() >= 2 {
                let r = |i: usize, j: usize| (s[i][j] / (s[i][i] * s[j][j]).sqrt()).abs();
                let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
                let gmean = |v: &[f64]| (v.iter().map(|x| x.ln()).sum::<f64>() / v.len() as f64).exp();
                let within = |set: &[usize]| -> Vec<f64> {
                    let mut v = Vec::new();
                    for (a, &i) in set.iter().enumerate() {
                        for &j in &set[a + 1..] {
                            v.push(r(i, j));
                        }
                    }
                    v
                };
                let mut rows = Vec::new();
                for (a, (na, ia)) in sets.iter().enumerate() {
                    let mut row = vec![text(na.clone())];
                    for (nb, ib) in sets.iter().take(a) {
                        let hetero: Vec<f64> = ib.iter().flat_map(|&i| ia.iter().map(move |&j| (i, j))).map(|(i, j)| r(i, j)).collect();
                        let (wa, wb) = (within(ia), within(ib));
                        let htmt = mean(&hetero) / (mean(&wa) * mean(&wb)).sqrt();
                        let htmt2 = gmean(&hetero) / (gmean(&wa) * gmean(&wb)).sqrt();
                        set(&mut b, format!("rel.htmt.{nb}.{na}"), htmt);
                        set(&mut b, format!("rel.htmt2.{nb}.{na}"), htmt2);
                        row.push(text(format!("{} ({})", crate::apa::no_leading_zero(htmt2, 2), crate::apa::no_leading_zero(htmt, 2))));
                    }
                    row.push(text("—"));
                    while row.len() < sets.len() + 1 {
                        row.push(text(""));
                    }
                    rows.push(row);
                }
                let mut columns = vec!["Factor".to_string()];
                columns.extend(sets.iter().map(|x| x.0.clone()));
                b.tables.push(Table {
                    id: "htmt".into(),
                    title: "Discriminant validity: heterotrait-monotrait ratios".into(),
                    columns,
                    rows,
                    footnotes: vec!["HTMT2 (geometric means; Roemer, Schuberth & Henseler, 2021), with the original HTMT (arithmetic means; Henseler, Ringle & Sarstedt, 2015) in parentheses, from the absolute correlations of the indicators. Values below .85 (or .90 for conceptually close factors) suggest discriminant validity.".into()],
                });
            }
        }
    }

    // Modification indices.
    if spec.modindices && m.df > 0.0 && pr.ordinal.is_none() {
        let cands = candidates(&model);
        if let Some((g, h)) = estimate::gradient_info(&model, &blocks, &th, &cands.iter().map(|(_, pl)| vec![*pl]).collect::<Vec<_>>()) {
            let q = model.nfree;
            let hi = invert(&h[..q].iter().map(|r| r[..q].to_vec()).collect::<Vec<_>>());
            let imp: Vec<_> = (0..model.groups).map(|gi| model.implied_full(gi, &th)).collect();
            let mut mis: Vec<(String, f64, f64, f64)> = Vec::new();
            if let Some(hi) = hi {
                for (c, (name, pl)) in cands.iter().enumerate() {
                    let k = q + c;
                    let mut cond = h[k][k];
                    for a in 0..q {
                        for d in 0..q {
                            cond -= h[k][a] * hi[a][d] * h[d][k];
                        }
                    }
                    if cond <= 1e-10 * h[k][k].abs().max(1e-300) {
                        continue;
                    }
                    let mi = n * g[k] * g[k] / (2.0 * cond);
                    let epc = -g[k] / cond;
                    let Some(im) = &imp[pl.group] else { continue };
                    let sepc = match pl.mat {
                        Matrix::A => epc * im.full[pl.j][pl.j].sqrt() / im.full[pl.i][pl.i].sqrt(),
                        _ => epc / (im.s[pl.i][pl.i] * im.s[pl.j][pl.j]).sqrt(),
                    };
                    let name = if multi { format!("[{}] {name}", glabel(pl.group)) } else { name.clone() };
                    mis.push((name, mi, epc, sepc));
                }
            }
            mis.sort_by(|a, c| c.1.total_cmp(&a.1));
            for (k, (name, mi, epc, sepc)) in mis.iter().enumerate() {
                let key = format!("mi.{}", k + 1);
                set_text(&mut b, format!("{key}.label"), name.clone());
                set(&mut b, format!("{key}.mi"), *mi);
                set(&mut b, format!("{key}.epc"), *epc);
                set(&mut b, format!("{key}.sepc"), *sepc);
            }
            set(&mut b, "mi.count", mis.len() as f64);
            let shown: Vec<_> = mis.iter().filter(|x| x.1 >= 3.84).take(20).map(|(name, mi, epc, sepc)| vec![text(name.clone()), num(*mi, 2), num(*epc, 3), num(*sepc, 3)]).collect();
            if !shown.is_empty() {
                b.tables.push(Table {
                    id: "modindices".into(),
                    title: "Modification indices".into(),
                    columns: vec!["Parameter".into(), "MI".into(), "Expected change".into(), "Standardized expected change".into()],
                    rows: shown,
                    footnotes: vec!["The largest modification indices of at least 3.84 (χ²(1) at α = .05), up to 20: the expected drop in χ² if the parameter were freed. Free parameters only for theoretical reasons.".into()],
                });
            }
        }
    }

    // The path diagram, with standardized estimates on its arrows (one value per group, in group order).
    if let Ok(mut d) = diagram::structure(&spec.model) {
        // The covariances lavaan adds by default (exogenous factors, pure outcomes, fixed predictors) are drawn too.
        for q in model.params.iter().filter(|q| q.group == 0 && q.op == "~~" && q.i != q.j && (q.free.is_some() || q.exo)) {
            let has = |n: &str| d.nodes.iter().any(|x| x.name == n);
            let drawn = d.edges.iter().any(|e| e.kind == diagram::EdgeKind::Covariance && ((e.from == q.lhs && e.to == q.rhs) || (e.from == q.rhs && e.to == q.lhs)));
            if has(&q.lhs) && has(&q.rhs) && !drawn {
                d.edges.push(diagram::Edge { from: q.lhs.clone(), to: q.rhs.clone(), kind: diagram::EdgeKind::Covariance, modifier: None });
            }
        }
        diagram::layout(&mut d, &spec.layout);
        let mut labels = std::collections::BTreeMap::new();
        for e in &d.edges {
            let key = e.param();
            let alt_key = if e.kind == diagram::EdgeKind::Covariance { format!("{} ~~ {}", e.to, e.from) } else { key.clone() };
            let vals: Vec<String> = names.iter().enumerate().filter(|(_, nm)| **nm == key || **nm == alt_key).map(|(k, _)| crate::apa::no_leading_zero(std[k], 2)).collect();
            if !vals.is_empty() {
                labels.insert(key, vals.join(" / "));
            }
        }
        let what = if multi { format!("model, with standardized estimates for {} in that order", pr.group_labels.join(" and ")) } else { "model".to_string() };
        let alt = diagram::alt_text(&d, &labels, &what);
        b.charts.push(crate::chart::Chart { id: "diagram".into(), title: "Path diagram with standardized estimates".into(), svg: diagram::svg(&d, &labels, &alt), alt });
    }

    // Diagnostics.
    b.diagnostics.push(if fit.converged {
        Diagnostic { id: "convergence".into(), title: "Convergence".into(), state: DiagnosticState::NoConcernDetected, message: format!("The estimates converged in {} iterations.", fit.iterations) }
    } else {
        Diagnostic { id: "convergence".into(), title: "Convergence".into(), state: DiagnosticState::ReviewNeeded, message: "The estimation stopped before converging; the estimates may not be the maximum likelihood ones. Check the model or give starting values.".into() }
    });
    let negative: Vec<String> = model.params.iter().filter(|p| p.mat == Matrix::S && p.i == p.j && p.free.is_some() && p.value < 0.0).map(|p| p.lhs.clone()).collect();
    b.diagnostics.push(if negative.is_empty() {
        Diagnostic { id: "heywood".into(), title: "Heywood cases".into(), state: DiagnosticState::NoConcernDetected, message: "Every estimated variance is positive.".into() }
    } else {
        Diagnostic { id: "heywood".into(), title: "Heywood cases".into(), state: DiagnosticState::ReviewNeeded, message: format!("Negative variance estimate{} for {}: the solution is improper. The model may be misspecified or the sample too small.", if negative.len() == 1 { "" } else { "s" }, negative.join(", ")) }
    });
    b.diagnostics.push(common::size_diagnostic(n as usize, format!("{} {} for {} free parameters", n as usize, if pr.fiml { "cases (incomplete ones included by FIML)" } else { "complete cases" }, model.nfree)));
    Ok(b)
}
