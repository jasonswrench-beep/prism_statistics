//! Missing data (checklist B8).
//!
//! * Missing-data patterns (`missing.patterns`): how often each variable is missing, the patterns of missingness and their
//!   counts, and Little's MCAR test, from the maximum likelihood means and covariances of the numeric variables found by the
//!   EM algorithm (the statistic of R's naniar::mcar_test, with EM run to convergence).
//! * Multiple imputation for linear regression (`missing.mi_regression`): each variable with missing values is imputed
//!   by chained equations with Bayesian linear-regression draws (mice's "norm" method), m times from a seeded generator;
//!   the regression is fitted to each completed dataset and the results pooled by Rubin's rules with Barnard and Rubin's
//!   degrees of freedom, as mice::pool does.

use prismqda_stats::dist::{chisq_sf, t_quantile, t_two_sided};
use prismqda_stats::rng::SplitMix64;
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use super::linalg::{cholesky, invert, Mat};
use crate::apa;
use crate::error::{Error, Result};
use crate::model::{Column, Dataset};
use crate::result::{CaseAccounting, ResultBundle, Table};

pub const PATTERNS: &str = "missing.patterns";
pub const MI_REGRESSION: &str = "missing.mi_regression";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatternsSpec {
    pub variables: Vec<String>,
}

/// A cell as present or missing (empty or a user-missing code), with its value when present.
fn values(ds: &Dataset, name: &str) -> Result<Vec<Option<f64>>> {
    let i = ds.find(name).ok_or_else(|| Error::UnknownVariable(name.to_string()))?;
    Ok(match &ds.columns[i] {
        Column::Numeric(_) => numeric(ds, name)?.iter().map(Slot::value).collect(),
        Column::Text(v) => v.iter().map(|t| t.as_ref().map(|_| 0.0)).collect(),
    })
}

/// Maximum likelihood mean and covariance of multivariate normal data with missing values, by EM.
pub fn em(rows: &[Vec<Option<f64>>], p: usize) -> Option<(Vec<f64>, Mat)> {
    let n = rows.len() as f64;
    let mut mu: Vec<f64> = (0..p).map(|j| { let v: Vec<f64> = rows.iter().filter_map(|r| r[j]).collect(); v.iter().sum::<f64>() / v.len() as f64 }).collect();
    let mut sigma: Mat = (0..p).map(|a| (0..p).map(|c| if a == c { let v: Vec<f64> = rows.iter().filter_map(|r| r[a]).collect(); v.iter().map(|x| (x - mu[a]).powi(2)).sum::<f64>() / v.len() as f64 } else { 0.0 }).collect()).collect();
    for _ in 0..10000 {
        let mut t1 = vec![0.0; p];
        let mut t2 = vec![vec![0.0; p]; p];
        for r in rows {
            let o: Vec<usize> = (0..p).filter(|&j| r[j].is_some()).collect();
            let m: Vec<usize> = (0..p).filter(|&j| r[j].is_none()).collect();
            let mut x: Vec<f64> = r.iter().map(|v| v.unwrap_or(0.0)).collect();
            let mut extra = vec![vec![0.0; p]; p];
            if !m.is_empty() {
                let soo: Mat = o.iter().map(|&a| o.iter().map(|&c| sigma[a][c]).collect()).collect();
                let inv = if o.is_empty() { Vec::new() } else { invert(&soo)? };
                // Regression of the missing on the observed: B = S_mo S_oo⁻¹.
                let bm: Mat = m.iter().map(|&a| (0..o.len()).map(|k| o.iter().enumerate().map(|(l, &c)| sigma[a][c] * inv[l][k]).sum()).collect()).collect();
                for (mi, &a) in m.iter().enumerate() {
                    x[a] = mu[a] + o.iter().enumerate().map(|(k, &c)| bm[mi][k] * (r[c].expect("observed") - mu[c])).sum::<f64>();
                }
                for (mi, &a) in m.iter().enumerate() {
                    for &c in &m {
                        extra[a][c] = sigma[a][c] - o.iter().enumerate().map(|(k, &oc)| bm[mi][k] * sigma[oc][c]).sum::<f64>();
                    }
                }
            }
            for a in 0..p {
                t1[a] += x[a];
                for c in 0..p {
                    t2[a][c] += x[a] * x[c] + extra[a][c];
                }
            }
        }
        let new_mu: Vec<f64> = t1.iter().map(|v| v / n).collect();
        let new_sigma: Mat = (0..p).map(|a| (0..p).map(|c| t2[a][c] / n - new_mu[a] * new_mu[c]).collect()).collect();
        let change = new_mu.iter().zip(&mu).map(|(a, b)| (a - b).abs()).chain(new_sigma.iter().flatten().zip(sigma.iter().flatten()).map(|(a, b)| (a - b).abs())).fold(0.0, f64::max);
        mu = new_mu;
        sigma = new_sigma;
        if change < 1e-12 {
            break;
        }
    }
    Some((mu, sigma))
}

