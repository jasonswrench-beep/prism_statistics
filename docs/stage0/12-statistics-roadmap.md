# What Is Left to Build Statistically

Draft 1 · November 12, 2026 · Decisions D01, D08, D16 · Product owner request: "I would rather have all of the statistic elements built," including structural equation modeling.

This list covers every statistical feature not yet built, from gaps in the 1.0 catalog (05 §1) through the research (Stage 6) and specialist (Stage 7) modules in the checklist, plus structural equation modeling. Each item is checked against R the way the current analyses are (and against the product owner's other reference packages), with a dialog, APA template text, checks, an MCP tool and an R export.

**Already built** (for orientation): frequencies, descriptives, crosstabs with chi-square and Fisher (2 × 2), chi-square goodness of fit; one-sample, paired and independent t tests; one-way ANOVA with Welch, Tukey and Games-Howell; factorial ANOVA (Type III); Pearson, Spearman, Kendall, partial and canonical correlation; Mann-Whitney, Wilcoxon, Kruskal-Wallis with Dunn, Friedman, McNemar, sign, median, runs, binomial and two-sample Kolmogorov-Smirnov tests; linear and binary logistic regression; Cronbach's alpha; power analysis; five chart types.

## Part A. Finishing the 1.0 catalog

These are promised in 05 §1 or the checklist and are the shortest path to a complete teaching release.

| # | Feature | What it adds | Reference |
|---|---|---|---|
| A1 | Chi-square goodness of fit (**built 2026-11-12**) | Equal or given proportions, expected counts, effect size *w* | `chisq.test(p =)` |
| A2 | More nonparametric tests | **Built 2026-11-12:** binomial, sign, runs, median test, two-sample Kolmogorov-Smirnov. Still to do: Cochran's Q, and the one-sample Kolmogorov-Smirnov test (with Lilliefors' correction, beside Shapiro-Wilk in Explore) | base R, `DescTools` |
| A3 | Crosstabs measures | Relative risk and odds ratio with intervals; gamma, tau-b, tau-c, Somers' *d* for ordered categories; Cochran-Mantel-Haenszel by a layer variable | base R, `DescTools` |
| A4 | Agreement | Cohen's kappa (weighted too), Fleiss' kappa, Krippendorff's alpha, intraclass correlations (the stats crate has kappa and alpha) | `irr`, `psych::ICC` |
| A5 | Explore | Descriptives by group, percentiles, skewness and kurtosis with SEs, Shapiro-Wilk (in the crate), boxplots and Q-Q plots per group | base R |
| A6 | Linear regression, full | Hierarchical blocks with *R*² change, interaction terms (with optional centering), residual statistics, residual and influence plots, saving predicted values and residuals as new variables | `lm`, `anova()` |
| A7 | Logistic regression, full | Blocks with likelihood-ratio change, interactions, ROC curve with AUC interval | `glm`, `pROC` |
| A8 | Factorial ANOVA, full | Estimated marginal means with intervals, simple effects, post hoc tests on main effects, Type II for additive models (05 §3 row 8), profile plots, ANCOVA covariates | `emmeans`, `car::Anova` |
| A9 | Contrasts and multiplicity | Planned contrasts for one-way ANOVA; Bonferroni and Holm options wherever several tests run | `multcomp` |
| A10 | One-sided tests | Explicit direction choice for t tests, correlations and rank tests (05 §3 row 2) | base R |
| A11 | Diagnostic plots in analyses | Residual, Q-Q and leverage plots attached to regression and ANOVA results | base R |
| A12 | Curve estimation | Linear, quadratic, cubic, logarithmic, exponential and power fits (in the crate) | `lm`, `nls` |
| A13 | Bootstrap intervals | Percentile and BCa intervals as an option for means, medians, correlations and regression coefficients (the crate has the engine) | `boot` |

**Data features that the statistics depend on** (Transform and Data menus): Compute and Recode, scale scores with reverse scoring and a minimum of answered items, Select Cases, Split File (run any analysis by group) and frequency weights. Without these, students can't build the scale totals most analyses need.

## Part B. Research methods (Stage 6)

