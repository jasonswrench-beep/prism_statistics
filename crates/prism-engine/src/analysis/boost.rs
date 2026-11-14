//! Gradient boosting (checklist E-8, part 2) as R's gbm (2.3) grows it.
//!
//! Each iteration fits a regression tree to the working response (y − f for squared error; y − p for the Bernoulli
//! deviance) and adds it, shrunk by λ, to the predictions f. A tree is grown best-first, one split at a time, to
//! `depth` splits (gbm's interaction.depth); every split is three-way (left, right, and a branch for missing values)
//! and must leave at least `min_obs` sampled cases on the left and right. A split's improvement is gbm's
//! wL·wR(z̄L − z̄R)²/(wL + wR) (with the three pairwise terms when cases are missing). Numeric predictors split at
//! midpoints (missing values first in the sort, as gbm orders them); categorical predictors order their categories by
//! mean and split that order. Terminal values are the node means (squared error) or one Newton step Σz/Σp(1 − p)
//! (Bernoulli); a missing branch with fewer than `min_obs` cases takes the weighted mean of its siblings, as gbm's
//! Adjust. Cases are subsampled each iteration (`bag_fraction`, gbm's sequential sampling); with bag_fraction = 1 the
//! fit is deterministic and equals gbm's. The number of trees can be chosen by k-fold cross-validation. Relative
//! influence is each predictor's share of the summed split improvements.

#![allow(clippy::needless_range_loop)]

use prismqda_stats::rng::SplitMix64;
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::apa;
use crate::chart;
use crate::error::{Error, Result};
use crate::model::{Column, Dataset, Measure as Level};
use crate::result::{Diagnostic, DiagnosticState, ResultBundle, Table};

pub const BOOST: &str = "predict.boost";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Distribution {
    /// Chosen from the outcome: Bernoulli for a two-category outcome.
    #[default]
    Auto,
    Gaussian,
    Bernoulli,
}

fn hundred() -> usize {
    100
}
fn one() -> usize {
    1
}
fn ten() -> usize {
    10
}
fn lambda() -> f64 {
    0.1
}
fn half() -> f64 {
    0.5
}
fn five() -> usize {
    5
}
fn seed() -> u64 {
    20_260_101
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub dependent: String,
    pub predictors: Vec<String>,
    #[serde(default)]
    pub distribution: Distribution,
    #[serde(default = "hundred")]
    pub n_trees: usize,
    /// gbm's interaction.depth: splits per tree.
    #[serde(default = "one")]
    pub depth: usize,
    #[serde(default = "lambda")]
    pub shrinkage: f64,
    #[serde(default = "half")]
    pub bag_fraction: f64,
    #[serde(default = "ten")]
    pub min_obs: usize,
    /// Cross-validation folds to choose the number of trees (0: none).
    #[serde(default = "five")]
    pub cv_folds: usize,
    #[serde(default)]
    pub fold_variable: Option<String>,
    #[serde(default = "seed")]
    pub seed: u64,
}

pub const MISSING: usize = usize::MAX;

#[derive(Clone)]
pub enum Pred {
    Num(Vec<f64>),
    Cat(Vec<usize>, usize),
}

#[derive(Clone, Debug)]
enum Rule {
    Num(f64),
    /// Categories that go left.
    Cat(Vec<bool>),
}

#[derive(Clone, Debug)]
enum Node {
    Leaf { pred: f64, w: f64, n: usize },
    Split { var: usize, rule: Rule, improve: f64, pred: f64, w: f64, kids: [usize; 3] },
}

/// A tree in an arena; node 0 is the root.
#[derive(Clone, Debug)]
pub struct Tree {
    nodes: Vec<Node>,
}

/// −1 left, 1 right, 0 missing.
fn which(x: &[Pred], var: usize, rule: &Rule, i: usize) -> i8 {
    match (&x[var], rule) {
        (Pred::Num(v), Rule::Num(c)) => {
            if v[i].is_nan() {
                0
            } else if v[i] < *c {
                -1
            } else {
                1
            }
        }
        (Pred::Cat(v, _), Rule::Cat(left)) => {
            if v[i] == MISSING {
                0
            } else if left.get(v[i]).copied().unwrap_or(false) {
                -1
            } else {
                1
            }
        }
        _ => 0,
    }
}

