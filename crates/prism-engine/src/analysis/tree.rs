//! Decision trees (checklist D-3, part 2) as R's rpart grows, prunes and cross-validates them: regression trees
//! (method "anova": splits that most reduce the sum of squares) and classification trees (method "class": Gini splits
//! with priors equal to the observed class proportions and 0-1 loss, so a node's risk is its misclassified cases).
//!
//! Growth follows rpart's partition(): a node is split when it has at least `minsplit` cases, each child at least
//! `minbucket`, the depth limit allows it and its complexity can exceed α = cp × (root risk); the complexity of every
//! subtree is computed as rpart computes it, and subtrees that cannot pay for themselves at α are removed. Numeric
//! predictors split at midpoints between adjacent distinct values; categorical predictors are ordered by their mean
//! (or, for two classes, the share of the first class) and split in that order, or searched exhaustively for more than
//! two classes. Ties go to the first split found, as rpart's. The complexity table, its cross-validation (each fold
//! regrown with α rescaled to the fold, held-out cases run down it at the geometric means of adjacent complexity
//! values) and the pruning follow rpart; the left child is the one with the lower mean (or mean class index).
//!
//! Missing predictor values are handled as rpart handles them: a case is left out only when its outcome or every
//! predictor is missing. Each split is chosen from the cases with that predictor, and up to `maxsurrogate` surrogate
//! splits (rpart's choose_surg: the split of another predictor that best reproduces the primary split, ranked by
//! agreement over the node, kept when it beats the majority rule) send the cases missing it; cases missing every
//! surrogate follow the majority (rpart's usesurrogate = 2). Variable importance credits primary splits with their
//! improvement and surrogates with their adjusted agreement times it, as rpart's importance().

#![allow(clippy::needless_range_loop)]

use prismqda_stats::rng::SplitMix64;
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::apa;
use crate::chart;
use crate::error::{Error, Result};
use crate::model::{Column, Dataset, Measure as Level};
use crate::result::{Diagnostic, DiagnosticState, ResultBundle, Table};

pub const TREE: &str = "predict.tree";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Chosen from the outcome: categorical outcomes give a classification tree.
    #[default]
    Auto,
    Regression,
    Classification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Prune {
    /// rpart's tree at the given cp.
    None,
    /// The size with the smallest cross-validated error.
    Min,
    /// The smallest tree within one standard error of the smallest cross-validated error.
    #[default]
    #[serde(rename = "1se")]
    OneSe,
}

fn twenty() -> usize {
    20
}
fn cp() -> f64 {
    0.01
}
fn depth() -> usize {
    30
}
fn ten() -> usize {
    10
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
    /// Predictors: numeric ones split at cut points, categorical ones (text or nominal) into groups of categories.
    #[serde(default)]
    pub predictors: Vec<String>,
    #[serde(default)]
    pub kind: Kind,
    #[serde(default = "twenty")]
    pub minsplit: usize,
    /// Smallest leaf; a third of minsplit (rounded) when left out.
    #[serde(default)]
    pub minbucket: Option<usize>,
    #[serde(default = "cp")]
    pub cp: f64,
    #[serde(default = "depth")]
    pub maxdepth: usize,
    #[serde(default = "ten")]
    pub folds: usize,
    #[serde(default = "seed")]
    pub seed: u64,
    #[serde(default)]
    pub fold_variable: Option<String>,
    #[serde(default)]
    pub prune: Prune,
    /// Surrogate splits kept for each split (0: cases missing the split's predictor follow the majority).
    #[serde(default = "five")]
    pub maxsurrogate: usize,
}

#[derive(Clone)]
enum Pred {
    Num(Vec<f64>),
    /// Category codes 0.., and the number of categories.
    Cat(Vec<usize>, usize),
}

#[derive(Clone)]
enum Rule {
    /// Cases with x below the cut go left when `low_left`.
    Num { cut: f64, low_left: bool },
    /// For each category: true = left, false = right; None for a category with no cases (multi-class only).
    Cat(Vec<Option<bool>>),
}

/// A missing category code.
const MISSING: usize = usize::MAX;

#[derive(Clone)]
struct Surrogate {
    var: usize,
    rule: Rule,
    agree: f64,
    adj: f64,
}

#[derive(Clone)]
struct Split {
    var: usize,
    rule: Rule,
    improve: f64,
    surrogates: Vec<Surrogate>,
    /// Where cases missing the primary and every surrogate go: −1 left, 1 right, 0 nowhere (rpart's lastsurrogate).
    default: i8,
}

#[derive(Clone)]
struct Node {
    id: u64,
    n: usize,
    /// Mean, or class index.
    yval: f64,
    /// Class counts (classification).
    counts: Vec<f64>,
    risk: f64,
    complexity: f64,
    split: Option<Split>,
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,
}

struct Problem<'a> {
    y: &'a [f64],
    classes: usize,
    x: &'a [Pred],
    minsplit: usize,
    minbucket: usize,
    alpha: f64,
    maxnode: u64,
    iscale: f64,
    maxsur: usize,
}

