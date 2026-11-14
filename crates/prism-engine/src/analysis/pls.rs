//! PLS-SEM (checklist C2): partial least squares path modeling, as seminr computes it.
//!
//! The model is written like a covariance-based one: `f =~ x1 + x2` is a mode A (reflective) block, `f <~ x1 + x2` a
//! mode B (formative) block, and `y ~ f + g` the structural paths. Estimation is Lohmöller's algorithm as seminr runs
//! it: standardized indicators (n − 1), outer weights started at 1, then repeatedly construct scores, inner proxies by
//! the path weighting scheme (or factorial or centroid), and new outer weights (mode A: covariances with the proxy;
//! mode B: the regression of the proxy on the block), rescaled so the scores have unit variance, until the weights
//! change by less than 1e-12 in total. Paths are then the regressions of the scores, and loadings the covariances of
//! the indicators with their construct's score.
//!
//! Quality criteria follow seminr: R² and adjusted R²; f² from refitting without each path; VIF of each construct's
//! antecedents; Cronbach's α, ρA (Dijkstra-Henseler), ρC and AVE; HTMT. Also the SRMR of the saturated composite model
//! (Henseler et al., 2014) and Q²predict (PLSpredict, Shmueli et al., 2019) over ten folds of cases i mod 10.
//! Bootstrap standard errors and percentile intervals, and a permutation test of path differences between two groups
//! (PLS-MGA, Chin & Dibbern, 2010), use a seeded generator.

use prismqda_stats::dist::t_two_sided;
use prismqda_stats::rng::SplitMix64;
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use super::linalg::{invert, Mat};
use super::sem::syntax::{self, Op, Statement};
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Diagnostic, DiagnosticState, ResultBundle, Table};

pub const PLS: &str = "pls.model";
pub const METHOD_VERSION: u32 = 1;

fn default_level() -> f64 {
    0.95
}

fn default_seed() -> u64 {
    20_260_101
}

fn default_folds() -> usize {
    10
}

fn default_permutations() -> usize {
    1000
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Scheme {
    #[default]
    Path,
    Factorial,
    Centroid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub model: String,
    #[serde(default)]
    pub scheme: Scheme,
    /// Bootstrap resamples for standard errors and intervals (0: none).
    #[serde(default)]
    pub bootstrap: usize,
    #[serde(default = "default_seed")]
    pub seed: u64,
    #[serde(default = "default_level")]
    pub level: f64,
    /// Folds for Q²predict (0: none).
    #[serde(default = "default_folds")]
    pub folds: usize,
    /// A two-group variable for a multigroup analysis.
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default = "default_permutations")]
    pub permutations: usize,
    /// Positions of the path diagram's constructs and indicators (from the diagram editor).
    #[serde(default)]
    pub layout: std::collections::BTreeMap<String, [f64; 2]>,
}

#[derive(Debug, Clone)]
struct Construct {
    name: String,
    /// Columns of the data matrix.
    items: Vec<usize>,
    mode_b: bool,
}

#[derive(Debug, Clone)]
struct PlsModel {
    constructs: Vec<Construct>,
    /// (from, to) construct indices.
    paths: Vec<(usize, usize)>,
    /// Indicator names, by column.
    items: Vec<String>,
}

impl PlsModel {
    fn antecedents(&self, c: usize) -> Vec<usize> {
        self.paths.iter().filter(|p| p.1 == c).map(|p| p.0).collect()
    }

    fn endogenous(&self) -> Vec<usize> {
        (0..self.constructs.len()).filter(|&c| self.paths.iter().any(|p| p.1 == c)).collect()
    }

    /// The model without one path, and without any construct left with no paths.
    fn without(&self, path: (usize, usize)) -> (PlsModel, Vec<Option<usize>>) {
        let paths: Vec<(usize, usize)> = self.paths.iter().copied().filter(|p| *p != path).collect();
        let keep: Vec<usize> = (0..self.constructs.len()).filter(|&c| paths.iter().any(|p| p.0 == c || p.1 == c)).collect();
        let map: Vec<Option<usize>> = (0..self.constructs.len()).map(|c| keep.iter().position(|k| *k == c)).collect();
        let m = PlsModel {
            constructs: keep.iter().map(|&c| self.constructs[c].clone()).collect(),
            paths: paths.iter().map(|&(a, b)| (map[a].expect("kept"), map[b].expect("kept"))).collect(),
            items: self.items.clone(),
        };
        (m, map)
    }
}

