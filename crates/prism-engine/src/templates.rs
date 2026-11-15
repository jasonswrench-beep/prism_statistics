//! The template writer for every analysis (GUIDE-07): one APA-style paragraph per result, filled only from the result's
//! named fields, so the text can never disagree with the tables. A result that may not be interpreted gets no paragraph.
//!
//! The rules the paragraphs follow (10 §3): statistics as APA prints them (italic symbols marked with asterisks, no
//! leading zero for p and r, "< .001"), the interval and the effect size with every test, and a non-significant result
//! described as "did not differ significantly", never as evidence that there is no difference.

use crate::analysis::{
    agreement, anova_factorial, meta, gee, panel, dpanel, lpa, boost, forest, predict, tree, spc, doe, anova_oneway, bootstrap, canonical, factor, glm_more, manova, missing, mixed, permutation, pls, process, sem, survey, lca, irt, survival, timeseries, rm_anova, trimmed, classic_np, correlation, curve, explore, crosstabs, logistic, partial_correlation, power, rank_tests, regression, related, reliability, ttest_independent,
    ttest_one_sample, ttest_paired,
};
use crate::apa::*;
use crate::result::ResultBundle;

/// The paragraph for any result, or None when the result may not be interpreted (or the method has no template yet).
pub fn paragraph(r: &ResultBundle) -> Option<String> {
    if !r.is_ok() {
        return None;
    }
    match r.method_id.as_str() {
        ttest_independent::METHOD_ID => independent_t_paragraph(r),
        ttest_one_sample::METHOD_ID => one_sample(r),
        ttest_paired::METHOD_ID => paired(r),
        anova_oneway::METHOD_ID => anova(r),
        crosstabs::METHOD_ID => chi_square(r),
        correlation::METHOD_ID => correlations(r),
        rank_tests::MANN_WHITNEY => mann_whitney(r),
        rank_tests::WILCOXON => wilcoxon(r),
        rank_tests::KRUSKAL => kruskal(r),
        regression::METHOD_ID => linear(r).map(|t| match r.text("robust") {
            Some(kind) => format!("{t} Standard errors are heteroskedasticity-consistent ({kind})."),
            None => t,
        }),
        trimmed::YUEN | trimmed::YUEN_PAIRED | trimmed::T1WAY => trimmed_paragraph(r),
        mixed::METHOD_ID => mixed_paragraph(r),
        permutation::METHOD_ID => permutation_paragraph(r),
        survival::KM => km_paragraph(r),
        survival::COX => cox_paragraph(r),
        survival::ROC => roc_paragraph(r),
        timeseries::ACF => acf_paragraph(r),
        timeseries::SMOOTH => smooth_paragraph(r),
        timeseries::ARIMA => arima_paragraph(r),
        survey::ESTIMATES => survey_estimates_paragraph(r),
        sem::SEM => sem_paragraph(r),
        sem::invariance::INVARIANCE => invariance_paragraph(r),
        pls::PLS => pls_paragraph(r),
        lca::LCA => lca_paragraph(r),
        meta::META => meta_paragraph(r),
        gee::GEE => gee_paragraph(r),
        panel::PANEL => panel_paragraph(r),
        dpanel::PGMM => pgmm_paragraph(r),
        lpa::LPA => lpa_paragraph(r),
        boost::BOOST => boost_paragraph(r),
        forest::FOREST => forest_paragraph(r),
        predict::PREDICT => predict_paragraph(r),
        tree::TREE => tree_paragraph(r),
        spc::SPC => spc_paragraph(r),
        doe::DESIGN => design_paragraph(r),
        doe::EFFECTS => effects_paragraph(r),
        doe::RSM => rsm_paragraph(r),
        doe::MIXTURE => mixture_paragraph(r),
        irt::IRT => irt_paragraph(r),
        survey::REGRESSION => survey_regression_paragraph(r),
        missing::PATTERNS => missing_paragraph(r),
        missing::MI_REGRESSION => mi_paragraph(r),
        related::MCNEMAR => mcnemar(r),
        related::FRIEDMAN => friedman(r),
        partial_correlation::METHOD_ID => partial(r),
        curve::METHOD_ID => curve_paragraph(r),
        bootstrap::METHOD_ID => bootstrap_paragraph(r),
        rm_anova::METHOD_ID => rm_anova_paragraph(r),
        manova::METHOD_ID => manova_paragraph(r),
        factor::METHOD_ID => factor_paragraph(r),
        process::MEDIATION => mediation_paragraph(r),
        glm_more::ORDINAL | glm_more::MULTINOMIAL | glm_more::COUNT => glm_more_paragraph(r),
        process::MODERATION => moderation_paragraph(r),
        anova_factorial::METHOD_ID => factorial(r),
        reliability::METHOD_ID => cronbach(r),
        logistic::METHOD_ID => logistic_paragraph(r),
        canonical::METHOD_ID => canonical_paragraph(r),
        power::METHOD_ID => power_paragraph(r),
        classic_np::GOF => gof_paragraph(r),
        related::COCHRAN => cochran_paragraph(r),
        agreement::KAPPA => kappa_paragraph(r),
        agreement::FLEISS => fleiss_paragraph(r),
        agreement::KRIPP => kripp_paragraph(r),
        agreement::ICC => icc_paragraph(r),
        explore::METHOD_ID => explore_paragraph(r),
        classic_np::BINOMIAL => binomial_paragraph(r),
        classic_np::RUNS => runs_paragraph(r),
        classic_np::SIGN => sign_paragraph(r),
        classic_np::MEDIAN => median_paragraph(r),
        classic_np::KS2 => ks2_paragraph(r),
        _ => None,
    }
    .map(|text| text + &bayes_sentence(r) + &exclusion_sentence(r))
}

/// " A Bayes factor gave BF₁₀ = 4.21, moderate evidence for an effect (prior: ...)." when the result has one.
fn bayes_sentence(r: &ResultBundle) -> String {
    match (r.number("bf10"), r.text("bf.prior")) {
        (Some(bf), Some(prior)) => format!(
            " A Bayes factor gave BF₁₀ = {}, {} (prior: {}).",
            if bf >= 1000.0 || bf < 0.001 { format!("{:.2e}", bf) } else { num(bf, 2) },
            crate::analysis::bayes::describe(bf),
            prior.to_lowercase()
        ),
        _ => String::new(),
    }
}

fn alpha(r: &ResultBundle) -> f64 {
    1.0 - r.number("level").unwrap_or(0.95)
}

const NOT_SAME: &str = "This result does not show that there is no difference; the interval shows the differences that remain compatible with the data.";

fn one_sample(r: &ResultBundle) -> Option<String> {
    let y = r.text("y.label")?;
    let mu = plain(r.number("test_value")?);
    let (m, sd, t, dfv, p) = (
        r.number("mean")?,
        r.number("sd")?,
        r.number("t")?,
        r.number("df")?,
        r.number("p")?,
    );
    let level = r.number("level")?;
    let stats = format!(
        "*t*({}) = {}, *p* {}, mean difference = {}, {}, Cohen's *d* = {}, {}",
        df(dfv),
        num(t, 2),
        crate::apa::sided_p(r, p),
        num(r.number("diff")?, 2),
        crate::apa::sided_ci(r, level, "ci", 2)?,
        num(r.number("d")?, 2),
        ci(level, r.number("d.ci.lower")?, r.number("d.ci.upper")?, 2)
    );
    let mut text = format!("A one-sample *t* test compared the mean {y} with {mu}. ");
    if p < alpha(r) {
        let dir = if m > r.number("test_value")? {
            "higher"
        } else {
            "lower"
        };
        text += &format!(
            "The mean {y} (*M* = {}, *SD* = {}) was significantly {dir} than {mu}, {stats}.",
            num(m, 2),
            num(sd, 2)
        );
    } else {
        text += &format!("The mean {y} (*M* = {}, *SD* = {}) did not differ significantly from {mu}, {stats}. {NOT_SAME}", num(m, 2), num(sd, 2));
    }
    Some(text)
}

fn paired(r: &ResultBundle) -> Option<String> {
    let (a, c) = (r.text("x1.label")?, r.text("x2.label")?);
    let (m1, sd1, m2, sd2) = (
        r.number("x1.mean")?,
        r.number("x1.sd")?,
        r.number("x2.mean")?,
        r.number("x2.sd")?,
    );
    let (t, dfv, p, level) = (
        r.number("t")?,
        r.number("df")?,
        r.number("p")?,
        r.number("level")?,
    );
    let diff = r.number("diff")?;
    let stats = format!(
        "*t*({}) = {}, *p* {}, mean difference = {}, {}, Cohen's *d*<sub>z</sub> = {}, {}",
        df(dfv),
        num(t, 2),
        crate::apa::sided_p(r, p),
        num(diff, 2),
        crate::apa::sided_ci(r, level, "ci", 2)?,
        num(r.number("d")?, 2),
        ci(level, r.number("d.ci.lower")?, r.number("d.ci.upper")?, 2)
    );
    let n = r.number("n")? as usize;
    let mut text = format!("A paired-samples *t* test compared {a} and {c} for the {n} participants with both measures. ");
    if p < alpha(r) {
        let dir = if diff > 0.0 { "higher" } else { "lower" };
        text += &format!("{} (*M* = {}, *SD* = {}) was significantly {dir} than {c} (*M* = {}, *SD* = {}), {stats}.", sentence_case(a), num(m1, 2), num(sd1, 2), num(m2, 2), num(sd2, 2));
    } else {
        text += &format!("{} (*M* = {}, *SD* = {}) and {c} (*M* = {}, *SD* = {}) did not differ significantly, {stats}. {NOT_SAME}", sentence_case(a), num(m1, 2), num(sd1, 2), num(m2, 2), num(sd2, 2));
    }
    Some(text)
}

fn group_list(r: &ResultBundle, center: &str) -> Option<String> {
    let k = r.number("k")? as usize;
    let mut items = Vec::new();
    for i in 1..=k {
        let label = r.text(&format!("group.{i}.label"))?;
        items.push(match center {
            "mean" => {
                let m = num(r.number(&format!("group.{i}.mean"))?, 2);
                // A group of one has no SD; say how many cases it has instead of printing an undefined value.
                match r.number(&format!("group.{i}.sd")).filter(|v| v.is_finite()) {
                    Some(sd) => format!("{label} (*M* = {m}, *SD* = {})", num(sd, 2)),
                    None => format!("{label} (*M* = {m}, *n* = {})", r.number(&format!("group.{i}.n"))? as usize),
                }
            }
            _ => format!(
                "{label} (*Mdn* = {})",
                num(r.number(&format!("group.{i}.median"))?, 2)
            ),
        });
    }
    Some(series(&items))
}

fn pair_sentence(r: &ResultBundle, name: &str) -> Option<String> {
    let mut sig = Vec::new();
    let mut i = 1;
    while let Some(a) = r.text(&format!("pair.{i}.a")) {
        let b = r.text(&format!("pair.{i}.b"))?;
        let p = r.number(&format!("pair.{i}.p"))?;
        if p < alpha(r) {
            sig.push(format!("{a} and {b} (*p* {})", p_clause(p)));
        }
        i += 1;
    }
    if i == 1 {
        return None;
    }
    Some(if sig.is_empty() {
        format!(" {name} comparisons found no pair of groups that differed significantly.")
    } else {
        format!(
            " {name} comparisons showed that these groups differed significantly: {}.",
            series(&sig)
        )
    })
}

fn anova(r: &ResultBundle) -> Option<String> {
    let (y, f) = (r.text("y.label")?, r.text("factor.label")?);
    let k = r.number("k")? as usize;
    let welch = r.text("test") == Some("welch");
    let (fv, d1, d2, p) = if welch {
        (
            r.number("welch.F")?,
            r.number("welch.df1")?,
            r.number("welch.df2")?,
            r.number("welch.p")?,
        )
    } else {
        (
            r.number("F")?,
            r.number("df1")?,
            r.number("df2")?,
            r.number("p")?,
        )
    };
    let test = if welch { "Welch's *F*" } else { "*F*" };
    let stats = format!(
        "{test}({}, {}) = {}, *p* {}, ω² = {}",
        df(d1),
        df(d2),
        num(fv, 2),
        p_clause(p),
        no_leading_zero(r.number("omega2")?, 2)
    );
    let mut text = format!(
        "A one-way analysis of variance{} compared {y} across the {} groups of {f}: {}. ",
        if welch {
            " (Welch's test, which does not assume equal variances)"
        } else {
            ""
        },
        count_words(k),
        group_list(r, "mean")?
    );
    if p < alpha(r) {
        text += &format!(
            "{} differed significantly across the groups, {stats}.",
            sentence_case(y)
        );
    } else {
        text += &format!(
            "{} did not differ significantly across the groups, {stats}.",
            sentence_case(y)
        );
    }
    if let Some(name) = r.text("post_hoc") {
        text += &pair_sentence(r, name).unwrap_or_default();
    }
    let adjusted = r.text("contrast_adjust").is_some_and(|a| a != "none");
    for i in 1..=(r.number("contrasts").unwrap_or(0.0) as usize) {
        let key = format!("contrast.{i}");
        let (Some(label), Some(t), Some(d), Some(pv)) = (r.text(&format!("{key}.label")), r.number(&format!("{key}.t")), r.number(&format!("{key}.df")), r.number(&format!("{key}.p"))) else {
            continue;
        };
        let pv = if adjusted { r.number(&format!("{key}.p_adj")).unwrap_or(pv) } else { pv };
        let (lo, hi) = (r.number(&format!("{key}.lower"))?, r.number(&format!("{key}.upper"))?);
        let label = if label.starts_with("Contrast") { label.to_string() } else { format!("The {}", label.to_lowercase()) };
        text += &format!(
            " {label} was {}significant, estimate = {}, {}, *t*({}) = {}, *p* {}{}.",
            if pv < alpha(r) { "" } else { "not " },
            num(r.number(&format!("{key}.est"))?, 2),
            ci(r.number("level").unwrap_or(0.95), lo, hi, 2),
            df(d),
            num(t, 2),
            p_clause(pv),
            if adjusted { format!(" ({}-adjusted)", r.text("contrast_adjust")?) } else { String::new() }
        );
    }
    Some(text)
}

/// "Fisher's exact test gave p = .041, odds ratio = 3.10, 95% CI [1.02, 10.4]."
fn fisher_sentence(r: &ResultBundle) -> Option<String> {
    let fp = r.number("fisher.p")?;
    let hi = r.number("fisher.or.upper")?;
    Some(format!(
        "Fisher's exact test gave *p* {}, conditional odds ratio = {}, 95% CI [{}, {}].",
        p_clause(fp),
        num(r.number("fisher.or")?, 2),
        num(r.number("fisher.or.lower")?, 2),
        if hi.is_finite() { num(hi, 2) } else { "∞".into() }
    ))
}

fn chi_square(r: &ResultBundle) -> Option<String> {
    let (a, b) = (r.text("row.label")?, r.text("col.label")?);
    if r.number("chi2").is_none() {
        let fp = r.number("fisher.p")?;
        let verdict = if fp < alpha(r) { "The relationship was significant." } else { "The relationship was not significant; this does not show that the two are unrelated." };
        return Some(format!("Fisher's exact test examined the relationship between {a} and {b} (*N* = {}). {verdict} {}", r.number("n")? as usize, fisher_sentence(r)?));
    }
    let (chi, dfv, p, n) = (
        r.number("chi2")?,
        r.number("df")?,
        r.number("p")?,
        r.number("n")?,
    );
    let effect = match r.number("phi") {
        Some(phi) => format!("φ = {}", no_leading_zero(phi, 2)),
        None => format!("Cramér's *V* = {}", no_leading_zero(r.number("V")?, 2)),
    };
    let stats = format!(
        "χ²({}, *N* = {}) = {}, *p* {}, {effect}",
        df(dfv),
        n as usize,
        num(chi, 2),
        p_clause(p)
    );
    let mut text = format!(
        "A chi-square test of independence examined the relationship between {a} and {b}. "
    );
    if p < alpha(r) {
        text += &format!("The relationship was significant, {stats}.");
    } else {
        text += &format!("The relationship was not significant, {stats}. This does not show that the two are unrelated.");
    }
    if r.number("share_below_5").is_some_and(|s| s > 0.2)
        || r.number("min_expected").is_some_and(|m| m < 1.0)
    {
        text += if r.number("fisher.p").is_some() { " Some expected counts were small, so the exact test below is the safer guide." } else { " Some expected counts were small, so the *p* value should be read with caution." };
    }
    if let Some(f) = fisher_sentence(r) {
        text += " ";
        text += &f;
    }
    text += &crosstab_extras(r);
    Some(text)
}

