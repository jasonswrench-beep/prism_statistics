//! One-way MANOVA (`anova.manova`, checklist B2b): two or more numeric outcomes compared across the groups of one factor.
//!
//! The four multivariate tests (Pillai's trace, Wilks' lambda, the Hotelling-Lawley trace and Roy's largest root) come
//! from the eigenvalues of E⁻¹H, with the F approximations R's `summary.manova` uses. Box's M tests the equality of the
//! groups' covariance matrices (chi-square approximation). Each outcome's one-way ANOVA follows. Cases are used when every
//! outcome and the factor are valid.

use prismqda_stats::dist::{chisq_sf, f_sf};
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use super::linalg::{det, relative_eigenvalues, Mat};
use crate::apa;
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Diagnostic, DiagnosticState, ResultBundle, Table};

pub const METHOD_ID: &str = "anova.manova";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub dependent: Vec<String>,
    pub factor: String,
    /// The test the paragraph reports; every test is in the table.
    #[serde(default)]
    pub test: Test,
    #[serde(default = "yes")]
    pub univariate: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Test {
    #[default]
    Pillai,
    Wilks,
    HotellingLawley,
    Roy,
}

/// A multivariate test: the statistic and its F approximation (R's `summary.manova`).
pub struct MvTest {
    pub stat: f64,
    pub f: f64,
    pub df1: f64,
    pub df2: f64,
    pub p: f64,
}

