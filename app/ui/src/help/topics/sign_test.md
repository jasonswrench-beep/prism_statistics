# Sign Test (2 related samples)

Tests whether two measures of the same cases differ by counting how many differences are positive and how many are negative, ignoring their size.

## When to use it
- Paired data where only the direction of change can be trusted, such as rough ratings, or where a few huge differences would dominate other tests.
- It assumes very little, which also makes it less powerful.
- If the size of the differences is meaningful, **Wilcoxon Signed-Rank** uses more information. If the differences are roughly normal, use the **Paired-Samples T Test**.

## How to run it
Choose **Analyze › Nonparametric Tests › Sign Test (2 related samples)**. Add one or more pairs to **Paired Variables**, choosing the first measure and then the second. The difference is first minus second.

## Options
This dialog has no options. The test is exact.

## What you get
**Sign test**: one row per pair with
- **Positive differences** and **Negative differences**: how many cases scored higher on the first and on the second measure.
- **Ties**: cases with no difference, which are set aside.
- **Median difference**: the typical size and direction of change.
- **p (exact, two-sided)**: from the binomial distribution with a probability of one half. A small p value says one measure tends to be higher than the other more often than chance would produce.

A case is used when both measures in the pair are valid.

## Checked against
R's `binom.test()` on the counts of positive and negative differences.