fn correlations(r: &ResultBundle) -> Option<String> {
    let kind = r.text("coefficient")?;
    let (name, sym) = match kind {
        "pearson" => ("Pearson correlations", "*r*"),
        "spearman" => ("Spearman rank correlations", "*r*<sub>s</sub>"),
        _ => ("Kendall's tau-b correlations", "τ<sub>b</sub>"),
    };
    let mut labels = Vec::new();
    while let Some(l) = r.text(&format!("var.{}.label", labels.len() + 1)) {
        labels.push(l.to_string());
    }
    let sided = match r.text("alternative") {
        Some("greater") => " (one-sided tests of a positive correlation)",
        Some("less") => " (one-sided tests of a negative correlation)",
        _ => "",
    };
    let adjust = r.text("adjust").filter(|a| *a != "none");
    let mut sentences = vec![format!(
        "{name} were computed among {}{sided}{}.",
        series(&labels),
        adjust.map_or(String::new(), |a| format!(", with *p* values adjusted for the number of pairs by {a}'s method"))
    )];
    let level = r.number("level")?;
    for i in 1..=labels.len() {
        for j in (i + 1)..=labels.len() {
            let key = format!("pair.{i}.{j}");
            let (Some(rv), Some(p)) = (
                r.number(&format!("{key}.r")),
                if adjust.is_some() { r.number(&format!("{key}.p_adj")) } else { r.number(&format!("{key}.p")) },
            ) else {
                sentences.push(format!(
                    "The correlation between {} and {} could not be computed.",
                    labels[i - 1],
                    labels[j - 1]
                ));
                continue;
            };
            let n = r.number(&format!("{key}.n"))? as usize;
            let head = match r.number(&format!("{key}.df")) {
                Some(d) if kind != "kendall" => format!("{sym}({})", df(d)),
                _ => sym.to_string(),
            };
            let interval = match (
                r.number(&format!("{key}.ci.lower")),
                r.number(&format!("{key}.ci.upper")),
            ) {
                (Some(lo), Some(hi)) => format!(
                    ", {}% CI [{}, {}]",
                    num(level * 100.0, 0),
                    no_leading_zero(lo, 2),
                    no_leading_zero(hi, 2)
                ),
                _ => String::new(),
            };
            let stats = format!(
                "{head} = {}, *p* {}{interval}",
                no_leading_zero(rv, 2),
                p_clause(p)
            );
            let n_note = if kind == "kendall" {
                format!(" (*N* = {n})")
            } else {
                String::new()
            };
            sentences.push(if p < alpha(r) {
                let dir = if rv > 0.0 { "positively" } else { "negatively" };
                format!(
                    "{} and {} were significantly {dir} correlated, {stats}{n_note}.",
                    sentence_case(&labels[i - 1]),
                    labels[j - 1]
                )
            } else {
                format!(
                    "{} and {} were not significantly correlated, {stats}{n_note}.",
                    sentence_case(&labels[i - 1]),
                    labels[j - 1]
                )
            });
        }
    }
    Some(sentences.join(" "))
}

fn mann_whitney(r: &ResultBundle) -> Option<String> {
    let y = r.text("y.label")?;
    let (g1, g2) = (r.text("g1.label")?, r.text("g2.label")?);
    let (md1, md2) = (r.number("g1.median")?, r.number("g2.median")?);
    let (u, p, rb) = (r.number("U")?, r.number("p")?, r.number("r")?);
    let z = r
        .number("z")
        .map(|z| format!(", *z* = {}", num(z, 2)))
        .unwrap_or_default();
    let stats = format!(
        "*U* = {}{z}, *p* {}, rank-biserial *r* = {}",
        num(u, 1).trim_end_matches(".0"),
        crate::apa::sided_p(r, p),
        no_leading_zero(rb, 2)
    );
    let mut text = format!("A Mann-Whitney *U* test compared {y} between the {g1} (*Mdn* = {}, *n* = {}) and {g2} (*Mdn* = {}, *n* = {}) groups. ", num(md1, 2), r.number("g1.n")? as usize, num(md2, 2), r.number("g2.n")? as usize);
    if p < alpha(r) {
        let dir = if rb > 0.0 { "higher" } else { "lower" };
        text += &format!(
            "{} tended to be significantly {dir} in the {g1} group, {stats}.",
            sentence_case(y)
        );
    } else {
        text += &format!("The groups did not differ significantly, {stats}.");
    }
    Some(text)
}

fn wilcoxon(r: &ResultBundle) -> Option<String> {
    let (a, c) = (r.text("x1.label")?, r.text("x2.label")?);
    let (v, p, rb) = (r.number("V")?, r.number("p")?, r.number("r")?);
    let z = r
        .number("z")
        .map(|z| format!(", *z* = {}", num(z, 2)))
        .unwrap_or_default();
    let stats = format!(
        "*V* = {}{z}, *p* {}, matched-pairs rank-biserial *r* = {}",
        num(v, 1).trim_end_matches(".0"),
        crate::apa::sided_p(r, p),
        no_leading_zero(rb, 2)
    );
    let n = r.number("n")? as usize;
    let mut text = format!(
        "A Wilcoxon signed-rank test compared {a} (*Mdn* = {}) and {c} (*Mdn* = {}) for the {n} participants with both measures. ",
        num(r.number("x1.median")?, 2),
        num(r.number("x2.median")?, 2)
    );
    let ties = r.number("ties")? as usize;
    if p < alpha(r) {
        let dir = if rb > 0.0 { "higher" } else { "lower" };
        text += &format!(
            "{} was significantly {dir} than {c}, {stats}.",
            sentence_case(a)
        );
    } else {
        text += &format!("The two did not differ significantly, {stats}.");
    }
    if ties > 0 {
        text += &format!(
            " {} with no difference {} left out of the ranking.",
            sentence_case(&format!(
                "{} {}",
                count_words(ties),
                if ties == 1 { "pair" } else { "pairs" }
            )),
            if ties == 1 { "was" } else { "were" }
        );
    }
    Some(text)
}

fn kruskal(r: &ResultBundle) -> Option<String> {
    let (y, f) = (r.text("y.label")?, r.text("factor.label")?);
    let k = r.number("k")? as usize;
    let (h, dfv, p) = (r.number("H")?, r.number("df")?, r.number("p")?);
    let stats = format!(
        "*H*({}) = {}, *p* {}, ε² = {}",
        df(dfv),
        num(h, 2),
        p_clause(p),
        no_leading_zero(r.number("epsilon2")?, 2)
    );
    let mut text = format!(
        "A Kruskal-Wallis test compared {y} across the {} groups of {f}: {}. ",
        count_words(k),
        group_list(r, "median")?
    );
    if p < alpha(r) {
        text += &format!(
            "{} differed significantly across the groups, {stats}.",
            sentence_case(y)
        );
    } else {
        text += &format!(
            "{} did not differ significantly across the groups, {stats}.",
            sentence_case(y)
        );
    }
    let dunn = match r.text("dunn_adjust").unwrap_or("Holm") {
        "none" => "Dunn's (unadjusted)".to_string(),
        a => format!("Dunn's ({a}-adjusted)"),
    };
    text += &pair_sentence(r, &dunn).unwrap_or_default();
    Some(text)
}

fn linear(r: &ResultBundle) -> Option<String> {
    let y = r.text("y.label")?;
    let preds: Vec<String> = r
        .text("predictors")?
        .split('\u{1f}')
        .map(str::to_string)
        .collect();
    let (r2, adj, fv, d1, d2, p) = (
        r.number("r2")?,
        r.number("adj_r2")?,
        r.number("F")?,
        r.number("df1")?,
        r.number("df2")?,
        r.number("p")?,
    );
    let level = r.number("level")?;
    let mut text = format!(
        "A linear regression predicted {y} from {}. The model {} {}% of the variance in {y}, *R*² = {}, adjusted *R*² = {}, *F*({}, {}) = {}, *p* {}.",
        series(&preds),
        if p < alpha(r) { "significantly explained" } else { "explained" },
        num(r2 * 100.0, 1),
        no_leading_zero(r2, 2),
        no_leading_zero(adj, 2),
        df(d1),
        df(d2),
        num(fv, 2),
        p_clause(p)
    );
    if p >= alpha(r) {
        text += " The model as a whole was not significant.";
    }
    // Hierarchical entry: what each later block added.
    let blocks = r.number("blocks").unwrap_or(1.0) as usize;
    for k in 2..=blocks {
        let key = format!("step.{k}");
        let added: Vec<String> = r.text(&format!("{key}.added")).unwrap_or("").split('\u{1f}').map(str::to_string).collect();
        if let (Some(dr2), Some(fch), Some(d1), Some(d2), Some(pch)) = (r.number(&format!("{key}.dr2")), r.number(&format!("{key}.F")), r.number(&format!("{key}.df1")), r.number(&format!("{key}.df2")), r.number(&format!("{key}.p"))) {
            text += &format!(
                " Adding {} in step {k} {} the explained variance, Δ*R*² = {}, *F*({}, {}) = {}, *p* {}.",
                series(&added),
                if pch < alpha(r) { "significantly increased" } else { "did not significantly increase" },
                no_leading_zero(dr2, 2),
                df(d1),
                df(d2),
                num(fch, 2),
                p_clause(pch)
            );
        }
    }
    let terms = r.number("terms")? as usize;
    for i in 1..=terms {
        let key = format!("term.{i}");
        let label = r.text(&format!("{key}.label"))?;
        let (b, t, pt) = (
            r.number(&format!("{key}.b"))?,
            r.number(&format!("{key}.t"))?,
            r.number(&format!("{key}.p"))?,
        );
        let beta = r
            .number(&format!("{key}.beta"))
            .map(|v| format!(", β = {}", num(v, 2)))
            .unwrap_or_default();
        let stats = format!(
            "*B* = {}{beta}, *t*({}) = {}, *p* {}, {}",
            num(b, 2),
            df(d2),
            num(t, 2),
            p_clause(pt),
            ci(
                level,
                r.number(&format!("{key}.ci.lower"))?,
                r.number(&format!("{key}.ci.upper"))?,
                2
            )
        );
        text += &if pt < alpha(r) {
            format!(
                " {} was a significant predictor, {stats}.",
                sentence_case(label)
            )
        } else {
            format!(
                " {} was not a significant predictor, {stats}.",
                sentence_case(label)
            )
        };
    }
    Some(text)
}

fn mcnemar(r: &ResultBundle) -> Option<String> {
    let (a, c) = (r.text("x1.label")?, r.text("x2.label")?);
    let cat = r.text("cat.2.label")?;
    let changed = r.number("b")? + r.number("c")?;
    let (s1, s2) = (r.number("x1.share2")?, r.number("x2.share2")?);
    let n = r.number("n")? as usize;
    let exact = changed < related::EXACT_BELOW;
    let pv = if exact { r.number("p.exact")? } else { r.number("p")? };
    let stats = if exact {
        format!("exact *p* {} ({} of the {n} cases changed)", p_clause(pv), changed as usize)
    } else {
        format!("χ²(1, *N* = {n}) = {}, *p* {}", num(r.number("chi2")?, 2), p_clause(pv))
    };
    let shares = format!("{}% {cat} for {a} and {}% for {c}", num(s1 * 100.0, 1), num(s2 * 100.0, 1));
    let mut text = format!("McNemar's test compared {a} and {c} for the {n} cases with both ({shares}). ");
    if pv < 0.05 {
        text += &format!("The share answering {cat} changed significantly, {stats}.");
    } else {
        text += &format!("The share answering {cat} did not change significantly, {stats}. This does not show that the share stayed the same.");
    }
    Some(text)
}

fn friedman(r: &ResultBundle) -> Option<String> {
    let k = r.number("k")? as usize;
    let n = r.number("n")? as usize;
    let mut items = Vec::new();
    for i in 1..=k {
        items.push(format!("{} (*Mdn* = {})", r.text(&format!("group.{i}.label"))?, num(r.number(&format!("group.{i}.median"))?, 2)));
    }
    let (chi, dfv, p) = (r.number("chi2")?, r.number("df")?, r.number("p")?);
    let stats = format!("χ²({}, *N* = {n}) = {}, *p* {}, Kendall's *W* = {}", df(dfv), num(chi, 2), p_clause(p), no_leading_zero(r.number("W")?, 2));
    let mut text = format!("A Friedman test compared {} related measures for the {n} cases with all of them: {}. ", count_words(k), series(&items));
    if p < 0.05 {
        text += &format!("The measures differed significantly, {stats}.");
    } else {
        text += &format!("The measures did not differ significantly, {stats}.");
    }
    Some(text)
}

/// "Curve estimation related exam score to study hours (*N* = 57). The linear model, *y* = 52.1 + 2.40*x*, explained 28%
/// of the variance, *F*(1, 55) = 21.4, *p* < .001; ..."
fn curve_paragraph(r: &ResultBundle) -> Option<String> {
    let (y, x, n) = (r.text("y.label")?, r.text("x.label")?, r.number("n")? as usize);
    let mut text = format!("Curve estimation related {y} to {x} (*N* = {n}).");
    let mut fitted = Vec::new();
    for i in 1..=(r.number("models")? as usize) {
        let k = format!("model.{i}");
        let label = r.text(&format!("{k}.label"))?;
        let Some(r2) = r.number(&format!("{k}.r2")) else {
            text += &format!(" The {} curve could not be fitted.", label.to_lowercase());
            continue;
        };
        let (f, d1, d2, pv) = (r.number(&format!("{k}.F"))?, r.number(&format!("{k}.df1"))?, r.number(&format!("{k}.df2"))?, r.number(&format!("{k}.p"))?);
        let logged = matches!(r.text(&format!("{k}.kind")), Some("power" | "exponential" | "s_curve"));
        let original = r.number(&format!("{k}.r2_original"))?;
        let share = if logged {
            format!("*R*² = {} on the logarithmic scale ({} on the original scale)", no_leading_zero(r2, 2), no_leading_zero(original, 2))
        } else {
            format!("*R*² = {}", no_leading_zero(r2, 2))
        };
        text += &format!(
            " The {} model, {}, {} significant, {share}, *F*({}, {}) = {}, *p* {}.",
            label.to_lowercase(),
            r.text(&format!("{k}.equation"))?,
            if pv < alpha(r) { "was" } else { "was not" },
            df(d1),
            df(d2),
            num(f, 2),
            p_clause(pv)
        );
        fitted.push((label.to_lowercase(), original));
    }
    if fitted.len() > 1 {
        let best = fitted.iter().max_by(|a, b| a.1.total_cmp(&b.1))?;
        text += &format!(" On the original scale the {} curve fit most closely; a curve with more terms always fits at least as well, so the choice also rests on the scatterplot and on theory.", best.0);
    }
    Some(text)
}

/// "Bias-corrected and accelerated bootstrap intervals (2,000 resamples) were computed for the means: exam score,
/// estimate = 61.9, 95% CI [59.8, 64.0]; ..."
fn bootstrap_paragraph(r: &ResultBundle) -> Option<String> {
    let what = r.text("what")?;
    let level = r.number("level")?;
    let resamples = r.number("resamples")? as usize;
    let r_like = matches!(r.text("statistic"), Some("correlation"));
    let fmt = |v: f64| if r_like { no_leading_zero(v, 2) } else { num(v, 2) };
    let mut items = Vec::new();
    let mut any_percentile = false;
    for i in 1..=(r.number("rows")? as usize) {
        let k = format!("row.{i}");
        let label = r.text(&format!("{k}.label"))?;
        let Some(est) = r.number(&format!("{k}.estimate")) else {
            items.push(format!("{label}, not computed"));
            continue;
        };
        let (lo, hi, kind) = match (r.number(&format!("{k}.bca.lower")), r.number(&format!("{k}.bca.upper"))) {
            (Some(lo), Some(hi)) => (lo, hi, ""),
            _ => {
                any_percentile = true;
                (r.number(&format!("{k}.pct.lower"))?, r.number(&format!("{k}.pct.upper"))?, " (percentile)")
            }
        };
        items.push(format!("{label}, estimate = {}, {}% CI{kind} [{}, {}]", fmt(est), num(level * 100.0, 0), fmt(lo), fmt(hi)));
    }
    let target = match r.text("y.label") {
        Some(y) => format!("the coefficients of the regression of {y}"),
        None => format!("the {what}"),
    };
    Some(format!(
        "Bias-corrected and accelerated bootstrap intervals ({} resamples{}) were computed for {target}: {}.",
        thousands(resamples),
        if any_percentile { "; percentile intervals where BCa was not available" } else { "" },
        items.join("; ")
    ))
}

