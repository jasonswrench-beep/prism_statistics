//! From a result to the model's input, and from the model's output back to text.
//!
//! Every named field of the result becomes a slot. Numbers are written by [`format`], with the same APA rules as the
//! template writer, so `{p}` becomes "= .012" or "< .001" and `{ci}` becomes "95% CI [0.39, 0.74]". The model sees each
//! slot's meaning and its formatted value, so it can choose the right words, but it can only cite the value.

use prism_engine::apa;
use prism_engine::result::{DiagnosticState, Field, ResultBundle, Status};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Slot {
    pub name: String,
    pub meaning: String,
    /// The text the slot becomes, already formatted.
    pub value: String,
}

/// Everything the model is given. No raw data: only what the Output tab already shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Input {
    pub method_id: String,
    pub system: String,
    pub user: String,
    pub slots: Vec<Slot>,
}

// Kept short on purpose: on a CPU, reading the prompt costs about as much per word as writing does.
const SYSTEM: &str = "Write the Results paragraph of an APA 7 report from verified statistics.
- Never write a number. Cite every number as a slot in braces from the list, e.g. {t}.
- Slots are formatted already: write *p* {p}, since {p} becomes '= .012' or '< .001'. A slot ending in .ci becomes the whole interval, '95% CI [0.39, 0.74]', so don't write 'confidence interval' around it. {alpha} is the significance level.
- APA notation: (*M* = {g1.mean}, *SD* = {g1.sd}), *t*({df}) = {t}. Italicize symbols with single asterisks.
- No causal claims. A non-significant result is 'not significant', never 'no difference'.
- One concise paragraph, past tense.
- Report the statistics plainly; don't add interpretations such as \"reliable\" or \"meaningful\".
Example (paired t test): A paired-samples *t* test compared {x1.label} and {x2.label}. Anxiety was lower after the training (*M* = {x2.mean}, *SD* = {x2.sd}) than before it (*M* = {x1.mean}, *SD* = {x1.sd}), *t*({df}) = {t}, *p* {p}, mean difference = {diff}, {ci}, Cohen's *d* = {d}, {d.ci}.";

/// Fields that are bookkeeping rather than something to report.
fn skipped(key: &str) -> bool {
    let last = key.rsplit('.').next().unwrap_or(key);
    matches!(key, "level" | "exact" | "share_below_5" | "rows" | "cols" | "terms" | "k" | "statistics" | "variables" | "test" | "coefficient" | "post_hoc" | "predictors")
        || last == "owner"
        || key.starts_with("count.")
}

/// Slots worth offering the model: what a results paragraph reports. Others (SEs, VIFs, internal counts) stay
/// available to fill but aren't listed, which keeps the prompt short and the model on task.
fn listed(key: &str) -> bool {
    let last = key.rsplit('.').next().unwrap_or(key);
    matches!(
        last,
        "alpha" | "level" | "label" | "a" | "b" | "method" | "mean" | "sd" | "median" | "n" | "t" | "df" | "df1" | "df2" | "p" | "diff" | "ci" | "g" | "d" | "F" | "chi2" | "V" | "phi" | "r" | "U" | "z" | "H" | "epsilon2" | "omega2" | "r2" | "adj_r2" | "beta" | "test_value"
    ) && !key.starts_with("var.")
        || key.starts_with("var.")
}

/// Statistics bounded by 1 in absolute value, written without a leading zero.
fn bounded(key: &str) -> bool {
    let last = key.rsplit('.').next().unwrap_or(key);
    matches!(last, "r" | "eta2" | "omega2" | "epsilon2" | "eta2_h" | "V" | "phi" | "r2" | "adj_r2" | "ps")
}

fn is_count(key: &str) -> bool {
    let last = key.rsplit('.').next().unwrap_or(key);
    matches!(last, "n" | "neg" | "pos" | "ties") || key == "n"
}

/// A number written the way the slot should appear.
pub fn format(key: &str, v: f64) -> String {
    let last = key.rsplit('.').next().unwrap_or(key);
    if !v.is_finite() {
        return "—".into();
    }
    if last == "p" {
        apa::p_clause(v)
    } else if last.starts_with("df") {
        apa::df(v)
    } else if is_count(key) {
        format!("{}", v.round() as i64)
    } else if bounded(key) {
        apa::no_leading_zero(v, 2)
    } else if key == "test_value" {
        apa::plain(v)
    } else {
        apa::num(v, 2)
    }
}

