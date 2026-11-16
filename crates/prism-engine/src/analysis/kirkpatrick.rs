//! Training evaluation by Kirkpatrick's four levels (`eval.kirkpatrick`, development list item 7).
//!
//! One report with a section for each level the data cover, each optional:
//! - **Level 1, Reaction**: rating items (a reaction survey). For each item and for all of them together: N, mean, SD
//!   and the percent favorable (ratings at or above a cut, by default the top two points of the scale); the scale mean
//!   per person (cases with every item, listwise) and Cronbach's alpha with two or more items.
//! - **Level 2, Learning**: test scores before and after, either paired (two variables, the same people) or as two
//!   groups (one score and a variable saying which administration it was, as when two classes' answer sheets from
//!   OpenAssess are added together without names). Paired: the mean gain with its interval, the paired t test and
//!   Cohen's d_z. Grouped: Welch's t test and Cohen's d with the pooled SD. With the points possible, Hake's normalized
//!   gain of the means, (post − pre) / (max − pre), and for paired data the mean of each person's normalized gain.
//! - **Level 3, Behavior** and **Level 4, Results**: before-and-after pairs (as Level 2, without the normalized gain)
//!   and single measures (N, mean, SD, lowest and highest).
//!
//! Each section uses the cases complete on what it reads, so the levels can have different N, and the report says so.
//! None of it shows that the training caused a change: without a comparison group the report says that plainly.

use serde::{Deserialize, Serialize};

use super::common::{self, *};
use crate::apa;
use crate::error::{Error, Result};
use crate::model::Dataset;
use crate::result::{Cell, Diagnostic, DiagnosticState, ResultBundle, Table};

pub const METHOD_ID: &str = "eval.kirkpatrick";
pub const METHOD_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pair {
    pub pre: String,
    pub post: String,
}

/// Before and after as two groups of cases: one score and the variable whose codes say which is which.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Grouped {
    pub score: String,
    pub time: String,
    /// The codes (or texts) of the time variable for before and after.
    pub pre: String,
    pub post: String,
}

