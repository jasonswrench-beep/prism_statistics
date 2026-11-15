//! APA formatting and the template writer (GUIDE-07).
//!
//! Formatting lives in one place so tables, the template writer and the AI writer all write numbers the same way.
//! Italic statistic symbols are marked with asterisks (`*t*`), which the interface and the exports turn into italics.

use crate::result::ResultBundle;

/// A number with a fixed number of decimals, with a true minus sign.
pub fn num(x: f64, decimals: usize) -> String {
    if x.is_infinite() {
        return if x > 0.0 { "∞".into() } else { "−∞".into() };
    }
    let s = format!("{:.*}", decimals, x);
    // "-0.00" is not a number anyone should read.
    let s = if s.trim_start_matches('-').chars().all(|c| c == '0' || c == '.') { s.trim_start_matches('-').to_string() } else { s };
    s.replace('-', "\u{2212}")
}

/// A value as a person would type it (60, 2.5), with a true minus sign.
pub fn plain(x: f64) -> String {
    crate::model::format_code(x).replace('-', "\u{2212}")
}

/// A statistic that can't exceed 1 in absolute value (p, r): no leading zero (".045").
pub fn no_leading_zero(x: f64, decimals: usize) -> String {
    let s = num(x, decimals);
    if let Some(rest) = s.strip_prefix("0.") {
        format!(".{rest}")
    } else if let Some(rest) = s.strip_prefix("\u{2212}0.") {
        format!("\u{2212}.{rest}")
    } else {
        s
    }
}

/// "= .045" or "< .001".
pub fn p_clause(p: f64) -> String {
    if p < 0.001 {
        "< .001".to_string()
    } else {
        format!("= {}", no_leading_zero(p, 3))
    }
}

/// The p value as a table cell: ".045" or "< .001".
pub fn p_cell(p: f64) -> String {
    if p < 0.001 {
        "< .001".to_string()
    } else {
        no_leading_zero(p, 3)
    }
}

/// Degrees of freedom: whole numbers as integers, Welch-type fractional df with two decimals.
pub fn df(x: f64) -> String {
    if (x - x.round()).abs() < 1e-9 {
        format!("{}", x.round() as i64)
    } else {
        num(x, 2)
    }
}

/// A confidence interval: "95% CI [0.39, 0.74]".
pub fn ci(level: f64, lo: f64, hi: f64, decimals: usize) -> String {
    format!("{}% CI [{}, {}]", num(level * 100.0, 0), num(lo, decimals), num(hi, decimals))
}

/// Capitalizes the first letter, for a label that starts a sentence.
pub fn sentence_case(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Numbers below ten are written as words in running text (APA 6.32), except when they precede a unit.
pub fn count_words(n: usize) -> String {
    const W: [&str; 10] = ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine"];
    if n < 10 {
        W[n].to_string()
    } else {
        n.to_string()
    }
}

/// The template paragraph for an independent-samples t test, or None when the result may not be interpreted.
/// True when the result's test was one-sided.
pub fn one_sided(r: &ResultBundle) -> bool {
    matches!(r.text("alternative"), Some("greater" | "less"))
}

/// The p clause ("= .031"), marked "(one-sided)" for a one-sided test.
pub fn sided_p(r: &ResultBundle, p: f64) -> String {
    if one_sided(r) {
        format!("{} (one-sided)", p_clause(p))
    } else {
        p_clause(p)
    }
}

/// The interval stored under `{prefix}.lower` and `{prefix}.upper`, with ∞ for the open end of a one-sided interval
/// (which is not stored).
pub fn sided_ci(r: &ResultBundle, level: f64, prefix: &str, decimals: usize) -> Option<String> {
    let get = |end: &str| r.number(&format!("{prefix}.{end}")).filter(|v| v.is_finite());
    let (lo, hi) = match r.text("alternative") {
        Some("greater") => (get("lower")?, f64::INFINITY),
        Some("less") => (f64::NEG_INFINITY, get("upper")?),
        _ => (get("lower")?, get("upper")?),
    };
    Some(ci(level, lo, hi, decimals))
}

pub fn independent_t_paragraph(r: &ResultBundle) -> Option<String> {
    if !r.is_ok() {
        return None;
    }
    let y = r.text("y.label")?;
    let (g1, g2) = (r.text("g1.label")?, r.text("g2.label")?);
    let (m1, sd1, m2, sd2) = (r.number("g1.mean")?, r.number("g1.sd")?, r.number("g2.mean")?, r.number("g2.sd")?);
    let (t, dfv, p) = (r.number("t")?, r.number("df")?, r.number("p")?);
    let (diff, level) = (r.number("diff")?, r.number("level")?);
    let interval = sided_ci(r, level, "ci", 2)?;
    let (g, glo, ghi) = (r.number("g")?, r.number("g.ci.lower")?, r.number("g.ci.upper")?);
    let method = r.text("method")?;
    let alpha = 1.0 - level;
    let comparison = if diff > 0.0 { "higher" } else { "lower" };
    let stats = format!(
        "*t*({}) = {}, *p* {}, mean difference = {}, {interval}, Hedges' *g* = {}, {}",
        df(dfv),
        num(t, 2),
        sided_p(r, p),
        num(diff, 2),
        num(g, 2),
        ci(level, glo, ghi, 2)
    );
    let mut text = format!("An independent-samples *t* test ({method}) compared {} between the {g1} and {g2} groups. ", y);
    if p < alpha {
        text += &format!(
            "{} was {comparison} in the {g1} group (*M* = {}, *SD* = {}) than in the {g2} group (*M* = {}, *SD* = {}), {stats}.",
            sentence_case(y),
            num(m1, 2),
            num(sd1, 2),
            num(m2, 2),
            num(sd2, 2)
        );
    } else {
        // A non-significant result is not evidence of no difference, so the sentence says only what was observed.
        text += &format!(
            "The {g1} group (*M* = {}, *SD* = {}) and the {g2} group (*M* = {}, *SD* = {}) did not differ significantly, {stats}. {}",
            num(m1, 2),
            num(sd1, 2),
            num(m2, 2),
            num(sd2, 2),
            if one_sided(r) {
                "This result does not show that the groups are the same."
            } else {
                "The interval is compatible with differences in either direction, so this result does not show that the groups are the same."
            }
        );
    }
    Some(text)
}

/// " In total, three cases were excluded from this analysis (reason: n; ...)." or nothing when none were.
pub fn exclusion_sentence(r: &ResultBundle) -> String {
    let excluded = r.cases.total - r.cases.analyzed;
    if excluded == 0 {
        return String::new();
    }
    let word = if excluded == 1 { "case was" } else { "cases were" };
    format!(" In total, {} {word} excluded from this analysis ({}).", count_words(excluded), exclusion_list(r))
}

/// Joins items as "a, b, and c" (APA uses the serial comma).
pub fn series(items: &[String]) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].clone(),
        2 => format!("{} and {}", items[0], items[1]),
        n => format!("{}, and {}", items[..n - 1].join(", "), items[n - 1]),
    }
}

