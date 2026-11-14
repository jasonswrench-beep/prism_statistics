//! Repeated-measures ANOVA (`anova.repeated`, checklist B1a) and mixed ANOVA (B1b): one within-subjects factor made of
//! two or more measures of the same cases, optionally crossed with one between-subjects factor.
//!
//! The univariate tests come from the multivariate linear model, as R's `car::Anova` (and `afex`) compute them: the
//! measures are fitted on the between-subjects design (sum-to-zero coding, Type III), transformed by orthonormal
//! contrasts of the within factor, and each effect's sum of squares is the trace of its hypothesis SSP matrix. Sphericity
//! is checked with Mauchly's test; the Greenhouse-Geisser epsilon and the Huynh-Feldt epsilon (with Lecoutre's
//! correction, as R gives it) adjust the degrees of freedom. Effect sizes: partial eta squared and generalized eta
//! squared (Olejnik and Algina, every factor treated as manipulated). Cases are used when every measure (and the between
//! factor) is valid.

use prismqda_stats::dist::{chisq_sf, f_sf};
use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::apa;
use crate::chart::{self, Chart};
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Cell, Diagnostic, DiagnosticState, ResultBundle, Table};

pub const METHOD_ID: &str = "anova.repeated";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Correction {
    None,
    /// Greenhouse-Geisser (the default: the paragraph reports it whatever Mauchly's test says).
    #[default]
    GreenhouseGeisser,
    HuynhFeldt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PostHoc {
    #[default]
    None,
    Bonferroni,
    Holm,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    /// The measures, in order: the levels of the within-subjects factor.
    pub measures: Vec<String>,
    /// The within-subjects factor's name, for output ("time").
    #[serde(default = "default_name")]
    pub factor_name: String,
    /// A between-subjects factor (mixed ANOVA).
    #[serde(default)]
    pub between: Option<String>,
    #[serde(default)]
    pub correction: Correction,
    /// Paired comparisons of the measures (without a between factor).
    #[serde(default)]
    pub post_hoc: PostHoc,
    #[serde(default)]
    pub plots: bool,
    #[serde(default = "default_level")]
    pub level: f64,
}

fn default_name() -> String {
    "time".into()
}

fn default_level() -> f64 {
    0.95
}

use super::linalg::{invert, mul, trace, transpose, det, Mat};

/// The fitted multivariate model of `z` (n × q) on the design: the error SSP, and the Type III hypothesis SSP of the
/// intercept and of the between factor.
struct Fit {
    error: Mat,
    intercept: Mat,
    group: Mat,
}

fn fit(x: &Mat, xtx_inv: &Mat, z: &Mat) -> Fit {
    let b = mul(xtx_inv, &mul(&transpose(x), z));
    let fitted = mul(x, &b);
    let e: Mat = z.iter().zip(&fitted).map(|(r, f)| r.iter().zip(f).map(|(a, c)| a - c).collect()).collect();
    let error = mul(&transpose(&e), &e);
    let g = x[0].len();
    let q = z[0].len();
    // Intercept: L = (1, 0, ..., 0).
    let b0 = vec![b[0].clone()];
    let intercept: Mat = mul(&transpose(&b0), &b0).into_iter().map(|r| r.into_iter().map(|v| v / xtx_inv[0][0]).collect()).collect();
    let group = if g > 1 {
        let bg: Mat = b[1..].to_vec();
        let sub: Mat = xtx_inv[1..].iter().map(|r| r[1..].to_vec()).collect();
        let inv = invert(&sub).unwrap_or_else(|| vec![vec![0.0; g - 1]; g - 1]);
        mul(&transpose(&bg), &mul(&inv, &bg))
    } else {
        vec![vec![0.0; q]; q]
    };
    Fit { error, intercept, group }
}

/// One tested effect.
struct Effect {
    key: &'static str,
    label: String,
    ss: f64,
    df1: f64,
    ss_error: f64,
    df2: f64,
    within: bool,
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    let k = spec.measures.len();
    if k < 2 {
        return Err(Error::Invalid("Choose at least two measures (the levels of the within-subjects factor).".into()));
    }
    let mut names: Vec<&String> = spec.measures.iter().collect();
    if let Some(bf) = &spec.between {
        names.push(bf);
    }
    for (i, a) in names.iter().enumerate() {
        if names[..i].iter().any(|b| ds.find(a) == ds.find(b)) {
            return Err(Error::Invalid(format!("{a} is used more than once.")));
        }
    }
    check_level(spec.level)?;
    let factor = if spec.factor_name.trim().is_empty() { "time".to_string() } else { spec.factor_name.trim().to_string() };
    let labels: Vec<String> = spec.measures.iter().map(|n| ds.variable(n).map(|v| v.display_name().to_string())).collect::<Result<_>>()?;
    let cells: Vec<Vec<Slot<f64>>> = spec.measures.iter().map(|n| numeric(ds, n)).collect::<Result<_>>()?;
    let total = cells[0].len();
    let (gslots, levels) = match &spec.between {
        Some(bf) => {
            let (s, l) = categories(ds, bf)?;
            (Some(s), l)
        }
        None => (None, Vec::new()),
    };
    let mut tally = Tally::default();
    let mut rows = Vec::new();
    for r in 0..total {
        if cells.iter().zip(&spec.measures).any(|(c, n)| tally.check(&c[r], n)) {
            continue;
        }
        if let Some(gs) = &gslots {
            if tally.check(&gs[r], "between-subjects factor") {
                continue;
            }
        }
        rows.push(r);
    }
    let y: Mat = rows.iter().map(|&r| cells.iter().map(|c| c[r].value().expect("checked")).collect()).collect();
    // Groups that have cases, in code order.
    let raw_group: Vec<usize> = rows.iter().map(|&r| gslots.as_ref().map_or(0, |gs| gs[r].value().expect("checked"))).collect();
    let present: Vec<usize> = {
        let mut v = raw_group.clone();
        v.sort_unstable();
        v.dedup();
        v
    };
    let group: Vec<usize> = raw_group.iter().map(|g| present.iter().position(|p| p == g).expect("present")).collect();
    let group_labels: Vec<String> = present.iter().map(|&g| levels.get(g).map_or_else(String::new, |l| l.label.clone())).collect();
    let g = present.len().max(1);
    let n = y.len();
    let between_label = spec.between.as_ref().map(|b| ds.variable(b).map(|v| v.display_name().to_string())).transpose()?;

    let mixed = spec.between.is_some();
    let note = format!(
        "{} ANOVA with {factor} ({k} measures) as the within-subjects factor{}. Sums of squares are Type III from the multivariate linear model with sum-to-zero coding, as R's car and afex give them. Sphericity: Mauchly's test; corrections: Greenhouse-Geisser and Huynh-Feldt (with Lecoutre's correction) epsilon. Effect sizes: partial eta squared and generalized eta squared (every factor treated as manipulated). Cases were used when every measure{} had a valid value. Computed by PrismStatistics with prismqda-stats {}.",
        if mixed { "Mixed" } else { "Repeated-measures" },
        between_label.as_ref().map_or(String::new(), |b| format!(" and {b} as the between-subjects factor")),
        if mixed { " and the between-subjects factor" } else { "" },
        super::STATS_VERSION
    );
    let title = match &between_label {
        Some(b) => format!("Mixed ANOVA: {factor} × {b}"),
        None => format!("Repeated-Measures ANOVA: {factor}"),
    };
    let mut b = common::bundle(METHOD_ID, METHOD_VERSION, title, serde_json::to_value(spec).expect("a spec always serializes"), tally.accounting(total, n), note);
    set_text(&mut b, "factor", factor.clone());
    set(&mut b, "k", k as f64);
    set(&mut b, "n", n as f64);
    set(&mut b, "g", g as f64);
    set(&mut b, "level", spec.level);
    if let Some(bl) = &between_label {
        set_text(&mut b, "between.label", bl.clone());
    }
    for (j, l) in labels.iter().enumerate() {
        set_text(&mut b, format!("measure.{}.label", j + 1), l.clone());
        let col: Vec<f64> = y.iter().map(|r| r[j]).collect();
        if !col.is_empty() {
            set(&mut b, format!("measure.{}.mean", j + 1), col.iter().sum::<f64>() / col.len() as f64);
        }
        if col.len() > 1 {
            let m = col.iter().sum::<f64>() / col.len() as f64;
            set(&mut b, format!("measure.{}.sd", j + 1), (col.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (col.len() - 1) as f64).sqrt());
        }
    }
    for (i, gl) in group_labels.iter().enumerate().filter(|_| mixed) {
        set_text(&mut b, format!("group.{}.label", i + 1), gl.clone());
        set(&mut b, format!("group.{}.n", i + 1), group.iter().filter(|&&x| x == i).count() as f64);
    }
    // Descriptive statistics: each measure, overall or in each group.
    let mut desc = Vec::new();
    for (j, l) in labels.iter().enumerate() {
        let groups: Vec<Option<usize>> = if mixed { (0..g).map(Some).collect() } else { vec![None] };
        for gi in groups {
            let col: Vec<f64> = y.iter().zip(&group).filter(|(_, gg)| gi.is_none_or(|x| **gg == x)).map(|(r, _)| r[j]).collect();
            let mut row = vec![text(l)];
            if let Some(gi) = gi {
                row.push(text(&group_labels[gi]));
            }
            row.extend(summary_row("", &col).into_iter().skip(1));
            desc.push(row);
        }
    }
    let mut dcols = vec!["Measure".into()];
    if let Some(bl) = &between_label {
        dcols.push(bl.clone());
    }
    dcols.extend(["N".into(), "Mean".into(), "SD".into(), "SE of mean".into()]);
    b.tables.push(Table { id: "descriptives".into(), title: "Descriptive statistics".into(), columns: dcols, rows: desc, footnotes: Vec::new() });

    if n <= g || (mixed && g < 2) {
        fail(&mut b, if mixed && g < 2 { "The between-subjects factor needs at least two groups with cases.".to_string() } else { format!("The analysis needs more cases than groups; there {} {n} with every measure.", if n == 1 { "is" } else { "are" }) });
        return Ok(b);
    }
    // The between-subjects design: intercept and sum-to-zero contrasts.
    let x: Mat = group.iter().map(|&gi| std::iter::once(1.0).chain((0..g - 1).map(|j| if gi == j { 1.0 } else if gi == g - 1 { -1.0 } else { 0.0 })).collect()).collect();
    let Some(xtx_inv) = invert(&mul(&transpose(&x), &x)) else {
        fail(&mut b, "The between-subjects design can't be estimated.");
        return Ok(b);
    };
    let p = k - 1;
    let contrasts = super::anova_oneway::polynomial_contrasts(k, p);
    let cmat: Mat = (0..k).map(|i| contrasts.iter().map(|c| c[i]).collect()).collect();
    let z = mul(&y, &cmat);
    let within = fit(&x, &xtx_inv, &z);
    let means: Mat = y.iter().map(|r| vec![r.iter().sum::<f64>() / (k as f64).sqrt()]).collect();
    let between = fit(&x, &xtx_inv, &means);
    let df_error_w = (p * (n - g)) as f64;
    let ss_error_w = trace(&within.error);
    let ss_subjects = between.error[0][0];
    if ss_error_w <= 0.0 {
        fail(&mut b, "There is no variation within the cases beyond the effects, so the F tests are undefined.");
        return Ok(b);
    }

    // Sphericity.
    let m = &within.error;
    let gg = trace(m).powi(2) / (p as f64 * trace(&mul(m, m)));
    let dfe = (n - g) as f64;
    let pf = p as f64;
    let hf = (((dfe + 1.0) * pf * gg - 2.0) / (pf * (dfe - pf * gg))).min(1.0);
    set(&mut b, "eps.gg", gg);
    set(&mut b, "eps.hf", hf);
    let mut sphericity_rows = Vec::new();
    if p >= 2 {
        let w = det(m) / (trace(m) / pf).powf(pf);
        let rho = 1.0 - (2.0 * pf * pf + pf + 2.0) / (6.0 * pf * dfe);
        let chi = -dfe * rho * w.ln();
        let df = pf * (pf + 1.0) / 2.0 - 1.0;
        let pv = chisq_sf(chi, df);
        for (kk, v) in [("mauchly.W", w), ("mauchly.chi2", chi), ("mauchly.df", df), ("mauchly.p", pv)] {
            set(&mut b, kk, v);
        }
        sphericity_rows.push(vec![text(&factor), num(w, 4), num(chi, 3), num(df, 0), p_cell(pv), num(gg, 4), num(hf, 4), num(1.0 / pf, 4)]);
        b.diagnostics.push(if pv < 0.05 {
            Diagnostic { id: "sphericity".into(), title: "Sphericity".into(), state: DiagnosticState::ReviewNeeded, message: format!("Mauchly's test suggests the variances of the differences between measures are unequal, W = {}, χ²({}) = {}, p {}. Use a corrected test (Greenhouse-Geisser ε = {}).", apa::num(w, 2), apa::df(df), apa::num(chi, 2), apa::p_clause(pv), apa::no_leading_zero(gg, 2)) }
        } else {
            Diagnostic { id: "sphericity".into(), title: "Sphericity".into(), state: DiagnosticState::NoConcernDetected, message: format!("Mauchly's test: W = {}, χ²({}) = {}, p {}. With few cases the test has little power, so the corrected tests are shown as well.", apa::num(w, 2), apa::df(df), apa::num(chi, 2), apa::p_clause(pv)) }
        });
        b.tables.push(Table {
            id: "sphericity".into(),
            title: "Mauchly's test of sphericity".into(),
            columns: vec!["Within-subjects effect".into(), "Mauchly's W".into(), "Chi-square".into(), "df".into(), "p".into(), "Greenhouse-Geisser ε".into(), "Huynh-Feldt ε".into(), "Lower-bound ε".into()],
            rows: sphericity_rows,
            footnotes: vec!["Huynh-Feldt epsilon uses Lecoutre's correction and is capped at 1.".into()],
        });
    } else {
        b.diagnostics.push(not_assessed("sphericity", "Sphericity", "With two measures there is one difference between them, so sphericity always holds and no correction is needed."));
    }

    let mut effects = vec![Effect { key: "within", label: factor.clone(), ss: trace(&within.intercept), df1: pf, ss_error: ss_error_w, df2: df_error_w, within: true }];
    if mixed {
        let bl = between_label.clone().unwrap_or_default();
        effects.insert(0, Effect { key: "between", label: bl.clone(), ss: between.group[0][0], df1: (g - 1) as f64, ss_error: ss_subjects, df2: dfe, within: false });
        effects.push(Effect { key: "interaction", label: format!("{factor} × {bl}"), ss: trace(&within.group), df1: pf * (g - 1) as f64, ss_error: ss_error_w, df2: df_error_w, within: true });
    }
    let total_error = ss_subjects + ss_error_w;
    let mut rows_t = Vec::new();
    for e in &effects {
        let f = (e.ss / e.df1) / (e.ss_error / e.df2);
        let pes = e.ss / (e.ss + e.ss_error);
        let ges = e.ss / (e.ss + total_error);
        let key = e.key;
        set_text(&mut b, format!("{key}.label"), e.label.clone());
        for (kk, v) in [("ss", e.ss), ("F", f), ("df1", e.df1), ("df2", e.df2), ("p", f_sf(f, e.df1, e.df2)), ("pes", pes), ("ges", ges)] {
            set(&mut b, format!("{key}.{kk}"), v);
        }
        let variants: Vec<(&str, f64)> = if e.within { vec![("Sphericity assumed", 1.0), ("Greenhouse-Geisser", gg), ("Huynh-Feldt", hf)] } else { vec![("", 1.0)] };
        for (vi, (vname, eps)) in variants.iter().enumerate() {
            let (d1, d2) = (e.df1 * eps, e.df2 * eps);
            let pv = f_sf(f, d1, d2);
            if e.within {
                let suffix = ["none", "gg", "hf"][vi];
                set(&mut b, format!("{key}.{suffix}.df1"), d1);
                set(&mut b, format!("{key}.{suffix}.df2"), d2);
                set(&mut b, format!("{key}.{suffix}.p"), pv);
            }
            rows_t.push(vec![
                if vi == 0 { text(&e.label) } else { Cell::Empty },
                text(*vname),
                num(e.ss, 3),
                num(d1, if vi == 0 { 0 } else { 2 }),
                num(e.ss / d1, 3),
                num(f, 3),
                p_cell(pv),
                if vi == 0 { num(pes, 3) } else { Cell::Empty },
                if vi == 0 { num(ges, 3) } else { Cell::Empty },
            ]);
        }
    }
    let errs: Vec<(&str, f64, f64)> = if mixed { vec![("Error (between subjects)", ss_subjects, dfe), ("Error (within subjects)", ss_error_w, df_error_w)] } else { vec![("Error", ss_error_w, df_error_w)] };
    for (lab, ss, df) in errs {
        rows_t.push(vec![text(lab), Cell::Empty, num(ss, 3), num(df, 0), num(ss / df, 3), Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty]);
    }
    set(&mut b, "error.ss", ss_error_w);
    set(&mut b, "subjects.ss", ss_subjects);
    set_text(&mut b, "correction", match spec.correction { Correction::None => "none", Correction::GreenhouseGeisser => "gg", Correction::HuynhFeldt => "hf" });
    b.tables.push(Table {
        id: "tests".into(),
        title: "Tests of effects".into(),
        columns: vec!["Source".into(), "Correction".into(), "Sum of squares".into(), "df".into(), "Mean square".into(), "F".into(), "p".into(), "Partial η²".into(), "Generalized η²".into()],
        rows: rows_t,
        footnotes: vec!["A correction multiplies both degrees of freedom of a within-subjects test by its epsilon; the F value is unchanged.".into()],
    });

    // Paired comparisons of the measures.
    if spec.post_hoc != PostHoc::None && !mixed {
        let mut raw = Vec::new();
        let mut pairs = Vec::new();
        for a in 0..k {
            for c in (a + 1)..k {
                let xa: Vec<f64> = y.iter().map(|r| r[a]).collect();
                let xc: Vec<f64> = y.iter().map(|r| r[c]).collect();
                if let Some(t) = prismqda_stats::ttest::paired(&xa, &xc, spec.level) {
                    raw.push(t.p);
                    pairs.push((a, c, Some(t)));
                } else {
                    raw.push(f64::NAN);
                    pairs.push((a, c, None));
                }
            }
        }
        let method = if spec.post_hoc == PostHoc::Holm { prismqda_stats::adjust::Adjustment::Holm } else { prismqda_stats::adjust::Adjustment::Bonferroni };
        let adj = prismqda_stats::adjust::p_adjust(&raw, method);
        let name = if spec.post_hoc == PostHoc::Holm { "Holm" } else { "Bonferroni" };
        set_text(&mut b, "post_hoc", name);
        let mut rows_p = Vec::new();
        for (i, ((a, c, t), pa)) in pairs.iter().zip(&adj).enumerate() {
            let key = format!("pair.{}", i + 1);
            set_text(&mut b, format!("{key}.a"), labels[*a].clone());
            set_text(&mut b, format!("{key}.b"), labels[*c].clone());
            if let Some(t) = t {
                set(&mut b, format!("{key}.diff"), t.estimate);
                set(&mut b, format!("{key}.p"), *pa);
                rows_p.push(vec![text(&labels[*a]), text(&labels[*c]), num(t.estimate, 3), num(t.se, 3), num(t.t, 3), num(t.df, 0), p_cell(t.p), p_cell(*pa)]);
            }
        }
        b.tables.push(Table {
            id: "post_hoc".into(),
            title: format!("Paired comparisons ({name})"),
            columns: vec!["Measure (I)".into(), "Measure (J)".into(), "Mean difference (I − J)".into(), "SE".into(), "t".into(), "df".into(), "p".into(), format!("p ({name})")],
            rows: rows_p,
            footnotes: vec![format!("Paired t tests; p values adjusted by {name}'s method for the {} comparisons.", pairs.len())],
        });
    }

    if spec.plots {
        let series: Vec<(String, Vec<f64>)> = if mixed {
            (0..g).map(|gi| (group_labels[gi].clone(), (0..k).map(|j| { let v: Vec<f64> = y.iter().zip(&group).filter(|(_, gg)| **gg == gi).map(|(r, _)| r[j]).collect(); v.iter().sum::<f64>() / v.len() as f64 }).collect())).collect()
        } else {
            vec![("Mean".to_string(), (0..k).map(|j| y.iter().map(|r| r[j]).sum::<f64>() / n as f64).collect())]
        };
        let alt = format!("Profile plot of the mean of each measure of {factor}{}.", between_label.as_ref().map_or(String::new(), |bl| format!(", a line for each category of {bl}")));
        b.charts.push(Chart { id: "profile".into(), title: format!("Means across {factor}"), alt: alt.clone(), svg: chart::profile(&labels, &series, &factor, "Mean", between_label.as_deref().unwrap_or(""), &alt) });
    }
    b.diagnostics.push(size_diagnostic(n, format!("{n} cases had every measure")));
    Ok(b)
}

fn p_cell(v: f64) -> Cell {
    common::p(v)
}