/// A repeated-measures or mixed ANOVA: Mauchly's test, then each effect with the chosen correction.
fn rm_anova_paragraph(r: &ResultBundle) -> Option<String> {
    let factor = r.text("factor")?;
    let k = r.number("k")? as usize;
    let n = r.number("n")? as usize;
    let corr = r.text("correction").unwrap_or("gg");
    let mut measures = Vec::new();
    for j in 1..=k {
        let m = r.number(&format!("measure.{j}.mean"))?;
        let sd = r.number(&format!("measure.{j}.sd")).map(|s| format!(", *SD* = {}", num(s, 2))).unwrap_or_default();
        measures.push(format!("{} (*M* = {}{sd})", r.text(&format!("measure.{j}.label"))?, num(m, 2)));
    }
    let mut text = match r.text("between.label") {
        Some(bl) => format!("A mixed analysis of variance examined {factor} ({count} measures: {list}) as a within-subjects factor and {bl} as a between-subjects factor (*N* = {n}).", count = count_words(k), list = series(&measures)),
        None => format!("A one-way repeated-measures analysis of variance compared the {count} measures of {factor}: {list} (*N* = {n}).", count = count_words(k), list = series(&measures)),
    };
    let correction = match (corr, r.number("mauchly.p")) {
        (_, None) => None,
        ("gg", Some(_)) => Some(("gg", "Greenhouse-Geisser", r.number("eps.gg")?)),
        ("hf", Some(_)) => Some(("hf", "Huynh-Feldt", r.number("eps.hf")?)),
        _ => None,
    };
    if let (Some(w), Some(chi), Some(d), Some(pv)) = (r.number("mauchly.W"), r.number("mauchly.chi2"), r.number("mauchly.df"), r.number("mauchly.p")) {
        text += &format!(
            " Mauchly's test {} the assumption of sphericity, *W* = {}, χ²({}) = {}, *p* {}{}.",
            if pv < 0.05 { "indicated a violation of" } else { "did not indicate a violation of" },
            no_leading_zero(w, 2),
            df(d),
            num(chi, 2),
            p_clause(pv),
            match &correction {
                Some((_, name, eps)) => format!("; within-subjects tests use the {name} correction (ε = {})", no_leading_zero(*eps, 2)),
                None => "; within-subjects tests assume sphericity".to_string(),
            }
        );
    }
    for key in ["between", "within", "interaction"] {
        let Some(label) = r.text(&format!("{key}.label")) else { continue };
        let f = r.number(&format!("{key}.F"))?;
        let (d1, d2, pv) = match (&correction, key) {
            (Some((suffix, _, _)), "within" | "interaction") => (r.number(&format!("{key}.{suffix}.df1"))?, r.number(&format!("{key}.{suffix}.df2"))?, r.number(&format!("{key}.{suffix}.p"))?),
            _ => (r.number(&format!("{key}.df1"))?, r.number(&format!("{key}.df2"))?, r.number(&format!("{key}.p"))?),
        };
        let what = match key {
            "interaction" => format!("The interaction of {label}"),
            "between" => format!("The main effect of {label}"),
            _ => format!("The effect of {label}"),
        };
        text += &format!(
            " {what} was {}significant, *F*({}, {}) = {}, *p* {}, η²<sub>p</sub> = {}, η²<sub>G</sub> = {}.",
            if pv < alpha(r) { "" } else { "not " },
            if d1.fract() == 0.0 { df(d1) } else { num(d1, 2) },
            if d2.fract() == 0.0 { df(d2) } else { num(d2, 2) },
            num(f, 2),
            p_clause(pv),
            no_leading_zero(r.number(&format!("{key}.pes"))?, 2),
            no_leading_zero(r.number(&format!("{key}.ges"))?, 2)
        );
    }
    if let Some(name) = r.text("post_hoc") {
        text += &pair_sentence(r, &format!("Paired ({name}-adjusted)")).unwrap_or_default();
    }
    Some(text)
}

/// "An exploratory factor analysis (maximum likelihood factor analysis with oblimin rotation) of 12 items (N = 240) ..."
fn factor_paragraph(r: &ResultBundle) -> Option<String> {
    let p = r.number("p")? as usize;
    let m = r.number("m")? as usize;
    let unit = r.text("unit")?;
    let units = if m == 1 { unit.to_string() } else { format!("{unit}s") };
    let mut text = format!(
        "A {} with {} was run on {} variables (*N* = {}). The Kaiser-Meyer-Olkin measure of sampling adequacy was {}, and Bartlett's test of sphericity was {}, χ²({}) = {}, *p* {}.",
        r.text("extraction")?,
        r.text("rotation")?,
        count_words(p),
        r.number("n")? as usize,
        no_leading_zero(r.number("kmo")?, 2),
        if r.number("bartlett.p")? < alpha(r) { "significant" } else { "not significant" },
        df(r.number("bartlett.df")?),
        num(r.number("bartlett.chi2")?, 2),
        p_clause(r.number("bartlett.p")?)
    );
    if let Some(s) = r.number("parallel.suggested") {
        let s = s as usize;
        text += &format!(" Parallel analysis suggested {} {}.", count_words(s), if s == 1 { unit.to_string() } else { format!("{unit}s") });
    }
    text += &format!(" {} {units} {} retained, accounting for {}% of the variance.", sentence_case(&count_words(m)), if m == 1 { "was" } else { "were" }, num(r.number("variance.pct")?, 1));
    // The variables that load .40 or more on each factor.
    for k in 1..=m {
        let mut items = Vec::new();
        for i in 1..=p {
            if let Some(v) = r.number(&format!("var.{i}.loading.{k}")).filter(|v| v.abs() >= 0.4) {
                items.push(format!("{} ({})", r.text(&format!("var.{i}.label"))?, no_leading_zero(v, 2)));
            }
        }
        if m > 1 || !items.is_empty() {
            text += &format!(" {} {k}: {}.", sentence_case(unit), if items.is_empty() { "no variable loaded .40 or more".to_string() } else { format!("loadings of .40 or more for {}", series(&items)) });
        }
    }
    if let (Some(c), Some(d), Some(pv)) = (r.number("fit.chi2"), r.number("fit.df"), r.number("fit.p")) {
        text += &format!(" The test of {} {units} being enough gave χ²({}) = {}, *p* {}.", count_words(m), df(d), num(c, 2), p_clause(pv));
    }
    if let Some(phi) = r.number("phi.1.2").filter(|_| m == 2) {
        text += &format!(" The two factors correlated *r* = {}.", no_leading_zero(phi, 2));
    }
    Some(text)
}

/// Ordinal, multinomial and count regression: the model test, then each coefficient with its ratio.
fn glm_more_paragraph(r: &ResultBundle) -> Option<String> {
    let y = r.text("y.label")?;
    let n = r.number("n")? as usize;
    let level = r.number("level").unwrap_or(0.95);
    let (kind, ratio) = match r.method_id.as_str() {
        glm_more::ORDINAL => ("An ordinal logistic regression (proportional odds)".to_string(), "OR"),
        glm_more::MULTINOMIAL => (format!("A multinomial logistic regression with {} as the reference category", r.text("reference.label")?), "OR"),
        _ => (format!("A {} regression", r.text("family")?), "IRR"),
    };
    let mut text = format!("{kind} modeled {y} (*N* = {n}).");
    if let (Some(c), Some(d), Some(pv)) = (r.number("chi2"), r.number("df"), r.number("p")) {
        text += &format!(
            " The model {} better than one without predictors, χ²({}) = {}, *p* {}{}.",
            if pv < alpha(r) { "fit significantly" } else { "did not fit significantly" },
            df(d),
            num(c, 2),
            p_clause(pv),
            r.number("nagelkerke").map_or(String::new(), |v| format!(", Nagelkerke *R*² = {}", no_leading_zero(v, 2)))
        );
    }
    if let Some(t) = r.number("theta") {
        text += &format!(" The dispersion parameter was θ = {}.", num(t, 2));
    }
    let describe = |key: &str, label: &str| -> Option<String> {
        let (b, pv) = (r.number(&format!("{key}.b"))?, r.number(&format!("{key}.p"))?);
        let ratio_part = match (r.number(&format!("{key}.ratio")), r.number(&format!("{key}.ratio.lower")), r.number(&format!("{key}.ratio.upper"))) {
            (Some(v), Some(lo), Some(hi)) => format!(", {ratio} = {}, {}", num(v, 2), ci(level, lo, hi, 2)),
            _ => String::new(),
        };
        Some(format!("{label} (*b* = {}, *p* {}{ratio_part})", num(b, 2), p_clause(pv)))
    };
    if r.method_id == glm_more::MULTINOMIAL {
        let k = r.number("k")? as usize;
        for c in 1..k {
            let cat = r.text(&format!("cat.{c}.label"))?;
            let mut items = Vec::new();
            let mut t = 1;
            while r.number(&format!("cat.{c}.term.{t}.b")).is_some() {
                let label = r.text(&format!("cat.{c}.term.{t}.label")).map(str::to_string).unwrap_or_else(|| format!("term {t}"));
                if let Some(d) = describe(&format!("cat.{c}.term.{t}"), &label) {
                    items.push(d);
                }
                t += 1;
            }
            if !items.is_empty() {
                text += &format!(" For {cat} against the reference: {}.", series(&items));
            }
        }
    } else {
        let mut items = Vec::new();
        let mut t = 1;
        while let Some(label) = r.text(&format!("term.{t}.label")) {
            if let Some(d) = describe(&format!("term.{t}"), label) {
                items.push(d);
            }
            t += 1;
        }
        if !items.is_empty() {
            text += &format!(" Coefficients: {}.", series(&items));
        }
    }
    Some(text)
}

fn missing_paragraph(r: &ResultBundle) -> Option<String> {
    let n = r.number("n")? as usize;
    let p = r.number("p")? as usize;
    let mut parts = Vec::new();
    for j in 1..=p {
        let m = r.number(&format!("var.{j}.missing"))?;
        if m > 0.0 {
            parts.push(format!("{} ({}%)", r.text(&format!("var.{j}.label"))?, num(100.0 * m / n as f64, 1)));
        }
    }
    let complete = r.number("complete")? as usize;
    let mut text = format!(
        "Of the {n} cases, {complete} ({}%) had every variable. {}",
        num(100.0 * complete as f64 / n as f64, 1),
        if parts.is_empty() { "No values were missing.".to_string() } else { format!("Missing values: {}.", series(&parts)) }
    );
    if let (Some(c), Some(d), Some(pv)) = (r.number("little.chi2"), r.number("little.df"), r.number("little.p")) {
        text += &format!(
            " Little's MCAR test was {}, χ²({}) = {}, *p* {}{}.",
            if pv < alpha(r) { "significant" } else { "not significant" },
            df(d),
            num(c, 2),
            p_clause(pv),
            if pv < alpha(r) { ", so the values are not missing completely at random" } else { "; this is consistent with, but does not show, that values are missing completely at random" }
        );
    }
    Some(text)
}

fn mi_paragraph(r: &ResultBundle) -> Option<String> {
    let y = r.text("y.label")?;
    let mut items = Vec::new();
    let mut k = 1;
    while let Some(label) = r.text(&format!("term.{k}.label")) {
        let key = format!("term.{k}");
        items.push(format!("{label} (*b* = {}, *t*({}) = {}, *p* {})", num(r.number(&format!("{key}.b"))?, 2), num(r.number(&format!("{key}.df"))?, 1), num(r.number(&format!("{key}.t"))?, 2), p_clause(r.number(&format!("{key}.p"))?)));
        k += 1;
    }
    Some(format!(
        "Missing values were handled by multiple imputation by chained equations ({} imputations); {} of {} cases had every variable. The regression of {y} was fitted to each imputed dataset and the results pooled by Rubin's rules: {}.",
        r.number("m")? as usize,
        r.number("complete")? as usize,
        r.number("n")? as usize,
        series(&items)
    ))
}

fn sem_paragraph(r: &ResultBundle) -> Option<String> {
    if r.text("estimator") == Some("Bayes") {
        let s = &r.settings;
        let mut text = format!(
            "A Bayesian structural equation model was estimated with blavaan's default priors (*N* = {}; {} chains of {} draws after {} of warm-up). The posterior predictive *p* was {}, DIC = {}, and WAIC = {}; the largest PSRF was {}.",
            r.number("n")? as usize,
            s["chains"].as_u64()?,
            s["samples"].as_u64()?,
            s["burnin"].as_u64()?,
            no_leading_zero(r.number("fit.ppp")?, 3),
            num(r.number("fit.dic")?, 2),
            num(r.number("fit.waic")?, 2),
            num(r.number("rhat.max")?, 3)
        );
        let count = r.number("par.count")? as usize;
        let loads: Vec<f64> = (1..=count).filter(|k| r.text(&format!("par.{k}.label")).is_some_and(|l| l.contains(" =~ "))).filter_map(|k| r.number(&format!("par.{k}.std"))).collect();
        if !loads.is_empty() {
            let (lo, hi) = loads.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(*v), b.max(*v)));
            text += &format!(" Posterior means of the standardized loadings ranged from {} to {}.", no_leading_zero(lo, 2), no_leading_zero(hi, 2));
        }
        return Some(text);
    }
    let n = r.number("n")? as usize;
    let dfv = r.number("fit.df")?;
    let what = match r.title.as_str() {
        "Confirmatory Factor Analysis" => "A confirmatory factor analysis",
        "Path Analysis" => "A path analysis",
        "Latent Growth Model" => "A latent growth model",
        "Two-Level Structural Equation Model" => "A two-level structural equation model",
        _ => "A structural equation model",
    };
    let fiml = r.number("fiml") == Some(1.0);
    let est = r.text("estimator").unwrap_or("ML");
    let mut text = if est == "WLSMV" {
        format!("{what} of the ordinal indicators was estimated by diagonally weighted least squares on their polychoric correlations, with robust standard errors and the mean- and variance-adjusted χ² (WLSMV; *N* = {n}).")
    } else {
        format!(
        "{what} was estimated by {}maximum likelihood{}{} (*N* = {n}{}).",
        if fiml { "full-information " } else { "" },
        if fiml { ", which uses every case under the assumption that values are missing at random" } else { "" },
        match est {
            "MLM" => ", with Satorra-Bentler robust standard errors and scaled χ²",
            "MLR" => ", with robust standard errors and the Yuan-Bentler scaled χ²",
            _ => "",
        },
        r.number("clusters").map(|g| format!(" in {} clusters", g as usize)).unwrap_or_default()
    )
    };
    if dfv > 0.0 && r.number("fit.chisq.scaled").is_some() {
        let s = |k: &str| r.number(k).map(|v| no_leading_zero(v, 3));
        let scaled_only = fiml || est == "WLSMV";
        let (cfi, tli, rmsea, kind) = if scaled_only { (s("fit.cfi.scaled")?, s("fit.tli.scaled")?, s("fit.rmsea.scaled")?, "scaled") } else { (s("fit.cfi.robust")?, s("fit.tli.robust")?, s("fit.rmsea.robust")?, "robust") };
        let ci = if scaled_only { String::new() } else { format!(", 90% CI [{}, {}]", s("fit.rmsea.ci.lower.robust")?, s("fit.rmsea.ci.upper.robust")?) };
        text += &format!(
            " The scaled χ² test {} exact fit, χ²({}) = {}, *p* {}; {kind} CFI = {cfi}, {kind} TLI = {tli}, {kind} RMSEA = {rmsea}{ci}, SRMR = {}.",
            if r.number("fit.pvalue.scaled")? < 0.05 { "rejected" } else { "did not reject" },
            df(dfv),
            num(r.number("fit.chisq.scaled")?, 2),
            p_clause(r.number("fit.pvalue.scaled")?),
            no_leading_zero(r.number("fit.srmr")?, 3)
        );
    } else if dfv > 0.0 {
        text += &format!(
            " The χ² test {} exact fit, χ²({}) = {}, *p* {}; CFI = {}, TLI = {}, RMSEA = {}, 90% CI [{}, {}], SRMR = {}.",
            if r.number("fit.pvalue")? < 0.05 { "rejected" } else { "did not reject" },
            df(dfv),
            num(r.number("fit.chisq")?, 2),
            p_clause(r.number("fit.pvalue")?),
            no_leading_zero(r.number("fit.cfi")?, 3),
            no_leading_zero(r.number("fit.tli")?, 3),
            no_leading_zero(r.number("fit.rmsea")?, 3),
            no_leading_zero(r.number("fit.rmsea.lower")?, 3),
            no_leading_zero(r.number("fit.rmsea.upper")?, 3),
            no_leading_zero(r.number("fit.srmr")?, 3)
        );
    } else {
        text += " The model was just identified (*df* = 0), so it fits the data exactly and its fit can't be tested.";
    }
    let count = r.number("par.count")? as usize;
    let loadings: Vec<f64> = (1..=count).filter(|k| r.text(&format!("par.{k}.label")).is_some_and(|l| l.contains(" =~ "))).filter_map(|k| r.number(&format!("par.{k}.std"))).collect();
    if !loadings.is_empty() {
        let (lo, hi) = loadings.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(*v), b.max(*v)));
        text += &format!(" Standardized loadings ranged from {} to {}.", no_leading_zero(lo, 2), no_leading_zero(hi, 2));
    }
    let omegas: Vec<String> = (1..=count)
        .filter_map(|k| r.text(&format!("par.{k}.label")).and_then(|l| l.split_once(" =~ ").map(|x| x.0.to_string())))
        .fold(Vec::new(), |mut v, f| {
            if !v.contains(&f) {
                v.push(f);
            }
            v
        })
        .into_iter()
        .filter_map(|f| Some(format!("{} ({f})", no_leading_zero(r.number(&format!("rel.omega.{f}"))?, 2))))
        .collect();
    if !omegas.is_empty() {
        text += &format!(" Composite reliability (ω) was {}.", series(&omegas));
    }
    if let Some(reps) = r.number("boot.resamples") {
        text += &format!(" Standard errors and confidence intervals are from {} bootstrap resamples.", reps as usize);
    }
    let mut paths = Vec::new();
    for k in 1..=count {
        let Some(label) = r.text(&format!("par.{k}.label")) else { continue };
        let Some((y, x)) = label.split_once(" ~ ") else { continue };
        let (Some(beta), Some(pv)) = (r.number(&format!("par.{k}.std")), r.number(&format!("par.{k}.p"))) else { continue };
        paths.push(format!("{x} → {y} (β = {}, *p* {})", no_leading_zero(beta, 2), p_clause(pv)));
    }
    if !paths.is_empty() {
        text += &format!(" Standardized paths: {}.", series(&paths));
    }
    Some(text)
}

