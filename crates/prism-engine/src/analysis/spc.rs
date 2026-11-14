//! Statistical process control (checklist D-4, part 1): Shewhart control charts and process capability as R's qcc
//! (version 2.7) computes them.
//!
//! * X̄ with R or S: subgroups from a subgroup variable (in sorted order, as qcc.groups) or one row per subgroup with
//!   the measurements in several variables. σ is the mean of R/d₂ (qcc's "UWAVE-R"; with any subgroup over 25, the
//!   pooled "RMSDF" estimate) for X̄ and R charts, and the mean of S/c₄ ("UWAVE-SD") for X̄-S; d₂ and d₃ are qcc's
//!   tables. The R chart is centered on R̄ with limits R̄ ± 3d₃σ, the S chart on S̄ with limits S̄ ± 3σ√(1 − c₄²).
//! * Individuals with the moving range: σ = mean moving range / d₂(2); the moving-range chart is qcc's R chart of
//!   consecutive pairs.
//! * p, np, c and u charts for counts, with qcc's limits (truncated at 0, and at the sample size for np).
//!
//! Points beyond the limits and runs of 7 or more on one side of the center line are flagged, as qcc's
//! shewhart.rules. Capability (X̄ and individuals charts, with specification limits): Cp, Cp_l, Cp_u, Cpk and Cpm with
//! qcc's intervals, expected and observed shares outside the limits, and Pp and Ppk from the overall standard deviation.

#![allow(clippy::needless_range_loop)]

use prismqda_stats::dist::{chisq_quantile, norm_cdf, norm_quantile};
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::apa;
use crate::chart;
use crate::error::{Error, Result};
use crate::model::{Column, Dataset};
use crate::result::{Diagnostic, DiagnosticState, ResultBundle, Table};

pub const SPC: &str = "spc.chart";
pub const METHOD_VERSION: u32 = 1;

/// qcc's d₂ (expected range of n standard normals), n = 1..25.
const D2: [f64; 25] = [f64::NAN, 1.128, 1.693, 2.059, 2.326, 2.534, 2.704, 2.847, 2.970, 3.078, 3.173, 3.258, 3.336, 3.407, 3.472, 3.532, 3.588, 3.640, 3.689, 3.735, 3.778, 3.819, 3.858, 3.895, 3.931];
/// qcc's d₃ (standard deviation of that range), n = 1..50.
const D3: [f64; 50] = [
    f64::NAN, 0.8525033, 0.8883697, 0.8798108, 0.8640855, 0.8480442, 0.8332108, 0.8198378, 0.8078413, 0.7970584, 0.787323, 0.7784873, 0.7704257, 0.763033, 0.7562217, 0.7499188, 0.7440627, 0.7386021, 0.7334929, 0.728698, 0.7241851, 0.7199267, 0.7158987, 0.7120802, 0.7084528, 0.7050004, 0.7017086, 0.6985648, 0.6955576, 0.692677, 0.6899137, 0.6872596, 0.6847074, 0.6822502, 0.6798821, 0.6775973, 0.675391, 0.6732584, 0.6711952, 0.6691976, 0.6672619, 0.6653848, 0.6635632, 0.6617943, 0.6600754, 0.6584041, 0.656778, 0.655195, 0.6536532, 0.6521506,
];

fn c4(n: f64) -> f64 {
    use prismqda_stats::dist::ln_gamma;
    (2.0 / (n - 1.0)).sqrt() * (ln_gamma(n / 2.0) - ln_gamma((n - 1.0) / 2.0)).exp()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ChartType {
    #[default]
    XbarR,
    XbarS,
    Imr,
    P,
    Np,
    C,
    U,
    /// Tabular CUSUM of standardized subgroup means (or individual values).
    Cusum,
    /// Exponentially weighted moving average.
    Ewma,
}

fn three() -> f64 {
    3.0
}
fn seven() -> usize {
    7
}
fn five() -> f64 {
    5.0
}
fn one() -> f64 {
    1.0
}
fn lambda() -> f64 {
    0.2
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    #[serde(default)]
    pub chart: ChartType,
    /// The measurement (X̄, individuals), or several measurement variables, one row per subgroup.
    #[serde(default)]
    pub measures: Vec<String>,
    /// The subgroup of each row (X̄ charts with one measurement variable).
    #[serde(default)]
    pub subgroup: Option<String>,
    /// Defectives (p, np) or defects (c, u) in each sample.
    #[serde(default)]
    pub count: Option<String>,
    /// Each sample's size (p, np, u).
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default = "three")]
    pub nsigmas: f64,
    #[serde(default = "seven")]
    pub run_length: usize,
    /// Specification limits and target for capability (X̄ and individuals charts).
    #[serde(default)]
    pub lsl: Option<f64>,
    #[serde(default)]
    pub usl: Option<f64>,
    #[serde(default)]
    pub target: Option<f64>,
    #[serde(default = "level")]
    pub level: f64,
    /// CUSUM: the decision interval h and the shift to detect (both in standard errors), and the head start.
    #[serde(default = "five")]
    pub decision_interval: f64,
    #[serde(default = "one")]
    pub se_shift: f64,
    #[serde(default)]
    pub head_start: f64,
    /// EWMA: the smoothing weight.
    #[serde(default = "lambda")]
    pub lambda: f64,
    /// CUSUM and EWMA: a known target center and process standard deviation (estimated when left out).
    #[serde(default)]
    pub center: Option<f64>,
    #[serde(default)]
    pub std_dev: Option<f64>,
}

