# Permutation Test: Correlation

A permutation test of Pearson's correlation between two numeric variables, with a p value from shuffling one variable rather than from the t distribution.

## When to use it
- Small samples, or data where you don't want to rely on normality to test whether two variables are related.
- It tests the same r as usual; only the p value is found differently.
- For several variables, confidence intervals or rank correlations, use **Bivariate**.

## How to run it
Choose **Analyze › Permutation Tests › Correlation**. Put one numeric variable in **First variable** and the other in **Second variable**.

## Options
- **Random rearrangements**: default 10000. Used when there are too many to list them all.
- **List every rearrangement up to**: default 100000. With n cases there are n! orderings; if that is within this limit (about 9 cases or fewer), every one is listed and the p value is exact.
- **Seed**: default 20260101. The same seed gives the same p value.

## What you get
**Permutation test**: **Pearson r**, the two-sided p value, and the number of rearrangements used.

If the variables are unrelated, any pairing of their values is as likely as the observed one. The test shuffles one variable against the other and counts how often the shuffled r is at least as far from zero as the observed r. When random rearrangements are used, p = (count + 1) / (B + 1), and a footnote gives its Monte Carlo standard error.

Cases are used when both values are valid, and at least three are needed. Like any Pearson correlation, r measures straight-line association; a **Scatterplot** shows whether that fits.

## Checked against
R, by computing `cor()` over every ordering of one variable.
