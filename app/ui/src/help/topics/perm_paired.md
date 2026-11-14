# Permutation Test: Paired Measures

A permutation test of whether two measures of the same cases differ on average, with a p value from flipping the signs of the differences instead of from the t distribution.

## When to use it
- The same people (or matched pairs) give both values, and you don't want to rely on the differences being normal, especially with few pairs.
- With more pairs and roughly normal differences, the **Paired-Samples T Test** gives nearly the same answer and a confidence interval. For a rank-based test, use **Wilcoxon Signed-Rank**.

## How to run it
Choose **Analyze › Permutation Tests › Paired Measures**. Put one measure in **First measure** and the other in **Second measure**. The difference is first minus second.

## Options
- **Random rearrangements**: default 10000. Used when there are too many to list them all.
- **List every rearrangement up to**: default 100000. With n pairs there are 2ⁿ sign flips; if that is within this limit, every one is listed and the p value is exact.
- **Seed**: default 20260101. The same seed gives the same p value.

## What you get
**Permutation test**: the **Mean difference**, the two-sided p value, and the number of rearrangements used.

If the two measures don't differ, each case's difference is as likely to be positive as negative. The test flips the signs in every possible way (or at random) and asks how often the mean difference is at least as far from zero as the observed one. When random rearrangements are used, the footnote gives the p value's Monte Carlo standard error.

The test needs at least three cases with both values; cases missing either measure are left out and counted.

## Checked against
R, by enumerating every sign flip of the differences.