/// A participants description from Frequencies results (categorical variables) and one Descriptives result (measured
/// variables), reporting only the statistics that were run (10 §3a). Percentages are of all participants, with "did
/// not respond" as its own category, so each list adds to 100%.
pub fn participants_paragraph(frequencies: &[&ResultBundle], descriptives: Option<&ResultBundle>) -> Option<String> {
    use crate::analysis::frequencies::categories;
    let mut sentences = Vec::new();
    let total = frequencies.iter().chain(descriptives.iter()).find_map(|b| b.number("n.total").or(Some(b.cases.total as f64)))? as usize;
    sentences.push(format!("The sample included {total} participants."));
    for b in frequencies.iter().filter(|b| b.is_ok()) {
        let label = b.text("var.label")?;
        let n_total = b.number("n.total")? as usize;
        let missing = b.number("n.missing")? as usize;
        let pct = |n: usize| num(100.0 * n as f64 / n_total as f64, 1);
        let mut items: Vec<String> = categories(b).iter().map(|c| format!("{}% {} (*n* = {})", pct(c.count), c.label, c.count)).collect();
        if missing > 0 {
            items.push(format!("{}% did not respond (*n* = {missing})", pct(missing)));
        }
        sentences.push(format!("For {label}, {}.", series(&items)));
    }
    if let Some(d) = descriptives.filter(|d| d.is_ok()) {
        let stats: Vec<&str> = d.text("statistics").unwrap_or("").split(',').collect();
        let has = |s: &str| stats.contains(&s);
        for key in d.text("variables").unwrap_or("").split('\u{1f}').filter(|k| !k.is_empty()) {
            let label = d.text(&format!("{key}.label"))?;
            let get = |s: &str| d.number(&format!("{key}.{s}"));
            let n = get("n")? as usize;
            let mut parts = Vec::new();
            if n < total {
                parts.push(format!("Of the {total} participants, {n} reported {label}."));
            }
            match (has("mean").then(|| get("mean")).flatten(), has("sd").then(|| get("sd")).flatten()) {
                (Some(m), Some(sd)) => parts.push(format!("The mean {label} was {} (*SD* = {}).", num(m, 2), num(sd, 2))),
                (Some(m), None) => parts.push(format!("The mean {label} was {}.", num(m, 2))),
                (None, Some(sd)) => parts.push(format!("The standard deviation of {label} was {}.", num(sd, 2))),
                (None, None) => {}
            }
            let (min, max) = (has("min").then(|| get("min")).flatten(), has("max").then(|| get("max")).flatten());
            let range = has("range").then(|| get("range")).flatten();
            match (min, max) {
                (Some(lo), Some(hi)) => {
                    let r = range.map(|r| format!(", a range of {}", num(r, 2))).unwrap_or_default();
                    parts.push(format!("The lowest {label} was {} and the highest was {}{r}.", num(lo, 2), num(hi, 2)));
                }
                (Some(lo), None) => parts.push(format!("The lowest {label} was {}.", num(lo, 2))),
                (None, Some(hi)) => parts.push(format!("The highest {label} was {}.", num(hi, 2))),
                (None, None) => {
                    if let Some(r) = range {
                        parts.push(format!("The range of {label} was {}.", num(r, 2)));
                    }
                }
            }
            if let Some(md) = has("median").then(|| get("median")).flatten() {
                parts.push(format!("The median {label} was {}.", num(md, 2)));
            }
            sentences.extend(parts);
        }
    }
    (sentences.len() > 1).then(|| sentences.join(" "))
}

fn exclusion_list(r: &ResultBundle) -> String {
    r.cases.excluded.iter().map(|e| format!("{}: {}", e.reason, e.count)).collect::<Vec<_>>().join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_like_apa() {
        assert_eq!(p_clause(0.0004), "< .001");
        assert_eq!(p_clause(0.0451), "= .045");
        assert_eq!(p_cell(0.5), ".500");
        assert_eq!(num(-0.004, 2), "0.00");
        assert_eq!(num(-1.5, 2), "\u{2212}1.50");
        assert_eq!(no_leading_zero(-0.25, 2), "\u{2212}.25");
        assert_eq!(df(232.0), "232");
        assert_eq!(df(225.4567), "225.46");
        assert_eq!(ci(0.95, 0.388, 0.738, 2), "95% CI [0.39, 0.74]");
        assert_eq!(count_words(3), "three");
        assert_eq!(count_words(12), "12");
    }
}
