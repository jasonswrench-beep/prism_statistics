//! Formulas (Compute Variable, Select Cases) written as R code for the R script export, without ever copying the stored
//! text into the script. A project is a file anyone can edit, so its formula text is untrusted: it is first read by the
//! engine's own parser (anything it can't read is refused), then rebuilt token by token. Numbers keep only digits,
//! names become quoted R names, strings become quoted R strings, and functions and operators come from the formula
//! language's own short lists. Nothing else can reach the script.

/// The functions of the formula language, with their R spelling. The row functions are defined by `ROW_HELPERS`.
const FUNCTIONS: &[&str] = &["abs", "sqrt", "exp", "log", "log2", "log10", "sin", "cos", "tan", "floor", "ceiling", "trunc", "round", "pmin", "pmax", "ifelse", "is.na", "as.numeric", "rowmean", "rowsum", "rowmin", "rowmax", "nmiss", "c"];

/// R versions of the language's functions across variables, with the engine's rule that a case with no valid value
/// gets a missing result.
pub const ROW_HELPERS: &[&str] = &[
    "# The formula language's functions across variables, as PrismStatistics works them out.",
    "rowfun <- function(f, ...) { m <- cbind(...); apply(m, 1, function(v) { v <- v[!is.na(v)]; if (length(v)) f(v) else NA_real_ }) }",
    "rowmean <- function(...) rowfun(mean, ...); rowsum <- function(...) rowfun(sum, ...)",
    "rowmin <- function(...) rowfun(min, ...); rowmax <- function(...) rowfun(max, ...)",
    "nmiss <- function(...) rowSums(is.na(cbind(...)))",
];

fn q(s: &str) -> String {
    crate::rscript::q(s)
}

/// A variable name as an R name in backticks, with backslashes and backticks escaped.
pub fn name(n: &str) -> String {
    format!("`{}`", n.replace('\\', "\\\\").replace('`', "\\`").replace(['\r', '\n'], " "))
}

/// The R code for a formula, and whether it uses the row functions. Err with the reason when the engine can't read it.
pub fn to_r(text: &str) -> Result<(String, bool), String> {
    prism_engine::data_ops::check_formula(text)?;
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut rows = false;
    let mut i = 0;
    let push = |out: &mut String, s: &str| {
        if !out.is_empty() && !out.ends_with('(') && !s.starts_with([')', ',']) {
            out.push(' ');
        }
        out.push_str(s);
    };
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                let mut j = i + 1;
                if j < chars.len() && (chars[j] == '+' || chars[j] == '-') {
                    j += 1;
                }
                if j < chars.len() && chars[j].is_ascii_digit() {
                    while j < chars.len() && chars[j].is_ascii_digit() {
                        j += 1;
                    }
                    i = j;
                }
            }
            let digits: String = chars[start..i].iter().collect();
            let v: f64 = digits.parse().map_err(|_| format!("'{digits}' is not a number"))?;
            push(&mut out, &format!("{v:?}"));
        } else if c == '"' || c == '\'' {
            let quote = c;
            i += 1;
            let mut s = String::new();
            loop {
                match chars.get(i) {
                    None => return Err("a quotation mark is not closed".into()),
                    Some('\\') if i + 1 < chars.len() => {
                        s.push(chars[i + 1]);
                        i += 2;
                    }
                    Some(&ch) if ch == quote => {
                        i += 1;
                        break;
                    }
                    Some(&ch) => {
                        s.push(ch);
                        i += 1;
                    }
                }
            }
            push(&mut out, &q(&s));
        } else if c == '`' {
            i += 1;
            let start = i;
            while i < chars.len() && chars[i] != '`' {
                i += 1;
            }
            let n: String = chars[start..i.min(chars.len())].iter().collect();
            i += 1;
            push(&mut out, &name(&n));
        } else if c.is_alphabetic() || c == '.' || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '.' || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let call = chars[i..].iter().find(|ch| !ch.is_whitespace()) == Some(&'(');
            if call {
                // The parser accepted it, so it is one of the language's functions; check again all the same.
                let f = FUNCTIONS.iter().find(|f| **f == word).ok_or_else(|| format!("there is no function named {word}"))?;
                rows |= f.starts_with("row") || *f == "nmiss";
                push(&mut out, f);
            } else {
                match word.as_str() {
                    "TRUE" | "FALSE" | "NA" | "pi" | "Inf" => push(&mut out, &word),
                    _ => push(&mut out, &name(&word)),
                }
            }
        } else if c == '%' {
            let end = chars[i + 1..].iter().position(|&x| x == '%').map(|p| i + 1 + p).ok_or("a % sign is not closed")?;
            let s: String = chars[i..=end].iter().collect();
            let op = ["%in%", "%%", "%/%"].into_iter().find(|o| *o == s).ok_or_else(|| format!("'{s}' is not an operator"))?;
            push(&mut out, op);
            i = end + 1;
        } else {
            let two: String = chars[i..chars.len().min(i + 2)].iter().collect();
            let op2 = match two.as_str() {
                "==" | "!=" | "<=" | ">=" => Some(two.clone()),
                "&&" => Some("&".to_string()),
                "||" => Some("|".to_string()),
                _ => None,
            };
            if let Some(op) = op2 {
                push(&mut out, &op);
                i += 2;
                continue;
            }
            let one = match c {
                '+' | '-' | '*' | '/' | '^' | '<' | '>' | '&' | '|' | '!' | '(' | ')' | ',' => c.to_string(),
                _ => return Err(format!("'{c}' is not something the formula language uses")),
            };
            if one == "(" {
                // A call's parenthesis follows its name directly.
                if out.ends_with(|ch: char| ch.is_alphanumeric() || ch == '.' || ch == '_') && FUNCTIONS.iter().any(|f| out.ends_with(f)) {
                    out.push('(');
                } else {
                    push(&mut out, "(");
                }
            } else {
                push(&mut out, &one);
            }
            i += 1;
        }
    }
    Ok((out, rows))
}