fn parse(ds: &Dataset, text: &str) -> Result<(PlsModel, Vec<String>)> {
    let statements = syntax::parse(text)?;
    let mut constructs: Vec<Construct> = Vec::new();
    let mut items: Vec<String> = Vec::new();
    let mut paths: Vec<(String, String)> = Vec::new();
    for s in &statements {
        match s {
            Statement::Relation { lhs, op: op @ (Op::Measured | Op::Composite), rhs } => {
                if ds.find(lhs).is_some() {
                    return Err(Error::Invalid(format!("\"{lhs}\" is a variable in the data, so it can't also be a construct.")));
                }
                if ds.find(&rhs.var).is_none() {
                    return Err(Error::Invalid(format!("\"{}\" is not a variable in the data; in PLS-SEM every indicator is observed.", rhs.var)));
                }
                if !rhs.modifiers.is_empty() {
                    return Err(Error::Invalid("Labels and fixed values are not used in PLS-SEM.".into()));
                }
                if items.contains(&rhs.var) && !constructs.iter().any(|c| c.name == *lhs && c.items.iter().any(|&i| items[i] == rhs.var)) {
                    return Err(Error::Invalid(format!("{} indicates more than one construct; in PLS-SEM each indicator belongs to one block.", rhs.var)));
                }
                let col = match items.iter().position(|i| *i == rhs.var) {
                    Some(c) => c,
                    None => {
                        items.push(rhs.var.clone());
                        items.len() - 1
                    }
                };
                let mode_b = *op == Op::Composite;
                match constructs.iter_mut().find(|c| c.name == *lhs) {
                    Some(c) => {
                        if c.mode_b != mode_b {
                            return Err(Error::Invalid(format!("{lhs} is written with both =~ and <~.")));
                        }
                        c.items.push(col);
                    }
                    None => constructs.push(Construct { name: lhs.clone(), items: vec![col], mode_b }),
                }
            }
            Statement::Relation { lhs, op: Op::Regression, rhs } => {
                if rhs.var == "1" || !rhs.modifiers.is_empty() {
                    return Err(Error::Invalid("Intercepts, labels and fixed values are not used in PLS-SEM.".into()));
                }
                paths.push((rhs.var.clone(), lhs.clone()));
            }
            _ => return Err(Error::Invalid("A PLS-SEM model uses =~ (mode A), <~ (mode B) and ~ (paths) only.".into())),
        }
    }
    let idx = |n: &str| -> Result<usize> { constructs.iter().position(|c| c.name == n).ok_or_else(|| Error::Invalid(format!("\"{n}\" in a path is not a construct; define it with =~ or <~ (a single indicator is fine).")) ) };
    let mut pp = Vec::new();
    for (a, b) in &paths {
        let p = (idx(a)?, idx(b)?);
        if p.0 == p.1 || pp.contains(&p) {
            return Err(Error::Invalid(format!("The path {a} → {b} is repeated or loops back.")));
        }
        pp.push(p);
    }
    if pp.is_empty() {
        return Err(Error::Invalid("The model has no structural paths (y ~ x).".into()));
    }
    for c in &constructs {
        let i = idx(&c.name)?;
        if !pp.iter().any(|p| p.0 == i || p.1 == i) {
            return Err(Error::Invalid(format!("{} is not in any path.", c.name)));
        }
    }
    // Recursive models only.
    let n = constructs.len();
    let mut reach = vec![vec![false; n]; n];
    for &(a, b) in &pp {
        reach[a][b] = true;
    }
    for k in 0..n {
        for i in 0..n {
            for j in 0..n {
                if reach[i][k] && reach[k][j] {
                    reach[i][j] = true;
                }
            }
        }
    }
    if (0..n).any(|i| reach[i][i]) {
        return Err(Error::Invalid("The structural model has a feedback loop; PLS-SEM needs a recursive model.".into()));
    }
    Ok((PlsModel { constructs, paths: pp, items: items.clone() }, items))
}

fn standardize(cols: &mut [Vec<f64>]) -> Option<(Vec<f64>, Vec<f64>)> {
    let n = cols.first().map_or(0, Vec::len) as f64;
    let mut means = Vec::new();
    let mut sds = Vec::new();
    for c in cols.iter_mut() {
        let m = c.iter().sum::<f64>() / n;
        let sd = (c.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (n - 1.0)).sqrt();
        if sd.is_nan() || sd <= 0.0 {
            return None;
        }
        c.iter_mut().for_each(|v| *v = (*v - m) / sd);
        means.push(m);
        sds.push(sd);
    }
    Some((means, sds))
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn cor_of(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len() as f64;
    let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        sab += (x - ma) * (y - mb);
        saa += (x - ma).powi(2);
        sbb += (y - mb).powi(2);
    }
    sab / (saa * sbb).sqrt()
}

/// Regression coefficients of standardized y on standardized xs (from correlations).
fn regress(r: &Mat, xs: &[usize], y: usize) -> Option<Vec<f64>> {
    let rxx: Mat = xs.iter().map(|&a| xs.iter().map(|&b| r[a][b]).collect()).collect();
    let inv = invert(&rxx)?;
    Some(inv.iter().map(|row| row.iter().zip(xs).map(|(v, &x)| v * r[x][y]).sum()).collect())
}

#[derive(Debug, Clone)]
struct Estimate {
    /// Outer weights and loadings by item column (of the item's construct).
    weights: Vec<f64>,
    loadings: Vec<f64>,
    /// Path coefficients, in the order of `model.paths`.
    paths: Vec<f64>,
    /// Construct-score correlations.
    r: Mat,
    /// R² of each construct (NaN for exogenous).
    r2: Vec<f64>,
    iterations: usize,
    /// Indicator means and standard deviations (for prediction).
    means: Vec<f64>,
    sds: Vec<f64>,
}

