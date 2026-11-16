# Version 1 Procedure Scope and Statistical Defaults (T0-04, T0-05)

Draft 1 · November 10, 2026 · Spec §3, §10 (STAT-01–04) · Decisions D01, D08

Once approved, this list is **frozen** for 1.0. Anything added later needs a scope change that names the user task, the affected contracts, the validation cost, the dependencies and the release impact (spec §3).

## 1. Method catalog

The method ID is stable forever. The version goes up whenever computation or defaults change (STAT-04).

| Method ID | Display name | Required roles | Key options (default **bold**) | Must handle explicitly | Independent reference (planned) | Task |
|---|---|---|---|---|---|---|
| `desc.frequencies` | Frequencies | variables (any) | percent base **valid** / total; show missing **on** | All-missing; very many categories; empty categories | Hand-computed fixtures | T3-07 |
| `desc.descriptives` | Descriptives | variables (scale) | stats **N, mean, SD, min, max**; quantile type **7**; mean CI **t-based 95%** | Constant; single case; huge magnitudes (use stable algorithms) | NIST StRD univariate; R | T3-07 |
| `desc.crosstabs` | Crosstabs | row, column (categorical) | cell %: **row** | Empty rows/cols; structural zeros | Hand fixtures | T3-07 |
| `ttest.one_sample` | One-Sample T Test | test var; reference value | CI **95%**; tails **two** | n < 2; zero variance | R `t.test` | T3-08 |
| `ttest.paired` | Paired-Samples T Test | ordered pair(s) | effect size **d_z** (named) | Unmatched pairs dropped and counted; swapping order flips the sign | R `t.test(paired=TRUE)` | T3-08 |
| `ttest.independent` | Independent-Samples T Test | test var; grouping var + 2 groups | estimator **Welch** / Student; effect size **Hedges' g, pooled SD** (named) | >2 categories (QA04); tiny groups; zero variance; missing group codes | R `t.test`; hand fixtures | T2-02 |
| `anova.oneway` | One-Way ANOVA | outcome; factor | **conventional** / Welch; post hoc **none** / Tukey (conventional) / Games-Howell (Welch); effect **ω²** and η² | Single-member groups; empty groups | R `aov`, `oneway.test`; NIST StRD ANOVA | T3-09 |
| `anova.factorial` | Factorial ANOVA (two or three factors) | outcome; 2–3 factors | every interaction **on**; **Type III SS** with **sum-to-zero** contrasts (§3 row 8); effect **partial η²**; Brown-Forsythe test of equal spread | Empty cells → refuse with the cells named; unbalanced | R `lm` with sum contrasts, terms dropped by hand (D16) | T3-10 |
| `cat.chisq_gof` | Chi-Square Goodness of Fit | variable; expected proportions **equal** | — | Expected < 5 warning; zero expected | R `chisq.test` | T3-11 |
| `cat.chisq_indep` | Chi-Square Test of Independence | row; column | continuity **off**; effect **Cramér's V** | Small expected counts flagged; no auto-merging | R `chisq.test(correct=FALSE)` | T3-11 |
| `cat.fisher_2x2` | Fisher's Exact Test (2×2) | row; column (2 levels each) | two-sided **sum of P ≤ observed**; conditional ML odds ratio with exact interval. Built as an option of `desc.crosstabs` (D16) | Not 2×2 → stated in the method note | R `fisher.test` | T3-11 |
| `cat.mcnemar` | McNemar Test (built as `np.mcnemar`) | paired binary vars | continuity-corrected χ² and exact p both shown; the paragraph reports the **exact** p below 25 changed cases (stated) | Zero discordant pairs; more than two categories | R `mcnemar.test`, `binom.test` | T3-11 |
| `np.mann_whitney` | Mann-Whitney U | test var; grouping + 2 groups | exact when **n small and no ties**, else asymptotic with tie correction; effect **rank-biserial r** | Ties; all ties | R `wilcox.test`; hand | T3-12 |
| `np.wilcoxon` | Wilcoxon Signed-Rank | ordered pair | zero differences **Wilcoxon (drop)** / Pratt | All-zero differences | R `wilcox.test`, `coin` | T3-12 |
| `np.kruskal` | Kruskal-Wallis | outcome; factor | post hoc **none** / Dunn + Holm | Ties | R `kruskal.test` | T3-12 |
| `np.friedman` | Friedman | ≥2 repeated vars (usually 3 or more) | tie correction; effect **Kendall's W** | Incomplete blocks dropped and counted | R `friedman.test` | T3-12 |
| `corr.bivariate` | Bivariate Correlation | ≥2 vars | **Pearson** / Spearman / Kendall τ-b; missing **listwise** / pairwise; CI **Fisher z** (Pearson) | Constant var → undefined with reason | R `cor.test` | T3-13 |
| `corr.partial` | Partial Correlation | vars; controls | listwise; zero-order r **shown**; CI **Fisher z** | Singular control matrix → refuse | R (residual correlations, by hand) | T3-13 |
| `corr.canonical` | Canonical Correlation | set 1; set 2 (numeric) | standardized coefficients; Wilks' λ with **Rao's F**; structure coefficients; sign so set 1's structure sums positive | Collinear set → refuse; too few cases | R `cancor`; Wilks by hand (D16) | D16 |
| `scale.item_analysis` | Test Scoring and Item Analysis | items (≥2), a key (or points earned) | difficulty; **point-biserial (total includes the question)** and **corrected item-total r (total leaves the item out)**; upper and lower 27% (⌊(27n + 50)/100⌋, ties by case order) and D; options by score group; **KR-20** (alpha with partial credit), **KR-21** (equal points), SEM | Blanks score 0 and are counted; an all-blank case is left out; fewer than 30 cases flagged unstable; negative or low corrected r, upside-down distractors and unused options flagged | R: psych `score.multiple.choice`, base R; the shared C3 test set | Development list item 2 |
| `scale.reliability` | Reliability Analysis | items (≥2) | **alpha** with **Feldt's** interval; standardized alpha; item-rest r; alpha if deleted; optional reverse-scoring (stated) | Constant items; negative item-rest r flagged, never reversed automatically | R, by hand (`psych::alpha` not installed) | T3-14 |
| `reg.linear` | Linear Regression | outcome; predictors; blocks | contrasts **treatment, ref shown**; CI **95%** | Rank deficiency → named; no stepwise | NIST StRD Longley, Norris, Wampler; R `lm` | T3-15 |
| `reg.logistic` | Binary Logistic Regression | outcome + event category; predictors | CI **profile likelihood** / Wald (stated); threshold **0.5** (shown) | Separation detected → inference suppressed; non-convergence | R `glm`; hand | T3-16 |
| `power.*` | Power and Sample Size (built as `power`) | — (planning) | alpha **0.05**; power **0.80**; tails **two**; equal groups | Effect entered as raw + SD, not just "medium" | R `pwr` formulas in base R (noncentral t and F); Cohen (1988) values 34, 64, 53, 85 reproduced | T3-17 |

