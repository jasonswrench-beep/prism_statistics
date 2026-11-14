# MANOVA

A one-way multivariate analysis of variance: tests whether groups differ on two or more numeric outcomes considered together.

## When to use it
- Several related outcomes (for example, three subscales of one questionnaire) and one grouping factor.
- When you want one overall test before looking at each outcome, or when the groups may differ on a combination of outcomes that no single outcome shows.
- For a single outcome, use **One-Way ANOVA**. If the outcomes are unrelated, separate ANOVAs are easier to interpret.

## How to run it
Choose **Analyze › Compare Means › MANOVA**. Put two or more numeric outcomes in **Dependent variables** and the grouping variable in **Factor**.

## Options
- **Test reported**: **Pillai's trace (default)**, the most robust, **Wilks' lambda**, **Hotelling-Lawley trace**, or **Roy's largest root**.
- **Each outcome's ANOVA**: a one-way ANOVA for every outcome after the multivariate test; on by default.

## What you get
- **Descriptive statistics** for each outcome in each group.
- **Multivariate tests**: the chosen statistic with its F approximation, degrees of freedom and p. A small p value says the groups differ on the set of outcomes. Roy's root is an upper bound, so its p value is a lower bound and can look more significant than it is.
- **Equal covariance matrices**: Box's M test of whether the groups' outcomes vary and covary alike. It is very sensitive in large samples; Pillai's trace holds up well when it is significant and group sizes are similar.
- **Univariate tests of each outcome**, to see which outcomes carry the difference.

Cases are used when every outcome and the factor are valid.

## Checked against
R's `manova()` and `summary.manova()`, with Box's M written out in R.