/// Runs the PLS algorithm on raw data (rows × item columns).
fn estimate(model: &PlsModel, data: &[Vec<f64>], scheme: Scheme) -> Option<Estimate> {
    let n = data.len();
    let nf = n as f64;
    let ni = model.items.len();
    let mut z: Vec<Vec<f64>> = (0..ni).map(|j| data.iter().map(|r| r[j]).collect()).collect();
    let (means, sds) = standardize(&mut z)?;
    let k = model.constructs.len();
    let mut w = vec![0.0; ni];
    for c in &model.constructs {
        for &i in &c.items {
            w[i] = 1.0;
        }
    }
    let score = |w: &[f64], c: &Construct| -> Vec<f64> { (0..n).map(|r| c.items.iter().map(|&i| z[i][r] * w[i]).sum()).collect() };
    let stdz = |v: &mut Vec<f64>| -> Option<()> {
        let m = v.iter().sum::<f64>() / nf;
        let sd = (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (nf - 1.0)).sqrt();
        if sd.is_nan() || sd <= 0.0 {
            return None;
        }
        v.iter_mut().for_each(|x| *x = (*x - m) / sd);
        Some(())
    };
    let mut iterations = 0;
    for it in 0..=1000 {
        iterations = it;
        let mut s: Vec<Vec<f64>> = model.constructs.iter().map(|c| score(&w, c)).collect();
        for v in s.iter_mut() {
            stdz(v)?;
        }
        let r: Mat = (0..k).map(|a| (0..k).map(|b| if a == b { 1.0 } else { dot(&s[a], &s[b]) / (nf - 1.0) }).collect()).collect();
        // Inner weights e[from][to]: how construct `from` enters the proxy of `to`.
        let mut e = vec![vec![0.0; k]; k];
        for &(a, b) in &model.paths {
            match scheme {
                Scheme::Path => e[b][a] = r[a][b],
                Scheme::Factorial => {
                    e[a][b] = r[a][b];
                    e[b][a] = r[a][b];
                }
                Scheme::Centroid => {
                    e[a][b] = r[a][b].signum();
                    e[b][a] = r[a][b].signum();
                }
            }
        }
        if scheme == Scheme::Path {
            for c in model.endogenous() {
                let ante = model.antecedents(c);
                let b = regress(&r, &ante, c)?;
                for (x, v) in ante.iter().zip(b) {
                    e[*x][c] = v;
                }
            }
        }
        let mut proxy: Vec<Vec<f64>> = (0..k).map(|c| (0..n).map(|row| (0..k).map(|f| s[f][row] * e[f][c]).sum()).collect()).collect();
        for v in proxy.iter_mut() {
            stdz(v)?;
        }
        let old = w.clone();
        for (ci, c) in model.constructs.iter().enumerate() {
            if c.mode_b {
                let rxx: Mat = c.items.iter().map(|&a| c.items.iter().map(|&b| dot(&z[a], &z[b]) / (nf - 1.0)).collect()).collect();
                let rxy: Vec<f64> = c.items.iter().map(|&a| dot(&z[a], &proxy[ci]) / (nf - 1.0)).collect();
                let inv = invert(&rxx)?;
                for (row, &i) in inv.iter().zip(&c.items) {
                    w[i] = dot(row, &rxy);
                }
            } else {
                for &i in &c.items {
                    w[i] = dot(&z[i], &proxy[ci]) / (nf - 1.0);
                }
            }
            let sc = score(&w, c);
            let sd = (sc.iter().map(|v| v * v).sum::<f64>() / (nf - 1.0)).sqrt();
            for &i in &c.items {
                w[i] /= sd;
            }
        }
        let diff: f64 = w.iter().zip(&old).map(|(a, b)| (a - b).abs()).sum();
        if diff < 1e-12 {
            break;
        }
    }
    let s: Vec<Vec<f64>> = model.constructs.iter().map(|c| score(&w, c)).collect();
    let r: Mat = (0..k).map(|a| (0..k).map(|b| if a == b { 1.0 } else { cor_of(&s[a], &s[b]) }).collect()).collect();
    let mut loadings = vec![0.0; ni];
    for (ci, c) in model.constructs.iter().enumerate() {
        for &i in &c.items {
            loadings[i] = dot(&z[i], &s[ci]) / (nf - 1.0);
        }
    }
    let mut paths = vec![0.0; model.paths.len()];
    let mut r2 = vec![f64::NAN; k];
    for c in model.endogenous() {
        let ante = model.antecedents(c);
        let b = regress(&r, &ante, c)?;
        r2[c] = ante.iter().zip(&b).map(|(&x, v)| v * r[x][c]).sum();
        for (x, v) in ante.iter().zip(b) {
            let pi = model.paths.iter().position(|p| *p == (*x, c)).expect("a path");
            paths[pi] = v;
        }
    }
    Some(Estimate { weights: w, loadings, paths, r, r2, iterations, means, sds })
}