impl Tree {
    pub fn predict(&self, x: &[Pred], i: usize) -> f64 {
        let mut k = 0;
        loop {
            match &self.nodes[k] {
                Node::Leaf { pred, .. } => return *pred,
                Node::Split { var, rule, kids, .. } => {
                    k = match which(x, *var, rule, i) {
                        -1 => kids[0],
                        1 => kids[1],
                        _ => kids[2],
                    };
                }
            }
        }
    }

    /// gbm's Adjust: a missing branch with fewer than `min_obs` cases takes its siblings' weighted mean; a split's
    /// value becomes the weighted mean of its children.
    fn adjust(&mut self, k: usize, min_obs: usize) {
        let (kids, _) = match &self.nodes[k] {
            Node::Leaf { .. } => return,
            Node::Split { kids, .. } => (*kids, ()),
        };
        self.adjust(kids[0], min_obs);
        self.adjust(kids[1], min_obs);
        let info = |t: &Tree, j: usize| -> (f64, f64) {
            match &t.nodes[j] {
                Node::Leaf { pred, w, .. } => (*pred, *w),
                Node::Split { pred, w, .. } => (*pred, *w),
            }
        };
        let (pl, wl) = info(self, kids[0]);
        let (pr, wr) = info(self, kids[1]);
        let small = matches!(self.nodes[kids[2]], Node::Leaf { n, .. } if n < min_obs);
        let newpred = if small {
            let p = (wl * pl + wr * pr) / (wl + wr);
            if let Node::Leaf { pred, .. } = &mut self.nodes[kids[2]] {
                *pred = p;
            }
            p
        } else {
            self.adjust(kids[2], min_obs);
            let (pm, wm) = info(self, kids[2]);
            (wl * pl + wr * pr + wm * pm) / (wl + wr + wm)
        };
        if let Node::Split { pred, .. } = &mut self.nodes[k] {
            *pred = newpred;
        }
    }

    pub fn improvements(&self) -> Vec<(usize, f64)> {
        self.nodes.iter().filter_map(|n| if let Node::Split { var, improve, .. } = n { Some((*var, *improve)) } else { None }).collect()
    }
}

/// gbm's Improvement.
fn improvement(lw: f64, rw: f64, mw: f64, ls: f64, rs: f64, ms: f64) -> f64 {
    if mw == 0.0 {
        let d = ls / lw - rs / rw;
        lw * rw * d * d / (lw + rw)
    } else {
        let mut r = 0.0;
        let d = ls / lw - rs / rw;
        r += lw * rw * d * d;
        let d = ls / lw - ms / mw;
        r += lw * mw * d * d;
        let d = rs / rw - ms / mw;
        r += rw * mw * d * d;
        r / (lw + rw + mw)
    }
}

/// One terminal node's search (gbm's CNodeSearch).
#[derive(Clone)]
struct Search {
    arena: usize,
    init: (f64, f64, usize),
    best_imp: f64,
    best_var: usize,
    best_rule: Option<Rule>,
    best_l: (f64, f64, usize),
    best_r: (f64, f64, usize),
    best_m: (f64, f64, usize),
}

impl Search {
    fn new(arena: usize, init: (f64, f64, usize)) -> Self {
        Search { arena, init, best_imp: 0.0, best_var: usize::MAX, best_rule: None, best_l: (0.0, 0.0, 0), best_r: init, best_m: (0.0, 0.0, 0) }
    }
}

/// The data a boosting run needs: predictors, numeric sort orders (missing first, as gbm's order(na.last = FALSE)).
pub struct Data<'a> {
    pub x: &'a [Pred],
    pub order: Vec<Vec<usize>>,
}

