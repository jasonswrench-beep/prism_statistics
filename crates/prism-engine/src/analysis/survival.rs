//! Survival analysis (checklist B11a–b) and ROC analysis (B11c).
//!
//! * Kaplan-Meier (`surv.km`): the product-limit estimate overall or by group, Greenwood standard errors and intervals on
//!   the log scale (R's survfit default), median survival with its interval, and the log-rank test (survdiff).
//! * Cox regression (`surv.cox`): the proportional hazards model with Efron's handling of tied times (R's coxph default),
//!   hazard ratios, and the likelihood-ratio, Wald and score tests.
//! * ROC analysis (`roc.analysis`): for each test variable, the area under the ROC curve with DeLong's standard error and
//!   interval, the cutoff that maximizes Youden's J, and the curve.

use prismqda_stats::dist::{chisq_sf, norm_quantile, norm_two_sided};
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use super::linalg::{invert, Mat};
use super::regression::{self, Factor};
use crate::apa;
use crate::chart::{self, Chart};
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Cell, ResultBundle, Table};

pub const KM: &str = "surv.km";
pub const COX: &str = "surv.cox";
pub const ROC: &str = "roc.analysis";
pub const METHOD_VERSION: u32 = 1;

fn default_level() -> f64 {
    0.95
}

/// The cases' times and whether the event happened (the status equals one of the event codes).
fn time_status(ds: &Dataset, time: &str, status: &str, events: &[f64], tally: &mut Tally, rows: &[usize]) -> Result<Vec<(usize, f64, bool)>> {
    if events.is_empty() {
        return Err(Error::Invalid("Give the status code (or codes) that mean the event happened.".into()));
    }
    let (ts, ss) = (numeric(ds, time)?, numeric(ds, status)?);
    let mut out = Vec::new();
    for &r in rows {
        if tally.check(&ts[r], "time") || tally.check(&ss[r], "status") {
            continue;
        }
        let t = ts[r].value().expect("checked");
        if t < 0.0 {
            tally.add("negative time");
            continue;
        }
        out.push((r, t, events.contains(&ss[r].value().expect("checked"))));
    }
    Ok(out)
}

/// One Kaplan-Meier curve: (time, at risk, events, censored, survival, SE of log S) at each distinct time with an event.
pub struct Curve {
    pub steps: Vec<(f64, usize, usize, usize, f64, f64)>,
    pub n: usize,
    pub events: usize,
}

pub fn kaplan_meier(data: &[(f64, bool)]) -> Curve {
    let mut d: Vec<(f64, bool)> = data.to_vec();
    d.sort_by(|a, b| a.0.total_cmp(&b.0));
    let n = d.len();
    let mut at_risk = n;
    let mut s = 1.0;
    let mut gw = 0.0;
    let mut steps = Vec::new();
    let mut i = 0;
    while i < n {
        let t = d[i].0;
        let mut j = i;
        let (mut ev, mut cen) = (0, 0);
        while j < n && d[j].0 == t {
            if d[j].1 { ev += 1 } else { cen += 1 }
            j += 1;
        }
        if ev > 0 {
            s *= 1.0 - ev as f64 / at_risk as f64;
            if at_risk > ev {
                gw += ev as f64 / (at_risk as f64 * (at_risk - ev) as f64);
            }
            steps.push((t, at_risk, ev, cen, s, if s > 0.0 { gw.sqrt() } else { f64::NAN }));
        }
        at_risk -= ev + cen;
        i = j;
    }
    Curve { events: data.iter().filter(|x| x.1).count(), steps, n }
}

