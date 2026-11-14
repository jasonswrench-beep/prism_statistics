//! Permutation tests (`perm.*`, checklist B10): p values from rearranging the data rather than from a distribution.
//!
//! * Two independent groups: the difference in means, over reassignments of cases to groups.
//! * Paired measures: the mean difference, over sign flips of the differences.
//! * Correlation: Pearson's r, over rearrangements of one variable.
//! * One-way ANOVA: the F statistic, over reassignments of cases to groups.
//!
//! When there are at most `exact_limit` rearrangements (100,000 by default) every one is enumerated and the p value is
//! exact; otherwise `resamples` random rearrangements are drawn from a seeded generator and p = (count + 1) / (B + 1).
//! Tests are two-sided: a rearrangement counts when its statistic is at least as far from the null as the observed one.

use prismqda_stats::rng::SplitMix64;
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::apa;
use crate::error::{Error, Result};
use crate::model::{format_code, Dataset};
use crate::result::{ResultBundle, Table};

pub const METHOD_ID: &str = "perm.test";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    #[default]
    TwoGroups,
    Paired,
    Correlation,
    Anova,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    #[serde(default)]
    pub test: Kind,
    /// The outcome (two groups, ANOVA), or the two variables (paired, correlation).
    pub variables: Vec<String>,
    /// The grouping variable (two groups, ANOVA).
    #[serde(default)]
    pub group: Option<String>,
    /// The two group codes (two groups).
    #[serde(default)]
    pub groups: Option<(f64, f64)>,
    #[serde(default = "default_resamples")]
    pub resamples: usize,
    #[serde(default = "default_limit")]
    pub exact_limit: usize,
    #[serde(default = "default_seed")]
    pub seed: u64,
}

fn default_resamples() -> usize {
    10_000
}

fn default_limit() -> usize {
    100_000
}

fn default_seed() -> u64 {
    20_260_101
}

const TOL: f64 = 1e-12;

fn mean(x: &[f64]) -> f64 {
    x.iter().sum::<f64>() / x.len() as f64
}

fn pearson(x: &[f64], y: &[f64]) -> f64 {
    let (mx, my) = (mean(x), mean(y));
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (a, b) in x.iter().zip(y) {
        sxy += (a - mx) * (b - my);
        sxx += (a - mx).powi(2);
        syy += (b - my).powi(2);
    }
    sxy / (sxx * syy).sqrt()
}

/// The one-way F statistic of values with group labels 0..k.
fn f_stat(y: &[f64], g: &[usize], k: usize) -> f64 {
    let n = y.len() as f64;
    let grand = mean(y);
    let mut sums = vec![0.0; k];
    let mut counts = vec![0.0; k];
    for (v, gi) in y.iter().zip(g) {
        sums[*gi] += v;
        counts[*gi] += 1.0;
    }
    let ssb: f64 = sums.iter().zip(&counts).map(|(s, c)| c * (s / c - grand).powi(2)).sum();
    let sst: f64 = y.iter().map(|v| (v - grand).powi(2)).sum();
    (ssb / (k as f64 - 1.0)) / ((sst - ssb) / (n - k as f64))
}

fn ln_choose(n: usize, k: usize) -> f64 {
    prismqda_stats::dist::ln_choose(n as f64, k as f64)
}

/// Shuffles in place (Fisher-Yates).
fn shuffle<T>(v: &mut [T], rng: &mut SplitMix64) {
    for i in (1..v.len()).rev() {
        let j = rng.below(i + 1);
        v.swap(i, j);
    }
}

/// Calls `f` with every subset of size k of 0..n (as a membership mask), in lexicographic order.
fn for_each_subset(n: usize, k: usize, f: &mut dyn FnMut(&[bool])) {
    let mut idx: Vec<usize> = (0..k).collect();
    let mut mask = vec![false; n];
    loop {
        mask.iter_mut().for_each(|m| *m = false);
        for &i in &idx {
            mask[i] = true;
        }
        f(&mask);
        // The rightmost index that can still move right.
        let mut i = k;
        while i > 0 && idx[i - 1] == n - k + i - 1 {
            i -= 1;
        }
        if i == 0 {
            return;
        }
        idx[i - 1] += 1;
        for j in i..k {
            idx[j] = idx[j - 1] + 1;
        }
    }
}

