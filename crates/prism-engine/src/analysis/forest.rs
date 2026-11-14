//! Random forests (checklist E-8, part 1) as R's randomForest (4.7) grows them (Breiman and Cutler's algorithm).
//!
//! Each tree is grown on a bootstrap sample (or a subsample without replacement) to nodes of at most `nodesize`
//! cases, trying `mtry` randomly chosen predictors at each node. Regression trees take the split that most reduces
//! the sum of squares (cases with x ≤ the midpoint go left; a categorical predictor's categories are ordered by their
//! mean in the node); classification trees maximize the Gini criterion Σ nL,k²/nL + Σ nR,k²/nR on the bootstrap
//! counts (categorical predictors: every split of up to 10 categories, else categories ordered by the share of the
//! first class), and a node is final when it is pure. Ties between equally good splits, and between classes in a
//! leaf, are broken at random, as randomForest does. Out-of-bag predictions give the error (MSE and % of variance
//! explained, or the error rate and confusion matrix); importance is the permutation increase in out-of-bag error
//! (mean over trees divided by its standard error, randomForest's scaled importance) and the total decrease in node
//! impurity per tree.
//!
//! With mtry = the number of predictors and every case used once (`replace` off, the full sample), a regression tree
//! is deterministic and equals randomForest's.

#![allow(clippy::needless_range_loop)]

use prismqda_stats::rng::SplitMix64;
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::chart;
use crate::error::{Error, Result};
use crate::model::{Column, Dataset, Measure as Level};
use crate::result::{Diagnostic, DiagnosticState, ResultBundle, Table};

pub const FOREST: &str = "predict.forest";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Auto,
    Regression,
    Classification,
}

fn trees() -> usize {
    500
}
fn yes() -> bool {
    true
}
fn seed() -> u64 {
    20_260_101
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub dependent: String,
    pub predictors: Vec<String>,
    #[serde(default)]
    pub kind: Kind,
    #[serde(default = "trees")]
    pub n_trees: usize,
    /// Predictors tried at each split (randomForest's default when left out: p/3 or √p).
    #[serde(default)]
    pub mtry: Option<usize>,
    /// The largest node left unsplit (default 5 for regression, 1 for classification).
    #[serde(default)]
    pub nodesize: Option<usize>,
    #[serde(default = "yes")]
    pub replace: bool,
    /// Cases drawn for each tree (all of them when left out).
    #[serde(default)]
    pub sample_size: Option<usize>,
    #[serde(default = "yes")]
    pub importance: bool,
    #[serde(default = "seed")]
    pub seed: u64,
}

const MISSING: usize = usize::MAX;

#[derive(Clone)]
enum Pred {
    Num(Vec<f64>),
    Cat(Vec<usize>, usize),
}

#[derive(Clone, Debug)]
enum Rule {
    /// x ≤ cut goes left.
    Num(f64),
    Cat(Vec<bool>),
}

#[derive(Clone, Debug)]
struct Node {
    split: Option<(usize, Rule, usize, usize)>,
    /// Mean, or class.
    value: f64,
}

#[derive(Clone, Debug)]
struct Tree {
    nodes: Vec<Node>,
}

fn goes_left(x: &[Pred], var: usize, rule: &Rule, i: usize) -> bool {
    match (&x[var], rule) {
        (Pred::Num(v), Rule::Num(c)) => v[i] <= *c,
        (Pred::Cat(v, _), Rule::Cat(l)) => l.get(v[i]).copied().unwrap_or(false),
        _ => false,
    }
}

impl Tree {
    fn predict(&self, x: &[Pred], i: usize) -> f64 {
        let mut k = 0;
        while let Some((var, rule, l, r)) = &self.nodes[k].split {
            k = if goes_left(x, *var, rule, i) { *l } else { *r };
        }
        self.nodes[k].value
    }
}

fn uniform(rng: &mut SplitMix64) -> f64 {
    (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64
}

struct Grower<'a> {
    x: &'a [Pred],
    y: &'a [f64],
    classes: usize,
    mtry: usize,
    nodesize: usize,
}

