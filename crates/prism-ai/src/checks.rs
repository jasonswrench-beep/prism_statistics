//! The checks every AI draft goes through before the user sees it as ready (design 10 §5).
//!
//! A **fail** means the draft must not be used (the template paragraph is offered instead); a **flag** means the user
//! has to look at the phrase before accepting. The checks are deliberately simple and conservative: they look for the
//! specific mistakes that matter in a results paragraph, not for style.

use prism_engine::result::{DiagnosticState, ResultBundle};
use serde::{Deserialize, Serialize};

use crate::slots::Input;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckState {
    Pass,
    Flag,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Check {
    pub id: String,
    pub title: String,
    pub state: CheckState,
    pub message: String,
}

fn check(id: &str, title: &str, state: CheckState, message: impl Into<String>) -> Check {
    Check { id: id.into(), title: title.into(), state, message: message.into() }
}

/// The slots a paragraph for each method must cite (APA's required elements).
fn required(b: &ResultBundle) -> Vec<String> {
    let has = |k: &str| b.fields.contains_key(k);
    let mut v: Vec<&str> = match b.method_id.as_str() {
        "ttest.independent" => vec!["g1.mean", "g1.sd", "g2.mean", "g2.sd", "t", "df", "p", "ci", "g"],
        "ttest.one_sample" => vec!["mean", "sd", "t", "df", "p", "ci", "d"],
        "ttest.paired" => vec!["x1.mean", "x1.sd", "x2.mean", "x2.sd", "t", "df", "p", "ci", "d"],
        "anova.oneway" if b.text("test") == Some("welch") => vec!["welch.F", "welch.df1", "welch.df2", "welch.p", "omega2"],
        "anova.oneway" => vec!["F", "df1", "df2", "p", "omega2"],
        "desc.crosstabs" => vec!["chi2", "df", "p", "n", if has("phi") { "phi" } else { "V" }],
        "np.mann_whitney" => vec!["U", "p", "r", "g1.median", "g2.median"],
        "np.wilcoxon" => vec!["V", "p", "r"],
        "np.kruskal" => vec!["H", "df", "p", "epsilon2"],
        "reg.linear" => vec!["r2", "F", "df1", "df2", "p"],
        "np.mcnemar" => vec!["p"],
        "np.friedman" => vec!["chi2", "df", "p", "W"],
        "reg.logistic" => vec!["chi2", "df", "p", "nagelkerke"],
        "cat.chisq_gof" => vec!["chi2", "df", "p", "w"],
        "np.binomial" | "np.sign" | "np.runs" | "np.ks2" => vec!["p"],
        "np.median" => vec!["chi2", "df", "p"],
        "desc.explore" => vec![],
        "bootstrap" => vec!["row.1.estimate"],
        "anova.repeated" => vec!["within.F", "within.pes"],
        "anova.manova" => vec!["pillai.pes"],
        "factor.efa" => vec!["kmo", "variance.pct"],
        "process.mediation" => vec!["indirect.1.effect", "indirect.1.lower", "indirect.1.upper"],
        "process.moderation" => vec!["xw.b", "xw.p"],
        "reg.ordinal" | "reg.multinomial" => vec!["chi2", "df", "p"],
        "reg.count" => vec!["chi2", "p"],
        "robust.yuen" | "robust.yuen_paired" => vec!["t", "df", "p"],
        "robust.t1way" => vec!["F", "df1", "df2", "p"],
        "mixed.lmm" => vec!["term.1.b", "term.1.p"],
        "missing.patterns" => vec![],
        "perm.test" => vec!["statistic", "p"],
        "surv.km" => vec![],
        "surv.cox" => vec!["lr.chi2", "lr.p"],
        "roc.analysis" => vec!["var.1.auc"],
        "lca.model" => vec!["llik", "bic", "P.1"],
        "lpa.mixture" => vec!["bic", "ll"],
        "predict.forest" => vec![],
        "predict.boost" => vec!["best_trees"],
        "meta.analysis" => vec!["beta.1.est", "beta.1.p", "i2"],
        "gee.model" => vec!["coef.2.b", "coef.2.p"],
        "panel.model" => vec!["coef.1.b", "coef.1.p", "r2"],
        "panel.gmm" => vec!["sargan", "ar2"],
        "predict.penalized" => vec!["lambda.1se", "nonzero.1se"],
        "predict.tree" => vec!["chosen.nsplit", "leaves"],
        "spc.chart" => vec![],
        "doe.design" => vec![],
        "doe.effects" => vec!["lenth.pse"],
        "doe.rsm" => vec!["r2"],
        "doe.mixture" => vec!["r2"],
        "irt.model" => vec!["logl", "bic", "m2", "m2.rmsea", "m2.srmsr"],
        "ts.acf" => vec!["lb.Q", "lb.p"],
        "ts.smooth" => vec![],
        "ts.arima" => vec!["sigma2", "aic"],
        "survey.estimates" => vec!["est.1.value", "est.1.se"],
        "survey.regression" => vec!["term.1.b", "term.1.p"],
        "sem.model" => vec!["fit.chisq", "fit.cfi", "fit.rmsea", "par.1.est"],
        "sem.invariance" => vec!["step.2.dchisq", "step.2.p", "step.2.dcfi"],
        "pls.model" => vec!["srmr", "n"],
        "missing.mi_regression" => vec!["term.1.b", "term.1.p"],
        "regression.curve" => vec!["model.1.r2", "model.1.F", "model.1.p"],
        "np.cochran" => vec!["Q", "df", "p"],
        "agree.kappa" | "agree.fleiss" => vec!["kappa", "p"],
        "agree.kripp" => vec!["alpha"],
        "agree.icc" => vec!["icc", "F", "p"],
        "scale.reliability" => vec!["alpha", "alpha.ci.lower", "alpha.ci.upper"],
        "scale.item_analysis" => vec!["reliability", "mean", "sd", "n"],
        "eval.kirkpatrick" => vec!["learning.1.t", "learning.1.p", "learning.1.d"],
        _ => vec![],
    };
    let mut extra = Vec::new();
    if b.method_id == "corr.bivariate" || b.method_id == "corr.partial" {
        extra.extend(b.fields.keys().filter(|k| k.starts_with("pair.") && (k.ends_with(".r") || k.ends_with(".p"))).cloned());
    }
    if b.method_id == "reg.linear" {
        extra.extend(b.fields.keys().filter(|k| k.starts_with("term.") && (k.ends_with(".b") || k.ends_with(".p"))).cloned());
    }
    v.retain(|k| has(k) || k.ends_with("ci"));
    v.into_iter().map(str::to_string).chain(extra).collect()
}

/// Number words that never belong in a results paragraph: every number there must come from a slot. Small numbers
/// (one to ten) are allowed because APA writes counts below ten as words ("three groups").
const NUMBER_WORDS: [&str; 41] = [
    "zero", "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety", "hundred", "thousand", "million", "oh", "oo",
    // Ordinals from fourth up ("the ninety-fifth percentile", "Ninth confidence interval"); first to third are ordinary words.
    "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth", "eleventh", "twelfth", "twentieth", "thirtieth", "fiftieth", "ninetieth", "hundredth", "thousandth", "fifths", "tenths", "hundredths",
];
const SMALL: [&str; 10] = ["one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten"];

/// True when the text spells a number in words, looking only at words that end at or after `from` (so a generator
/// can ask about the newest piece). Catches big number words and decimals or percentages built from small ones
/// ("one point nine", "five percent").
pub fn spells_a_number(text: &str, from: usize) -> bool {
    let lower = text.to_lowercase();
    let mut words: Vec<(usize, &str)> = Vec::new();
    let mut start = None;
    for (i, c) in lower.char_indices().chain(std::iter::once((lower.len(), ' '))) {
        match (start, c.is_alphabetic()) {
            (None, true) => start = Some(i),
            (Some(s), false) => {
                words.push((i, &lower[s..i]));
                start = None;
            }
            _ => {}
        }
    }
    words.iter().enumerate().any(|(k, (end, w))| {
        if *end < from {
            return false;
        }
        if compound_number(w) {
            return true;
        }
        let prev = k.checked_sub(1).map(|j| words[j].1);
        let numeric = |p: &str| SMALL.contains(&p) || NUMBER_WORDS.contains(&p);
        NUMBER_WORDS.contains(w)
            || (matches!(*w, "point" | "percent" | "per" | "cent" | "hundredths" | "tenths") && prev.is_some_and(numeric))
            || (SMALL.contains(w) && prev.is_some_and(|p| p == "point" || SMALL.contains(&p)))
    })
}

/// A word built entirely from two or more number words run together ("ninefive", "nineteenfifty").
fn compound_number(w: &str) -> bool {
    fn parts(w: &str) -> Option<usize> {
        if w.is_empty() {
            return Some(0);
        }
        SMALL.iter().chain(NUMBER_WORDS.iter()).filter(|n| n.len() > 2 && w.starts_with(*n)).find_map(|n| parts(&w[n.len()..]).map(|k| k + 1))
    }
    parts(w).is_some_and(|k| k >= 2)
}

fn sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = text.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        cur.push(*c);
        // A full stop followed by a space and a capital ends a sentence ("p < .001" doesn't).
        if *c == '.' && chars.get(i + 1) == Some(&' ') && chars.get(i + 2).is_some_and(|n| n.is_uppercase()) {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

fn contains_word(s: &str, w: &str) -> bool {
    s.match_indices(w).any(|(i, _)| {
        let before = s[..i].chars().next_back();
        let after = s[i + w.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}

pub fn run(b: &ResultBundle, _input: &Input, raw: &str, text: &str, unknown: &[String]) -> Vec<Check> {
    use CheckState::*;
    let mut out = Vec::new();
    let lower = text.to_lowercase();

    // Numbers: the only digits allowed are inside slots (decoding enforces this; this proves it).
    let mut outside = String::new();
    let mut depth = 0;
    for c in raw.chars() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ if depth == 0 => outside.push(c),
            _ => {}
        }
    }
    out.push(if !unknown.is_empty() {
        check("numbers", "Numbers", Fail, format!("The draft cites values that don't exist: {}.", unknown.join(", ")))
    } else if outside.chars().any(char::is_numeric) {
        check("numbers", "Numbers", Fail, "The draft contains a number that didn't come from the result.")
    } else if spells_a_number(&outside, 0) {
        check("numbers", "Numbers", Fail, "The draft spells out a number in words instead of taking it from the result.")
    } else {
        check("numbers", "Numbers", Pass, "Every number comes from the result.")
    });

    // Formatting: italics are single asterisks in pairs, and slots read as part of a sentence.
    let stars = text.matches('*').count();
    let glitches: Vec<&str> = [" was =", " is =", "= =", ". .", ".  .", "= <", "were =", "of =", " , ", "[a, b]", "CI of 95% CI", "interval of 95% CI"].into_iter().filter(|g| text.contains(g)).collect();
    out.push(if text.contains("**") || stars % 2 == 1 {
        check("formatting", "Formatting", Fail, "The draft's italics are broken (stray or repeated asterisks).")
    } else if !glitches.is_empty() {
        check("formatting", "Formatting", Flag, format!("Garbled wording around a value (\"{}\"). Edit the sentence.", glitches.join("\", \"")))
    } else {
        check("formatting", "Formatting", Pass, "Formatting is clean.")
    });

    // Notation: the symbol in front of a value must be that value's statistic ("*t*(2) = {welch.F}" is wrong).
    let symbol = |slot: &str| -> Option<&'static str> {
        match slot.rsplit('.').next().unwrap_or(slot) {
            "t" => Some("t"),
            "F" => Some("F"),
            "r" => Some("r"),
            "U" => Some("U"),
            "H" => Some("H"),
            "z" => Some("z"),
            "mean" => Some("M"),
            "sd" => Some("SD"),
            "median" => Some("Mdn"),
            "d" => Some("d"),
            "g" => Some("g"),
            _ => None,
        }
    };
    let mut mislabeled = Vec::new();
    for (i, _) in raw.match_indices("= {") {
        let Some(name) = raw[i + 3..].split('}').next() else { continue };
        let Some(want) = symbol(name) else { continue };
        // The nearest italic symbol before the "=": *t*(...) = or *M* =.
        let before = &raw[..i];
        let Some(end) = before.rfind('*') else { continue };
        if i - end > 24 {
            continue;
        }
        let Some(start) = before[..end].rfind('*') else { continue };
        let got = &before[start + 1..end];
        if got.chars().all(char::is_alphabetic) && !got.is_empty() && got != want && !(want == "d" && got == "d") {
            mislabeled.push(format!("*{got}* for {name}"));
        }
    }
    out.push(if mislabeled.is_empty() {
        check("notation", "Statistical notation", Pass, "Each value has the right symbol.")
    } else {
        check("notation", "Statistical notation", Fail, format!("A value carries the wrong symbol: {}.", mislabeled.join(", ")))
    });

    // Coverage: the elements APA requires for this method.
    let missing: Vec<String> = required(b).into_iter().filter(|k| !raw.contains(&format!("{{{k}}}"))).collect();
    out.push(if missing.is_empty() {
        check("coverage", "Required statistics", Pass, "Every required statistic is reported.")
    } else {
        check("coverage", "Required statistics", Flag, format!("Not reported: {}. Add them or use the template paragraph.", missing.join(", ")))
    });

    // Direction: in a sentence that names both groups and a comparison, the group said to be higher must be higher.
    let pairs = [("g1.label", "g2.label"), ("x1.label", "x2.label")];
    let higher_first = b.number("diff").or_else(|| (b.method_id == "np.mann_whitney").then(|| b.number("r")).flatten()).map(|d| d > 0.0);
    let mut direction = check("direction", "Direction", Pass, "Comparisons agree with the result.");
    if let Some(first_is_higher) = higher_first {
        for (a, c) in pairs {
            let (Some(la), Some(lc)) = (b.text(a), b.text(c)) else { continue };
            for s in sentences(raw) {
                let ia = s.find(&format!("{{{a}}}")).or_else(|| s.find(la));
                let ic = s.find(&format!("{{{c}}}")).or_else(|| s.find(lc));
                let (Some(ia), Some(ic)) = (ia, ic) else { continue };
                let sl = s.to_lowercase();
                let up = ["higher", "greater", "larger", "more"].iter().filter_map(|w| sl.find(w)).min();
                let down = ["lower", "smaller", "less", "fewer"].iter().filter_map(|w| sl.find(w)).min();
                let says_first_higher = match (up, down) {
                    (Some(u), Some(d)) => u < d,
                    (Some(_), None) => true,
                    (None, Some(_)) => false,
                    (None, None) => continue,
                } == (ia < ic);
                if says_first_higher != first_is_higher {
                    direction = check("direction", "Direction", Fail, format!("This sentence gets the direction wrong: \"{}\"", s.trim()));
                }
            }
        }
    }
    out.push(direction);

    // Significance language, sentence by sentence: a sentence that calls something significant (or not) must agree
    // with every p value it cites. A sentence that cites none is judged against the main test's p, when there is one.
    let alpha = 1.0 - b.number("level").unwrap_or(0.95);
    let main_p = b.number("p").or_else(|| b.number("welch.p"));
    let mut sig = check("significance", "Significance", Pass, "Significance is described correctly.");
    for s in sentences(raw) {
        let sl = s.to_lowercase();
        // Every claim in the sentence, with where it is: (position, says significant).
        let negs = ["not significant", "non-significant", "nonsignificant", "not statistically significant", "did not differ significantly", "not significantly", "no significant"];
        let mut claims: Vec<(usize, bool)> = Vec::new();
        for (i, _) in sl.match_indices("significant") {
            let negated = negs.iter().any(|n| {
                let k = n.find("significant").unwrap_or(0);
                i >= k && sl[i - k..].starts_with(n)
            });
            claims.push((i, !negated));
        }
        if claims.is_empty() {
            continue;
        }
        // Each p value cited, paired with the nearest claim; a sentence citing none is judged by the main test.
        let cited: Vec<(usize, f64)> = s
            .match_indices('{')
            .filter_map(|(i, _)| s[i + 1..].split('}').next().map(|name| (i, name)))
            .filter(|(_, name)| *name == "p" || name.ends_with(".p"))
            .filter_map(|(i, name)| b.number(name).map(|v| (i, v)))
            .collect();
        // Without a cited p, a claim can only be judged when the analysis has a single test.
        let single_test = !b.fields.keys().any(|k| k.ends_with(".p") && k != "welch.p");
        let pairs: Vec<(bool, f64)> = if cited.is_empty() {
            main_p.filter(|_| single_test).map(|p| claims.iter().map(|(_, c)| (*c, p)).collect()).unwrap_or_default()
        } else {
            cited.iter().map(|(i, p)| (claims.iter().min_by_key(|(j, _)| j.abs_diff(*i)).expect("a claim").1, *p)).collect()
        };
        if let Some((says_sig, _)) = pairs.iter().find(|(c, p)| *c != (*p < alpha)) {
            sig = check(
                "significance",
                "Significance",
                Fail,
                format!("This sentence calls a result {} that {}: \"{}\"", if *says_sig { "significant" } else { "not significant" }, if *says_sig { "isn't" } else { "is" }, s.trim()),
            );
            break;
        }
    }
    let banned = ["no difference", "no effect", "no relationship", "no association", "proves", "proved", "prove that", "the same as"].iter().filter(|w| lower.contains(*w)).copied().collect::<Vec<_>>();
    if sig.state == Pass && !banned.is_empty() {
        sig = check("significance", "Significance", Flag, format!("Rephrase: \"{}\". A non-significant result doesn't show there is no difference, and no single study proves anything.", banned.join("\", \"")));
    }
    out.push(sig);

    // Causal language: no design has been recorded as randomized.
    let causal: Vec<&str> = ["caused", "causes", "led to", "leads to", "resulted in", "due to", "because of", "impact of", "effect of"].into_iter().filter(|w| contains_word(&lower, w)).collect();
    out.push(if causal.is_empty() {
        check("causal", "Causal language", Pass, "No causal claims.")
    } else {
        check("causal", "Causal language", Flag, format!("\"{}\" claims a cause. That needs a randomized design; otherwise describe a difference or an association.", causal.join("\", \"")))
    });

    // Checks that need review should be acknowledged.
    let review: Vec<&str> = b.diagnostics.iter().filter(|d| d.state == DiagnosticState::ReviewNeeded).map(|d| d.title.as_str()).collect();
    out.push(if review.is_empty() {
        check("diagnostics", "Checks needing review", Pass, "No check needs review.")
    } else {
        check("diagnostics", "Checks needing review", Flag, format!("Make sure the paragraph is honest about: {}.", review.join(", ")))
    });
    out
}