/// The permutation p value and how it was found: (p, number of rearrangements, exact?).
pub struct Outcome {
    pub p: f64,
    pub count: usize,
    pub exact: bool,
}

pub fn two_groups(x1: &[f64], x2: &[f64], resamples: usize, limit: usize, seed: u64) -> Outcome {
    let all: Vec<f64> = x1.iter().chain(x2).copied().collect();
    let (n1, n) = (x1.len(), all.len());
    let total: f64 = all.iter().sum();
    let obs = (mean(x1) - mean(x2)).abs();
    let stat = |s1: f64| (s1 / n1 as f64 - (total - s1) / (n - n1) as f64).abs();
    if ln_choose(n, n1) <= (limit as f64).ln() {
        let (mut hits, mut count) = (0usize, 0usize);
        for_each_subset(n, n1, &mut |mask| {
            let s1: f64 = all.iter().zip(mask).filter(|(_, m)| **m).map(|(v, _)| v).sum();
            if stat(s1) >= obs - TOL * obs.max(1.0) {
                hits += 1;
            }
            count += 1;
        });
        return Outcome { p: hits as f64 / count as f64, count, exact: true };
    }
    let mut rng = SplitMix64::new(seed);
    let mut v = all.clone();
    let mut hits = 0;
    for r in 0..resamples {
        if crate::job::step(r, resamples) {
            break;
        }
        shuffle(&mut v, &mut rng);
        if stat(v[..n1].iter().sum()) >= obs - TOL * obs.max(1.0) {
            hits += 1;
        }
    }
    Outcome { p: (hits + 1) as f64 / (resamples + 1) as f64, count: resamples, exact: false }
}

pub fn paired(d: &[f64], resamples: usize, limit: usize, seed: u64) -> Outcome {
    let n = d.len();
    let obs = d.iter().sum::<f64>().abs();
    if (n as f64) * 2f64.ln() <= (limit as f64).ln() {
        let mut hits = 0usize;
        let count = 1usize << n;
        for m in 0..count {
            if m % 4096 == 0 && crate::job::step(m, count) {
                break;
            }
            let s: f64 = d.iter().enumerate().map(|(i, v)| if m >> i & 1 == 1 { -v } else { *v }).sum();
            if s.abs() >= obs - TOL * obs.max(1.0) {
                hits += 1;
            }
        }
        return Outcome { p: hits as f64 / count as f64, count, exact: true };
    }
    let mut rng = SplitMix64::new(seed);
    let mut hits = 0;
    for r in 0..resamples {
        if crate::job::step(r, resamples) {
            break;
        }
        let s: f64 = d.iter().map(|v| if rng.next_u64() & 1 == 1 { -v } else { *v }).sum();
        if s.abs() >= obs - TOL * obs.max(1.0) {
            hits += 1;
        }
    }
    Outcome { p: (hits + 1) as f64 / (resamples + 1) as f64, count: resamples, exact: false }
}

