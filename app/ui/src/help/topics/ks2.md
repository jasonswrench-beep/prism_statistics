# Kolmogorov-Smirnov (2 independent samples)

Tests whether two groups have the same distribution, by the largest distance between their cumulative distributions. Unlike the Mann-Whitney U test, it responds to differences in spread and shape as well as in location.

## When to use it
- Two separate groups and a numeric outcome, when the question is whether the distributions differ in any way, not only whether one group tends to be higher.
- It has less power than the **Mann-Whitney U** test for a plain shift in location, so use Mann-Whitney when that is the question. For means, use the **Independent-Samples T Test**.

## How to run it
Choose **Analyze › Nonparametric Tests › Kolmogorov-Smirnov (2 independent samples)**. Put outcomes in **Test Variables** (one result each) and the group variable in **Grouping Variable**, then choose the two groups to compare by code.

## Options
None: the test is two-sided.

## What you get
- **Kolmogorov-Smirnov test**: the groups, N, D (the largest distance between the two cumulative distributions, from 0 to 1) and p.
- The p value is exact when the product of the two group sizes is under 10,000, and otherwise from the limiting distribution; the column heading says which.
- A note when the smaller group is small.

## Checked against
R's `ks.test()`.