fn design_paragraph(r: &ResultBundle) -> Option<String> {
    let runs = r.number("runs")? as usize;
    let k = r.number("factors")? as usize;
    let kind = r.settings["kind"].as_str().unwrap_or("");
    let unit = if matches!(kind, "lattice" | "centroid" | "vertices") { ("component", "components") } else { ("factor", "factors") };
    let mut text = format!("{} with {} {} in {} runs was generated", r.title, k, if k == 1 { unit.0 } else { unit.1 }, runs);
    if let Some(res) = r.number("resolution") {
        let roman = ["", "I", "II", "III", "IV", "V", "VI", "VII", "VIII"].get(res as usize).copied().unwrap_or("");
        text += &format!(" (resolution {roman}; generators {})", r.text("generators")?);
    }
    if let Some(a) = r.number("alpha") {
        text += &format!(" (axial distance α = {})", num(a, 3));
    }
    if let (Some(d), Some(g)) = (r.number("opt.d"), r.number("opt.g")) {
        if r.settings["criterion"].as_str() == Some("I") {
            text += &format!(" by an exchange algorithm minimizing the average prediction variance (I = {}, D = {})", num(r.number("opt.i")?, 3), num(d, 3));
        } else {
            text += &format!(" by Federov's exchange algorithm (D = {}, G-efficiency = {})", num(d, 3), num(g, 2));
        }
    }
    text.push('.');
    if kind == "pb" {
        text += " A Plackett-Burman design estimates main effects only: each is partially aliased with the two-factor interactions that do not involve it.";
    }
    if r.settings["randomize"].as_bool() == Some(true) {
        text += " The run order was randomized.";
    }
    if let Some(res) = r.number("resolution") {
        text += match res as usize {
            3 => " In a resolution III design, main effects are aliased with two-factor interactions.",
            4 => " In a resolution IV design, main effects are clear of two-factor interactions, which are aliased with one another.",
            _ => " Main effects and two-factor interactions are clear of one another.",
        };
    }
    Some(text)
}

fn effects_paragraph(r: &ResultBundle) -> Option<String> {
    let mut active = Vec::new();
    let mut i = 1;
    while let Some(t) = r.text(&format!("active.{i}")) {
        active.push(t.to_string());
        i += 1;
    }
    let mut parts = Vec::new();
    let mut j = 1;
    while let Some(term) = r.text(&format!("effect.{j}.term")) {
        if active.iter().any(|a| a == term) {
            let e = r.number(&format!("effect.{j}.est"))?;
            parts.push(match r.number(&format!("effect.{j}.p")) {
                Some(p) => format!("{term} (effect = {}, *p* {})", num(e, 2), p_clause(p)),
                None => format!("{term} (effect = {})", num(e, 2)),
            });
        }
        j += 1;
    }
    let mut text = format!("The effects on {} were estimated from the factorial experiment (*N* = {}); Lenth's method gave a pseudo standard error of {} and a margin of error of {}.", r.text("y.label")?, r.number("n")? as usize, num(r.number("lenth.pse")?, 2), num(r.number("lenth.me")?, 2));
    if parts.is_empty() {
        text += " No effect exceeded the margin of error.";
    } else {
        text += &format!(" Effects beyond the margin of error were {}.", series(&parts));
    }
    Some(text)
}

fn mixture_paragraph(r: &ResultBundle) -> Option<String> {
    let mut text = format!(
        "A Scheffé {} mixture model was fitted to {} (*N* = {}), *F*({}, {}) = {}, *p* {}, *R*² = {}, adjusted *R*² = {}.",
        r.text("model")?,
        r.text("y.label")?,
        r.number("n")? as usize,
        r.number("df1")? as usize,
        r.number("df2")? as usize,
        num(r.number("f")?, 2),
        p_clause(r.number("f.p")?),
        no_leading_zero(r.number("r2")?, 2),
        no_leading_zero(r.number("adj_r2")?, 2)
    );
    if let (Some(f), Some(p)) = (r.number("lof.f"), r.number("lof.p")) {
        text += &format!(" Lack of fit was {}, *F* = {}, *p* {}.", if p < 0.05 { "significant" } else { "not significant" }, num(f, 2), p_clause(p));
    }
    Some(text)
}

fn rsm_paragraph(r: &ResultBundle) -> Option<String> {
    let mut text = format!(
        "A second-order response surface was fitted to {} (*N* = {}, *R*² = {}, adjusted *R*² = {}).",
        r.text("y.label")?,
        r.number("n")? as usize,
        no_leading_zero(r.number("r2")?, 2),
        no_leading_zero(r.number("adj_r2")?, 2)
    );
    if let (Some(f), Some(p)) = (r.number("lof.f"), r.number("lof.p")) {
        text += &format!(" Lack of fit was {}, *F* = {}, *p* {}.", if p < 0.05 { "significant" } else { "not significant" }, num(f, 2), p_clause(p));
    }
    if let Some(kind) = r.text("stationary.kind") {
        let mut xs = Vec::new();
        let mut j = 1;
        while let Some(v) = r.number(&format!("stationary.{j}")) {
            xs.push(num(v, 2));
            j += 1;
        }
        text += &format!(" The stationary point, ({}) in coded units, is {kind}, with a predicted response of {}.", xs.join(", "), num(r.number("stationary.y")?, 2));
    }
    Some(text)
}

fn spc_paragraph(r: &ResultBundle) -> Option<String> {
    let s = &r.settings;
    let k = s["nsigmas"].as_f64().unwrap_or(3.0);
    let run = s["run_length"].as_u64().unwrap_or(7);
    match s["chart"].as_str() {
        Some("cusum") => {
            let samples = r.number("cusum_pos.count")? as usize;
            let mut text = format!(
                "The process was monitored with a tabular CUSUM chart ({samples} samples, σ = {}; shift to detect {} standard errors, decision interval h = {}).",
                num(r.number("std_dev")?, 4),
                num(s["se_shift"].as_f64().unwrap_or(1.0), 1),
                num(s["decision_interval"].as_f64().unwrap_or(5.0), 1)
            );
            let up = r.text("cusum_pos.beyond.points").unwrap_or("");
            let lo = r.text("cusum_neg.beyond.points").unwrap_or("");
            let mut sig = Vec::new();
            if !up.is_empty() {
                sig.push(format!("the upper sum signaled an upward shift at samples {up}"));
            }
            if !lo.is_empty() {
                sig.push(format!("the lower sum signaled a downward shift at samples {lo}"));
            }
            text += &if sig.is_empty() { " Neither cumulative sum crossed the decision interval, so no shift in the process mean was detected.".to_string() } else { format!(" The process mean shifted: {}.", series(&sig)) };
            return Some(text);
        }
        Some("ewma") => {
            let samples = r.number("ewma.count")? as usize;
            let mut text = format!("The process was monitored with an EWMA chart ({samples} samples, λ = {}, limits at ±{} standard errors, σ = {}).", num(s["lambda"].as_f64().unwrap_or(0.2), 2), num(k, 1), num(r.number("std_dev")?, 4));
            let pts = r.text("ewma.beyond.points").unwrap_or("");
            text += &if pts.is_empty() { " No smoothed value fell outside the control limits, so no shift in the process mean was detected.".to_string() } else { format!(" The smoothed values at samples {pts} fell outside the control limits, signaling a shift in the process mean.") };
            return Some(text);
        }
        _ => {}
    }
    let (charts, keys): (&str, Vec<(&str, &str)>) = match s["chart"].as_str().unwrap_or("xbar_r") {
        "xbar_r" => ("X̄ and R charts", vec![("xbar", "X̄"), ("r", "R")]),
        "xbar_s" => ("X̄ and S charts", vec![("xbar", "X̄"), ("s", "S")]),
        "imr" => ("individuals and moving-range charts", vec![("i", "individuals"), ("mr", "moving-range")]),
        "p" => ("a p chart", vec![("p", "p")]),
        "np" => ("an np chart", vec![("np", "np")]),
        "c" => ("a c chart", vec![("c", "c")]),
        _ => ("a u chart", vec![("u", "u")]),
    };
    let samples = r.number(&format!("{}.count", keys[0].0))? as usize;
    let mut text = format!("The process was monitored with {charts} ({samples} samples; limits at ±{} standard deviations", num(k, 0));
    if let Some(sd) = r.number("std_dev") {
        text += &format!(", σ = {}", num(sd, 4));
    }
    text += ").";
    let mut flagged = Vec::new();
    for (key, name) in &keys {
        let beyond = r.text(&format!("{key}.beyond.points")).unwrap_or("");
        let runs = r.text(&format!("{key}.runs.points")).unwrap_or("");
        if !beyond.is_empty() {
            flagged.push(format!("on the {name} chart, samples {beyond} fell outside the control limits"));
        }
        if !runs.is_empty() {
            flagged.push(format!("on the {name} chart, samples {runs} completed runs of {run} or more on one side of the center line"));
        }
    }
    if flagged.is_empty() {
        text += " No sample fell outside the control limits or formed a long run, so the process appeared to be in statistical control.";
    } else {
        text += &format!(" The process was not in statistical control: {}.", series(&flagged));
    }
    if let (Some(cp), Some(cpk)) = (r.number("cp"), r.number("cpk")) {
        text += &format!(
            " Process capability was Cp = {}, Cpk = {}, 95% CI [{}, {}]{}.",
            num(cp, 2),
            num(cpk, 2),
            num(r.number("cpk.lower")?, 2),
            num(r.number("cpk.upper")?, 2),
            if cpk >= 1.33 { ", above the common benchmark of 1.33" } else if cpk >= 1.0 { ", capable but below the common benchmark of 1.33" } else { ", below 1, so the process does not meet its specification" }
        );
    } else if let Some(cpk) = r.number("cpk") {
        text += &format!(" Process capability was Cpk = {}.", num(cpk, 2));
    }
    Some(text)
}

fn boost_paragraph(r: &ResultBundle) -> Option<String> {
    let s = &r.settings;
    let bern = r.text("distribution")? == "bernoulli";
    let mut text = format!(
        "A gradient boosting model ({} loss, as gbm) predicted {} (*N* = {}) from {} predictors with trees of {} split{}, shrinkage {} and a bag fraction of {}; {} trees were used{}.",
        if bern { "Bernoulli" } else { "squared-error" },
        r.text("y.label")?,
        r.number("n")? as usize,
        s["predictors"].as_array()?.len(),
        s["depth"].as_u64().unwrap_or(1),
        if s["depth"].as_u64().unwrap_or(1) == 1 { "" } else { "s" },
        num(s["shrinkage"].as_f64().unwrap_or(0.1), 2),
        num(s["bag_fraction"].as_f64().unwrap_or(0.5), 2),
        r.number("best_trees")? as usize,
        if r.number("cv_error.1").is_some() { ", the number with the smallest cross-validated deviance" } else { "" }
    );
    if let Some(cv) = r.number("cv.r2") {
        text += &format!(" The model explained an estimated {}% of the variance in new data (cross-validated).", num(100.0 * cv, 1));
    } else if let Some(acc) = r.number("train.accuracy") {
        text += &format!(" It classified {}% of the training cases correctly.", num(100.0 * acc, 1));
    }
    let mut imp: Vec<(String, f64)> = r.fields.iter().filter_map(|(k, f)| match f {
        crate::result::Field::Number(v) => k.strip_prefix("influence.").map(|n| (n.to_string(), *v)),
        _ => None,
    }).collect();
    imp.sort_by(|a, b| b.1.total_cmp(&a.1));
    if let Some((name, share)) = imp.first() {
        text += &format!(" The most influential predictor was {name} ({}% relative influence).", num(*share, 1));
    }
    Some(text)
}

fn forest_paragraph(r: &ResultBundle) -> Option<String> {
    let s = &r.settings;
    let kind = r.text("kind")?;
    let mut text = format!(
        "A {kind} random forest (as randomForest) of {} trees, trying {} of {} predictors at each split, was grown to predict {} (*N* = {}).",
        s["n_trees"].as_u64().unwrap_or(500),
        r.number("mtry")? as usize,
        s["predictors"].as_array()?.len(),
        r.text("y.label")?,
        r.number("n")? as usize
    );
    if let Some(rsq) = r.number("oob.rsq") {
        text += &format!(" It explained {}% of the variance out of bag (an estimate for new data).", num(100.0 * rsq, 1));
    } else if let Some(e) = r.number("oob.error") {
        text += &format!(" Its out-of-bag error rate (an estimate for new data) was {}%.", num(100.0 * e, 1));
    }
    let mut imp: Vec<(String, f64)> = r.fields.iter().filter_map(|(k, f)| match f {
        crate::result::Field::Number(v) => k.strip_prefix("purity.").map(|n| (n.to_string(), *v)),
        _ => None,
    }).collect();
    imp.sort_by(|a, b| b.1.total_cmp(&a.1));
    if imp.len() >= 2 {
        text += &format!(" The most important predictors (by the decrease in node impurity) were {} and {}.", imp[0].0, imp[1].0);
    }
    Some(text)
}

fn lpa_paragraph(r: &ResultBundle) -> Option<String> {
    let g = r.number("classes")? as usize;
    let k = r.number("n")? as usize;
    let model = r.text("model")?;
    let mut shares = Vec::new();
    for c in 1..=g {
        shares.push(format!("{}%", num(100.0 * r.number(&format!("class.{c}.pro"))?, 1)));
    }
    let mut text = format!(
        "A latent profile analysis (Gaussian mixture models fitted by EM, as mclust) of {} cases compared covariance structures and numbers of classes by BIC; the {} model with {} {} fitted best (BIC = {}, log-likelihood = {}",
        k,
        model,
        g,
        if g == 1 { "class" } else { "classes" },
        num(r.number("bic")?, 2),
        num(r.number("ll")?, 2)
    );
    if g > 1 {
        text += &format!(", entropy = {}", no_leading_zero(r.number("entropy")?, 2));
    }
    text += ").";
    if g > 1 {
        text += &format!(" The classes held {} of the cases.", series(&shares));
    }
    Some(text)
}

fn pgmm_paragraph(r: &ResultBundle) -> Option<String> {
    let s = &r.settings;
    let two = s["model"].as_str() != Some("onestep");
    let dep = s["dependent"].as_str()?;
    let lag1 = r.number(&format!("coef.{dep}.l1.est"))?;
    let p1 = r.number(&format!("coef.{dep}.l1.p"))?;
    let mut text = format!(
        "A dynamic panel model of {} was estimated by Arellano and Bond's {}-step difference GMM ({} observations from {} individuals, {} instruments{}). The lagged outcome had a coefficient of {}, *p* {}.",
        r.text("y.label")?,
        if two { "two" } else { "one" },
        r.number("n")? as usize,
        r.number("individuals")? as usize,
        r.number("instruments")? as usize,
        if s["robust"].as_bool() != Some(false) { if two { "; Windmeijer-corrected standard errors" } else { "; robust standard errors" } } else { "" },
        num(lag1, 3),
        p_clause(p1)
    );
    if let (Some(a1), Some(p1), Some(a2), Some(p2)) = (r.number("ar1"), r.number("ar1.p"), r.number("ar2"), r.number("ar2.p")) {
        text += &format!(" The Arellano-Bond tests showed {} first-order (*z* = {}, *p* {}) and {} second-order autocorrelation (*z* = {}, *p* {}) in the differenced residuals", if p1 < 0.05 { "" } else { "no" }, num(a1, 2), p_clause(p1), if p2 < 0.05 { "" } else { "no" }, num(a2, 2), p_clause(p2));
        text = text.replace("showed  first", "showed first").replace("and  second", "and second");
    }
    if let (Some(sg), Some(df), Some(sp)) = (r.number("sargan"), r.number("sargan.df"), r.number("sargan.p")) {
        text += &format!(", and the Sargan test {} the overidentifying restrictions, χ²({}) = {}, *p* {}.", if sp < 0.05 { "rejected" } else { "did not reject" }, num(df, 0), num(sg, 2), p_clause(sp));
    }
    Some(text)
}