pub fn correlation(x: &[f64], y: &[f64], resamples: usize, limit: usize, seed: u64) -> Outcome {
    let n = x.len();
    let obs = pearson(x, y).abs();
    let ln_fact: f64 = (1..=n).map(|i| (i as f64).ln()).sum();
    if ln_fact <= (limit as f64).ln() {
        // Every ordering of y (Heap's algorithm).
        let mut v = y.to_vec();
        let mut c = vec![0usize; n];
        let (mut hits, mut count) = (0usize, 0usize);
        let mut check = |v: &[f64]| {
            if pearson(x, v).abs() >= obs - TOL {
                hits += 1;
            }
            count += 1;
        };
        check(&v);
        let mut i = 0;
        while i < n {
            if c[i] < i {
                if i % 2 == 0 { v.swap(0, i) } else { v.swap(c[i], i) }
                check(&v);
                c[i] += 1;
                i = 0;
            } else {
                c[i] = 0;
                i += 1;
            }
        }
        return Outcome { p: hits as f64 / count as f64, count, exact: true };
    }
    let mut rng = SplitMix64::new(seed);
    let mut v = y.to_vec();
    let mut hits = 0;
    for r in 0..resamples {
        if crate::job::step(r, resamples) {
            break;
        }
        shuffle(&mut v, &mut rng);
        if pearson(x, &v).abs() >= obs - TOL {
            hits += 1;
        }
    }
    Outcome { p: (hits + 1) as f64 / (resamples + 1) as f64, count: resamples, exact: false }
}

pub fn anova(y: &[f64], g: &[usize], k: usize, resamples: usize, seed: u64) -> Outcome {
    let obs = f_stat(y, g, k);
    let mut rng = SplitMix64::new(seed);
    let mut labels = g.to_vec();
    let mut hits = 0;
    for r in 0..resamples {
        if crate::job::step(r, resamples) {
            break;
        }
        shuffle(&mut labels, &mut rng);
        if f_stat(y, &labels, k) >= obs - TOL * obs.max(1.0) {
            hits += 1;
        }
    }
    Outcome { p: (hits + 1) as f64 / (resamples + 1) as f64, count: resamples, exact: false }
}

