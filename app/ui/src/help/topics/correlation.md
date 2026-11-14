# Bivariate Correlation

Bivariate correlations measure how strongly pairs of numeric variables move together, with a test and a confidence interval for each pair.

## When to use it
- To describe linear relationships between numeric variables (Pearson), or monotonic relationships with ranks (Spearman or Kendall's tau-b), which suit ordinal data and outliers.
- To remove the influence of other variables, use **Partial**. To relate two sets of variables, use **Canonical**. To check one correlation without distributional assumptions, use the permutation **Correlation** test.

## How to run it
Choose **Analyze › Correlate › Bivariate**. Move at least two numeric variables into **Variables**. Every pair is correlated.

## Options
- **Coefficients**: **Pearson** (default), **Spearman**, **Kendall's tau-b**; at least one.
- **Missing values**: **Exclude cases listwise (default)**, so every coefficient uses the same cases, or **Exclude cases pairwise**, so each pair uses every case it can (N is shown for each).
- **Hypothesis**: **Two-sided (default)**, **One-sided: a positive correlation**, or **One-sided: a negative correlation**.
- **Adjust p values for the number of pairs**: **No (default)**, **Bonferroni** or **Holm**.
- **Bayes factor**: off by default.
- **Confidence interval (%)**: default 95.

## What you get
- **Correlation matrix**: each coefficient, from −1 to +1. The sign gives the direction; values near zero mean little relationship.
- **Pairs**: each pair's coefficient, N, p and, for Pearson's r, a Fisher-z confidence interval.
- **Bayes factors** when asked.

A constant variable gives an undefined correlation with the reason, never a number. A correlation shows association, not cause, and Pearson's r can miss curved relationships, so a **Scatterplot** is worth a look.

## Checked against
R's `cor.test()` (Pearson, Spearman and Kendall) and BayesFactor's default prior.
