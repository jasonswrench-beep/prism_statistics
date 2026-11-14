# Permutation Test: One-Way ANOVA

A permutation version of one-way ANOVA: tests whether three or more groups differ in their means, with a p value from reassigning cases to groups rather than from the F distribution.

## When to use it
- One grouping factor and a numeric outcome, when you don't want to rely on normality, especially with small groups.
- It uses the familiar F statistic; only the p value is found differently.
- With larger samples and roughly normal data, the regular **One-Way ANOVA** under Compare Means gives nearly the same p value plus effect sizes and post hoc tests. For a rank-based test, use **Kruskal-Wallis**.

## How to run it
Choose **Analyze › Permutation Tests › One-Way ANOVA**. Put the numeric outcome in **Outcome** and the grouping variable in **Factor**.

## Options
- **Random rearrangements**: default 10000. Used when there are too many to list them all.
- **List every rearrangement up to**: default 100000. If the number of possible reassignments is within this limit, every one is listed and the p value is exact.
- **Seed**: default 20260101. The same seed gives the same p value.

## What you get
**Permutation test**: the **F** statistic, the p value, and the number of rearrangements used.

If group membership doesn't matter, any reassignment of the cases to groups of the same sizes is as likely as the observed one. The test counts how often the reassigned F is at least as large as the observed F. When random rearrangements are used, p = (count + 1) / (B + 1), and a footnote gives its Monte Carlo standard error, which shrinks with more rearrangements.

The test assumes the cases are exchangeable under no effect; very unequal spread across groups weakens that assumption. It doesn't say which groups differ.

## Checked against
R, with the F statistic of `aov()` computed over the rearrangements.