impl Problem<'_> {
    /// The node's estimate and risk: mean and sum of squares, or the modal class (lowest index on ties) and the
    /// misclassified count.
    fn eval(&self, rows: &[usize]) -> (f64, Vec<f64>, f64) {
        if self.classes == 0 {
            let m = rows.iter().map(|&i| self.y[i]).sum::<f64>() / rows.len() as f64;
            (m, Vec::new(), rows.iter().map(|&i| (self.y[i] - m).powi(2)).sum())
        } else {
            let mut c = vec![0.0; self.classes];
            for &i in rows {
                c[self.y[i] as usize] += 1.0;
            }
            let n = rows.len() as f64;
            let best = (0..self.classes).fold(0, |b, k| if n - c[k] < n - c[b] { k } else { b });
            (best as f64, c.clone(), n - c[best])
        }
    }

    fn present(&self, var: usize, i: usize) -> bool {
        match &self.x[var] {
            Pred::Num(x) => !x[i].is_nan(),
            Pred::Cat(x, _) => x[i] != MISSING,
        }
    }

    /// Where a rule sends case i: −1 left, 1 right, 0 when the value is missing (or a category without a direction).
    fn dir_of(&self, var: usize, rule: &Rule, i: usize) -> i8 {
        match (rule, &self.x[var]) {
            (Rule::Num { cut, low_left }, Pred::Num(x)) if !x[i].is_nan() => {
                if (x[i] < *cut) == *low_left {
                    -1
                } else {
                    1
                }
            }
            (Rule::Cat(dir), Pred::Cat(x, _)) if x[i] != MISSING => match dir.get(x[i]).copied().flatten() {
                Some(true) => -1,
                Some(false) => 1,
                None => 0,
            },
            _ => 0,
        }
    }

    /// The primary split's direction, else the first surrogate's that applies (0 when none does).
    fn split_dir(&self, s: &Split, i: usize) -> i8 {
        let d = self.dir_of(s.var, &s.rule, i);
        if d != 0 {
            return d;
        }
        s.surrogates.iter().map(|g| self.dir_of(g.var, &g.rule, i)).find(|d| *d != 0).unwrap_or(0)
    }

    /// rpart's surrogate(): the surrogate splits of a node's primary split, best agreement first, and the default
    /// direction (the side the primary sends more cases to).
    fn surrogates(&self, rows: &[usize], s: &Split) -> (Vec<Surrogate>, i8) {
        let ty: Vec<i8> = rows.iter().map(|&i| self.dir_of(s.var, &s.rule, i)).collect();
        let tleft = ty.iter().filter(|d| **d == -1).count() as f64;
        let tright = ty.iter().filter(|d| **d == 1).count() as f64;
        let default = if tleft < tright { 1 } else if tleft > tright { -1 } else { 0 };
        let mut list: Vec<Surrogate> = Vec::new();
        if self.maxsur == 0 {
            // rpart skips surrogate() entirely, leaving no default: such cases stay in the node.
            return (list, 0);
        }
        for v in 0..self.x.len() {
            if v == s.var {
                continue;
            }
            let Some((agree, adj, rule)) = self.choose_surg(rows, &ty, v, tleft, tright) else { continue };
            if adj <= 1e-10 {
                continue;
            }
            // rpart's insert_split: before the first with a smaller agreement; the list keeps the best maxsur.
            let pos = list.iter().position(|g| agree > g.agree).unwrap_or(list.len());
            if pos >= self.maxsur {
                continue;
            }
            list.insert(pos, Surrogate { var: v, rule, agree, adj });
            list.truncate(self.maxsur);
        }
        (list, default)
    }

    /// rpart's choose_surg(): (agreement, adjusted agreement, rule) of the best surrogate split on `var`.
    fn choose_surg(&self, rows: &[usize], ty: &[i8], var: usize, tleft: f64, tright: f64) -> Option<(f64, f64, Rule)> {
        let (agree, rule) = match &self.x[var] {
            Pred::Num(x) => {
                let mut idx: Vec<usize> = (0..rows.len()).filter(|&p| !x[rows[p]].is_nan()).collect();
                if idx.is_empty() {
                    return None;
                }
                idx.sort_by(|&a, &b| x[rows[a]].total_cmp(&x[rows[b]]).then(a.cmp(&b)));
                let (mut ll, mut rl) = (0i64, 0i64);
                for &q in &idx {
                    match ty[q] {
                        -1 => ll += 1,
                        1 => rl += 1,
                        _ => {}
                    }
                }
                let (mut llwt, mut rlwt) = (ll as f64, rl as f64);
                let mut lastx = x[rows[idx[0]]];
                let mut agree = llwt.max(rlwt);
                let (mut lr, mut rr) = (0i64, 0i64);
                let (mut lrwt, mut rrwt) = (0.0, 0.0);
                let mut low_left = true;
                let mut cut = lastx;
                let mut success = false;
                let mut i = 0;
                while ll + rl >= 2 && i < idx.len() {
                    let q = idx[i];
                    let xv = x[rows[q]];
                    if lr + rr >= 2 && xv != lastx {
                        if llwt + rrwt > agree {
                            success = true;
                            agree = llwt + rrwt;
                            low_left = false;
                            cut = (xv + lastx) / 2.0;
                        } else if lrwt + rlwt > agree {
                            success = true;
                            agree = lrwt + rlwt;
                            low_left = true;
                            cut = (xv + lastx) / 2.0;
                        }
                    }
                    match ty[q] {
                        -1 => {
                            ll -= 1;
                            lr += 1;
                            llwt -= 1.0;
                            lrwt += 1.0;
                        }
                        1 => {
                            rl -= 1;
                            rr += 1;
                            rlwt -= 1.0;
                            rrwt += 1.0;
                        }
                        _ => {}
                    }
                    lastx = xv;
                    i += 1;
                }
                if !success {
                    return None;
                }
                (agree, Rule::Num { cut, low_left })
            }
            Pred::Cat(x, nc) => {
                let nc = *nc;
                let (mut left, mut right) = (vec![0usize; nc], vec![0usize; nc]);
                for (p, &i) in rows.iter().enumerate() {
                    if x[i] == MISSING {
                        continue;
                    }
                    match ty[p] {
                        -1 => left[x[i]] += 1,
                        1 => right[x[i]] += 1,
                        _ => {}
                    }
                }
                let llwt: usize = left.iter().sum();
                let rrwt: usize = right.iter().sum();
                let defleft = llwt > rrwt;
                let mut agree = 0.0;
                let (mut lcount, mut rcount) = (0usize, 0usize);
                let mut dir = vec![None; nc];
                for c in 0..nc {
                    if left[c] == 0 && right[c] == 0 {
                        continue;
                    }
                    if left[c] < right[c] || (left[c] == right[c] && !defleft) {
                        agree += right[c] as f64;
                        dir[c] = Some(false);
                        lcount += left[c];
                        rcount += right[c];
                    } else {
                        agree += left[c] as f64;
                        dir[c] = Some(true);
                        lcount += right[c];
                        rcount += left[c];
                    }
                }
                if !(lcount > 1 && rcount > 1) {
                    return None;
                }
                (agree, Rule::Cat(dir))
            }
        };
        let total = tleft + tright;
        let agreement = agree / total;
        let majority = tleft.max(tright) / total;
        Some((agreement, (agreement - majority) / (1.0 - majority), rule))
    }

    /// The best split of one predictor in a node: (improvement, rule), as rpart's anova() and gini().
    fn choose(&self, rows: &[usize], var: usize, risk: f64) -> Option<(f64, Rule)> {
        let edge = self.minbucket;
        let kept: Vec<usize> = rows.iter().copied().filter(|&i| self.present(var, i)).collect();
        let rows = &kept[..];
        let n = rows.len();
        if n == 0 {
            return None;
        }
        match &self.x[var] {
            Pred::Num(x) => {
                let mut idx = rows.to_vec();
                idx.sort_by(|&a, &b| x[a].total_cmp(&x[b]).then(a.cmp(&b)));
                if x[idx[0]] == x[idx[n - 1]] {
                    return None;
                }
                if self.classes == 0 {
                    let grand = idx.iter().map(|&i| self.y[i]).sum::<f64>() / n as f64;
                    let (mut ls, mut rs, mut lw, mut rw) = (0.0, 0.0, 0.0, n as f64);
                    let (mut best, mut where_, mut low_left) = (0.0, 0, true);
                    let mut rn = n;
                    let mut i = 0;
                    while rn > edge {
                        lw += 1.0;
                        rw -= 1.0;
                        rn -= 1;
                        let t = self.y[idx[i]] - grand;
                        ls += t;
                        rs -= t;
                        let ln = i + 1;
                        if x[idx[i + 1]] != x[idx[i]] && ln >= edge {
                            let temp = ls * ls / lw + rs * rs / rw;
                            if temp > best {
                                best = temp;
                                where_ = i;
                                low_left = ls < rs;
                            }
                        }
                        i += 1;
                    }
                    (best > 0.0).then(|| (best / risk, Rule::Num { cut: (x[idx[where_]] + x[idx[where_ + 1]]) / 2.0, low_left }))
                } else {
                    let k = self.classes;
                    let (mut left, mut right) = (vec![0.0; k], vec![0.0; k]);
                    for &i in &idx {
                        right[self.y[i] as usize] += 1.0;
                    }
                    let (mut lw, mut rw) = (0.0, n as f64);
                    let gini = |c: &[f64], w: f64| if w > 0.0 { c.iter().map(|v| { let p = v / w; w * p * (1.0 - p) }).sum::<f64>() } else { 0.0 };
                    let total = gini(&right, rw);
                    let (mut best, mut where_, mut low_left) = (total, 0, true);
                    let mut rtot = n;
                    let mut i = 0;
                    while rtot > edge {
                        let c = self.y[idx[i]] as usize;
                        rw -= 1.0;
                        lw += 1.0;
                        rtot -= 1;
                        right[c] -= 1.0;
                        left[c] += 1.0;
                        let ltot = i + 1;
                        if ltot >= edge && x[idx[i + 1]] != x[idx[i]] {
                            let temp = gini(&left, lw) + gini(&right, rw);
                            if temp < best {
                                best = temp;
                                where_ = i;
                                let lm: f64 = (0..k).map(|j| left[j] / lw * j as f64).sum();
                                let rm: f64 = (0..k).map(|j| right[j] / rw * j as f64).sum();
                                low_left = lm < rm;
                            }
                        }
                        i += 1;
                    }
                    let improve = total - best;
                    (improve > 0.0).then(|| (improve, Rule::Num { cut: (x[idx[where_]] + x[idx[where_ + 1]]) / 2.0, low_left }))
                }
            }
            Pred::Cat(x, nc) => {
                let nc = *nc;
                let mut count = vec![0usize; nc];
                for &i in rows {
                    count[x[i]] += 1;
                }
                if count.iter().filter(|c| **c > 0).count() < 2 {
                    return None;
                }
                if self.classes == 0 {
                    let grand = rows.iter().map(|&i| self.y[i]).sum::<f64>() / n as f64;
                    let mut sums = vec![0.0; nc];
                    for &i in rows {
                        sums[x[i]] += self.y[i] - grand;
                    }
                    let mean: Vec<f64> = (0..nc).map(|j| if count[j] > 0 { sums[j] / count[j] as f64 } else { 0.0 }).collect();
                    let order = gray_order(&count, &mean);
                    let mut side: Vec<Option<bool>> = (0..nc).map(|j| (count[j] > 0).then_some(false)).collect();
                    let (mut ln, mut rn, mut lw, mut rw, mut ls, mut rs) = (0usize, n, 0.0, n as f64, 0.0, 0.0);
                    let mut best = 0.0;
                    let mut out: Option<Vec<Option<bool>>> = None;
                    for &j in &order {
                        side[j] = Some(true);
                        ln += count[j];
                        rn -= count[j];
                        lw += count[j] as f64;
                        rw -= count[j] as f64;
                        ls += sums[j];
                        rs -= sums[j];
                        if ln >= edge && rn >= edge {
                            let temp = ls * ls / lw + rs * rs / rw;
                            if temp > best {
                                best = temp;
                                // The group with the lower mean goes left.
                                let flip = ls / lw > rs / rw;
                                out = Some(side.iter().map(|s| s.map(|v| v != flip)).collect());
                            }
                        }
                    }
                    out.map(|o| (best / risk, Rule::Cat(o)))
                } else {
                    let k = self.classes;
                    let mut cc = vec![vec![0.0; nc]; k];
                    for &i in rows {
                        cc[self.y[i] as usize][x[i]] += 1.0;
                    }
                    let gini = |c: &[f64], w: f64| if w > 0.0 { c.iter().map(|v| { let p = v / w; w * p * (1.0 - p) }).sum::<f64>() } else { 0.0 };
                    let (mut left, mut right) = (vec![0.0; k], (0..k).map(|c| cc[c].iter().sum::<f64>()).collect::<Vec<f64>>());
                    let (mut lw, mut rw) = (0.0, n as f64);
                    let (mut ltot, mut rtot) = (0usize, n);
                    let total = gini(&right, rw);
                    let mut best = total;
                    let mut out: Option<Vec<Option<bool>>> = None;
                    // Two classes: categories ordered by the share of the first class; more: every subset (Gray code).
                    let moves: Vec<usize> = if k == 2 {
                        let rate: Vec<f64> = (0..nc).map(|j| if count[j] > 0 { cc[0][j] / count[j] as f64 } else { 0.0 }).collect();
                        gray_order(&count, &rate)
                    } else {
                        gray_subsets(&count)
                    };
                    let mut side: Vec<Option<bool>> = (0..nc).map(|j| (count[j] > 0).then_some(false)).collect();
                    for j in moves {
                        let to_left = side[j] != Some(true);
                        side[j] = Some(to_left);
                        let s = if to_left { 1.0 } else { -1.0 };
                        lw += s * count[j] as f64;
                        rw -= s * count[j] as f64;
                        if to_left {
                            ltot += count[j];
                            rtot -= count[j];
                        } else {
                            ltot -= count[j];
                            rtot += count[j];
                        }
                        for c in 0..k {
                            left[c] += s * cc[c][j];
                            right[c] -= s * cc[c][j];
                        }
                        if ltot >= edge && rtot >= edge {
                            let temp = gini(&left, lw) + gini(&right, rw);
                            if temp < best {
                                best = temp;
                                let lm: f64 = (0..k).map(|c| left[c] / lw * c as f64).sum();
                                let rm: f64 = (0..k).map(|c| right[c] / rw * c as f64).sum();
                                let keep = lm < rm;
                                out = Some(side.iter().map(|v| v.map(|b| b == keep)).collect());
                            }
                        }
                    }
                    let improve = total - best;
                    out.filter(|_| improve > 0.0).map(|o| (improve, Rule::Cat(o)))
                }
            }
        }
    }

    fn bsplit(&mut self, rows: &[usize], risk: f64) -> Option<Split> {
        let mut best: Option<Split> = None;
        for v in 0..self.x.len() {
            let Some((improve, rule)) = self.choose(rows, v, risk) else { continue };
            if improve > self.iscale {
                self.iscale = improve;
            }
            if improve > self.iscale * 1e-10 && best.as_ref().is_none_or(|b| improve > b.improve) {
                best = Some(Split { var: v, rule, improve, surrogates: Vec::new(), default: 0 });
            }
        }
        best
    }

    /// rpart's partition(): returns (number of splits kept, risk of the leaves).
    fn partition(&mut self, nodenum: u64, me: &mut Node, rows: &[usize]) -> (usize, f64) {
        let mut tempcp;
        if nodenum > 1 {
            let (yval, counts, risk) = self.eval(rows);
            me.yval = yval;
            me.counts = counts;
            me.risk = risk;
            me.n = rows.len();
            tempcp = me.risk.min(me.complexity);
        } else {
            tempcp = me.risk;
        }
        if me.n < self.minsplit || tempcp <= self.alpha || nodenum > self.maxnode {
            me.complexity = self.alpha;
            return (0, me.risk);
        }
        let Some(mut split) = self.bsplit(rows, me.risk) else {
            me.complexity = self.alpha;
            return (0, me.risk);
        };
        (split.surrogates, split.default) = self.surrogates(rows, &split);
        // rpart's nodesplit(): the primary, then the surrogates, then the default direction; with no default (a tie)
        // a case stays in this node.
        let (lrows, rrows): (Vec<usize>, Vec<usize>) = {
            let (mut l, mut r) = (Vec::new(), Vec::new());
            for &i in rows {
                match self.split_dir(&split, i) {
                    -1 => l.push(i),
                    1 => r.push(i),
                    _ => match split.default {
                        -1 => l.push(i),
                        1 => r.push(i),
                        _ => {}
                    },
                }
            }
            (l, r)
        };
        me.split = Some(split);
        let mut left = Box::new(Node { id: 2 * nodenum, n: 0, yval: 0.0, counts: Vec::new(), risk: 0.0, complexity: tempcp - self.alpha, split: None, left: None, right: None });
        let (mut ls, mut lr) = self.partition(2 * nodenum, &mut left, &lrows);
        tempcp = ((me.risk - lr) / (ls as f64 + 1.0)).max(me.risk - left.risk).min(me.complexity);
        let mut right = Box::new(Node { id: 2 * nodenum + 1, n: 0, yval: 0.0, counts: Vec::new(), risk: 0.0, complexity: tempcp - self.alpha, split: None, left: None, right: None });
        let (mut rs, mut rr) = self.partition(2 * nodenum + 1, &mut right, &rrows);
        tempcp = (me.risk - (lr + rr)) / ((ls + rs) as f64 + 1.0);
        if right.complexity > left.complexity {
            if tempcp > left.complexity {
                lr = left.risk;
                ls = 0;
                tempcp = (me.risk - (lr + rr)) / ((ls + rs) as f64 + 1.0);
                if tempcp > right.complexity {
                    rr = right.risk;
                    rs = 0;
                }
            }
        } else if tempcp > right.complexity {
            rs = 0;
            rr = right.risk;
            tempcp = (me.risk - (lr + rr)) / ((ls + rs) as f64 + 1.0);
            if tempcp > left.complexity {
                lr = left.risk;
                ls = 0;
            }
        }
        me.complexity = (me.risk - (lr + rr)) / ((ls + rs) as f64 + 1.0);
        if me.complexity <= self.alpha {
            me.split = None;
            (0, me.risk)
        } else {
            me.left = Some(left);
            me.right = Some(right);
            (ls + rs + 1, lr + rr)
        }
    }

    fn grow(&mut self, rows: &[usize]) -> Node {
        let (yval, counts, risk) = self.eval(rows);
        let mut root = Node { id: 1, n: rows.len(), yval, counts, risk, complexity: risk, split: None, left: None, right: None };
        self.partition(1, &mut root, rows);
        root
    }
}

