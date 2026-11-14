//! One-sided tests (checklist A10). `prismqda_stats` gives two-sided p values; this module gives the one-sided ones the
//! way R's `t.test`, `cor.test` and `wilcox.test` compute them, including the exact distributions of the rank tests.

use prismqda_stats::dist::{norm_cdf, norm_quantile, norm_sf, t_cdf, t_quantile, t_sf};
use prismqda_stats::rank::{rank_average, tie_sizes};
use serde::{Deserialize, Serialize};

/// The alternative hypothesis. `Greater` means the estimate (the first group or measure minus the second, or the
/// correlation) is above its null value; `Less` that it is below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Alternative {
    #[default]
    TwoSided,
    Greater,
    Less,
}

impl Alternative {
    pub fn is_two_sided(self) -> bool {
        self == Alternative::TwoSided
    }

    /// "two-sided", "one-sided, greater" or "one-sided, less".
    pub fn words(self) -> &'static str {
        match self {
            Alternative::TwoSided => "two-sided",
            Alternative::Greater => "one-sided (greater)",
            Alternative::Less => "one-sided (less)",
        }
    }

    /// The key written to results: "two_sided", "greater" or "less".
    pub fn key(self) -> &'static str {
        match self {
            Alternative::TwoSided => "two_sided",
            Alternative::Greater => "greater",
            Alternative::Less => "less",
        }
    }
}

/// The p value of a t statistic for `alt`.
pub fn t_p(t: f64, df: f64, two_sided: f64, alt: Alternative) -> f64 {
    match alt {
        Alternative::TwoSided => two_sided,
        Alternative::Greater => t_sf(t, df),
        Alternative::Less => t_cdf(t, df),
    }
}

/// The p value of a standard normal statistic for `alt`.
pub fn z_p(z: f64, two_sided: f64, alt: Alternative) -> f64 {
    match alt {
        Alternative::TwoSided => two_sided,
        Alternative::Greater => norm_sf(z),
        Alternative::Less => norm_cdf(z),
    }
}

/// The interval of an estimate with a t distribution: two-sided, or open on one side (R's `t.test`).
pub fn t_interval(estimate: f64, se: f64, df: f64, level: f64, alt: Alternative) -> (f64, f64) {
    match alt {
        Alternative::TwoSided => {
            let q = t_quantile(1.0 - (1.0 - level) / 2.0, df);
            (estimate - q * se, estimate + q * se)
        }
        Alternative::Greater => (estimate - t_quantile(level, df) * se, f64::INFINITY),
        Alternative::Less => (f64::NEG_INFINITY, estimate + t_quantile(level, df) * se),
    }
}

/// A t test made one-sided: its p value and the interval of its estimate (open on one side). The effect-size intervals
/// stay two-sided.
pub fn apply_t(t: &mut prismqda_stats::ttest::TTest, alt: Alternative, level: f64) {
    if alt.is_two_sided() {
        return;
    }
    t.p = t_p(t.t, t.df, t.p, alt);
    t.ci = t_interval(t.estimate, t.se, t.df, level, alt);
}

/// A table's footnotes with the hypothesis of a one-sided test spelled out: `greater` and `less` complete "The test
/// asks whether …".
pub fn footnotes(alt: Alternative, first: &str, greater: &str, less: &str) -> Vec<String> {
    let mut out = vec![first.to_string()];
    match alt {
        Alternative::TwoSided => {}
        Alternative::Greater => out.push(format!("One-sided test: it asks whether {greater}. The interval is one-sided; effect-size intervals stay two-sided.")),
        Alternative::Less => out.push(format!("One-sided test: it asks whether {less}. The interval is one-sided; effect-size intervals stay two-sided.")),
    }
    out
}

/// Pearson's r: the one-sided Fisher-z interval (R's `cor.test`).
pub fn pearson_interval(r: f64, n: usize, level: f64, alt: Alternative) -> Option<(f64, f64)> {
    if n <= 3 || r.abs() >= 1.0 {
        return None;
    }
    let z = r.atanh();
    let se = 1.0 / (n as f64 - 3.0).sqrt();
    Some(match alt {
        Alternative::TwoSided => {
            let h = norm_quantile(1.0 - (1.0 - level) / 2.0) * se;
            ((z - h).tanh(), (z + h).tanh())
        }
        Alternative::Greater => ((z - norm_quantile(level) * se).tanh(), 1.0),
        Alternative::Less => (-1.0, (z + norm_quantile(level) * se).tanh()),
    })
}