pub fn orders(x: &[Pred], rows: &[usize]) -> Vec<Vec<usize>> {
    x.iter()
        .map(|p| match p {
            Pred::Num(v) => {
                let mut o = rows.to_vec();
                o.sort_by(|&a, &b| match (v[a].is_nan(), v[b].is_nan()) {
                    (true, true) => a.cmp(&b),
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => v[a].total_cmp(&v[b]).then(a.cmp(&b)),
                });
                o
            }
            Pred::Cat(..) => rows.to_vec(),
        })
        .collect()
}

/// Grows one tree on the working response z for the cases in `rows` (the training cases; `bag` marks those
/// sampled), and returns it with each training case's terminal node.
fn grow(d: &Data, rows: &[usize], bag: &[bool], z: &[f64], depth: usize, min_obs: usize) -> (Tree, Vec<usize>) {
    let n = z.len();
    let (mut sz, mut sw, mut nb) = (0.0, 0.0, 0usize);
    for &i in rows {
        if bag[i] {
            sz += z[i];
            sw += 1.0;
            nb += 1;
        }
    }
    let mut tree = Tree { nodes: vec![Node::Leaf { pred: sz / sw, w: sw, n: nb }] };
    // Terminal nodes in gbm's order, each with its search; `assign` holds each case's terminal index.
    let mut searches: Vec<Search> = vec![Search::new(0, (sz, sw, nb))];
    let mut assign = vec![usize::MAX; n];
    for &i in rows {
        assign[i] = 0;
    }
    for _ in 0..depth {
        // Search every terminal node, variable by variable.
        for s in searches.iter_mut() {
            *s = Search::new(s.arena, s.init);
        }
        for (var, p) in d.x.iter().enumerate() {
            let nt = searches.len();
            match p {
                Pred::Num(v) => {
                    // Current left / right / missing sums per terminal node.
                    let mut cur: Vec<[(f64, f64, usize); 3]> = searches.iter().map(|s| [(0.0, 0.0, 0), s.init, (0.0, 0.0, 0)]).collect();
                    let mut last = vec![f64::NEG_INFINITY; nt];
                    for &i in &d.order[var] {
                        if !bag[i] || assign[i] == usize::MAX {
                            continue;
                        }
                        let t = assign[i];
                        let c = &mut cur[t];
                        if v[i].is_nan() {
                            c[2].0 += z[i];
                            c[2].1 += 1.0;
                            c[2].2 += 1;
                            c[1].0 -= z[i];
                            c[1].1 -= 1.0;
                            c[1].2 -= 1;
                            continue;
                        }
                        if last[t] != v[i] && c[0].2 >= min_obs && c[1].2 >= min_obs {
                            let imp = improvement(c[0].1, c[1].1, c[2].1, c[0].0, c[1].0, c[2].0);
                            let s = &mut searches[t];
                            if imp > s.best_imp {
                                s.best_imp = imp;
                                s.best_var = var;
                                s.best_rule = Some(Rule::Num(0.5 * (last[t] + v[i])));
                                s.best_l = c[0];
                                s.best_r = c[1];
                            }
                        }
                        c[0].0 += z[i];
                        c[0].1 += 1.0;
                        c[0].2 += 1;
                        c[1].0 -= z[i];
                        c[1].1 -= 1.0;
                        c[1].2 -= 1;
                        last[t] = v[i];
                    }
                    for t in 0..nt {
                        if searches[t].best_var == var {
                            let c = cur[t];
                            searches[t].best_m = if c[2].2 > 0 { c[2] } else { (searches[t].init.0, searches[t].init.1, 0) };
                        }
                    }
                }
                Pred::Cat(v, nc) => {
                    let nc = *nc;
                    let mut gs = vec![vec![0.0; nc]; nt];
                    let mut gw = vec![vec![0.0; nc]; nt];
                    let mut gn = vec![vec![0usize; nc]; nt];
                    let mut miss = vec![(0.0, 0.0, 0usize); nt];
                    for &i in rows {
                        if !bag[i] {
                            continue;
                        }
                        let t = assign[i];
                        if v[i] == MISSING {
                            miss[t].0 += z[i];
                            miss[t].1 += 1.0;
                            miss[t].2 += 1;
                        } else {
                            gs[t][v[i]] += z[i];
                            gw[t][v[i]] += 1.0;
                            gn[t][v[i]] += 1;
                        }
                    }
                    for t in 0..nt {
                        let init = searches[t].init;
                        let m = miss[t];
                        let mut cats: Vec<usize> = (0..nc).collect();
                        let mean: Vec<f64> = (0..nc).map(|c| if gw[t][c] != 0.0 { gs[t][c] / gw[t][c] } else { f64::INFINITY }).collect();
                        let finite = mean.iter().filter(|m| m.is_finite()).count();
                        cats.sort_by(|&a, &b| mean[a].total_cmp(&mean[b]));
                        let mut l = (0.0, 0.0, 0usize);
                        let mut r = (init.0 - m.0, init.1 - m.1, init.2 - m.2);
                        let mut found = false;
                        for k in 0..finite.saturating_sub(1) {
                            let c = cats[k];
                            l = (l.0 + gs[t][c], l.1 + gw[t][c], l.2 + gn[t][c]);
                            r = (r.0 - gs[t][c], r.1 - gw[t][c], r.2 - gn[t][c]);
                            let imp = improvement(l.1, r.1, m.1, l.0, r.0, m.0);
                            if l.2 >= min_obs && r.2 >= min_obs && imp > searches[t].best_imp {
                                let mut left = vec![false; nc];
                                for &cc in &cats[..=k] {
                                    left[cc] = true;
                                }
                                let s = &mut searches[t];
                                s.best_imp = imp;
                                s.best_var = var;
                                s.best_rule = Some(Rule::Cat(left));
                                s.best_l = l;
                                s.best_r = r;
                                found = true;
                            }
                        }
                        if found || searches[t].best_var == var {
                            searches[t].best_m = if m.2 > 0 { m } else { (init.0, init.1, 0) };
                        }
                    }
                }
            }
        }
        // The best node to split.
        let (mut bt, mut bi) = (0usize, 0.0);
        for (t, s) in searches.iter().enumerate() {
            if s.best_imp > bi {
                bt = t;
                bi = s.best_imp;
            }
        }
        if bi == 0.0 {
            break;
        }
        let s = searches[bt].clone();
        let rule = s.best_rule.clone().expect("a split was found");
        let base = tree.nodes.len();
        let leaf = |c: (f64, f64, usize)| Node::Leaf { pred: if c.1 > 0.0 { c.0 / c.1 } else { 0.0 }, w: c.1, n: c.2 };
        tree.nodes.push(leaf(s.best_l));
        tree.nodes.push(leaf(s.best_r));
        tree.nodes.push(leaf(s.best_m));
        let (ppred, pw) = match &tree.nodes[s.arena] {
            Node::Leaf { pred, w, .. } => (*pred, *w),
            Node::Split { pred, w, .. } => (*pred, *w),
        };
        tree.nodes[s.arena] = Node::Split { var: s.best_var, rule: rule.clone(), improve: s.best_imp, pred: ppred, w: pw, kids: [base, base + 1, base + 2] };
        let nt = searches.len();
        for &i in rows {
            if assign[i] == bt {
                match which(d.x, s.best_var, &rule, i) {
                    1 => assign[i] = nt,
                    0 => assign[i] = nt + 1,
                    _ => {}
                }
            }
        }
        searches.push(Search::new(base + 1, s.best_r));
        searches.push(Search::new(base + 2, s.best_m));
        searches[bt] = Search::new(base, s.best_l);
    }
    let leaves: Vec<usize> = searches.iter().map(|s| s.arena).collect();
    let node_of: Vec<usize> = (0..n).map(|i| if assign[i] == usize::MAX { usize::MAX } else { leaves[assign[i]] }).collect();
    (tree, node_of)
}