/// rpart's ordered search: categories with no cases first, then by value (a stable insertion sort).
fn gray_order(count: &[usize], val: &[f64]) -> Vec<usize> {
    // rpart's graycode() starts after the categories with no cases, so they never move (and get no direction).
    let mut full: Vec<usize> = (0..count.len()).filter(|&j| count[j] > 0).collect();
    full.sort_by(|&a, &b| val[a].total_cmp(&val[b]));
    full
}

/// rpart's Gray-code enumeration of subsets (categories with cases only): the category that changes side each step.
fn gray_subsets(count: &[usize]) -> Vec<usize> {
    let maxc = count.len();
    let mut gray: Vec<u8> = count.iter().map(|c| if *c > 0 { 1 } else { 0 }).collect();
    let mut out = Vec::new();
    loop {
        let mut next = None;
        for i in 0..maxc.saturating_sub(1) {
            if gray[i] == 1 {
                gray[i] = 2;
                next = Some(i);
                break;
            } else if gray[i] == 2 {
                gray[i] = 1;
            }
        }
        match next {
            Some(i) => out.push(i),
            None => break,
        }
        if out.len() > 1 << 20 {
            break;
        }
    }
    out
}

fn fix_cp(me: &mut Node, parent: f64) {
    if me.complexity > parent {
        me.complexity = parent;
    }
    let c = me.complexity;
    if let (Some(l), Some(r)) = (me.left.as_mut(), me.right.as_mut()) {
        fix_cp(l, c);
        fix_cp(r, c);
    }
}

