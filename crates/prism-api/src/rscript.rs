//! File › Export › R script: the project's record as an R script, so the analysis can be checked or extended in R.
//!
//! The script reads the original data file, applies the variable settings (renamed columns, user-missing codes) the
//! way PrismStatistics does, and then runs each recorded analysis in order with base R (plus `haven` for .sav, .dta
//! and .sas7bdat files, `readxl` for workbooks, and `pwr` for two power analyses). The R functions are the ones the
//! reference tests check PrismStatistics against, with the same choices (Welch's t test, no continuity correction, sum
//! contrasts for Type III sums of squares), so the numbers should agree. Where R's defaults differ, the script says so.

use prism_engine::model::Variable;
use prism_engine::project::Project;
use serde_json::Value;

/// A variable as an R expression on the prepared data: x[["name"]].
fn col(name: &str) -> String {
    format!("x[[{}]]", q(name))
}

/// An R string literal. Backslashes, quotes, line breaks and other control characters are escaped, so the string can
/// neither end early nor span lines.
pub(crate) fn q(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:04x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A name for use inside an R formula: in backticks, with backslashes and backticks escaped.
fn f(name: &str) -> String {
    crate::rexpr::name(name)
}

/// Settings that are written into R code as they are (variable names, option words) may hold only letters, digits,
/// spaces, `.`, `_` and `-`. Free text (formulas, models, file names, labels) goes through `q()` or the formula
/// translator instead. A project is a file anyone can edit, so an operation whose settings fail this check is left out
/// of the script rather than trusted.
const FREE_TEXT: &[&str] = &["formula", "filter", "model", "path", "file_name", "label", "title", "text", "sheet", "reference", "event"];

fn plain(v: &Value) -> bool {
    fn ok(s: &str) -> bool {
        s.chars().all(|c| c.is_alphanumeric() || matches!(c, ' ' | '.' | '_' | '-'))
    }
    match v {
        Value::String(s) => ok(s),
        Value::Array(a) => a.iter().all(plain),
        Value::Object(o) => o.iter().all(|(k, v)| ok(k) && (FREE_TEXT.contains(&k.as_str()) && !matches!(v, Value::Object(_) | Value::Array(_)) || plain(v))),
        _ => true,
    }
}

fn strs(v: &Value) -> Vec<String> {
    match v {
        Value::Array(a) => a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        Value::String(s) => vec![s.clone()],
        _ => Vec::new(),
    }
}

fn vec_r(items: &[String]) -> String {
    format!("c({})", items.iter().map(|s| q(s)).collect::<Vec<_>>().join(", "))
}

fn num(v: &Value, default: f64) -> f64 {
    v.as_f64().unwrap_or(default)
}

fn level(a: &Value) -> f64 {
    num(&a["level"], 0.95)
}

fn r_num(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        format!("{x}")
    }
}

/// The lines that read the data file into `d`.
fn read_data(p: &Project) -> Vec<String> {
    let Some(src) = &p.source else {
        return vec!["# No data file was opened (only power analyses), so there is nothing to read.".into(), "d <- data.frame()".into()];
    };
    let name = &src.file_name;
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    let path = q(name);
    let mut out = vec![format!("# The data: {name}. Put the file in R's working directory (getwd()) or give its full path.")];
    match ext.as_str() {
        "csv" => out.push(format!("d <- read.csv({path}, check.names = FALSE, na.strings = \"\")")),
        "tsv" | "txt" | "tab" => out.push(format!("d <- read.delim({path}, check.names = FALSE, na.strings = \"\")")),
        "xlsx" | "xlsm" | "xls" => out.push(format!("d <- as.data.frame(readxl::read_excel({path}{}))  # install.packages(\"readxl\") once", src.report.sheet.as_ref().map(|s| format!(", sheet = {}", q(s))).unwrap_or_default())),
        "ods" => out.push(format!("d <- as.data.frame(readODS::read_ods({path}))  # install.packages(\"readODS\") once")),
        "sav" | "zsav" | "dta" | "sas7bdat" => {
            let fun = match ext.as_str() {
                "dta" => "read_dta",
                "sas7bdat" => "read_sas",
                _ => "read_sav",
            };
            out.push("# haven reads the file; values the file declares user-missing become NA, as in PrismStatistics.".to_string());
            let opt = if fun == "read_sav" { ", user_na = TRUE" } else { "" };
            out.push(format!("d <- haven::{fun}({path}{opt})  # install.packages(\"haven\") once"));
            out.push("d <- as.data.frame(lapply(d, function(v) { v <- haven::zap_labels(haven::zap_missing(v)); attributes(v) <- NULL; v }))".into());
            out.push("# Codes set as missing in PrismStatistics are applied by prep() below.".into());
        }
        "jasp" => {
            out.push(format!("# {name} is a .jasp file. Save its data as a .csv file with the same name, then:"));
            out.push(format!("d <- read.csv(sub(\"\\\\.jasp$\", \".csv\", {path}), check.names = FALSE, na.strings = \"\")"));
        }
        _ => out.push(format!("d <- read.csv({path}, check.names = FALSE, na.strings = \"\")  # adjust for this file type")),
    }
    for (from, to) in &src.report.renamed {
        out.push(format!("names(d)[names(d) == {}] <- {}  # renamed on import, as in PrismStatistics", q(from), q(to)));
    }
    out
}

/// R code reading another data file (for an append or merge) into d2.
fn read_other(name: &str) -> String {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    let path = q(name);
    match ext.as_str() {
        "tsv" | "txt" | "tab" => format!("d2 <- read.delim({path}, check.names = FALSE, na.strings = \"\")"),
        "xlsx" | "xlsm" | "xls" => format!("d2 <- as.data.frame(readxl::read_excel({path}))"),
        "sav" | "zsav" => format!("d2 <- as.data.frame(lapply(haven::read_sav({path}), function(v) {{ v <- haven::zap_labels(v); attributes(v) <- NULL; v }}))"),
        "dta" => format!("d2 <- as.data.frame(lapply(haven::read_dta({path}), function(v) {{ v <- haven::zap_labels(v); attributes(v) <- NULL; v }}))"),
        _ => format!("d2 <- read.csv({path}, check.names = FALSE, na.strings = \"\")"),
    }
}

/// `prep()`: user-missing codes and ranges become NA, with the variable settings as they are now.
fn prep(vars: &[Variable]) -> Vec<String> {
    let mut out = vec!["prep <- function(x) {".to_string()];
    let mut any = false;
    for v in vars {
        if !v.missing_values.is_empty() {
            any = true;
            // A variable made later in the script (Compute, Reverse Score...) isn't there yet when prep() first runs.
            out.push(format!("  if ({n} %in% names(x)) {c}[{c} %in% c({})] <- NA", v.missing_values.iter().map(|m| r_num(*m)).collect::<Vec<_>>().join(", "), c = col(&v.name), n = q(&v.name)));
        }
        for (lo, hi) in &v.missing_ranges {
            any = true;
            out.push(format!("  if ({n} %in% names(x)) {c}[!is.na({c}) & {c} >= {} & {c} <= {}] <- NA", r_num(*lo), r_num(*hi), c = col(&v.name), n = q(&v.name)));
        }
    }
    if !any {
        out.push("  # No user-missing codes are set.".into());
    }
    out.push("  x".into());
    out.push("}".into());
    out
}

