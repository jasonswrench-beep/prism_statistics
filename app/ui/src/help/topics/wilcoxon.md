# Wilcoxon Signed-Rank (2 related samples)

A rank-based test of whether two measures of the same cases differ, using the size and direction of each case's difference. It is the usual nonparametric alternative to the paired t test.

## When to use it
- The same people (or matched pairs) give both values, and the differences are skewed, have outliers, or come from an ordinal scale with meaningful differences.
- If the differences look roughly normal, the **Paired-Samples T Test** is more familiar. If only the direction of change can be trusted, use the **Sign Test**. For a robust test of typical values, try the **Paired Yuen Test**. For three or more measures, use **Friedman**.

## How to run it
Choose **Analyze › Nonparametric Tests › Wilcoxon Signed-Rank (2 related samples)**. Add one or more pairs to **Paired Variables**, choosing the first measure and then the second. The difference is first minus second.

## Options
- **Hypothesis**: **Two-sided (default)**, **One-sided: the first tends to be higher**, or **One-sided: the first tends to be lower**.

## What you get
- **Ranks**: how many differences are negative, positive and tied, with the mean rank and sum of ranks of the negative and positive ones.
- **Wilcoxon signed-rank test**: the statistic V (the sum of the positive ranks), z and p. Zero differences are dropped (Wilcoxon's method). The p value is exact below 50 pairs when there are no ties or zero differences; otherwise it uses the normal approximation with continuity and tie corrections.
- **Effect size**: the matched-pairs rank-biserial correlation, from −1 to +1. Positive values mean the first measure tends to be higher.

A case is used when both measures of the pair are valid.

## Checked against
R's `wilcox.test(x, y, paired = TRUE)`.