fn level() -> f64 {
    0.95
}

/// One chart: its points, center line, limits (one pair per point) and the rules' flags.
struct Chart {
    key: &'static str,
    name: &'static str,
    labels: Vec<String>,
    stats: Vec<f64>,
    center: Vec<f64>,
    lcl: Vec<f64>,
    ucl: Vec<f64>,
    beyond: Vec<usize>,
    runs: Vec<usize>,
}

/// qcc's rules: points beyond the limits, and points in runs of at least `run` on one side of the center.
fn rules(stats: &[f64], center: &[f64], lcl: &[f64], ucl: &[f64], run: usize) -> (Vec<usize>, Vec<usize>) {
    let mut beyond: Vec<usize> = (0..stats.len()).filter(|&i| stats[i] > ucl[i]).collect();
    beyond.extend((0..stats.len()).filter(|&i| stats[i] < lcl[i]));
    let side: Vec<i8> = stats.iter().zip(center).map(|(s, c)| if s > c { 1 } else if s < c { -1 } else { 0 }).collect();
    let mut runs = Vec::new();
    if run > 0 {
        for dir in [1i8, -1] {
            let mut i = 0;
            while i < side.len() {
                if side[i] != dir {
                    i += 1;
                    continue;
                }
                let start = i;
                while i < side.len() && side[i] == dir {
                    i += 1;
                }
                if i - start >= run {
                    runs.extend(start + run - 1..i);
                }
            }
        }
    }
    (beyond, runs)
}

#[allow(clippy::too_many_arguments)]
fn make(key: &'static str, name: &'static str, labels: Vec<String>, stats: Vec<f64>, center: Vec<f64>, lcl: Vec<f64>, ucl: Vec<f64>, run: usize) -> Chart {
    let (beyond, runs) = rules(&stats, &center, &lcl, &ucl, run);
    Chart { key, name, labels, stats, center, lcl, ucl, beyond, runs }
}

/// Values of a column as text labels (codes or texts).
fn labels_of(ds: &Dataset, name: &str) -> Result<Vec<Option<String>>> {
    let i = ds.find(name).ok_or_else(|| Error::UnknownVariable(name.to_string()))?;
    let var = ds.variable(name)?;
    Ok(match &ds.columns[i] {
        Column::Text(t) => t.clone(),
        Column::Numeric(c) => c.iter().map(|v| v.map(|x| var.value_text(x))).collect(),
    })
}

