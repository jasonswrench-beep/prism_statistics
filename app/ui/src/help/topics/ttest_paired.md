# Paired-Samples T Test

Compares two measures taken on the same cases, such as scores before and after training, by testing whether their mean difference is zero.

## When to use it
- The same people (or matched pairs) give both values, and the outcome is numeric.
- If the differences are skewed or have outliers, try the **Paired Yuen Test** or **Wilcoxon Signed-Rank**.
- For three or more measures of the same people, use **Repeated-Measures ANOVA**.

## How to run it
Choose **Analyze › Compare Means › Paired-Samples T Test**. Add one or more pairs to **Paired Variables**: pick the first measure, then the second. The difference is first minus second, so swapping the pair flips the sign.

## Options
- **Hypothesis**: **Two-sided (default)**, **One-sided: the first is higher**, or **One-sided: the first is lower**.
- **Bayes factor**: BF₁₀ with R's BayesFactor default prior; off by default.
- **Confidence interval (%)**: default 95.

## What you get
- **Paired-samples statistics**: N, mean, SD and standard error of each measure.
- **Paired-samples correlation**: how closely the two measures track each other. A higher correlation is why a paired design is more sensitive than comparing separate groups.
- **Paired-samples t test**: the mean difference with its SD and confidence interval, t, df and p. If the interval excludes zero, the measures differ on average.
- **Effect size**: Cohen's d_z, the mean difference over the SD of the differences. It is named as d_z because it is not on the same scale as a two-group d.

A case is used only when both measures of the pair are valid; the others are counted and reported.

## Checked against
R's `t.test(x, y, paired = TRUE)` and BayesFactor's `ttestBF()`.