/// A fitted boosting model.
pub struct Fit {
    pub init: f64,
    pub trees: Vec<Tree>,
    pub train_error: Vec<f64>,
    pub f: Vec<f64>,
    /// Validation deviance by iteration (for the held-out cases given).
    pub valid_error: Vec<f64>,
}

fn deviance(bern: bool, y: &[f64], f: &[f64], rows: &[usize]) -> f64 {
    let n = rows.len() as f64;
    if bern {
        -2.0 * rows.iter().map(|&i| y[i] * f[i] - (1.0 + f[i].exp()).ln()).sum::<f64>() / n
    } else {
        rows.iter().map(|&i| (y[i] - f[i]).powi(2)).sum::<f64>() / n
    }
}

/// Boosting on the training rows, tracking the validation rows' deviance.
#[allow(clippy::too_many_arguments)]
pub fn fit(x: &[Pred], y: &[f64], train: &[usize], valid: &[usize], bern: bool, spec: &Spec, rng: &mut SplitMix64) -> Fit {
    let n = y.len();
    let d = Data { x, order: orders(x, train) };
    let sum: f64 = train.iter().map(|&i| y[i]).sum();
    let init = if bern { (sum / (train.len() as f64 - sum)).ln() } else { sum / train.len() as f64 };
    let mut f = vec![init; n];
    let mut trees = Vec::with_capacity(spec.n_trees);
    let mut train_error = Vec::with_capacity(spec.n_trees);
    let mut valid_error = Vec::new();
    let nt = train.len();
    let in_bag = ((spec.bag_fraction * nt as f64) as usize).max(1);
    let mut bag = vec![false; n];
    for tree in 0..spec.n_trees {
        if crate::job::step(tree, spec.n_trees) {
            break;
        }
        // gbm's sequential sampling of exactly in_bag cases.
        let mut taken = 0;
        for (k, &i) in train.iter().enumerate() {
            let u = (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
            bag[i] = u * ((nt - k) as f64) < (in_bag - taken) as f64;
            if bag[i] {
                taken += 1;
            }
        }
        let z: Vec<f64> = (0..n).map(|i| if bern { y[i] - 1.0 / (1.0 + (-f[i]).exp()) } else { y[i] - f[i] }).collect();
        let (mut tree, node_of) = grow(&d, train, &bag, &z, spec.depth, spec.min_obs);
        if bern {
            // One Newton step per terminal node.
            let mut num = vec![0.0; tree.nodes.len()];
            let mut den = vec![0.0; tree.nodes.len()];
            for &i in train {
                if bag[i] {
                    let p = 1.0 / (1.0 + (-f[i]).exp());
                    num[node_of[i]] += z[i];
                    den[node_of[i]] += p * (1.0 - p);
                }
            }
            for (k, nd) in tree.nodes.iter_mut().enumerate() {
                if let Node::Leaf { pred, .. } = nd {
                    *pred = if den[k] == 0.0 { 0.0 } else { num[k] / den[k] };
                }
            }
        }
        tree.adjust(0, spec.min_obs);
        for &i in train {
            let leaf = node_of[i];
            if let Node::Leaf { pred, .. } = tree.nodes[leaf] {
                f[i] += spec.shrinkage * pred;
            }
        }
        for &i in valid {
            f[i] += spec.shrinkage * tree.predict(x, i);
        }
        train_error.push(deviance(bern, y, &f, train));
        if !valid.is_empty() {
            valid_error.push(deviance(bern, y, &f, valid));
        }
        trees.push(tree);
    }
    Fit { init, trees, train_error, f, valid_error }
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    if spec.predictors.is_empty() {
        return Err(Error::Invalid("Choose at least one predictor.".into()));
    }
    if spec.n_trees == 0 || spec.n_trees > 20_000 || spec.depth == 0 || spec.depth > 20 {
        return Err(Error::Invalid("Use 1 to 20,000 trees of depth 1 to 20.".into()));
    }
    if !(spec.shrinkage > 0.0 && spec.shrinkage <= 1.0) || !(spec.bag_fraction > 0.0 && spec.bag_fraction <= 1.0) {
        return Err(Error::Invalid("The shrinkage and the bag fraction must be above 0 and at most 1.".into()));
    }
    let yvar = ds.variable(&spec.dependent)?;
    let yi = ds.find(&spec.dependent).ok_or_else(|| Error::UnknownVariable(spec.dependent.clone()))?;
    let categorical_y = matches!(ds.columns[yi], Column::Text(_)) || yvar.measure != Level::Scale;
    let bern = match spec.distribution {
        Distribution::Bernoulli => true,
        Distribution::Gaussian => false,
        Distribution::Auto => categorical_y,
    };
    let (ys, ylevels): (Vec<Slot<f64>>, Vec<String>) = if bern {
        let (s, l) = categories(ds, &spec.dependent)?;
        (s.iter().map(|c| match c { Slot::Valid(i) => Slot::Valid(*i as f64), Slot::Empty => Slot::Empty, Slot::UserMissing => Slot::UserMissing }).collect(), l.iter().map(|lv| lv.label.clone()).collect())
    } else {
        (numeric(ds, &spec.dependent)?, Vec::new())
    };
    let mut cols: Vec<(String, Pred, Vec<String>)> = Vec::new();
    for name in &spec.predictors {
        let v = ds.variable(name)?;
        let i = ds.find(name).ok_or_else(|| Error::UnknownVariable(name.clone()))?;
        if matches!(ds.columns[i], Column::Text(_)) || v.measure == Level::Nominal {
            let (s, l) = categories(ds, name)?;
            cols.push((v.display_name().to_string(), Pred::Cat(s.iter().map(|c| c.value().unwrap_or(MISSING)).collect(), l.len()), l.iter().map(|x| x.label.clone()).collect()));
        } else {
            cols.push((v.display_name().to_string(), Pred::Num(numeric(ds, name)?.iter().map(|c| c.value().unwrap_or(f64::NAN)).collect()), Vec::new()));
        }
    }
    let folds_given: Option<Vec<Slot<f64>>> = spec.fold_variable.as_ref().map(|f| numeric(ds, f)).transpose()?;
    let total = ds.n_rows();
    let mut tally = Tally::default();
    let mut used = Vec::new();
    for r in 0..total {
        if tally.check(&ys[r], &spec.dependent) {
            continue;
        }
        if let Some(f) = &folds_given {
            if tally.check(&f[r], spec.fold_variable.as_deref().unwrap_or("fold")) {
                continue;
            }
        }
        used.push(r);
    }
    // Two-category outcome: the classes present, coded 0 and 1.
    let mut ylev = ylevels.clone();
    let y: Vec<f64> = if bern {
        let mut present: Vec<usize> = used.iter().map(|&r| ys[r].value().expect("checked") as usize).collect();
        present.sort_unstable();
        present.dedup();
        if present.len() != 2 {
            return Err(Error::Invalid("Bernoulli boosting needs an outcome with exactly two categories.".into()));
        }
        ylev = present.iter().map(|&k| ylevels[k].clone()).collect();
        used.iter().map(|&r| if ys[r].value().expect("checked") as usize == present[1] { 1.0 } else { 0.0 }).collect()
    } else {
        used.iter().map(|&r| ys[r].value().expect("checked")).collect()
    };
    let x: Vec<Pred> = cols.iter().map(|(_, p, _)| match p {
        Pred::Num(v) => Pred::Num(used.iter().map(|&r| v[r]).collect()),
        Pred::Cat(v, nc) => Pred::Cat(used.iter().map(|&r| v[r]).collect(), *nc),
    }).collect();
    let n = y.len();
    let names: Vec<String> = cols.iter().map(|c| c.0.clone()).collect();
    let note = format!(
        "Gradient boosting as R's gbm: {} loss, {} trees with {} split{} each, shrinkage {}, bag fraction {}, at least {} cases per node{}. Computed by PrismStatistics.",
        if bern { "Bernoulli (logistic)" } else { "squared-error (Gaussian)" },
        spec.n_trees,
        spec.depth,
        if spec.depth == 1 { "" } else { "s" },
        apa::num(spec.shrinkage, 3),
        apa::num(spec.bag_fraction, 2),
        spec.min_obs,
        if spec.cv_folds > 1 || spec.fold_variable.is_some() { "; the number of trees chosen by cross-validation" } else { "" }
    );
    let mut b = common::bundle(BOOST, METHOD_VERSION, format!("Gradient Boosting: {}", yvar.display_name()), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, n), note);
    set_text(&mut b, "y.label", yvar.display_name().to_string());
    set_text(&mut b, "distribution", if bern { "bernoulli" } else { "gaussian" });
    set(&mut b, "n", n as f64);
    if n < 2 * spec.min_obs + 1 {
        fail(&mut b, "There are too few cases for the minimum node size.");
        return Ok(b);
    }
    let all: Vec<usize> = (0..n).collect();
    let mut rng = SplitMix64::new(spec.seed);
    let model = fit(&x, &y, &all, &[], bern, spec, &mut rng);
    set(&mut b, "init", model.init);
    for (t, e) in model.train_error.iter().enumerate() {
        set(&mut b, format!("train_error.{}", t + 1), *e);
    }
    for i in 0..n {
        set(&mut b, format!("fit.{}", i + 1), model.f[i]);
    }
    // Cross-validation.
    let fold_of: Option<Vec<usize>> = if let Some(f) = &folds_given {
        let raw: Vec<f64> = used.iter().map(|&r| f[r].value().expect("checked")).collect();
        let mut u = raw.clone();
        u.sort_by(f64::total_cmp);
        u.dedup();
        Some(raw.iter().map(|v| u.iter().position(|x| x == v).expect("listed")).collect())
    } else if spec.cv_folds > 1 {
        let mut idx: Vec<usize> = (0..n).collect();
        let mut frng = SplitMix64::new(spec.seed ^ 0xC0FF_EE00);
        for i in (1..n).rev() {
            let j = frng.below(i + 1);
            idx.swap(i, j);
        }
        let mut f = vec![0; n];
        for (pos, &i) in idx.iter().enumerate() {
            f[i] = pos % spec.cv_folds;
        }
        Some(f)
    } else {
        None
    };
    let mut best = spec.n_trees;
    let mut cv_curve: Vec<f64> = Vec::new();
    if let Some(fo) = &fold_of {
        let k = fo.iter().max().map_or(0, |m| m + 1);
        let mut acc = vec![0.0; spec.n_trees];
        for fold in 0..k {
            let train: Vec<usize> = (0..n).filter(|&i| fo[i] != fold).collect();
            let valid: Vec<usize> = (0..n).filter(|&i| fo[i] == fold).collect();
            if train.is_empty() || valid.is_empty() {
                continue;
            }
            let mut frng = SplitMix64::new(spec.seed.wrapping_add(fold as u64 + 1));
            let m = fit(&x, &y, &train, &valid, bern, spec, &mut frng);
            for t in 0..spec.n_trees {
                acc[t] += m.valid_error[t] * valid.len() as f64;
            }
        }
        cv_curve = acc.iter().map(|a| a / n as f64).collect();
        best = cv_curve.iter().enumerate().fold((0, f64::INFINITY), |(bi, bv), (i, v)| if *v < bv { (i, *v) } else { (bi, bv) }).0 + 1;
        for (t, e) in cv_curve.iter().enumerate() {
            set(&mut b, format!("cv_error.{}", t + 1), *e);
        }
    }
    set(&mut b, "best_trees", best as f64);
    // Predictions at the chosen number of trees.
    let f_best: Vec<f64> = (0..n).map(|i| model.init + spec.shrinkage * model.trees[..best].iter().map(|t| t.predict(&x, i)).sum::<f64>()).collect();
    // Relative influence over the chosen trees.
    let mut infl = vec![0.0; names.len()];
    for t in &model.trees[..best] {
        for (v, imp) in t.improvements() {
            infl[v] += imp;
        }
    }
    let tot: f64 = infl.iter().sum();
    let mut order: Vec<usize> = (0..names.len()).collect();
    order.sort_by(|&a, &c| infl[c].total_cmp(&infl[a]));
    let mut irows = Vec::new();
    for &j in &order {
        let pct = if tot > 0.0 { 100.0 * infl[j] / tot } else { 0.0 };
        set(&mut b, format!("influence.{}", spec.predictors[j]), pct);
        irows.push(vec![text(names[j].clone()), num(pct, 1)]);
    }
    b.tables.push(Table { id: "influence".into(), title: "Relative influence".into(), columns: vec!["Predictor".into(), "Relative influence (%)".into()], rows: irows, footnotes: vec![format!("Each predictor's share of the improvement from all splits on it, over the first {best} trees (gbm's relative influence).")] });
    let bars: Vec<(String, f64)> = order.iter().map(|&j| (names[j].clone(), if tot > 0.0 { 100.0 * infl[j] / tot } else { 0.0 })).collect();
    let alt = format!("Relative influence of {} predictors; the largest is {}.", names.len(), names[order[0]]);
    b.charts.push(chart::Chart { id: "influence".into(), title: "Relative influence".into(), svg: chart::bars(&bars, None, "Predictor", "Relative influence (%)", &alt), alt });
    // Performance.
    let mut perf = Vec::new();
    if bern {
        let correct = (0..n).filter(|&i| (f_best[i] > 0.0) == (y[i] > 0.5)).count() as f64;
        set(&mut b, "train.accuracy", correct / n as f64);
        perf.push(vec![text("Accuracy (training, at the chosen number of trees)"), num(correct / n as f64, 3)]);
        perf.push(vec![text("Bernoulli deviance (training)"), num(deviance(true, &y, &f_best, &all), 4)]);
        if !cv_curve.is_empty() {
            set(&mut b, "cv.deviance", cv_curve[best - 1]);
            perf.push(vec![text("Bernoulli deviance (cross-validated)"), num(cv_curve[best - 1], 4)]);
        }
        set_text(&mut b, "event", ylev[1].clone());
    } else {
        let ybar = y.iter().sum::<f64>() / n as f64;
        let sst: f64 = y.iter().map(|v| (v - ybar).powi(2)).sum::<f64>() / n as f64;
        let mse = deviance(false, &y, &f_best, &all);
        set(&mut b, "train.r2", 1.0 - mse / sst);
        perf.push(vec![text("R² (training)"), num(1.0 - mse / sst, 3)]);
        if !cv_curve.is_empty() {
            set(&mut b, "cv.r2", 1.0 - cv_curve[best - 1] / sst);
            perf.push(vec![text("R² (cross-validated)"), num(1.0 - cv_curve[best - 1] / sst, 3)]);
            perf.push(vec![text("RMSE (cross-validated)"), num(cv_curve[best - 1].sqrt(), 4)]);
        }
    }
    b.tables.insert(0, Table { id: "performance".into(), title: "Model".into(), columns: vec!["".into(), "Value".into()], rows: { let mut r = vec![vec![text("Trees used"), count(best)]]; r.extend(perf); r }, footnotes: if cv_curve.is_empty() { vec!["No cross-validation: all trees are used; training performance overstates performance on new data.".into()] } else { vec!["The number of trees with the smallest cross-validated deviance.".into()] } });
    // Error curve.
    let curves: Vec<(String, Vec<(f64, f64)>)> = {
        let mut c = vec![("Training".to_string(), model.train_error.iter().enumerate().map(|(t, e)| ((t + 1) as f64, *e)).collect())];
        if !cv_curve.is_empty() {
            c.push(("Cross-validated".to_string(), cv_curve.iter().enumerate().map(|(t, e)| ((t + 1) as f64, *e)).collect()));
        }
        c
    };
    let alt = format!("Deviance by number of trees{}.", if cv_curve.is_empty() { String::new() } else { format!("; the cross-validated deviance is smallest at {best} trees") });
    b.charts.push(chart::Chart { id: "curve".into(), title: "Error by number of trees".into(), svg: chart::scatter_curves(&[], &curves, "Trees", "Deviance", &alt), alt });
    if !cv_curve.is_empty() && best == spec.n_trees {
        b.diagnostics.push(Diagnostic { id: "trees".into(), title: "Number of trees".into(), state: DiagnosticState::ReviewNeeded, message: "The cross-validated error was still falling at the last tree: use more trees (or a larger shrinkage).".into() });
    }
    b.diagnostics.push(common::size_diagnostic(n, format!("{n} cases")));
    Ok(b)
}