/// What a slot means, from its name.
fn meaning(key: &str, b: &ResultBundle) -> String {
    let label = |k: &str| b.text(k).unwrap_or(k).to_string();
    let parts: Vec<&str> = key.split('.').collect();
    let last = *parts.last().unwrap_or(&key);
    let what = match last {
        "mean" => "mean",
        "sd" => "standard deviation",
        "median" => "median",
        "n" => "number of cases",
        "t" => "t statistic",
        "df" => "degrees of freedom",
        "df1" => "numerator degrees of freedom",
        "df2" => "denominator degrees of freedom",
        "p" => "p value clause, expands to '= .012' or '< .001'",
        "diff" => "mean difference",
        "se" => "standard error",
        "ci" => "confidence interval, expands to '95% CI [lower, upper]'",
        "g" => "Hedges' g",
        "d" => "Cohen's d",
        "F" => "F statistic",
        "chi2" => "chi-square statistic",
        "V" => "Cramér's V",
        "phi" => "phi coefficient",
        "r" => "correlation or rank-biserial correlation",
        "U" => "Mann-Whitney U",
        "z" => "z statistic",
        "H" => "Kruskal-Wallis H",
        "epsilon2" => "epsilon squared",
        "eta2" => "eta squared",
        "eta2_h" => "eta squared based on H",
        "omega2" => "omega squared",
        "r2" => "R squared",
        "adj_r2" => "adjusted R squared",
        "b" => "unstandardized coefficient B",
        "beta" => "standardized coefficient beta",
        "vif" => "variance inflation factor",
        "sigma" => "residual standard error",
        "label" => match parts.first().copied() {
            Some("y") => "name of the outcome (dependent variable)",
            Some("g1") => "name of the first group",
            Some("g2") => "name of the second group",
            Some("x1") => "name of the first measure",
            Some("x2") => "name of the second measure",
            Some("factor") => "name of the grouping factor",
            Some("row") => "name of the row variable",
            Some("col") => "name of the column variable",
            _ => "name",
        },
        "test_value" => "test value",
        "g2" => "likelihood-ratio chi-square",
        "min_expected" => "smallest expected count",
        "neg" => "number of negative differences",
        "pos" => "number of positive differences",
        "ties" => "number of zero differences",
        "ps" => "probability of superiority",
        _ => last,
    };
    let whose = match parts.first().copied() {
        Some("g1") | Some("g2") if parts.len() > 1 => format!(" for {}", label(&format!("{}.label", parts[0]))),
        Some("x1") | Some("x2") if parts.len() > 1 => format!(" for {}", label(&format!("{}.label", parts[0]))),
        Some("group") if parts.len() > 2 => format!(" for group {}", label(&format!("group.{}.label", parts[1]))),
        Some("term") if parts.len() > 2 => format!(" for {}", label(&format!("term.{}.label", parts[1]))),
        Some("b0") => " for the intercept".to_string(),
        Some("var") if parts.len() > 2 => format!(" (variable {})", parts[1]),
        Some("pair") if parts.len() > 3 && b.text(&format!("var.{}.label", parts[1])).is_some() => {
            format!(" for {} and {}", label(&format!("var.{}.label", parts[1])), label(&format!("var.{}.label", parts[2])))
        }
        Some("pair") if parts.len() > 2 => {
            let i = parts[1];
            match (b.text(&format!("pair.{i}.a")), b.text(&format!("pair.{i}.b"))) {
                (Some(a), Some(c)) => format!(" for {a} versus {c}"),
                _ => String::new(),
            }
        }
        _ => String::new(),
    };
    format!("{what}{whose}")
}