pub fn patterns(ds: &Dataset, spec: &PatternsSpec) -> Result<ResultBundle> {
    let p = spec.variables.len();
    if p == 0 {
        return Err(Error::Invalid("Choose at least one variable.".into()));
    }
    let labels: Vec<String> = spec.variables.iter().map(|n| ds.variable(n).map(|v| v.display_name().to_string())).collect::<Result<_>>()?;
    let cols: Vec<Vec<Option<f64>>> = spec.variables.iter().map(|n| values(ds, n)).collect::<Result<_>>()?;
    let n = cols[0].len();
    let rows: Vec<Vec<Option<f64>>> = (0..n).map(|r| cols.iter().map(|c| c[r]).collect()).collect();
    let note = "Missing values are empty cells and user-missing codes. Little's MCAR test uses the numeric variables' maximum likelihood means and covariances from the EM algorithm; it tests whether the means of the observed values differ across the missingness patterns more than chance allows. Computed by PrismStatistics.".to_string();
    let mut b = common::bundle(PATTERNS, METHOD_VERSION, format!("Missing Data Patterns: {}", labels.join(", ")), serde_json::to_value(spec).expect("a spec always serializes"), CaseAccounting { total: n, analyzed: n, excluded: Vec::new() }, note);
    set(&mut b, "n", n as f64);
    set(&mut b, "p", p as f64);
    let mut rows_v = Vec::new();
    for (j, (l, c)) in labels.iter().zip(&cols).enumerate() {
        let miss = c.iter().filter(|v| v.is_none()).count();
        set_text(&mut b, format!("var.{}.label", j + 1), l.clone());
        set(&mut b, format!("var.{}.missing", j + 1), miss as f64);
        rows_v.push(vec![text(l), count(n - miss), count(miss), num(100.0 * miss as f64 / n as f64, 1)]);
    }
    let complete = rows.iter().filter(|r| r.iter().all(Option::is_some)).count();
    set(&mut b, "complete", complete as f64);
    b.tables.push(Table { id: "variables".into(), title: "Missing values by variable".into(), columns: vec!["Variable".into(), "Valid".into(), "Missing".into(), "% missing".into()], rows: rows_v, footnotes: vec![format!("{complete} of {n} cases ({}%) have every variable.", apa::num(100.0 * complete as f64 / n as f64, 1))] });
    // Patterns, most common first.
    let mut pats: Vec<(Vec<bool>, usize)> = Vec::new();
    for r in &rows {
        let key: Vec<bool> = r.iter().map(Option::is_some).collect();
        match pats.iter_mut().find(|(k, _)| *k == key) {
            Some((_, c)) => *c += 1,
            None => pats.push((key, 1)),
        }
    }
    pats.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.0.iter().filter(|x| **x).count().cmp(&a.0.iter().filter(|x| **x).count())));
    set(&mut b, "patterns", pats.len() as f64);
    let mut pcols = vec!["Cases".into(), "%".into()];
    pcols.extend(labels.iter().cloned());
    pcols.push("Missing".into());
    b.tables.push(Table {
        id: "patterns".into(),
        title: "Missing-data patterns".into(),
        columns: pcols,
        rows: pats.iter().map(|(k, c)| { let mut row = vec![count(*c), num(100.0 * *c as f64 / n as f64, 1)]; row.extend(k.iter().map(|o| text(if *o { "✓" } else { "missing" }))); row.push(count(k.iter().filter(|x| !**x).count())); row }).collect(),
        footnotes: vec!["✓ means the variable has a value in that pattern.".into()],
    });
    // Little's test on the numeric variables.
    let numeric_idx: Vec<usize> = (0..p).filter(|&j| matches!(ds.columns[ds.find(&spec.variables[j]).expect("found")], Column::Numeric(_))).collect();
    let q = numeric_idx.len();
    let lrows: Vec<Vec<Option<f64>>> = rows.iter().map(|r| numeric_idx.iter().map(|&j| r[j]).collect::<Vec<_>>()).filter(|r: &Vec<Option<f64>>| r.iter().any(Option::is_some)).collect();
    if q >= 2 && lrows.len() > q {
        if let Some((mu, sigma)) = em(&lrows, q) {
            let mut groups: Vec<(Vec<usize>, Vec<Vec<f64>>)> = Vec::new();
            for r in &lrows {
                let o: Vec<usize> = (0..q).filter(|&j| r[j].is_some()).collect();
                let vals: Vec<f64> = o.iter().map(|&j| r[j].expect("observed")).collect();
                match groups.iter_mut().find(|(k, _)| *k == o) {
                    Some((_, v)) => v.push(vals),
                    None => groups.push((o, vec![vals])),
                }
            }
            let mut d2 = 0.0;
            let mut df = 0usize;
            let mut ok = true;
            for (o, vals) in &groups {
                let nj = vals.len() as f64;
                let means: Vec<f64> = (0..o.len()).map(|k| vals.iter().map(|v| v[k]).sum::<f64>() / nj).collect();
                let s: Mat = o.iter().map(|&a| o.iter().map(|&c| sigma[a][c]).collect()).collect();
                let Some(inv) = invert(&s) else { ok = false; break };
                let dvec: Vec<f64> = o.iter().enumerate().map(|(k, &a)| means[k] - mu[a]).collect();
                d2 += nj * (0..o.len()).map(|a| (0..o.len()).map(|c| dvec[a] * inv[a][c] * dvec[c]).sum::<f64>()).sum::<f64>();
                df += o.len();
            }
            let df = df as f64 - q as f64;
            if ok && df > 0.0 {
                let pv = chisq_sf(d2, df);
                set(&mut b, "little.chi2", d2);
                set(&mut b, "little.df", df);
                set(&mut b, "little.p", pv);
                set(&mut b, "little.patterns", groups.len() as f64);
                b.tables.push(Table {
                    id: "little".into(),
                    title: "Little's MCAR test".into(),
                    columns: vec!["Chi-square".into(), "df".into(), "p".into(), "Patterns".into()],
                    rows: vec![vec![num(d2, 3), num(df, 0), common::p(pv), count(groups.len())]],
                    footnotes: vec![format!("Numeric variables only ({q}). A significant result is evidence that the values are not missing completely at random; a non-significant one does not show that they are.")],
                });
            }
        }
    }
    Ok(b)
}