fn tree_paragraph(r: &ResultBundle) -> Option<String> {
    let s = &r.settings;
    let kind = r.text("kind")?;
    let prune = match s["prune"].as_str() {
        Some("none") => "was kept at the complexity parameter cp = ".to_string() + &num(s["cp"].as_f64().unwrap_or(0.01), 3),
        Some("min") => "was pruned to the size with the smallest cross-validated error".to_string(),
        _ => "was pruned by the one-standard-error rule".to_string(),
    };
    let folds = s["fold_variable"].as_str().map_or_else(|| s["folds"].as_u64().unwrap_or(10).to_string(), |_| "the given".to_string());
    let mut text = format!(
        "A {kind} tree for {} (*N* = {}) was grown by recursive partitioning (CART, as rpart) and, after {folds}-fold cross-validation, {prune}, leaving {} splits and {} terminal nodes.",
        r.text("y.label")?,
        r.number("n")? as usize,
        r.number("chosen.nsplit")? as usize,
        r.number("leaves")? as usize
    );
    if kind == "regression" {
        if let (Some(cv), Some(tr)) = (r.number("cv.r2"), r.number("train.r2")) {
            text += &format!(" The tree explained {}% of the variance in the training data and an estimated {}% in new data (cross-validated).", num(100.0 * tr, 1), num(100.0 * cv, 1));
        }
    } else if let (Some(cv), Some(tr)) = (r.number("cv.accuracy"), r.number("train.accuracy")) {
        text += &format!(" It classified {}% of the training cases correctly, with an estimated cross-validated accuracy of {}%.", num(100.0 * tr, 1), num(100.0 * cv, 1));
    }
    let mut imp: Vec<(String, f64)> = r.fields.iter().filter_map(|(k, f)| match f {
        crate::result::Field::Number(v) => k.strip_prefix("importance_pct.").map(|n| (n.to_string(), *v)),
        _ => None,
    }).collect();
    imp.sort_by(|a, b| b.1.total_cmp(&a.1));
    if let Some((name, share)) = imp.first() {
        text += &format!(" The most important predictor was {name} ({}% of the total variable importance).", num(*share, 0));
    }
    Some(text)
}

fn predict_paragraph(r: &ResultBundle) -> Option<String> {
    let s = &r.settings;
    let binary = s["outcome"].as_str() == Some("binary");
    let alpha = r.number("alpha")?;
    let penalty = match s["penalty"].as_str() {
        Some("ridge") => "ridge".to_string(),
        Some("elastic") => format!("elastic-net (α = {})", num(alpha, 2)),
        _ => "lasso".to_string(),
    };
    let one_se = s["choice"].as_str() != Some("min");
    let key = if one_se { "1se" } else { "min" };
    let k = s["fold_variable"].as_str().map_or_else(|| s["folds"].as_u64().unwrap_or(10).to_string(), |_| "the given".to_string());
    let reps = s["repeats"].as_u64().unwrap_or(1);
    let predictors = (1..).take_while(|j| r.text(&format!("term.{j}")).is_some()).count();
    let mut text = format!(
        "A {penalty} {} regression of {} on {} predictors (*N* = {}) was tuned by {}-fold cross-validation{}; the penalty λ = {} was chosen by {}, keeping {} of the {} predictors.",
        if binary { "logistic" } else { "linear" },
        r.text("y.label")?,
        predictors,
        r.number("n")? as usize,
        k,
        if reps > 1 { format!(" repeated {reps} times") } else { String::new() },
        num(r.number(&format!("lambda.{key}"))?, 4),
        if one_se { "the one-standard-error rule" } else { "the minimum cross-validated error" },
        r.number(&format!("nonzero.{key}"))? as usize,
        predictors
    );
    if binary {
        text += &format!(" Cross-validated AUC was {}, accuracy {} and Brier score {}.", no_leading_zero(r.number("cv.auc")?, 2), no_leading_zero(r.number("cv.accuracy")?, 2), no_leading_zero(r.number("cv.brier")?, 3));
    } else {
        text += &format!(" Cross-validated *R*² was {} (RMSE = {}), against {} in the training data.", no_leading_zero(r.number("cv.r2")?, 2), num(r.number("cv.rmse")?, 2), no_leading_zero(r.number("train.r2")?, 2));
    }
    Some(text)
}

fn panel_paragraph(r: &ResultBundle) -> Option<String> {
    let level = r.settings["level"].as_f64().unwrap_or(0.95);
    let lv = format!("{}%", num(100.0 * level, 0));
    let model = r.text("model")?;
    let robust = r.settings["robust"].as_bool() == Some(true);
    let random = model.starts_with("random");
    let effect = r.text("effect")?;
    let mut text = format!(
        "A {model} panel regression{} of {} ({} observations on {} individuals over {} periods{}) was estimated with {} standard errors.",
        if model.starts_with("fixed") || model.starts_with("random") { format!(" with {effect} effects") } else { String::new() },
        r.text("y.label")?,
        r.number("n")? as usize,
        r.number("individuals")? as usize,
        r.number("periods")? as usize,
        if r.number("balanced")? == 1.0 { "" } else { ", unbalanced" },
        if robust { "cluster-robust (by individual)" } else { "conventional" }
    );
    let stat = if random { "*z*" } else { "*t*" };
    let df = r.number("df")?;
    let mut parts = Vec::new();
    let mut j = 1;
    while let Some(name) = r.text(&format!("coef.{j}.name")) {
        if name != "(Intercept)" {
            let tlabel = if random { stat.to_string() } else { format!("{stat}({})", num(df, 0)) };
            parts.push(format!(
                "{name} (*b* = {}, {lv} CI [{}, {}], {tlabel} = {}, *p* {})",
                num(r.number(&format!("coef.{j}.b"))?, 3),
                num(r.number(&format!("coef.{j}.lower"))?, 3),
                num(r.number(&format!("coef.{j}.upper"))?, 3),
                num(r.number(&format!("coef.{j}.t"))?, 2),
                p_clause(r.number(&format!("coef.{j}.p"))?)
            ));
        }
        j += 1;
    }
    if !parts.is_empty() {
        text += &format!(" The estimates were {}", series(&parts));
        text += &format!("; *R*²{} = {}.", if model.starts_with("fixed") { " (within)" } else { "" }, no_leading_zero(r.number("r2")?, 2));
    }
    if let (Some(h), Some(hdf), Some(hp)) = (r.number("hausman.chisq"), r.number("hausman.df"), r.number("hausman.p")) {
        text += &format!(
            " A Hausman test {} the random-effects assumption, χ²({}) = {}, *p* {}{}.",
            if hp < 0.05 { "rejected" } else { "did not reject" },
            num(hdf, 0),
            num(h, 2),
            p_clause(hp),
            if hp < 0.05 { ", favoring fixed effects" } else { "" }
        );
    }
    if let (Some(f), Some(d1), Some(d2), Some(p)) = (r.number("ftest.f"), r.number("ftest.df1"), r.number("ftest.df2"), r.number("ftest.p")) {
        text += &format!(" Individual effects were {}, *F*({}, {}) = {}, *p* {}.", if p < 0.05 { "significant" } else { "not significant" }, num(d1, 0), num(d2, 0), num(f, 2), p_clause(p));
    }
    Some(text)
}

fn gee_paragraph(r: &ResultBundle) -> Option<String> {
    let level = r.settings["level"].as_f64().unwrap_or(0.95);
    let lv = format!("{}%", num(100.0 * level, 0));
    let fam = r.text("family")?;
    let (model, ratio) = match fam {
        "binomial" => ("a logistic", Some("odds ratio")),
        "poisson" => ("a Poisson", Some("rate ratio")),
        _ => ("a linear", None),
    };
    let mut text = format!(
        "The outcome, {}, was modeled with generalized estimating equations ({model} model with an {} working correlation; {} observations in {} clusters), with robust standard errors.",
        r.text("y.label")?,
        r.text("corstr")?,
        r.number("n")? as usize,
        r.number("clusters")? as usize
    );
    let mut parts = Vec::new();
    let mut j = 2;
    while let Some(name) = r.text(&format!("coef.{j}.name")) {
        let (b, lo, hi, w, p) = (r.number(&format!("coef.{j}.b"))?, r.number(&format!("coef.{j}.lower"))?, r.number(&format!("coef.{j}.upper"))?, r.number(&format!("coef.{j}.wald"))?, r.number(&format!("coef.{j}.p"))?);
        parts.push(match ratio {
            Some(rn) => format!("{name} ({rn} = {}, {lv} CI [{}, {}], Wald χ²(1) = {}, *p* {})", num(b.exp(), 2), num(lo.exp(), 2), num(hi.exp(), 2), num(w, 2), p_clause(p)),
            None => format!("{name} (*b* = {}, {lv} CI [{}, {}], Wald χ²(1) = {}, *p* {})", num(b, 2), num(lo, 2), num(hi, 2), num(w, 2), p_clause(p)),
        });
        j += 1;
    }
    if !parts.is_empty() {
        text += &format!(" The predictors were {}.", series(&parts));
    }
    if let Some(a) = r.number("alpha.1").filter(|_| r.number("alpha.2").is_none()) {
        text += &format!(" The estimated working correlation was {}.", no_leading_zero(a, 2));
    }
    Some(text)
}

fn meta_paragraph(r: &ResultBundle) -> Option<String> {
    let s = &r.settings;
    if s["method"].as_str() == Some("bayes") {
        let level = s["level"].as_f64().unwrap_or(0.95);
        let lv = format!("{}%", num(100.0 * level, 0));
        let measure = r.text("measure")?.to_lowercase();
        let back: meta::BackTransform = match s["measure"].as_str() {
            Some("or") => Some(("odds ratio", f64::exp)),
            Some("rr") => Some(("risk ratio", f64::exp)),
            Some("zcor") => Some(("correlation", f64::tanh)),
            _ => None,
        };
        let (m, lo, hi) = (r.number("mu.median")?, r.number("mu.lower")?, r.number("mu.upper")?);
        let mut text = format!(
            "A Bayesian random-effects meta-analysis of {} studies ({} prior for the heterogeneity τ) gave a posterior median pooled {measure} of {}, {lv} credible interval [{}, {}]",
            r.number("k")? as usize,
            r.text("tau_prior")?,
            num(m, 2),
            num(lo, 2),
            num(hi, 2)
        );
        if let Some((name, t)) = back {
            text += &format!(" ({name} = {}, [{}, {}])", num(t(m), 2), num(t(lo), 2), num(t(hi), 2));
        }
        text += &format!(
            ". The posterior median of τ was {}, {lv} credible interval [{}, {}], and the {lv} prediction interval for the effect in a new study was [{}, {}].",
            num(r.number("tau.median")?, 3),
            num(r.number("tau.lower")?, 3),
            num(r.number("tau.upper")?, 3),
            num(r.number("pred.lower")?, 2),
            num(r.number("pred.upper")?, 2)
        );
        return Some(text);
    }
    let level = s["level"].as_f64().unwrap_or(0.95);
    let lv = format!("{}%", num(100.0 * level, 0));
    let knha = s["knha"].as_bool() == Some(true);
    let stat = if knha { format!("*t*({})", num(r.number("qe.df")?, 0)) } else { "*z*".to_string() };
    let k = r.number("k")? as usize;
    let method = r.text("method")?;
    let measure = r.text("measure")?.to_lowercase();
    let fe = method == "fixed-effect";
    let clusters = r.number("clusters");
    let model = if clusters.is_some() { format!("A three-level random-effects ({method})") } else if fe { "A fixed-effect".to_string() } else { format!("A random-effects (τ² estimated by {method})") };
    let studies = match clusters {
        Some(c) => format!("{k} effects in {} clusters", c as usize),
        None => format!("{k} studies"),
    };
    let back: meta::BackTransform = match s["measure"].as_str() {
        Some("or") => Some(("odds ratio", f64::exp)),
        Some("rr") => Some(("risk ratio", f64::exp)),
        Some("zcor") => Some(("correlation", f64::tanh)),
        _ => None,
    };
    let ci = |lo: f64, hi: f64| format!("{lv} CI [{}, {}]", num(lo, 2), num(hi, 2));
    let mut text;
    let moderated = r.number("beta.2.est").is_some();
    if !moderated {
        let (est, lo, hi, z, p) = (r.number("beta.1.est")?, r.number("beta.1.lower")?, r.number("beta.1.upper")?, r.number("beta.1.stat")?, r.number("beta.1.p")?);
        text = format!("{model} meta-analysis of {studies} gave a pooled {measure} of {}, {}, {stat} = {}, *p* {}", num(est, 2), ci(lo, hi), num(z, 2), p_clause(p));
        if let Some((name, t)) = back {
            text += &format!(" ({name} = {}, {})", num(t(est), 2), ci(t(lo), t(hi)));
        }
        text.push('.');
    } else {
        text = format!("{model} meta-regression of {studies} tested");
        let mut terms = Vec::new();
        let mut j = 2;
        while let Some(term) = r.text(&format!("beta.{j}.term")) {
            terms.push(format!(
                "{} (*b* = {}, {}, *p* {})",
                term,
                num(r.number(&format!("beta.{j}.est"))?, 3),
                ci(r.number(&format!("beta.{j}.lower"))?, r.number(&format!("beta.{j}.upper"))?),
                p_clause(r.number(&format!("beta.{j}.p"))?)
            ));
            j += 1;
        }
        text += &format!(" {} as moderators", series(&terms));
        if let (Some(qm), Some(p), Some(df)) = (r.number("qm"), r.number("qm.p"), r.number("qm.df")) {
            text += &if knha { format!("; together, *F*({}, {}) = {}, *p* {}", num(df, 0), num(r.number("qe.df")?, 0), num(qm, 2), p_clause(p)) } else { format!("; together, *Q*<sub>M</sub>({}) = {}, *p* {}", num(df, 0), num(qm, 2), p_clause(p)) };
        }
        if let Some(r2) = r.number("r2") {
            text += &format!(", accounting for {}% of the heterogeneity", num(r2, 1));
        }
        text.push('.');
    }
    let (qe, df, pq) = (r.number("qe")?, r.number("qe.df")?, r.number("qe.p")?);
    let i2 = r.number("i2")?;
    text += &format!(
        " {} was {}, *Q*({}) = {}, *p* {}, *I*² = {}%",
        if moderated { "Residual heterogeneity" } else { "Heterogeneity" },
        match i2 {
            x if x < 25.0 => "low",
            x if x < 50.0 => "moderate",
            x if x < 75.0 => "substantial",
            _ => "considerable",
        },
        num(df, 0),
        num(qe, 2),
        p_clause(pq),
        num(i2, 1)
    );
    if let (Some(lo), Some(hi)) = (r.number("i2.lower"), r.number("i2.upper")) {
        text += &format!(", {lv} CI [{}, {}]", num(lo, 1), num(hi, 1));
    }
    if let (Some(s1), Some(s2)) = (r.number("sigma2.1"), r.number("sigma2.2")) {
        text += &format!(
            "; the variance between clusters was σ²<sub>3</sub> = {} (*I*² = {}%{}) and within clusters σ²<sub>2</sub> = {} (*I*² = {}%{})",
            num(s1, 3),
            num(r.number("i2.1")?, 1),
            r.number("lrt.1.p").map_or(String::new(), |p| format!(", likelihood-ratio test *p* {}", p_clause(p))),
            num(s2, 3),
            num(r.number("i2.2")?, 1),
            r.number("lrt.2.p").map_or(String::new(), |p| format!(", *p* {}", p_clause(p)))
        );
    } else if !fe {
        text += &format!(", τ² = {}", num(r.number("tau2")?, 3));
    }
    text.push('.');
    if let (Some(lo), Some(hi)) = (r.number("pi.lower"), r.number("pi.upper")) {
        let (lo, hi) = back.map_or((lo, hi), |(_, t)| (t(lo), t(hi)));
        text += &format!(" The {lv} prediction interval for the true effect in a new study was [{}, {}]{}.", num(lo, 2), num(hi, 2), back.map_or(String::new(), |(name, _)| format!(" (on the {name} scale)")));
    }
    if let (Some(z), Some(p)) = (r.number("egger.stat"), r.number("egger.p")) {
        text += &format!(" Egger's regression test {} funnel-plot asymmetry, {} = {}, *p* {}", if p < 0.05 { "indicated" } else { "did not indicate" }, if knha { "*t*" } else { "*z*" }, num(z, 2), p_clause(p));
        if let Some(k0) = r.number("tf.k0") {
            if k0 == 0.0 {
                text += "; trim and fill imputed no missing studies";
            } else {
                let (est, lo, hi) = (r.number("tf.est")?, r.number("tf.lower")?, r.number("tf.upper")?);
                let (est, lo, hi) = back.map_or((est, lo, hi), |(_, t)| (t(est), t(lo), t(hi)));
                text += &format!("; trim and fill imputed {} {} on the {} side, adjusting the estimate to {}, {}", count_words(k0 as usize), if k0 == 1.0 { "study" } else { "studies" }, r.text("tf.side")?, num(est, 2), ci(lo, hi));
            }
        }
        text.push('.');
    }
    Some(text)
}

