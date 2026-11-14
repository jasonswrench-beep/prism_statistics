//! Measurement invariance (checklist C1l): the same model fitted across groups with more and more parameters held
//! equal (configural, metric, scalar, strict), each compared with the one before by a likelihood-ratio (χ² difference)
//! test, as lavaan's lavTestLRT does, and by the change in CFI and RMSEA. With MLM or MLR the difference test is the
//! Satorra-Bentler (2001) scaled one, and CFI and RMSEA are the robust ones.

use prismqda_stats::dist::chisq_sf;
use serde::{Deserialize, Serialize};

use super::{core, prepare, Estimator, Missing, Spec};
use crate::analysis::common::{self, *};
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{ResultBundle, Table};

pub const INVARIANCE: &str = "sem.invariance";
pub const METHOD_VERSION: u32 = 1;

fn yes() -> bool {
    true
}

fn default_steps() -> Vec<String> {
    STEPS.iter().map(|s| s.0.to_string()).collect()
}

/// The steps and what each holds equal.
const STEPS: [(&str, &str, &[&str]); 4] = [
    ("configural", "Configural", &[]),
    ("metric", "Metric (equal loadings)", &["loadings"]),
    ("scalar", "Scalar (equal loadings and intercepts)", &["loadings", "intercepts"]),
    ("strict", "Strict (equal loadings, intercepts and residual variances)", &["loadings", "intercepts", "residuals"]),
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvarianceSpec {
    pub model: String,
    pub group: String,
    #[serde(default = "default_steps")]
    pub steps: Vec<String>,
    /// Parameters left free across groups in every step (partial invariance), such as "x3 ~1".
    #[serde(default)]
    pub group_partial: Vec<String>,
    #[serde(default)]
    pub std_lv: bool,
    #[serde(default = "yes")]
    pub fixed_x: bool,
    #[serde(default)]
    pub estimator: Estimator,
    #[serde(default)]
    pub missing: Missing,
}

struct StepFit {
    label: &'static str,
    chisq: f64,
    df: f64,
    c: Option<f64>,
    cfi: f64,
    rmsea: f64,
    srmr: f64,
}

pub fn run(ds: &Dataset, spec: &InvarianceSpec) -> Result<ResultBundle> {
    let mut steps: Vec<&(&str, &str, &[&str])> = Vec::new();
    for s in &spec.steps {
        steps.push(STEPS.iter().find(|x| x.0 == s).ok_or_else(|| Error::Invalid(format!("\"{s}\" is not an invariance step; use configural, metric, scalar or strict.")))?);
    }
    steps.sort_by_key(|s| STEPS.iter().position(|x| x.0 == s.0));
    steps.dedup_by_key(|s| s.0);
    if steps.len() < 2 {
        return Err(Error::Invalid("Choose at least two invariance steps to compare.".into()));
    }
    let robust = spec.estimator != Estimator::Ml;
    let gname = ds.variable(&spec.group)?.display_name().to_string();
    let mut fits: Vec<StepFit> = Vec::new();
    let mut accounting = None;
    let mut fiml = false;
    let mut failure = None;
    let mut labels = Vec::new();
    for (key, label, equal) in &steps {
        let s = Spec {
            model: spec.model.clone(),
            std_lv: spec.std_lv,
            fixed_x: spec.fixed_x,
            estimator: spec.estimator,
            missing: spec.missing,
            group: Some(spec.group.clone()),
            group_equal: equal.iter().map(|e| e.to_string()).collect(),
            group_partial: spec.group_partial.clone(),
            modindices: false,
            ..Spec::default()
        };
        let pr = prepare(ds, &s)?;
        fiml = pr.fiml;
        labels = pr.group_labels.clone();
        let n = super::estimate::total_n(&pr.blocks);
        accounting.get_or_insert_with(|| pr.tally.clone().accounting(pr.total, n as usize));
        match core(&pr, &s) {
            Ok(c) => {
                let m = &c.m;
                let (cfi, rmsea) = match (&m.scaled, fiml) {
                    (Some(sc), false) => (sc.cfi_robust, sc.rmsea_robust),
                    (Some(sc), true) => (sc.cfi, sc.rmsea),
                    (None, _) => (m.cfi, m.rmsea),
                };
                fits.push(StepFit { label, chisq: m.chisq, df: m.df, c: m.scaled.as_ref().map(|s| s.c), cfi, rmsea, srmr: m.srmr });
            }
            Err(why) => {
                failure = Some(format!("The {key} model could not be fitted. {why}"));
                break;
            }
        }
    }
    let kind = match (robust, fiml) {
        (false, _) => "",
        (true, false) => " CFI and RMSEA are the robust ones",
        (true, true) => " CFI and RMSEA are the scaled ones",
    };
    let note = format!(
        "Measurement invariance across {gname}: the model is fitted to each group with more parameters held equal at each step (lavaan's group.equal), groups in order of first appearance in the data ({}). Each step is compared with the one before it by {}.{kind}{} Computed by PrismStatistics.",
        labels.join(", "),
        if robust { "the Satorra-Bentler (2001) scaled χ² difference test" } else { "the χ² difference (likelihood-ratio) test" },
        if spec.group_partial.is_empty() { String::new() } else { format!(" Left free in every group (partial invariance): {}.", spec.group_partial.join(", ")) },
    );
    let mut b = common::bundle(INVARIANCE, METHOD_VERSION, format!("Measurement Invariance: {gname}"), serde_json::to_value(spec).expect("a spec always serializes"), accounting.unwrap_or_default(), note);
    set_text(&mut b, "group.label", gname);
    set(&mut b, "robust", f64::from(u8::from(robust)));
    if let Some(f) = failure {
        fail(&mut b, f);
        return Ok(b);
    }
    let small = |v: f64| crate::apa::no_leading_zero(v, 3);
    let mut rows = Vec::new();
    for (k, s) in fits.iter().enumerate() {
        let key = format!("step.{}", k + 1);
        set_text(&mut b, format!("{key}.label"), s.label);
        set_text(&mut b, format!("{key}.id"), steps[k].0);
        for (f, v) in [("chisq", s.chisq), ("df", s.df), ("cfi", s.cfi), ("rmsea", s.rmsea), ("srmr", s.srmr)] {
            set(&mut b, format!("{key}.{f}"), v);
        }
        let mut row = vec![text(s.label), num(s.chisq, 2), num(s.df, 0), text(small(s.cfi)), text(small(s.rmsea)), text(small(s.srmr))];
        if k == 0 {
            row.extend([text(""), text(""), text(""), text(""), text("")]);
        } else {
            let a = &fits[k - 1];
            let ddf = s.df - a.df;
            let dchisq = match (a.c, s.c) {
                (Some(ca), Some(cb)) => {
                    let cd = (s.df * cb - a.df * ca) / ddf;
                    (s.chisq - a.chisq) / cd
                }
                _ => s.chisq - a.chisq,
            };
            let pv = if dchisq.is_finite() && dchisq >= 0.0 && ddf > 0.0 { chisq_sf(dchisq, ddf) } else { f64::NAN };
            for (f, v) in [("dchisq", dchisq), ("ddf", ddf), ("p", pv), ("dcfi", s.cfi - a.cfi), ("drmsea", s.rmsea - a.rmsea)] {
                set(&mut b, format!("{key}.{f}"), v);
            }
            row.extend([num(dchisq, 2), num(ddf, 0), if pv.is_finite() { common::p(pv) } else { text("") }, text(small(s.cfi - a.cfi)), text(small(s.rmsea - a.rmsea))]);
        }
        rows.push(row);
    }
    set(&mut b, "steps", fits.len() as f64);
    b.tables.push(Table {
        id: "invariance".into(),
        title: "Measurement invariance".into(),
        columns: vec!["Model".into(), if robust { "Scaled χ²".into() } else { "χ²".into() }, "df".into(), "CFI".into(), "RMSEA".into(), "SRMR".into(), "Δχ²".into(), "Δdf".into(), "p".into(), "ΔCFI".into(), "ΔRMSEA".into()],
        rows: rows
            .into_iter()
            .enumerate()
            .map(|(k, mut r)| {
                // Show the scaled χ² in the table; the difference test above uses the unscaled ones as it should.
                if let Some(c) = fits[k].c {
                    r[1] = num(fits[k].chisq / c, 2);
                }
                r
            })
            .collect(),
        footnotes: vec!["Each model is compared with the one above it. A common supplement to the χ² difference test (which is sensitive to sample size) is the change in CFI: a drop of more than .010, with an RMSEA increase of more than .015, suggests the added equality constraints do not hold (Chen, 2007).".into()],
    });
    Ok(b)
}
