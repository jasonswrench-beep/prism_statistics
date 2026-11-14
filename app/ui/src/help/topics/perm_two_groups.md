# Permutation Test: Two Groups

A permutation test of whether two independent groups differ in their means, with a p value from rearranging the data rather than from the t distribution.

## When to use it
- Two separate groups and a numeric outcome, when you don't want to rely on normality, especially with small samples.
- In a randomized experiment, it tests exactly what random assignment justifies: whether the group labels matter.
- With larger samples and roughly normal data, the **Independent-Samples T Test** gives nearly the same answer and a confidence interval. For ranks, use **Mann-Whitney U**.

## How to run it
Choose **Analyze › Permutation Tests › Two Groups**. Put the numeric outcome in **Outcome** and the group variable in **Grouping Variable**, then choose the two groups by code.

## Options
- **Random rearrangements**: default 10000. Used when there are too many rearrangements to list them all.
- **List every rearrangement up to**: default 100000. If the number of possible rearrangements is at most this, every one is listed and the p value is exact.
- **Seed**: default 20260101. The same seed gives the same p value.

## What you get
**Permutation test**: the **Difference in means** and the mean of each group, the two-sided p value, and the number of rearrangements used (all of them, or the random ones).

The p value is the share of rearrangements of cases between the groups whose difference is at least as far from zero as the one observed. When random rearrangements are used, p = (count + 1) / (B + 1), and a footnote gives its Monte Carlo standard error, which shrinks as you add rearrangements.

The test assumes only that, with no effect, every rearrangement of the data was as likely as the one observed (exchangeability). If the groups differ greatly in spread, that assumption is weaker.

## Checked against
R's `coin::oneway_test()` with the exact distribution.