/// The time a curve first reaches `p` (the median for 0.5), as R's survfit finds it: where the curve is exactly at `p`
/// over an interval, the midpoint of that interval.
fn quantile_time(times: &[f64], surv: &[f64], p: f64) -> Option<f64> {
    for (k, &sv) in surv.iter().enumerate() {
        if (sv - p).abs() < 1e-12 {
            return Some(match times.get(k + 1) { Some(next) => (times[k] + next) / 2.0, None => times[k] });
        }
        if sv < p {
            return Some(times[k]);
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KmSpec {
    pub time: String,
    pub status: String,
    /// The status codes that mean the event happened (the others are censored).
    pub events: Vec<f64>,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default = "yes")]
    pub plot: bool,
    #[serde(default = "default_level")]
    pub level: f64,
}

fn yes() -> bool {
    true
}

pub fn km(ds: &Dataset, spec: &KmSpec) -> Result<ResultBundle> {
    check_level(spec.level)?;
    let total = ds.n_rows();
    let mut tally = Tally::default();
    let all: Vec<usize> = (0..total).collect();
    let rows = time_status(ds, &spec.time, &spec.status, &spec.events, &mut tally, &all)?;
    let (gslots, levels) = match &spec.group {
        Some(g) => {
            let (s, l) = categories(ds, g)?;
            (Some(s), l)
        }
        None => (None, Vec::new()),
    };
    let rows: Vec<(usize, f64, bool)> = rows.into_iter().filter(|(r, _, _)| gslots.as_ref().is_none_or(|gs| !tally.check(&gs[*r], "group"))).collect();
    let t_label = ds.variable(&spec.time)?.display_name().to_string();
    let g_label = spec.group.as_ref().map(|g| ds.variable(g).map(|v| v.display_name().to_string())).transpose()?;
    let lv = level_text(spec.level);
    let z = norm_quantile(1.0 - (1.0 - spec.level) / 2.0);
    let note = format!("Kaplan-Meier estimates with Greenwood standard errors and {lv} intervals on the log scale (as R's survfit); the median is the first time survival falls below 0.5. {}Computed by PrismStatistics.", if g_label.is_some() { "Groups are compared by the log-rank test (as R's survdiff). " } else { "" });
    let mut b = common::bundle(KM, METHOD_VERSION, match &g_label { Some(g) => format!("Kaplan-Meier: {t_label} by {g}"), None => format!("Kaplan-Meier: {t_label}") }, serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, rows.len()), note);
    set_text(&mut b, "time.label", t_label.clone());
    set(&mut b, "n", rows.len() as f64);
    set(&mut b, "level", spec.level);
    if rows.is_empty() {
        fail(&mut b, "No case has a valid time and status.");
        return Ok(b);
    }
    // The groups present, in code order.
    let group_of = |r: usize| gslots.as_ref().map_or(0, |gs| gs[r].value().expect("checked"));
    let mut present: Vec<usize> = rows.iter().map(|(r, _, _)| group_of(*r)).collect();
    present.sort_unstable();
    present.dedup();
    let mut summary = Vec::new();
    let mut steps_rows = Vec::new();
    let mut curves = Vec::new();
    for (gi, &g) in present.iter().enumerate() {
        let data: Vec<(f64, bool)> = rows.iter().filter(|(r, _, _)| group_of(*r) == g).map(|(_, t, e)| (*t, *e)).collect();
        let c = kaplan_meier(&data);
        let name = if gslots.is_some() { levels[g].label.clone() } else { "All cases".to_string() };
        let key = format!("group.{}", gi + 1);
        set_text(&mut b, format!("{key}.label"), name.clone());
        set(&mut b, format!("{key}.n"), c.n as f64);
        set(&mut b, format!("{key}.events"), c.events as f64);
        let times: Vec<f64> = c.steps.iter().map(|s| s.0).collect();
        let surv: Vec<f64> = c.steps.iter().map(|s| s.4).collect();
        let lower: Vec<f64> = c.steps.iter().map(|s| if s.4 > 0.0 { (s.4.ln() - z * s.5).exp() } else { 0.0 }).collect();
        let upper: Vec<f64> = c.steps.iter().map(|s| if s.4 > 0.0 { ((s.4.ln() + z * s.5).exp()).min(1.0) } else { 0.0 }).collect();
        let med = quantile_time(&times, &surv, 0.5);
        // The lower confidence curve reaches 0.5 first, so it gives the lower limit.
        let med_lo = quantile_time(&times, &lower, 0.5);
        let med_hi = quantile_time(&times, &upper, 0.5);
        for (f, v) in [("median", med), ("median.lower", med_lo), ("median.upper", med_hi)] {
            if let Some(v) = v {
                set(&mut b, format!("{key}.{f}"), v);
            }
        }
        summary.push(vec![text(&name), count(c.n), count(c.events), opt(med, 2), opt(med_lo, 2), opt(med_hi, 2)]);
        for (k, s) in c.steps.iter().enumerate() {
            if k == 0 {
                set(&mut b, format!("{key}.first.surv"), s.4);
            }
            steps_rows.push(vec![if k == 0 { text(&name) } else { Cell::Empty }, num(s.0, 2), count(s.1), count(s.2), num(s.4, 4), num(s.4 * s.5, 4), num(lower[k], 4), num(upper[k], 4)]);
        }
        if let Some(last) = c.steps.last() {
            set(&mut b, format!("{key}.last.surv"), last.4);
        }
        // The step curve, from 1 at time 0.
        let mut pts = vec![(0.0, 1.0)];
        let mut prev = 1.0;
        for s in &c.steps {
            pts.push((s.0, prev));
            pts.push((s.0, s.4));
            prev = s.4;
        }
        if let Some(maxt) = data.iter().map(|x| x.0).fold(None, |m: Option<f64>, t| Some(m.map_or(t, |v| v.max(t)))) {
            pts.push((maxt, prev));
        }
        curves.push((name, pts));
    }
    b.tables.push(Table { id: "summary".into(), title: "Survival summary".into(), columns: vec![g_label.clone().unwrap_or_else(|| "Group".into()), "N".into(), "Events".into(), "Median".into(), format!("{lv} CI lower"), format!("{lv} CI upper")], rows: summary, footnotes: vec!["A blank median means survival never fell below 0.5.".into()] });
    b.tables.push(Table { id: "steps".into(), title: "Survival table".into(), columns: vec!["Group".into(), "Time".into(), "At risk".into(), "Events".into(), "Survival".into(), "SE".into(), format!("{lv} CI lower"), format!("{lv} CI upper")], rows: steps_rows, footnotes: Vec::new() });
    // Log-rank.
    if present.len() >= 2 {
        let k = present.len();
        let gidx = |r: usize| present.iter().position(|&g| g == group_of(r)).expect("present");
        let mut times: Vec<f64> = rows.iter().filter(|x| x.2).map(|x| x.1).collect();
        times.sort_by(f64::total_cmp);
        times.dedup();
        let mut o = vec![0.0; k];
        let mut e = vec![0.0; k];
        let mut v = vec![vec![0.0; k]; k];
        for &t in &times {
            let risk: Vec<f64> = (0..k).map(|g| rows.iter().filter(|x| x.1 >= t && gidx(x.0) == g).count() as f64).collect();
            let dth: Vec<f64> = (0..k).map(|g| rows.iter().filter(|x| x.1 == t && x.2 && gidx(x.0) == g).count() as f64).collect();
            let (nt, dt): (f64, f64) = (risk.iter().sum(), dth.iter().sum());
            for g in 0..k {
                o[g] += dth[g];
                e[g] += dt * risk[g] / nt;
                if nt > 1.0 {
                    for h in 0..k {
                        v[g][h] += dt * (nt - dt) / (nt - 1.0) * risk[g] / nt * (if g == h { 1.0 } else { 0.0 } - risk[h] / nt);
                    }
                }
            }
        }
        let sub: Mat = v[..k - 1].iter().map(|r| r[..k - 1].to_vec()).collect();
        if let Some(inv) = invert(&sub) {
            let dvec: Vec<f64> = (0..k - 1).map(|g| o[g] - e[g]).collect();
            let chi: f64 = (0..k - 1).map(|a| (0..k - 1).map(|c| dvec[a] * inv[a][c] * dvec[c]).sum::<f64>()).sum();
            let df = (k - 1) as f64;
            let pv = chisq_sf(chi, df);
            set(&mut b, "logrank.chi2", chi);
            set(&mut b, "logrank.df", df);
            set(&mut b, "logrank.p", pv);
            b.tables.push(Table {
                id: "logrank".into(),
                title: "Log-rank test".into(),
                columns: vec!["Group".into(), "Observed events".into(), "Expected events".into()],
                rows: (0..k).map(|g| vec![text(&levels[present[g]].label), num(o[g], 0), num(e[g], 2)]).collect(),
                footnotes: vec![format!("χ²({}) = {}, p {}.", apa::df(df), apa::num(chi, 3), apa::p_clause(pv))],
            });
        }
    }
    if spec.plot {
        let alt = format!("Kaplan-Meier survival curves of {t_label}{}.", g_label.as_ref().map_or(String::new(), |g| format!(" by {g}")));
        b.charts.push(Chart { id: "km".into(), title: "Survival curves".into(), alt: alt.clone(), svg: chart::scatter_curves(&[(0.0, 0.0), (curves.iter().flat_map(|c| c.1.iter().map(|p| p.0)).fold(0.0, f64::max), 1.0)], &curves, &t_label, "Survival probability", &alt) });
    }
    Ok(b)
}