/// One row of rpart's complexity table.
#[derive(Clone)]
struct CpRow {
    cp: f64,
    risk: f64,
    nsplit: usize,
    xrisk: f64,
    xstd: f64,
}

fn make_cp_list(me: &mut Node, parent: f64, alpha: f64, list: &mut Vec<CpRow>) {
    if me.complexity > parent {
        me.complexity = parent;
    }
    let me_cp = me.complexity.max(alpha);
    if let (Some(l), Some(r)) = (me.left.as_mut(), me.right.as_mut()) {
        make_cp_list(l, me_cp, alpha, list);
        make_cp_list(r, me_cp, alpha, list);
    }
    if me_cp < parent {
        let mut at = list.len();
        for (k, row) in list.iter().enumerate() {
            if me_cp == row.cp {
                return;
            }
            if me_cp > row.cp {
                at = k;
                break;
            }
        }
        list.insert(at, CpRow { cp: me_cp, risk: 0.0, nsplit: 0, xrisk: 0.0, xstd: 0.0 });
    }
}

/// rpart's make_cp_table(): returns the index it stopped at.
fn make_cp_table(me: &Node, parent: f64, nsplit: usize, list: &mut [CpRow]) -> isize {
    let mut k: isize = if let (Some(l), Some(r)) = (&me.left, &me.right) {
        make_cp_table(l, me.complexity, 0, list);
        make_cp_table(r, me.complexity, nsplit + 1, list)
    } else {
        list.len() as isize - 1
    };
    while k >= 0 && list[k as usize].cp < parent {
        list[k as usize].risk += me.risk;
        list[k as usize].nsplit += nsplit;
        k -= 1;
    }
    k
}