fn lca_paragraph(r: &ResultBundle) -> Option<String> {
    let classes = r.number("classes")? as usize;
    let shares: Vec<String> = (1..=classes).filter_map(|c| r.number(&format!("P.{c}")).map(|p| format!("{}% (class {c})", num(100.0 * p, 1)))).collect();
    Some(format!(
        "A latent class analysis with {} classes (*N* = {}) had a log-likelihood of {}, AIC = {}, BIC = {}, and a relative entropy of {}. Estimated class sizes were {}.",
        count_words(classes),
        r.number("n")? as usize,
        num(r.number("llik")?, 2),
        num(r.number("aic")?, 2),
        num(r.number("bic")?, 2),
        no_leading_zero(r.number("entropy")?, 2),
        series(&shares)
    ))
}

fn irt_paragraph(r: &ResultBundle) -> Option<String> {
    let what = match r.title.as_str() {
        "Rasch Model" => "A Rasch model",
        "Graded Response Model" => "A graded response model",
        _ => "A two-parameter logistic model",
    };
    let mut a = Vec::new();
    let mut b = Vec::new();
    for (k, f) in &r.fields {
        if let crate::result::Field::Number(v) = f {
            if k.starts_with("a1.") && !k.starts_with("a1.se.") {
                a.push(*v);
            }
            // b.<item> or b<category>.<item>; not bic, which also starts with b.
            let is_b = k.strip_prefix('b').is_some_and(|rest| rest.starts_with('.') || rest.starts_with(|c: char| c.is_ascii_digit()));
            if is_b && !k.contains(".se.") && !k.contains(".p.") {
                b.push(*v);
            }
        }
    }
    let range = |v: &[f64]| v.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), x| (lo.min(*x), hi.max(*x)));
    let (alo, ahi) = range(&a);
    let (blo, bhi) = range(&b);
    let mut text = format!("{what} was fitted to {} items by marginal maximum likelihood (*N* = {}; log-likelihood = {}, BIC = {}).", a.len(), r.number("n")? as usize, num(r.number("logl")?, 2), num(r.number("bic")?, 2));
    if what != "A Rasch model" {
        text += &format!(" Discriminations ranged from {} to {},", num(alo, 2), num(ahi, 2));
        text += &format!(" and difficulties (thresholds) from {} to {}.", num(blo, 2), num(bhi, 2));
    } else {
        text += &format!(" Item difficulties ranged from {} to {}; the variance of the trait was {}.", num(blo, 2), num(bhi, 2), num(r.number("var")?, 2));
    }
    Some(text)
}

fn pls_paragraph(r: &ResultBundle) -> Option<String> {
    let n = r.number("n")? as usize;
    let mut r2 = Vec::new();
    let mut paths = Vec::new();
    for (k, f) in &r.fields {
        if let (Some(c), crate::result::Field::Number(v)) = (k.strip_prefix("r2."), f) {
            r2.push(format!("{} for {c}", no_leading_zero(*v, 2)));
        }
        if let (Some(rest), crate::result::Field::Number(v)) = (k.strip_prefix("path."), f) {
            if rest.matches('.').count() == 1 {
                let (a, c) = rest.split_once('.')?;
                let tail = match r.number(&format!("{k}.p")) {
                    Some(p) => format!(", *t* = {}, *p* {}", num(r.number(&format!("{k}.t"))?, 2), p_clause(p)),
                    None => String::new(),
                };
                paths.push(format!("{a} → {c} (β = {}{tail})", no_leading_zero(*v, 2)));
            }
        }
    }
    let mut text = format!("A PLS-SEM model was estimated (*N* = {n}){}.", if r.number("boot.used").is_some() { ", with bootstrap tests of the paths" } else { "" });
    if !paths.is_empty() {
        text += &format!(" Path coefficients: {}.", series(&paths));
    }
    if !r2.is_empty() {
        text += &format!(" *R*² was {}.", series(&r2));
    }
    text += &format!(" SRMR (saturated model) was {}.", no_leading_zero(r.number("srmr")?, 3));
    Some(text)
}

fn invariance_paragraph(r: &ResultBundle) -> Option<String> {
    let steps = r.number("steps")? as usize;
    let mut parts = Vec::new();
    for k in 2..=steps {
        let key = format!("step.{k}");
        let id = r.text(&format!("{key}.id"))?;
        let pv = r.number(&format!("{key}.p"))?;
        parts.push(format!(
            "{id} invariance was {} (Δχ²({}) = {}, *p* {}; ΔCFI = {}, ΔRMSEA = {})",
            if pv < 0.05 { "rejected" } else { "not rejected" },
            df(r.number(&format!("{key}.ddf"))?),
            num(r.number(&format!("{key}.dchisq"))?, 2),
            p_clause(pv),
            no_leading_zero(r.number(&format!("{key}.dcfi"))?, 3),
            no_leading_zero(r.number(&format!("{key}.drmsea"))?, 3)
        ));
    }
    Some(format!(
        "Measurement invariance across {} was tested with increasingly constrained models{}. The configural model fit with CFI = {} and RMSEA = {}. In χ² difference tests against the preceding model, {}.",
        r.text("group.label")?,
        if r.number("robust") == Some(1.0) { " (scaled χ² difference tests; robust fit indices)" } else { "" },
        no_leading_zero(r.number("step.1.cfi")?, 3),
        no_leading_zero(r.number("step.1.rmsea")?, 3),
        series(&parts)
    ))
}

fn survey_design_words(r: &ResultBundle) -> Option<String> {
    let (psu, strata) = (r.number("psu")? as usize, r.number("strata")? as usize);
    Some(format!("{psu} primary sampling units in {} strat{}", count_words(strata), if strata == 1 { "um" } else { "a" }))
}

fn survey_estimates_paragraph(r: &ResultBundle) -> Option<String> {
    let level = r.number("level").unwrap_or(0.95);
    let mut items = Vec::new();
    for k in 1..=(r.number("count")? as usize) {
        let key = format!("est.{k}");
        items.push(format!("{} = {} (*SE* = {}), {}", r.text(&format!("{key}.label"))?, num(r.number(&format!("{key}.value"))?, 2), num(r.number(&format!("{key}.se"))?, 2), ci(level, r.number(&format!("{key}.lower"))?, r.number(&format!("{key}.upper"))?, 2)));
    }
    Some(format!("Design-based estimates, with sampling weights and linearization standard errors ({}): {}.", survey_design_words(r)?, series(&items)))
}

fn survey_regression_paragraph(r: &ResultBundle) -> Option<String> {
    let mut items = Vec::new();
    let mut k = 1;
    while let Some(label) = r.text(&format!("term.{k}.label")) {
        let key = format!("term.{k}");
        items.push(format!("{label} (*b* = {}, *t*({}) = {}, *p* {})", num(r.number(&format!("{key}.b"))?, 2), df(r.number("df")?), num(r.number(&format!("{key}.t"))?, 2), p_clause(r.number(&format!("{key}.p"))?)));
        k += 1;
    }
    Some(format!("A survey-weighted linear regression of {} with design-based standard errors (*N* = {}) gave: {}.", r.text("y.label")?, r.number("n")? as usize, series(&items)))
}

fn acf_paragraph(r: &ResultBundle) -> Option<String> {
    let lags = r.number("lags")? as usize;
    let bound = r.number("bound")?;
    let beyond: Vec<String> = (1..=lags).filter(|k| r.number(&format!("acf.{k}")).is_some_and(|v| v.abs() > bound)).map(|k| k.to_string()).collect();
    Some(format!(
        "The autocorrelations of {} ({} observations) were examined to lag {lags}. The lag-1 autocorrelation was {}; {}. The Ljung-Box test {}, *Q*({}) = {}, *p* {}.",
        r.text("y.label")?,
        r.number("n")? as usize,
        no_leading_zero(r.number("acf.1")?, 2),
        if beyond.is_empty() { "no autocorrelation fell outside ±1.96/√*n*".to_string() } else { format!("autocorrelations fell outside ±1.96/√*n* at lag{} {}", if beyond.len() == 1 { "" } else { "s" }, beyond.join(", ")) },
        if r.number("lb.p")? < alpha(r) { "rejected independence" } else { "did not reject independence" },
        df(r.number("lb.df")?),
        num(r.number("lb.Q")?, 2),
        p_clause(r.number("lb.p")?)
    ))
}

fn smooth_paragraph(r: &ResultBundle) -> Option<String> {
    let y = r.text("y.label")?;
    Some(match (r.number("alpha"), r.number("window")) {
        (Some(a), _) => format!("Simple exponential smoothing of {y} ({} observations) used α = {}{}; the final smoothed level was {}.", r.number("n")? as usize, num(a, 3), if r.method_note.contains("minimizes") { ", chosen to minimize the squared one-step errors" } else { "" }, num(r.number("level")?, 2)),
        (_, Some(w)) => format!("{y} ({} observations) was smoothed with a centered moving average over {} cases.", r.number("n")? as usize, w as usize),
        _ => return None,
    })
}

fn arima_paragraph(r: &ResultBundle) -> Option<String> {
    let (p, d, q) = (r.number("p")? as usize, r.number("d")? as usize, r.number("q")? as usize);
    let mut terms = Vec::new();
    for i in 1..=p {
        terms.push(format!("φ{} = {} (*SE* = {})", i, num(r.number(&format!("ar{i}.est"))?, 2), num(r.number(&format!("ar{i}.se"))?, 2)));
    }
    for i in 1..=q {
        terms.push(format!("θ{} = {} (*SE* = {})", i, num(r.number(&format!("ma{i}.est"))?, 2), num(r.number(&format!("ma{i}.se"))?, 2)));
    }
    let mut text = format!(
        "An ARIMA({p}, {d}, {q}) model of {} ({} observations) was fitted by maximum likelihood{}: σ² = {}, AIC = {}.",
        r.text("y.label")?,
        r.number("n")? as usize,
        if terms.is_empty() { String::new() } else { format!(", {}", series(&terms)) },
        num(r.number("sigma2")?, 3),
        num(r.number("aic")?, 2)
    );
    if let (Some(lq), Some(ld), Some(lp)) = (r.number("lb.Q"), r.number("lb.df"), r.number("lb.p")) {
        text += &format!(" The residuals {} autocorrelated, Ljung-Box *Q*({}) = {}, *p* {}.", if lp < alpha(r) { "remained" } else { "were not significantly" }, df(ld), num(lq, 2), p_clause(lp));
    }
    Some(text)
}

fn km_paragraph(r: &ResultBundle) -> Option<String> {
    let t = r.text("time.label")?;
    let level = r.number("level").unwrap_or(0.95);
    let mut groups = Vec::new();
    let mut i = 1;
    while let Some(label) = r.text(&format!("group.{i}.label")) {
        let key = format!("group.{i}");
        let (n, ev) = (r.number(&format!("{key}.n"))? as usize, r.number(&format!("{key}.events"))? as usize);
        let med = match (r.number(&format!("{key}.median")), r.number(&format!("{key}.median.lower")), r.number(&format!("{key}.median.upper"))) {
            (Some(m), Some(lo), Some(hi)) => format!("median {} = {}, {}", t, num(m, 1), ci(level, lo, hi, 1)),
            (Some(m), _, _) => format!("median {} = {}", t, num(m, 1)),
            _ => "median not reached".to_string(),
        };
        groups.push(format!("{label} ({ev} events among {n}; {med})"));
        i += 1;
    }
    let mut text = format!("Kaplan-Meier estimates of survival: {}.", series(&groups));
    if let (Some(c), Some(d), Some(pv)) = (r.number("logrank.chi2"), r.number("logrank.df"), r.number("logrank.p")) {
        text += &format!(" The survival curves {} by the log-rank test, χ²({}) = {}, *p* {}.", if pv < alpha(r) { "differed significantly" } else { "did not differ significantly" }, df(d), num(c, 2), p_clause(pv));
    }
    Some(text)
}

fn cox_paragraph(r: &ResultBundle) -> Option<String> {
    let level = r.number("level").unwrap_or(0.95);
    let mut items = Vec::new();
    let mut k = 1;
    while let Some(label) = r.text(&format!("term.{k}.label")) {
        let key = format!("term.{k}");
        items.push(format!("{label} (HR = {}, {}, *p* {})", num(r.number(&format!("{key}.hr"))?, 2), ci(level, r.number(&format!("{key}.hr.lower"))?, r.number(&format!("{key}.hr.upper"))?, 2), p_clause(r.number(&format!("{key}.p"))?)));
        k += 1;
    }
    Some(format!(
        "A Cox proportional hazards regression of {} ({} events among {} cases) {} better than no predictors, likelihood-ratio χ²({}) = {}, *p* {}. Hazard ratios: {}.",
        r.text("time.label")?,
        r.number("events")? as usize,
        r.number("n")? as usize,
        if r.number("lr.p")? < alpha(r) { "fit significantly" } else { "did not fit significantly" },
        df(r.number("df")?),
        num(r.number("lr.chi2")?, 2),
        p_clause(r.number("lr.p")?),
        series(&items)
    ))
}

fn roc_paragraph(r: &ResultBundle) -> Option<String> {
    let level = r.number("level").unwrap_or(0.95);
    let mut items = Vec::new();
    let mut k = 1;
    while let Some(label) = r.text(&format!("var.{k}.label")) {
        let key = format!("var.{k}");
        if let Some(auc) = r.number(&format!("{key}.auc")) {
            items.push(format!(
                "{label}, AUC = {}, {}% CI [{}, {}], *p* {} (best cutoff {}: sensitivity {}, specificity {})",
                no_leading_zero(auc, 2),
                num(level * 100.0, 0),
                no_leading_zero(r.number(&format!("{key}.lower"))?, 2),
                no_leading_zero(r.number(&format!("{key}.upper"))?, 2),
                p_clause(r.number(&format!("{key}.p"))?),
                num(r.number(&format!("{key}.cutoff"))?, 2),
                no_leading_zero(r.number(&format!("{key}.sens"))?, 2),
                no_leading_zero(r.number(&format!("{key}.spec"))?, 2)
            ));
        }
        k += 1;
    }
    Some(format!("ROC analysis assessed how well each variable distinguished {} on {}: {}.", r.text("positive.label")?, r.text("state.label")?, series(&items)))
}

fn permutation_paragraph(r: &ResultBundle) -> Option<String> {
    let what = match r.text("test")? {
        "two_groups" => "compared the means of the two groups",
        "paired" => "tested the mean difference between the paired measures",
        "correlation" => "tested the correlation",
        _ => "compared the means of the groups",
    };
    let exact = r.number("exact")? == 1.0;
    let count = r.number("rearrangements")? as usize;
    let pv = r.number("p")?;
    let stat = r.number("statistic")?;
    let name = r.text("statistic.name")?;
    Some(format!(
        "A two-sided permutation test ({}) {what} (*N* = {}): {} = {}, *p* {}{}.",
        if exact { format!("exact, over all {} rearrangements", thousands(count)) } else { format!("{} random rearrangements", thousands(count)) },
        r.number("n")? as usize,
        name.to_lowercase(),
        if name == "Pearson r" { no_leading_zero(stat, 2) } else { num(stat, 2) },
        p_clause(pv),
        if pv < alpha(r) { ", a significant result" } else { "" }
    ))
}

fn mixed_paragraph(r: &ResultBundle) -> Option<String> {
    let (y, g) = (r.text("y.label")?, r.text("group.label")?);
    let mut re = vec!["a random intercept".to_string()];
    let mut i = 2;
    while let Some(l) = r.text(&format!("re.{i}.label")) {
        re.push(format!("a random slope of {l}"));
        i += 1;
    }
    let mut text = format!(
        "A linear mixed model of {y} with {} for {g} was fitted by {} ({} observations in {} groups).",
        series(&re),
        if r.method_note.contains("(REML)") { "restricted maximum likelihood" } else { "maximum likelihood" },
        r.number("n")? as usize,
        r.number("groups")? as usize
    );
    let mut items = Vec::new();
    let mut k = 1;
    while let Some(label) = r.text(&format!("term.{k}.label")) {
        let key = format!("term.{k}");
        let (est, d, t, pv) = (r.number(&format!("{key}.b"))?, r.number(&format!("{key}.df"))?, r.number(&format!("{key}.t"))?, r.number(&format!("{key}.p"))?);
        items.push(format!("{label} (*b* = {}, *t*({}) = {}, *p* {})", num(est, 2), num(d, 2), num(t, 2), p_clause(pv)));
        k += 1;
    }
    if !items.is_empty() {
        text += &format!(" Fixed effects, with Satterthwaite's degrees of freedom: {}.", series(&items));
    }
    text += &format!(" The random intercept had *SD* = {}; the residual *SD* was {}.", num(r.number("re.1.sd")?, 2), num(r.number("sigma2")?.sqrt(), 2));
    if let Some(icc) = r.number("icc") {
        text += &format!(" The intraclass correlation was {}.", no_leading_zero(icc, 2));
    }
    Some(text)
}