/// The four tests from the eigenvalues of E⁻¹H, `q` hypothesis and `df_res` error degrees of freedom.
pub fn tests(eig: &[f64], q: f64, df_res: f64) -> [MvTest; 4] {
    let p = eig.len() as f64;
    let s = p.min(q);
    let m = 0.5 * ((p - q).abs() - 1.0);
    let n = 0.5 * (df_res - p - 1.0);
    let make = |stat: f64, f: f64, df1: f64, df2: f64| MvTest { stat, f, df1, df2, p: f_sf(f, df1, df2) };
    let pillai = {
        let v: f64 = eig.iter().map(|e| e / (1.0 + e)).sum();
        let (t1, t2) = (2.0 * m + s + 1.0, 2.0 * n + s + 1.0);
        make(v, (t2 / t1 * v) / (s - v), s * t1, s * t2)
    };
    let wilks = {
        let l: f64 = eig.iter().map(|e| 1.0 / (1.0 + e)).product();
        let t1 = df_res - 0.5 * (p - q + 1.0);
        let t2 = (p * q - 2.0) / 4.0;
        let t3 = p * p + q * q - 5.0;
        let t3 = if t3 > 0.0 { (((p * q).powi(2) - 4.0) / t3).sqrt() } else { 1.0 };
        make(l, ((l.powf(-1.0 / t3) - 1.0) * (t1 * t3 - 2.0 * t2)) / p / q, p * q, t1 * t3 - 2.0 * t2)
    };
    let hotelling = {
        let t: f64 = eig.iter().sum();
        let t1 = 2.0 * m + s + 1.0;
        let t2 = 2.0 * (s * n + 1.0);
        make(t, (t2 * t) / s / s / t1, s * t1, t2)
    };
    let roy = {
        let r = eig.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let t1 = p.max(q);
        let t2 = df_res - t1 + q;
        make(r, (t2 * r) / t1, t1, t2)
    };
    [pillai, wilks, hotelling, roy]
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    let pn = spec.dependent.len();
    if pn < 2 {
        return Err(Error::Invalid("Choose at least two dependent variables (for one, use one-way ANOVA).".into()));
    }
    let names: Vec<&String> = spec.dependent.iter().chain(std::iter::once(&spec.factor)).collect();
    for (i, a) in names.iter().enumerate() {
        if names[..i].iter().any(|b| ds.find(a) == ds.find(b)) {
            return Err(Error::Invalid(format!("{a} is used more than once.")));
        }
    }
    let labels: Vec<String> = spec.dependent.iter().map(|n| ds.variable(n).map(|v| v.display_name().to_string())).collect::<Result<_>>()?;
    let f_label = ds.variable(&spec.factor)?.display_name().to_string();
    let cells: Vec<Vec<Slot<f64>>> = spec.dependent.iter().map(|n| numeric(ds, n)).collect::<Result<_>>()?;
    let (gslots, levels) = categories(ds, &spec.factor)?;
    let total = cells[0].len();
    let mut tally = Tally::default();
    let mut rows = Vec::new();
    for r in 0..total {
        if cells.iter().zip(&spec.dependent).any(|(c, n)| tally.check(&c[r], n)) || tally.check(&gslots[r], "factor") {
            continue;
        }
        rows.push(r);
    }
    let mut present: Vec<usize> = rows.iter().map(|&r| gslots[r].value().expect("checked")).collect();
    present.sort_unstable();
    present.dedup();
    let g = present.len();
    let n = rows.len();
    let groups: Vec<Vec<Vec<f64>>> = present
        .iter()
        .map(|&gi| rows.iter().filter(|&&r| gslots[r].value() == Some(gi)).map(|&r| cells.iter().map(|c| c[r].value().expect("checked")).collect()).collect())
        .collect();
    let note = format!(
        "One-way multivariate analysis of variance of {pn} outcomes across the groups of {f_label}: Pillai's trace, Wilks' lambda, the Hotelling-Lawley trace and Roy's largest root, with the F approximations R's summary.manova uses (Roy's is an upper bound, so its p value is a lower bound). Box's M tests equal covariance matrices (chi-square approximation). Effect size: partial eta squared for Pillai's trace (V / s). Cases were used when every outcome and the factor had valid values. Computed by PrismStatistics."
    );
    let mut b = common::bundle(METHOD_ID, METHOD_VERSION, format!("MANOVA: {} by {f_label}", labels.join(", ")), serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, n), note);
    set_text(&mut b, "factor.label", f_label.clone());
    set(&mut b, "p", pn as f64);
    set(&mut b, "g", g as f64);
    set(&mut b, "n", n as f64);
    for (j, l) in labels.iter().enumerate() {
        set_text(&mut b, format!("dv.{}.label", j + 1), l.clone());
    }
    // Group means.
    let mean = |rows: &[Vec<f64>], j: usize| rows.iter().map(|r| r[j]).sum::<f64>() / rows.len() as f64;
    let mut desc = Vec::new();
    for (gi, grp) in groups.iter().enumerate() {
        let gl = &levels[present[gi]].label;
        set_text(&mut b, format!("group.{}.label", gi + 1), gl.clone());
        set(&mut b, format!("group.{}.n", gi + 1), grp.len() as f64);
        for (j, l) in labels.iter().enumerate() {
            let col: Vec<f64> = grp.iter().map(|r| r[j]).collect();
            let mut row = vec![text(l), text(gl)];
            row.extend(summary_row("", &col).into_iter().skip(1));
            desc.push(row);
        }
    }
    b.tables.push(Table { id: "descriptives".into(), title: "Descriptive statistics".into(), columns: vec!["Outcome".into(), f_label.clone(), "N".into(), "Mean".into(), "SD".into(), "SE of mean".into()], rows: desc, footnotes: Vec::new() });
    if g < 2 || n < g + pn {
        fail(&mut b, if g < 2 { "The factor needs at least two groups with cases.".to_string() } else { format!("There are too few cases ({n}) for {pn} outcomes in {g} groups: the error degrees of freedom must be at least the number of outcomes.") });
        return Ok(b);
    }
    let all: Vec<Vec<f64>> = groups.iter().flatten().cloned().collect();
    let grand: Vec<f64> = (0..pn).map(|j| mean(&all, j)).collect();
    let mut h: Mat = vec![vec![0.0; pn]; pn];
    let mut e: Mat = vec![vec![0.0; pn]; pn];
    let mut covs = Vec::new();
    for grp in &groups {
        let m: Vec<f64> = (0..pn).map(|j| mean(grp, j)).collect();
        let mut s: Mat = vec![vec![0.0; pn]; pn];
        for r in grp {
            for a in 0..pn {
                for c in 0..pn {
                    s[a][c] += (r[a] - m[a]) * (r[c] - m[c]);
                }
            }
        }
        for a in 0..pn {
            for c in 0..pn {
                h[a][c] += grp.len() as f64 * (m[a] - grand[a]) * (m[c] - grand[c]);
                e[a][c] += s[a][c];
            }
        }
        covs.push((grp.len(), s));
    }
    let q = (g - 1) as f64;
    let df_res = (n - g) as f64;
    let Some(eig) = relative_eigenvalues(&h, &e) else {
        fail(&mut b, "The outcomes are perfectly related within the groups (the error matrix is singular), so the multivariate tests are undefined. Drop an outcome that is a combination of the others.");
        return Ok(b);
    };
    let ts = tests(&eig, q, df_res);
    let names = ["Pillai's trace", "Wilks' lambda", "Hotelling-Lawley trace", "Roy's largest root"];
    let keys = ["pillai", "wilks", "hotelling", "roy"];
    let s = (pn as f64).min(q);
    let mut rows_t = Vec::new();
    for ((t, name), key) in ts.iter().zip(names).zip(keys) {
        for (kk, v) in [("stat", t.stat), ("F", t.f), ("df1", t.df1), ("df2", t.df2), ("p", t.p)] {
            set(&mut b, format!("{key}.{kk}"), v);
        }
        rows_t.push(vec![text(name), num(t.stat, 4), num(t.f, 3), num(t.df1, 2), num(t.df2, 2), p(t.p)]);
    }
    let pes = ts[0].stat / s;
    set(&mut b, "pillai.pes", pes);
    set_text(&mut b, "test", match spec.test { Test::Pillai => "pillai", Test::Wilks => "wilks", Test::HotellingLawley => "hotelling", Test::Roy => "roy" });
    b.tables.push(Table {
        id: "multivariate".into(),
        title: format!("Multivariate tests: {f_label}"),
        columns: vec!["Test".into(), "Value".into(), "F".into(), "df1".into(), "df2".into(), "p".into()],
        rows: rows_t,
        footnotes: vec![format!("Partial η² from Pillai's trace (V / s, s = {}) = {}.", s, apa::num(pes, 3))],
    });

    // Box's M.
    let pooled: Mat = e.iter().map(|r| r.iter().map(|v| v / df_res).collect()).collect();
    let ld_pooled = det(&pooled).ln();
    let fine = covs.iter().all(|(ni, _)| *ni > pn);
    if fine && ld_pooled.is_finite() {
        let pf = pn as f64;
        let mut mstat = df_res * ld_pooled;
        let mut inv_sum = 0.0;
        for (ni, s) in &covs {
            let dfi = (*ni - 1) as f64;
            let cov: Mat = s.iter().map(|r| r.iter().map(|v| v / dfi).collect()).collect();
            mstat -= dfi * det(&cov).ln();
            inv_sum += 1.0 / dfi;
        }
        let c = (inv_sum - 1.0 / df_res) * (2.0 * pf * pf + 3.0 * pf - 1.0) / (6.0 * (pf + 1.0) * q);
        let chi = mstat * (1.0 - c);
        let dfb = pf * (pf + 1.0) * q / 2.0;
        let pv = chisq_sf(chi, dfb);
        for (kk, v) in [("boxm.M", mstat), ("boxm.chi2", chi), ("boxm.df", dfb), ("boxm.p", pv)] {
            set(&mut b, kk, v);
        }
        b.diagnostics.push(Diagnostic {
            id: "covariances".into(),
            title: "Equal covariance matrices".into(),
            state: if pv < 0.001 { DiagnosticState::ReviewNeeded } else { DiagnosticState::NoConcernDetected },
            message: format!(
                "Box's M = {}, χ²({}) = {}, p {}.{}",
                apa::num(mstat, 2),
                apa::df(dfb),
                apa::num(chi, 2),
                apa::p_clause(pv),
                if pv < 0.001 { " Box's M is conventionally judged at .001 because it is very sensitive; the groups' covariance matrices differ, so Pillai's trace, the most robust test, is the one to report." } else { " Box's M is sensitive to non-normality, so a significant result at .001 is the usual threshold." }
            ),
        });
    } else {
        b.diagnostics.push(not_assessed("covariances", "Equal covariance matrices", "Box's M needs more cases than outcomes in every group."));
    }

    if spec.univariate {
        let mut rows_u = Vec::new();
        for (j, l) in labels.iter().enumerate() {
            let (ssh, sse) = (h[j][j], e[j][j]);
            let f = (ssh / q) / (sse / df_res);
            let pv = f_sf(f, q, df_res);
            let key = format!("dv.{}", j + 1);
            set(&mut b, format!("{key}.F"), f);
            set(&mut b, format!("{key}.p"), pv);
            set(&mut b, format!("{key}.pes"), ssh / (ssh + sse));
            rows_u.push(vec![text(l), num(ssh, 3), num(q, 0), num(sse, 3), num(df_res, 0), num(f, 3), p(pv), num(ssh / (ssh + sse), 3)]);
        }
        b.tables.push(Table {
            id: "univariate".into(),
            title: "Univariate tests of each outcome".into(),
            columns: vec!["Outcome".into(), "SS (between)".into(), "df".into(), "SS (error)".into(), "df (error)".into(), "F".into(), "p".into(), "Partial η²".into()],
            rows: rows_u,
            footnotes: vec!["Each is the one-way ANOVA of that outcome; they are not adjusted for the number of outcomes.".into()],
        });
    }
    let smallest = groups.iter().map(Vec::len).min().unwrap_or(0);
    b.diagnostics.push(size_diagnostic(smallest, format!("The smallest group has {smallest} cases")));
    Ok(b)
}
