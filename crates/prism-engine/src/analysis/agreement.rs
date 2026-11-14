//! Agreement between raters (Analyze › Scale): Cohen's kappa (`agree.kappa`), Fleiss' kappa (`agree.fleiss`),
//! Krippendorff's alpha (`agree.kripp`) and intraclass correlations (`agree.icc`).
//!
//! Each rater is a variable and each case a rated unit. Defaults (05 §1): Cohen's kappa unweighted, with linear or
//! quadratic weights on request, the test from the standard error under no agreement and the interval from the
//! asymptotic standard error (Fleiss, Cohen and Everitt, 1969), as R's irr and psych give them; Fleiss' kappa with its
//! test (irr); Krippendorff's alpha from the coincidence matrix with the metric chosen (nominal, ordinal, interval or
//! ratio), using every unit with two or more ratings; the six intraclass correlations of McGraw and Wong (1996), with F
//! tests and intervals as irr::icc. Cases need every rating except for Krippendorff's alpha.

use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{ResultBundle, Table};
use prismqda_stats::dist::{f_quantile, f_sf, norm_cdf, norm_quantile};

pub const KAPPA: &str = "agree.kappa";
pub const FLEISS: &str = "agree.fleiss";
pub const KRIPP: &str = "agree.kripp";
pub const ICC: &str = "agree.icc";
pub const METHOD_VERSION: u32 = 1;

fn level95() -> f64 {
    0.95
}

fn labels(ds: &Dataset, names: &[String]) -> Result<Vec<String>> {
    names.iter().map(|n| ds.variable(n).map(|v| v.display_name().to_string())).collect()
}

fn check_distinct(ds: &Dataset, names: &[String], min: usize) -> Result<()> {
    if names.len() < min {
        return Err(Error::Invalid(format!("Choose at least {min} raters (variables).")));
    }
    for (i, a) in names.iter().enumerate() {
        if names[..i].iter().any(|b| ds.find(a) == ds.find(b)) {
            return Err(Error::Invalid(format!("{a} is listed twice.")));
        }
    }
    Ok(())
}

/// Each case's ratings (None where missing), the reasons for exclusion, and the cases with every rating.
type Ratings = (Vec<Vec<Option<f64>>>, Tally, Vec<usize>);

fn ratings(ds: &Dataset, names: &[String]) -> Result<Ratings> {
    let cells: Vec<Vec<Slot<f64>>> = names.iter().map(|n| numeric(ds, n)).collect::<Result<_>>()?;
    let total = cells[0].len();
    let mut tally = Tally::default();
    let complete: Vec<usize> = (0..total).filter(|&r| !cells.iter().zip(names).any(|(c, n)| tally.check(&c[r], n))).collect();
    let rows = (0..total).map(|r| cells.iter().map(|c| c[r].value()).collect()).collect();
    Ok((rows, tally, complete))
}

fn mk(id: &str, title: String, spec: &impl Serialize, cases: crate::result::CaseAccounting, note: String) -> ResultBundle {
    common::bundle(id, METHOD_VERSION, title, serde_json::to_value(spec).expect("a spec always serializes"), cases, format!("{note} Computed by PrismStatistics."))
}