**Power covers** one-sample, paired and independent t; one-way ANOVA; Pearson r; one and two proportions. There's no "observed power" for completed tests.

## 2. Explicitly out of 1.0

Repeated or mixed ANOVA, ANCOVA, MANOVA, robust or clustered SEs, GLMs beyond binary logistic, mixed models, **omega** (proposed move to Stage 6, see D01), PCA/EFA/CFA, Bayesian, survival, time series beyond basic time plots, multiple imputation, survey design, non-frequency weights, Fisher beyond 2×2, stepwise selection. (Omega moved to Stage 6 by product owner decision, 2026-11-10.)

If guidance meets one of these designs, it says so and offers export. It **never** runs an independent-observations analysis in its place (spec §3).

## 3. Statistical defaults (D08)

Each line is checked by reference runs (R and the other reference packages the product owner has) and signed by the product owner (D08, 2026-11-11).

| # | Setting | Proposed default | Disclosure in output | Signed |
|---|---|---|---|---|
| 1 | Confidence level | 95%, editable per analysis | Printed with every interval | ☐ |
| 2 | Test direction | Two-sided | One-sided needs an explicit choice and direction, which is printed | ☐ |
| 3 | Independent t | Welch | The method note names it; the Student alternative is one click away; never chosen from a Levene p value | ☐ |
| 4 | Missing data | Complete cases per procedure (listwise) | Total, eligible and analyzed N plus exclusion reasons | ☐ |
| 5 | Pairwise deletion | Only where the procedure supports it; per-pair N shown | Per-pair N table | ☐ |
| 6 | Reference category | Chosen in the dialog; stored by **category code** | Named in the coefficient table | ☐ |
| 7 | Contrast coding | Treatment (dummy) for regression; sum-to-zero inside ANOVA when Type III is used | Method note | ☐ |
| 8 | **ANOVA sums of squares** | Additive model: Type II. Interaction model: **Type III with sum-to-zero contrasts** (product owner preference, 2026-11-10; matches JASP, PSPP and the usual menu-driven default). Type II one click away. | Label always printed: "Type II SS" / "Type III SS (sum-to-zero contrasts)" | ☐ |
| 9 | Multiplicity | None unless a family is defined; Holm offered | Raw and adjusted p in separate columns | ☐ |
| 10 | p formatting | 3 decimals; "< .001" below; never "0.000" | — | ☐ |
| 11 | Effect sizes | Always named with their formula (e.g. "Hedges' g, pooled SD") | Glossary link | ☐ |
| 12 | Effect benchmarks | Off; optional named reference (e.g. Cohen 1988) | Source shown | ☐ |
| 13 | Quantiles | Hyndman-Fan type 7 (configurable) | Stated in table footnote | ☐ |
| 14 | SD / variance | Sample (n − 1) | Footnote | ☐ |
| 15 | Seeds | Recorded whenever randomness is used | Generator, version, seed, count | ☐ |
| 16 | Numerical failure | Visible status and diagnostic; no interpretation text | Status banner | ☐ |
| 17 | Chi-square continuity | Off | Stated when on | ☐ |
| 18 | Logistic CI | Profile likelihood (proposed) | Method named | ☐ |