/// Runs a case down the tree for each cp (largest first), as rpart's rundown(): its prediction at each.
fn rundown<'a>(root: &'a Node, p: &Problem, i: usize, cps: &[f64]) -> Vec<&'a Node> {
    let mut node = root;
    let mut out = Vec::with_capacity(cps.len());
    for &c in cps {
        while c < node.complexity {
            let Some(s) = &node.split else { break };
            let (Some(l), Some(r)) = (&node.left, &node.right) else { break };
            let d = match p.split_dir(s, i) {
                0 => s.default,
                d => d,
            };
            node = if d == -1 { l } else { r };
        }
        out.push(node);
    }
    out
}

/// The leaf a case reaches in the tree pruned at `cp` (nodes with complexity ≤ cp are leaves).
fn leaf<'a>(root: &'a Node, p: &Problem, i: usize, cp: f64) -> &'a Node {
    let mut node = root;
    while node.complexity > cp {
        let (Some(s), Some(l), Some(r)) = (&node.split, &node.left, &node.right) else { break };
        node = match p.split_dir(s, i) {
            -1 => l,
            1 => r,
            _ if l.n > r.n => l,
            _ if r.n > l.n => r,
            _ => break,
        };
    }
    node
}

fn loss(y: f64, node: &Node, classes: usize) -> f64 {
    if classes == 0 {
        (y - node.yval).powi(2)
    } else if (node.yval - y).abs() < 0.5 {
        0.0
    } else {
        1.0
    }
}

/// Every node of the pruned tree, depth first, with its depth.
fn walk<'a>(n: &'a Node, cp: f64, depth: usize, out: &mut Vec<(&'a Node, usize, bool)>) {
    let internal = n.complexity > cp && n.left.is_some();
    out.push((n, depth, internal));
    if internal {
        walk(n.left.as_ref().expect("internal"), cp, depth + 1, out);
        walk(n.right.as_ref().expect("internal"), cp, depth + 1, out);
    }
}