// ---------------------------------------------------------------- Multiple imputation

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MiSpec {
    pub dependent: String,
    pub covariates: Vec<String>,
    /// Extra numeric variables used only to impute.
    #[serde(default)]
    pub auxiliary: Vec<String>,
    #[serde(default = "default_m")]
    pub imputations: usize,
    #[serde(default = "default_iterations")]
    pub iterations: usize,
    #[serde(default = "default_seed")]
    pub seed: u64,
    #[serde(default = "default_level")]
    pub level: f64,
}

fn default_m() -> usize {
    20
}

fn default_iterations() -> usize {
    10
}

fn default_seed() -> u64 {
    20_260_101
}

fn default_level() -> f64 {
    0.95
}

/// Pooled results for one parameter (Rubin's rules, Barnard-Rubin df, as mice::pool.scalar).
#[derive(Debug, Clone, PartialEq)]
pub struct Pooled {
    pub estimate: f64,
    pub within: f64,
    pub between: f64,
    pub total: f64,
    pub df: f64,
    pub fmi: f64,
    pub riv: f64,
}

pub fn pool(q: &[f64], u: &[f64], dfcom: f64) -> Pooled {
    let m = q.len() as f64;
    let qbar = q.iter().sum::<f64>() / m;
    let ubar = u.iter().sum::<f64>() / m;
    let b = q.iter().map(|v| (v - qbar).powi(2)).sum::<f64>() / (m - 1.0);
    let t = ubar + (1.0 + 1.0 / m) * b;
    let lambda = ((1.0 + 1.0 / m) * b / t).max(1e-4);
    let dfold = (m - 1.0) / (lambda * lambda);
    let dfobs = (dfcom + 1.0) / (dfcom + 3.0) * dfcom * (1.0 - lambda);
    let df = if dfcom.is_finite() { dfold * dfobs / (dfold + dfobs) } else { dfold };
    let riv = (1.0 + 1.0 / m) * b / ubar;
    let fmi = (riv + 2.0 / (df + 3.0)) / (1.0 + riv);
    Pooled { estimate: qbar, within: ubar, between: b, total: t, df, fmi, riv }
}