| # | Feature | What it adds | Reference |
|---|---|---|---|
| B1 | Repeated-measures ANOVA | One-way within-subjects and mixed (between × within) designs; Mauchly's test with Greenhouse-Geisser and Huynh-Feldt corrections; generalized eta squared; pairwise comparisons | `afex`, `emmeans` |
| B2 | ANCOVA and MANOVA | Covariates with homogeneity-of-slopes check; Pillai, Wilks, Hotelling and Roy tests with follow-ups; Box's M | `car`, base `manova` |
| B3 | Factor analysis | PCA and EFA (maximum likelihood, principal axis, minimum residual), varimax, oblimin and promax rotations, parallel analysis and scree plot, KMO and Bartlett's test; then McDonald's omega (moved here by D01) | `psych` |
| B4 | Mediation and moderation | Simple and parallel mediation with bootstrapped indirect effects; moderation with simple slopes and Johnson-Neyman regions; conditional process models | `lavaan`, `interactions` |
| B5 | Generalized linear models | Ordinal logistic, multinomial logistic, Poisson and negative binomial regression | `MASS`, `nnet` |
| B6 | Robust options | Heteroscedasticity-consistent standard errors (HC3), robust t tests (Yuen's trimmed means), robust ANOVA | `sandwich`, `WRS2` |
| B7 | Linear mixed models | Random intercepts and slopes, REML or ML, Satterthwaite degrees of freedom, ICC, singular-fit warnings | `lme4`, `lmerTest` |
| B8 | Missing data | Little's MCAR test, multiple imputation with pooled results (Rubin's rules), and full-information maximum likelihood in SEM (C1) | `mice`, `naniar` |
| B9 | Bayesian core | Bayes factors for t tests, correlation, contingency tables, ANOVA and regression, with prior sensitivity | `BayesFactor` |
| B10 | Resampling | Permutation tests for the core comparisons | `coin` |
| B11 | Survival and ROC | Kaplan-Meier with log-rank, Cox regression; ROC analysis outside logistic regression | `survival`, `pROC` |
| B12 | Time series basics | Autocorrelation, smoothing, ARIMA | base `stats` |
| B13 | Survey design | Weights, strata and clusters with design-based standard errors | `survey` |

## Part C. Structural equation modeling (new, to be recorded as D17)

A visual tool for drawing and estimating models, with the model also editable as text. Two estimation families, as the leading tools offer:

**C1. Covariance-based SEM**
- **Models:** path analysis, confirmatory factor analysis, full latent-variable models, higher-order factors, multigroup models.
- **Estimators:** maximum likelihood; robust ML with Satorra-Bentler or Yuan-Bentler corrections; WLSMV for ordinal indicators; FIML for missing data.
- **Fit:** χ², CFI, TLI, RMSEA with its 90% interval, and SRMR.
- **Estimates:** unstandardized and standardized estimates, *R*² for each endogenous variable, and modification indices.
- **Indirect effects:** bootstrapped confidence intervals, plus user-defined effects ("a*b").
- **Measurement invariance:** configural, metric, scalar and strict.
- **Reliability and validity:** composite reliability, AVE and HTMT.

**C2. Partial least squares SEM (PLS-SEM)**
- **Measurement models:** reflective and formative.
- **Estimation:** the path-weighting scheme with bootstrapped significance.
- **Fit:** *R*², *f*² and *Q*², plus SRMR for PLS.
- **Reliability and validity:** reliability (rho_A, composite reliability), AVE, HTMT and outer weights and loadings.
- **Group comparisons:** multigroup analysis.

**C3. The canvas**
- **Drawing:** drag variables onto a canvas; draw latent variables as ovals and observed ones as rectangles; connect them with arrows (paths) and double-headed arrows (covariances).
- **Layout and labels:** automatic layout, fixed and labeled parameters, and constraints across groups.
- **Two-way text editing:** the diagram and a text model (the open lavaan model syntax, so the model also runs in R) edit each other.
- **Estimates on the diagram:** estimates appear on the drawing, and a click on a path shows its table row.
- **APA output:** the diagram exports as an APA figure (SVG with alt text), alongside fit, loading and path tables in the APA style already used for tables.
- **Accessibility:** a keyboard-only way to build the model (the text model, and a list of paths), so the canvas isn't the only way in.

**C4. Validation**
- **Open-source references:** lavaan for covariance-based SEM and seminr for PLS-SEM.
- **Product owner's packages:** the product owner's reference packages, on a shared set of published example models.
- **Scope:** every estimator and fit index is checked.

**Later (Stage 7):** latent growth models, multilevel SEM, latent class and mixture models, item response theory (Rasch, 2PL, graded response), and Bayesian SEM.

## Part D. Specialist (Stage 7, after the above)

- Meta-analysis (fixed and random effects, forest and funnel plots, moderators).
- Generalized estimating equations and panel models.
- Predictive modeling with cross-validation.
- Statistical process control and designed experiments.

## Suggested order

1. **Part A and the data features**, to complete 1.0 for spring classes.
2. **B1 to B4** (repeated measures, ANCOVA/MANOVA, factor analysis with omega, mediation and moderation). These are the methods students and theses use most after the basics.
3. **C1 and C3** (covariance-based SEM with the canvas). CFA from B3 leads straight into it.
4. **C2** (PLS-SEM), then B5 to B13, then Part D and the Stage 7 SEM extensions.

## What validation needs

The current tests use only base R. Most items above are checked against CRAN packages: lavaan, seminr, psych, car, emmeans, afex, lme4, lmerTest, mice, BayesFactor, survival, pROC, irr, DescTools, WRS2, sandwich, survey, coin and boot. They were installed from CRAN on 2026-11-12 with the product owner's go-ahead (lavaan 0.7.3, seminr 2.6.0 and the rest, into R 4.6.1's user library). The product owner's own reference packages give the second check (05 §3).