/// The R code for one recorded analysis, or None for a command scripts don't repeat.
fn analysis(cmd: &str, a: &Value) -> Option<Vec<String>> {
    let lv = level(a);
    let mut o: Vec<String> = Vec::new();
    match cmd {
        "frequencies" => {
            for v in strs(&a["variables"]) {
                o.push(format!("tb <- table({}, useNA = \"ifany\"); print(cbind(Frequency = tb, Percent = round(100 * prop.table(tb), 1)))", col(&v)));
            }
        }
        "descriptives" => {
            o.push(format!("vars <- {}", vec_r(&strs(&a["variables"]))));
            o.push("print(t(sapply(x[vars], function(v) c(N = sum(!is.na(v)), Mean = mean(v, na.rm = TRUE), SD = sd(v, na.rm = TRUE), Min = min(v, na.rm = TRUE), Max = max(v, na.rm = TRUE), Median = median(v, na.rm = TRUE)))))".into());
        }
        "ttest_independent" => {
            let alt = alt_arg(a);
            let g = a["grouping_variable"].as_str()?;
            let groups = a["groups"].as_array()?;
            let (g1, g2) = (num(&groups[0], 1.0), num(&groups[1], 2.0));
            let welch = a["estimator"].as_str() != Some("student");
            for v in strs(&a["test_variables"]) {
                o.push(format!("print(t.test({c}[{gc} %in% {}], {c}[{gc} %in% {}], var.equal = {}, conf.level = {lv}{alt}))  # first group minus second", r_num(g1), r_num(g2), if welch { "FALSE" } else { "TRUE" }, c = col(&v), gc = col(g)));
            }
        }
        "ttest_one_sample" => {
            for v in strs(&a["test_variables"]) {
                o.push(format!("print(t.test({}, mu = {}, conf.level = {lv}{}))", col(&v), r_num(num(&a["test_value"], 0.0)), alt_arg(a)));
            }
        }
        "ttest_paired" => {
            for p in a["pairs"].as_array()? {
                let p = strs(p);
                o.push(format!("print(t.test({}, {}, paired = TRUE, conf.level = {lv}{}))", col(&p[0]), col(&p[1]), alt_arg(a)));
            }
        }
        "anova_oneway" => {
            let fac = a["factor"].as_str()?;
            for y in strs(&a["dependent"]) {
                o.push(format!("fit <- aov({} ~ factor({}), data = x); print(summary(fit))", f(&y), f(fac)));
                if a["estimator"].as_str() == Some("welch") {
                    o.push(format!("print(oneway.test({} ~ factor({}), data = x, var.equal = FALSE))", f(&y), f(fac)));
                }
                match a["post_hoc"].as_str() {
                    Some("tukey") => o.push(format!("print(TukeyHSD(fit, conf.level = {lv}))  # R gives later minus earlier group")),
                    Some("games_howell") => o.push("# Games-Howell isn't in base R; rstatix::games_howell_test() gives it.".into()),
                    Some(m @ ("bonferroni" | "holm")) => o.push(format!("print(pairwise.t.test(x${}, x${}, p.adjust.method = \"{m}\"))", f(&y), f(fac))),
                    Some("scheffe") => o.push("# Scheffe's test: DescTools::ScheffeTest(fit) gives it.".into()),
                    _ => {}
                }
                let cons: Vec<String> = a["contrasts"].as_array().map(|cs| cs.iter().filter_map(|c| c.as_array()).map(|c| format!("c({})", c.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "))).collect()).unwrap_or_default();
                let adj = a["contrast_adjust"].as_str().unwrap_or("none");
                if !cons.is_empty() {
                    let named: Vec<String> = cons.iter().enumerate().map(|(i, c)| format!("c{} = {c}", i + 1)).collect();
                    o.push(format!("print(summary(emmeans::contrast(emmeans::emmeans(fit, ~ factor({})), list({}), adjust = \"{adj}\"), infer = TRUE))  # install.packages(\"emmeans\") once", f(fac), named.join(", ")));
                }
                if a["polynomial"].as_bool() == Some(true) {
                    o.push(format!("print(summary(lm({} ~ C(factor({}), contr.poly), data = x)))  # .L, .Q and .C rows are the trends", f(&y), f(fac)));
                }
                if a["plots"].as_bool() == Some(true) {
                    o.push("par(mfrow = c(1, 2)); plot(fit, which = 1:2)".into());
                }
            }
        }
        "crosstabs" => {
            let (r, c) = (a["row"].as_str()?, a["column"].as_str()?);
            o.push(format!("tab <- table({}, {}); print(addmargins(tab))", col(r), col(c)));
            if a["chi_square"].as_bool() != Some(false) {
                o.push(format!("print(chisq.test(tab, correct = {}))", if a["continuity"].as_bool() == Some(true) { "TRUE" } else { "FALSE" }));
            }
            if a["fisher"].as_bool() == Some(true) {
                o.push("if (all(dim(tab) == 2)) print(fisher.test(tab))".into());
            }
            if a["ordinal"].as_bool() == Some(true) {
                o.push("print(c(gamma = DescTools::GoodmanKruskalGamma(tab), tau_b = DescTools::KendallTauB(tab), tau_c = DescTools::StuartTauC(tab)))  # install.packages(\"DescTools\") once".into());
            }
            if let Some(l) = a["layer"].as_str() {
                o.push(format!("tab3 <- table({}, {}, {}); print(mantelhaen.test(tab3, correct = FALSE)); print(DescTools::BreslowDayTest(tab3))", col(r), col(c), col(l)));
            }
        }
        "correlation" => {
            let vars = strs(&a["variables"]);
            let pairwise = a["missing"].as_str() == Some("pairwise");
            let kinds = strs(&a["coefficients"]);
            let kinds = if kinds.is_empty() { vec!["pearson".to_string()] } else { kinds };
            if !pairwise {
                o.push(format!("x <- x[complete.cases(x[{}]), ]  # listwise", vec_r(&vars)));
            }
            for k in &kinds {
                for i in 0..vars.len() {
                    for j in (i + 1)..vars.len() {
                        o.push(format!("print(cor.test({}, {}, method = {}, conf.level = {lv}{}{}))", col(&vars[i]), col(&vars[j]), q(k), if k == "pearson" { "" } else { ", exact = FALSE" }, alt_arg(a)));
                    }
                }
            }
            if let Some(m @ ("bonferroni" | "holm")) = a["adjust"].as_str() {
                o.push(format!("# PrismStatistics adjusted these p values for the number of pairs: p.adjust(c(p1, p2, ...), \"{m}\")."));
            }
        }
        "regression" | "logistic" => {
            let y = a["dependent"].as_str()?;
            let mut terms: Vec<String> = strs(&a["covariates"]).iter().map(|c| f(c)).collect();
            for fac in a["factors"].as_array().into_iter().flatten() {
                let name = fac["name"].as_str().or(fac.as_str())?;
                terms.push(match fac["reference"].as_str() {
                    Some(r) => format!("relevel(factor({}), ref = {})", f(name), q(r)),
                    None => format!("factor({})", f(name)),
                });
            }
            if cmd == "regression" {
                // Later blocks and interactions (hierarchical): each model adds its block; anova() tests the R² change.
                let mut models = vec![terms.clone()];
                for blk in a["blocks"].as_array().into_iter().flatten() {
                    let mut t = models.last().cloned().unwrap_or_default();
                    t.extend(strs(&blk["covariates"]).iter().map(|c| f(c)));
                    for fac in blk["factors"].as_array().into_iter().flatten() {
                        t.push(format!("factor({})", f(fac["name"].as_str().unwrap_or(""))));
                    }
                    models.push(t);
                }
                if let Some(ints) = a["interactions"].as_array().filter(|i| !i.is_empty()) {
                    if a["center"].as_bool() == Some(true) {
                        o.push("# Numeric predictors in interactions are mean-centered in PrismStatistics; center them here too, e.g. x$hours <- x$hours - mean(x$hours, na.rm = TRUE)".into());
                    }
                    let mut t = models.last().cloned().unwrap_or_default();
                    t.extend(ints.iter().map(|p| { let p = strs(p); format!("{}:{}", f(&p[0]), f(&p[1])) }));
                    models.push(t);
                }
                if models.len() > 1 {
                    for (k, t) in models.iter().enumerate() {
                        o.push(format!("m{} <- lm({} ~ {}, data = x)", k + 1, f(y), t.join(" + ")));
                    }
                    o.push(format!("print(anova({}))  # R² change, block by block", (1..=models.len()).map(|k| format!("m{k}")).collect::<Vec<_>>().join(", ")));
                    o.push(format!("m <- m{}", models.len()));
                    o.push(format!("print(summary(m)); print(confint(m, level = {lv}))"));
                } else {
                    o.push(format!("m <- lm({} ~ {}, data = x); print(summary(m)); print(confint(m, level = {lv}))", f(y), terms.join(" + ")));
                }
                if a["plots"].as_bool() == Some(true) {
                    o.push("plot(m, which = 1:2)  # residuals against fitted values, normal Q-Q".into());
                }
                if let Some(kind) = a["robust"].as_str() {
                    o.push(format!("print(lmtest::coeftest(m, vcov. = sandwich::vcovHC(m, type = \"{}\")))  # install.packages(c(\"sandwich\", \"lmtest\")) once", kind.to_uppercase()));
                }
            } else {
                let event = match a["event"].as_str() {
                    Some(e) => q(e),
                    None => format!("max({}, na.rm = TRUE)", col(y)),
                };
                o.push(format!("x$event <- as.integer({} == {event})  # the category modeled", col(y)));
                o.push(format!("m <- glm(event ~ {}, family = binomial, data = x); print(summary(m))", terms.join(" + ")));
                o.push(format!("print(exp(cbind(OR = coef(m), confint(m, level = {lv}))))  # profile-likelihood intervals"));
                if a["roc"].as_bool() == Some(true) {
                    o.push("r <- pROC::roc(m$y, fitted(m), quiet = TRUE); print(pROC::ci.auc(r, method = \"delong\")); plot(r)  # install.packages(\"pROC\") once".into());
                }
                if a["blocks"].as_array().is_some_and(|b| !b.is_empty()) || a["interactions"].as_array().is_some_and(|b| !b.is_empty()) {
                    o.push("# PrismStatistics entered later blocks and interactions step by step; fit each model with glm() and compare them with anova(m1, m2, test = \"Chisq\").".into());
                }
            }
        }
        "mann_whitney" => {
            let g = a["grouping_variable"].as_str()?;
            let groups = a["groups"].as_array()?;
            for v in strs(&a["test_variables"]) {
                o.push(format!("print(wilcox.test({c}[{gc} %in% {}], {c}[{gc} %in% {}]{}))", r_num(num(&groups[0], 1.0)), r_num(num(&groups[1], 2.0)), alt_arg(a), c = col(&v), gc = col(g)));
            }
        }
        "wilcoxon" => {
            for p in a["pairs"].as_array()? {
                let p = strs(p);
                o.push(format!("print(wilcox.test({}, {}, paired = TRUE{}))", col(&p[0]), col(&p[1]), alt_arg(a)));
            }
        }
        "kruskal" => {
            let fac = a["factor"].as_str()?;
            for y in strs(&a["dependent"]) {
                o.push(format!("print(kruskal.test({} ~ factor({}), data = x))", f(&y), f(fac)));
            }
        }
        "mcnemar" => {
            for p in a["pairs"].as_array()? {
                let p = strs(p);
                o.push(format!("tab <- table({}, {}); print(tab); print(mcnemar.test(tab)); print(binom.test(tab[1, 2], tab[1, 2] + tab[2, 1]))  # the exact test", col(&p[0]), col(&p[1])));
            }
        }
        "friedman" => {
            o.push(format!("print(friedman.test(as.matrix(na.omit(x[{}]))))", vec_r(&strs(&a["variables"]))));
        }
        "partial_correlation" => {
            let (vars, controls) = (strs(&a["variables"]), strs(&a["controls"]));
            let all: Vec<String> = vars.iter().chain(&controls).cloned().collect();
            o.push(format!("x <- x[complete.cases(x[{}]), ]; z <- as.matrix(x[{}])", vec_r(&all), vec_r(&controls)));
            for i in 0..vars.len() {
                for j in (i + 1)..vars.len() {
                    o.push(format!("r <- cor(resid(lm({} ~ z)), resid(lm({} ~ z))); df <- nrow(x) - 2 - ncol(z); print(c(partial_r = r, df = df, p = 2 * pt(-abs(r * sqrt(df / (1 - r^2))), df)))", col(&vars[i]), col(&vars[j])));
                }
            }
        }
        "sem" => {
            let model = a["model"].as_str()?;
            let mut opts = String::new();
            if a["std_lv"].as_bool() == Some(true) {
                opts += ", std.lv = TRUE";
            }
            if a["meanstructure"].as_bool() == Some(true) {
                opts += ", meanstructure = TRUE";
            }
            if a["fixed_x"].as_bool() == Some(false) {
                opts += ", fixed.x = FALSE";
            }
            match a["estimator"].as_str() {
                Some("mlm") if a["h1_information"].as_str() == Some("unstructured") => opts += ", estimator = \"MLM\", h1.information = \"unstructured\"",
                Some("mlm") => opts += ", estimator = \"MLM\"",
                Some("mlr") => opts += ", estimator = \"MLR\"",
                Some("wlsmv") => {
                    let ord = strs(&a["ordered"]);
                    opts += &if ord.is_empty() { ", ordered = TRUE".to_string() } else { format!(", ordered = {}", vec_r(&ord)) };
                }
                _ => {}
            }
            if a["missing"].as_str() == Some("fiml") {
                opts += ", missing = \"ml\"";
            }
            if let Some(c) = a["cluster"].as_str() {
                opts += &format!(", cluster = {}", q(c));
            }
            if let Some(g) = a["group"].as_str() {
                opts += &format!(", group = {}", q(g));
                let eq = strs(&a["group_equal"]);
                if !eq.is_empty() {
                    opts += &format!(", group.equal = {}", vec_r(&eq));
                }
                let pa = strs(&a["group_partial"]);
                if !pa.is_empty() {
                    opts += &format!(", group.partial = {}", vec_r(&pa.iter().map(|s| s.replace(' ', "")).collect::<Vec<_>>()));
                }
            }
            let boot = a["bootstrap"].as_u64().unwrap_or(0);
            if boot > 0 {
                opts += &format!(", se = \"bootstrap\", bootstrap = {boot}");
            }
            o.push(format!("m <- {}  # install.packages(\"lavaan\") once", q(model)));
            if boot > 0 {
                o.push(format!("set.seed({})  # R's random numbers differ from PrismStatistics', so bootstrap intervals differ slightly", a["seed"].as_u64().unwrap_or(20_260_101)));
            }
            let fun = if a["growth"].as_bool() == Some(true) { "growth" } else { "sem" };
            if a["estimator"].as_str() == Some("bayes") {
                o.push(format!(
                    "fit <- blavaan::bsem(m, data = x, n.chains = {}, burnin = {}, sample = {}, seed = {})  # install.packages(\"blavaan\") once; MCMC draws differ from PrismStatistics', within Monte Carlo error",
                    a["chains"].as_u64().unwrap_or(3),
                    a["burnin"].as_u64().unwrap_or(500),
                    a["samples"].as_u64().unwrap_or(1000),
                    a["seed"].as_u64().unwrap_or(20_260_101)
                ));
                o.push("print(summary(fit)); print(lavaan::fitMeasures(fit, c(\"ppp\", \"dic\", \"waic\")))".to_string());
                return Some(o);
            }
            o.push(format!("fit <- lavaan::{fun}(m, data = x{opts}); print(summary(fit, fit.measures = TRUE, standardized = TRUE, rsquare = TRUE))"));
            if boot > 0 {
                let kind = if a["bias_corrected"].as_bool() == Some(true) { "bca.simple" } else { "perc" };
                o.push(format!("print(lavaan::parameterEstimates(fit, boot.ci.type = \"{kind}\", level = {}))", num(&a["level"], 0.95)));
            }
            if a["reliability"].as_bool() != Some(false) && a["group"].is_null() {
                o.push("print(semTools::compRelSEM(fit)); print(semTools::AVE(fit)); print(semTools::htmt(m, x))  # install.packages(\"semTools\") once".to_string());
            }
            if a["modindices"].as_bool() != Some(false) {
                o.push("print(head(lavaan::modindices(fit, sort. = TRUE), 20))".to_string());
            }
        }
        "meta" => {
            let m = a["measure"].as_str().unwrap_or("generic");
            let col = |k: &str| a[k].as_str().map(f);
            o.push("library(metafor)  # install.packages(\"metafor\") once".to_string());
            let es = match m {
                "smd" | "md" => format!(
                    "d <- escalc(\"{}\", m1i = {}, sd1i = {}, n1i = {}, m2i = {}, sd2i = {}, n2i = {}, data = x)",
                    m.to_uppercase(),
                    col("mean1")?,
                    col("sd1")?,
                    col("n1")?,
                    col("mean2")?,
                    col("sd2")?,
                    col("n2")?
                ),
                "or" | "rr" | "rd" => format!("d <- escalc(\"{}\", ai = {}, n1i = {}, ci = {}, n2i = {}, data = x)", m.to_uppercase(), col("events1")?, col("n1")?, col("events2")?, col("n2")?),
                "zcor" => format!("d <- escalc(\"ZCOR\", ri = {}, ni = {}, data = x)", col("r")?, col("n1")?),
                _ => match (col("variance"), col("se")) {
                    (Some(v), _) => format!("d <- escalc(\"GEN\", yi = {}, vi = {}, data = x)", col("effect")?, v),
                    (None, Some(s)) => format!("d <- escalc(\"GEN\", yi = {}, sei = {}, data = x)", col("effect")?, s),
                    _ => return None,
                },
            };
            o.push(es);
            let mods = strs(&a["moderators"]);
            let method = a["method"].as_str().unwrap_or("reml").to_uppercase();
            if method == "BAYES" {
                let sc = a["tau_prior_scale"].as_f64().unwrap_or(0.5);
                let tp = match a["tau_prior"].as_str() {
                    Some("half_normal") => format!(", tau.prior = function(t) dhalfnormal(t, scale = {sc})"),
                    Some("half_cauchy") => format!(", tau.prior = function(t) dhalfcauchy(t, scale = {sc})"),
                    _ => String::new(),
                };
                let mp = match (a["mu_prior_mean"].as_f64(), a["mu_prior_sd"].as_f64()) {
                    (Some(m), Some(s)) => format!(", mu.prior.mean = {m}, mu.prior.sd = {s}"),
                    _ => String::new(),
                };
                o.push("library(bayesmeta)  # install.packages(\"bayesmeta\") once".to_string());
                o.push(format!("res <- bayesmeta(y = d$yi, sigma = sqrt(d$vi){tp}{mp}); print(res$summary); print(res$theta); forestplot(res)  # bayesmeta's mu and theta summaries use its DIRECT grid approximation"));
                return Some(o);
            }
            let mut call = match a["cluster"].as_str() {
                Some(c) => format!("d$es_id <- seq_len(nrow(d)); d$cluster <- d[[\"{c}\"]]; res <- rma.mv(yi, vi, random = ~ 1 | cluster/es_id, data = d, method = \"{method}\""),
                None => format!("res <- rma(yi, vi, data = d, method = \"{method}\""),
            };
            if !mods.is_empty() {
                call += &format!(", mods = ~ {}", mods.iter().map(|v| f(v)).collect::<Vec<_>>().join(" + "));
            }
            if a["knha"].as_bool() == Some(true) {
                call += if a["cluster"].is_string() { ", test = \"t\"" } else { ", test = \"knha\"" };
            }
            if let Some(l) = a["level"].as_f64() {
                call += &format!(", level = {}", 100.0 * l);
            }
            if let Some(s) = col("study") {
                call += &format!(", slab = {s}");
            }
            o.push(call + ")");
            o.push("print(res); print(confint(res)); if (res$p == 1) print(predict(res, transf = NULL))".to_string());
            let multi = a["cluster"].is_string();
            if a["bias"].as_bool() != Some(false) && mods.is_empty() && !multi {
                o.push("print(regtest(res)); print(trimfill(res))".to_string());
            }
            if a["leave_one_out"].as_bool() == Some(true) && !multi {
                o.push("print(leave1out(res))".to_string());
            }
            if mods.is_empty() {
                o.push("forest(res); funnel(res)".to_string());
            }
        }
        "lpa" => {
            let ind: Vec<String> = strs(&a["indicators"]);
            let models: Vec<String> = a["models"].as_array().map(|m| m.iter().filter_map(|v| v.as_str().map(|s| format!("\"{s}\""))).collect()).unwrap_or_else(|| ["EEI", "VVI", "EEE", "VVV"].iter().map(|s| format!("\"{s}\"")).collect());
            o.push("library(mclust)  # install.packages(\"mclust\") once".to_string());
            o.push(format!(
                "X <- na.omit(x[, {}]); b <- mclustBIC(X, G = {}:{}, modelNames = c({})); print(summary(b)); m <- Mclust(X, x = b); print(summary(m, parameters = TRUE)); plot(m, what = \"BIC\")  # mclust starts EM from a hierarchical clustering; PrismStatistics from many random starts",
                vec_r(&ind),
                a["min_classes"].as_u64().unwrap_or(1),
                a["max_classes"].as_u64().unwrap_or(6),
                models.join(", ")
            ));
        }
        "forest" => {
            let preds: Vec<String> = strs(&a["predictors"]).iter().map(|v| f(v)).collect();
            let mut extra = String::new();
            if let Some(m) = a["mtry"].as_u64() {
                extra += &format!(", mtry = {m}");
            }
            if let Some(m) = a["nodesize"].as_u64() {
                extra += &format!(", nodesize = {m}");
            }
            if a["replace"].as_bool() == Some(false) {
                extra += ", replace = FALSE";
            }
            if let Some(m) = a["sample_size"].as_u64() {
                extra += &format!(", sampsize = {m}");
            }
            o.push("library(randomForest)  # install.packages(\"randomForest\") once; its random draws differ from PrismStatistics'".to_string());
            o.push(format!(
                "set.seed({}); rf <- randomForest({} ~ {}, data = na.omit(x[, {}]), ntree = {}{extra}, importance = TRUE); print(rf); print(importance(rf)); varImpPlot(rf)",
                a["seed"].as_u64().unwrap_or(20_260_101),
                f(a["dependent"].as_str()?),
                preds.join(" + "),
                vec_r(&std::iter::once(a["dependent"].as_str()?.to_string()).chain(strs(&a["predictors"])).collect::<Vec<_>>()),
                a["n_trees"].as_u64().unwrap_or(500)
            ));
        }
        "boost" => {
            let preds: Vec<String> = strs(&a["predictors"]).iter().map(|v| f(v)).collect();
            let dist = match a["distribution"].as_str() {
                Some("bernoulli") => ", distribution = \"bernoulli\"",
                Some("gaussian") => ", distribution = \"gaussian\"",
                _ => "",
            };
            o.push("library(gbm)  # install.packages(\"gbm\") once; with bag.fraction = 1 the fit equals PrismStatistics'".to_string());
            o.push(format!(
                "set.seed({}); m <- gbm({} ~ {}, data = x{dist}, n.trees = {}, interaction.depth = {}, shrinkage = {}, bag.fraction = {}, n.minobsinnode = {}, cv.folds = {}); best <- if (m$cv.folds > 0) gbm.perf(m, method = \"cv\") else m$n.trees; print(summary(m, n.trees = best))",
                a["seed"].as_u64().unwrap_or(20_260_101),
                f(a["dependent"].as_str()?),
                preds.join(" + "),
                a["n_trees"].as_u64().unwrap_or(100),
                a["depth"].as_u64().unwrap_or(1),
                a["shrinkage"].as_f64().unwrap_or(0.1),
                a["bag_fraction"].as_f64().unwrap_or(0.5),
                a["min_obs"].as_u64().unwrap_or(10),
                a["cv_folds"].as_u64().unwrap_or(5)
            ));
        }
        "lca" => {
            let vars = strs(&a["variables"]);
            o.push("library(poLCA)  # install.packages(\"poLCA\") once; poLCA needs categories coded 1, 2, …".to_string());
            o.push(format!("set.seed({}); m <- poLCA(cbind({}) ~ 1, data = x, nclass = {}, nrep = {}, maxiter = 10000)  # random starts differ from PrismStatistics', but the best solution should agree", a["seed"].as_u64().unwrap_or(20_260_101), vars.iter().map(|v| f(v)).collect::<Vec<_>>().join(", "), a["classes"].as_u64().unwrap_or(2), a["starts"].as_u64().unwrap_or(30)));
        }
        "irt" => {
            let items = strs(&a["items"]);
            let kind = if a["model"].as_str() == Some("rasch") { "Rasch" } else { "graded" };
            o.push("library(mirt)  # install.packages(\"mirt\") once".to_string());
            o.push(format!("m <- mirt(x[, {}], 1, itemtype = \"{kind}\", SE = TRUE); print(coef(m, IRTpars = TRUE, printSE = TRUE)); print(c(logLik = logLik(m), AIC = extract.mirt(m, \"AIC\"), BIC = extract.mirt(m, \"BIC\")))", vec_r(&items)));
        }
        "pls" => {
            // The model in seminr's own functions: one composite() per block, paths() per outcome.
            let mut blocks: Vec<(String, Vec<String>, bool)> = Vec::new();
            let mut paths: Vec<(String, Vec<String>)> = Vec::new();
            for line in a["model"].as_str()?.lines() {
                let line = line.split('#').next().unwrap_or("").trim();
                let (lhs, rhs, kind) = if let Some((l, r)) = line.split_once("=~") {
                    (l, r, 'A')
                } else if let Some((l, r)) = line.split_once("<~") {
                    (l, r, 'B')
                } else if let Some((l, r)) = line.split_once('~') {
                    (l, r, 'P')
                } else {
                    continue;
                };
                let terms: Vec<String> = rhs.split('+').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
                if kind == 'P' {
                    paths.push((lhs.trim().to_string(), terms));
                } else {
                    blocks.push((lhs.trim().to_string(), terms, kind == 'B'));
                }
            }
            let comps: Vec<String> = blocks.iter().map(|(n, it, b)| format!("  composite({}, {}{})", q(n), vec_r(it), if *b { ", weights = mode_B" } else { "" })).collect();
            let rels: Vec<String> = paths.iter().map(|(to, from)| format!("  paths(from = {}, to = {})", vec_r(from), q(to))).collect();
            o.push("library(seminr)  # install.packages(\"seminr\") once".to_string());
            o.push(format!("mm <- constructs(\n{}\n)", comps.join(",\n")));
            o.push(format!("sm <- relationships(\n{}\n)", rels.join(",\n")));
            let scheme = if a["scheme"].as_str() == Some("factorial") { ", inner_weights = path_factorial" } else { "" };
            o.push(format!("pls <- estimate_pls(x, mm, sm{scheme}); print(summary(pls))"));
            if let Some(boot) = a["bootstrap"].as_u64().filter(|b| *b > 0) {
                o.push(format!("boot <- bootstrap_model(pls, nboot = {boot}, seed = {}); print(summary(boot))  # R's random numbers differ, so intervals differ slightly", a["seed"].as_u64().unwrap_or(20_260_101)));
            }
        }
        "sem_invariance" => {
            let model = a["model"].as_str()?;
            let mut opts = format!(", group = {}", q(a["group"].as_str()?));
            if a["std_lv"].as_bool() == Some(true) {
                opts += ", std.lv = TRUE";
            }
            match a["estimator"].as_str() {
                Some("mlm") => opts += ", estimator = \"MLM\"",
                Some("mlr") => opts += ", estimator = \"MLR\"",
                _ => {}
            }
            if a["missing"].as_str() == Some("fiml") {
                opts += ", missing = \"ml\"";
            }
            let pa = strs(&a["group_partial"]);
            if !pa.is_empty() {
                opts += &format!(", group.partial = {}", vec_r(&pa.iter().map(|s| s.replace(' ', "")).collect::<Vec<_>>()));
            }
            o.push(format!("m <- {}  # install.packages(\"lavaan\") once", q(model)));
            let steps = { let s = strs(&a["steps"]); if s.is_empty() { vec!["configural".to_string(), "metric".into(), "scalar".into(), "strict".into()] } else { s } };
            let mut names = Vec::new();
            for st in &steps {
                let eq = match st.as_str() {
                    "metric" => "c(\"loadings\")",
                    "scalar" => "c(\"loadings\", \"intercepts\")",
                    "strict" => "c(\"loadings\", \"intercepts\", \"residuals\")",
                    _ => "character()",
                };
                o.push(format!("{st} <- lavaan::cfa(m, data = x{opts}, group.equal = {eq})"));
                names.push(st.clone());
            }
            o.push(format!("print(lavaan::lavTestLRT({}))", names.join(", ")));
            o.push(format!("print(sapply(list({}), lavaan::fitMeasures, fit.measures = c(\"chisq\", \"df\", \"cfi\", \"rmsea\", \"srmr\")))", names.iter().map(|n| format!("{n} = {n}")).collect::<Vec<_>>().join(", ")));
        }
        "survey_estimates" | "survey_regression" => {
            let ids = a["cluster"].as_str().map_or("~1".to_string(), |c| format!("~{}", f(c)));
            let strata = a["strata"].as_str().map(|s| format!(", strata = ~{}", f(s))).unwrap_or_default();
            o.push(format!("des <- survey::svydesign(id = {ids}{strata}, weights = ~{}, data = x)  # install.packages(\"survey\") once", f(a["weight"].as_str()?)));
            if cmd == "survey_estimates" {
                let fun = if a["statistic"].as_str() == Some("total") { "svytotal" } else { "svymean" };
                let vars = strs(&a["variables"]).iter().map(|v| f(v)).collect::<Vec<_>>().join(" + ");
                o.push(format!("e <- survey::{fun}(~{vars}, des, na.rm = TRUE); print(e); print(confint(e, level = {lv}))"));
            } else {
                let mut rhs: Vec<String> = strs(&a["covariates"]).iter().map(|c| f(c)).collect();
                for fa in a["factors"].as_array().into_iter().flatten() {
                    if let Some(n) = fa["name"].as_str() {
                        rhs.push(format!("factor({})", f(n)));
                    }
                }
                o.push(format!("print(summary(survey::svyglm({} ~ {}, des)))", f(a["dependent"].as_str()?), rhs.join(" + ")));
            }
        }
        "ts_acf" => {
            let lag = a["lags"].as_u64().map(|l| format!(", lag.max = {l}")).unwrap_or_default();
            o.push(format!("s <- na.omit({}); acf(s{lag}); pacf(s{lag}); print(Box.test(s, lag = {}, type = \"Ljung-Box\"))", col(a["variable"].as_str()?), a["lags"].as_u64().unwrap_or(10)));
        }
        "ts_smooth" => {
            let v = col(a["variable"].as_str()?);
            if a["method"].as_str() == Some("moving_average") {
                let w = a["window"].as_u64().unwrap_or(3);
                o.push(format!("print(stats::filter(na.omit({v}), rep(1 / {w}, {w}), sides = 2))"));
            } else {
                let al = a["alpha"].as_f64().map(|v| format!(", alpha = {v}")).unwrap_or_default();
                o.push(format!("hw <- HoltWinters(na.omit({v}), beta = FALSE, gamma = FALSE{al}); print(hw); plot(hw)"));
            }
        }
        "arima" => {
            let (p, d, q) = (a["p"].as_u64().unwrap_or(0), a["d"].as_u64().unwrap_or(0), a["q"].as_u64().unwrap_or(0));
            let mean = if a["mean"].as_bool() == Some(false) { ", include.mean = FALSE" } else { "" };
            o.push(format!("m <- arima(na.omit({}), order = c({p}, {d}, {q}), method = \"ML\"{mean}); print(m); print(Box.test(residuals(m), lag = 10, type = \"Ljung-Box\", fitdf = {}))", col(a["variable"].as_str()?), p + q));
            if let Some(h) = a["forecast"].as_u64().filter(|h| *h > 0) {
                o.push(format!("print(predict(m, n.ahead = {h}))"));
            }
        }
        "kaplan_meier" | "cox_regression" => {
            let ev = a["events"].as_array().map(|v| v.iter().filter_map(Value::as_f64).map(r_num).collect::<Vec<_>>().join(", ")).unwrap_or_default();
            let surv = format!("survival::Surv({}, {} %in% c({ev}))", f(a["time"].as_str()?), f(a["status"].as_str()?));
            if cmd == "kaplan_meier" {
                let rhs = a["group"].as_str().map_or("1".to_string(), |g| format!("factor({})", f(g)));
                o.push(format!("km <- survival::survfit({surv} ~ {rhs}, data = x, conf.int = {lv}); print(km); plot(km)  # install.packages(\"survival\") once"));
                if a["group"].is_string() {
                    o.push(format!("print(survival::survdiff({surv} ~ {rhs}, data = x))"));
                }
            } else {
                let mut rhs: Vec<String> = strs(&a["covariates"]).iter().map(|c| f(c)).collect();
                for fa in a["factors"].as_array().into_iter().flatten() {
                    if let Some(n) = fa["name"].as_str() {
                        rhs.push(format!("factor({})", f(n)));
                    }
                }
                o.push(format!("m <- survival::coxph({surv} ~ {}, data = x); print(summary(m, conf.int = {lv})); print(survival::cox.zph(m))  # proportional hazards check", rhs.join(" + ")));
            }
        }
        "roc" => {
            let st = a["state"].as_str()?;
            let pos = a["positive"].as_f64().unwrap_or(1.0);
            for v in strs(&a["variables"]) {
                o.push(format!("r <- pROC::roc(as.integer({} %in% {}), {}, levels = c(0, 1), direction = \"<\"); print(pROC::ci.auc(r, method = \"delong\", conf.level = {lv})); print(pROC::coords(r, \"best\", best.method = \"youden\")); plot(r)  # install.packages(\"pROC\") once", col(st), r_num(pos), col(&v)));
            }
        }
        "permutation" => {
            let vars = strs(&a["variables"]);
            let b = a["resamples"].as_u64().unwrap_or(10_000);
            let line = match a["test"].as_str() {
                Some("paired") => format!("dd <- na.omit(x[c({}, {})]); print(coin::symmetry_test(dd[[1]] - dd[[2]] ~ 1, distribution = coin::approximate(nresample = {b})))  # sign flips; install.packages(\"coin\") once", q(&vars[0]), q(vars.get(1).map_or("", |s| s.as_str()))),
                Some("correlation") => format!("dd <- na.omit(x[c({}, {})]); print(coin::independence_test(dd[[1]] ~ dd[[2]], distribution = coin::approximate(nresample = {b})))", q(&vars[0]), q(vars.get(1).map_or("", |s| s.as_str()))),
                _ => format!("print(coin::oneway_test({} ~ factor({}), data = x, distribution = coin::approximate(nresample = {b})))  # install.packages(\"coin\") once", f(&vars[0]), f(a["group"].as_str().unwrap_or(""))),
            };
            o.push(line);
        }
        "missing_patterns" => {
            o.push(format!("dd <- x[{}]; print(naniar::miss_var_summary(dd)); print(mice::md.pattern(dd, plot = FALSE)); print(naniar::mcar_test(dd))  # install.packages(c(\"naniar\", \"mice\")) once", vec_r(&strs(&a["variables"]))));
        }
        "mi_regression" => {
            let y = a["dependent"].as_str()?;
            let covs = strs(&a["covariates"]);
            let all: Vec<String> = std::iter::once(y.to_string()).chain(covs.iter().cloned()).chain(strs(&a["auxiliary"])).collect();
            o.push(format!("imp <- mice::mice(x[{}], m = {}, maxit = {}, method = \"norm\", seed = {}, printFlag = FALSE)  # install.packages(\"mice\") once", vec_r(&all), a["imputations"].as_u64().unwrap_or(20), a["iterations"].as_u64().unwrap_or(10), a["seed"].as_u64().unwrap_or(20_260_101) % 2_147_483_647));
            o.push(format!("print(summary(mice::pool(with(imp, lm({} ~ {}))), conf.int = TRUE, conf.level = {lv}))", f(y), covs.iter().map(|c| f(c)).collect::<Vec<_>>().join(" + ")));
        }
        "mixed_model" => {
            let y = f(a["dependent"].as_str()?);
            let mut rhs: Vec<String> = strs(&a["covariates"]).iter().map(|c| f(c)).collect();
            for fa in a["factors"].as_array().into_iter().flatten() {
                if let Some(n) = fa["name"].as_str() {
                    rhs.push(format!("factor({})", f(n)));
                }
            }
            let rhs = if rhs.is_empty() { "1".to_string() } else { rhs.join(" + ") };
            let slopes: Vec<String> = strs(&a["random_slopes"]).iter().map(|s| f(s)).collect();
            let re = if slopes.is_empty() { "1".to_string() } else { format!("1 + {}", slopes.join(" + ")) };
            let reml = if a["reml"].as_bool() == Some(false) { "FALSE" } else { "TRUE" };
            o.push(format!("m <- lmerTest::lmer({y} ~ {rhs} + ({re} | factor({})), data = x, REML = {reml}); print(summary(m)); print(confint(m, method = \"Wald\", level = {lv}))  # install.packages(\"lmerTest\") once", f(a["group"].as_str()?)));
        }
        "yuen" => {
            let g = a["grouping_variable"].as_str()?;
            let groups = a["groups"].as_array()?;
            let tr = a["trim"].as_f64().unwrap_or(0.2);
            for v in strs(&a["test_variables"]) {
                o.push(format!("dd <- x[x[[{}]] %in% c({}, {}), ]; print(WRS2::yuen({} ~ factor({}), data = dd, tr = {tr}))  # install.packages(\"WRS2\") once", q(g), r_num(num(&groups[0], 1.0)), r_num(num(&groups[1], 2.0)), f(&v), f(g)));
            }
        }
        "yuen_paired" => {
            let tr = a["trim"].as_f64().unwrap_or(0.2);
            for p in a["pairs"].as_array()? {
                let p = strs(p);
                o.push(format!("dd <- na.omit(x[c({}, {})]); print(WRS2::yuend(dd[[1]], dd[[2]], tr = {tr}))", q(&p[0]), q(&p[1])));
            }
        }
        "trimmed_anova" => {
            let tr = a["trim"].as_f64().unwrap_or(0.2);
            for y in strs(&a["dependent"]) {
                o.push(format!("print(WRS2::t1way({} ~ factor({}), data = x, tr = {tr}))", f(&y), f(a["factor"].as_str()?)));
            }
        }
        "doe_design" => {
            let k = a["factors"].as_array().map_or(0, Vec::len);
            let center = a["center"].as_u64().unwrap_or(0);
            match a["kind"].as_str().unwrap_or("fractional") {
                "full" => {
                    let levels: Vec<String> = a["factors"].as_array()?.iter().map(|f| f["levels"].as_u64().unwrap_or(2).to_string()).collect();
                    o.push(format!("library(DoE.base); d <- fac.design(nlevels = c({}), randomize = {}); print(d)", levels.join(", "), if a["randomize"].as_bool() == Some(true) { "TRUE" } else { "FALSE" }));
                }
                "ccd" => {
                    let alpha = match a["alpha"].as_str() { Some("rotatable") => "rotatable", Some("face") => "faces", _ => "orthogonal" };
                    o.push(format!("library(rsm); d <- ccd({k}, n0 = c({center}, {}), alpha = \"{alpha}\", randomize = FALSE); print(d)", a["center_axial"].as_u64().unwrap_or(center)));
                }
                "bbd" => o.push(format!("library(rsm); d <- bbd({k}, n0 = {center}, randomize = FALSE); print(d)")),
                "pb" => o.push(format!("library(FrF2); d <- pb({}, {k}, randomize = FALSE); print(d)", a["runs"].as_u64().unwrap_or(12))),
                "lattice" => o.push(format!("library(AlgDesign); d <- gen.mixture({}, {k}); print(d)  # install.packages(\"AlgDesign\") once", a["degree"].as_u64().unwrap_or(2) + 1)),
                "centroid" => o.push(format!("d <- do.call(rbind, lapply(1:{k}, function(s) t(apply(combn({k}, s), 2, function(ix) {{ x <- rep(0, {k}); x[ix] <- 1 / s; x }})))); print(d)")),
                "optimal" => {
                    let names: Vec<String> = (0..k).map(|i| ((b'A' + i as u8) as char).to_string()).collect();
                    let frml = match a["model"].as_str() {
                        Some("linear") => format!("~{}", names.join(" + ")),
                        Some("interactions") => format!("~({})^2", names.join(" + ")),
                        _ => format!("~quad({})", names.join(", ")),
                    };
                    let levels = if a["model"].as_str().is_some_and(|m| m != "quadratic") { 2 } else { 3 };
                    o.push(format!("library(AlgDesign); cand <- gen.factorial({levels}, {k}, varNames = {}); o <- optFederov({frml}, cand, nTrials = {}, criterion = \"{}\", nRepeats = 100); print(o)  # install.packages(\"AlgDesign\") once", vec_r(&names), a["runs"].as_u64().unwrap_or(0), a["criterion"].as_str().unwrap_or("D")));
                }
                "vertices" => {
                    let fs = a["factors"].as_array()?;
                    let lo: Vec<String> = fs.iter().map(|f| f["low"].as_f64().unwrap_or(0.0).to_string()).collect();
                    let hi: Vec<String> = fs.iter().map(|f| f["high"].as_f64().unwrap_or(1.0).to_string()).collect();
                    o.push("library(rcdd)  # install.packages(\"rcdd\") once; the extreme vertices of the constrained mixture region".to_string());
                    o.push(format!("lo <- c({}); hi <- c({}); q <- length(lo); h <- rbind(c(1, 1, rep(-1, q)), cbind(0, -lo, diag(q)), cbind(0, hi, -diag(q))); v <- unique(round(q2d(scdd(d2q(h))$output)[, -(1:2)], 12)); print(v)", lo.join(", "), hi.join(", ")));
                }
                _ => o.push(format!("library(FrF2); d <- FrF2({}, {k}, randomize = FALSE); print(d); print(design.info(d)$aliased)", a["runs"].as_u64().unwrap_or(8))),
            }
        }
        "doe_effects" => {
            let fs = strs(&a["factors"]);
            o.push("# Code each factor -1/+1 (low/high) first; then:".to_string());
            o.push(format!("m <- lm({} ~ ({})^{}, data = x); print(2 * coef(m)[-1]); BsMD::LenthPlot(m)  # install.packages(\"BsMD\") once", f(a["response"].as_str()?), fs.iter().map(|v| f(v)).collect::<Vec<_>>().join(" + "), a["order"].as_u64().unwrap_or(fs.len() as u64)));
        }
        "doe_mixture" => {
            let cs: Vec<String> = strs(&a["components"]).iter().map(|v| f(v)).collect();
            let mut terms = cs.clone();
            if a["model"].as_str() != Some("linear") {
                for i in 0..cs.len() {
                    for j in i + 1..cs.len() {
                        terms.push(format!("{}:{}", cs[i], cs[j]));
                    }
                }
            }
            if a["model"].as_str() == Some("special_cubic") {
                for i in 0..cs.len() {
                    for j in i + 1..cs.len() {
                        for l in j + 1..cs.len() {
                            terms.push(format!("{}:{}:{}", cs[i], cs[j], cs[l]));
                        }
                    }
                }
            }
            o.push("# Components as proportions of their total; the F test and R² relative to the mean come from the same model with an intercept and one linear term dropped.".to_string());
            o.push(format!("m <- lm({} ~ -1 + {}, data = x); print(summary(m)$coefficients)", f(a["response"].as_str()?), terms.join(" + ")));
        }
        "rsm" => {
            let fs: Vec<String> = strs(&a["factors"]).iter().map(|v| f(v)).collect();
            let block = a["block"].as_str().map_or(String::new(), |b| format!("{} + ", f(b)));
            let model = if a["order"].as_u64() == Some(1) { format!("FO({})", fs.join(", ")) } else { format!("SO({})", fs.join(", ")) };
            o.push(format!("library(rsm); r <- rsm({} ~ {block}{model}, data = x); print(summary(r))  # install.packages(\"rsm\") once", f(a["response"].as_str()?)));
        }
        "spc" => {
            let chart = a["chart"].as_str().unwrap_or("xbar_r");
            let k = a["nsigmas"].as_f64().unwrap_or(3.0);
            o.push(format!("library(qcc)  # install.packages(\"qcc\") once; qcc.options(run.length = {})", a["run_length"].as_u64().unwrap_or(7)));
            let measures = strs(&a["measures"]);
            match chart {
                "xbar_r" | "xbar_s" => {
                    if measures.len() == 1 {
                        o.push(format!("g <- qcc.groups(x[[\"{}\"]], x[[\"{}\"]])", measures[0], a["subgroup"].as_str()?));
                    } else {
                        o.push(format!("g <- as.matrix(x[, {}])", vec_r(&measures)));
                    }
                    let (second, sd) = if chart == "xbar_r" { ("R", "") } else { ("S", ", std.dev = \"UWAVE-SD\"") };
                    o.push(format!("q <- qcc(g, type = \"xbar\", nsigmas = {k}{sd}); qcc(g, type = \"{second}\", nsigmas = {k})"));
                }
                "imr" => {
                    o.push(format!("v <- na.omit(x[[\"{}\"]]); q <- qcc(v, type = \"xbar.one\", nsigmas = {k}); qcc(cbind(v[-length(v)], v[-1]), type = \"R\", nsigmas = {k})  # the moving-range chart", measures.first()?));
                }
                "cusum" | "ewma" => {
                    if let Some(sg) = a["subgroup"].as_str() {
                        o.push(format!("g <- qcc.groups(x[[\"{}\"]], x[[\"{sg}\"]])", measures.first()?));
                    } else {
                        o.push(format!("g <- na.omit(x[[\"{}\"]])", measures.first()?));
                    }
                    let known = [("center", "center"), ("std_dev", "std.dev")].iter().filter_map(|(k, r)| a[*k].as_f64().map(|v| format!(", {r} = {v}"))).collect::<String>();
                    if chart == "cusum" {
                        o.push(format!("cusum(g, decision.interval = {}, se.shift = {}, head.start = {}{known})", a["decision_interval"].as_f64().unwrap_or(5.0), a["se_shift"].as_f64().unwrap_or(1.0), a["head_start"].as_f64().unwrap_or(0.0)));
                    } else {
                        o.push(format!("ewma(g, lambda = {}, nsigmas = {k}{known})", a["lambda"].as_f64().unwrap_or(0.2)));
                    }
                }
                c => {
                    let size = a["size"].as_str().map_or(String::new(), |s| format!(", sizes = x[[\"{s}\"]]"));
                    o.push(format!("q <- qcc(x[[\"{}\"]], type = \"{c}\"{size}, nsigmas = {k})", a["count"].as_str()?));
                }
            }
            if (a["lsl"].is_number() || a["usl"].is_number()) && matches!(chart, "xbar_r" | "xbar_s" | "imr") {
                let num_or_na = |v: &Value| v.as_f64().map_or("NA".to_string(), |x| x.to_string());
                let target = a["target"].as_f64().map_or(String::new(), |t| format!(", target = {t}"));
                o.push(format!("process.capability(q, spec.limits = c({}, {}){target})", num_or_na(&a["lsl"]), num_or_na(&a["usl"])));
            }
        }
        "tree" => {
            let preds = strs(&a["predictors"]);
            let method = match a["kind"].as_str() {
                Some("regression") => "anova",
                Some("classification") => "class",
                _ => "",
            };
            o.push("library(rpart)  # ships with R".to_string());
            let xval = match a["fold_variable"].as_str() {
                Some(fv) => format!("x${fv}"),
                None => a["folds"].as_u64().unwrap_or(10).to_string(),
            };
            o.push(format!(
                "set.seed({}); fit <- rpart({} ~ {}, data = x{}, control = rpart.control(minsplit = {}, cp = {}, maxdepth = {}, xval = {xval}{}))  # random folds differ from PrismStatistics' unless a fold variable is given",
                a["seed"].as_u64().unwrap_or(20_260_101),
                f(a["dependent"].as_str()?),
                preds.iter().map(|p| f(p)).collect::<Vec<_>>().join(" + "),
                if method.is_empty() { String::new() } else { format!(", method = \"{method}\"") },
                a["minsplit"].as_u64().unwrap_or(20),
                a["cp"].as_f64().unwrap_or(0.01),
                a["maxdepth"].as_u64().unwrap_or(30),
                a["minbucket"].as_u64().map_or(String::new(), |m| format!(", minbucket = {m}")) + &a["maxsurrogate"].as_u64().filter(|m| *m != 5).map_or(String::new(), |m| format!(", maxsurrogate = {m}"))
            ));
            o.push("printcp(fit); cpt <- fit$cptable; i <- which.min(cpt[, \"xerror\"])".to_string());
            match a["prune"].as_str() {
                Some("none") => o.push("print(fit); plot(fit); text(fit)".to_string()),
                Some("min") => o.push("pf <- prune(fit, cp = cpt[i, \"CP\"]); print(pf); plot(pf); text(pf)".to_string()),
                _ => o.push("j <- which(cpt[, \"xerror\"] <= cpt[i, \"xerror\"] + cpt[i, \"xstd\"])[1]; pf <- prune(fit, cp = cpt[j, \"CP\"]); print(pf); plot(pf); text(pf)".to_string()),
            }
        }
        "penalized" => {
            let mut rhs: Vec<String> = strs(&a["covariates"]).iter().map(|c| f(c)).collect();
            for fa in a["factors"].as_array().into_iter().flatten() {
                if let Some(n) = fa["name"].as_str() {
                    rhs.push(format!("factor({})", f(n)));
                }
            }
            let binary = a["outcome"].as_str() == Some("binary");
            let alpha = match a["penalty"].as_str() {
                Some("ridge") => 0.0,
                Some("elastic") => a["alpha"].as_f64().unwrap_or(0.5),
                _ => 1.0,
            };
            o.push("library(glmnet)  # install.packages(\"glmnet\") once".to_string());
            o.push(format!("d <- na.omit(x[, c(\"{}\", {})]); X <- model.matrix(~ {}, d)[, -1, drop = FALSE]", a["dependent"].as_str()?, vec_r(&strs(&a["covariates"]).into_iter().chain(a["factors"].as_array().into_iter().flatten().filter_map(|fa| fa["name"].as_str().map(String::from))).collect::<Vec<_>>()), rhs.join(" + ")));
            let folds = match a["fold_variable"].as_str() {
                Some(fv) => format!("foldid = x[rownames(d), \"{fv}\"]"),
                None => format!("nfolds = {}", a["folds"].as_u64().unwrap_or(10)),
            };
            o.push(format!(
                "set.seed({}); cv <- cv.glmnet(X, d[[\"{}\"]], family = \"{}\", alpha = {alpha}, {folds}, keep = TRUE)  # folds are drawn differently from PrismStatistics' unless a fold variable is given",
                a["seed"].as_u64().unwrap_or(20_260_101),
                a["dependent"].as_str()?,
                if binary { "binomial" } else { "gaussian" }
            ));
            let s = if a["choice"].as_str() == Some("min") { "lambda.min" } else { "lambda.1se" };
            o.push(format!("print(cv); plot(cv); print(coef(cv, s = \"{s}\"))"));
        }
        "pgmm" => {
            let y = a["dependent"].as_str()?;
            let yl = a["y_lags"].as_u64().unwrap_or(1);
            let xl = a["x_lags"].as_u64().unwrap_or(0);
            let mut rhs = vec![format!("lag({y}, 1:{yl})")];
            for c in strs(&a["covariates"]) {
                rhs.push(if xl == 0 { c.clone() } else { format!("lag({c}, 0:{xl})") });
            }
            o.push("library(plm)  # install.packages(\"plm\") once".to_string());
            o.push(format!("p <- pdata.frame(x, index = c(\"{}\", \"{}\"))", a["individual"].as_str()?, a["time"].as_str()?));
            o.push(format!(
                "m <- pgmm({y} ~ {} | lag({y}, {}:{}), data = p, effect = \"{}\", model = \"{}\", collapse = {}); print(summary(m, robust = {}))",
                rhs.join(" + "),
                a["gmm_min"].as_u64().unwrap_or(2),
                a["gmm_max"].as_u64().unwrap_or(99),
                a["effect"].as_str().unwrap_or("twoways"),
                a["model"].as_str().unwrap_or("twosteps"),
                if a["collapse"].as_bool() == Some(true) { "TRUE" } else { "FALSE" },
                if a["robust"].as_bool() == Some(false) { "FALSE" } else { "TRUE" }
            ));
        }
        "panel" => {
            let y = f(a["dependent"].as_str()?);
            let mut rhs: Vec<String> = strs(&a["covariates"]).iter().map(|c| f(c)).collect();
            for fa in a["factors"].as_array().into_iter().flatten() {
                if let Some(n) = fa["name"].as_str() {
                    rhs.push(format!("factor({})", f(n)));
                }
            }
            let model = a["model"].as_str().unwrap_or("within");
            let effect = a["effect"].as_str().unwrap_or("individual");
            o.push("library(plm)  # install.packages(\"plm\") once".to_string());
            o.push(format!("m <- plm({y} ~ {}, data = x, index = c(\"{}\", \"{}\"), model = \"{model}\", effect = \"{effect}\")", rhs.join(" + "), a["individual"].as_str()?, a["time"].as_str()?));
            if a["robust"].as_bool() == Some(true) {
                o.push("print(summary(m, vcov = vcovHC(m, method = \"arellano\", type = \"HC0\", cluster = \"group\")))".to_string());
            } else {
                o.push("print(summary(m))".to_string());
            }
            if a["tests"].as_bool() != Some(false) {
                o.push("w <- update(m, model = \"within\", effect = \"individual\"); r <- update(m, model = \"random\", effect = \"individual\"); p <- update(m, model = \"pooling\", effect = \"individual\"); print(pFtest(w, p)); print(phtest(w, r)); print(plmtest(p, type = \"bp\"))".to_string());
            }
        }
        "gee" => {
            let y = f(a["dependent"].as_str()?);
            let mut rhs: Vec<String> = strs(&a["covariates"]).iter().map(|c| f(c)).collect();
            for fa in a["factors"].as_array().into_iter().flatten() {
                if let Some(n) = fa["name"].as_str() {
                    rhs.push(format!("factor({})", f(n)));
                }
            }
            let rhs = if rhs.is_empty() { "1".to_string() } else { rhs.join(" + ") };
            let fam = a["family"].as_str().unwrap_or("gaussian");
            let cs = a["corstr"].as_str().unwrap_or("exchangeable");
            let waves = a["wave"].as_str().map_or(String::new(), |w| format!(", waves = {}", f(w)));
            o.push("library(geepack)  # install.packages(\"geepack\") once; rows must be sorted by cluster".to_string());
            o.push(format!("m <- geeglm({y} ~ {rhs}, id = {}{waves}, data = x, family = {fam}, corstr = \"{cs}\"); print(summary(m)); print(QIC(m))", f(a["cluster"].as_str()?)));
        }
        "ordinal_regression" | "multinomial_regression" | "count_regression" => {
            let y = f(a["dependent"].as_str()?);
            let mut rhs: Vec<String> = strs(&a["covariates"]).iter().map(|c| f(c)).collect();
            for fa in a["factors"].as_array().into_iter().flatten() {
                if let Some(n) = fa["name"].as_str() {
                    rhs.push(format!("factor({})", f(n)));
                }
            }
            let rhs = if rhs.is_empty() { "1".to_string() } else { rhs.join(" + ") };
            match cmd {
                "ordinal_regression" => o.push(format!("m <- MASS::polr(factor({y}, ordered = TRUE) ~ {rhs}, data = x, Hess = TRUE); print(summary(m)); print(exp(cbind(OR = coef(m), confint.default(m))))")),
                "multinomial_regression" => o.push(format!("m <- nnet::multinom(factor({y}) ~ {rhs}, data = x); print(summary(m)); print(exp(coef(m)))  # install.packages(\"nnet\") once")),
                _ => {
                    let off = a["exposure"].as_str().map(|e| format!(" + offset(log({}))", f(e))).unwrap_or_default();
                    if a["family"].as_str() == Some("negative_binomial") {
                        o.push(format!("m <- MASS::glm.nb({y} ~ {rhs}{off}, data = x); print(summary(m)); print(exp(cbind(IRR = coef(m), confint.default(m))))"));
                    } else {
                        o.push(format!("m <- glm({y} ~ {rhs}{off}, family = poisson, data = x); print(summary(m)); print(exp(cbind(IRR = coef(m), confint.default(m))))"));
                    }
                }
            }
        }
        "mediation" => {
            let (x, y) = (f(a["x"].as_str()?), f(a["y"].as_str()?));
            let ms: Vec<String> = strs(&a["mediators"]).iter().map(|m| f(m)).collect();
            let covs: Vec<String> = strs(&a["covariates"]).iter().map(|c| f(c)).collect();
            let plus = |v: &[String]| v.iter().map(|s| format!(" + {s}")).collect::<String>();
            let mut model: Vec<String> = ms.iter().enumerate().map(|(i, m)| format!("{m} ~ a{} * {x}{}", i + 1, plus(&covs))).collect();
            model.push(format!("{y} ~ cp * {x}{}{}", ms.iter().enumerate().map(|(i, m)| format!(" + b{} * {m}", i + 1)).collect::<String>(), plus(&covs)));
            for i in 0..ms.len() {
                model.push(format!("ind{} := a{} * b{}", i + 1, i + 1, i + 1));
            }
            o.push(format!("fit <- lavaan::sem({}, data = x, se = \"bootstrap\", bootstrap = {})  # install.packages(\"lavaan\") once", q(&model.join("\n")), a["resamples"].as_u64().unwrap_or(5000)));
            o.push(format!("print(lavaan::parameterEstimates(fit, boot.ci.type = \"perc\", level = {lv}))"));
        }
        "moderation" => {
            let (x, w, y) = (f(a["x"].as_str()?), f(a["w"].as_str()?), f(a["y"].as_str()?));
            let covs: String = strs(&a["covariates"]).iter().map(|c| format!(" + {}", f(c))).collect();
            o.push(format!("m <- lm({y} ~ {x} * {w}{covs}, data = x); print(summary(m))"));
            o.push(format!("print(interactions::sim_slopes(m, pred = {x}, modx = {w}, johnson_neyman = TRUE))  # install.packages(\"interactions\") once"));
        }
        "factor" => {
            let vars = strs(&a["variables"]);
            let rot = a["rotation"].as_str().unwrap_or("oblimin");
            let nf = a["factors"].as_u64().map_or("1  # PrismStatistics chose this by parallel analysis; see psych::fa.parallel()".to_string(), |k| k.to_string());
            o.push(format!("dd <- na.omit(x[{}]); print(psych::KMO(dd)); print(psych::cortest.bartlett(cor(dd), n = nrow(dd)))  # install.packages(\"psych\") once", vec_r(&vars)));
            match a["extraction"].as_str() {
                Some("pca") => o.push(format!("print(psych::principal(dd, nfactors = {nf}, rotate = \"{rot}\"))")),
                Some(fm) => {
                    let fm = match fm { "paf" => "pa", "minres" => "minres", _ => "ml" };
                    o.push(format!("print(psych::fa(dd, nfactors = {nf}, fm = \"{fm}\", rotate = \"{rot}\"))"));
                }
                None => o.push(format!("print(psych::fa(dd, nfactors = {nf}, fm = \"ml\", rotate = \"{rot}\"))")),
            }
            o.push("psych::fa.parallel(dd)  # parallel analysis and scree plot".into());
        }
        "manova" => {
            let ys = strs(&a["dependent"]).iter().map(|v| f(v)).collect::<Vec<_>>().join(", ");
            let fac = f(a["factor"].as_str()?);
            o.push(format!("m <- manova(cbind({ys}) ~ factor({fac}), data = x)"));
            o.push("for (tt in c(\"Pillai\", \"Wilks\", \"Hotelling-Lawley\", \"Roy\")) print(summary(m, test = tt))".into());
            o.push("print(summary.aov(m))  # each outcome's ANOVA; Box's M: heplots::boxM()".into());
        }
        "rm_anova" => {
            let ms = strs(&a["measures"]);
            let rhs = match a["between"].as_str() {
                Some(bf) => format!("factor({})", f(bf)),
                None => "1".into(),
            };
            let name = a["factor_name"].as_str().unwrap_or("time");
            o.push(format!("idata <- data.frame({} = factor(seq_len({})))", f(name), ms.len()));
            o.push(format!("m <- lm(cbind({}) ~ {rhs}, data = x, contrasts = {})", ms.iter().map(|m| f(m)).collect::<Vec<_>>().join(", "), if a["between"].is_string() { format!("list(`factor({})` = \"contr.sum\")", a["between"].as_str().unwrap_or("")) } else { "NULL".into() }));
            o.push(format!("print(summary(car::Anova(m, idata = idata, idesign = ~{}, type = 3), multivariate = FALSE))  # Mauchly, GG and HF; install.packages(\"car\") once", f(name)));
            o.push("# Generalized eta squared: afex::aov_ez() on the data in long form.".into());
        }
        "anova_factorial" => {
            let y = a["dependent"].as_str()?;
            let facs = strs(&a["factors"]);
            let covs: Vec<String> = strs(&a["covariates"]).iter().map(|c| f(c)).collect();
            let names: Vec<String> = (1..=facs.len()).map(|i| format!("F{i}")).collect();
            for (n, v) in names.iter().zip(&facs) {
                o.push(format!("x${n} <- factor({})  # {n} is {v}", col(v)));
            }
            let joiner = if a["interactions"].as_bool() == Some(false) { " + " } else { " * " };
            let rhs = covs.iter().cloned().chain(std::iter::once(names.join(joiner))).collect::<Vec<_>>().join(" + ");
            o.push(format!("m <- lm({} ~ {rhs}, data = x, contrasts = list({}))", f(y), names.iter().map(|n| format!("{n} = \"contr.sum\"")).collect::<Vec<_>>().join(", ")));
            if a["ss_type"].as_str() == Some("2") {
                o.push("print(car::Anova(m, type = 2))  # install.packages(\"car\") once".into());
            } else {
                o.push("print(drop1(m, . ~ ., test = \"F\"))  # Type III: each effect dropped from the full model with sum contrasts".into());
            }
            if a["emmeans"].as_bool() != Some(false) || a["post_hoc"].as_str().is_some_and(|p| p != "none") {
                let adj = a["post_hoc"].as_str().filter(|p| *p != "none").unwrap_or("tukey");
                for n in &names {
                    o.push(format!("e <- emmeans::emmeans(m, ~ {n}); print(e); print(pairs(e, adjust = \"{adj}\"), infer = TRUE)  # install.packages(\"emmeans\") once"));
                }
            }
            if a["simple_effects"].as_bool() == Some(true) && names.len() == 2 {
                o.push("print(emmeans::joint_tests(m, by = \"F2\")); print(emmeans::joint_tests(m, by = \"F1\"))  # simple effects".into());
            }
            if a["plots"].as_bool() == Some(true) && names.len() >= 2 {
                o.push("emmeans::emmip(m, F2 ~ F1)  # profile plot".into());
            }
        }
        "reliability" => {
            let items = strs(&a["items"]);
            o.push(format!("items <- x[complete.cases(x[{v}]), {v}]", v = vec_r(&items)));
            for r in strs(&a["reverse"]) {
                let lo = a["scale_min"].as_f64().map(r_num).unwrap_or_else(|| "min(as.matrix(items))".into());
                let hi = a["scale_max"].as_f64().map(r_num).unwrap_or_else(|| "max(as.matrix(items))".into());
                o.push(format!("items[[{}]] <- {lo} + {hi} - items[[{}]]  # reverse-scored", q(&r), q(&r)));
            }
            o.push("k <- ncol(items); alpha <- k / (k - 1) * (1 - sum(apply(items, 2, var)) / var(rowSums(items))); n <- nrow(items)".into());
            o.push(format!("print(c(alpha = alpha, lower = 1 - (1 - alpha) * qf({}, n - 1, (n - 1) * (k - 1)), upper = 1 - (1 - alpha) * qf({}, n - 1, (n - 1) * (k - 1))))  # Feldt's interval", 1.0 - (1.0 - lv) / 2.0, (1.0 - lv) / 2.0));
        }
        "item_analysis" => {
            let items = strs(&a["items"]);
            let key = a["key"].as_array().cloned().unwrap_or_default();
            let pts: Vec<String> = (0..items.len()).map(|i| r_num(key.get(i).and_then(|k| k["points"].as_f64()).unwrap_or(1.0))).collect();
            if a["scored"].as_bool() == Some(true) {
                o.push(format!("X <- x[complete.cases(x[{v}]), {v}]", v = vec_r(&items)));
                if key.is_empty() {
                    o.push("possible <- apply(X, 2, max)".into());
                } else {
                    o.push(format!("possible <- c({})", pts.join(", ")));
                }
            } else {
                let keys: Vec<String> = key
                    .iter()
                    .map(|k| match k["adjustment"].as_str() {
                        Some("dropped") => "NULL".to_string(),
                        Some("credit_all") => "TRUE".to_string(),
                        _ => format!("c({})", strs(&k["correct"]).iter().map(|c| q(c)).collect::<Vec<_>>().join(", ")),
                    })
                    .collect();
                o.push(format!("keys <- list({})  # NULL: dropped; TRUE: credited to everyone", keys.join(", ")));
                o.push(format!("possible <- c({})", pts.join(", ")));
                o.push(format!("R <- as.data.frame(lapply(x[{}], function(v) {{ v <- toupper(gsub(\"[[:space:],;+&]\", \"\", as.character(v))); v[is.na(v)] <- \"\"; sapply(strsplit(v, \"\"), function(l) if (all(grepl(\"^[A-Z]$\", l))) paste(sort(unique(l)), collapse = \"\") else paste(l, collapse = \"\")) }}))", vec_r(&items)));
                o.push("R <- R[rowSums(R != \"\") > 0, , drop = FALSE]  # a sheet left entirely blank is left out".into());
                o.push("live <- !sapply(keys, is.null); X <- sapply(which(live), function(j) if (isTRUE(keys[[j]])) rep(possible[j], nrow(R)) else possible[j] * (R[[j]] %in% keys[[j]])); possible <- possible[live]".into());
            }
            o.push("tot <- rowSums(X); n <- nrow(X); k <- ncol(X); varn <- function(v) mean((v - mean(v))^2)".into());
            o.push("print(c(n = n, mean = mean(tot), sd = sd(tot), KR20 = k / (k - 1) * (1 - sum(apply(X, 2, varn)) / varn(tot))))  # KR-20 (alpha with partial credit)".into());
            o.push(format!("g <- ({} * n + 50) %/% 100; ord <- order(-tot, seq_len(n)); up <- ord[1:g]; lo <- ord[(n - g + 1):n]", ((a["group_fraction"].as_f64().unwrap_or(0.27)) * 100.0).round()));
            o.push("print(round(data.frame(p = colMeans(sweep(X, 2, possible, \"/\")), r_pb = apply(X, 2, cor, tot), r_corrected = sapply(seq_len(k), function(j) cor(X[, j], tot - X[, j])), D = colMeans(sweep(X[up, , drop = FALSE], 2, possible, \"/\")) - colMeans(sweep(X[lo, , drop = FALSE], 2, possible, \"/\"))), 3))".into());
        }
        "canonical" => {
            let (s1, s2) = (strs(&a["set1"]), strs(&a["set2"]));
            let all: Vec<String> = s1.iter().chain(&s2).cloned().collect();
            o.push(format!("cc <- x[complete.cases(x[{}]), ]; print(cancor(as.matrix(cc[{}]), as.matrix(cc[{}]))$cor)", vec_r(&all), vec_r(&s1), vec_r(&s2)));
        }
        "chisq_gof" => {
            let v = a["variable"].as_str()?;
            match a["expected"].as_array() {
                Some(e) => o.push(format!("print(chisq.test(table({}), p = c({}), rescale.p = TRUE))", col(v), e.iter().map(|x| r_num(num(x, 1.0))).collect::<Vec<_>>().join(", "))),
                None => o.push(format!("print(chisq.test(table({})))", col(v))),
            }
        }
        "binomial" => {
            for v in strs(&a["variables"]) {
                o.push(format!("tb <- table({}); print(binom.test(tb[1], sum(tb), p = {}, conf.level = {lv}))  # the first category; see the result for the one PrismStatistics counted", col(&v), num(&a["test_proportion"], 0.5)));
            }
        }
        "runs" => {
            for v in strs(&a["variables"]) {
                o.push(format!("v <- na.omit({}); up <- v >= median(v); print(c(runs = 1 + sum(up[-1] != up[-length(up)]), below = sum(!up), above = sum(up)))  # runs test (exact p: DescTools::RunsTest)", col(&v)));
            }
        }
        "sign_test" => {
            for p in a["pairs"].as_array()? {
                let p = strs(p);
                o.push(format!("dd <- {} - {}; dd <- dd[!is.na(dd) & dd != 0]; print(binom.test(sum(dd > 0), length(dd)))  # sign test", col(&p[0]), col(&p[1])));
            }
        }
        "median_test" => {
            let fac = a["factor"].as_str()?;
            for y in strs(&a["dependent"]) {
                o.push(format!("mm <- x[!is.na({c}) & !is.na({g}), ]; md <- median(mm[[{}]]); print(chisq.test(table(mm[[{}]] > md, mm[[{}]]), correct = FALSE))  # median test", q(&y), q(&y), q(fac), c = col(&y), g = col(fac)));
            }
        }
        "ks2" => {
            let g = a["grouping_variable"].as_str()?;
            let groups = a["groups"].as_array()?;
            for v in strs(&a["test_variables"]) {
                o.push(format!("print(ks.test({c}[{gc} %in% {}], {c}[{gc} %in% {}]))", r_num(num(&groups[0], 1.0)), r_num(num(&groups[1], 2.0)), c = col(&v), gc = col(g)));
            }
        }
        "explore" => {
            for y in strs(&a["dependent"]) {
                let by = a["factor"].as_str().map(|f| format!(", {}", col(f))).unwrap_or_default();
                o.push(format!("print(tapply({c}, list(rep(1, nrow(x)){by}), function(v) {{ v <- na.omit(v); c(n = length(v), mean = mean(v), trimmed = mean(v, trim = 0.05), sd = sd(v), skew = DescTools::Skew(v, method = 2), kurt = DescTools::Kurt(v, method = 2), shapiro_p = shapiro.test(v)$p.value, lilliefors_p = DescTools::LillieTest(v)$p.value) }}))", c = col(&y)));
            }
        }
        "bootstrap" => {
            let vars = strs(&a["variables"]);
            let reps = a["resamples"].as_u64().unwrap_or(2000);
            o.push(format!("set.seed({})  # R's random numbers differ from PrismStatistics', so intervals agree only roughly", a["seed"].as_u64().unwrap_or(20_260_101)));
            let stat = a["statistic"].as_str().unwrap_or("mean");
            let body = match stat {
                "median" => "median(d[i])".to_string(),
                "trimmed_mean" => "mean(d[i], trim = 0.05)".to_string(),
                "sd" => "sd(d[i])".to_string(),
                _ => "mean(d[i])".to_string(),
            };
            match stat {
                "correlation" => {
                    let m = if a["coefficient"].as_str() == Some("spearman") { "spearman" } else { "pearson" };
                    o.push(format!("dd <- na.omit(x[{}]); b <- boot::boot(dd, function(d, i) {{ r <- cor(d[i, ], method = \"{m}\"); r[upper.tri(r)] }}, R = {reps})  # install.packages(\"boot\") once", vec_r(&vars)));
                    o.push(format!("for (k in seq_along(b$t0)) print(boot::boot.ci(b, conf = {lv}, type = c(\"perc\", \"bca\"), index = k))"));
                }
                "regression" => {
                    let y = f(a["dependent"].as_str()?);
                    let rhs = vars.iter().map(|v| f(v)).collect::<Vec<_>>().join(" + ");
                    o.push(format!("dd <- na.omit(x[c({}, {})]); b <- boot::boot(dd, function(d, i) coef(lm({y} ~ {rhs}, data = d[i, ])), R = {reps})  # install.packages(\"boot\") once", q(a["dependent"].as_str()?), vars.iter().map(|v| q(v)).collect::<Vec<_>>().join(", ")));
                    o.push(format!("for (k in seq_along(b$t0)) print(boot::boot.ci(b, conf = {lv}, type = c(\"perc\", \"bca\"), index = k))"));
                }
                _ => {
                    for v in &vars {
                        o.push(format!("b <- boot::boot(na.omit({}), function(d, i) {body}, R = {reps}); print(b); print(boot::boot.ci(b, conf = {lv}, type = c(\"perc\", \"bca\")))  # install.packages(\"boot\") once", col(v)));
                    }
                }
            }
        }
        "curve" => {
            let xv = f(a["independent"].as_str()?);
            let kinds = strs(&a["models"]);
            let kinds = if kinds.is_empty() { vec!["linear".to_string()] } else { kinds };
            for y in strs(&a["dependent"]) {
                let yv = f(&y);
                for k in &kinds {
                    let formula = match k.as_str() {
                        "logarithmic" => format!("{yv} ~ log({xv})"),
                        "inverse" => format!("{yv} ~ I(1 / {xv})"),
                        "quadratic" => format!("{yv} ~ {xv} + I({xv}^2)"),
                        "cubic" => format!("{yv} ~ {xv} + I({xv}^2) + I({xv}^3)"),
                        "power" => format!("log({yv}) ~ log({xv})"),
                        "exponential" => format!("log({yv}) ~ {xv}"),
                        "s_curve" => format!("log({yv}) ~ I(1 / {xv})"),
                        _ => format!("{yv} ~ {xv}"),
                    };
                    o.push(format!("print(summary(lm({formula}, data = x)))  # {k}"));
                }
            }
        }
        "cochran" => {
            o.push(format!("print(DescTools::CochranQTest(as.matrix(na.omit(x[{}])) == max(as.matrix(na.omit(x[{}])))))  # install.packages(\"DescTools\") once", vec_r(&strs(&a["variables"])), vec_r(&strs(&a["variables"]))));
        }
        "kappa" => {
            let w = match a["weights"].as_str() { Some("linear") => "equal", Some("quadratic") => "squared", _ => "unweighted" };
            o.push(format!("print(irr::kappa2(na.omit(x[{}]), weight = \"{w}\"))  # install.packages(\"irr\") once", vec_r(&[a["rater1"].as_str()?.to_string(), a["rater2"].as_str()?.to_string()])));
        }
        "fleiss" => o.push(format!("print(irr::kappam.fleiss(na.omit(x[{}])))", vec_r(&strs(&a["raters"])))),
        "kripp" => o.push(format!("print(irr::kripp.alpha(t(as.matrix(x[{}])), method = {}))", vec_r(&strs(&a["raters"])), q(a["metric"].as_str().unwrap_or("nominal")))),
        "icc" => {
            let model = if a["model"].as_str() == Some("one_way") { "oneway" } else { "twoway" };
            o.push(format!("print(irr::icc(na.omit(x[{}]), model = \"{model}\", type = {}, unit = {}, conf.level = {lv}))", vec_r(&strs(&a["raters"])), q(a["kind"].as_str().unwrap_or("agreement")), q(a["unit"].as_str().unwrap_or("single"))));
        }
        "chart" => {
            let vars = strs(&a["variables"]);
            match a["kind"].as_str()? {
                "histogram" => {
                    for v in &vars {
                        o.push(format!("v <- na.omit({}); h <- hist(v, main = \"\", xlab = {}, col = \"grey75\")", col(v), q(v)));
                        if a["normal"].as_bool() == Some(true) {
                            o.push("curve(dnorm(x, mean(v), sd(v)) * length(v) * diff(h$breaks)[1], add = TRUE)".into());
                        }
                    }
                }
                "boxplot" => match a["group"].as_str() {
                    Some(g) => {
                        for v in &vars {
                            o.push(format!("boxplot({} ~ factor({}), data = x, col = \"grey75\", xlab = {}, ylab = {})", f(v), f(g), q(g), q(v)));
                        }
                    }
                    None => o.push(format!("boxplot(x[{}], col = \"grey75\")", vec_r(&vars))),
                },
                "bar" => {
                    let v = vars.first()?;
                    match (a["bar_stat"].as_str(), a["mean_of"].as_str()) {
                        (Some("mean"), Some(m)) => o.push(format!("means <- tapply({}, {}, mean, na.rm = TRUE); barplot(means, col = \"grey75\", xlab = {}, ylab = {})  # error bars: see the means table", col(m), col(v), q(v), q(&format!("Mean {m}")))),
                        (Some("percent"), _) => o.push(format!("barplot(100 * prop.table(table({}, useNA = \"no\")), col = \"grey75\", xlab = {}, ylab = \"Percent\")", col(v), q(v))),
                        _ => o.push(format!("barplot(table({}, useNA = \"no\"), col = \"grey75\", xlab = {}, ylab = \"Frequency\")", col(v), q(v))),
                    }
                }
                "scatter" => {
                    let (xv, yv) = (a["x"].as_str()?, a["y"].as_str()?);
                    o.push(format!("plot({}, {}, xlab = {}, ylab = {})", col(xv), col(yv), q(xv), q(yv)));
                    if a["line"].as_bool() == Some(true) {
                        o.push(format!("abline(lm({} ~ {}, data = x))", f(yv), f(xv)));
                    }
                }
                "qq" => {
                    for v in &vars {
                        o.push(format!("qqnorm({c}, main = \"\", ylab = {}); qqline({c})", q(v), c = col(v)));
                    }
                }
                _ => return None,
            }
        }
        "power" => {
            let alpha = num(&a["alpha"], 0.05);
            let solve_n = a["solve_for"].as_str() != Some("power");
            let target = if solve_n { format!("power = {}", num(&a["power"], 0.8)) } else { format!("n = {}", a["n"].as_u64().unwrap_or(0)) };
            let es = a["effect_size"].as_f64();
            match a["test"].as_str()? {
                t @ ("one_sample" | "paired" | "independent") => {
                    let (delta, sd) = match es { Some(d) => (d, 1.0), None => (num(&a["mean_difference"], 0.0), num(&a["sd"], 1.0)) };
                    let ty = match t { "one_sample" => "one.sample", "paired" => "paired", _ => "two.sample" };
                    o.push(format!("print(power.t.test(delta = {delta}, sd = {sd}, sig.level = {alpha}, {target}, type = \"{ty}\"))"));
                }
                "anova" => {
                    if let Some(means) = a["means"].as_array() {
                        let m: Vec<String> = means.iter().map(|v| r_num(num(v, 0.0))).collect();
                        o.push(format!("means <- c({}); print(power.anova.test(groups = length(means), between.var = var(means), within.var = {}^2, sig.level = {alpha}, {target}))", m.join(", "), num(&a["sd"], 1.0)));
                    } else {
                        let k = a["groups"].as_u64().unwrap_or(2);
                        o.push(format!("k <- {k}; f <- {}; print(power.anova.test(groups = k, between.var = f^2 * k / (k - 1), within.var = 1, sig.level = {alpha}, {target}))  # between.var is the variance of the means", es.unwrap_or(0.0)));
                    }
                }
                "correlation" => o.push(format!("print(pwr::pwr.r.test(r = {}, sig.level = {alpha}, {target}))  # install.packages(\"pwr\") once", es.unwrap_or(0.0))),
                t @ ("one_proportion" | "two_proportions") => {
                    let h = match es { Some(h) => r_num(h), None => format!("pwr::ES.h({}, {})", num(&a["p1"], 0.5), num(&a["p2"], 0.5)) };
                    let fun = if t == "one_proportion" { "pwr.p.test" } else { "pwr.2p.test" };
                    o.push(format!("print(pwr::{fun}(h = {h}, sig.level = {alpha}, {target}))  # install.packages(\"pwr\") once"));
                }
                _ => return None,
            }
        }
        _ => return None,
    }
    if a["bayes"].as_bool() == Some(true) {
        let rs = a["prior_scale"].as_f64().map(|v| format!(", rscale = {v}")).unwrap_or_default();
        o.push("# Bayes factor (install.packages(\"BayesFactor\") once):".into());
        match cmd {
            "ttest_one_sample" => {
                for v in strs(&a["test_variables"]) {
                    o.push(format!("print(BayesFactor::ttestBF(na.omit({}), mu = {}{rs}))", col(&v), r_num(num(&a["test_value"], 0.0))));
                }
            }
            "ttest_paired" => {
                for pr in a["pairs"].as_array().into_iter().flatten() {
                    let pr = strs(pr);
                    o.push(format!("dd <- na.omit(x[c({}, {})]); print(BayesFactor::ttestBF(dd[[1]], dd[[2]], paired = TRUE{rs}))", q(&pr[0]), q(&pr[1])));
                }
            }
            "ttest_independent" => {
                let g = a["grouping_variable"].as_str().unwrap_or("");
                let groups = a["groups"].as_array().cloned().unwrap_or_default();
                for v in strs(&a["test_variables"]) {
                    o.push(format!("print(BayesFactor::ttestBF(na.omit({c}[{gc} %in% {}]), na.omit({c}[{gc} %in% {}]){rs}))", r_num(groups.first().map_or(1.0, |v| num(v, 1.0))), r_num(groups.get(1).map_or(2.0, |v| num(v, 2.0))), c = col(&v), gc = col(g)));
                }
            }
            "correlation" => {
                let vars = strs(&a["variables"]);
                for i in 0..vars.len() {
                    for j in (i + 1)..vars.len() {
                        o.push(format!("dd <- na.omit(x[c({}, {})]); print(BayesFactor::correlationBF(dd[[1]], dd[[2]]{rs}))", q(&vars[i]), q(&vars[j])));
                    }
                }
            }
            "crosstabs" => {
                let st = match a["sampling"].as_str() {
                    Some("rows") => "sampleType = \"indepMulti\", fixedMargin = \"rows\"",
                    Some("columns") => "sampleType = \"indepMulti\", fixedMargin = \"cols\"",
                    Some("poisson") => "sampleType = \"poisson\"",
                    _ => "sampleType = \"jointMulti\"",
                };
                o.push(format!("print(BayesFactor::contingencyTableBF(tab, {st}))"));
            }
            "anova_oneway" => {
                for y in strs(&a["dependent"]) {
                    o.push(format!("dd <- na.omit(x[c({}, {})]); dd[[2]] <- factor(dd[[2]]); print(BayesFactor::anovaBF({} ~ {}, data = dd{}))", q(&y), q(a["factor"].as_str().unwrap_or("")), f(&y), f(a["factor"].as_str().unwrap_or("")), a["prior_scale"].as_f64().map(|v| format!(", rscaleFixed = {v}")).unwrap_or_default()));
                }
            }
            "regression" => {
                let rhs = strs(&a["covariates"]).iter().map(|c| f(c)).collect::<Vec<_>>().join(" + ");
                o.push(format!("print(BayesFactor::lmBF({} ~ {rhs}, data = na.omit(x[c({}, {})]){}))", f(a["dependent"].as_str().unwrap_or("")), q(a["dependent"].as_str().unwrap_or("")), strs(&a["covariates"]).iter().map(|c| q(c)).collect::<Vec<_>>().join(", "), a["prior_scale"].as_f64().map(|v| format!(", rscaleCont = {v}")).unwrap_or_default()));
            }
            _ => {}
        }
    }
    Some(o)
}

/// The whole R script for a project.
pub fn script(p: &Project) -> String {
    let mut out = vec![
        "# R script exported by PrismStatistics. It repeats this project's analyses in R, in the order they were run.".to_string(),
        "# Base R throughout, with haven, readxl or pwr where a line says so. User-missing codes use the variable settings".to_string(),
        "# as they are now, so a result marked out of date in PrismStatistics may differ here until it is run again.".to_string(),
        String::new(),
    ];
    out.extend(read_data(p));
    out.push(String::new());
    out.extend(prep(&p.dataset.variables));
    let mut n = 0;
    let mut dates_helper = false;
    let mut row_helpers = false;
    for op in &p.operations {
        if !plain(&op.args) {
            out.push(String::new());
            out.push(format!("# {} is left out: its settings hold characters that can't be written safely into R code.", op.command));
            continue;
        }
        match op.command.as_str() {
            "import" | "update_variable" | "clear_results" => {}
            "compute_variable" => {
                let a = &op.args;
                out.push(String::new());
                let target = a["target"].as_str().unwrap_or("");
                match crate::rexpr::to_r(a["formula"].as_str().unwrap_or("")) {
                    Ok((expr, rows)) => {
                        if rows && !row_helpers {
                            row_helpers = true;
                            out.extend(crate::rexpr::ROW_HELPERS.iter().map(|s| s.to_string()));
                        }
                        out.push(format!("d[[{}]] <- with(prep(d), {{ v <- {expr}; if (is.logical(v)) as.numeric(v) else v }})  # Compute Variable", q(target)));
                    }
                    Err(why) => out.push(format!("# Compute Variable {target} is left out: its formula can't be read ({why}).")),
                }
            }
            "sort_cases" => {
                let keys: Vec<&Value> = op.args["keys"].as_array().map(|k| k.iter().collect()).unwrap_or_default();
                let by: Vec<String> = keys.iter().map(|k| format!("p[[{}]]", q(k["name"].as_str().unwrap_or("")))).collect();
                let dec: Vec<&str> = keys.iter().map(|k| if k["descending"].as_bool().unwrap_or(false) { "TRUE" } else { "FALSE" }).collect();
                out.push(String::new());
                out.push(format!("p <- prep(d); d <- d[order({}, decreasing = c({}), method = \"radix\", na.last = TRUE), , drop = FALSE]; rownames(d) <- NULL  # Sort Cases: missing values last", by.join(", "), dec.join(", ")));
            }
            "rank_cases" => {
                let a = &op.args;
                let vars = strs(&a["variables"]);
                let into = strs(&a["into"]);
                let ties = match a["ties"].as_str() {
                    Some("low") => "min",
                    Some("high") => "max",
                    Some("sequential") => "first",
                    _ => "average",
                };
                let sign = if a["descending"].as_bool().unwrap_or(false) { "-" } else { "" };
                out.push(String::new());
                for (k, v) in vars.iter().enumerate() {
                    let t = into.get(k).cloned().unwrap_or_else(|| format!("r_{v}"));
                    match a["by"].as_str() {
                        Some(g) => out.push(format!("d[[{}]] <- local({{ x <- {sign}prep(d)[[{}]]; g <- prep(d)[[{}]]; r <- rep(NA_real_, length(x)); for (k in unique(na.omit(g))) {{ i <- which(g %in% k); r[i] <- rank(x[i], ties.method = \"{ties}\", na.last = \"keep\") }}; r }})  # Rank Cases within groups", q(&t), q(v), q(g))),
                        None => out.push(format!("d[[{}]] <- rank({sign}prep(d)[[{}]], ties.method = \"{ties}\", na.last = \"keep\")  # Rank Cases", q(&t), q(v))),
                    }
                }
            }
            "standardize" => {
                let a = &op.args;
                let into = strs(&a["into"]);
                out.push(String::new());
                for (k, v) in strs(&a["variables"]).iter().enumerate() {
                    let t = into.get(k).cloned().unwrap_or_else(|| format!("z_{v}"));
                    out.push(format!("d[[{}]] <- as.numeric(scale(prep(d)[[{}]]))  # z-scores, sample SD", q(&t), q(v)));
                }
            }
            "reverse_score" => {
                let a = &op.args;
                let into = strs(&a["into"]);
                let replace = a["replace"].as_bool().unwrap_or(false);
                let sum = a["min"].as_f64().unwrap_or(0.0) + a["max"].as_f64().unwrap_or(0.0);
                out.push(String::new());
                for (k, v) in strs(&a["variables"]).iter().enumerate() {
                    let t = if replace { v.clone() } else { into.get(k).cloned().unwrap_or_else(|| format!("{v}_r")) };
                    out.push(format!("d[[{}]] <- ifelse(is.na(prep(d)[[{}]]), d[[{}]], {} - d[[{}]])  # Reverse Score; missing codes kept", q(&t), q(v), q(v), r_num(sum), q(v)));
                }
            }
            "date_parts" | "date_difference" => {
                let a = &op.args;
                if !dates_helper {
                    dates_helper = true;
                    out.push(String::new());
                    out.push("# Dates as days since 1970-01-01, read as PrismStatistics reads them.".to_string());
                    out.push("pdate <- function(x, src) {".to_string());
                    out.push("  if (src %in% c(\"text_ymd\", \"text_mdy\", \"text_dmy\")) {".to_string());
                    out.push("    o <- switch(src, text_ymd = c(1, 2, 3), text_mdy = c(3, 1, 2), text_dmy = c(3, 2, 1))".to_string());
                    out.push("    vapply(strsplit(trimws(as.character(x)), \"[^0-9]+\"), function(a) {".to_string());
                    out.push("      a <- as.numeric(a[a != \"\"]); if (length(a) < 3) return(NA_real_); a <- c(a, 0, 0, 0)".to_string());
                    out.push("      t <- as.POSIXct(sprintf(\"%04d-%02d-%02d %02d:%02d:%02d\", a[o[1]], a[o[2]], a[o[3]], a[4], a[5], a[6]), format = \"%Y-%m-%d %H:%M:%S\", tz = \"UTC\")".to_string());
                    out.push("      as.numeric(t) / 86400".to_string());
                    out.push("    }, 0)".to_string());
                    out.push("  } else switch(src, spreadsheet = x - 25569, seconds1582 = x / 86400 - 141428, days1960 = x - 3653, seconds1960 = x / 86400 - 3653)".to_string());
                    out.push("}".to_string());
                    out.push("lt <- function(days) as.POSIXlt(round(days * 86400), origin = \"1970-01-01\", tz = \"UTC\")".to_string());
                }
                let src = a["source"].as_str().unwrap_or("text_ymd");
                out.push(String::new());
                if op.command == "date_parts" {
                    let v = a["variable"].as_str().unwrap_or("");
                    out.push(format!("dd <- pdate(prep(d)[[{}]], \"{src}\"); t <- lt(dd)  # Date and Time: parts of {v}", q(v)));
                    for part in strs(&a["parts"]) {
                        let (key, expr) = match part.as_str() {
                            "year" => ("year", "t$year + 1900"),
                            "month" => ("month", "t$mon + 1"),
                            "day" => ("day", "t$mday"),
                            "weekday" => ("weekday", "(t$wday + 6) %% 7 + 1"),
                            "day_of_year" => ("yday", "t$yday + 1"),
                            "hour" => ("hour", "t$hour"),
                            "minute" => ("minute", "t$min"),
                            _ => ("days", "dd"),
                        };
                        out.push(format!("d[[{}]] <- {expr}", q(&format!("{v}_{key}"))));
                    }
                } else {
                    let (s0, e0, target) = (a["start"].as_str().unwrap_or(""), a["end"].as_str().unwrap_or(""), a["target"].as_str().unwrap_or("days"));
                    out.push(format!("ds <- pdate(prep(d)[[{}]], \"{src}\"); de <- pdate(prep(d)[[{}]], \"{src}\"); s <- lt(floor(ds)); e <- lt(floor(de))  # Date and Time: {s0} to {e0}", q(s0), q(e0)));
                    out.push("m <- (e$year - s$year) * 12 + (e$mon - s$mon); m <- ifelse(de >= ds, m - (e$mday < s$mday), m + (e$mday > s$mday))".to_string());
                    let value = match a["unit"].as_str() {
                        Some("weeks") => "(de - ds) / 7",
                        Some("months") => "m",
                        Some("years") => "trunc(m / 12)",
                        _ => "de - ds",
                    };
                    out.push(format!("d[[{}]] <- {value}", q(target)));
                }
            }
            "scale_score" => {
                let a = &op.args;
                let items = strs(&a["items"]);
                let k = items.len();
                out.push(String::new());
                out.push(format!("it <- prep(d)[{}]", vec_r(&items)));
                for r in strs(&a["reverse"]) {
                    out.push(format!("it[[{}]] <- {} - it[[{}]]  # reverse-scored", q(&r), r_num(a["scale_min"].as_f64().unwrap_or(0.0) + a["scale_max"].as_f64().unwrap_or(0.0)), q(&r)));
                }
                let need = a["min_items"].as_u64().map_or(k, |m| m as usize);
                let value = match a["method"].as_str() {
                    Some("sum") => "rowSums(it, na.rm = TRUE)".to_string(),
                    Some("prorated_sum") => format!("rowMeans(it, na.rm = TRUE) * {k}"),
                    _ => "rowMeans(it, na.rm = TRUE)".to_string(),
                };
                out.push(format!("d[[{}]] <- ifelse(rowSums(!is.na(it)) >= {need}, {value}, NA)", q(a["target"].as_str().unwrap_or(""))));
            }
            "recode" => {
                let a = &op.args;
                let vars = strs(&a["variables"]);
                let into = strs(&a["into"]);
                out.push(String::new());
                for (k, v) in vars.iter().enumerate() {
                    let target = into.get(k).cloned().unwrap_or_else(|| v.clone());
                    out.push(format!("old <- d[[{}]]; miss <- is.na(prep(d)[[{}]]); new <- {}; done <- rep(FALSE, length(old))  # recode, first rule wins", q(v), q(v), if into.is_empty() { "old" } else { "rep(NA_real_, length(old))" }));
                    for rule in a["rules"].as_array().into_iter().flatten() {
                        let from = &rule["from"];
                        let hit = match from["kind"].as_str() {
                            Some("value") => format!("old %in% {}", r_num(from["value"].as_f64().unwrap_or(f64::NAN))),
                            Some("range") => {
                                let lo = from["lo"].as_f64().map(|l| format!(" & old >= {}", r_num(l))).unwrap_or_default();
                                let hi = from["hi"].as_f64().map(|h| format!(" & old <= {}", r_num(h))).unwrap_or_default();
                                format!("!miss{lo}{hi}")
                            }
                            Some("missing") => "miss".into(),
                            _ => "TRUE".into(),
                        };
                        let to = &rule["to"];
                        let value = match to["kind"].as_str() {
                            Some("value") => r_num(to["value"].as_f64().unwrap_or(f64::NAN)),
                            Some("copy") => "old[hit]".into(),
                            _ => "NA".into(),
                        };
                        out.push(format!("hit <- !done & ({hit}); new[hit] <- {value}; done <- done | hit"));
                    }
                    out.push(format!("d[[{}]] <- new", q(&target)));
                }
            }
            "set_cell" => {
                let (row, name) = (op.args["row"].as_u64().unwrap_or(0) + 1, op.args["name"].as_str().unwrap_or(""));
                let value = match &op.args["value"] {
                    Value::String(s) if s.parse::<f64>().is_ok() => s.clone(),
                    Value::String(s) => q(s),
                    _ => "NA".into(),
                };
                out.push(String::new());
                out.push(format!("d[{row}, {}] <- {value}  # a cell edited in Data View", q(name)));
            }
            "health_fix" => {
                let fx = &op.args["fix"];
                let name = fx["name"].as_str().unwrap_or("");
                out.push(String::new());
                match fx["kind"].as_str() {
                    Some("to_numeric") => out.push(format!("d[[{}]] <- suppressWarnings(as.numeric(d[[{}]]))  # converted to numbers (Data Health); text that isn't a number becomes NA", q(name), q(name))),
                    Some("mark_missing") => out.push(format!("# Data Health marked missing codes of {name}; prep() treats them as missing.")),
                    _ => out.push(format!("# Data Health changed the measure of {name} (no effect on the data).")),
                }
            }
            "paste_cells" => {
                let a = &op.args;
                out.push(String::new());
                let added = a["new_cases"].as_u64().unwrap_or(0);
                if added > 0 {
                    out.push(format!("d[nrow(d) + seq_len({added}), ] <- NA  # cases added by a paste"));
                }
                let cells = a["cells"].as_array().cloned().unwrap_or_default();
                let lines: Vec<String> = cells
                    .iter()
                    .map(|c| {
                        let value = match &c[2] {
                            Value::String(s) if s.parse::<f64>().is_ok() => s.clone(),
                            Value::String(s) => q(s),
                            _ => "NA".into(),
                        };
                        format!("d[{}, {}] <- {value}", c[0].as_u64().unwrap_or(0) + 1, q(c[1].as_str().unwrap_or("")))
                    })
                    .collect();
                out.push(format!("# {} cells pasted in Data View", lines.len()));
                out.extend(lines);
            }
            "append_cases" | "merge_variables" => {
                let a = &op.args;
                let name = a["file_name"].as_str().unwrap_or("other.csv");
                out.push(String::new());
                out.push(format!("# The other file: {name} (in R's working directory, or give its full path)."));
                out.push(read_other(name));
                if op.command == "append_cases" {
                    out.push("for (v in setdiff(names(d2), names(d))) d[[v]] <- NA; for (v in setdiff(names(d), names(d2))) d2[[v]] <- NA".to_string());
                    if let Some(src) = a["source"].as_str().filter(|s| !s.trim().is_empty()) {
                        out.push(format!("d[[{}]] <- 1; d2[[{}]] <- 2", q(src), q(src)));
                    }
                    out.push("d <- rbind(d, d2[names(d)])  # cases appended".to_string());
                } else {
                    let keys = strs(&a["keys"]);
                    let (x, y) = match a["join"].as_str() {
                        Some("inner") => ("FALSE", "FALSE"),
                        Some("all") => ("TRUE", "TRUE"),
                        _ => ("TRUE", "FALSE"),
                    };
                    out.push(format!("d <- merge(d, d2, by = {}, all.x = {x}, all.y = {y}, sort = FALSE, suffixes = c(\"\", \"_2\"), incomparables = NA)  # row order can differ from PrismStatistics'", vec_r(&keys)));
                }
            }
            "aggregate" => {
                let a = &op.args;
                let breaks = strs(&a["breaks"]);
                let by = format!("prep(d)[{}]", vec_r(&breaks));
                out.push(String::new());
                let fun = |f: &str| -> &'static str {
                    match f {
                        "sum" => "function(v) if (all(is.na(v))) NA else sum(v, na.rm = TRUE)",
                        "median" => "function(v) median(v, na.rm = TRUE)",
                        "min" => "function(v) if (all(is.na(v))) NA else min(v, na.rm = TRUE)",
                        "max" => "function(v) if (all(is.na(v))) NA else max(v, na.rm = TRUE)",
                        "sd" => "function(v) sd(v, na.rm = TRUE)",
                        "n" => "function(v) sum(!is.na(v))",
                        "nmiss" => "function(v) sum(is.na(v))",
                        "cases" => "length",
                        "first" => "function(v) v[!is.na(v)][1]",
                        "last" => "function(v) rev(v[!is.na(v)])[1]",
                        _ => "function(v) mean(v, na.rm = TRUE)",
                    }
                };
                let sums = a["summaries"].as_array().cloned().unwrap_or_default();
                if a["add_to_cases"].as_bool() == Some(true) {
                    out.push(format!("g <- interaction({by}, drop = TRUE)"));
                    for s in &sums {
                        let src = s["source"].as_str().map_or("rep(1, nrow(d))".to_string(), |v| format!("prep(d)[[{}]]", q(v)));
                        out.push(format!("d[[{}]] <- ave({src}, g, FUN = {})", q(s["target"].as_str().unwrap_or("")), fun(s["function"].as_str().unwrap_or("mean"))));
                    }
                } else {
                    out.push("# One row per group (aggregate() leaves out cases with a missing break value).".to_string());
                    let parts: Vec<String> = sums
                        .iter()
                        .map(|s| {
                            let src = s["source"].as_str().map_or("rep(1, nrow(d))".to_string(), |v| format!("prep(d)[[{}]]", q(v)));
                            format!("aggregate(list({} = {src}), by = {by}, FUN = {})", f(s["target"].as_str().unwrap_or("x")), fun(s["function"].as_str().unwrap_or("mean")))
                        })
                        .collect();
                    out.push(format!("d <- Reduce(function(x, y) merge(x, y, by = {}), list({}))", vec_r(&breaks), parts.join(", ")));
                }
            }
            "restructure_long" => {
                let a = &op.args;
                let stacks = a["stacks"].as_array().cloned().unwrap_or_default();
                let varying: Vec<String> = stacks.iter().map(|s| vec_r(&strs(&s["variables"]))).collect();
                let targets: Vec<String> = stacks.iter().filter_map(|s| s["target"].as_str().map(str::to_string)).collect();
                let index = a["index"].as_str().unwrap_or("index");
                out.push(String::new());
                let id = match a["id"].as_str() {
                    Some(i) => i.to_string(),
                    None => {
                        out.push("d$id <- seq_len(nrow(d))".to_string());
                        "id".to_string()
                    }
                };
                out.push(format!("d <- reshape(d, direction = \"long\", varying = list({}), v.names = {}, timevar = {}, idvar = {}); d <- d[order(d[[{}]], d[[{}]]), ]; rownames(d) <- NULL", varying.join(", "), vec_r(&targets), q(index), q(&id), q(&id), q(index)));
                if a["drop_missing"].as_bool() == Some(true) {
                    out.push(format!("d <- d[rowSums(!is.na(d[{}])) > 0, ]", vec_r(&targets)));
                }
            }
            "restructure_wide" => {
                let a = &op.args;
                let ids = strs(&a["id"]);
                let index = a["index"].as_str().unwrap_or("index");
                let vars = strs(&a["variables"]);
                let keep = if vars.is_empty() { "d".to_string() } else { format!("d[c({}, {}, {})]", vec_r(&ids), q(index), vec_r(&vars)) };
                out.push(String::new());
                out.push(format!("d <- reshape({keep}, direction = \"wide\", idvar = {}, timevar = {}, sep = \"_\"); rownames(d) <- NULL", vec_r(&ids), q(index)));
            }
            cmd => {
                if let Some(code) = analysis(cmd, &op.args) {
                    n += 1;
                    out.push(String::new());
                    let title = p.results.iter().find(|r| r.operation == op.seq).map(|r| r.bundle.title.clone()).unwrap_or_else(|| cmd.to_string());
                    out.push(format!("# {n}. {title}"));
                    if cmd != "power" {
                        out.push("x <- prep(d)".into());
                    }
                    let data = &op.args["data"];
                    if cmd != "power" && data.is_object() {
                        if let Some(f) = data["filter"].as_str() {
                            match crate::rexpr::to_r(f) {
                                Ok((expr, rows)) => {
                                    if rows && !row_helpers {
                                        row_helpers = true;
                                        out.extend(crate::rexpr::ROW_HELPERS.iter().map(|s| s.to_string()));
                                    }
                                    out.push(format!("x <- x[with(x, {expr}) %in% TRUE, ]  # Select cases"));
                                }
                                Err(why) => out.push(format!("stop(\"The Select Cases condition of this analysis can't be read ({}), so it is not repeated here.\")", why.replace(['"', '\\'], "'"))),
                            }
                        }
                        if let Some(w) = data["weight"].as_str() {
                            out.push(format!("x <- x[rep(seq_len(nrow(x)), ifelse(is.na({c}) | {c} <= 0, 0, {c})), ]  # frequency weights: each case repeated by its weight", c = col(w)));
                        }
                    }
                    match data["split"].as_str().filter(|_| cmd != "power") {
                        Some(sp) => {
                            out.push(format!("for (grp in sort(unique(na.omit({})))) {{  # Split file", col(sp)));
                            out.push(format!("  cat(\"\\n\", {}, \" = \", grp, \"\\n\", sep = \"\"); whole <- x; x <- x[{} %in% grp, ]", q(sp), col(sp)));
                            out.extend(code.into_iter().map(|l| format!("  {l}")));
                            out.push("  x <- whole".into());
                            out.push("}".into());
                        }
                        None => out.extend(code),
                    }
                }
            }
        }
    }
    if n == 0 {
        out.push(String::new());
        out.push("# No analyses have been run yet.".into());
    }
    out.iter().map(|l| l.replace(['\r', '\n', '\u{2028}', '\u{2029}', '\u{85}'], " ")).collect::<Vec<_>>().join("\n") + "\n"
}

/// `, alternative = "greater"` for a one-sided test, or nothing.
fn alt_arg(a: &Value) -> String {
    match a["alternative"].as_str() {
        Some(alt @ ("greater" | "less")) => format!(", alternative = \"{alt}\""),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use crate::Session;
    use serde_json::json;

    #[test]
    fn a_project_becomes_an_r_script() {
        let mut s = Session::new();
        s.handle("open_text", json!({ "name": "demo.csv", "text": "g,y,z\n1,3,2\n1,4,5\n1,6,3\n2,5,6\n2,7,7\n2,9,8\n9,1,1\n" })).unwrap();
        s.handle("update_variable", json!({ "name": "g", "missing_values": [9.0] })).unwrap();
        s.handle("ttest_independent", json!({ "test_variables": ["y"], "grouping_variable": "g", "groups": [1.0, 2.0] })).unwrap();
        s.handle("anova_factorial", json!({ "dependent": "y", "factors": ["g", "z"] })).ok();
        s.handle("set_cell", json!({ "row": 0, "name": "y", "value": "10" })).unwrap();
        s.handle("power", json!({ "test": "independent", "effect_size": 0.5 })).unwrap();
        let r = super::script(s.project.as_ref().unwrap());
        assert!(r.contains("d <- read.csv(\"demo.csv\", check.names = FALSE, na.strings = \"\")"), "{r}");
        assert!(r.contains("x[[\"g\"]][x[[\"g\"]] %in% c(9)] <- NA"), "{r}");
        assert!(r.contains("t.test(x[[\"y\"]][x[[\"g\"]] %in% 1], x[[\"y\"]][x[[\"g\"]] %in% 2], var.equal = FALSE, conf.level = 0.95)"), "{r}");
        assert!(r.contains("d[1, \"y\"] <- 10"), "{r}");
        assert!(r.contains("power.t.test(delta = 0.5, sd = 1, sig.level = 0.05, power = 0.8, type = \"two.sample\")"), "{r}");
        assert!(r.contains("# 1. Independent-Samples T Test"), "{r}");
    }

    #[test]
    fn an_item_analysis_becomes_r() {
        let mut s = Session::new();
        let csv = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../prism-engine/tests/reference/item_letters.csv")).unwrap();
        s.handle("open_text", json!({ "name": "item_letters.csv", "text": csv })).unwrap();
        let names: Vec<String> = (1..=12).map(|i| format!("q{i:02}")).collect();
        let key: Vec<serde_json::Value> = "B A D/B C B A C D B AC C D".split(' ').map(|k| json!({ "correct": k.split('/').collect::<Vec<_>>(), "points": 1, "adjustment": "none" })).collect();
        s.handle("item_analysis", json!({ "items": names, "key": key })).unwrap();
        let r = super::script(s.project.as_ref().unwrap());
        assert!(r.contains("keys <- list(c(\"B\"), c(\"A\"), c(\"D\", \"B\")"), "{r}");
        assert!(r.contains("KR20 = k / (k - 1)"), "{r}");
        if let Ok(path) = std::env::var("PRISM_WRITE_R") {
            std::fs::write(path, &r).unwrap();
        }
    }

    #[test]
    fn smoothed_charts_and_mixtures_become_r() {
        let mut s = Session::new();
        let mut text = String::from("sample,d,x1,x2,x3,y\n");
        for i in 0..24 {
            let (a, b) = [(1.0, 0.0), (0.5, 0.5), (0.0, 1.0), (0.5, 0.0), (0.0, 0.5), (0.0, 0.0)][i % 6];
            text += &format!("{},{},{a},{b},{},{}\n", i / 4 + 1, 10.0 + (i % 5) as f64 * 0.3, 1.0 - a - b, 5.0 + i as f64 * 0.1 + a * 2.0);
        }
        s.handle("open_text", json!({ "name": "qc.csv", "text": text })).unwrap();
        s.handle("spc", json!({ "chart": "cusum", "measures": ["d"], "subgroup": "sample", "head_start": 1 })).unwrap();
        s.handle("spc", json!({ "chart": "ewma", "measures": ["d"], "lambda": 0.3, "std_dev": 0.5 })).unwrap();
        s.handle("doe_mixture", json!({ "response": "y", "components": ["x1", "x2", "x3"] })).unwrap();
        let r = super::script(s.project.as_ref().unwrap());
        assert!(r.contains("g <- qcc.groups(x[[\"d\"]], x[[\"sample\"]])") && r.contains("cusum(g, decision.interval = 5, se.shift = 1, head.start = 1)"), "{r}");
        assert!(r.contains("ewma(g, lambda = 0.3, nsigmas = 3, std.dev = 0.5)"), "{r}");
        assert!(r.contains("lm(`y` ~ -1 + `x1` + `x2` + `x3` + `x1`:`x2` + `x1`:`x3` + `x2`:`x3`, data = x)"), "{r}");
        let csv = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../prism-engine/tests/reference/konstantopoulos.csv")).unwrap();
        let mut m = Session::new();
        m.handle("open_text", json!({ "name": "kd.csv", "text": csv })).unwrap();
        m.handle("meta", json!({ "measure": "generic", "effect": "yi", "variance": "vi", "cluster": "district" })).unwrap();
        let rm = super::script(m.project.as_ref().unwrap());
        assert!(rm.contains("rma.mv(yi, vi, random = ~ 1 | cluster/es_id"), "{rm}");
        let csv_e = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../prism-engine/tests/reference/empluk.csv")).unwrap();
        let mut g = Session::new();
        g.handle("open_text", json!({ "name": "em.csv", "text": csv_e })).unwrap();
        g.handle("pgmm", json!({ "dependent": "lemp", "covariates": ["lwage", "lcap"], "individual": "firm", "time": "year", "y_lags": 2, "x_lags": 1 })).unwrap();
        let rg = super::script(g.project.as_ref().unwrap());
        assert!(rg.contains("pgmm(lemp ~ lag(lemp, 1:2) + lag(lwage, 0:1) + lag(lcap, 0:1) | lag(lemp, 2:99)"), "{rg}");
        let csv_b = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../prism-engine/tests/reference/boston.csv")).unwrap();
        let mut ml = Session::new();
        ml.handle("open_text", json!({ "name": "bos.csv", "text": csv_b })).unwrap();
        ml.handle("forest", json!({ "dependent": "medv", "predictors": ["rm", "lstat", "crim"], "n_trees": 50, "importance": false })).unwrap();
        ml.handle("boost", json!({ "dependent": "medv", "predictors": ["rm", "lstat", "crim"], "n_trees": 50, "cv_folds": 3 })).unwrap();
        let rml = super::script(ml.project.as_ref().unwrap());
        assert!(rml.contains("randomForest(") && rml.contains("gbm("), "{rml}");
        let refdir = concat!(env!("CARGO_MANIFEST_DIR"), "/../prism-engine/tests/reference/");
        let mut dt = Session::new();
        dt.handle("open_text", json!({ "name": "people.csv", "text": std::fs::read_to_string(format!("{refdir}people.csv")).unwrap() })).unwrap();
        let ap = dt.handle("append_cases", json!({ "path": format!("{refdir}extra.csv"), "source": "src" })).unwrap();
        assert_eq!(ap["dataset"]["rows"], 16);
        dt.handle("merge_variables", json!({ "path": format!("{refdir}lookup.csv"), "keys": ["group"] })).unwrap();
        dt.handle("aggregate", json!({ "breaks": ["group"], "summaries": [{ "source": "score1", "function": "mean", "target": "m1" }], "add_to_cases": true })).unwrap();
        dt.handle("restructure_long", json!({ "stacks": [{ "target": "score", "variables": ["score1", "score2", "score3"] }], "id": "id" })).unwrap();
        let h = dt.handle("data_health", json!({})).unwrap();
        assert!(h["findings"].is_array());
        let pv = dt.handle("paste_cells", json!({ "row": 0, "name": "age", "text": "21
22
abc" })).unwrap();
        assert_eq!(pv["plan"]["problems"], 1);
        dt.handle("paste_cells", json!({ "row": 0, "name": "age", "text": "21
22
abc", "apply": true })).unwrap();
        dt.handle("health_fix", json!({ "fix": { "kind": "set_measure", "name": "id", "measure": "nominal" }, "apply": true })).unwrap();
        let rd = super::script(dt.project.as_ref().unwrap());
        assert!(rd.contains("d[1, \"age\"] <- 21") && rd.contains("d[2, \"age\"] <- 22") && !rd.contains("abc"), "{rd}");
        let hist2 = crate::script::from_history(&dt).unwrap();
        assert!(hist2.contains("paste_cells") && !hist2.contains("\"cells\""), "{hist2}");
        assert!(rd.contains("rbind(d, d2[names(d)])") && rd.contains("merge(d, d2, by = c(\"group\")") && rd.contains("ave(") && rd.contains("reshape(d, direction = \"long\""), "{rd}");
        let hist = crate::script::from_history(&dt).unwrap();
        assert!(hist.contains("append_cases") && hist.contains("\"path\":\"extra.csv\""), "{hist}");
        if let Ok(path) = std::env::var("PRISM_WRITE_R_DATA") {
            std::fs::write(&path, &rd).unwrap();
            for f in ["people.csv", "extra.csv", "lookup.csv"] {
                std::fs::copy(format!("{refdir}{f}"), std::path::Path::new(&path).with_file_name(f)).unwrap();
            }
        }
        if let Ok(path) = std::env::var("PRISM_WRITE_R_ML") {
            std::fs::write(&path, &rml).unwrap();
            std::fs::write(std::path::Path::new(&path).with_file_name("bos.csv"), &csv_b).unwrap();
        }
        if let Ok(path) = std::env::var("PRISM_WRITE_R_GMM") {
            std::fs::write(&path, &rg).unwrap();
            std::fs::write(std::path::Path::new(&path).with_file_name("em.csv"), &csv_e).unwrap();
        }
        if let Ok(path) = std::env::var("PRISM_WRITE_R_META") {
            std::fs::write(&path, &rm).unwrap();
            std::fs::write(std::path::Path::new(&path).with_file_name("kd.csv"), &csv).unwrap();
        }
        if let Ok(path) = std::env::var("PRISM_WRITE_R") {
            std::fs::write(path, &r).unwrap();
            std::fs::write(std::path::Path::new(&std::env::var("PRISM_WRITE_R").unwrap()).with_file_name("qc.csv"), text).unwrap();
        }
    }

    #[test]
    fn sorting_ranks_z_scores_reversals_and_dates_become_r() {
        let mut s = Session::new();
        let text = "id,g,x,item,born,seen\n1,2,3,1,2000-02-29,2026-02-28\n2,1,1,4,1999-12-31 23:30,2026-12-30\n3,2,3,9,2024-03-15 13:45,2024-03-16 09:15\n4,1,5,2,2026-11-14,2027-11-13\n5,9,2,5,1970-01-01,2027-01-01\n6,,4,3,,2020-01-01\n";
        s.handle("open_text", json!({ "name": "tx.csv", "text": text })).unwrap();
        s.handle("update_variable", json!({ "name": "g", "missing_values": [9.0] })).unwrap();
        s.handle("update_variable", json!({ "name": "item", "missing_values": [9.0] })).unwrap();
        s.handle("sort_cases", json!({ "keys": [{ "name": "g" }, { "name": "x", "descending": true }] })).unwrap();
        s.handle("rank_cases", json!({ "variables": ["x"], "ties": "low", "by": "g" })).unwrap();
        s.handle("rank_cases", json!({ "variables": ["x"], "descending": true, "into": ["rx_desc"] })).unwrap();
        s.handle("standardize", json!({ "variables": ["x"] })).unwrap();
        s.handle("reverse_score", json!({ "variables": ["item"], "min": 1, "max": 5 })).unwrap();
        s.handle("date_parts", json!({ "variable": "born", "source": "text_ymd", "parts": ["year", "month", "day", "weekday", "day_of_year", "hour", "minute", "day_number"] })).unwrap();
        for unit in ["days", "weeks", "months", "years"] {
            s.handle("date_difference", json!({ "start": "born", "end": "seen", "source": "text_ymd", "unit": unit, "target": format!("age_{unit}") })).unwrap();
        }
        let r = super::script(s.project.as_ref().unwrap());
        assert!(r.contains("order(p[[\"g\"]], p[[\"x\"]], decreasing = c(FALSE, TRUE), method = \"radix\", na.last = TRUE)"), "{r}");
        assert!(r.contains("rank(x[i], ties.method = \"min\", na.last = \"keep\")") && r.contains("rank(-prep(d)[[\"x\"]], ties.method = \"average\""), "{r}");
        assert!(r.contains("as.numeric(scale(prep(d)[[\"x\"]]))") && r.contains("6 - d[[\"item\"]]"), "{r}");
        assert!(r.contains("pdate <- function(x, src)") && r.matches("pdate <- function").count() == 1, "the date helper is written once");
        assert!(r.contains("d[[\"age_years\"]] <- trunc(m / 12)"), "{r}");
        // PRISM_WRITE_R_TX=<folder>: the script, its data and PrismStatistics' own values, to compare in R.
        if let Ok(dir) = std::env::var("PRISM_WRITE_R_TX") {
            let dir = std::path::Path::new(&dir);
            std::fs::write(dir.join("tx.csv"), text).unwrap();
            std::fs::write(dir.join("tx.R"), format!("{r}\nwrite.csv(d, \"r_out.csv\", row.names = FALSE)\n")).unwrap();
            let ds = &s.project.as_ref().unwrap().dataset;
            let mut out = vec![ds.variables.iter().map(|v| v.name.clone()).collect::<Vec<_>>().join(",")];
            for row in 0..ds.n_rows() {
                out.push(ds.columns.iter().map(|c| match c {
                    prism_engine::model::Column::Numeric(v) => v[row].map_or("NA".into(), |x| format!("{x:.15}")),
                    prism_engine::model::Column::Text(v) => v[row].clone().unwrap_or_else(|| "NA".into()),
                }).collect::<Vec<_>>().join(","));
            }
            std::fs::write(dir.join("ours.csv"), out.join("\n")).unwrap();
        }
    }

    /// The code an R script would run: the script with its strings, backtick names and comments taken out.
    fn r_code_only(script: &str) -> String {
        let mut out = String::new();
        let mut chars = script.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '"' | '\'' | '`' => {
                    while let Some(d) = chars.next() {
                        if d == '\\' {
                            chars.next();
                        } else if d == c {
                            break;
                        }
                    }
                    out.push_str("S");
                }
                '#' => {
                    for d in chars.by_ref() {
                        if d == '\n' {
                            out.push('\n');
                            break;
                        }
                    }
                }
                c => out.push(c),
            }
        }
        out
    }

    #[test]
    fn project_text_never_becomes_r_code() {
        let evil = "x\"); system('calc') # \n system('calc')\r\n`; unlink('~') \\";
        let mut s = Session::new();
        s.handle("open_text", json!({ "name": "demo.csv", "text": "g,y\n1,3\n1,4\n2,5\n2,7\n" })).unwrap();
        s.handle("descriptives", json!({ "variables": ["y"] })).unwrap();
        s.handle("compute_variable", json!({ "target": "y2", "formula": "y * 2" })).unwrap();
        {
            // A hand-edited project: every kind of stored text carries a line break, a quote and an R command.
            let p = s.project.as_mut().unwrap();
            p.source.as_mut().unwrap().file_name = format!("{evil}.csv");
            for op in &mut p.operations {
                match op.command.as_str() {
                    "compute_variable" => op.args["formula"] = json!(format!("y * 2\nsystem('calc')")),
                    "descriptives" => op.args["data"] = json!({ "filter": evil, "split": evil }),
                    _ => {}
                }
            }
            p.operations.push(prism_engine::project::Operation { seq: 90, command: "compute_variable".into(), args: json!({ "target": evil, "formula": "y + 1" }) });
            p.operations.push(prism_engine::project::Operation { seq: 91, command: "append_cases".into(), args: json!({ "file_name": evil, "path": evil }) });
            p.operations.push(prism_engine::project::Operation { seq: 92, command: "descriptives".into(), args: json!({ "variables": ["y"], "data": { "filter": "y > 1 & system('calc')" } }) });
            p.operations.push(prism_engine::project::Operation { seq: 93, command: "ttest_one_sample".into(), args: json!({ "test_variables": [evil], "test_value": 0 }) });
            p.operations.push(prism_engine::project::Operation { seq: 94, command: "sem".into(), args: json!({ "model": evil }) });
            for r in &mut p.results {
                r.bundle.title = evil.to_string();
            }
        }
        let script = super::script(s.project.as_ref().unwrap());
        let code = r_code_only(&script);
        for bad in ["system", "unlink", "calc"] {
            assert!(!code.contains(bad), "{bad} outside a string or comment:\n{code}\n--- script ---\n{script}");
        }
        assert!(script.contains("is left out: its settings hold characters"), "{script}");
        assert!(script.contains("Compute Variable y2 is left out") || script.contains("can't be read"), "{script}");
        assert!(script.contains("stop(\"The Select Cases condition"), "{script}");
        if let Ok(path) = std::env::var("PRISM_WRITE_R_EVIL") {
            std::fs::write(path, &script).unwrap();
        }
    }
}