/// Standard normal draws (Box-Muller).
fn normal(rng: &mut SplitMix64) -> f64 {
    let u = ((rng.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64;
    let v = ((rng.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64;
    (-2.0 * u.ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos()
}

/// A chi-square draw with integer degrees of freedom, as a sum of squared normals.
fn chisq(rng: &mut SplitMix64, df: usize) -> f64 {
    (0..df).map(|_| normal(rng).powi(2)).sum()
}

/// One completed dataset by chained equations with Bayesian regression draws ("norm").
fn impute(cols: &[Vec<Option<f64>>], iterations: usize, rng: &mut SplitMix64) -> Option<Vec<Vec<f64>>> {
    let p = cols.len();
    let n = cols[0].len();
    // Start from random draws among each variable's observed values.
    let mut data: Vec<Vec<f64>> = cols
        .iter()
        .map(|c| {
            let obs: Vec<f64> = c.iter().flatten().copied().collect();
            c.iter().map(|v| v.unwrap_or_else(|| obs[rng.below(obs.len())])).collect()
        })
        .collect();
    let with_missing: Vec<usize> = (0..p).filter(|&j| cols[j].iter().any(Option::is_none)).collect();
    for _ in 0..iterations {
        for &j in &with_missing {
            let obs: Vec<usize> = (0..n).filter(|&i| cols[j][i].is_some()).collect();
            let preds: Vec<usize> = (0..p).filter(|&k| k != j).collect();
            let row = |i: usize| -> Vec<f64> { std::iter::once(1.0).chain(preds.iter().map(|&k| data[k][i])).collect() };
            let k = preds.len() + 1;
            let mut xtx = vec![vec![0.0; k]; k];
            let mut xty = vec![0.0; k];
            for &i in &obs {
                let r = row(i);
                for a in 0..k {
                    xty[a] += r[a] * data[j][i];
                    for c in 0..k {
                        xtx[a][c] += r[a] * r[c];
                    }
                }
            }
            // A small ridge, as mice uses, keeps the solve stable.
            for (a, rowv) in xtx.iter_mut().enumerate() {
                rowv[a] += 1e-5 * rowv[a].abs().max(1e-12);
            }
            let inv = invert(&xtx)?;
            let beta: Vec<f64> = inv.iter().map(|r| r.iter().zip(&xty).map(|(a, c)| a * c).sum()).collect();
            let rss: f64 = obs.iter().map(|&i| { let r = row(i); (data[j][i] - r.iter().zip(&beta).map(|(a, c)| a * c).sum::<f64>()).powi(2) }).sum();
            let dfr = obs.len().saturating_sub(k).max(1);
            let sigma = (rss / chisq(rng, dfr)).sqrt();
            let chol = cholesky(&inv)?;
            let z: Vec<f64> = (0..k).map(|_| normal(rng)).collect();
            let bstar: Vec<f64> = (0..k).map(|a| beta[a] + sigma * (0..=a).map(|c| chol[a][c] * z[c]).sum::<f64>()).collect();
            let fills: Vec<(usize, f64)> = (0..n).filter(|&i| cols[j][i].is_none()).map(|i| (i, row(i).iter().zip(&bstar).map(|(a, c)| a * c).sum::<f64>())).collect();
            for (i, mean) in fills {
                data[j][i] = mean + sigma * normal(rng);
            }
        }
    }
    Some(data)
}

pub fn mi_regression(ds: &Dataset, spec: &MiSpec) -> Result<ResultBundle> {
    if spec.covariates.is_empty() {
        return Err(Error::Invalid("Choose at least one predictor.".into()));
    }
    if !(2..=500).contains(&spec.imputations) || !(1..=200).contains(&spec.iterations) {
        return Err(Error::Invalid("Use 2 to 500 imputations and 1 to 200 iterations.".into()));
    }
    check_level(spec.level)?;
    let names: Vec<&String> = std::iter::once(&spec.dependent).chain(&spec.covariates).chain(&spec.auxiliary).collect();
    for (i, a) in names.iter().enumerate() {
        if names[..i].iter().any(|b| ds.find(a) == ds.find(b)) {
            return Err(Error::Invalid(format!("{a} is used more than once.")));
        }
    }
    let labels: Vec<String> = names.iter().map(|n| ds.variable(n).map(|v| v.display_name().to_string())).collect::<Result<_>>()?;
    let cols: Vec<Vec<Option<f64>>> = names.iter().map(|n| numeric(ds, n).map(|c| c.iter().map(Slot::value).collect())).collect::<Result<_>>()?;
    // Cases missing every variable carry no information and are left out.
    let n_all = cols[0].len();
    let keep: Vec<usize> = (0..n_all).filter(|&r| cols.iter().any(|c| c[r].is_some())).collect();
    let cols: Vec<Vec<Option<f64>>> = cols.iter().map(|c| keep.iter().map(|&r| c[r]).collect()).collect();
    let n = keep.len();
    let k = spec.covariates.len();
    let lv = level_text(spec.level);
    let note = format!(
        "Multiple imputation by chained equations: each variable with missing values imputed from all the others by Bayesian linear-regression draws (mice's \"norm\" method), {} imputations of {} iterations each (seed {}){}. The regression was fitted to each completed dataset and pooled by Rubin's rules with Barnard and Rubin's degrees of freedom; {lv} intervals. Computed by PrismStatistics.",
        spec.imputations,
        spec.iterations,
        spec.seed,
        if spec.auxiliary.is_empty() { String::new() } else { format!(", with {} auxiliary variable{}", spec.auxiliary.len(), if spec.auxiliary.len() == 1 { "" } else { "s" }) }
    );
    let mut b = common::bundle(MI_REGRESSION, METHOD_VERSION, format!("Multiple Imputation Regression: {}", labels[0]), serde_json::to_value(spec).expect("a spec always serializes"), CaseAccounting { total: n_all, analyzed: n, excluded: if n < n_all { vec![crate::result::Exclusion { reason: "every variable missing".into(), count: n_all - n }] } else { Vec::new() } }, note);
    set_text(&mut b, "y.label", labels[0].clone());
    set(&mut b, "n", n as f64);
    set(&mut b, "m", spec.imputations as f64);
    set(&mut b, "level", spec.level);
    let complete = (0..n).filter(|&r| cols.iter().all(|c| c[r].is_some())).count();
    set(&mut b, "complete", complete as f64);
    if n <= k + 2 || cols.iter().any(|c| c.iter().flatten().count() < 3) {
        fail(&mut b, "Each variable needs at least three observed values, and there must be more cases than predictors.");
        return Ok(b);
    }
    let mut rng = SplitMix64::new(spec.seed);
    let mut qs: Vec<Vec<f64>> = vec![Vec::new(); k + 1];
    let mut us: Vec<Vec<f64>> = vec![Vec::new(); k + 1];
    let mut r2s = Vec::new();
    for m in 0..spec.imputations {
        if crate::job::step(m, spec.imputations) {
            break;
        }
        let Some(data) = impute(&cols, spec.iterations, &mut rng) else {
            fail(&mut b, "The imputation models could not be fitted: the variables may be collinear.");
            return Ok(b);
        };
        let Some(o) = super::process::ols(&data[0], &data[1..=k]) else {
            fail(&mut b, "The regression could not be fitted to an imputed dataset.");
            return Ok(b);
        };
        r2s.push(o.r2);
        for j in 0..=k {
            qs[j].push(o.b[j]);
            us[j].push(o.vcov[j][j]);
        }
    }
    let dfcom = (n - k - 1) as f64;
    let mut rows = Vec::new();
    for j in 0..=k {
        let pl = pool(&qs[j], &us[j], dfcom);
        let se = pl.total.sqrt();
        let t = pl.estimate / se;
        let pv = t_two_sided(t, pl.df);
        let qq = t_quantile(1.0 - (1.0 - spec.level) / 2.0, pl.df);
        let name = if j == 0 { "(Intercept)".to_string() } else { labels[j].clone() };
        let key = if j == 0 { "b0".to_string() } else { format!("term.{j}") };
        if j > 0 {
            set_text(&mut b, format!("{key}.label"), name.clone());
        }
        for (f, v) in [("b", pl.estimate), ("se", se), ("t", t), ("df", pl.df), ("p", pv), ("lower", pl.estimate - qq * se), ("upper", pl.estimate + qq * se), ("fmi", pl.fmi)] {
            set(&mut b, format!("{key}.{f}"), v);
        }
        rows.push(vec![text(name), num(pl.estimate, 4), num(se, 4), num(t, 3), num(pl.df, 2), p(pv), num(pl.estimate - qq * se, 4), num(pl.estimate + qq * se, 4), num(pl.fmi, 3)]);
    }
    let r2 = r2s.iter().sum::<f64>() / r2s.len() as f64;
    set(&mut b, "r2", r2);
    b.tables.push(Table {
        id: "pooled".into(),
        title: "Pooled coefficients".into(),
        columns: vec!["Term".into(), "b".into(), "SE".into(), "t".into(), "df".into(), "p".into(), format!("{lv} CI lower"), format!("{lv} CI upper"), "FMI".into()],
        rows,
        footnotes: vec![format!("FMI: fraction of missing information. {complete} of {n} cases had every variable; mean R² across imputations = {}.", apa::num(r2, 3))],
    });
    b.diagnostics.push(not_assessed("mar", "Missing at random", "Multiple imputation assumes the values are missing at random given the variables in the imputation model. Add auxiliary variables that predict missingness or the missing values."));
    Ok(b)
}