// ---------------------------------------------------------------- Cox regression

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoxSpec {
    pub time: String,
    pub status: String,
    pub events: Vec<f64>,
    #[serde(default)]
    pub covariates: Vec<String>,
    #[serde(default)]
    pub factors: Vec<Factor>,
    #[serde(default = "default_level")]
    pub level: f64,
}

/// Efron's partial log-likelihood with gradient and Hessian.
fn cox_ll(beta: &[f64], t: &[f64], e: &[bool], x: &[Vec<f64>]) -> (f64, Vec<f64>, Mat) {
    let p = beta.len();
    let n = t.len();
    let eta: Vec<f64> = (0..n).map(|i| (0..p).map(|j| beta[j] * x[j][i]).sum()).collect();
    let w: Vec<f64> = eta.iter().map(|v| v.exp()).collect();
    let mut times: Vec<f64> = (0..n).filter(|&i| e[i]).map(|i| t[i]).collect();
    times.sort_by(f64::total_cmp);
    times.dedup();
    let mut ll = 0.0;
    let mut g = vec![0.0; p];
    let mut h = vec![vec![0.0; p]; p];
    for &tt in &times {
        let risk: Vec<usize> = (0..n).filter(|&i| t[i] >= tt).collect();
        let dead: Vec<usize> = (0..n).filter(|&i| t[i] == tt && e[i]).collect();
        let d = dead.len() as f64;
        let s0r: f64 = risk.iter().map(|&i| w[i]).sum();
        let s0d: f64 = dead.iter().map(|&i| w[i]).sum();
        let s1r: Vec<f64> = (0..p).map(|j| risk.iter().map(|&i| w[i] * x[j][i]).sum()).collect();
        let s1d: Vec<f64> = (0..p).map(|j| dead.iter().map(|&i| w[i] * x[j][i]).sum()).collect();
        let s2r: Mat = (0..p).map(|a| (0..p).map(|c| risk.iter().map(|&i| w[i] * x[a][i] * x[c][i]).sum()).collect()).collect();
        let s2d: Mat = (0..p).map(|a| (0..p).map(|c| dead.iter().map(|&i| w[i] * x[a][i] * x[c][i]).sum()).collect()).collect();
        for &i in &dead {
            ll += eta[i];
            for j in 0..p {
                g[j] += x[j][i];
            }
        }
        for l in 0..dead.len() {
            let f = l as f64 / d;
            let phi = s0r - f * s0d;
            ll -= phi.ln();
            let a: Vec<f64> = (0..p).map(|j| (s1r[j] - f * s1d[j]) / phi).collect();
            for j in 0..p {
                g[j] -= a[j];
                for k in 0..p {
                    h[j][k] -= (s2r[j][k] - f * s2d[j][k]) / phi - a[j] * a[k];
                }
            }
        }
    }
    (ll, g, h)
}

