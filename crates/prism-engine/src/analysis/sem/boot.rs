//! Bootstrap standard errors and intervals (checklist C1k), as lavaan's se = "bootstrap": cases are drawn with
//! replacement within each group from a seeded generator, the model is refitted to each resample (starting from the
//! estimates), and every parameter, including defined ones such as indirect effects, gets the standard deviation of
//! its resampled values as its standard error and a percentile or bias-corrected percentile interval. Resamples whose
//! fit fails or does not converge are dropped and counted.

use prismqda_stats::dist::{norm_cdf, norm_quantile};
use prismqda_stats::rng::SplitMix64;

use super::estimate::{self, Block};
use super::model::Model;
use crate::analysis::linalg::Mat;

pub struct Boot {
    /// Each kept resample's free parameters.
    pub thetas: Vec<Vec<f64>>,
    /// Each kept resample's parameter values (every parameter, then the defined ones).
    pub values: Vec<Vec<f64>>,
    pub failed: usize,
}

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

/// Resamples and refits. `model` holds the estimates (the starting values of every refit); `values` maps free
/// parameters to the reported values.
pub fn resample(model: &Model, blocks: &[Block], resamples: usize, seed: u64, values: &dyn Fn(&[f64]) -> Vec<f64>) -> Boot {
    // Every case: its group, the variables it has, and its values.
    let mut cases: Vec<Vec<(usize, usize)>> = vec![Vec::new(); model.groups];
    for (bi, b) in blocks.iter().enumerate() {
        for ri in 0..b.rows.len() {
            cases[b.group].push((bi, ri));
        }
    }
    let mut rng = SplitMix64::new(seed);
    let mut out = Boot { thetas: Vec::new(), values: Vec::new(), failed: 0 };
    for r in 0..resamples {
        if crate::job::step(r, resamples) {
            break;
        }
        let mut groups: Vec<(usize, Vec<usize>, Vec<Vec<f64>>)> = Vec::new();
        for (g, list) in cases.iter().enumerate() {
            for _ in 0..list.len() {
                let (bi, ri) = list[rng.below(list.len())];
                let b = &blocks[bi];
                match groups.iter_mut().find(|(h, vars, _)| *h == g && *vars == b.vars) {
                    Some((_, _, rows)) => rows.push(b.rows[ri].clone()),
                    None => groups.push((g, b.vars.clone(), vec![b.rows[ri].clone()])),
                }
            }
        }
        let rb: Vec<Block> = groups
            .into_iter()
            .map(|(g, vars, rows)| {
                let (cov, mean) = moments(&rows);
                Block { group: g, vars, n: rows.len() as f64, cov, mean, rows }
            })
            .collect();
        // Fixed predictors take this resample's own sample moments, as they would in a fit to these cases.
        let mut m = model.clone();
        for g in 0..model.groups {
            let mut xs: Vec<Vec<f64>> = Vec::new();
            for b in rb.iter().filter(|b| b.group == g) {
                let pos: Vec<usize> = model.xvars.iter().map(|x| b.vars.iter().position(|v| v == x).expect("fixed predictors are observed")).collect();
                xs.extend(b.rows.iter().map(|r| pos.iter().map(|&k| r[k]).collect::<Vec<f64>>()));
            }
            if xs.is_empty() || model.xvars.is_empty() {
                continue;
            }
            let (cov, mean) = moments(&xs);
            let at = |v: usize| model.xvars.iter().position(|x| *x == v).expect("a fixed predictor");
            for q in m.params.iter_mut().filter(|q| q.group == g && q.exo) {
                q.value = match q.mat {
                    super::model::Matrix::S => cov[at(q.i)][at(q.j)],
                    _ => mean[at(q.i)],
                };
            }
        }
        match estimate::fit(&m, &rb) {
            Ok(f) if f.converged => {
                out.values.push(values(&f.theta));
                out.thetas.push(f.theta);
            }
            _ => out.failed += 1,
        }
    }
    out
}

/// The value at a position among sorted values, as R's boot:::norm.inter finds it (the convention of the other
/// bootstrap intervals in PrismStatistics).
fn norm_inter(sorted: &[f64], alpha: f64) -> f64 {
    let r = sorted.len();
    let rk = (r as f64 + 1.0) * alpha;
    let k = rk.trunc().max(0.0) as usize;
    if k == 0 {
        return sorted[0];
    }
    if k >= r {
        return sorted[r - 1];
    }
    if (rk - k as f64).abs() < 1e-12 {
        return sorted[k - 1];
    }
    let (t1, t2) = (norm_quantile(alpha), norm_quantile(k as f64 / (r as f64 + 1.0)));
    let t3 = norm_quantile((k as f64 + 1.0) / (r as f64 + 1.0));
    sorted[k - 1] + (t1 - t2) / (t3 - t2) * (sorted[k] - sorted[k - 1])
}

/// The standard deviation of resampled values, and their percentile or bias-corrected percentile interval.
pub fn summarize(estimate: f64, reps: &[f64], level: f64, bias_corrected: bool) -> Option<(f64, f64, f64)> {
    let mut s: Vec<f64> = reps.iter().copied().filter(|v| v.is_finite()).collect();
    if s.len() < 20 {
        return None;
    }
    s.sort_by(f64::total_cmp);
    let n = s.len() as f64;
    let mean = s.iter().sum::<f64>() / n;
    let sd = (s.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt();
    let (lo, hi) = ((1.0 - level) / 2.0, (1.0 + level) / 2.0);
    let (a1, a2) = if bias_corrected {
        let below = s.iter().filter(|v| **v < estimate).count() as f64 / n;
        if below <= 0.0 || below >= 1.0 {
            return Some((sd, f64::NAN, f64::NAN));
        }
        let z0 = norm_quantile(below);
        (norm_cdf(2.0 * z0 + norm_quantile(lo)), norm_cdf(2.0 * z0 + norm_quantile(hi)))
    } else {
        (lo, hi)
    };
    Some((sd, norm_inter(&s, a1), norm_inter(&s, a2)))
}

/// The covariance of the resampled free parameters (divisor R − 1).
pub fn covariance(thetas: &[Vec<f64>]) -> Mat {
    let r = thetas.len() as f64;
    let q = thetas.first().map_or(0, Vec::len);
    let mean: Vec<f64> = (0..q).map(|k| thetas.iter().map(|t| t[k]).sum::<f64>() / r).collect();
    let mut v = vec![vec![0.0; q]; q];
    for t in thetas {
        for a in 0..q {
            for c in 0..q {
                v[a][c] += (t[a] - mean[a]) * (t[c] - mean[c]) / (r - 1.0);
            }
        }
    }
    v
}