/// Total effects: (I − B)⁻¹ − I for the path matrix B (row: from, column: to).
fn total_effects(model: &PlsModel, est: &Estimate) -> Mat {
    let k = model.constructs.len();
    let mut b = vec![vec![0.0; k]; k];
    for (p, &(a, c)) in model.paths.iter().enumerate() {
        b[a][c] = est.paths[p];
    }
    let mut total = b.clone();
    let mut power = b.clone();
    for _ in 1..k {
        power = (0..k).map(|i| (0..k).map(|j| (0..k).map(|l| power[i][l] * b[l][j]).sum()).collect()).collect();
        for i in 0..k {
            for j in 0..k {
                total[i][j] += power[i][j];
            }
        }
    }
    total
}

/// Predicted indicator values of `test` rows from a model estimated on training data (direct antecedents, as
/// seminr's predict_DA).
fn predict(model: &PlsModel, est: &Estimate, test: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let k = model.constructs.len();
    let scores: Vec<Vec<f64>> = test
        .iter()
        .map(|row| model.constructs.iter().map(|c| c.items.iter().map(|&i| (row[i] - est.means[i]) / est.sds[i] * est.weights[i]).sum()).collect())
        .collect();
    test.iter()
        .zip(&scores)
        .map(|(_, s)| {
            let pred: Vec<f64> = (0..k)
                .map(|c| {
                    let ante: Vec<(usize, f64)> = model.paths.iter().enumerate().filter(|(_, p)| p.1 == c).map(|(pi, p)| (p.0, est.paths[pi])).collect();
                    if ante.is_empty() {
                        s[c]
                    } else {
                        ante.iter().map(|(a, b)| s[*a] * b).sum()
                    }
                })
                .collect();
            let mut out = vec![0.0; model.items.len()];
            for (ci, c) in model.constructs.iter().enumerate() {
                for &i in &c.items {
                    out[i] = pred[ci] * est.loadings[i] * est.sds[i] + est.means[i];
                }
            }
            out
        })
        .collect()
}