// ---------------------------------------------------------------- Cohen's kappa

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Weights {
    #[default]
    None,
    Linear,
    Quadratic,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KappaSpec {
    pub rater1: String,
    pub rater2: String,
    #[serde(default)]
    pub weights: Weights,
    #[serde(default = "level95")]
    pub level: f64,
}

/// Cohen's kappa of a square table of proportions with agreement weights `w`: (kappa, asymptotic SE, SE under no
/// agreement, observed agreement, chance agreement).
pub fn kappa_table(p: &[Vec<f64>], w: &[Vec<f64>], n: f64) -> (f64, f64, f64, f64, f64) {
    let k = p.len();
    let pr: Vec<f64> = (0..k).map(|i| p[i].iter().sum()).collect();
    let pc: Vec<f64> = (0..k).map(|j| p.iter().map(|r| r[j]).sum()).collect();
    let po: f64 = (0..k).flat_map(|i| (0..k).map(move |j| (i, j))).map(|(i, j)| w[i][j] * p[i][j]).sum();
    let pe: f64 = (0..k).flat_map(|i| (0..k).map(move |j| (i, j))).map(|(i, j)| w[i][j] * pr[i] * pc[j]).sum();
    let kappa = (po - pe) / (1.0 - pe);
    let wr: Vec<f64> = (0..k).map(|i| (0..k).map(|j| pc[j] * w[i][j]).sum()).collect();
    let wc: Vec<f64> = (0..k).map(|j| (0..k).map(|i| pr[i] * w[i][j]).sum()).collect();
    let mut s = 0.0;
    let mut s0 = 0.0;
    for i in 0..k {
        for j in 0..k {
            s += p[i][j] * (w[i][j] - (wr[i] + wc[j]) * (1.0 - kappa)).powi(2);
            s0 += pr[i] * pc[j] * (w[i][j] - (wr[i] + wc[j])).powi(2);
        }
    }
    let ase = ((s - (kappa - pe * (1.0 - kappa)).powi(2)).max(0.0) / (n * (1.0 - pe).powi(2))).sqrt();
    let se0 = ((s0 - pe * pe).max(0.0) / (n * (1.0 - pe).powi(2))).sqrt();
    (kappa, ase, se0, po, pe)
}

pub fn kappa(ds: &Dataset, spec: &KappaSpec) -> Result<ResultBundle> {
    let names = vec![spec.rater1.clone(), spec.rater2.clone()];
    check_distinct(ds, &names, 2)?;
    check_level(spec.level)?;
    let l = labels(ds, &names)?;
    let (rows, tally, complete) = ratings(ds, &names)?;
    let lv = level_text(spec.level);
    let wname = match spec.weights {
        Weights::None => "unweighted",
        Weights::Linear => "with linear weights",
        Weights::Quadratic => "with quadratic weights",
    };
    let mut b = mk(KAPPA, format!("Cohen's Kappa: {} and {}", l[0], l[1]), spec, tally.accounting(rows.len(), complete.len()), format!("Cohen's kappa {wname} for two raters. The categories are the codes either rater used, in code order. The test uses the standard error under no agreement and the {lv} interval the asymptotic standard error (Fleiss, Cohen and Everitt, 1969). Cases were used when both raters gave a valid code."));
    set_text(&mut b, "r1.label", l[0].clone());
    set_text(&mut b, "r2.label", l[1].clone());
    set_text(&mut b, "weights", wname);
    set(&mut b, "level", spec.level);
    let mut codes: Vec<f64> = complete.iter().flat_map(|&r| rows[r].iter().flatten().copied()).collect();
    codes.sort_by(f64::total_cmp);
    codes.dedup();
    let n = complete.len();
    set(&mut b, "n", n as f64);
    if codes.len() < 2 || n < 2 {
        fail(&mut b, "Kappa needs at least two cases and at least two categories between the raters.");
        return Ok(b);
    }
    let k = codes.len();
    let idx = |v: f64| codes.iter().position(|c| *c == v).expect("listed");
    let mut t = vec![vec![0.0; k]; k];
    for &r in &complete {
        t[idx(rows[r][0].expect("complete"))][idx(rows[r][1].expect("complete"))] += 1.0;
    }
    let props: Vec<Vec<f64>> = t.iter().map(|row| row.iter().map(|c| c / n as f64).collect()).collect();
    let w: Vec<Vec<f64>> = (0..k)
        .map(|i| {
            (0..k)
                .map(|j| {
                    let d = (i as f64 - j as f64).abs() / (k as f64 - 1.0);
                    match spec.weights {
                        Weights::None => f64::from(u8::from(i == j)),
                        Weights::Linear => 1.0 - d,
                        Weights::Quadratic => 1.0 - d * d,
                    }
                })
                .collect()
        })
        .collect();
    let (kap, ase, se0, po, pe) = kappa_table(&props, &w, n as f64);
    let z = kap / se0;
    let pv = 2.0 * norm_cdf(-z.abs());
    let zc = norm_quantile(1.0 - (1.0 - spec.level) / 2.0);
    for (key, v) in [("kappa", kap), ("ase", ase), ("se0", se0), ("z", z), ("p", pv), ("ci.lower", kap - zc * ase), ("ci.upper", kap + zc * ase), ("agreement", po), ("chance", pe)] {
        set(&mut b, key, v);
    }
    let mut cols = vec![l[0].clone()];
    let vlab = |c: f64| ds.variable(&spec.rater1).map(|v| v.value_text(c)).unwrap_or_default();
    cols.extend(codes.iter().map(|c| format!("{}: {}", l[1], vlab(*c))));
    b.tables.push(Table { id: "table".into(), title: format!("{} × {}", l[0], l[1]), columns: cols, rows: (0..k).map(|i| { let mut r = vec![text(vlab(codes[i]))]; r.extend(t[i].iter().map(|c| count(*c as usize))); r }).collect(), footnotes: Vec::new() });
    b.tables.push(Table {
        id: "kappa".into(),
        title: "Cohen's kappa".into(),
        columns: vec!["Kappa".into(), "ASE".into(), format!("{lv} CI lower"), format!("{lv} CI upper"), "z".into(), "p".into(), "Observed agreement".into(), "Chance agreement".into(), "N".into()],
        rows: vec![vec![num(kap, 3), num(ase, 3), num(kap - zc * ase, 3), num(kap + zc * ase, 3), num(z, 3), p(pv), num(po, 3), num(pe, 3), count(n)]],
        footnotes: vec![format!("Kappa {wname}.")],
    });
    b.diagnostics.push(not_assessed("prevalence", "Prevalence and bias", "Kappa falls when one category is very common or the raters use categories at different rates, even when agreement is high. Read it with the observed agreement and the table."));
    Ok(b)
}

// ---------------------------------------------------------------- Fleiss' kappa

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RatersSpec {
    pub raters: Vec<String>,
}

pub fn fleiss(ds: &Dataset, spec: &RatersSpec) -> Result<ResultBundle> {
    check_distinct(ds, &spec.raters, 2)?;
    let l = labels(ds, &spec.raters)?;
    let (rows, tally, complete) = ratings(ds, &spec.raters)?;
    let m = spec.raters.len() as f64;
    let mut b = mk(FLEISS, format!("Fleiss' Kappa: {} raters", spec.raters.len()), spec, tally.accounting(rows.len(), complete.len()), "Fleiss' kappa for several raters, with the z test of no agreement (as R's irr::kappam.fleiss). Cases were used when every rater gave a valid code.".into());
    set_text(&mut b, "raters", l.join("\u{1f}"));
    let n = complete.len() as f64;
    set(&mut b, "n", n);
    let mut codes: Vec<f64> = complete.iter().flat_map(|&r| rows[r].iter().flatten().copied()).collect();
    codes.sort_by(f64::total_cmp);
    codes.dedup();
    if codes.len() < 2 || n < 2.0 {
        fail(&mut b, "Fleiss' kappa needs at least two cases and at least two categories.");
        return Ok(b);
    }
    let counts: Vec<Vec<f64>> = complete.iter().map(|&r| codes.iter().map(|c| rows[r].iter().filter(|v| **v == Some(*c)).count() as f64).collect()).collect();
    let pj: Vec<f64> = (0..codes.len()).map(|j| counts.iter().map(|r| r[j]).sum::<f64>() / (n * m)).collect();
    let pbar = counts.iter().map(|r| (r.iter().map(|x| x * x).sum::<f64>() - m) / (m * (m - 1.0))).sum::<f64>() / n;
    let pe: f64 = pj.iter().map(|x| x * x).sum();
    let kap = (pbar - pe) / (1.0 - pe);
    let sq: f64 = pj.iter().map(|x| x * (1.0 - x)).sum();
    let se = 2f64.sqrt() / (sq * (n * m * (m - 1.0)).sqrt()) * (sq * sq - pj.iter().map(|x| x * (1.0 - x) * (1.0 - 2.0 * x)).sum::<f64>()).max(0.0).sqrt();
    let z = kap / se;
    let pv = 2.0 * norm_cdf(-z.abs());
    for (key, v) in [("kappa", kap), ("z", z), ("p", pv), ("agreement", pbar), ("chance", pe), ("rater_count", m)] {
        set(&mut b, key, v);
    }
    b.tables.push(Table { id: "kappa".into(), title: "Fleiss' kappa".into(), columns: vec!["Kappa".into(), "z".into(), "p".into(), "Observed agreement".into(), "Chance agreement".into(), "Raters".into(), "N".into()], rows: vec![vec![num(kap, 3), num(z, 3), p(pv), num(pbar, 3), num(pe, 3), count(spec.raters.len()), count(complete.len())]], footnotes: Vec::new() });
    Ok(b)
}

// ---------------------------------------------------------------- Krippendorff's alpha

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    #[default]
    Nominal,
    Ordinal,
    Interval,
    Ratio,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KrippSpec {
    pub raters: Vec<String>,
    #[serde(default)]
    pub metric: Metric,
}

/// Krippendorff's alpha from units of ratings (each unit's valid values), for the metric. None when undefined.
pub fn kripp_alpha(units: &[Vec<f64>], metric: Metric) -> Option<(f64, usize, f64)> {
    let mut values: Vec<f64> = units.iter().filter(|u| u.len() >= 2).flatten().copied().collect();
    values.sort_by(f64::total_cmp);
    values.dedup();
    let q = values.len();
    if q < 2 {
        return None;
    }
    let idx = |v: f64| values.iter().position(|x| *x == v).expect("listed");
    let mut o = vec![vec![0.0; q]; q];
    let mut pairable = 0;
    for u in units.iter().filter(|u| u.len() >= 2) {
        pairable += 1;
        let m = u.len() as f64;
        for (i, a) in u.iter().enumerate() {
            for (j, c) in u.iter().enumerate() {
                if i != j {
                    o[idx(*a)][idx(*c)] += 1.0 / (m - 1.0);
                }
            }
        }
    }
    let nc: Vec<f64> = o.iter().map(|r| r.iter().sum()).collect();
    let n: f64 = nc.iter().sum();
    let delta = |c: usize, k: usize| -> f64 {
        match metric {
            Metric::Nominal => f64::from(u8::from(c != k)),
            Metric::Interval => (values[c] - values[k]).powi(2),
            Metric::Ratio => ((values[c] - values[k]) / (values[c] + values[k])).powi(2),
            Metric::Ordinal => {
                let (lo, hi) = (c.min(k), c.max(k));
                ((lo..=hi).map(|g| nc[g]).sum::<f64>() - (nc[c] + nc[k]) / 2.0).powi(2)
            }
        }
    };
    let (mut d_o, mut d_e) = (0.0, 0.0);
    for c in 0..q {
        for k in 0..q {
            let dd = delta(c, k);
            d_o += o[c][k] * dd;
            d_e += nc[c] * nc[k] * dd;
        }
    }
    let alpha = 1.0 - (n - 1.0) * d_o / d_e;
    alpha.is_finite().then_some((alpha, pairable, n))
}

pub fn kripp(ds: &Dataset, spec: &KrippSpec) -> Result<ResultBundle> {
    check_distinct(ds, &spec.raters, 2)?;
    let l = labels(ds, &spec.raters)?;
    let (rows, _, _) = ratings(ds, &spec.raters)?;
    let units: Vec<Vec<f64>> = rows.iter().map(|r| r.iter().flatten().copied().collect()).collect();
    let used = units.iter().filter(|u| u.len() >= 2).count();
    let mut tally = Tally::default();
    for u in &units {
        if u.len() < 2 {
            tally.add("fewer than two ratings");
        }
    }
    let mname = match spec.metric {
        Metric::Nominal => "nominal",
        Metric::Ordinal => "ordinal",
        Metric::Interval => "interval",
        Metric::Ratio => "ratio",
    };
    let mut b = mk(KRIPP, format!("Krippendorff's Alpha ({mname}): {} raters", spec.raters.len()), spec, tally.accounting(rows.len(), used), format!("Krippendorff's alpha with the {mname} metric, from the coincidence matrix of every unit with two or more ratings (missing ratings allowed), as R's irr::kripp.alpha."));
    set_text(&mut b, "raters", l.join("\u{1f}"));
    set_text(&mut b, "metric", mname);
    let Some((a, units_used, pairable)) = kripp_alpha(&units, spec.metric) else {
        fail(&mut b, "Alpha needs units with at least two ratings and more than one value among them.");
        return Ok(b);
    };
    set(&mut b, "alpha", a);
    set(&mut b, "units", units_used as f64);
    set(&mut b, "values", pairable);
    b.tables.push(Table { id: "alpha".into(), title: "Krippendorff's alpha".into(), columns: vec!["Alpha".into(), "Metric".into(), "Units".into(), "Pairable values".into(), "Raters".into()], rows: vec![vec![num(a, 3), text(mname), count(units_used), num(pairable, 0), count(spec.raters.len())]], footnotes: vec!["Krippendorff suggests relying on data with alpha of at least .800, and drawing only tentative conclusions from .667 to .800.".into()] });
    Ok(b)
}

// ---------------------------------------------------------------- intraclass correlation

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IccModel {
    OneWay,
    #[default]
    TwoWay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IccType {
    #[default]
    Agreement,
    Consistency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IccUnit {
    #[default]
    Single,
    Average,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IccSpec {
    pub raters: Vec<String>,
    #[serde(default)]
    pub model: IccModel,
    #[serde(default)]
    pub kind: IccType,
    #[serde(default)]
    pub unit: IccUnit,
    #[serde(default = "level95")]
    pub level: f64,
}

/// The ICC with its F test and interval (McGraw and Wong, 1996), from complete ratings (rows are subjects).
pub fn icc_values(x: &[Vec<f64>], model: IccModel, kind: IccType, unit: IccUnit, level: f64) -> Option<(f64, f64, f64, f64, f64, f64, f64)> {
    let n = x.len();
    let k = x.first()?.len();
    if n < 2 || k < 2 {
        return None;
    }
    let (nf, kf) = (n as f64, k as f64);
    let grand = x.iter().flatten().sum::<f64>() / (nf * kf);
    let rm: Vec<f64> = x.iter().map(|r| r.iter().sum::<f64>() / kf).collect();
    let cm: Vec<f64> = (0..k).map(|j| x.iter().map(|r| r[j]).sum::<f64>() / nf).collect();
    let sst: f64 = x.iter().flatten().map(|v| (v - grand).powi(2)).sum();
    let ssr = kf * rm.iter().map(|m| (m - grand).powi(2)).sum::<f64>();
    let ssc = nf * cm.iter().map(|m| (m - grand).powi(2)).sum::<f64>();
    let sse = sst - ssr - ssc;
    let msr = ssr / (nf - 1.0);
    let msc = ssc / (kf - 1.0);
    let mse = sse / ((nf - 1.0) * (kf - 1.0));
    let msw = (ssc + sse) / (nf * (kf - 1.0));
    let alpha = 1.0 - level;
    let (icc, f, df1, df2, lo, hi);
    match (model, kind) {
        (IccModel::OneWay, _) => {
            f = msr / msw;
            df1 = nf - 1.0;
            df2 = nf * (kf - 1.0);
            let fl = f / f_quantile(1.0 - alpha / 2.0, df1, df2);
            let fu = f * f_quantile(1.0 - alpha / 2.0, df2, df1);
            match unit {
                IccUnit::Single => {
                    icc = (msr - msw) / (msr + (kf - 1.0) * msw);
                    lo = (fl - 1.0) / (fl + kf - 1.0);
                    hi = (fu - 1.0) / (fu + kf - 1.0);
                }
                IccUnit::Average => {
                    icc = (msr - msw) / msr;
                    lo = 1.0 - 1.0 / fl;
                    hi = 1.0 - 1.0 / fu;
                }
            }
        }
        (IccModel::TwoWay, IccType::Consistency) => {
            f = msr / mse;
            df1 = nf - 1.0;
            df2 = (nf - 1.0) * (kf - 1.0);
            let fl = f / f_quantile(1.0 - alpha / 2.0, df1, df2);
            let fu = f * f_quantile(1.0 - alpha / 2.0, df2, df1);
            match unit {
                IccUnit::Single => {
                    icc = (msr - mse) / (msr + (kf - 1.0) * mse);
                    lo = (fl - 1.0) / (fl + kf - 1.0);
                    hi = (fu - 1.0) / (fu + kf - 1.0);
                }
                IccUnit::Average => {
                    icc = (msr - mse) / msr;
                    lo = 1.0 - 1.0 / fl;
                    hi = 1.0 - 1.0 / fu;
                }
            }
        }
        (IccModel::TwoWay, IccType::Agreement) => {
            f = msr / mse;
            df1 = nf - 1.0;
            df2 = (nf - 1.0) * (kf - 1.0);
            let single = (msr - mse) / (msr + (kf - 1.0) * mse + kf / nf * (msc - mse));
            let a = kf * single / (nf * (1.0 - single));
            let bb = 1.0 + kf * single * (nf - 1.0) / (nf * (1.0 - single));
            let v = (a * msc + bb * mse).powi(2) / ((a * msc).powi(2) / (kf - 1.0) + (bb * mse).powi(2) / ((nf - 1.0) * (kf - 1.0)));
            match unit {
                IccUnit::Single => {
                    let fl = f_quantile(1.0 - alpha / 2.0, nf - 1.0, v);
                    let fu = f_quantile(1.0 - alpha / 2.0, v, nf - 1.0);
                    icc = single;
                    lo = nf * (msr - fl * mse) / (fl * (kf * msc + (kf * nf - kf - nf) * mse) + nf * msr);
                    hi = nf * (fu * msr - mse) / (kf * msc + (kf * nf - kf - nf) * mse + nf * fu * msr);
                }
                IccUnit::Average => {
                    // irr::icc's interval for the average: the Satterthwaite df from the average ICC itself.
                    icc = (msr - mse) / (msr + (msc - mse) / nf);
                    let a = kf * icc / (nf * (1.0 - icc));
                    let bb = 1.0 + kf * icc * (nf - 1.0) / (nf * (1.0 - icc));
                    let v = (a * msc + bb * mse).powi(2) / ((a * msc).powi(2) / (kf - 1.0) + (bb * mse).powi(2) / ((nf - 1.0) * (kf - 1.0)));
                    let fl = f_quantile(1.0 - alpha / 2.0, nf - 1.0, v);
                    let fu = f_quantile(1.0 - alpha / 2.0, v, nf - 1.0);
                    lo = nf * (msr - fl * mse) / (fl * (msc - mse) + nf * msr);
                    hi = nf * (fu * msr - mse) / (msc - mse + nf * fu * msr);
                }
            }
        }
    }
    Some((icc, f, df1, df2, f_sf(f, df1, df2), lo, hi))
}

pub fn icc(ds: &Dataset, spec: &IccSpec) -> Result<ResultBundle> {
    check_distinct(ds, &spec.raters, 2)?;
    check_level(spec.level)?;
    let l = labels(ds, &spec.raters)?;
    let (rows, tally, complete) = ratings(ds, &spec.raters)?;
    let x: Vec<Vec<f64>> = complete.iter().map(|&r| rows[r].iter().map(|v| v.expect("complete")).collect()).collect();
    let name = match (spec.model, spec.kind, spec.unit) {
        (IccModel::OneWay, _, IccUnit::Single) => "ICC(1), one-way random, single rater",
        (IccModel::OneWay, _, IccUnit::Average) => "ICC(k), one-way random, average of the raters",
        (IccModel::TwoWay, IccType::Agreement, IccUnit::Single) => "ICC(A,1), two-way, absolute agreement, single rater",
        (IccModel::TwoWay, IccType::Agreement, IccUnit::Average) => "ICC(A,k), two-way, absolute agreement, average of the raters",
        (IccModel::TwoWay, IccType::Consistency, IccUnit::Single) => "ICC(C,1), two-way, consistency, single rater",
        (IccModel::TwoWay, IccType::Consistency, IccUnit::Average) => "ICC(C,k), two-way, consistency, average of the raters",
    };
    let lv = level_text(spec.level);
    let mut b = mk(ICC, format!("Intraclass Correlation: {} raters", spec.raters.len()), spec, tally.accounting(rows.len(), complete.len()), format!("{name} (McGraw and Wong, 1996), with the F test of no reliability and a {lv} interval, as R's irr::icc. The same formulas serve random and mixed raters; the interpretation differs. Cases were used when every rater gave a valid value."));
    set_text(&mut b, "raters", l.join("\u{1f}"));
    set_text(&mut b, "form", name);
    set(&mut b, "level", spec.level);
    set(&mut b, "n", x.len() as f64);
    let Some((v, f, d1, d2, pv, lo, hi)) = icc_values(&x, spec.model, spec.kind, spec.unit, spec.level) else {
        fail(&mut b, "The ICC needs at least two cases with every rating.");
        return Ok(b);
    };
    for (key, val) in [("icc", v), ("F", f), ("df1", d1), ("df2", d2), ("p", pv), ("ci.lower", lo), ("ci.upper", hi)] {
        set(&mut b, key, val);
    }
    b.tables.push(Table {
        id: "icc".into(),
        title: "Intraclass correlation".into(),
        columns: vec!["Form".into(), "ICC".into(), format!("{lv} CI lower"), format!("{lv} CI upper"), "F".into(), "df1".into(), "df2".into(), "p".into()],
        rows: vec![vec![text(name), num(v, 3), num(lo, 3), num(hi, 3), num(f, 3), num(d1, 0), num(d2, 2), p(pv)]],
        footnotes: vec![format!("{} subjects rated by {} raters.", x.len(), spec.raters.len())],
    });
    b.diagnostics.push(size_diagnostic(x.len(), format!("{} subjects had every rating", x.len())));
    Ok(b)
}