/// Yuen's tests and the robust one-way ANOVA of trimmed means.
fn trimmed_paragraph(r: &ResultBundle) -> Option<String> {
    let pct = num(r.number("trim")? * 100.0, 0);
    let sig = |pv: f64| if pv < 0.05 { "differed significantly" } else { "did not differ significantly" };
    match r.method_id.as_str() {
        trimmed::T1WAY => {
            let (f, d1, d2, pv) = (r.number("F")?, r.number("df1")?, r.number("df2")?, r.number("p")?);
            let mut means = Vec::new();
            for i in 1..=(r.number("k")? as usize) {
                means.push(format!("{} (*M*<sub>t</sub> = {})", r.text(&format!("group.{i}.label"))?, num(r.number(&format!("group.{i}.tmean"))?, 2)));
            }
            Some(format!(
                "A robust one-way analysis of variance of {pct}% trimmed means (Wilcox's heteroscedastic method) compared {} across the groups of {}: {}. The trimmed means {}, *F*({}, {}) = {}, *p* {}.",
                r.text("y.label")?,
                r.text("factor.label")?,
                series(&means),
                sig(pv),
                df(d1),
                num(d2, 2),
                num(f, 2),
                p_clause(pv)
            ))
        }
        id => {
            let (t, d, pv, diff, lo, hi) = (r.number("t")?, r.number("df")?, r.number("p")?, r.number("diff")?, r.number("ci.lower")?, r.number("ci.upper")?);
            let level = r.number("level").unwrap_or(0.95);
            let what = if id == trimmed::YUEN {
                format!(
                    "Yuen's test compared the {pct}% trimmed means of {} between the {} (*M*<sub>t</sub> = {}) and {} (*M*<sub>t</sub> = {}) groups",
                    r.text("y.label")?,
                    r.text("g1.label")?,
                    num(r.number("g1.tmean")?, 2),
                    r.text("g2.label")?,
                    num(r.number("g2.tmean")?, 2)
                )
            } else {
                format!(
                    "A paired Yuen test compared the {pct}% trimmed means of {} (*M*<sub>t</sub> = {}) and {} (*M*<sub>t</sub> = {})",
                    r.text("x1.label")?,
                    num(r.number("x1.tmean")?, 2),
                    r.text("x2.label")?,
                    num(r.number("x2.tmean")?, 2)
                )
            };
            Some(format!(
                "{what}. They {}, *t*({}) = {}, *p* {}, difference = {}, {}.",
                sig(pv),
                if d.fract() == 0.0 { df(d) } else { num(d, 2) },
                num(t, 2),
                p_clause(pv),
                num(diff, 2),
                ci(level, lo, hi, 2)
            ))
        }
    }
}

fn mediation_paragraph(r: &ResultBundle) -> Option<String> {
    let (x, y) = (r.text("x.label")?, r.text("y.label")?);
    let k = r.number("k")? as usize;
    let level = r.number("level")?;
    let ms: Vec<String> = (1..=k).filter_map(|i| r.text(&format!("m.{i}.label")).map(str::to_string)).collect();
    let coef = |key: &str| -> Option<String> { Some(format!("*b* = {}, *p* {}", num(r.number(&format!("{key}.b"))?, 2), p_clause(r.number(&format!("{key}.p"))?))) };
    let mut text = format!(
        "A mediation analysis (PROCESS model 4) tested whether {} mediated the relationship between {x} and {y} (*N* = {}). The total effect of {x} on {y} was {}; with the mediator{} in the model, the direct effect was {}.",
        series(&ms),
        r.number("n")? as usize,
        coef("total")?,
        if k == 1 { "" } else { "s" },
        coef("direct")?
    );
    for i in 1..=k {
        let key = format!("indirect.{i}");
        let (Some(e), Some(lo), Some(hi)) = (r.number(&format!("{key}.effect")), r.number(&format!("{key}.lower")), r.number(&format!("{key}.upper"))) else { continue };
        let sig = lo > 0.0 || hi < 0.0;
        text += &format!(
            " The indirect effect through {} was {}, {}% percentile bootstrap CI [{}, {}], so {}.",
            ms[i - 1],
            num(e, 2),
            num(level * 100.0, 0),
            num(lo, 2),
            num(hi, 2),
            if sig { "it differed from zero" } else { "it was not distinguishable from zero" }
        );
    }
    if let (Some(e), Some(lo), Some(hi)) = (r.number("indirect.total.effect"), r.number("indirect.total.lower"), r.number("indirect.total.upper")) {
        text += &format!(" The total indirect effect was {}, CI [{}, {}].", num(e, 2), num(lo, 2), num(hi, 2));
    }
    Some(text)
}

fn moderation_paragraph(r: &ResultBundle) -> Option<String> {
    let (x, w, y) = (r.text("x.label")?, r.text("w.label")?, r.text("y.label")?);
    let (b3, p3) = (r.number("xw.b")?, r.number("xw.p")?);
    let mut text = format!(
        "A moderation analysis (PROCESS model 1) tested whether {w} moderated the effect of {x} on {y} (*N* = {}). The interaction was {}significant, *b* = {}, *t*({}) = {}, *p* {}{}.",
        r.number("n")? as usize,
        if p3 < alpha(r) { "" } else { "not " },
        num(b3, 2),
        df(r.number("df")?),
        num(r.number("xw.t")?, 2),
        p_clause(p3),
        r.number("r2.change").map_or(String::new(), |d| format!(", Δ*R*² = {}", no_leading_zero(d, 3)))
    );
    let mut slopes = Vec::new();
    for i in 1..=3 {
        let key = format!("slope.{i}");
        if let (Some(lab), Some(wv), Some(b), Some(pv)) = (r.text(&format!("{key}.label")), r.number(&format!("{key}.w")), r.number(&format!("{key}.b")), r.number(&format!("{key}.p"))) {
            slopes.push(format!("at the {} of {w} ({}), *b* = {}, *p* {}", lab.to_lowercase(), num(wv, 2), num(b, 2), p_clause(pv)));
        }
    }
    if !slopes.is_empty() {
        text += &format!(" The slope of {x} was: {}.", slopes.join("; "));
    }
    if let Some(jn) = r.text("jn.text") {
        text += &format!(" Johnson-Neyman: {jn}");
    }
    Some(text)
}

fn manova_paragraph(r: &ResultBundle) -> Option<String> {
    let f_label = r.text("factor.label")?;
    let p_n = r.number("p")? as usize;
    let dvs: Vec<String> = (1..=p_n).filter_map(|j| r.text(&format!("dv.{j}.label")).map(str::to_string)).collect();
    let (key, name, sym) = match r.text("test").unwrap_or("pillai") {
        "wilks" => ("wilks", "Wilks' lambda", "Λ"),
        "hotelling" => ("hotelling", "the Hotelling-Lawley trace", "*T*"),
        "roy" => ("roy", "Roy's largest root", "Θ"),
        _ => ("pillai", "Pillai's trace", "*V*"),
    };
    let (stat, f, d1, d2, pv) = (r.number(&format!("{key}.stat"))?, r.number(&format!("{key}.F"))?, r.number(&format!("{key}.df1"))?, r.number(&format!("{key}.df2"))?, r.number(&format!("{key}.p"))?);
    let fmt_df = |d: f64| if d.fract() == 0.0 { df(d) } else { num(d, 2) };
    let mut text = format!(
        "A one-way multivariate analysis of variance compared {} across the groups of {f_label} (*N* = {}). Using {name}, the combined outcomes {} across the groups, {sym} = {}, *F*({}, {}) = {}, *p* {}, η²<sub>p</sub> = {}.",
        series(&dvs),
        r.number("n")? as usize,
        if pv < alpha(r) { "differed significantly" } else { "did not differ significantly" },
        num(stat, 2),
        fmt_df(d1),
        fmt_df(d2),
        num(f, 2),
        p_clause(pv),
        no_leading_zero(r.number("pillai.pes")?, 2)
    );
    if let (Some(chi), Some(d), Some(bp)) = (r.number("boxm.chi2"), r.number("boxm.df"), r.number("boxm.p")) {
        text += &format!(" Box's test of equal covariance matrices gave χ²({}) = {}, *p* {}.", df(d), num(chi, 2), p_clause(bp));
    }
    let mut uni = Vec::new();
    for j in 1..=p_n {
        if let (Some(fj), Some(pj)) = (r.number(&format!("dv.{j}.F")), r.number(&format!("dv.{j}.p"))) {
            uni.push(format!("{} (*F* = {}, *p* {})", r.text(&format!("dv.{j}.label"))?, num(fj, 2), p_clause(pj)));
        }
    }
    if !uni.is_empty() {
        text += &format!(" Univariate tests, not adjusted for the number of outcomes: {}.", series(&uni));
    }
    Some(text)
}

fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn partial(r: &ResultBundle) -> Option<String> {
    let controls: Vec<String> = r.text("controls")?.split('\u{1f}').map(str::to_string).collect();
    let mut labels = Vec::new();
    while let Some(l) = r.text(&format!("var.{}.label", labels.len() + 1)) {
        labels.push(l.to_string());
    }
    let level = r.number("level")?;
    let mut sentences = vec![format!("Partial correlations among {} were computed controlling for {} (*N* = {}).", series(&labels), series(&controls), r.number("n")? as usize)];
    for i in 1..=labels.len() {
        for j in (i + 1)..=labels.len() {
            let key = format!("pair.{i}.{j}");
            let (Some(rv), Some(p), Some(d)) = (r.number(&format!("{key}.r")), r.number(&format!("{key}.p")), r.number(&format!("{key}.df"))) else {
                sentences.push(format!("The partial correlation between {} and {} could not be computed.", labels[i - 1], labels[j - 1]));
                continue;
            };
            let interval = match (r.number(&format!("{key}.ci.lower")), r.number(&format!("{key}.ci.upper"))) {
                (Some(lo), Some(hi)) => format!(", {}% CI [{}, {}]", num(level * 100.0, 0), no_leading_zero(lo, 2), no_leading_zero(hi, 2)),
                _ => String::new(),
            };
            let zero = r.number(&format!("{key}.r0")).map(|z| format!(" (zero-order *r* = {})", no_leading_zero(z, 2))).unwrap_or_default();
            let stats = format!("partial *r*({}) = {}, *p* {}{interval}", df(d), no_leading_zero(rv, 2), p_clause(p));
            sentences.push(if p < 1.0 - level {
                let dir = if rv > 0.0 { "positively" } else { "negatively" };
                format!("{} and {} remained significantly {dir} correlated, {stats}{zero}.", sentence_case(&labels[i - 1]), labels[j - 1])
            } else {
                format!("{} and {} were not significantly correlated once the controls were taken into account, {stats}{zero}.", sentence_case(&labels[i - 1]), labels[j - 1])
            });
        }
    }
    Some(sentences.join(" "))
}

fn factorial(r: &ResultBundle) -> Option<String> {
    let y = r.text("y.label")?;
    let m = r.number("factors")? as usize;
    let mut names = Vec::new();
    let mut dims = Vec::new();
    for i in 1..=m {
        names.push(r.text(&format!("factor.{i}.label"))?.to_string());
        dims.push((r.number(&format!("factor.{i}.levels"))? as usize).to_string());
    }
    let dfe = r.number("error.df")?;
    let sst = r.text("ss_type").unwrap_or("Type III");
    let covs: Vec<String> = r.text("covariate.labels").map(|c| c.split('\u{1f}').map(str::to_string).collect()).unwrap_or_default();
    let kind = if covs.is_empty() { "analysis of variance".to_string() } else { format!("analysis of covariance, controlling for {},", series(&covs)) };
    let mut text = format!("A {} factorial {kind} with {sst} sums of squares examined {y} by {} (*N* = {}).", dims.join(" × "), series(&names), r.number("n")? as usize);
    for e in 1..=(r.number("effects")? as usize) {
        let key = format!("effect.{e}");
        let label = r.text(&format!("{key}.label"))?;
        let (fv, d, p, pes) = (r.number(&format!("{key}.F"))?, r.number(&format!("{key}.df"))?, r.number(&format!("{key}.p"))?, r.number(&format!("{key}.pes"))?);
        let stats = format!("*F*({}, {}) = {}, *p* {}, η<sub>p</sub>² = {}", df(d), df(dfe), num(fv, 2), p_clause(p), no_leading_zero(pes, 2));
        if r.number(&format!("{key}.covariate")) == Some(1.0) {
            text += &if p < 0.05 { format!(" The covariate {label} was significantly related to {y}, {stats}.") } else { format!(" The covariate {label} was not significantly related to {y}, {stats}.") };
            continue;
        }
        let what = if label.contains(" × ") { format!("interaction of {}", label.replace(" × ", " and ")) } else { format!("main effect of {label}") };
        text += &if p < 0.05 { format!(" The {what} was significant, {stats}.") } else { format!(" The {what} was not significant, {stats}.") };
    }
    // Pairwise comparisons that were significant.
    if let (Some(n), Some(adj)) = (r.number("pairs"), r.text("post_hoc")) {
        let mut sig = Vec::new();
        for i in 1..=(n as usize) {
            let key = format!("pair.{i}");
            if let (Some(pv), Some(a), Some(c)) = (r.number(&format!("{key}.p")), r.text(&format!("{key}.a")), r.text(&format!("{key}.b"))) {
                if pv < 0.05 {
                    sig.push(format!("{a} and {c} (*p* {})", p_clause(pv)));
                }
            }
        }
        text += &if sig.is_empty() { format!(" {adj}-adjusted comparisons of the marginal means found no significant pairwise differences.") } else { format!(" {adj}-adjusted comparisons of the marginal means showed significant differences between {}.", series(&sig)) };
    }
    if let Some(n) = r.number("simple") {
        let mut sig = Vec::new();
        for i in 1..=(n as usize) {
            let key = format!("simple.{i}");
            if let (Some(pv), Some(ef), Some(w), Some(fv), Some(d1)) = (r.number(&format!("{key}.p")), r.text(&format!("{key}.effect")), r.text(&format!("{key}.within")), r.number(&format!("{key}.F")), r.number(&format!("{key}.df1"))) {
                if pv < 0.05 {
                    sig.push(format!("{ef} where {w}, *F*({}, {}) = {}, *p* {}", df(d1), df(dfe), num(fv, 2), p_clause(pv)));
                }
            }
        }
        if !sig.is_empty() {
            text += &format!(" Simple effects tests showed significant effects of {}.", sig.join("; "));
        }
    }
    Some(text)
}

fn cronbach(r: &ResultBundle) -> Option<String> {
    let items: Vec<String> = r.text("items")?.split('\u{1f}').map(str::to_string).collect();
    let level = r.number("level")?;
    let mut text = format!(
        "Internal consistency of the {}-item scale ({}) was estimated with Cronbach's alpha, α = {}, {}% CI [{}, {}] (*N* = {}).",
        count_words(items.len()),
        series(&items),
        no_leading_zero(r.number("alpha")?, 2),
        num(level * 100.0, 0),
        no_leading_zero(r.number("alpha.ci.lower")?, 2),
        no_leading_zero(r.number("alpha.ci.upper")?, 2),
        r.number("n")? as usize
    );
    if let Some(w) = r.number("omega") {
        text += &format!(" McDonald's omega, from a one-factor model, was ω = {}.", no_leading_zero(w, 2));
    }
    if let Some(rev) = r.text("reversed") {
        let rev: Vec<String> = rev.split('\u{1f}').map(str::to_string).collect();
        text += &format!(" Before the analysis, {} {} reverse-scored.", series(&rev), if rev.len() == 1 { "was" } else { "were" });
    }
    Some(text)
}