pub fn cox(ds: &Dataset, spec: &CoxSpec) -> Result<ResultBundle> {
    check_level(spec.level)?;
    if spec.covariates.is_empty() && spec.factors.is_empty() {
        return Err(Error::Invalid("Choose at least one predictor.".into()));
    }
    // The design, with time as the outcome so its missing values are handled, then the status.
    let reg = regression::Spec { dependent: spec.time.clone(), covariates: spec.covariates.clone(), factors: spec.factors.clone(), level: spec.level, blocks: Vec::new(), interactions: Vec::new(), center: false, plots: false, robust: None, bayes: false, prior_scale: None };
    let pr = regression::prepare(ds, &reg)?;
    let mut tally = pr.tally;
    let rows = time_status(ds, &spec.time, &spec.status, &spec.events, &mut tally, &pr.used)?;
    let keep: Vec<usize> = rows.iter().map(|(r, _, _)| pr.used.iter().position(|u| u == r).expect("used")).collect();
    let t: Vec<f64> = rows.iter().map(|x| x.1).collect();
    let e: Vec<bool> = rows.iter().map(|x| x.2).collect();
    let x: Vec<Vec<f64>> = pr.columns.iter().map(|c| keep.iter().map(|&i| c[i]).collect()).collect();
    let n = t.len();
    let p = x.len();
    let lv = level_text(spec.level);
    let note = format!("Cox proportional hazards regression with Efron's method for tied times (as R's coxph); hazard ratios with {lv} Wald intervals. Cases were used when time, status and every predictor had valid values. Computed by PrismStatistics.");
    let mut b = common::bundle(COX, METHOD_VERSION, format!("Cox Regression: {}", pr.y_label), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(pr.total, n), note);
    let events = e.iter().filter(|v| **v).count();
    set(&mut b, "n", n as f64);
    set(&mut b, "events", events as f64);
    set(&mut b, "level", spec.level);
    set_text(&mut b, "time.label", pr.y_label.clone());
    if events < 2 || n <= p {
        fail(&mut b, format!("The model needs at least two events and more cases than predictors; there are {events} events among {n} cases."));
        return Ok(b);
    }
    let (ll0, g0, h0) = cox_ll(&vec![0.0; p], &t, &e, &x);
    let mut beta = vec![0.0; p];
    let (mut ll, mut g, mut h) = (ll0, g0.clone(), h0.clone());
    for _ in 0..100 {
        let Some(inv) = invert(&h.iter().map(|r| r.iter().map(|v| -v).collect()).collect()) else { break };
        let step: Vec<f64> = inv.iter().map(|r| r.iter().zip(&g).map(|(a, c)| a * c).sum()).collect();
        let mut tt = 1.0;
        let mut moved = false;
        for _ in 0..30 {
            let cand: Vec<f64> = beta.iter().zip(&step).map(|(a, s)| a + tt * s).collect();
            let (lc, gc, hc) = cox_ll(&cand, &t, &e, &x);
            if lc >= ll - 1e-12 {
                beta = cand;
                ll = lc;
                g = gc;
                h = hc;
                moved = true;
                break;
            }
            tt /= 2.0;
        }
        if !moved || g.iter().map(|v| v.abs()).fold(0.0, f64::max) < 1e-10 {
            break;
        }
    }
    let Some(vcov) = invert(&h.iter().map(|r| r.iter().map(|v| -v).collect()).collect()) else {
        fail(&mut b, "The information matrix is singular: a predictor may separate the event times perfectly, or the predictors are collinear.");
        return Ok(b);
    };
    let q = norm_quantile(1.0 - (1.0 - spec.level) / 2.0);
    let mut rows_t = Vec::new();
    for j in 0..p {
        let se = vcov[j][j].sqrt();
        let z = beta[j] / se;
        let pv = norm_two_sided(z);
        let key = format!("term.{}", j + 1);
        set_text(&mut b, format!("{key}.label"), pr.names[j].clone());
        for (f, v) in [("b", beta[j]), ("se", se), ("z", z), ("p", pv), ("hr", beta[j].exp()), ("hr.lower", (beta[j] - q * se).exp()), ("hr.upper", (beta[j] + q * se).exp())] {
            set(&mut b, format!("{key}.{f}"), v);
        }
        rows_t.push(vec![text(&pr.names[j]), num(beta[j], 4), num(se, 4), num(z, 3), common::p(pv), num(beta[j].exp(), 3), num((beta[j] - q * se).exp(), 3), num((beta[j] + q * se).exp(), 3)]);
    }
    b.tables.push(Table { id: "coefficients".into(), title: "Coefficients".into(), columns: vec!["Predictor".into(), "b".into(), "SE".into(), "z".into(), "p".into(), "Hazard ratio".into(), format!("{lv} CI lower"), format!("{lv} CI upper")], rows: rows_t, footnotes: vec!["A hazard ratio above 1 means a higher hazard (earlier events) for higher values or against the reference category.".into()] });
    let lr = 2.0 * (ll - ll0);
    let info: Mat = h.iter().map(|r| r.iter().map(|v| -v).collect()).collect();
    let wald: f64 = (0..p).map(|a| (0..p).map(|c| beta[a] * info[a][c] * beta[c]).sum::<f64>()).sum();
    let score = invert(&h0.iter().map(|r| r.iter().map(|v| -v).collect()).collect()).map(|i0| (0..p).map(|a| (0..p).map(|c| g0[a] * i0[a][c] * g0[c]).sum::<f64>()).sum::<f64>());
    let df = p as f64;
    let mut rows_f = Vec::new();
    for (name, key, v) in [("Likelihood ratio", "lr", Some(lr)), ("Wald", "wald", Some(wald)), ("Score (log-rank)", "score", score)] {
        if let Some(v) = v {
            set(&mut b, format!("{key}.chi2"), v);
            set(&mut b, format!("{key}.p"), chisq_sf(v, df));
            rows_f.push(vec![text(name), num(v, 3), num(df, 0), common::p(chisq_sf(v, df))]);
        }
    }
    set(&mut b, "df", df);
    set(&mut b, "loglik", ll);
    b.tables.push(Table { id: "tests".into(), title: "Tests of the model".into(), columns: vec!["Test".into(), "Chi-square".into(), "df".into(), "p".into()], rows: rows_f, footnotes: Vec::new() });
    b.diagnostics.push(not_assessed("ph", "Proportional hazards", "The model assumes each predictor multiplies the hazard by the same amount at every time. Compare Kaplan-Meier curves by group: curves that cross suggest the assumption fails."));
    Ok(b)
}