/// What each test hands back: title, statistic name and value, outcome, cases used and total, the tally, extra rows.
type Prepared<'a> = (String, &'a str, f64, Outcome, usize, usize, Tally, Vec<(String, f64)>);

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    if !(100..=1_000_000).contains(&spec.resamples) {
        return Err(Error::Invalid("The number of random rearrangements must be between 100 and 1,000,000.".into()));
    }
    let need = match spec.test {
        Kind::Paired | Kind::Correlation => 2,
        _ => 1,
    };
    if spec.variables.len() != need {
        return Err(Error::Invalid(format!("This test needs {need} variable{}.", if need == 1 { "" } else { "s" })));
    }
    let label = |n: &str| ds.variable(n).map(|v| v.display_name().to_string());
    let how = |o: &Outcome| if o.exact { format!("exact, over all {} rearrangements", o.count) } else { format!("Monte Carlo, {} random rearrangements (seed {}); p = (count + 1) / ({} + 1)", o.count, spec.seed, o.count) };
    let (title, stat_name, stat, o, n, total, tally, extra): Prepared = match spec.test {
        Kind::TwoGroups | Kind::Anova => {
            let gname = spec.group.as_ref().ok_or_else(|| Error::Invalid("Choose the grouping variable.".into()))?;
            let yname = &spec.variables[0];
            let (ylab, glab) = (label(yname)?, label(gname)?);
            if spec.test == Kind::TwoGroups {
                let (c1, c2) = spec.groups.ok_or_else(|| Error::Invalid("Choose the two groups' codes.".into()))?;
                let (ys, gs) = (numeric(ds, yname)?, numeric(ds, gname)?);
                let mut tally = Tally::default();
                let (mut x1, mut x2) = (Vec::new(), Vec::new());
                for (y, g) in ys.iter().zip(&gs) {
                    if tally.check(g, "grouping variable") || tally.check(y, "outcome") {
                        continue;
                    }
                    let gv = g.value().expect("checked");
                    if gv == c1 { x1.push(y.value().expect("checked")) } else if gv == c2 { x2.push(y.value().expect("checked")) } else { tally.add("in neither group") }
                }
                if x1.len() < 2 || x2.len() < 2 {
                    return Err(Error::Invalid("Each group needs at least two values.".into()));
                }
                let diff = mean(&x1) - mean(&x2);
                let gv = ds.variable(gname)?;
                let o = two_groups(&x1, &x2, spec.resamples, spec.exact_limit, spec.seed);
                (format!("Permutation Test: {ylab} by {glab}"), "Difference in means", diff, o, x1.len() + x2.len(), ys.len(), tally, vec![(format!("Mean, {} ({})", gv.value_text(c1), format_code(c1)), mean(&x1)), (format!("Mean, {} ({})", gv.value_text(c2), format_code(c2)), mean(&x2))])
            } else {
                let super::anova_oneway::Split { groups, tally, total, .. } = super::anova_oneway::split(ds, yname, gname)?;
                if groups.len() < 2 {
                    return Err(Error::Invalid("The test needs at least two groups with data.".into()));
                }
                let y: Vec<f64> = groups.iter().flatten().copied().collect();
                let g: Vec<usize> = groups.iter().enumerate().flat_map(|(i, v)| std::iter::repeat_n(i, v.len())).collect();
                let f = f_stat(&y, &g, groups.len());
                let o = anova(&y, &g, groups.len(), spec.resamples, spec.seed);
                (format!("Permutation ANOVA: {ylab} by {glab}"), "F", f, o, y.len(), total, tally, Vec::new())
            }
        }
        Kind::Paired | Kind::Correlation => {
            let (a, c) = (&spec.variables[0], &spec.variables[1]);
            let (la, lc) = (label(a)?, label(c)?);
            let (x, y, tally, total) = super::ttest_paired::matched(ds, a, c)?;
            if x.len() < 3 {
                return Err(Error::Invalid("The test needs at least three cases with both values.".into()));
            }
            if spec.test == Kind::Paired {
                let d: Vec<f64> = x.iter().zip(&y).map(|(u, v)| u - v).collect();
                let o = paired(&d, spec.resamples, spec.exact_limit, spec.seed);
                (format!("Paired Permutation Test: {la} and {lc}"), "Mean difference", mean(&d), o, d.len(), total, tally, Vec::new())
            } else {
                let o = correlation(&x, &y, spec.resamples, spec.exact_limit, spec.seed);
                (format!("Permutation Test of Correlation: {la} and {lc}"), "Pearson r", pearson(&x, &y), o, x.len(), total, tally, Vec::new())
            }
        }
    };
    let note = format!("Permutation test, two-sided: the p value is the share of rearrangements of the data whose statistic ({}) is at least as far from the null as the one observed ({}). It assumes only that, if there is no effect, the rearranged data were as likely as those observed (exchangeability). Computed by PrismStatistics.", stat_name.to_lowercase(), how(&o));
    let mut b = common::bundle(METHOD_ID, METHOD_VERSION, title, serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, n), note);
    set_text(&mut b, "test", serde_json::to_value(spec.test).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default());
    set_text(&mut b, "statistic.name", stat_name);
    set(&mut b, "statistic", stat);
    set(&mut b, "p", o.p);
    set(&mut b, "rearrangements", o.count as f64);
    set(&mut b, "exact", if o.exact { 1.0 } else { 0.0 });
    set(&mut b, "n", n as f64);
    let mut rows = vec![vec![text(stat_name), num(stat, 4)]];
    for (l, v) in &extra {
        rows.push(vec![text(l), num(*v, 4)]);
    }
    rows.push(vec![text("p (two-sided)"), p(o.p)]);
    rows.push(vec![text(if o.exact { "Rearrangements (all)" } else { "Random rearrangements" }), count(o.count)]);
    b.tables.push(Table { id: "test".into(), title: "Permutation test".into(), columns: vec!["".into(), "Value".into()], rows, footnotes: vec![if o.exact { "The p value is exact.".into() } else { format!("The Monte Carlo p value varies a little with the seed; with {} rearrangements its standard error is about {}.", o.count, apa::num((o.p * (1.0 - o.p) / o.count as f64).sqrt(), 4)) }] });
    Ok(b)
}