#[cfg(test)]
mod tests {
    use super::to_r;

    #[test]
    fn formulas_become_r_and_nothing_else_does() {
        assert_eq!(to_r("(pre + post) / 2").unwrap().0, "(`pre` + `post`) / 2.0");
        assert_eq!(to_r("ifelse(age >= 18 & site == \"North\", 1, 0)").unwrap().0, "ifelse(`age` >= 18.0 & `site` == \"North\", 1.0, 0.0)");
        assert_eq!(to_r("`Stat age` * 2").unwrap().0, "`Stat age` * 2.0");
        assert_eq!(to_r("g %in% c(1, 2) && !is.na(x)").unwrap().0, "`g` %in% c(1.0, 2.0) & ! is.na(`x`)");
        let (r, rows) = to_r("rowmean(a, b) > 3").unwrap();
        assert!(rows && r == "rowmean(`a`, `b`) > 3.0", "{r}");
        // Text that tries to become code stays a string or a name, or is refused.
        assert_eq!(to_r("x == \"a\\\"); unlink('~')\"").unwrap().0, "`x` == \"a\\\"); unlink('~')\"");
        assert_eq!(to_r("`a`` + 1").unwrap_err(), to_r("`a`` + 1").unwrap_err(), "an unclosed backtick is refused");
        for bad in ["system('x')", "x; unlink('~')", "x\nsystem('x')", "x # c", "x <- 1", "x = 1", "{x}", "x$y", "unlink(dir())", "`x\\` + 1"] {
            let r = to_r(bad);
            assert!(r.is_err() || !r.as_ref().unwrap().0.contains("unlink") && !r.as_ref().unwrap().0.contains("system"), "{bad}: {r:?}");
            if let Ok((code, _)) = &r {
                assert!(!code.contains('\n') && !code.contains(';') && !code.contains('#'), "{bad}: {code}");
            }
        }
        assert!(to_r("unlink(dir())").is_err() && to_r("system('x')").is_err() && to_r("x; y").is_err());
    }
}