impl Grower<'_> {
    /// Grows a tree on `cases` (with repeats for the bootstrap); returns it and each variable's impurity decrease.
    fn grow(&self, cases: &[usize], rng: &mut SplitMix64) -> (Tree, Vec<f64>) {
        let p = self.x.len();
        let mut dec = vec![0.0; p];
        let mut nodes: Vec<Node> = Vec::new();
        // Queue of (node index, its cases), processed in creation order as randomForest's.
        let mut members: Vec<Vec<usize>> = Vec::new();
        nodes.push(Node { split: None, value: 0.0 });
        members.push(cases.to_vec());
        let mut k = 0;
        while k < nodes.len() {
            let rows = std::mem::take(&mut members[k]);
            nodes[k].value = self.value(&rows, rng);
            let terminal = (k > 0 && rows.len() <= self.nodesize) || (self.classes > 0 && self.pure(&rows)) || rows.len() < 2;
            if !terminal {
                if let Some((var, rule, gain)) = self.best_split(&rows, rng) {
                    let (l, r): (Vec<usize>, Vec<usize>) = rows.iter().partition(|&&i| goes_left(self.x, var, &rule, i));
                    if !l.is_empty() && !r.is_empty() {
                        dec[var] += gain;
                        let li = nodes.len();
                        nodes.push(Node { split: None, value: 0.0 });
                        nodes.push(Node { split: None, value: 0.0 });
                        nodes[k].split = Some((var, rule, li, li + 1));
                        members.push(l);
                        members.push(r);
                    }
                }
            }
            k += 1;
        }
        (Tree { nodes }, dec)
    }

    fn pure(&self, rows: &[usize]) -> bool {
        rows.windows(2).all(|w| self.y[w[0]] == self.y[w[1]])
    }

    /// The node's prediction: the mean, or the most frequent class (ties at random).
    fn value(&self, rows: &[usize], rng: &mut SplitMix64) -> f64 {
        if rows.is_empty() {
            return 0.0;
        }
        if self.classes == 0 {
            return rows.iter().map(|&i| self.y[i]).sum::<f64>() / rows.len() as f64;
        }
        let mut c = vec![0.0; self.classes];
        for &i in rows {
            c[self.y[i] as usize] += 1.0;
        }
        let (mut best, mut pp, mut ntie) = (0usize, 0.0, 1);
        for j in 0..self.classes {
            if c[j] > pp {
                best = j;
                pp = c[j];
                ntie = 1;
            }
            if c[j] == pp && c[j] > 0.0 {
                ntie += 1;
                if uniform(rng) < 1.0 / ntie as f64 {
                    best = j;
                }
            }
        }
        best as f64
    }

    /// The best split over mtry random predictors: (variable, rule, impurity decrease).
    fn best_split(&self, rows: &[usize], rng: &mut SplitMix64) -> Option<(usize, Rule, f64)> {
        let p = self.x.len();
        let mut idx: Vec<usize> = (0..p).collect();
        let mut last = p;
        let n = rows.len() as f64;
        let (mut best, mut best_crit, mut tie_var): (Option<(usize, Rule)>, f64, usize) = (None, 0.0, 1);
        let parent = if self.classes == 0 {
            let s: f64 = rows.iter().map(|&i| self.y[i]).sum();
            s * s / n
        } else {
            let mut c = vec![0.0; self.classes];
            for &i in rows {
                c[self.y[i] as usize] += 1.0;
            }
            c.iter().map(|v| v * v).sum::<f64>() / n
        };
        for _ in 0..self.mtry.min(p) {
            let j = ((uniform(rng) * last as f64) as usize).min(last - 1);
            let var = idx[j];
            idx.swap(j, last - 1);
            last -= 1;
            // Each case's value on the split scale (numeric x, or its category's node mean / class share).
            let (key, catval): (Vec<f64>, Option<Vec<f64>>) = match &self.x[var] {
                Pred::Num(v) => (rows.iter().map(|&i| v[i]).collect(), None),
                Pred::Cat(v, nc) => {
                    let mut s = vec![0.0; *nc];
                    let mut c = vec![0.0; *nc];
                    for &i in rows {
                        c[v[i]] += 1.0;
                        s[v[i]] += if self.classes == 0 { self.y[i] } else if self.y[i] == 0.0 { 1.0 } else { 0.0 };
                    }
                    if self.classes > 2 && *nc <= 10 {
                        // Exhaustive search over category subsets for several classes.
                        if let Some((rule, crit)) = self.exhaustive(rows, var, *nc) {
                            let gain = crit;
                            if gain > best_crit {
                                best = Some((var, rule));
                                best_crit = gain;
                                tie_var = 1;
                            } else if gain == best_crit && best.is_some() {
                                tie_var += 1;
                                if uniform(rng) < 1.0 / tie_var as f64 {
                                    best = Some((var, rule));
                                }
                            }
                        }
                        continue;
                    }
                    let avg: Vec<f64> = (0..*nc).map(|k| if c[k] > 0.0 { s[k] / c[k] } else { 0.0 }).collect();
                    (rows.iter().map(|&i| avg[v[i]]).collect(), Some(avg))
                }
            };
            let mut order: Vec<usize> = (0..rows.len()).collect();
            order.sort_by(|&a, &b| key[a].total_cmp(&key[b]));
            if key[order[0]] >= key[order[rows.len() - 1]] {
                continue;
            }
            let (mut best_v, mut val, mut tie_val) = (0.0f64, 0.0, 1);
            if self.classes == 0 {
                let total: f64 = rows.iter().map(|&i| self.y[i]).sum();
                let (mut sl, mut sr) = (0.0, total);
                for q in 0..rows.len() - 1 {
                    let yv = self.y[rows[order[q]]];
                    sl += yv;
                    sr -= yv;
                    let (nl, nr) = ((q + 1) as f64, (rows.len() - q - 1) as f64);
                    if key[order[q]] < key[order[q + 1]] {
                        let crit = sl * sl / nl + sr * sr / nr - parent;
                        if crit > best_v {
                            val = (key[order[q]] + key[order[q + 1]]) / 2.0;
                            best_v = crit;
                            tie_val = 1;
                        }
                        if crit == best_v {
                            tie_val += 1;
                            if uniform(rng) < 1.0 / tie_val as f64 {
                                val = (key[order[q]] + key[order[q + 1]]) / 2.0;
                            }
                        }
                    }
                }
            } else {
                let mut wl = vec![0.0; self.classes];
                let mut wr = vec![0.0; self.classes];
                for &i in rows {
                    wr[self.y[i] as usize] += 1.0;
                }
                let mut rrn: f64 = wr.iter().map(|v| v * v).sum();
                let mut rln = 0.0;
                let (mut rld, mut rrd) = (0.0, n);
                best_v = -1e25;
                for q in 0..rows.len() - 1 {
                    let k = self.y[rows[order[q]]] as usize;
                    rln += 2.0 * wl[k] + 1.0;
                    rrn += -2.0 * wr[k] + 1.0;
                    rld += 1.0;
                    rrd -= 1.0;
                    wl[k] += 1.0;
                    wr[k] -= 1.0;
                    if key[order[q]] < key[order[q + 1]] && rrd.min(rld) > 1e-5 {
                        let crit = rln / rld + rrn / rrd;
                        if crit > best_v {
                            val = (key[order[q]] + key[order[q + 1]]) / 2.0;
                            best_v = crit;
                            tie_val = 1;
                        }
                        if crit == best_v {
                            tie_val += 1;
                            if uniform(rng) < 1.0 / tie_val as f64 {
                                val = (key[order[q]] + key[order[q + 1]]) / 2.0;
                            }
                        }
                    }
                }
                best_v -= parent;
            }
            if best_v <= 0.0 {
                continue;
            }
            let rule = match catval {
                None => Rule::Num(val),
                Some(avg) => Rule::Cat(avg.iter().map(|a| *a <= val).collect()),
            };
            if best_v > best_crit {
                best = Some((var, rule));
                best_crit = best_v;
                tie_var = 1;
            } else if best_v == best_crit {
                tie_var += 1;
                if uniform(rng) < 1.0 / tie_var as f64 {
                    best = Some((var, rule));
                }
            }
        }
        best.map(|(v, r)| (v, r, best_crit))
    }

    /// Every split of a categorical predictor's categories (classification): the best rule and its Gini gain.
    fn exhaustive(&self, rows: &[usize], var: usize, nc: usize) -> Option<(Rule, f64)> {
        let Pred::Cat(v, _) = &self.x[var] else { return None };
        let k = self.classes;
        let mut tab = vec![vec![0.0; k]; nc];
        for &i in rows {
            tab[v[i]][self.y[i] as usize] += 1.0;
        }
        let present: Vec<usize> = (0..nc).filter(|&c| tab[c].iter().sum::<f64>() > 0.0).collect();
        if present.len() < 2 {
            return None;
        }
        let tot: Vec<f64> = (0..k).map(|j| present.iter().map(|&c| tab[c][j]).sum()).collect();
        let n: f64 = tot.iter().sum();
        let parent: f64 = tot.iter().map(|v| v * v).sum::<f64>() / n;
        let m = present.len();
        let mut best: Option<(u32, f64)> = None;
        // Subsets with the last present category on the right.
        for mask in 1u32..(1 << (m - 1)) {
            let mut l = vec![0.0; k];
            for (b, &c) in present.iter().enumerate() {
                if mask & (1 << b) != 0 {
                    for j in 0..k {
                        l[j] += tab[c][j];
                    }
                }
            }
            let ln: f64 = l.iter().sum();
            let rn = n - ln;
            if ln < 1e-5 || rn < 1e-5 {
                continue;
            }
            let crit = l.iter().map(|v| v * v).sum::<f64>() / ln + (0..k).map(|j| (tot[j] - l[j]).powi(2)).sum::<f64>() / rn - parent;
            if best.is_none_or(|(_, b)| crit > b) {
                best = Some((mask, crit));
            }
        }
        let (mask, crit) = best?;
        if crit <= 0.0 {
            return None;
        }
        let mut left = vec![false; nc];
        for (b, &c) in present.iter().enumerate() {
            if mask & (1 << b) != 0 {
                left[c] = true;
            }
        }
        Some((Rule::Cat(left), crit))
    }
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    if spec.predictors.is_empty() {
        return Err(Error::Invalid("Choose at least one predictor.".into()));
    }
    if spec.n_trees == 0 || spec.n_trees > 10_000 {
        return Err(Error::Invalid("Use 1 to 10,000 trees.".into()));
    }
    let yvar = ds.variable(&spec.dependent)?;
    let yi = ds.find(&spec.dependent).ok_or_else(|| Error::UnknownVariable(spec.dependent.clone()))?;
    let categorical_y = match spec.kind {
        Kind::Classification => true,
        Kind::Regression => false,
        Kind::Auto => matches!(ds.columns[yi], Column::Text(_)) || yvar.measure != Level::Scale,
    };
    let (ys, ylevels): (Vec<Slot<f64>>, Vec<String>) = if categorical_y {
        let (s, l) = categories(ds, &spec.dependent)?;
        (s.iter().map(|c| match c { Slot::Valid(i) => Slot::Valid(*i as f64), Slot::Empty => Slot::Empty, Slot::UserMissing => Slot::UserMissing }).collect(), l.iter().map(|lv| lv.label.clone()).collect())
    } else {
        (numeric(ds, &spec.dependent)?, Vec::new())
    };
    let mut cols: Vec<(String, Pred)> = Vec::new();
    for name in &spec.predictors {
        let v = ds.variable(name)?;
        let i = ds.find(name).ok_or_else(|| Error::UnknownVariable(name.clone()))?;
        if matches!(ds.columns[i], Column::Text(_)) || v.measure == Level::Nominal {
            let (s, l) = categories(ds, name)?;
            if l.len() > 32 {
                return Err(Error::Invalid(format!("{name} has more than 32 categories.")));
            }
            cols.push((v.display_name().to_string(), Pred::Cat(s.iter().map(|c| c.value().unwrap_or(MISSING)).collect(), l.len())));
        } else {
            cols.push((v.display_name().to_string(), Pred::Num(numeric(ds, name)?.iter().map(|c| c.value().unwrap_or(f64::NAN)).collect())));
        }
    }
    let total = ds.n_rows();
    let mut tally = Tally::default();
    let mut used = Vec::new();
    'r: for r in 0..total {
        if tally.check(&ys[r], &spec.dependent) {
            continue;
        }
        for (k, (_, p)) in cols.iter().enumerate() {
            let miss = match p {
                Pred::Num(v) => v[r].is_nan(),
                Pred::Cat(v, _) => v[r] == MISSING,
            };
            if miss {
                tally.add(format!("missing {}", spec.predictors[k]));
                continue 'r;
            }
        }
        used.push(r);
    }
    let n = used.len();
    let (y, ylev): (Vec<f64>, Vec<String>) = if categorical_y {
        let mut present: Vec<usize> = used.iter().map(|&r| ys[r].value().expect("checked") as usize).collect();
        present.sort_unstable();
        present.dedup();
        (used.iter().map(|&r| present.binary_search(&(ys[r].value().expect("checked") as usize)).expect("present") as f64).collect(), present.iter().map(|&k| ylevels[k].clone()).collect())
    } else {
        (used.iter().map(|&r| ys[r].value().expect("checked")).collect(), Vec::new())
    };
    let x: Vec<Pred> = cols.iter().map(|(_, p)| match p {
        Pred::Num(v) => Pred::Num(used.iter().map(|&r| v[r]).collect()),
        Pred::Cat(v, nc) => Pred::Cat(used.iter().map(|&r| v[r]).collect(), *nc),
    }).collect();
    let names: Vec<String> = cols.iter().map(|c| c.0.clone()).collect();
    let p = names.len();
    let classes = if categorical_y { ylev.len() } else { 0 };
    let mtry = spec.mtry.unwrap_or(if categorical_y { ((p as f64).sqrt().floor() as usize).max(1) } else { (p / 3).max(1) }).clamp(1, p);
    let nodesize = spec.nodesize.unwrap_or(if categorical_y { 1 } else { 5 }).max(1);
    let ss = spec.sample_size.unwrap_or(if spec.replace { n } else { ((0.632 * n as f64).ceil()) as usize }).clamp(1, n);
    let kind = if categorical_y { "Classification" } else { "Regression" };
    let note = format!(
        "{kind} random forest as R's randomForest: {} trees, mtry = {mtry} of {p} predictors, nodes of at most {nodesize} case{} left unsplit, {} of {ss} cases per tree; out-of-bag error and permutation importance. Random draws differ from R's, so results agree with randomForest statistically, not digit for digit. Computed by PrismStatistics.",
        spec.n_trees,
        if nodesize == 1 { "" } else { "s" },
        if spec.replace { "bootstrap samples" } else { "subsamples without replacement" },
    );
    let mut b = common::bundle(FOREST, METHOD_VERSION, format!("Random Forest: {}", yvar.display_name()), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, n), note);
    set_text(&mut b, "y.label", yvar.display_name().to_string());
    set_text(&mut b, "kind", kind.to_lowercase());
    set(&mut b, "n", n as f64);
    set(&mut b, "mtry", mtry as f64);
    if n < 5 || (categorical_y && classes < 2) {
        fail(&mut b, "There are too few cases (or only one class) to grow a forest.");
        return Ok(b);
    }
    let g = Grower { x: &x, y: &y, classes, mtry, nodesize };
    let mut rng = SplitMix64::new(spec.seed);
    let mut oob_sum = vec![0.0; n];
    let mut oob_votes = vec![vec![0usize; classes.max(1)]; n];
    let mut oob_count = vec![0usize; n];
    let mut fitted_sum = vec![0.0; n];
    let mut dec = vec![0.0; p];
    let mut perm: Vec<Vec<f64>> = vec![Vec::with_capacity(spec.n_trees); p];
    for tree in 0..spec.n_trees {
        if crate::job::step(tree, spec.n_trees) {
            break;
        }
        // Sample.
        let cases: Vec<usize> = if spec.replace {
            (0..ss).map(|_| rng.below(n)).collect()
        } else {
            let mut idx: Vec<usize> = (0..n).collect();
            for i in 0..ss {
                let j = i + rng.below(n - i);
                idx.swap(i, j);
            }
            idx.truncate(ss);
            idx
        };
        let mut inbag = vec![false; n];
        for &c in &cases {
            inbag[c] = true;
        }
        let (tree, d) = g.grow(&cases, &mut rng);
        for v in 0..p {
            dec[v] += d[v];
        }
        let oob: Vec<usize> = (0..n).filter(|&i| !inbag[i]).collect();
        let pred: Vec<f64> = (0..n).map(|i| tree.predict(&x, i)).collect();
        for i in 0..n {
            fitted_sum[i] += pred[i];
        }
        for &i in &oob {
            oob_count[i] += 1;
            if classes == 0 {
                oob_sum[i] += pred[i];
            } else {
                oob_votes[i][pred[i] as usize] += 1;
            }
        }
        // Permutation importance on this tree's out-of-bag cases.
        if spec.importance && !oob.is_empty() {
            let err = |pr: &dyn Fn(usize) -> f64| -> f64 {
                oob.iter().map(|&i| { let v = pr(i); if classes == 0 { (y[i] - v).powi(2) } else if (v - y[i]).abs() < 0.5 { 0.0 } else { 1.0 } }).sum::<f64>() / oob.len() as f64
            };
            let base = err(&|i| pred[i]);
            for v in 0..p {
                let mut shuffled = oob.clone();
                for i in (1..shuffled.len()).rev() {
                    let j = rng.below(i + 1);
                    shuffled.swap(i, j);
                }
                let mut xp = x.clone();
                match (&mut xp[v], &x[v]) {
                    (Pred::Num(t), Pred::Num(s)) => {
                        for (a, b2) in oob.iter().zip(&shuffled) {
                            t[*a] = s[*b2];
                        }
                    }
                    (Pred::Cat(t, _), Pred::Cat(s, _)) => {
                        for (a, b2) in oob.iter().zip(&shuffled) {
                            t[*a] = s[*b2];
                        }
                    }
                    _ => {}
                }
                let e = err(&|i| tree.predict(&xp, i));
                perm[v].push(e - base);
            }
        }
    }
    let nt = spec.n_trees as f64;
    for i in 0..n {
        set(&mut b, format!("fitted.{}", i + 1), fitted_sum[i] / nt);
    }
    // Out-of-bag performance.
    let with_oob: Vec<usize> = (0..n).filter(|&i| oob_count[i] > 0).collect();
    let mut perf = vec![vec![text("Trees"), count(spec.n_trees)], vec![text("Predictors tried at each split (mtry)"), count(mtry)]];
    if classes == 0 {
        let mse = with_oob.iter().map(|&i| (y[i] - oob_sum[i] / oob_count[i] as f64).powi(2)).sum::<f64>() / with_oob.len().max(1) as f64;
        let ybar = y.iter().sum::<f64>() / n as f64;
        let var = y.iter().map(|v| (v - ybar).powi(2)).sum::<f64>() / n as f64;
        set(&mut b, "oob.mse", mse);
        set(&mut b, "oob.rsq", 1.0 - mse / var);
        perf.push(vec![text("Out-of-bag mean squared error"), num(mse, 4)]);
        perf.push(vec![text("Variance explained (out of bag, %)"), num(100.0 * (1.0 - mse / var), 2)]);
    } else {
        let mut conf = vec![vec![0usize; classes]; classes];
        let mut wrong = 0;
        for &i in &with_oob {
            let votes = &oob_votes[i];
            let mx = *votes.iter().max().unwrap_or(&0);
            let pick = (0..classes).find(|&c| votes[c] == mx).unwrap_or(0);
            conf[y[i] as usize][pick] += 1;
            if pick != y[i] as usize {
                wrong += 1;
            }
        }
        let rate = wrong as f64 / with_oob.len().max(1) as f64;
        set(&mut b, "oob.error", rate);
        perf.push(vec![text("Out-of-bag error rate (%)"), num(100.0 * rate, 2)]);
        let mut crow = Vec::new();
        for a in 0..classes {
            let tot: usize = conf[a].iter().sum();
            let mut row = vec![text(ylev[a].clone())];
            row.extend(conf[a].iter().map(|c| count(*c)));
            row.push(num(if tot > 0 { 100.0 * (tot - conf[a][a]) as f64 / tot as f64 } else { 0.0 }, 1));
            crow.push(row);
        }
        let mut ccols = vec!["Observed".to_string()];
        ccols.extend(ylev.iter().map(|l| format!("Predicted {l}")));
        ccols.push("Class error (%)".into());
        b.tables.push(Table { id: "confusion".into(), title: "Out-of-bag confusion matrix".into(), columns: ccols, rows: crow, footnotes: vec!["Each case is classified by the trees that did not see it.".into()] });
    }
    b.tables.insert(0, Table { id: "performance".into(), title: "Forest".into(), columns: vec!["".into(), "Value".into()], rows: perf, footnotes: vec!["Out-of-bag estimates use, for each case, only the trees grown without it: an honest estimate of performance on new data.".into()] });
    // Importance.
    let mut rows = Vec::new();
    let mut order: Vec<usize> = (0..p).collect();
    let scaled: Vec<f64> = (0..p)
        .map(|v| {
            let m = perm[v].len() as f64;
            if m < 2.0 {
                return f64::NAN;
            }
            let mean = perm[v].iter().sum::<f64>() / m;
            let sd = (perm[v].iter().map(|e| (e - mean).powi(2)).sum::<f64>() / (m - 1.0)).sqrt();
            if sd > 0.0 { mean / (sd / m.sqrt()) } else { 0.0 }
        })
        .collect();
    order.sort_by(|&a, &c| (dec[c]).total_cmp(&dec[a]));
    for &v in &order {
        let key = &spec.predictors[v];
        set(&mut b, format!("purity.{key}"), dec[v] / nt);
        if scaled[v].is_finite() {
            set(&mut b, format!("permutation.{key}"), scaled[v]);
        }
        rows.push(vec![text(names[v].clone()), if scaled[v].is_finite() { num(scaled[v], 2) } else { text("—") }, num(dec[v] / nt, 3)]);
    }
    b.tables.push(Table {
        id: "importance".into(),
        title: "Variable importance".into(),
        columns: vec!["Predictor".into(), if classes == 0 { "%IncMSE (scaled)".into() } else { "Mean decrease in accuracy (scaled)".into() }, if classes == 0 { "Increase in node purity".into() } else { "Mean decrease in Gini".into() }],
        rows,
        footnotes: vec!["Permutation importance: the increase in out-of-bag error when a predictor's values are shuffled, averaged over trees and divided by its standard error. Node purity: the decrease in the sum of squares (or the Gini criterion) from its splits, per tree.".into()],
    });
    let bars: Vec<(String, f64)> = order.iter().map(|&v| (names[v].clone(), dec[v] / nt)).collect();
    let alt = format!("Importance (decrease in node impurity) of {p} predictors; the largest is {}.", names[order[0]]);
    b.charts.push(chart::Chart { id: "importance".into(), title: "Variable importance".into(), svg: chart::bars(&bars, None, "Predictor", if classes == 0 { "Increase in node purity" } else { "Mean decrease in Gini" }, &alt), alt });
    if with_oob.len() < n {
        b.diagnostics.push(Diagnostic { id: "oob".into(), title: "Out-of-bag cases".into(), state: DiagnosticState::ReviewNeeded, message: format!("{} cases were never out of bag; use more trees.", n - with_oob.len()) });
    }
    b.diagnostics.push(common::size_diagnostic(n, format!("{n} cases")));
    Ok(b)
}