**#8 (settled direction, pending reference check):** the product owner chose Type III for interaction models on 2026-11-10. Original reasoning: Option B (Type III with sum-to-zero contrasts) for interaction models. The output would print "Type III SS (sum-to-zero contrasts)", the method note would explain the marginality debate, and Type II would be one click away. Most migrating users come from menu-driven packages, JASP and PSPP, so matching them avoids the most common "your numbers are wrong" report. The spec leaves this to a reviewer; under D08 as decided on 2026-11-11, the reference runs and the product owner's signature settle it.

## 4. Naming conventions (T0-05)

- **Display names** use the familiar familiar textbook name ("Independent-Samples T Test"), with the precise name in the method note ("Welch's unequal-variances t test").
- **Effect sizes** always name the convention, never a bare "d".
- **Status words** are exactly these four: *No concern detected*, *Review needed*, *Unsupported condition*, *Not assessed* (GUIDE-02). No green ticks implying an assumption is "met".
- **Availability labels:** *Available*, *Coming in a later release*, *Not planned*.

## 5. Method contract checklist

A method enters 1.0 only when every row is complete (spec §3, §20):

- [ ] Registry descriptor (STAT-01): ID, aliases, roles, types, options and defaults, missing and weight policy, result schema, citations
- [ ] Algorithm note with references and edge-case behavior
- [ ] Computation, plus independent reference fixtures within tolerance (VAL-01–03)
- [ ] Diagnostics, with states per GUIDE-02
- [ ] Interpretation template, bound to result fields (GUIDE-07)
- [ ] Output tables and export (DOCX/HTML) with N and missing note
- [ ] Replay produces an equivalent result (QA01)
- [ ] Keyboard-complete dialog
- [ ] Migration-default note (§3 of the crosswalk)
- [ ] Reference-package check recorded and product owner sign-off