/// Subgroups (label, measurements): from a measurement and a subgroup variable (sorted, as qcc.groups), or one row per
/// subgroup with the measurements in several variables.
fn subgroups(ds: &Dataset, spec: &Spec, tally: &mut Tally, used: &mut usize) -> Result<Vec<(String, Vec<f64>)>> {
    let mut groups: Vec<(String, Vec<f64>)> = Vec::new();
    let total = ds.n_rows();
    if spec.measures.len() == 1 {
        let Some(sg) = &spec.subgroup else {
            return Err(Error::Invalid("With one measurement variable, choose the subgroup variable.".into()));
        };
        let x = numeric(ds, &spec.measures[0])?;
        let labs = labels_of(ds, sg)?;
        let codes = numeric(ds, sg).ok();
        // Sorted subgroups, as qcc.groups (numerically for numeric identifiers).
        let mut keys: Vec<(Option<f64>, String)> = Vec::new();
        let mut rows: Vec<Vec<f64>> = Vec::new();
        for r in 0..total {
            if tally.check(&x[r], &spec.measures[0]) {
                continue;
            }
            let Some(l) = labs[r].clone() else {
                tally.add(format!("no {sg}"));
                continue;
            };
            let code = codes.as_ref().and_then(|c| c[r].value());
            let at = match keys.iter().position(|(_, kk)| *kk == l) {
                Some(a) => a,
                None => {
                    keys.push((code, l));
                    rows.push(Vec::new());
                    keys.len() - 1
                }
            };
            rows[at].push(x[r].value().expect("checked"));
            *used += 1;
        }
        let mut order: Vec<usize> = (0..keys.len()).collect();
        order.sort_by(|&a, &b| match (keys[a].0, keys[b].0) {
            (Some(p), Some(q)) => p.total_cmp(&q),
            _ => keys[a].1.cmp(&keys[b].1),
        });
        groups = order.into_iter().map(|g| (keys[g].1.clone(), rows[g].clone())).collect();
    } else {
        let cols: Vec<Vec<Slot<f64>>> = spec.measures.iter().map(|m| numeric(ds, m)).collect::<Result<_>>()?;
        for r in 0..total {
            let v: Vec<f64> = cols.iter().filter_map(|c| c[r].value()).collect();
            if v.len() < 2 {
                tally.add("fewer than two measurements");
                continue;
            }
            *used += 1;
            groups.push(((r + 1).to_string(), v));
        }
    }
    Ok(groups)
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    common::check_level(spec.level)?;
    let k = spec.nsigmas;
    let total = ds.n_rows();
    let mut tally = Tally::default();
    let mut charts: Vec<Chart> = Vec::new();
    // The individual measurements (for capability), the σ used, and the capability-eligible center.
    let mut values: Vec<f64> = Vec::new();
    let mut sigma = f64::NAN;
    let mut xcenter = f64::NAN;
    let mut used = 0;
    let (title, note): (String, String);
    match spec.chart {
        ChartType::XbarR | ChartType::XbarS => {
            if spec.measures.is_empty() {
                return Err(Error::Invalid("Choose the measurement variable (with a subgroup variable), or the measurements of each subgroup in separate variables.".into()));
            }
            let groups = subgroups(ds, spec, &mut tally, &mut used)?;
            let sizes: Vec<f64> = groups.iter().map(|g| g.1.len() as f64).collect();
            if groups.len() < 2 || sizes.iter().any(|s| *s < 2.0) {
                return Err(Error::Invalid("Each of at least two subgroups needs two or more measurements.".into()));
            }
            let means: Vec<f64> = groups.iter().map(|g| g.1.iter().sum::<f64>() / g.1.len() as f64).collect();
            let ranges: Vec<f64> = groups.iter().map(|g| g.1.iter().copied().fold(f64::NEG_INFINITY, f64::max) - g.1.iter().copied().fold(f64::INFINITY, f64::min)).collect();
            let sds: Vec<f64> = groups.iter().zip(&means).map(|(g, m)| (g.1.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (g.1.len() as f64 - 1.0)).sqrt()).collect();
            let ng = groups.len() as f64;
            let large = sizes.iter().any(|s| *s > 25.0);
            sigma = if spec.chart == ChartType::XbarS {
                sds.iter().zip(&sizes).map(|(s, n)| s / c4(*n)).sum::<f64>() / ng
            } else if large {
                let w: f64 = sizes.iter().map(|n| n - 1.0).sum();
                (sds.iter().zip(&sizes).map(|(s, n)| s * s * (n - 1.0)).sum::<f64>() / w).sqrt() / c4(w + 1.0)
            } else {
                ranges.iter().zip(&sizes).map(|(r, n)| r / D2[*n as usize - 1]).sum::<f64>() / ng
            };
            let tot: f64 = sizes.iter().sum();
            xcenter = means.iter().zip(&sizes).map(|(m, n)| m * n).sum::<f64>() / tot;
            values = groups.iter().flat_map(|g| g.1.iter().copied()).collect();
            let labels: Vec<String> = groups.iter().map(|g| g.0.clone()).collect();
            let m = groups.len();
            let half: Vec<f64> = sizes.iter().map(|n| k * sigma / n.sqrt()).collect();
            charts.push(make("xbar", "X̄", labels.clone(), means, vec![xcenter; m], (0..m).map(|i| xcenter - half[i]).collect(), (0..m).map(|i| xcenter + half[i]).collect(), spec.run_length));
            if spec.chart == ChartType::XbarR {
                if sizes.iter().any(|s| *s > 50.0) {
                    return Err(Error::Invalid("The R chart needs subgroups of at most 50; use the S chart.".into()));
                }
                let rbar = ranges.iter().zip(&sizes).map(|(r, n)| r * n).sum::<f64>() / tot;
                let se: Vec<f64> = sizes.iter().map(|n| D3[*n as usize - 1] * sigma).collect();
                charts.push(make("r", "R", labels, ranges, vec![rbar; m], se.iter().map(|s| (rbar - k * s).max(0.0)).collect(), se.iter().map(|s| rbar + k * s).collect(), spec.run_length));
            } else {
                let sbar = sds.iter().zip(&sizes).map(|(s, n)| s * n).sum::<f64>() / tot;
                let se: Vec<f64> = sizes.iter().map(|n| sigma * (1.0 - c4(*n).powi(2)).sqrt()).collect();
                charts.push(make("s", "S", labels, sds, vec![sbar; m], se.iter().map(|s| (sbar - k * s).max(0.0)).collect(), se.iter().map(|s| sbar + k * s).collect(), spec.run_length));
            }
            title = format!("Control Charts: X̄ and {}", if spec.chart == ChartType::XbarR { "R" } else { "S" });
            note = format!(
                "Shewhart X̄ and {} charts as R's qcc: σ estimated as {}; limits at ±{} standard errors. Flags: points beyond the limits, and runs of {} or more on one side of the center line. Computed by PrismStatistics.",
                if spec.chart == ChartType::XbarR { "R" } else { "S" },
                if spec.chart == ChartType::XbarS { "the mean of S/c₄" } else if large { "the pooled standard deviation / c₄ (subgroups over 25)" } else { "the mean of R/d₂" },
                apa::num(k, 1),
                spec.run_length
            );
        }
        ChartType::Imr => {
            let Some(name) = spec.measures.first() else {
                return Err(Error::Invalid("Choose the measurement variable.".into()));
            };
            let x = numeric(ds, name)?;
            let mut labels = Vec::new();
            for r in 0..total {
                if tally.check(&x[r], name) {
                    continue;
                }
                values.push(x[r].value().expect("checked"));
                labels.push((r + 1).to_string());
            }
            used = values.len();
            if values.len() < 3 {
                return Err(Error::Invalid("The individuals chart needs at least three observations.".into()));
            }
            let n = values.len();
            let mr: Vec<f64> = (1..n).map(|i| (values[i] - values[i - 1]).abs()).collect();
            sigma = mr.iter().sum::<f64>() / mr.len() as f64 / D2[1];
            xcenter = values.iter().sum::<f64>() / n as f64;
            charts.push(make("i", "Individuals", labels.clone(), values.clone(), vec![xcenter; n], vec![xcenter - k * sigma; n], vec![xcenter + k * sigma; n], spec.run_length));
            // qcc's R chart of consecutive pairs: center = mean MR; σ the same; limits mean MR ± k d₃(2) σ.
            let mbar = mr.iter().sum::<f64>() / mr.len() as f64;
            let m = mr.len();
            charts.push(make("mr", "Moving range", labels[1..].to_vec(), mr, vec![mbar; m], vec![(mbar - k * D3[1] * sigma).max(0.0); m], vec![mbar + k * D3[1] * sigma; m], spec.run_length));
            title = "Control Charts: Individuals and Moving Range".into();
            note = format!("Individuals and moving-range charts as R's qcc: σ = mean moving range / d₂(2); limits at ±{} σ. Flags: points beyond the limits, and runs of {} or more on one side of the center line. Computed by PrismStatistics.", apa::num(k, 1), spec.run_length);
        }
        ChartType::Cusum | ChartType::Ewma => {
            if spec.measures.is_empty() {
                return Err(Error::Invalid("Choose the measurement variable (with a subgroup variable for subgroup means).".into()));
            }
            // Subgroups, or individual values (one measurement variable and no subgroup variable).
            let groups: Vec<(String, Vec<f64>)> = if spec.measures.len() == 1 && spec.subgroup.is_none() {
                let x = numeric(ds, &spec.measures[0])?;
                let mut g = Vec::new();
                for r in 0..total {
                    if tally.check(&x[r], &spec.measures[0]) {
                        continue;
                    }
                    g.push(((r + 1).to_string(), vec![x[r].value().expect("checked")]));
                }
                used = g.len();
                g
            } else {
                subgroups(ds, spec, &mut tally, &mut used)?
            };
            let m = groups.len();
            if m < 3 {
                return Err(Error::Invalid("The chart needs at least three samples.".into()));
            }
            let sizes: Vec<f64> = groups.iter().map(|g| g.1.len() as f64).collect();
            let single = sizes.contains(&1.0);
            if single && sizes.iter().any(|s| *s > 1.0) && spec.std_dev.is_none() {
                return Err(Error::Invalid("Some subgroups have a single measurement: give the process standard deviation, or use subgroups of two or more.".into()));
            }
            let stats: Vec<f64> = groups.iter().map(|g| g.1.iter().sum::<f64>() / g.1.len() as f64).collect();
            let tot: f64 = sizes.iter().sum();
            let center = spec.center.unwrap_or(stats.iter().zip(&sizes).map(|(m, n)| m * n).sum::<f64>() / tot);
            xcenter = center;
            // σ as qcc: the mean moving range / d₂(2) for individual values; R/d₂ (or the pooled SD over 25) for subgroups.
            sigma = match spec.std_dev {
                Some(s) => s,
                None if single => (1..m).map(|i| (stats[i] - stats[i - 1]).abs()).sum::<f64>() / (m - 1) as f64 / D2[1],
                None if sizes.iter().any(|s| *s > 25.0) => {
                    let sds: Vec<f64> = groups.iter().zip(&stats).map(|(g, mu)| (g.1.iter().map(|v| (v - mu).powi(2)).sum::<f64>() / (g.1.len() as f64 - 1.0)).sqrt()).collect();
                    let w: f64 = sizes.iter().map(|n| n - 1.0).sum();
                    (sds.iter().zip(&sizes).map(|(s, n)| s * s * (n - 1.0)).sum::<f64>() / w).sqrt() / c4(w + 1.0)
                }
                None => groups.iter().map(|g| (g.1.iter().copied().fold(f64::NEG_INFINITY, f64::max) - g.1.iter().copied().fold(f64::INFINITY, f64::min)) / D2[g.1.len() - 1]).sum::<f64>() / m as f64,
            };
            if sigma.is_nan() || sigma <= 0.0 {
                return Err(Error::Invalid("The process standard deviation must be positive.".into()));
            }
            let labels: Vec<String> = groups.iter().map(|g| g.0.clone()).collect();
            let what = if single { "individual values" } else { "subgroup means" };
            if spec.chart == ChartType::Cusum {
                let h = spec.decision_interval;
                if h.is_nan() || h <= 0.0 || spec.head_start.is_nan() || spec.head_start < 0.0 || spec.head_start >= h || spec.se_shift.is_nan() || spec.se_shift < 0.0 {
                    return Err(Error::Invalid("The decision interval must be positive, the shift zero or more, and the head start from 0 up to the decision interval.".into()));
                }
                let z: Vec<f64> = (0..m).map(|i| (stats[i] - center) / (sigma / sizes[i].sqrt())).collect();
                let half = spec.se_shift / 2.0;
                let (mut pos, mut neg) = (vec![0.0; m], vec![0.0; m]);
                for i in 0..m {
                    let (pp, pn) = if i == 0 { (spec.head_start, spec.head_start) } else { (pos[i - 1], -neg[i - 1]) };
                    pos[i] = (pp + z[i] - half).max(0.0);
                    neg[i] = -(pn - z[i] - half).max(0.0);
                }
                charts.push(make("cusum_pos", "Upper CUSUM", labels.clone(), pos, vec![0.0; m], vec![f64::NEG_INFINITY; m], vec![h; m], 0));
                charts.push(make("cusum_neg", "Lower CUSUM", labels, neg, vec![0.0; m], vec![-h; m], vec![f64::INFINITY; m], 0));
                title = "Control Chart: CUSUM".into();
                note = format!(
                    "Tabular CUSUM of standardized {what} as R's qcc: z = (x̄ − {}) / (σ/√n) with σ = {}; the upper and lower sums accumulate z − {} and −z − {} (half the {}-standard-error shift to detect), from a head start of {}, and signal beyond the decision interval ±{}. Computed by PrismStatistics.",
                    apa::num(center, 4),
                    apa::num(sigma, 4),
                    apa::num(half, 2),
                    apa::num(half, 2),
                    apa::num(spec.se_shift, 2),
                    apa::num(spec.head_start, 2),
                    apa::num(h, 2)
                );
            } else {
                let l = spec.lambda;
                if l.is_nan() || l <= 0.0 || l > 1.0 {
                    return Err(Error::Invalid("The EWMA weight λ must be above 0 and at most 1.".into()));
                }
                let mut y = vec![0.0; m];
                let (mut lcl, mut ucl) = (vec![0.0; m], vec![0.0; m]);
                for i in 0..m {
                    y[i] = l * stats[i] + (1.0 - l) * if i == 0 { center } else { y[i - 1] };
                    let se = (sigma * sigma / sizes[i] * (l / (2.0 - l)) * (1.0 - (1.0 - l).powi(2 * (i as i32 + 1)))).sqrt();
                    lcl[i] = center - k * se;
                    ucl[i] = center + k * se;
                }
                charts.push(make("ewma", "EWMA", labels, y, vec![center; m], lcl, ucl, 0));
                title = "Control Chart: EWMA".into();
                note = format!(
                    "Exponentially weighted moving average of {what} as R's qcc: z₀ = {}, zᵢ = λx̄ᵢ + (1 − λ)zᵢ₋₁ with λ = {}; limits at ±{} standard errors of zᵢ, with σ = {}. Flags: points beyond the limits. Computed by PrismStatistics.",
                    apa::num(center, 4),
                    apa::num(l, 2),
                    apa::num(k, 1),
                    apa::num(sigma, 4)
                );
            }
        }
        ChartType::P | ChartType::Np | ChartType::C | ChartType::U => {
            let Some(cn) = &spec.count else {
                return Err(Error::Invalid("Choose the variable holding each sample's count.".into()));
            };
            let need_size = spec.chart != ChartType::C;
            if need_size && spec.size.is_none() {
                return Err(Error::Invalid("Choose the variable holding each sample's size.".into()));
            }
            let x = numeric(ds, cn)?;
            let sz = spec.size.as_ref().map(|s| numeric(ds, s)).transpose()?;
            let (mut d, mut nn, mut labels) = (Vec::new(), Vec::new(), Vec::new());
            for r in 0..total {
                if tally.check(&x[r], cn) {
                    continue;
                }
                let s = match &sz {
                    Some(c) => {
                        if tally.check(&c[r], spec.size.as_deref().unwrap_or("size")) {
                            continue;
                        }
                        c[r].value().expect("checked")
                    }
                    None => 1.0,
                };
                d.push(x[r].value().expect("checked"));
                nn.push(s);
                labels.push((r + 1).to_string());
            }
            used = d.len();
            let m = d.len();
            if m < 2 {
                return Err(Error::Invalid("A chart needs at least two samples.".into()));
            }
            if d.iter().any(|v| *v < 0.0) || nn.iter().any(|v| *v <= 0.0) {
                return Err(Error::Invalid("Counts must be zero or more and sizes positive.".into()));
            }
            let pbar = d.iter().sum::<f64>() / nn.iter().sum::<f64>();
            match spec.chart {
                ChartType::P => {
                    if d.iter().zip(&nn).any(|(a, b)| a > b) {
                        return Err(Error::Invalid("A count of defectives exceeds its sample size.".into()));
                    }
                    let stats: Vec<f64> = d.iter().zip(&nn).map(|(a, b)| a / b).collect();
                    let tol: Vec<f64> = nn.iter().map(|n| k * (pbar * (1.0 - pbar) * n).sqrt()).collect();
                    let lcl = (0..m).map(|i| (pbar * nn[i] - tol[i]).max(0.0) / nn[i]).collect();
                    let ucl = (0..m).map(|i| (pbar * nn[i] + tol[i]).min(nn[i]) / nn[i]).collect();
                    charts.push(make("p", "Proportion defective", labels, stats, vec![pbar; m], lcl, ucl, spec.run_length));
                }
                ChartType::Np => {
                    let center: Vec<f64> = nn.iter().map(|n| n * pbar).collect();
                    let pb = center.iter().zip(&nn).map(|(c, n)| c / n).sum::<f64>() / m as f64;
                    let tol: Vec<f64> = nn.iter().map(|n| k * (pb * (1.0 - pb) * n).sqrt()).collect();
                    let lcl = (0..m).map(|i| (center[i] - tol[i]).max(0.0)).collect();
                    let ucl = (0..m).map(|i| (center[i] + tol[i]).min(nn[i])).collect();
                    charts.push(make("np", "Number defective", labels, d.clone(), center, lcl, ucl, spec.run_length));
                }
                ChartType::C => {
                    let c = d.iter().sum::<f64>() / m as f64;
                    charts.push(make("c", "Defects", labels, d.clone(), vec![c; m], vec![(c - k * c.sqrt()).max(0.0); m], vec![c + k * c.sqrt(); m], spec.run_length));
                }
                _ => {
                    let stats: Vec<f64> = d.iter().zip(&nn).map(|(a, b)| a / b).collect();
                    let lcl = (0..m).map(|i| (pbar * nn[i] - k * (pbar * nn[i]).sqrt()).max(0.0) / nn[i]).collect();
                    let ucl = (0..m).map(|i| (pbar * nn[i] + k * (pbar * nn[i]).sqrt()) / nn[i]).collect();
                    charts.push(make("u", "Defects per unit", labels, stats, vec![pbar; m], lcl, ucl, spec.run_length));
                }
            }
            title = format!("Control Chart: {}", match spec.chart {
                ChartType::P => "p",
                ChartType::Np => "np",
                ChartType::C => "c",
                _ => "u",
            });
            note = format!("Attribute control chart as R's qcc, with limits at ±{} standard deviations (truncated at 0{}). Flags: points beyond the limits, and runs of {} or more on one side of the center line. Computed by PrismStatistics.", apa::num(k, 1), if spec.chart == ChartType::Np { " and at the sample size" } else { "" }, spec.run_length);
        }
    }
    let mut b = common::bundle(SPC, METHOD_VERSION, title, serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, used), note);
    set(&mut b, "n", used as f64);
    if sigma.is_finite() {
        set(&mut b, "std_dev", sigma);
    }
    let mut summary = Vec::new();
    let mut flagged_total = 0;
    let smoothed = matches!(spec.chart, ChartType::Cusum | ChartType::Ewma);
    for c in &charts {
        let key = c.key;
        set(&mut b, format!("{key}.center"), c.center[0]);
        set(&mut b, format!("{key}.count"), c.stats.len() as f64);
        for i in 0..c.stats.len() {
            set(&mut b, format!("{key}.stat.{}", i + 1), c.stats[i]);
            if c.lcl[i].is_finite() {
                set(&mut b, format!("{key}.lcl.{}", i + 1), c.lcl[i]);
            }
            if c.ucl[i].is_finite() {
                set(&mut b, format!("{key}.ucl.{}", i + 1), c.ucl[i]);
            }
        }
        let mut beyond = c.beyond.clone();
        beyond.sort_unstable();
        let mut runs = c.runs.clone();
        runs.sort_unstable();
        set(&mut b, format!("{key}.beyond"), beyond.len() as f64);
        set(&mut b, format!("{key}.runs"), runs.len() as f64);
        set_text(&mut b, format!("{key}.beyond.points"), beyond.iter().map(|i| c.labels[*i].clone()).collect::<Vec<_>>().join(", "));
        set_text(&mut b, format!("{key}.runs.points"), runs.iter().map(|i| c.labels[*i].clone()).collect::<Vec<_>>().join(", "));
        flagged_total += beyond.len() + runs.len();
        let close = |w: &[f64]| w[0] == w[1] || (w[0] - w[1]).abs() < 1e-12;
        let same = c.lcl.windows(2).all(close) && c.ucl.windows(2).all(close);
        summary.push(vec![
            text(c.name),
            count(c.stats.len()),
            num(c.center[0], 4),
            if !c.lcl[0].is_finite() { text("—") } else if same { num(c.lcl[0], 4) } else { text("varies") },
            if !c.ucl[0].is_finite() { text("—") } else if same { num(c.ucl[0], 4) } else { text("varies") },
            text(if beyond.is_empty() { "none".to_string() } else { beyond.iter().map(|i| c.labels[*i].clone()).collect::<Vec<_>>().join(", ") }),
            text(if smoothed { "—".to_string() } else if runs.is_empty() { "none".to_string() } else { runs.iter().map(|i| c.labels[*i].clone()).collect::<Vec<_>>().join(", ") }),
        ]);
        let mut rows = Vec::new();
        for i in 0..c.stats.len() {
            let flag = if c.beyond.contains(&i) { "beyond limits" } else if c.runs.contains(&i) { "run" } else { "" };
            let lim = |v: f64| if v.is_finite() { num(v, 4) } else { text("—") };
            rows.push(vec![text(c.labels[i].clone()), num(c.stats[i], 4), lim(c.lcl[i]), num(c.center[i], 4), lim(c.ucl[i]), text(flag)]);
        }
        b.tables.push(Table { id: format!("{key}_points"), title: format!("{} chart", c.name), columns: vec!["Sample".into(), "Value".into(), "LCL".into(), "Center".into(), "UCL".into(), "Flag".into()], rows, footnotes: Vec::new() });
        if key == "cusum_neg" {
            continue;
        }
        if key == "cusum_pos" {
            let lower = charts.iter().find(|x| x.key == "cusum_neg").expect("both CUSUM sums");
            let flag = |ch: &Chart| (0..ch.stats.len()).map(|i| ch.beyond.contains(&i)).collect::<Vec<bool>>();
            let n_flag = c.beyond.len() + lower.beyond.len();
            let alt = format!("CUSUM chart of {} samples: upper and lower cumulative sums against the decision interval ±{}; {}.", c.stats.len(), apa::num(c.ucl[0], 1), if n_flag == 0 { "no signals".to_string() } else { format!("{n_flag} points beyond it") });
            b.charts.push(chart::Chart { id: "cusum_chart".into(), title: "CUSUM chart".into(), svg: chart::cusum_chart(&c.stats, &lower.stats, c.ucl[0], &flag(c), &flag(lower), "Sample", "Cumulative sum", &alt), alt });
            continue;
        }
        let pts: Vec<(f64, f64)> = c.stats.iter().enumerate().map(|(i, v)| ((i + 1) as f64, *v)).collect();
        let flags: Vec<bool> = (0..c.stats.len()).map(|i| c.beyond.contains(&i) || c.runs.contains(&i)).collect();
        let alt = format!("{} chart of {} samples with center {} and limits {} to {}; {} flagged.", c.name, c.stats.len(), apa::num(c.center[0], 3), apa::num(c.lcl[0], 3), apa::num(c.ucl[0], 3), if flagged_total == 0 { "no points".to_string() } else { format!("{} points", beyond.len() + runs.len()) });
        b.charts.push(chart::Chart { id: format!("{key}_chart"), title: format!("{} chart", c.name), svg: chart::control_chart(&pts, &c.center, &c.lcl, &c.ucl, &flags, "Sample", c.name, &alt), alt });
    }
    b.tables.insert(0, Table {
        id: "summary".into(),
        title: "Control limits".into(),
        columns: vec!["Chart".into(), "Samples".into(), "Center".into(), "LCL".into(), "UCL".into(), "Beyond limits".into(), format!("Runs of {}+", spec.run_length)],
        rows: summary,
        footnotes: {
            let mut f = Vec::new();
            if sigma.is_finite() {
                f.push(format!("Process standard deviation (within subgroups) σ = {}.", apa::num(sigma, 4)));
            }
            if smoothed {
                f.push(if spec.chart == ChartType::Cusum { "CUSUM: the upper sum signals an upward shift above the decision interval (UCL), the lower sum a downward shift below −h (LCL). Run rules do not apply." } else { "EWMA limits widen to their steady-state values over the first samples. Run rules do not apply to smoothed values." }.into());
            }
            f
        },
    });
    // Capability.
    if (spec.lsl.is_some() || spec.usl.is_some()) && sigma.is_finite() && !matches!(spec.chart, ChartType::Cusum | ChartType::Ewma) {
        let n = values.len() as f64;
        let (lsl, usl) = (spec.lsl.unwrap_or(f64::NAN), spec.usl.unwrap_or(f64::NAN));
        let target = spec.target.unwrap_or(if lsl.is_finite() && usl.is_finite() { (lsl + usl) / 2.0 } else if lsl.is_finite() { lsl } else { usl });
        let mu = xcenter;
        let cp = (usl - lsl) / (2.0 * k * sigma);
        let cpu = (usl - mu) / (k * sigma);
        let cpl = (mu - lsl) / (k * sigma);
        let cpk = if cpu.is_nan() { cpl } else if cpl.is_nan() { cpu } else { cpu.min(cpl) };
        let cpm = cp / (1.0 + ((mu - target) / sigma).powi(2)).sqrt();
        let a = 1.0 - spec.level;
        let zc = norm_quantile(spec.level);
        let za = norm_quantile(1.0 - a / 2.0);
        let one_side = |c: f64, z: f64| [c * (1.0 - z * (1.0 / (9.0 * n * c * c) + 1.0 / (2.0 * (n - 1.0))).sqrt()), c * (1.0 + z * (1.0 / (9.0 * n * c * c) + 1.0 / (2.0 * (n - 1.0))).sqrt())];
        let cp_lim = [cp * (chisq_quantile(a / 2.0, n - 1.0) / (n - 1.0)).sqrt(), cp * (chisq_quantile(1.0 - a / 2.0, n - 1.0) / (n - 1.0)).sqrt()];
        let dfm = n * (1.0 + ((mu - target) / sigma).powi(2)) / (1.0 + 2.0 * ((mu - target) / sigma).powi(2));
        let cpm_lim = [cpm * (chisq_quantile(a / 2.0, dfm) / dfm).sqrt(), cpm * (chisq_quantile(1.0 - a / 2.0, dfm) / dfm).sqrt()];
        let overall = (values.iter().map(|v| (v - mu).powi(2)).sum::<f64>() / (n - 1.0)).sqrt();
        let pp = (usl - lsl) / (2.0 * k * overall);
        let ppk = ((usl - mu) / (k * overall)).min((mu - lsl) / (k * overall));
        let ppk = if ppk.is_nan() { if usl.is_finite() { (usl - mu) / (k * overall) } else { (mu - lsl) / (k * overall) } } else { ppk };
        let mut rows = Vec::new();
        for (name, key, v, lim) in [("Cp", "cp", cp, cp_lim), ("Cp_l", "cp_l", cpl, one_side(cpl, zc)), ("Cp_u", "cp_u", cpu, one_side(cpu, zc)), ("Cpk", "cpk", cpk, one_side(cpk, za)), ("Cpm", "cpm", cpm, cpm_lim)] {
            if !v.is_finite() {
                continue;
            }
            set(&mut b, key, v);
            set(&mut b, format!("{key}.lower"), lim[0]);
            set(&mut b, format!("{key}.upper"), lim[1]);
            rows.push(vec![text(name), num(v, 3), text(format!("[{}, {}]", apa::num(lim[0], 3), apa::num(lim[1], 3)))]);
        }
        for (name, key, v) in [("Pp", "pp", pp), ("Ppk", "ppk", ppk)] {
            if v.is_finite() {
                set(&mut b, key, v);
                rows.push(vec![text(name), num(v, 3), text("")]);
            }
        }
        // As qcc, an expected share below 0.01% is reported as 0.
        let tiny = |v: f64| if v < 1e-4 { 0.0 } else { v };
        let exp_l = if lsl.is_finite() { tiny(norm_cdf((lsl - mu) / sigma)) } else { f64::NAN };
        let exp_u = if usl.is_finite() { tiny(1.0 - norm_cdf((usl - mu) / sigma)) } else { f64::NAN };
        let obs_l = if lsl.is_finite() { values.iter().filter(|v| **v < lsl).count() as f64 / n } else { f64::NAN };
        let obs_u = if usl.is_finite() { values.iter().filter(|v| **v > usl).count() as f64 / n } else { f64::NAN };
        for (key, v) in [("exp_lsl", exp_l), ("exp_usl", exp_u), ("obs_lsl", obs_l), ("obs_usl", obs_u), ("overall_sd", overall)] {
            if v.is_finite() {
                set(&mut b, key, v);
            }
        }
        b.tables.push(Table {
            id: "capability".into(),
            title: "Process capability".into(),
            columns: vec!["Index".into(), "Value".into(), format!("{} CI", level_text(spec.level))],
            rows,
            footnotes: vec![
                format!("LSL = {}, USL = {}, target = {}; center = {}, σ (within) = {}, overall SD = {}.", if lsl.is_finite() { apa::num(lsl, 4) } else { "none".into() }, if usl.is_finite() { apa::num(usl, 4) } else { "none".into() }, apa::num(target, 4), apa::num(mu, 4), apa::num(sigma, 4), apa::num(overall, 4)),
                format!("Expected outside the limits (normal): {} below, {} above; observed: {} below, {} above.", pct(exp_l), pct(exp_u), pct(obs_l), pct(obs_u)),
                "Cp-family indices use σ within subgroups (qcc); Pp and Ppk use the overall standard deviation.".into(),
            ],
        });
    }
    b.diagnostics.push(if flagged_total == 0 {
        Diagnostic { id: "control".into(), title: "Statistical control".into(), state: DiagnosticState::NoConcernDetected, message: "No point is beyond the limits or in a long run: the process looks in control.".into() }
    } else {
        Diagnostic { id: "control".into(), title: "Statistical control".into(), state: DiagnosticState::ReviewNeeded, message: format!("{flagged_total} points are flagged: look for special causes before trusting the limits (and before assessing capability).") }
    });
    if charts[0].stats.len() < 20 {
        b.diagnostics.push(Diagnostic { id: "samples".into(), title: "Number of samples".into(), state: DiagnosticState::ReviewNeeded, message: "Control limits are usually based on 20 to 25 samples or more.".into() });
    }
    Ok(b)
}

fn pct(v: f64) -> String {
    if v.is_finite() {
        format!("{}%", apa::num(100.0 * v, 2))
    } else {
        "—".into()
    }
}