// ---------------------------------------------------------------- ROC

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RocSpec {
    /// Test variables (higher values should point to the positive state).
    pub variables: Vec<String>,
    pub state: String,
    /// The state variable's code for the positive state.
    pub positive: f64,
    #[serde(default = "yes")]
    pub plot: bool,
    #[serde(default = "default_level")]
    pub level: f64,
}

pub fn roc(ds: &Dataset, spec: &RocSpec) -> Result<ResultBundle> {
    if spec.variables.is_empty() {
        return Err(Error::Invalid("Choose at least one test variable.".into()));
    }
    check_level(spec.level)?;
    let state = numeric(ds, &spec.state)?;
    let s_label = ds.variable(&spec.state)?.display_name().to_string();
    let pos_label = ds.variable(&spec.state)?.value_text(spec.positive);
    let lv = level_text(spec.level);
    let note = format!("ROC analysis of each test variable against {s_label}, with {pos_label} as the positive state and higher test values taken to point to it. The area under the curve has DeLong's standard error and {lv} interval (as R's pROC); the test of AUC = 0.5 uses that standard error. The best cutoff maximizes Youden's J (sensitivity + specificity − 1). Cases were used when the state and that test variable were valid. Computed by PrismStatistics.");
    let mut tally = Tally::default();
    let total = state.len();
    let mut b = common::bundle(ROC, METHOD_VERSION, format!("ROC Analysis: {s_label}"), serde_json::to_value(spec).expect("a spec always serializes"), CaseAccountingShim::all(total), note);
    set_text(&mut b, "state.label", s_label.clone());
    set_text(&mut b, "positive.label", pos_label.clone());
    set(&mut b, "level", spec.level);
    let z = norm_quantile(1.0 - (1.0 - spec.level) / 2.0);
    let mut rows = Vec::new();
    let mut curves = Vec::new();
    let mut analyzed = 0;
    for (k, v) in spec.variables.iter().enumerate() {
        let lab = ds.variable(v)?.display_name().to_string();
        let xs = numeric(ds, v)?;
        let (mut pos, mut neg) = (Vec::new(), Vec::new());
        for (s, x) in state.iter().zip(&xs) {
            if tally.check(s, "state") || tally.check(x, "test variable") {
                continue;
            }
            if s.value() == Some(spec.positive) { pos.push(x.value().expect("checked")) } else { neg.push(x.value().expect("checked")) }
        }
        analyzed = analyzed.max(pos.len() + neg.len());
        let key = format!("var.{}", k + 1);
        set_text(&mut b, format!("{key}.label"), lab.clone());
        set(&mut b, format!("{key}.n.pos"), pos.len() as f64);
        set(&mut b, format!("{key}.n.neg"), neg.len() as f64);
        let Some((auc, se)) = super::logistic::delong(&pos, &neg) else {
            rows.push(vec![text(&lab), Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty]);
            continue;
        };
        let (lo, hi) = ((auc - z * se).max(0.0), (auc + z * se).min(1.0));
        let pv = norm_two_sided((auc - 0.5) / se);
        // Youden's J over the cutoffs: positive when the value is at least the cutoff.
        let mut cuts: Vec<f64> = pos.iter().chain(&neg).copied().collect();
        cuts.sort_by(f64::total_cmp);
        cuts.dedup();
        let mut best = (f64::NEG_INFINITY, 0.0, 0.0, 0.0);
        let mut pts = vec![(0.0, 0.0)];
        for w in cuts.windows(2).map(|w| (w[0] + w[1]) / 2.0).chain(std::iter::once(cuts[0] - 1.0)) {
            let sens = pos.iter().filter(|x| **x >= w).count() as f64 / pos.len() as f64;
            let spec_ = neg.iter().filter(|x| **x < w).count() as f64 / neg.len() as f64;
            pts.push((1.0 - spec_, sens));
            if sens + spec_ - 1.0 > best.0 + 1e-12 {
                best = (sens + spec_ - 1.0, w, sens, spec_);
            }
        }
        pts.push((1.0, 1.0));
        pts.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        for (f, val) in [("auc", auc), ("se", se), ("lower", lo), ("upper", hi), ("p", pv), ("youden", best.0), ("cutoff", best.1), ("sens", best.2), ("spec", best.3)] {
            set(&mut b, format!("{key}.{f}"), val);
        }
        rows.push(vec![text(&lab), num(auc, 3), num(se, 3), num(lo, 3), num(hi, 3), p(pv), num(best.1, 3), num(best.2, 3), num(best.3, 3)]);
        curves.push((lab, pts));
    }
    b.cases = tally.accounting(total, analyzed);
    b.tables.push(Table {
        id: "auc".into(),
        title: "Area under the curve".into(),
        columns: vec!["Test variable".into(), "AUC".into(), "SE".into(), format!("{lv} CI lower"), format!("{lv} CI upper"), "p (AUC = 0.5)".into(), "Best cutoff".into(), "Sensitivity".into(), "Specificity".into()],
        rows,
        footnotes: vec![format!("A case counts as positive for {pos_label} when its value is at or above the cutoff. The best cutoff maximizes Youden's J.")],
    });
    if spec.plot && !curves.is_empty() {
        let alt = format!("ROC curves for predicting {pos_label} on {s_label}, with the diagonal of chance.");
        let mut all = curves.clone();
        all.push(("Chance".to_string(), vec![(0.0, 0.0), (1.0, 1.0)]));
        b.charts.push(Chart { id: "roc".into(), title: "ROC curves".into(), alt: alt.clone(), svg: chart::scatter_curves(&[(0.0, 0.0), (1.0, 1.0)], &all, "1 − specificity", "Sensitivity", &alt) });
    }
    Ok(b)
}

/// A placeholder accounting, replaced once the cases are counted.
struct CaseAccountingShim;
impl CaseAccountingShim {
    fn all(total: usize) -> crate::result::CaseAccounting {
        crate::result::CaseAccounting { total, analyzed: total, excluded: Vec::new() }
    }
}