/// A predictor's display name, category codes and labels (categorical) or values (numeric).
type Column4 = (String, Vec<Slot<usize>>, Vec<String>, Vec<Slot<f64>>);

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    if spec.predictors.is_empty() {
        return Err(Error::Invalid("Choose at least one predictor.".into()));
    }
    let yvar = ds.variable(&spec.dependent)?;
    let yi = ds.find(&spec.dependent).ok_or_else(|| Error::UnknownVariable(spec.dependent.clone()))?;
    let categorical_y = match spec.kind {
        Kind::Classification => true,
        Kind::Regression => false,
        Kind::Auto => matches!(ds.columns[yi], Column::Text(_)) || yvar.measure != Level::Scale,
    };
    let (yslots, ylevels): (Vec<Slot<f64>>, Vec<String>) = if categorical_y {
        let (s, l) = categories(ds, &spec.dependent)?;
        (s.iter().map(|c| match c { Slot::Valid(i) => Slot::Valid(*i as f64), Slot::Empty => Slot::Empty, Slot::UserMissing => Slot::UserMissing }).collect(), l.iter().map(|lv| lv.label.clone()).collect())
    } else {
        (numeric(ds, &spec.dependent)?, Vec::new())
    };
    // Predictors: categorical when text or nominal.
    let mut cols: Vec<Column4> = Vec::new();
    let mut is_cat = Vec::new();
    for name in &spec.predictors {
        let v = ds.variable(name)?;
        let i = ds.find(name).ok_or_else(|| Error::UnknownVariable(name.clone()))?;
        let cat = matches!(ds.columns[i], Column::Text(_)) || v.measure == Level::Nominal;
        is_cat.push(cat);
        if cat {
            let (s, l) = categories(ds, name)?;
            cols.push((v.display_name().to_string(), s, l.iter().map(|lv| lv.label.clone()).collect(), Vec::new()));
        } else {
            cols.push((v.display_name().to_string(), Vec::new(), Vec::new(), numeric(ds, name)?));
        }
    }
    let folds_given: Option<Vec<Slot<f64>>> = spec.fold_variable.as_ref().map(|f| numeric(ds, f)).transpose()?;
    let total = ds.n_rows();
    let mut tally = Tally::default();
    let mut used = Vec::new();
    for r in 0..total {
        if tally.check(&yslots[r], &spec.dependent) {
            continue;
        }
        // As rpart's na.rpart: a case is used unless every predictor is missing.
        let any = cols.iter().enumerate().any(|(k, (_, cs, _, ns))| if is_cat[k] { cs[r].value().is_some() } else { ns[r].value().is_some() });
        if !any {
            tally.add("every predictor missing");
            continue;
        }
        if let Some(f) = &folds_given {
            if tally.check(&f[r], spec.fold_variable.as_deref().unwrap_or("fold")) {
                continue;
            }
        }
        used.push(r);
    }
    let n = used.len();
    let y: Vec<f64> = used.iter().map(|&r| yslots[r].value().expect("checked")).collect();
    // Class codes renumbered to the classes present (as R's factor levels after dropping unused ones).
    let (y, ylevels) = if categorical_y {
        let mut present: Vec<usize> = y.iter().map(|v| *v as usize).collect();
        present.sort_unstable();
        present.dedup();
        (y.iter().map(|v| present.binary_search(&(*v as usize)).expect("present") as f64).collect::<Vec<f64>>(), present.iter().map(|&k| ylevels[k].clone()).collect::<Vec<String>>())
    } else {
        (y, ylevels)
    };
    let x: Vec<Pred> = cols
        .iter()
        .enumerate()
        .map(|(k, (_, cs, levels, ns))| if is_cat[k] { Pred::Cat(used.iter().map(|&r| cs[r].value().unwrap_or(MISSING)).collect(), levels.len()) } else { Pred::Num(used.iter().map(|&r| ns[r].value().unwrap_or(f64::NAN)).collect()) })
        .collect();
    let classes = if categorical_y { ylevels.len() } else { 0 };
    let minbucket = spec.minbucket.unwrap_or(((spec.minsplit as f64) / 3.0).round() as usize).max(1);
    let lv_kind = if categorical_y { "Classification" } else { "Regression" };
    let note = format!(
        "{} tree as R's rpart (method \"{}\"): minsplit = {}, minbucket = {minbucket}, cp = {}, maxdepth = {}; {}-fold cross-validation of the complexity table{}. Missing predictor values are handled by up to {} surrogate splits per split, then the majority direction (rpart's usesurrogate = 2). Computed by PrismStatistics.",
        lv_kind,
        if categorical_y { "class" } else { "anova" },
        spec.minsplit,
        apa::num(spec.cp, 4),
        spec.maxdepth,
        spec.folds,
        match spec.prune {
            Prune::None => "; the tree is rpart's at that cp".to_string(),
            Prune::Min => "; pruned to the size with the smallest cross-validated error".to_string(),
            Prune::OneSe => "; pruned by the one-standard-error rule".to_string(),
        },
        spec.maxsurrogate
    );
    let mut b = common::bundle(TREE, METHOD_VERSION, format!("{lv_kind} Tree: {}", yvar.display_name()), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, n), note);
    set_text(&mut b, "y.label", yvar.display_name().to_string());
    set_text(&mut b, "kind", lv_kind.to_lowercase());
    set(&mut b, "n", n as f64);
    if n < spec.minsplit.max(4) {
        fail(&mut b, "There are too few complete cases to grow a tree.");
        return Ok(b);
    }
    if categorical_y && classes < 2 {
        fail(&mut b, "The outcome has only one category among the cases used.");
        return Ok(b);
    }
    let mut prob = Problem { y: &y, classes, x: &x, minsplit: spec.minsplit, minbucket, alpha: 0.0, maxnode: (1u64 << spec.maxdepth.clamp(1, 62)) - 1, iscale: 0.0, maxsur: spec.maxsurrogate };
    let all: Vec<usize> = (0..n).collect();
    let (_, _, root_risk) = prob.eval(&all);
    prob.alpha = spec.cp * root_risk;
    let mut root = prob.grow(&all);
    let root_cp = root.complexity;
    let mut cpt = vec![CpRow { cp: root_cp, risk: root.risk, nsplit: 0, xrisk: 0.0, xstd: 0.0 }];
    let has_splits = root.left.is_some();
    if has_splits {
        let alpha = prob.alpha;
        make_cp_list(&mut root, root_cp, alpha, &mut cpt);
        // make_cp_list may lower the root's own complexity row only through its children; rebuild the risks.
        for row in cpt.iter_mut() {
            row.risk = 0.0;
            row.nsplit = 0;
        }
        cpt[0].risk = 0.0;
        make_cp_table(&root, root_cp, 0, &mut cpt);
        // The head row is the whole tree collapsed to the root.
        cpt[0].risk = root.risk;
    }
    // Cross-validation.
    let fold_of: Vec<usize> = if let Some(f) = &folds_given {
        let raw: Vec<f64> = used.iter().map(|&r| f[r].value().expect("checked")).collect();
        let mut u = raw.clone();
        u.sort_by(f64::total_cmp);
        u.dedup();
        raw.iter().map(|v| u.iter().position(|x| x == v).expect("listed")).collect()
    } else {
        let mut rng = SplitMix64::new(spec.seed);
        let mut idx: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            let j = rng.below(i + 1);
            idx.swap(i, j);
        }
        let mut f = vec![0; n];
        for (pos, &i) in idx.iter().enumerate() {
            f[i] = pos % spec.folds.max(2);
        }
        f
    };
    let nfold = fold_of.iter().max().map_or(0, |m| m + 1);
    let xval = has_splits && nfold > 1;
    if xval {
        let mut cps: Vec<f64> = vec![10.0 * cpt[0].cp];
        for k in 1..cpt.len() {
            cps.push((cpt[k - 1].cp * cpt[k].cp).sqrt());
        }
        let nf = n as f64;
        for fold in 0..nfold {
            let train: Vec<usize> = (0..n).filter(|&i| fold_of[i] != fold).collect();
            let test: Vec<usize> = (0..n).filter(|&i| fold_of[i] == fold).collect();
            if test.is_empty() || train.is_empty() {
                continue;
            }
            let scale = train.len() as f64 / nf;
            let fcps: Vec<f64> = cps.iter().map(|c| c * scale).collect();
            let mut fp = Problem { y: &y, classes, x: &x, minsplit: spec.minsplit, minbucket, alpha: prob.alpha * scale, maxnode: prob.maxnode, iscale: 0.0, maxsur: spec.maxsurrogate };
            let mut ftree = fp.grow(&train);
            let c0 = ftree.complexity;
            fix_cp(&mut ftree, c0);
            for &i in &test {
                for (k, node) in rundown(&ftree, &fp, i, &fcps).into_iter().enumerate() {
                    let e = loss(y[i], node, classes);
                    cpt[k].xrisk += e;
                    cpt[k].xstd += e * e;
                }
            }
        }
        for row in cpt.iter_mut() {
            row.xstd = (row.xstd - row.xrisk * row.xrisk / nf).max(0.0).sqrt();
        }
    }
    // The tree reported: rpart's, or pruned by cross-validation.
    let scale = 1.0 / root_risk;
    let imin = (0..cpt.len()).fold(0, |a, k| if cpt[k].xrisk < cpt[a].xrisk { k } else { a });
    let chosen = match spec.prune {
        _ if !xval => cpt.len() - 1,
        Prune::None => cpt.len() - 1,
        Prune::Min => imin,
        Prune::OneSe => (0..cpt.len()).find(|&k| cpt[k].xrisk <= cpt[imin].xrisk + cpt[imin].xstd).unwrap_or(imin),
    };
    let prune_cp = if chosen == cpt.len() - 1 { prob.alpha } else { cpt[chosen].cp };
    let mut cprows = Vec::new();
    for (k, row) in cpt.iter().enumerate() {
        let key = format!("cp.{}", k + 1);
        set(&mut b, format!("{key}.CP"), row.cp * scale);
        set(&mut b, format!("{key}.nsplit"), row.nsplit as f64);
        set(&mut b, format!("{key}.rel_error"), row.risk * scale);
        if xval {
            set(&mut b, format!("{key}.xerror"), row.xrisk * scale);
            set(&mut b, format!("{key}.xstd"), row.xstd * scale);
        }
        let mut cells = vec![num(row.cp * scale, 4), count(row.nsplit), num(row.risk * scale, 4)];
        if xval {
            cells.extend([num(row.xrisk * scale, 4), num(row.xstd * scale, 4)]);
        }
        cells.push(text(if k == chosen { "◀ chosen" } else { "" }));
        cprows.push(cells);
    }
    set(&mut b, "cp.count", cpt.len() as f64);
    set(&mut b, "chosen.nsplit", cpt[chosen].nsplit as f64);
    let mut cpcols = vec!["CP".to_string(), "Splits".into(), "Relative error".into()];
    if xval {
        cpcols.extend(["CV error".into(), "CV SE".into()]);
    }
    cpcols.push("".into());
    // Nodes of the chosen tree.
    let mut nodes = Vec::new();
    walk(&root, prune_cp, 0, &mut nodes);
    let names: Vec<String> = cols.iter().map(|c| c.0.clone()).collect();
    let rule_text = |s: &Split, left: bool| -> String {
        match &s.rule {
            Rule::Num { cut, low_left } => format!("{} {} {}", names[s.var], if left == *low_left { "<" } else { "≥" }, apa::num(*cut, 4)),
            Rule::Cat(dir) => {
                let levels = &cols[s.var].2;
                let set: Vec<String> = dir.iter().enumerate().filter(|(_, d)| **d == Some(left)).map(|(j, _)| levels[j].clone()).collect();
                format!("{} in {{{}}}", names[s.var], set.join(", "))
            }
        }
    };
    let pred_text = |nd: &Node| if classes == 0 { apa::num(nd.yval, 3) } else { format!("{} ({}%)", ylevels[nd.yval as usize], apa::num(100.0 * nd.counts[nd.yval as usize] / nd.n as f64, 1)) };
    let mut node_rows = Vec::new();
    let mut leaves = 0;
    let mut importance = vec![0.0; names.len()];
    let mut sur_rows = Vec::new();
    let rule_text_g = |g: &Surrogate, left: bool| rule_text(&Split { var: g.var, rule: g.rule.clone(), improve: 0.0, surrogates: Vec::new(), default: 0 }, left);
    for (nd, depth, internal) in &nodes {
        let key = format!("node.{}", nd.id);
        set(&mut b, format!("{key}.n"), nd.n as f64);
        set(&mut b, format!("{key}.yval"), if classes == 0 { nd.yval } else { nd.yval + 1.0 });
        set(&mut b, format!("{key}.dev"), nd.risk);
        let split = if *internal {
            let s = nd.split.as_ref().expect("internal");
            set_text(&mut b, format!("{key}.var"), names[s.var].clone());
            if let Rule::Num { cut, .. } = s.rule {
                set(&mut b, format!("{key}.cut"), cut);
            }
            // rpart's importance(): the primary split's improvement (as a change in risk for regression trees), and
            // each surrogate's adjusted agreement times it.
            let scaled = if classes == 0 { s.improve * nd.risk } else { s.improve };
            importance[s.var] += scaled;
            for (j, g) in s.surrogates.iter().enumerate() {
                importance[g.var] += scaled * g.adj;
                let sk = format!("{key}.sur.{}", j + 1);
                set_text(&mut b, format!("{sk}.var"), names[g.var].clone());
                set(&mut b, format!("{sk}.agree"), g.agree);
                set(&mut b, format!("{sk}.adj"), g.adj);
                if let Rule::Num { cut, .. } = g.rule {
                    set(&mut b, format!("{sk}.cut"), cut);
                }
                sur_rows.push(vec![text(nd.id.to_string()), text(names[s.var].clone()), text(format!("{}  |  {}", rule_text_g(g, true), rule_text_g(g, false))), num(g.agree, 3), num(g.adj, 3)]);
            }
            format!("{}  |  {}", rule_text(s, true), rule_text(s, false))
        } else {
            leaves += 1;
            String::new()
        };
        node_rows.push(vec![text(format!("{}{}", "  ".repeat(*depth), nd.id)), count(nd.n), text(pred_text(nd)), num(nd.risk, 3), text(if *internal { split } else { "leaf".into() })]);
    }
    set(&mut b, "leaves", leaves as f64);
    b.tables.push(Table {
        id: "nodes".into(),
        title: "Tree".into(),
        columns: vec!["Node".into(), "n".into(), if classes == 0 { "Mean".into() } else { "Predicted class".into() }, if classes == 0 { "Sum of squares".into() } else { "Misclassified".into() }, "Split (left  |  right)".into()],
        rows: node_rows,
        footnotes: vec![format!("Nodes are numbered as rpart numbers them (the children of node k are 2k and 2k + 1); the tree has {} splits and {leaves} leaves.", cpt[chosen].nsplit)],
    });
    b.tables.push(Table { id: "cptable".into(), title: "Complexity (cost-complexity pruning)".into(), columns: cpcols, rows: cprows, footnotes: vec!["Errors are relative to the root node's (no splits). CV error is the cross-validated relative error with its standard error.".into()] });
    if !sur_rows.is_empty() {
        b.tables.push(Table {
            id: "surrogates".into(),
            title: "Surrogate splits".into(),
            columns: vec!["Node".into(), "Primary split on".into(), "Surrogate (left  |  right)".into(), "Agreement".into(), "Adjusted agreement".into()],
            rows: sur_rows,
            footnotes: vec!["A case missing the primary split's predictor is sent by the first surrogate it has a value for, then by the majority. Agreement is the share of the node's cases the surrogate sends the same way as the primary split; adjusted agreement is its gain over sending everyone the majority way.".into()],
        });
    }
    // Importance as rpart's importance().
    let tot: f64 = importance.iter().sum();
    if tot > 0.0 {
        let mut order: Vec<usize> = (0..names.len()).filter(|&j| importance[j] > 0.0).collect();
        order.sort_by(|&a, &c| importance[c].total_cmp(&importance[a]));
        for j in &order {
            set(&mut b, format!("importance.{}", spec.predictors[*j]), importance[*j]);
            set(&mut b, format!("importance_pct.{}", spec.predictors[*j]), 100.0 * importance[*j] / tot);
        }
        b.tables.push(Table {
            id: "importance".into(),
            title: "Predictor importance".into(),
            columns: vec!["Predictor".into(), "Share of the risk reduction (%)".into()],
            rows: order.iter().map(|&j| vec![text(names[j].clone()), num(100.0 * importance[j] / tot, 1)]).collect(),
            footnotes: vec!["As rpart's variable importance for the tree shown: each primary split's improvement, plus each surrogate split's adjusted agreement times it, as shares of the total.".into()],
        });
    }
    // Performance.
    let pred: Vec<&Node> = (0..n).map(|i| leaf(&root, &prob, i, prune_cp)).collect();
    let mut perf = Vec::new();
    if classes == 0 {
        let ybar = y.iter().sum::<f64>() / n as f64;
        let sst: f64 = y.iter().map(|v| (v - ybar).powi(2)).sum();
        let sse: f64 = (0..n).map(|i| (y[i] - pred[i].yval).powi(2)).sum();
        set(&mut b, "train.r2", 1.0 - sse / sst);
        set(&mut b, "train.rmse", (sse / n as f64).sqrt());
        perf.push(vec![text("R²"), if xval { num(1.0 - cpt[chosen].xrisk * scale, 3) } else { text("") }, num(1.0 - sse / sst, 3)]);
        perf.push(vec![text("RMSE"), if xval { num((cpt[chosen].xrisk / n as f64).sqrt(), 4) } else { text("") }, num((sse / n as f64).sqrt(), 4)]);
        if xval {
            set(&mut b, "cv.r2", 1.0 - cpt[chosen].xrisk * scale);
            set(&mut b, "cv.rmse", (cpt[chosen].xrisk / n as f64).sqrt());
        }
    } else {
        let wrong = (0..n).filter(|&i| (pred[i].yval - y[i]).abs() > 0.5).count() as f64;
        set(&mut b, "train.accuracy", 1.0 - wrong / n as f64);
        perf.push(vec![text("Accuracy"), if xval { num(1.0 - cpt[chosen].xrisk / n as f64, 3) } else { text("") }, num(1.0 - wrong / n as f64, 3)]);
        if xval {
            set(&mut b, "cv.accuracy", 1.0 - cpt[chosen].xrisk / n as f64);
        }
        let base = cpt[0].risk / n as f64;
        perf.push(vec![text("Accuracy of always predicting the largest class"), text(""), num(1.0 - base, 3)]);
    }
    b.tables.push(Table { id: "performance".into(), title: "Predictive performance".into(), columns: vec!["Measure".into(), "Cross-validated".into(), "In the training data".into()], rows: perf, footnotes: vec!["Cross-validated values come from the complexity table's CV error at the chosen size.".into()] });
    // The diagram.
    let draw: Vec<chart::TreeNode> = nodes
        .iter()
        .map(|(nd, _, internal)| chart::TreeNode {
            id: nd.id,
            label: if *internal { rule_text(nd.split.as_ref().expect("internal"), true) } else { String::new() },
            right_label: if *internal { rule_text(nd.split.as_ref().expect("internal"), false) } else { String::new() },
            value: pred_text(nd),
            n: nd.n,
            leaf: !internal,
        })
        .collect();
    let alt = format!("Decision tree with {} splits and {leaves} leaves; the first split is on {}.", cpt[chosen].nsplit, root.split.as_ref().filter(|_| root.complexity > prune_cp).map_or("nothing".to_string(), |s| names[s.var].clone()));
    b.charts.push(chart::Chart { id: "tree".into(), title: "Decision tree".into(), svg: chart::tree_diagram(&draw, &alt), alt });
    if !has_splits {
        b.diagnostics.push(Diagnostic { id: "nosplit".into(), title: "No split".into(), state: DiagnosticState::ReviewNeeded, message: "No split improved the fit enough at this cp; the tree is the root alone.".into() });
    }
    if xval && n < 10 * nfold {
        b.diagnostics.push(Diagnostic { id: "folds".into(), title: "Fold size".into(), state: DiagnosticState::ReviewNeeded, message: format!("With {n} cases the cross-validated error is noisy, and the chosen size can change with the folds.") });
    }
    b.diagnostics.push(common::size_diagnostic(n, format!("{n} cases")));
    Ok(b)
}
