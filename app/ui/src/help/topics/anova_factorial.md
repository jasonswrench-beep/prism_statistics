# Factorial ANOVA (Type III)

Tests the effects of two or three categorical factors, and their interactions, on one numeric outcome. Add covariates for an ANCOVA.

## When to use it
- A design with crossed factors, such as condition by sex, where you want main effects and whether one factor's effect depends on another.
- With covariates, to compare groups while adjusting for a numeric variable such as a pretest score.
- For a single factor, **One-Way ANOVA** is simpler. For measures of the same people over time, use **Repeated-Measures ANOVA**.

## How to run it
Choose **Analyze › Compare Means › Factorial ANOVA (Type III)**. Put the outcome in **Dependent Variable**, two or three factors in **Fixed Factors**, and optionally numeric **Covariates**. With interactions, every combination of categories must have cases.

## Options
- **Include the interactions**: on by default; turn off for a main-effects model.
- **Sums of squares**: **Type III (default)**, each effect after all the others, or **Type II**, the usual choice for a main-effects model.
- **Estimated marginal means**: on by default.
- **Post hoc comparisons of main effects**: **None (default)**, **Tukey**, **Bonferroni** or **Holm**.
- **Simple effects**: each factor within each category of the other (two factors with the interaction); off by default.
- **Profile and residual plots**: off by default.
- **Confidence interval (%)**: default 95.

## What you get
- **Descriptive statistics** for each cell.
- **Tests of between-subjects effects**: F, df, p and partial eta squared for each factor, interaction and covariate. A significant interaction means the effect of one factor changes across the levels of another; interpret it before the main effects.
- An **Equal spread** check (Brown-Forsythe), and with covariates a **Homogeneity of slopes** check.
- **Estimated marginal means**: cell and factor means with covariates at their means and each cell weighted equally.
- Pairwise comparisons, simple effects and plots when asked.

## Checked against
R's `lm()` with sum-to-zero contrasts, `car::Anova()` and emmeans.