/// The tail of S, the sum of squared rank differences of n untied pairs (R's `prho`, AS 89): P(S < is) when
/// `lower_tail`, otherwise P(S >= is). Exact up to 9 pairs, the Edgeworth series above.
fn prho(n: usize, is: f64, lower_tail: bool) -> f64 {
    let nf = n as f64;
    let n3 = nf * (nf * nf - 1.0) / 3.0;
    if is <= 0.0 {
        return if lower_tail { 0.0 } else { 1.0 };
    }
    if is > n3 {
        return if lower_tail { 1.0 } else { 0.0 };
    }
    if n <= 9 {
        let mut perm: Vec<usize> = (1..=n).collect();
        let (mut total, mut at_least) = (0u64, 0u64);
        let mut count = |p: &[usize]| {
            total += 1;
            let ise: usize = p.iter().enumerate().map(|(i, &v)| (i + 1).abs_diff(v).pow(2)).sum();
            if is <= ise as f64 {
                at_least += 1;
            }
        };
        let mut c = vec![0usize; n];
        count(&perm);
        let mut i = 0;
        while i < n {
            if c[i] < i {
                if i % 2 == 0 {
                    perm.swap(0, i);
                } else {
                    perm.swap(c[i], i);
                }
                count(&perm);
                c[i] += 1;
                i = 0;
            } else {
                c[i] = 0;
                i += 1;
            }
        }
        let all = total as f64;
        return if lower_tail { (all - at_least as f64) / all } else { at_least as f64 / all };
    }
    const C: [f64; 12] = [0.2274, 0.2531, 0.1745, 0.0758, 0.1033, 0.3932, 0.0879, 0.0151, 0.0072, 0.0831, 0.0131, 4.6e-4];
    let b = 1.0 / nf;
    let x = (6.0 * (is - 1.0) * b / (nf * nf - 1.0) - 1.0) * (1.0 / b - 1.0).sqrt();
    let mut y = x * x;
    let u = x * b * (C[0] + b * (C[1] + C[2] * b) + y * (-C[3] + b * (C[4] + C[5] * b) - y * b * (C[6] + C[7] * b - y * (C[8] - C[9] * b + y * b * (C[10] - C[11] * y)))));
    y = u / (y / 2.0).exp();
    let tail = if lower_tail { norm_cdf(x) } else { norm_sf(x) };
    (if lower_tail { -y } else { y } + tail).clamp(0.0, 1.0)
}

/// Spearman's rho without ties (the exact or Edgeworth test): the one-sided p value, as R's `cor.test`.
pub fn spearman_exact_p(n: usize, rho: f64, alt: Alternative) -> f64 {
    let nf = n as f64;
    let q = ((nf * nf * nf - nf) * (1.0 - rho) / 6.0).round();
    match alt {
        // A positive correlation is a small S.
        Alternative::Greater => prho(n, q + 2.0, true),
        // R uses q here (and q - 1 in its two-sided upper tail).
        Alternative::Less => prho(n, q, false),
        Alternative::TwoSided => {
            let p = if q > (nf * nf * nf - nf) / 6.0 { prho(n, q - 1.0, false) } else { prho(n, q + 2.0, true) };
            (2.0 * p).min(1.0)
        }
    }
}

/// P(at most q inversions) in a random ordering of n (Kendall's exact null distribution).
fn kendall_cdf(q: i64, n: usize) -> f64 {
    if q < 0 {
        return 0.0;
    }
    let max = n * (n - 1) / 2;
    let mut prob = vec![0.0; max + 1];
    prob[0] = 1.0;
    for i in 2..=n {
        let mut next = vec![0.0; max + 1];
        let mut window = 0.0;
        for (k, slot) in next.iter_mut().enumerate().take(i * (i - 1) / 2 + 1) {
            window += prob[k];
            if k >= i {
                window -= prob[k - i];
            }
            *slot = window / i as f64;
        }
        prob = next;
    }
    prob.iter().take(q.min(max as i64) as usize + 1).sum::<f64>().min(1.0)
}

/// Kendall's tau without ties and under 50 pairs (the exact test): the one-sided p value, as R's `cor.test`.
pub fn kendall_exact_p(n: usize, tau: f64, alt: Alternative) -> f64 {
    let nf = n as f64;
    let q = ((tau + 1.0) * nf * (nf - 1.0) / 4.0).round() as i64;
    match alt {
        Alternative::Greater => 1.0 - kendall_cdf(q - 1, n),
        Alternative::Less => kendall_cdf(q, n),
        Alternative::TwoSided => {
            let p = if q as f64 > nf * (nf - 1.0) / 4.0 { 1.0 - kendall_cdf(q - 1, n) } else { kendall_cdf(q, n) };
            (2.0 * p).min(1.0)
        }
    }
}

/// The continuity correction R's `wilcox.test` applies for each alternative.
fn correction(centred: f64, alt: Alternative) -> f64 {
    match alt {
        Alternative::TwoSided => 0.5 * if centred == 0.0 { 0.0 } else { centred.signum() },
        Alternative::Greater => 0.5,
        Alternative::Less => -0.5,
    }
}