fn level95() -> f64 {
    0.95
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Spec {
    #[serde(default)]
    pub reaction: Vec<String>,
    /// A rating at or above this is favorable; by default the scale's highest point less one (top two boxes).
    #[serde(default)]
    pub favorable: Option<f64>,
    /// The rating scale's highest point; the highest rating seen when left out.
    #[serde(default)]
    pub reaction_max: Option<f64>,
    #[serde(default)]
    pub learning: Vec<Pair>,
    #[serde(default)]
    pub learning_groups: Option<Grouped>,
    /// The test's points possible, for the normalized gain.
    #[serde(default)]
    pub learning_max: Option<f64>,
    #[serde(default)]
    pub behavior: Vec<Pair>,
    #[serde(default)]
    pub behavior_items: Vec<String>,
    #[serde(default)]
    pub results: Vec<Pair>,
    #[serde(default)]
    pub results_items: Vec<String>,
    #[serde(default = "level95")]
    pub level: f64,
}

fn mean(x: &[f64]) -> f64 {
    x.iter().sum::<f64>() / x.len() as f64
}

fn sd(x: &[f64]) -> f64 {
    let m = mean(x);
    (x.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (x.len() as f64 - 1.0)).sqrt()
}

fn values(ds: &Dataset, name: &str) -> Result<Vec<Option<f64>>> {
    Ok(numeric(ds, name)?.iter().map(Slot::value).collect())
}

fn label(ds: &Dataset, name: &str) -> Result<String> {
    Ok(ds.variable(name)?.display_name().to_string())
}

/// A paired before-and-after comparison.
struct PairResult {
    n: usize,
    pre_mean: f64,
    pre_sd: f64,
    post_mean: f64,
    post_sd: f64,
    gain: f64,
    gain_sd: f64,
    t: f64,
    df: f64,
    p: f64,
    lo: f64,
    hi: f64,
    d: f64,
}

fn paired(pre: &[f64], post: &[f64], level: f64) -> Option<PairResult> {
    let n = pre.len();
    if n < 2 {
        return None;
    }
    let diff: Vec<f64> = post.iter().zip(pre).map(|(a, b)| a - b).collect();
    let (g, s) = (mean(&diff), sd(&diff));
    let df = n as f64 - 1.0;
    let se = s / (n as f64).sqrt();
    let t = g / se;
    let crit = prismqda_stats::dist::t_quantile(1.0 - (1.0 - level) / 2.0, df);
    Some(PairResult { n, pre_mean: mean(pre), pre_sd: sd(pre), post_mean: mean(post), post_sd: sd(post), gain: g, gain_sd: s, t, df, p: prismqda_stats::dist::t_two_sided(t, df), lo: g - crit * se, hi: g + crit * se, d: g / s })
}

/// Welch's comparison of an after group with a before group.
fn welch(pre: &[f64], post: &[f64], level: f64) -> Option<PairResult> {
    let (n1, n2) = (pre.len() as f64, post.len() as f64);
    if n1 < 2.0 || n2 < 2.0 {
        return None;
    }
    let (m1, m2, s1, s2) = (mean(pre), mean(post), sd(pre), sd(post));
    let (v1, v2) = (s1 * s1 / n1, s2 * s2 / n2);
    let se = (v1 + v2).sqrt();
    let df = (v1 + v2).powi(2) / (v1 * v1 / (n1 - 1.0) + v2 * v2 / (n2 - 1.0));
    let g = m2 - m1;
    let t = g / se;
    let crit = prismqda_stats::dist::t_quantile(1.0 - (1.0 - level) / 2.0, df);
    let pooled = (((n1 - 1.0) * s1 * s1 + (n2 - 1.0) * s2 * s2) / (n1 + n2 - 2.0)).sqrt();
    Some(PairResult { n: (n1 + n2) as usize, pre_mean: m1, pre_sd: s1, post_mean: m2, post_sd: s2, gain: g, gain_sd: f64::NAN, t, df, p: prismqda_stats::dist::t_two_sided(t, df), lo: g - crit * se, hi: g + crit * se, d: g / pooled })
}

/// Fields and a table row for one comparison.
fn record(b: &mut ResultBundle, key: &str, what: &str, r: &PairResult, lv: &str) -> Vec<Cell> {
    for (k, v) in [("n", r.n as f64), ("pre.mean", r.pre_mean), ("pre.sd", r.pre_sd), ("post.mean", r.post_mean), ("post.sd", r.post_sd), ("gain", r.gain), ("t", r.t), ("df", r.df), ("p", r.p), ("ci.lower", r.lo), ("ci.upper", r.hi), ("d", r.d)] {
        set(b, format!("{key}.{k}"), v);
    }
    if r.gain_sd.is_finite() {
        set(b, format!("{key}.gain.sd"), r.gain_sd);
    }
    let _ = lv;
    vec![text(what.to_string()), count(r.n), num(r.pre_mean, 2), num(r.pre_sd, 2), num(r.post_mean, 2), num(r.post_sd, 2), num(r.gain, 2), text(format!("[{}, {}]", apa::num(r.lo, 2), apa::num(r.hi, 2))), num(r.t, 2), num(r.df, if r.df.fract() == 0.0 { 0 } else { 2 }), p(r.p), num(r.d, 2)]
}

fn change_columns(lv: &str) -> Vec<String> {
    ["Measure", "N", "Before M", "Before SD", "After M", "After SD", "Change", &format!("{lv} CI"), "t", "df", "p", "d"].iter().map(|s| s.to_string()).collect()
}

/// The complete cases of a pair.
fn pair_values(ds: &Dataset, pr: &Pair) -> Result<(Vec<f64>, Vec<f64>)> {
    let (a, z) = (values(ds, &pr.pre)?, values(ds, &pr.post)?);
    Ok(a.iter().zip(&z).filter_map(|(x, y)| Some((((*x)?), (*y)?))).unzip())
}

pub fn run(ds: &Dataset, spec: &Spec) -> Result<ResultBundle> {
    check_level(spec.level)?;
    let any = !spec.reaction.is_empty() || !spec.learning.is_empty() || spec.learning_groups.is_some() || !spec.behavior.is_empty() || !spec.behavior_items.is_empty() || !spec.results.is_empty() || !spec.results_items.is_empty();
    if !any {
        return Err(Error::Invalid("Choose the variables for at least one level.".into()));
    }
    if let Some(m) = spec.learning_max {
        if !(m.is_finite() && m > 0.0) {
            return Err(Error::Invalid("The test's points possible must be a number above zero.".into()));
        }
    }
    let lv = level_text(spec.level);
    let total = ds.n_rows();
    let mut levels_used = Vec::new();
    let mut note = String::from("Each level uses the cases complete on its own measures, so the levels can have different numbers of cases. ");
    let mut b = common::bundle(METHOD_ID, METHOD_VERSION, "Training evaluation by Kirkpatrick level".into(), serde_json::to_value(spec).expect("a spec always serializes"), crate::result::CaseAccounting { total, analyzed: total, excluded: Vec::new() }, String::new());
    let mut summary: Vec<Vec<Cell>> = Vec::new();
    let mut smallest = usize::MAX;

    // Level 1: Reaction.
    if !spec.reaction.is_empty() {
        levels_used.push(1);
        let cols: Vec<Vec<Option<f64>>> = spec.reaction.iter().map(|n| values(ds, n)).collect::<Result<_>>()?;
        let seen_max = cols.iter().flatten().flatten().cloned().fold(f64::NEG_INFINITY, f64::max);
        let top = spec.reaction_max.unwrap_or(seen_max);
        let cut = spec.favorable.unwrap_or(top - 1.0);
        set(&mut b, "reaction.favorable_at", cut);
        let mut rows = Vec::new();
        let (mut fav_all, mut n_all) = (0usize, 0usize);
        for (i, (name, col)) in spec.reaction.iter().zip(&cols).enumerate() {
            let v: Vec<f64> = col.iter().flatten().copied().collect();
            if v.is_empty() {
                return Err(Error::Invalid(format!("{} has no ratings.", label(ds, name)?)));
            }
            let fav = v.iter().filter(|x| **x >= cut).count();
            fav_all += fav;
            n_all += v.len();
            let k = format!("reaction.{}", i + 1);
            set(&mut b, format!("{k}.n"), v.len() as f64);
            set(&mut b, format!("{k}.mean"), mean(&v));
            set(&mut b, format!("{k}.sd"), if v.len() > 1 { sd(&v) } else { f64::NAN });
            set(&mut b, format!("{k}.favorable"), fav as f64 / v.len() as f64);
            set_text(&mut b, format!("{k}.label"), label(ds, name)?);
            rows.push(vec![text(label(ds, name)?), count(v.len()), num(mean(&v), 2), num(if v.len() > 1 { sd(&v) } else { f64::NAN }, 2), num(100.0 * fav as f64 / v.len() as f64, 1)]);
        }
        // Each person's mean of the items, when they rated all of them.
        let complete: Vec<usize> = (0..total).filter(|&r| cols.iter().all(|c| c[r].is_some())).collect();
        let person: Vec<f64> = complete.iter().map(|&r| mean(&cols.iter().map(|c| c[r].expect("complete")).collect::<Vec<_>>())).collect();
        set(&mut b, "reaction.favorable", fav_all as f64 / n_all as f64);
        let mut foot = vec![format!("Favorable: a rating of {} or more{}. The scale mean is each person's mean rating, for the {} who rated every item.", apa::plain(cut), if spec.favorable.is_none() { " (the top two points of the scale)" } else { "" }, complete.len())];
        if !person.is_empty() {
            set(&mut b, "reaction.scale.n", person.len() as f64);
            set(&mut b, "reaction.scale.mean", mean(&person));
            if person.len() > 1 {
                set(&mut b, "reaction.scale.sd", sd(&person));
            }
            smallest = smallest.min(person.len());
            let mut row = vec![text("Scale mean (all items)".to_string()), count(person.len()), num(mean(&person), 2), num(if person.len() > 1 { sd(&person) } else { f64::NAN }, 2), num(100.0 * fav_all as f64 / n_all as f64, 1)];
            if spec.reaction.len() >= 2 && complete.len() >= 3 {
                let items: Vec<Vec<f64>> = cols.iter().map(|c| complete.iter().map(|&r| c[r].expect("complete")).collect()).collect();
                let a = super::reliability::alpha(&items);
                if a.is_finite() {
                    set(&mut b, "reaction.alpha", a);
                    foot.push(format!("Cronbach's alpha of the {} items: {}.", spec.reaction.len(), apa::no_leading_zero(a, 2)));
                }
            }
            row.truncate(5);
            rows.push(row);
            summary.push(vec![text("1. Reaction"), text(format!("Mean rating {} of {} ({}% favorable)", apa::num(mean(&person), 2), apa::plain(top), apa::num(100.0 * fav_all as f64 / n_all as f64, 0))), count(person.len())]);
        }
        b.tables.push(Table { id: "reaction".into(), title: "Level 1: Reaction".into(), columns: vec!["Item".into(), "N".into(), "M".into(), "SD".into(), "% favorable".into()], rows, footnotes: foot });
    }

    // Level 2: Learning.
    if !spec.learning.is_empty() || spec.learning_groups.is_some() {
        levels_used.push(2);
        let mut rows = Vec::new();
        let mut gains = Vec::new();
        let mut foot = Vec::new();
        for (i, pr) in spec.learning.iter().enumerate() {
            let (a, z) = pair_values(ds, pr)?;
            let what = format!("{} to {}", label(ds, &pr.pre)?, label(ds, &pr.post)?);
            let key = format!("learning.{}", i + 1);
            let Some(r) = paired(&a, &z, spec.level) else {
                return Err(Error::Invalid(format!("{what}: fewer than two cases have both scores.")));
            };
            smallest = smallest.min(r.n);
            rows.push(record(&mut b, &key, &what, &r, &lv));
            if let Some(max) = spec.learning_max {
                let hake = (r.post_mean - r.pre_mean) / (max - r.pre_mean);
                set(&mut b, format!("{key}.hake"), hake);
                let ind: Vec<f64> = a.iter().zip(&z).filter(|(x, _)| **x < max).map(|(x, y)| (y - x) / (max - x)).collect();
                if !ind.is_empty() {
                    set(&mut b, format!("{key}.g_mean"), mean(&ind));
                }
                let at_max = a.len() - ind.len();
                foot.push(format!("{what}: normalized gain of the means g = {}; the mean of each person's normalized gain is {}{}.", apa::num(hake, 2), apa::num(mean(&ind), 2), if at_max > 0 { format!(" ({at_max} who scored {} before are left out of it)", apa::plain(max)) } else { String::new() }));
                gains.push(hake);
            }
            summary.push(vec![text("2. Learning"), text(format!("{}: mean gain {} points, d_z = {}{}", what, apa::num(r.gain, 2), apa::num(r.d, 2), spec.learning_max.map_or(String::new(), |m| format!(", normalized gain {}", apa::num((r.post_mean - r.pre_mean) / (m - r.pre_mean), 2))))), count(r.n)]);
        }
        if let Some(g) = &spec.learning_groups {
            let (codes, levels) = categories(ds, &g.time)?;
            let find = |code: &str| levels.iter().position(|l| l.key() == code || l.label == code).ok_or_else(|| Error::Invalid(format!("{} has no category {code}.", g.time)));
            let (ipre, ipost) = (find(&g.pre)?, find(&g.post)?);
            let score = values(ds, &g.score)?;
            let pick = |want: usize| -> Vec<f64> { (0..total).filter(|&r| codes[r].value() == Some(want)).filter_map(|r| score[r]).collect() };
            let (a, z) = (pick(ipre), pick(ipost));
            let what = format!("{} ({} against {})", label(ds, &g.score)?, levels[ipost].label, levels[ipre].label);
            let Some(r) = welch(&a, &z, spec.level) else {
                return Err(Error::Invalid(format!("{what}: each group needs at least two scores.")));
            };
            smallest = smallest.min(a.len().min(z.len()));
            set(&mut b, "learning.groups.n.pre", a.len() as f64);
            set(&mut b, "learning.groups.n.post", z.len() as f64);
            rows.push(record(&mut b, "learning.groups", &what, &r, &lv));
            foot.push(format!("{what}: two groups of cases ({} before, {} after) compared by Welch's t test; d uses the pooled SD. The people need not be the same.", a.len(), z.len()));
            if let Some(max) = spec.learning_max {
                let hake = (r.post_mean - r.pre_mean) / (max - r.pre_mean);
                set(&mut b, "learning.groups.hake", hake);
                foot.push(format!("{what}: normalized gain of the means g = {}.", apa::num(hake, 2)));
            }
            summary.push(vec![text("2. Learning"), text(format!("{}: mean difference {} points, d = {}", what, apa::num(r.gain, 2), apa::num(r.d, 2))), count(r.n)]);
        }
        if spec.learning_max.is_some() {
            foot.push("The normalized gain (Hake) is the share of the possible improvement that was made: (after − before) / (points possible − before).".into());
        }
        foot.push("Paired rows: the paired t test of after − before, with Cohen's d_z (the mean change over the SD of the changes).".into());
        let _ = gains;
        b.tables.push(Table { id: "learning".into(), title: "Level 2: Learning".into(), columns: change_columns(&lv), rows, footnotes: foot });
    }

    // Levels 3 and 4.
    for (lvl, name, pairs, items) in [(3usize, "Behavior", &spec.behavior, &spec.behavior_items), (4, "Results", &spec.results, &spec.results_items)] {
        if pairs.is_empty() && items.is_empty() {
            continue;
        }
        levels_used.push(lvl);
        let prefix = name.to_lowercase();
        if !pairs.is_empty() {
            let mut rows = Vec::new();
            for (i, pr) in pairs.iter().enumerate() {
                let (a, z) = pair_values(ds, pr)?;
                let what = format!("{} to {}", label(ds, &pr.pre)?, label(ds, &pr.post)?);
                let Some(r) = paired(&a, &z, spec.level) else {
                    return Err(Error::Invalid(format!("{what}: fewer than two cases have both values.")));
                };
                smallest = smallest.min(r.n);
                rows.push(record(&mut b, &format!("{prefix}.{}", i + 1), &what, &r, &lv));
                summary.push(vec![text(format!("{lvl}. {name}")), text(format!("{what}: mean change {}, d_z = {}", apa::num(r.gain, 2), apa::num(r.d, 2))), count(r.n)]);
            }
            b.tables.push(Table { id: format!("{prefix}_change"), title: format!("Level {lvl}: {name}, before and after"), columns: change_columns(&lv), rows, footnotes: vec!["The paired t test of after − before, with Cohen's d_z.".into()] });
        }
        if !items.is_empty() {
            let mut rows = Vec::new();
            for (i, n) in items.iter().enumerate() {
                let v: Vec<f64> = values(ds, n)?.into_iter().flatten().collect();
                if v.is_empty() {
                    return Err(Error::Invalid(format!("{} has no values.", label(ds, n)?)));
                }
                let k = format!("{prefix}.item.{}", i + 1);
                let (lo, hi) = v.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, z), x| (a.min(*x), z.max(*x)));
                set(&mut b, format!("{k}.n"), v.len() as f64);
                set(&mut b, format!("{k}.mean"), mean(&v));
                if v.len() > 1 {
                    set(&mut b, format!("{k}.sd"), sd(&v));
                }
                smallest = smallest.min(v.len());
                rows.push(vec![text(label(ds, n)?), count(v.len()), num(mean(&v), 2), num(if v.len() > 1 { sd(&v) } else { f64::NAN }, 2), num(lo, 2), num(hi, 2)]);
                summary.push(vec![text(format!("{lvl}. {name}")), text(format!("{}: M = {}", label(ds, n)?, apa::num(mean(&v), 2))), count(v.len())]);
            }
            b.tables.push(Table { id: format!("{prefix}_measures"), title: format!("Level {lvl}: {name}"), columns: vec!["Measure".into(), "N".into(), "M".into(), "SD".into(), "Lowest".into(), "Highest".into()], rows, footnotes: Vec::new() });
        }
    }

    b.tables.insert(0, Table { id: "summary".into(), title: "Summary by level".into(), columns: vec!["Level".into(), "Finding".into(), "N".into()], rows: summary, footnotes: vec!["Levels with no measures chosen are left out.".into()] });
    set_text(&mut b, "levels", levels_used.iter().map(|l| l.to_string()).collect::<Vec<_>>().join(","));
    note += &format!("Paired before-and-after comparisons use the paired t test with a {lv} interval for the mean change and Cohen's d_z; two groups of cases use Welch's t test and Cohen's d with the pooled SD. Computed by PrismStatistics.");
    b.method_note = note;
    b.diagnostics.push(Diagnostic { id: "design".into(), title: "Cause".into(), state: DiagnosticState::NotAssessed, message: "Before-and-after differences show change, not that the training caused it: other events, practice with the test and regression toward the mean can move scores too. A comparison group that did not take the training is what would separate them.".into() });
    if smallest != usize::MAX {
        b.diagnostics.push(size_diagnostic(smallest, format!("the smallest section has {smallest} cases")));
    }
    let missing: Vec<usize> = (1..=4).filter(|l| !levels_used.contains(l)).collect();
    if !missing.is_empty() {
        b.diagnostics.push(Diagnostic { id: "levels".into(), title: "Levels covered".into(), state: DiagnosticState::NotAssessed, message: format!("No measures were chosen for level {}.", apa::series(&missing.iter().map(|l| l.to_string()).collect::<Vec<_>>())) });
    }
    Ok(b)
}