/// The model's input for a result, or the reason there is nothing to write about.
pub fn prepare(b: &ResultBundle) -> Result<Input, String> {
    if let Status::Failed { reason } = &b.status {
        return Err(format!("This result can't be interpreted, so there is nothing to write up: {reason}"));
    }
    let level = b.number("level").unwrap_or(0.95);
    let mut slots: Vec<Slot> = vec![
        Slot { name: "alpha".into(), meaning: "significance level".into(), value: apa::no_leading_zero(1.0 - level, 2) },
        Slot { name: "level".into(), meaning: "confidence level, as in '{level} confidence interval'".into(), value: format!("{}%", apa::num(level * 100.0, 0)) },
    ];
    // Confidence intervals become one slot each: "ci", "g.ci", "pair.1.2.ci" and so on.
    // A one-sided interval stores only its closed end; the open end is infinite.
    let mut ci_prefixes: Vec<String> = b.fields.keys().filter_map(|k| k.strip_suffix("ci.lower").or_else(|| k.strip_suffix("ci.upper")).map(str::to_string)).collect();
    ci_prefixes.sort();
    ci_prefixes.dedup();
    let alternative = b.text("alternative");
    for pre in &ci_prefixes {
        let end = |e: &str, open: f64| b.number(&format!("{pre}ci.{e}")).filter(|v| v.is_finite()).or(match (alternative, e) {
            (Some("greater"), "upper") | (Some("less"), "lower") => Some(open),
            _ => None,
        });
        let (Some(lo), Some(hi)) = (end("lower", f64::NEG_INFINITY), end("upper", f64::INFINITY)) else { continue };
        let name = format!("{pre}ci");
        let r_like = b.method_id == "corr.bivariate";
        let value = if r_like {
            format!("{}% CI [{}, {}]", apa::num(level * 100.0, 0), apa::no_leading_zero(lo, 2), apa::no_leading_zero(hi, 2))
        } else {
            apa::ci(level, lo, hi, 2)
        };
        let of = match pre.trim_end_matches('.').rsplit('.').next().unwrap_or("") {
            "" => " of the mean difference".to_string(),
            "g" => " of Hedges' g".to_string(),
            "d" => " of Cohen's d".to_string(),
            _ => meaning(&format!("{pre}x"), b).trim_start_matches('x').to_string(),
        };
        slots.push(Slot { meaning: format!("confidence interval{of}, expands to '95% CI [lower, upper]'"), name, value });
    }
    for (k, f) in &b.fields {
        if skipped(k) || k.ends_with("ci.lower") || k.ends_with("ci.upper") {
            continue;
        }
        let value = match f {
            Field::Number(v) => format(k, *v),
            Field::Text(t) => t.clone(),
        };
        slots.push(Slot { name: k.clone(), meaning: meaning(k, b), value });
    }

    let alpha = 1.0 - level;
    let mut facts = Vec::new();
    match alternative {
        Some("greater") => facts.push("The test was one-sided, in the direction 'greater': write the p value as one-sided, such as '*p* {p} (one-sided)'.".to_string()),
        Some("less") => facts.push("The test was one-sided, in the direction 'less': write the p value as one-sided, such as '*p* {p} (one-sided)'.".to_string()),
        _ => {}
    }
    if let Some(p) = b.number("p") {
        facts.push(if p < alpha { format!("The main test was significant at alpha = {}.", apa::no_leading_zero(alpha, 2)) } else { format!("The main test was NOT significant at alpha = {}.", apa::no_leading_zero(alpha, 2)) });
    }
    for (diff, a, c) in [("diff", "g1.label", "g2.label"), ("diff", "x1.label", "x2.label")] {
        if let (Some(d), Some(a), Some(c)) = (b.number(diff), b.text(a), b.text(c)) {
            facts.push(if d > 0.0 { format!("{a} is higher than {c}.") } else { format!("{a} is lower than {c}.") });
        }
    }
    if b.method_id == "ttest.one_sample" {
        if let (Some(d), Some(y)) = (b.number("diff"), b.text("y.label")) {
            facts.push(format!("The mean {y} is {} the test value.", if d > 0.0 { "higher than" } else { "lower than" }));
        }
    }
    if b.method_id == "np.mann_whitney" {
        if let (Some(r), Some(a), Some(c)) = (b.number("r"), b.text("g1.label"), b.text("g2.label")) {
            facts.push(if r > 0.0 { format!("{a} values tend to be higher than {c} values.") } else { format!("{a} values tend to be lower than {c} values.") });
        }
    }
    // Each pair or term with its own p value: say plainly which are significant, so the model needn't guess.
    let mut each = Vec::new();
    for (k, f) in &b.fields {
        if let (Some(stem), Field::Number(p)) = (k.strip_suffix(".p"), f) {
            if !(stem.starts_with("pair.") || stem.starts_with("term.")) {
                continue;
            }
            let what = meaning(&format!("{stem}.x"), b);
            let what = what.trim_start_matches('x').trim();
            let what = if what.is_empty() { stem.to_string() } else { what.trim_start_matches("for ").to_string() };
            each.push(format!("{what}: {}", if *p < alpha { "significant" } else { "NOT significant" }));
        }
    }
    if !each.is_empty() {
        facts.push(format!("Separate tests: {}.", each.join("; ")));
    }
    let review: Vec<&str> = b.diagnostics.iter().filter(|d| d.state == DiagnosticState::ReviewNeeded).map(|d| d.title.as_str()).collect();
    if !review.is_empty() {
        facts.push(format!("Checks that need the reader's attention: {}. Mention them briefly.", review.join(", ")));
    }
    let method = b.method_note.split(". ").next().unwrap_or("").trim_end_matches('.');
    let mut user = format!("Analysis: {}. {method}.\n", b.title);
    if !facts.is_empty() {
        user += &format!("Facts: {}\n", facts.join(" "));
    }
    // The model is told what each number means but never its value: shown values tempt it to spell numbers out in
    // words, which is how a wrong number got into a draft in the first spike. Names (text) are shown as they are.
    user += "Slots ({name}: meaning):\n";
    for s in slots.iter().filter(|s| listed(&s.name)) {
        if b.text(&s.name).is_some() {
            user += &format!("{{{}}}: {} \"{}\"\n", s.name, s.meaning, s.value);
        } else {
            user += &format!("{{{}}}: {}\n", s.name, s.meaning);
        }
    }
    user += "Write the paragraph now.";
    Ok(Input { method_id: b.method_id.clone(), system: SYSTEM.to_string(), user, slots })
}

/// Replaces every `{slot}` with its value. Returns the text and any slot names that don't exist.
pub fn fill(input: &Input, raw: &str) -> (String, Vec<String>) {
    let mut out = String::new();
    let mut unknown = Vec::new();
    let mut rest = raw;
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        match rest[i..].find('}') {
            Some(j) => {
                let name = &rest[i + 1..i + j];
                match input.slots.iter().find(|s| s.name == name) {
                    Some(s) => out.push_str(&s.value),
                    None => {
                        unknown.push(name.to_string());
                        out.push_str(&rest[i..i + j + 1]);
                    }
                }
                rest = &rest[i + j + 1..];
            }
            None => {
                out.push_str(&rest[i..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    (out.trim().to_string(), unknown)
}
