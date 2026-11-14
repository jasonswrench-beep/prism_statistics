# Kruskal-Wallis (k independent samples)

A rank-based test of whether three or more independent groups differ, the usual nonparametric alternative to one-way ANOVA.

## When to use it
- One grouping factor and an outcome that is ordinal, or numeric but skewed or full of outliers.
- If the data are roughly normal with similar spread, **One-Way ANOVA** is more familiar. For a robust comparison of typical values, use **Robust ANOVA (Trimmed Means)**. For only two groups, use **Mann-Whitney U**. For a simpler test of medians, see the **Median Test**.

## How to run it
Choose **Analyze › Nonparametric Tests › Kruskal-Wallis (k independent samples)**. Put outcomes in **Dependent List** (one result each) and the grouping variable in **Factor**.

## Options
- **Post hoc comparisons**: **None (default)** or **Dunn's test**, which compares every pair of groups.
- **Adjust Dunn's p values**: **Holm (default)**, **Bonferroni** or **No adjustment**.

## What you get
- **Ranks**: N, median and mean rank for each group. Higher mean ranks mean higher values.
- **Kruskal-Wallis test**: the H statistic (compared with chi-square), df and p. A small p value says at least one group tends to have higher or lower values than the others. The test is corrected for ties.
- **Effect size**: epsilon squared, the share of rank variation explained by the groups.
- **Pairwise comparisons (Dunn)** when asked, with adjusted p values, to see which groups differ.

Holm's adjustment controls the chance of any false positive as well as Bonferroni does, and is never less powerful.

## Checked against
R's `kruskal.test()`.