/// The Mann-Whitney test of x against y for `alt` ("greater": x tends to be larger): (p, z), with z None for the exact
/// test. The same choice of exact or normal test as `prismqda_stats::nonparametric::mann_whitney`.
pub fn mann_whitney_p(x: &[f64], y: &[f64], alt: Alternative) -> Option<(f64, Option<f64>)> {
    let (n1, n2) = (x.len(), y.len());
    if n1 == 0 || n2 == 0 {
        return None;
    }
    let all: Vec<f64> = x.iter().chain(y).copied().collect();
    let ranks = rank_average(&all);
    let (n1f, n2f) = (n1 as f64, n2 as f64);
    let u = ranks[..n1].iter().sum::<f64>() - n1f * (n1f + 1.0) / 2.0;
    let ties = tie_sizes(&all);
    if n1 < 50 && n2 < 50 && ties.is_empty() {
        // The Gaussian binomial generating function of U.
        let len = n1 * n2 + 1;
        let mut poly = vec![0i128; len];
        poly[0] = 1;
        for i in 1..=n1 {
            for k in (n2 + i..len).rev() {
                poly[k] -= poly[k - (n2 + i)];
            }
            for k in i..len {
                poly[k] += poly[k - i];
            }
        }
        let total: f64 = poly.iter().map(|&c| c as f64).sum();
        let dist: Vec<f64> = poly.into_iter().map(|c| c as f64 / total).collect();
        let at_most = |k: f64| -> f64 { if k < 0.0 { 0.0 } else { dist.iter().take(k as usize + 1).sum::<f64>().min(1.0) } };
        let p = match alt {
            Alternative::Greater => 1.0 - at_most(u - 1.0),
            Alternative::Less => at_most(u),
            Alternative::TwoSided => (2.0 * if u > n1f * n2f / 2.0 { 1.0 - at_most(u - 1.0) } else { at_most(u) }).min(1.0),
        };
        return Some((p, None));
    }
    let big_n = n1f + n2f;
    let tie_term: f64 = ties.iter().map(|&t| (t as f64).powi(3) - t as f64).sum();
    let sigma = (n1f * n2f / 12.0 * ((big_n + 1.0) - tie_term / (big_n * (big_n - 1.0)))).sqrt();
    if sigma == 0.0 {
        return None;
    }
    let centred = u - n1f * n2f / 2.0;
    let z = (centred - correction(centred, alt)) / sigma;
    Some((z_p(z, (2.0 * norm_cdf(z).min(norm_cdf(-z))).min(1.0), alt), Some(z)))
}

/// The Wilcoxon signed-rank test of the differences for `alt` ("greater": the differences tend to be positive): (p, z),
/// z None for the exact test. Zero differences are set aside, as `prismqda_stats::nonparametric::wilcoxon` does.
pub fn wilcoxon_p(diffs: &[f64], alt: Alternative) -> Option<(f64, Option<f64>)> {
    let zeros = diffs.iter().filter(|&&d| d == 0.0).count();
    let nonzero: Vec<f64> = diffs.iter().copied().filter(|&d| d != 0.0).collect();
    let n = nonzero.len();
    if n == 0 {
        return None;
    }
    let abs: Vec<f64> = nonzero.iter().map(|d| d.abs()).collect();
    let ranks = rank_average(&abs);
    let v: f64 = nonzero.iter().zip(&ranks).filter(|(d, _)| **d > 0.0).map(|(_, r)| r).sum();
    let nf = n as f64;
    let total = nf * (nf + 1.0) / 2.0;
    let ties = tie_sizes(&abs);
    if n < 50 && ties.is_empty() && zeros == 0 {
        let mut counts = vec![0u128; total as usize + 1];
        counts[0] = 1;
        for i in 1..=n {
            for s in (i..counts.len()).rev() {
                counts[s] += counts[s - i];
            }
        }
        let all = 2f64.powi(n as i32);
        let at_most = |k: f64| -> f64 { if k < 0.0 { 0.0 } else { counts.iter().take(k as usize + 1).map(|&c| c as f64).sum::<f64>() / all } };
        let p = match alt {
            Alternative::Greater => 1.0 - at_most(v - 1.0),
            Alternative::Less => at_most(v),
            Alternative::TwoSided => (2.0 * if v > total / 2.0 { 1.0 - at_most(v - 1.0) } else { at_most(v) }).min(1.0),
        };
        return Some((p, None));
    }
    let tie_term: f64 = ties.iter().map(|&t| (t as f64).powi(3) - t as f64).sum();
    let sigma = (nf * (nf + 1.0) * (2.0 * nf + 1.0) / 24.0 - tie_term / 48.0).sqrt();
    if sigma == 0.0 {
        return None;
    }
    let centred = v - total / 2.0;
    let z = (centred - correction(centred, alt)) / sigma;
    Some((z_p(z, (2.0 * norm_cdf(z).min(norm_cdf(-z))).min(1.0), alt), Some(z)))
}