fn alpha(r: &Mat) -> f64 {
    let k = r.len() as f64;
    let total: f64 = r.iter().flatten().sum();
    k / (k - 1.0) * (1.0 - k / total)
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    check_level(spec.level)?;
    if spec.bootstrap > 0 && !(100..=100_000).contains(&spec.bootstrap) {
        return Err(Error::Invalid("Use between 100 and 100,000 bootstrap resamples.".into()));
    }
    let (model, items) = parse(ds, &spec.model)?;
    let cols: Vec<Vec<Slot<f64>>> = items.iter().map(|v| numeric(ds, v)).collect::<Result<_>>()?;
    let groups = spec.group.as_ref().map(|g| categories(ds, g)).transpose()?;
    if let Some((_, levels)) = &groups {
        if levels.len() != 2 {
            return Err(Error::Invalid("The multigroup analysis compares two groups; the grouping variable has a different number of categories.".into()));
        }
    }
    let total = ds.n_rows();
    let mut tally = Tally::default();
    let mut data: Vec<Vec<f64>> = Vec::new();
    let mut group_of: Vec<usize> = Vec::new();
    'case: for r in 0..total {
        let mut row = Vec::with_capacity(cols.len());
        for (c, name) in cols.iter().zip(&items) {
            if tally.check(&c[r], name) {
                continue 'case;
            }
            row.push(c[r].value().expect("checked"));
        }
        if let Some((slots, _)) = &groups {
            if tally.check(&slots[r], "group") {
                continue;
            }
            group_of.push(slots[r].value().expect("checked"));
        }
        data.push(row);
    }
    let n = data.len();
    let lv = level_text(spec.level);
    let scheme_name = match spec.scheme {
        Scheme::Path => "path",
        Scheme::Factorial => "factorial",
        Scheme::Centroid => "centroid",
    };
    let note = format!(
        "PLS-SEM by Lohmöller's algorithm with the {scheme_name} weighting scheme, as seminr: standardized indicators, mode A blocks (=~) weighted by covariances and mode B blocks (<~) by regression, iterated until the outer weights change by less than 1e-12 in total. Paths are regressions of the construct scores. Reliability as seminr (α, ρA, ρC, AVE), HTMT (arithmetic means), f² by refitting without each path, SRMR of the saturated composite model (Henseler et al., 2014){}{}. Computed by PrismStatistics.",
        if spec.folds >= 2 { format!(", and Q²predict over {} folds of cases (case i in fold i mod {})", spec.folds, spec.folds) } else { String::new() },
        if spec.bootstrap > 0 { format!("; standard errors, t tests (n − 1 df) and {lv} percentile intervals from {} bootstrap resamples (seed {})", spec.bootstrap, spec.seed) } else { String::new() },
    );
    let mut b = common::bundle(PLS, METHOD_VERSION, "PLS-SEM".into(), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, n), note);
    set(&mut b, "n", n as f64);
    set(&mut b, "level", spec.level);
    if n < 10 || n <= model.constructs.len() + 1 {
        fail(&mut b, "There are too few complete cases for this model.");
        return Ok(b);
    }
    let Some(est) = estimate(&model, &data, spec.scheme) else {
        fail(&mut b, "The model could not be estimated: an indicator is constant, a formative block's indicators are collinear, or a construct's antecedents are collinear.");
        return Ok(b);
    };
    set(&mut b, "iterations", est.iterations as f64);
    let names: Vec<&str> = model.constructs.iter().map(|c| c.name.as_str()).collect();
    let k = names.len();

    // Bootstrap.
    let boot: Option<(Vec<Vec<f64>>, usize)> = (spec.bootstrap > 0).then(|| {
        let mut rng = SplitMix64::new(spec.seed);
        let mut reps = Vec::new();
        let mut failed = 0;
        for r in 0..spec.bootstrap {
            if crate::job::step(r, spec.bootstrap) {
                break;
            }
            let rs: Vec<Vec<f64>> = (0..n).map(|_| data[rng.below(n)].clone()).collect();
            match estimate(&model, &rs, spec.scheme) {
                Some(e) => {
                    let tot = total_effects(&model, &e);
                    let mut v = e.paths.clone();
                    v.extend(&e.loadings);
                    v.extend(&e.weights);
                    v.extend(tot.iter().flatten());
                    reps.push(v);
                }
                None => failed += 1,
            }
        }
        (reps, failed)
    });
    let stat = |idx: usize, estv: f64| -> Option<(f64, f64, f64, f64, f64)> {
        let (reps, _) = boot.as_ref()?;
        let v: Vec<f64> = reps.iter().map(|r| r[idx]).collect();
        let m = v.len() as f64;
        if m < 20.0 {
            return None;
        }
        let mean = v.iter().sum::<f64>() / m;
        let se = (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (m - 1.0)).sqrt();
        let mut s = v.clone();
        s.sort_by(f64::total_cmp);
        let quant = |p: f64| {
            let h = (m - 1.0) * p;
            let lo = h.floor() as usize;
            let hi = (lo + 1).min(s.len() - 1);
            s[lo] + (h - lo as f64) * (s[hi] - s[lo])
        };
        let t = estv / se;
        Some((se, t, t_two_sided(t, n as f64 - 1.0), quant((1.0 - spec.level) / 2.0), quant((1.0 + spec.level) / 2.0)))
    };
    let np = model.paths.len();
    let ni = items.len();
    if let Some((reps, failed)) = &boot {
        set(&mut b, "boot.used", reps.len() as f64);
        set(&mut b, "boot.failed", *failed as f64);
    }
    let boot_cells = |idx: usize, estv: f64, b: &mut ResultBundle, key: &str| -> Vec<crate::result::Cell> {
        match stat(idx, estv) {
            Some((se, t, p, lo, hi)) => {
                for (f, v) in [("se", se), ("t", t), ("p", p), ("lower", lo), ("upper", hi)] {
                    set(b, format!("{key}.{f}"), v);
                }
                vec![num(se, 3), num(t, 2), common::p(p), num(lo, 3), num(hi, 3)]
            }
            None => vec![],
        }
    };

    // f² and VIF.
    let mut f2 = vec![f64::NAN; np];
    for (pi, &(a, c)) in model.paths.iter().enumerate() {
        let (reduced, map) = model.without((a, c));
        let without = match map[c] {
            Some(cc) if reduced.paths.iter().any(|p| p.1 == cc) => estimate(&reduced, &data, spec.scheme).map(|e| e.r2[cc]),
            _ => Some(0.0),
        };
        if let Some(w0) = without {
            f2[pi] = (est.r2[c] - w0) / (1.0 - est.r2[c]);
        }
    }
    let mut path_rows = Vec::new();
    for (pi, &(a, c)) in model.paths.iter().enumerate() {
        let key = format!("path.{}.{}", names[a], names[c]);
        set(&mut b, key.clone(), est.paths[pi]);
        set(&mut b, format!("f2.{}.{}", names[a], names[c]), f2[pi]);
        let ante = model.antecedents(c);
        let vif = if ante.len() >= 2 {
            let others: Vec<usize> = ante.iter().copied().filter(|&x| x != a).collect();
            regress(&est.r, &others, a).map(|bb| {
                let r2: f64 = others.iter().zip(&bb).map(|(&o, v)| v * est.r[o][a]).sum();
                1.0 / (1.0 - r2)
            })
        } else {
            None
        };
        if let Some(v) = vif {
            set(&mut b, format!("vif.{}.{}", names[c], names[a]), v);
        }
        let mut row = vec![text(format!("{} → {}", names[a], names[c])), num(est.paths[pi], 3)];
        row.extend(boot_cells(pi, est.paths[pi], &mut b, &key));
        row.push(num(f2[pi], 3));
        row.push(vif.map_or(text(""), |v| num(v, 2)));
        path_rows.push(row);
    }
    let boot_cols = |c: &mut Vec<String>| {
        if boot.is_some() {
            c.extend(["SE (bootstrap)".to_string(), "t".into(), "p".into(), format!("{lv} CI lower"), format!("{lv} CI upper")]);
        }
    };
    let mut cols = vec!["Path".to_string(), "β".into()];
    boot_cols(&mut cols);
    cols.extend(["f²".to_string(), "VIF".into()]);
    b.tables.push(Table { id: "paths".into(), title: "Path coefficients".into(), columns: cols, rows: path_rows, footnotes: vec!["f²: the change in R² of the outcome when the path is removed and the model refitted, over 1 − R². VIF: of the construct among the outcome's antecedents. Common guides: f² of .02, .15 and .35 are small, medium and large; VIF above 5 (or 3) signals collinearity.".into()] });

    // R².
    let mut r2_rows = Vec::new();
    for c in model.endogenous() {
        let ante = model.antecedents(c).len() as f64;
        let adj = 1.0 - (1.0 - est.r2[c]) * (n as f64 - 1.0) / (n as f64 - ante - 1.0);
        set(&mut b, format!("r2.{}", names[c]), est.r2[c]);
        set(&mut b, format!("adjr2.{}", names[c]), adj);
        r2_rows.push(vec![text(names[c]), num(est.r2[c], 3), num(adj, 3)]);
    }
    b.tables.push(Table { id: "r2".into(), title: "Variance explained".into(), columns: vec!["Construct".into(), "R²".into(), "Adjusted R²".into()], rows: r2_rows, footnotes: Vec::new() });

    // Loadings and weights.
    let mut mm_rows = Vec::new();
    for c in &model.constructs {
        for &i in &c.items {
            set(&mut b, format!("loading.{}", items[i]), est.loadings[i]);
            set(&mut b, format!("weight.{}", items[i]), est.weights[i]);
            let mut row = vec![text(c.name.clone()), text(items[i].clone()), text(if c.mode_b { "B" } else { "A" }), num(est.loadings[i], 3)];
            row.extend(boot_cells(np + i, est.loadings[i], &mut b, &format!("loading.{}", items[i])));
            row.push(num(est.weights[i], 3));
            row.extend(boot_cells(np + ni + i, est.weights[i], &mut b, &format!("weight.{}", items[i])));
            mm_rows.push(row);
        }
    }
    let mut cols = vec!["Construct".to_string(), "Indicator".into(), "Mode".into(), "Loading".into()];
    boot_cols(&mut cols);
    cols.push("Weight".into());
    boot_cols(&mut cols);
    b.tables.push(Table { id: "measurement".into(), title: "Outer loadings and weights".into(), columns: cols, rows: mm_rows, footnotes: vec!["Mode A blocks (=~) are weighted by the indicators' covariances with the construct; mode B blocks (<~) by regression. Loadings are correlations of the standardized indicators with the construct scores.".into()] });

    // Reliability.
    let zcols: Vec<Vec<f64>> = {
        let mut z: Vec<Vec<f64>> = (0..ni).map(|j| data.iter().map(|r| r[j]).collect()).collect();
        standardize(&mut z);
        z
    };
    let rr = |i: usize, j: usize| dot(&zcols[i], &zcols[j]) / (n as f64 - 1.0);
    let mut rel_rows = Vec::new();
    for c in &model.constructs {
        let it = &c.items;
        let (a, rho_a, rho_c, ave) = if it.len() == 1 {
            (1.0, 1.0, 1.0, 1.0)
        } else {
            let rm: Mat = it.iter().map(|&i| it.iter().map(|&j| rr(i, j)).collect()).collect();
            let l: Vec<f64> = it.iter().map(|&i| est.loadings[i]).collect();
            let sl: f64 = l.iter().sum();
            let rho_c = sl * sl / (sl * sl + l.iter().map(|x| 1.0 - x * x).sum::<f64>());
            let ave = l.iter().map(|x| x * x).sum::<f64>() / l.len() as f64;
            let rho_a = if c.mode_b {
                1.0
            } else {
                let w: Vec<f64> = it.iter().map(|&i| est.weights[i]).collect();
                let ww: f64 = dot(&w, &w);
                let (mut num_, mut den) = (0.0, 0.0);
                for a in 0..w.len() {
                    for bb in 0..w.len() {
                        if a != bb {
                            num_ += w[a] * rm[a][bb] * w[bb];
                            den += w[a] * w[a] * w[bb] * w[bb];
                        }
                    }
                }
                ww * ww * num_ / den
            };
            (alpha(&rm), rho_a, rho_c, ave)
        };
        for (f, v) in [("alpha", a), ("rhoA", rho_a), ("rhoC", rho_c), ("ave", ave)] {
            set(&mut b, format!("{f}.{}", c.name), v);
        }
        rel_rows.push(vec![text(c.name.clone()), num(a, 3), num(rho_a, 3), num(rho_c, 3), num(ave, 3)]);
    }
    b.tables.push(Table { id: "reliability".into(), title: "Reliability and convergent validity".into(), columns: vec!["Construct".into(), "Cronbach's α".into(), "ρA".into(), "ρC".into(), "AVE".into()], rows: rel_rows, footnotes: vec!["α, ρA and ρC should exceed .70 and AVE .50 (for mode A blocks). Single-indicator constructs are given 1, and ρA of mode B blocks is 1, as in seminr.".into()] });

    // HTMT (arithmetic means; a single-indicator construct's monotrait mean is 1).
    let mono = |c: &Construct| -> f64 {
        if c.items.len() < 2 {
            return 1.0;
        }
        let mut v = Vec::new();
        for (a, &i) in c.items.iter().enumerate() {
            for &j in &c.items[a + 1..] {
                v.push(rr(i, j).abs());
            }
        }
        v.iter().sum::<f64>() / v.len() as f64
    };
    let mut htmt_rows = Vec::new();
    for (ai, ca) in model.constructs.iter().enumerate() {
        let mut row = vec![text(ca.name.clone())];
        for cb in model.constructs.iter().take(ai) {
            let het: Vec<f64> = ca.items.iter().flat_map(|&i| cb.items.iter().map(move |&j| (i, j))).map(|(i, j)| rr(i, j).abs()).collect();
            let h = het.iter().sum::<f64>() / het.len() as f64 / (mono(ca) * mono(cb)).sqrt();
            set(&mut b, format!("htmt.{}.{}", cb.name, ca.name), h);
            row.push(num(h, 3));
        }
        row.push(text("—"));
        while row.len() <= k {
            row.push(text(""));
        }
        htmt_rows.push(row);
    }
    let mut cols = vec!["Construct".to_string()];
    cols.extend(names.iter().map(|s| s.to_string()));
    b.tables.push(Table { id: "htmt".into(), title: "Discriminant validity: HTMT".into(), columns: cols, rows: htmt_rows, footnotes: vec!["Heterotrait-monotrait ratios (Henseler, Ringle & Sarstedt, 2015) from the absolute indicator correlations; values below .85 (or .90) suggest discriminant validity.".into()] });

    // Total effects.
    let tot = total_effects(&model, &est);
    let mut tot_rows = Vec::new();
    for a in 0..k {
        for c in 0..k {
            if tot[a][c] != 0.0 {
                let key = format!("total.{}.{}", names[a], names[c]);
                set(&mut b, key.clone(), tot[a][c]);
                let mut row = vec![text(format!("{} → {}", names[a], names[c])), num(tot[a][c], 3)];
                row.extend(boot_cells(np + 2 * ni + a * k + c, tot[a][c], &mut b, &key));
                tot_rows.push(row);
            }
        }
    }
    let mut cols = vec!["Effect".to_string(), "Total".into()];
    boot_cols(&mut cols);
    b.tables.push(Table { id: "total".into(), title: "Total effects".into(), columns: cols, rows: tot_rows, footnotes: vec!["Direct plus indirect effects through every route.".into()] });

    // SRMR of the saturated composite model.
    let block: Vec<usize> = (0..ni).map(|i| model.constructs.iter().position(|c| c.items.contains(&i)).expect("a block")).collect();
    let mut ss = 0.0;
    let mut cnt = 0.0;
    for i in 0..ni {
        for j in 0..=i {
            let s = rr(i, j);
            let sig = if block[i] == block[j] { s } else { est.loadings[i] * est.r[block[i]][block[j]] * est.loadings[j] };
            ss += (s - sig).powi(2);
            cnt += 1.0;
        }
    }
    let srmr = (ss / cnt).sqrt();
    set(&mut b, "srmr", srmr);

    // Q²predict.
    let mut q2_rows = Vec::new();
    if spec.folds >= 2 && n >= 2 * spec.folds {
        let endo: Vec<usize> = model.endogenous();
        let mut sse = vec![0.0; ni];
        let mut sso = vec![0.0; ni];
        let mut ok = true;
        for f in 0..spec.folds {
            let train: Vec<Vec<f64>> = data.iter().enumerate().filter(|(r, _)| r % spec.folds != f).map(|(_, x)| x.clone()).collect();
            let test: Vec<Vec<f64>> = data.iter().enumerate().filter(|(r, _)| r % spec.folds == f).map(|(_, x)| x.clone()).collect();
            let Some(et) = estimate(&model, &train, spec.scheme) else {
                ok = false;
                break;
            };
            let pred = predict(&model, &et, &test);
            for &c in &endo {
                for &i in &model.constructs[c].items {
                    let mean = train.iter().map(|r| r[i]).sum::<f64>() / train.len() as f64;
                    for (row, p) in test.iter().zip(&pred) {
                        sse[i] += (row[i] - p[i]).powi(2);
                        sso[i] += (row[i] - mean).powi(2);
                    }
                }
            }
        }
        if ok {
            for &c in &endo {
                for &i in &model.constructs[c].items {
                    let q2 = 1.0 - sse[i] / sso[i];
                    set(&mut b, format!("q2.{}", items[i]), q2);
                    q2_rows.push(vec![text(names[c]), text(items[i].clone()), num(q2, 3)]);
                }
            }
        }
    }
    let mut fit_rows = vec![vec![text("SRMR (saturated model)"), text(crate::apa::no_leading_zero(srmr, 3))], vec![text("Iterations"), count(est.iterations)], vec![text("Cases"), count(n)]];
    if let Some((reps, failed)) = &boot {
        fit_rows.push(vec![text("Bootstrap resamples used"), text(format!("{} of {}", reps.len(), reps.len() + failed))]);
    }
    b.tables.insert(0, Table { id: "fit".into(), title: "Model fit".into(), columns: vec!["Measure".into(), "Value".into()], rows: fit_rows, footnotes: vec!["SRMR compares the indicator correlations with those implied by the loadings and the construct correlations (Henseler et al., 2014); below .08 is commonly taken as good fit.".into()] });
    if !q2_rows.is_empty() {
        b.tables.push(Table { id: "q2predict".into(), title: "Q²predict".into(), columns: vec!["Construct".into(), "Indicator".into(), "Q²predict".into()], rows: q2_rows, footnotes: vec![format!("PLSpredict (Shmueli et al., 2019) over {} folds (case i in fold i mod {}): 1 minus the prediction errors over the errors of the training mean. Above 0, the model predicts better than the mean.", spec.folds, spec.folds)] });
    }

    // Multigroup analysis.
    if let Some((_, levels)) = &groups {
        let est_group = |g: usize, labels: &[usize]| -> Option<Estimate> {
            let rows: Vec<Vec<f64>> = data.iter().zip(labels).filter(|(_, l)| **l == g).map(|(r, _)| r.clone()).collect();
            estimate(&model, &rows, spec.scheme)
        };
        if let (Some(e0), Some(e1)) = (est_group(0, &group_of), est_group(1, &group_of)) {
            let mut rng = SplitMix64::new(spec.seed ^ 0x5eed);
            let obs: Vec<f64> = e0.paths.iter().zip(&e1.paths).map(|(a, c)| a - c).collect();
            let mut exceed = vec![0usize; np];
            let mut used = 0usize;
            let mut labels = group_of.clone();
            for _ in 0..spec.permutations {
                for i in (1..labels.len()).rev() {
                    let j = rng.below(i + 1);
                    labels.swap(i, j);
                }
                if let (Some(a), Some(c)) = (est_group(0, &labels), est_group(1, &labels)) {
                    used += 1;
                    for p in 0..np {
                        if (a.paths[p] - c.paths[p]).abs() >= obs[p].abs() - 1e-12 {
                            exceed[p] += 1;
                        }
                    }
                }
            }
            let mut rows = Vec::new();
            for (pi, &(a, c)) in model.paths.iter().enumerate() {
                for (g, e) in [(0, &e0), (1, &e1)] {
                    set(&mut b, format!("mga.{}.path.{}.{}", levels[g].label, names[a], names[c]), e.paths[pi]);
                }
                let pv = (exceed[pi] + 1) as f64 / (used + 1) as f64;
                set(&mut b, format!("mga.p.{}.{}", names[a], names[c]), pv);
                rows.push(vec![text(format!("{} → {}", names[a], names[c])), num(e0.paths[pi], 3), num(e1.paths[pi], 3), num(obs[pi], 3), common::p(pv)]);
            }
            b.tables.push(Table {
                id: "mga".into(),
                title: format!("Multigroup analysis: {} vs {}", levels[0].label, levels[1].label),
                columns: vec!["Path".into(), format!("β ({})", levels[0].label), format!("β ({})", levels[1].label), "Difference".into(), "p (permutation)".into()],
                rows,
                footnotes: vec![format!("Each group estimated separately; p is the share of {used} permutations of the group labels (seed {}) with a difference at least as large, (count + 1)/(permutations + 1) (Chin & Dibbern, 2010).", spec.seed)],
            });
        } else {
            b.diagnostics.push(Diagnostic { id: "mga".into(), title: "Multigroup analysis".into(), state: DiagnosticState::ReviewNeeded, message: "The model could not be estimated in one of the groups, so the groups were not compared.".into() });
        }
    }
    // The path diagram: loadings (mode A), weights (mode B) and path coefficients.
    if let Ok(mut d) = super::sem::diagram::structure(&spec.model) {
        use super::sem::diagram::EdgeKind;
        super::sem::diagram::layout(&mut d, &spec.layout);
        let mut labels = std::collections::BTreeMap::new();
        for e in &d.edges {
            let v = match e.kind {
                EdgeKind::Loading => items.iter().position(|i| *i == e.to).map(|i| est.loadings[i]),
                EdgeKind::Composite => items.iter().position(|i| *i == e.from).map(|i| est.weights[i]),
                EdgeKind::Regression => {
                    let (a, c) = (names.iter().position(|n| *n == e.from), names.iter().position(|n| *n == e.to));
                    a.zip(c).and_then(|ac| model.paths.iter().position(|p| *p == ac)).map(|pi| est.paths[pi])
                }
                EdgeKind::Covariance => None,
            };
            if let Some(v) = v {
                labels.insert(e.param(), crate::apa::no_leading_zero(v, 2));
            }
        }
        let alt = super::sem::diagram::alt_text(&d, &labels, "PLS path model, with loadings, weights and path coefficients").replace("Values are standardized estimates.", "Values are loadings (mode A), weights (mode B) and path coefficients.");
        b.charts.push(crate::chart::Chart { id: "diagram".into(), title: "PLS path model".into(), svg: super::sem::diagram::svg(&d, &labels, &alt), alt });
    }
    b.diagnostics.push(if est.iterations < 1000 {
        Diagnostic { id: "convergence".into(), title: "Convergence".into(), state: DiagnosticState::NoConcernDetected, message: format!("The outer weights converged in {} iterations.", est.iterations) }
    } else {
        Diagnostic { id: "convergence".into(), title: "Convergence".into(), state: DiagnosticState::ReviewNeeded, message: "The outer weights had not converged after 1000 iterations.".into() }
    });
    b.diagnostics.push(common::size_diagnostic(n, format!("{n} complete cases")));
    Ok(b)
}