fn logistic_paragraph(r: &ResultBundle) -> Option<String> {
    let y = r.text("y.label")?;
    let (ev, nonev) = (r.text("event.label")?, r.text("nonevent.label")?);
    let preds: Vec<String> = r.text("predictors")?.split('\u{1f}').map(str::to_string).collect();
    let (chi, dfv, p, n) = (r.number("chi2")?, r.number("df")?, r.number("p")?, r.number("n")? as usize);
    let level = r.number("level")?;
    let mut text = format!(
        "A binary logistic regression predicted the probability that {y} was {ev} (rather than {nonev}) from {}. The model {} better than one with no predictors, χ²({}, *N* = {n}) = {}, *p* {}, Nagelkerke *R*² = {}, and classified {}% of the cases correctly.",
        series(&preds),
        if p < alpha(r) { "fit significantly" } else { "did not fit significantly" },
        df(dfv),
        num(chi, 2),
        p_clause(p),
        no_leading_zero(r.number("nagelkerke")?, 2),
        num(r.number("accuracy")? * 100.0, 1)
    );
    for k in 2..=(r.number("blocks").unwrap_or(1.0) as usize) {
        let key = format!("step.{k}");
        let added: Vec<String> = r.text(&format!("{key}.added")).unwrap_or("").split('\u{1f}').map(str::to_string).collect();
        if let (Some(c), Some(d), Some(pv)) = (r.number(&format!("{key}.chi2")), r.number(&format!("{key}.df")), r.number(&format!("{key}.p"))) {
            text += &format!(" Adding {} in step {k} {} the fit, χ²({}) = {}, *p* {}.", series(&added), if pv < alpha(r) { "significantly improved" } else { "did not significantly improve" }, df(d), num(c, 2), p_clause(pv));
        }
    }
    if let (Some(auc), Some(lo), Some(hi)) = (r.number("roc.auc"), r.number("roc.lower"), r.number("roc.upper")) {
        text += &format!(" The area under the ROC curve was {}, {}.", no_leading_zero(auc, 2), ci(level, lo, hi, 2));
    }
    let mut i = 1;
    while let Some(label) = r.text(&format!("term.{i}.label")) {
        let key = format!("term.{i}");
        let (b, wald, pt, or) = (r.number(&format!("{key}.b"))?, r.number(&format!("{key}.wald"))?, r.number(&format!("{key}.p"))?, r.number(&format!("{key}.or"))?);
        let stats = format!(
            "*B* = {}, Wald χ²(1) = {}, *p* {}, *OR* = {}, {}",
            num(b, 2),
            num(wald, 2),
            p_clause(pt),
            num(or, 2),
            ci(level, r.number(&format!("{key}.or.lower"))?, r.number(&format!("{key}.or.upper"))?, 2)
        );
        text += &if pt < alpha(r) {
            let dir = if b > 0.0 { "higher" } else { "lower" };
            format!(" {} was associated with {dir} odds of {ev}, {stats}.", sentence_case(label))
        } else {
            format!(" {} was not a significant predictor, {stats}.", sentence_case(label))
        };
        i += 1;
    }
    Some(text)
}

fn canonical_paragraph(r: &ResultBundle) -> Option<String> {
    let s1: Vec<String> = r.text("set1")?.split('\u{1f}').map(str::to_string).collect();
    let s2: Vec<String> = r.text("set2")?.split('\u{1f}').map(str::to_string).collect();
    let k = r.number("functions")? as usize;
    let mut text = format!("A canonical correlation analysis examined the relationship between {} and {} (*N* = {}), giving {} canonical function{}.", series(&s1), series(&s2), r.number("n")? as usize, count_words(k), if k == 1 { "" } else { "s" });
    for i in 1..=k {
        let key = format!("fn.{i}");
        let (rc, lam, fv, d1, d2, p) = (r.number(&format!("{key}.r"))?, r.number(&format!("{key}.lambda"))?, r.number(&format!("{key}.F"))?, r.number(&format!("{key}.df1"))?, r.number(&format!("{key}.df2"))?, r.number(&format!("{key}.p"))?);
        let stats = format!("Wilks' λ = {}, *F*({}, {}) = {}, *p* {}", no_leading_zero(lam, 2), df(d1), df(d2), num(fv, 2), p_clause(p));
        let scope = if i == 1 { if k == 1 { "The function".to_string() } else { "The full model (all functions together)".to_string() } } else if i == k { format!("Function {i} alone") } else { format!("Functions {i} to {k}") };
        text += &format!(
            " The {} canonical correlation was *r*<sub>c</sub> = {}. {scope} {}, {stats}.",
            if i == 1 { "first".to_string() } else if i == 2 { "second".to_string() } else if i == 3 { "third".to_string() } else { format!("number {i}") },
            no_leading_zero(rc, 2),
            if p < 0.05 { "was significant" } else { "was not significant" }
        );
    }
    Some(text)
}

fn power_paragraph(r: &ResultBundle) -> Option<String> {
    let test = r.text("test")?;
    let sym = r.text("symbol")?;
    let es = r.number("effect")?;
    let alpha = r.number("alpha")?;
    let (n, total, pw) = (r.number("n")? as usize, r.number("total")? as usize, r.number("power")?);
    let groups = r.number("groups")? as usize;
    let effect = format!("*{sym}* = {}", if sym == "r" { no_leading_zero(es, 2) } else { num(es, 2) });
    let alpha_text = no_leading_zero(alpha, 3).trim_end_matches('0').to_string();
    // An F test has no sides; every other planned test here is two-sided.
    let sided = if test.contains("analysis of variance") { "" } else { "two-sided " };
    let sample = if groups > 2 { format!("{n} participants per group ({} groups, *N* = {total})", groups) } else if groups == 2 { format!("{n} participants per group (*N* = {total})") } else if test.starts_with("paired") { format!("{total} pairs") } else { format!("{total} participants") };
    Some(if r.text("solve_for")? == "n" {
        format!(
            "An a priori power analysis for a {sided}{test} indicated that {sample} would give {}% power to detect an effect of {effect} at α = {alpha_text} (achieved power = {}).",
            num(r.number("target_power")? * 100.0, 0),
            no_leading_zero(pw, 3)
        )
    } else {
        format!("A power analysis for a {sided}{test} with {sample} showed {}% power to detect an effect of {effect} at α = {alpha_text}.", num(pw * 100.0, 1))
    })
}

fn gof_paragraph(r: &ResultBundle) -> Option<String> {
    let l = r.text("var.label")?;
    let k = r.number("k")? as usize;
    let n = r.number("n")? as usize;
    let mut cats = Vec::new();
    for i in 1..=k {
        cats.push(format!("{} (*n* = {})", r.text(&format!("cat.{i}.label"))?, r.number(&format!("cat.{i}.n"))? as usize));
    }
    let (chi, d, p, w) = (r.number("chi2")?, r.number("df")?, r.number("p")?, r.number("w")?);
    let stats = format!("χ²({}, *N* = {n}) = {}, *p* {}, *w* = {}", df(d), num(chi, 2), p_clause(p), no_leading_zero(w, 2));
    let verdict = if p < 0.05 { format!("The distribution differed significantly from the expected proportions, {stats}.") } else { format!("The distribution did not differ significantly from the expected proportions, {stats}. This does not show that the proportions match.") };
    Some(format!("A chi-square goodness-of-fit test compared the observed counts of {l} with the expected proportions: {}. {verdict}", series(&cats)))
}

fn binomial_paragraph(r: &ResultBundle) -> Option<String> {
    let l = r.text("var.label")?;
    let s = r.text("success.label")?;
    let (n, x, p0, prop, p, lo, hi, level) = (r.number("n")? as usize, r.number("x")? as usize, r.number("p0")?, r.number("prop")?, r.number("p")?, r.number("ci.lower")?, r.number("ci.upper")?, r.number("level")?);
    let stats = format!("{} of {n}, proportion = {}, {}, exact *p* {}", x, no_leading_zero(prop, 2), ci(level, lo, hi, 2), p_clause(p));
    Some(if p < 0.05 {
        format!("An exact binomial test compared the proportion of {l} that was {s} with {}. The proportion was significantly {} than {}, {stats}.", no_leading_zero(p0, 2), if prop > p0 { "higher" } else { "lower" }, no_leading_zero(p0, 2))
    } else {
        format!("An exact binomial test compared the proportion of {l} that was {s} with {}. The proportion did not differ significantly from {}, {stats}.", no_leading_zero(p0, 2), no_leading_zero(p0, 2))
    })
}

fn runs_paragraph(r: &ResultBundle) -> Option<String> {
    let l = r.text("var.label")?;
    let (runs, p, n) = (r.number("runs")? as usize, r.number("p")?, r.number("n")? as usize);
    let stats = format!("{runs} runs, *z* = {}, exact *p* {}", num(r.number("z")?, 2), p_clause(p));
    Some(if p < 0.05 { format!("A runs test of the order of {l} across the {n} cases (cut at {}) showed a non-random sequence, {stats}.", num(r.number("cut")?, 2)) } else { format!("A runs test of the order of {l} across the {n} cases (cut at {}) showed no significant departure from a random sequence, {stats}.", num(r.number("cut")?, 2)) })
}

fn sign_paragraph(r: &ResultBundle) -> Option<String> {
    let (a, c) = (r.text("x1.label")?, r.text("x2.label")?);
    let (pos, neg, z, p) = (r.number("pos")? as usize, r.number("neg")? as usize, r.number("zeros")? as usize, r.number("p")?);
    let stats = format!("{pos} positive and {neg} negative differences ({z} ties), exact *p* {}", p_clause(p));
    Some(if p < 0.05 { format!("A sign test compared {a} and {c}. {} was significantly {} than {c}, {stats}.", sentence_case(a), if pos > neg { "higher" } else { "lower" }) } else { format!("A sign test compared {a} and {c}. They did not differ significantly, {stats}.") })
}

fn median_paragraph(r: &ResultBundle) -> Option<String> {
    let (y, f) = (r.text("y.label")?, r.text("factor.label")?);
    let (chi, d, p, n) = (r.number("chi2")?, r.number("df")?, r.number("p")?, r.number("n")? as usize);
    let mut stats = format!("χ²({}, *N* = {n}) = {}, *p* {}", df(d), num(chi, 2), p_clause(p));
    if let Some(fp) = r.number("fisher.p") {
        stats += &format!(" (Fisher's exact *p* {})", p_clause(fp));
    }
    Some(if p < 0.05 { format!("A median test compared {y} across the groups of {f} (pooled median = {}). The groups differed significantly in their share of cases above the median, {stats}.", num(r.number("median")?, 2)) } else { format!("A median test compared {y} across the groups of {f} (pooled median = {}). The groups did not differ significantly, {stats}.", num(r.number("median")?, 2)) })
}

fn ks2_paragraph(r: &ResultBundle) -> Option<String> {
    let (y, g1, g2) = (r.text("y.label")?, r.text("g1.label")?, r.text("g2.label")?);
    let (d, p) = (r.number("D")?, r.number("p")?);
    let stats = format!("*D* = {}, *p* {}", no_leading_zero(d, 2), p_clause(p));
    Some(if p < 0.05 { format!("A two-sample Kolmogorov-Smirnov test showed that the distribution of {y} differed significantly between {g1} and {g2}, {stats}.") } else { format!("A two-sample Kolmogorov-Smirnov test showed no significant difference in the distribution of {y} between {g1} and {g2}, {stats}.") })
}


/// Sentences for the risk, ordinal and layer options of Crosstabs.
fn crosstab_extras(r: &ResultBundle) -> String {
    let mut t = String::new();
    if let (Some(or), Some(lo), Some(hi)) = (r.number("risk.or"), r.number("risk.or.lower"), r.number("risk.or.upper")) {
        if or.is_finite() {
            t += &format!(" The odds ratio was {}, 95% CI [{}, {}].", num(or, 2), num(lo, 2), num(hi, 2));
        }
    }
    if let (Some(g), Some(gp)) = (r.number("ord.gamma"), r.number("ord.gamma.p")) {
        t += &format!(" Treating the categories as ordered, Goodman and Kruskal's γ = {}, *p* {}, and Kendall's τ<sub>b</sub> = {}.", no_leading_zero(g, 2), p_clause(gp), no_leading_zero(r.number("ord.tau_b").unwrap_or(f64::NAN), 2));
    }
    if let (Some(c), Some(cp), Some(or)) = (r.number("cmh.chi2"), r.number("cmh.p"), r.number("cmh.or")) {
        let ll = r.text("layer.label").unwrap_or("the layers");
        t += &format!(" Controlling for {ll}, the Cochran-Mantel-Haenszel test gave χ²(1) = {}, *p* {}, with a common odds ratio of {}, 95% CI [{}, {}]; the Breslow-Day test of equal odds ratios gave χ²({}) = {}, *p* {}.", num(c, 2), p_clause(cp), num(or, 2), num(r.number("cmh.or.lower").unwrap_or(f64::NAN), 2), num(r.number("cmh.or.upper").unwrap_or(f64::NAN), 2), df(r.number("bd.df").unwrap_or(f64::NAN)), num(r.number("bd.chi2").unwrap_or(f64::NAN), 2), p_clause(r.number("bd.p").unwrap_or(f64::NAN)));
    }
    t
}

fn cochran_paragraph(r: &ResultBundle) -> Option<String> {
    let k = r.number("k")? as usize;
    let yes = r.text("yes.label")?;
    let mut items = Vec::new();
    for i in 1..=k {
        items.push(format!("{} ({}%)", r.text(&format!("group.{i}.label"))?, num(100.0 * r.number(&format!("group.{i}.share"))?, 1)));
    }
    let (q, d, p, n) = (r.number("Q")?, r.number("df")?, r.number("p")?, r.number("n")? as usize);
    let stats = format!("*Q*({}) = {}, *p* {}", df(d), num(q, 2), p_clause(p));
    Some(format!("Cochran's Q test compared the share of the {n} cases answering {yes} across {} related measures: {}. {}", count_words(k), series(&items), if p < 0.05 { format!("The shares differed significantly, {stats}.") } else { format!("The shares did not differ significantly, {stats}.") }))
}

fn kappa_paragraph(r: &ResultBundle) -> Option<String> {
    let (a, b) = (r.text("r1.label")?, r.text("r2.label")?);
    let w = r.text("weights")?;
    let (k, p, n, lv) = (r.number("kappa")?, r.number("p")?, r.number("n")? as usize, r.number("level")?);
    Some(format!(
        "Agreement between {a} and {b} for the {n} cases rated by both was assessed with Cohen's kappa ({w}), κ = {}, {}, *z* = {}, *p* {}; the raters agreed on {}% of cases.",
        no_leading_zero(k, 2),
        ci(lv, r.number("ci.lower")?, r.number("ci.upper")?, 2),
        num(r.number("z")?, 2),
        p_clause(p),
        num(100.0 * r.number("agreement")?, 1)
    ))
}

fn fleiss_paragraph(r: &ResultBundle) -> Option<String> {
    let raters: Vec<String> = r.text("raters")?.split('\u{1f}').map(str::to_string).collect();
    Some(format!("Agreement among {} raters ({}) for {} cases was assessed with Fleiss' kappa, κ = {}, *z* = {}, *p* {}.", count_words(raters.len()), series(&raters), r.number("n")? as usize, no_leading_zero(r.number("kappa")?, 2), num(r.number("z")?, 2), p_clause(r.number("p")?)))
}

fn kripp_paragraph(r: &ResultBundle) -> Option<String> {
    let raters: Vec<String> = r.text("raters")?.split('\u{1f}').map(str::to_string).collect();
    Some(format!("Agreement among {} raters ({}) was assessed with Krippendorff's alpha for {} data, α = {}, based on {} units.", count_words(raters.len()), series(&raters), r.text("metric")?, no_leading_zero(r.number("alpha")?, 2), r.number("units")? as usize))
}

fn icc_paragraph(r: &ResultBundle) -> Option<String> {
    let raters: Vec<String> = r.text("raters")?.split('\u{1f}').map(str::to_string).collect();
    let (v, lv) = (r.number("icc")?, r.number("level")?);
    Some(format!(
        "Reliability across {} raters ({}) for {} subjects was assessed with an intraclass correlation, {}: ICC = {}, {}, *F*({}, {}) = {}, *p* {}.",
        count_words(raters.len()),
        series(&raters),
        r.number("n")? as usize,
        r.text("form")?,
        no_leading_zero(v, 2),
        ci(lv, r.number("ci.lower")?, r.number("ci.upper")?, 2),
        df(r.number("df1")?),
        df(r.number("df2")?),
        num(r.number("F")?, 2),
        p_clause(r.number("p")?)
    ))
}

fn explore_paragraph(r: &ResultBundle) -> Option<String> {
    let y = r.text("y.label")?;
    let k = r.number("k")? as usize;
    let mut parts = Vec::new();
    for i in 1..=k {
        let key = format!("group.{i}");
        let lab = r.text(&format!("{key}.label"))?;
        let mut s = format!("*n* = {}, *M* = {}, *SD* = {}, *Mdn* = {}", r.number(&format!("{key}.n"))? as usize, num(r.number(&format!("{key}.mean"))?, 2), num(r.number(&format!("{key}.sd"))?, 2), num(r.number(&format!("{key}.median"))?, 2));
        if let (Some(sk), Some(ku)) = (r.number(&format!("{key}.skew")), r.number(&format!("{key}.kurt"))) {
            s += &format!(", skewness = {}, kurtosis = {}", num(sk, 2), num(ku, 2));
        }
        if let (Some(w), Some(p)) = (r.number(&format!("{key}.sw.W")), r.number(&format!("{key}.sw.p"))) {
            s += &format!(", Shapiro-Wilk *W* = {}, *p* {}", no_leading_zero(w, 2), p_clause(p));
        }
        parts.push(if k == 1 { s } else { format!("{lab} ({s})") });
    }
    Some(if k == 1 { format!("For {y}, {}.", parts[0]) } else { format!("{} by group: {}.", sentence_case(y), parts.join("; ")) })
}
