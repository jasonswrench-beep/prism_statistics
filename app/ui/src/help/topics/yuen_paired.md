# Paired Yuen Test

Compares two measures of the same cases on their trimmed means, a robust alternative to the paired t test.

## When to use it
- The same people (or matched pairs) give both values, and the scores have outliers or long tails.
- If the differences look roughly normal, the **Paired-Samples T Test** is simpler to report. For a rank-based alternative, use **Wilcoxon Signed-Rank**.

## How to run it
Choose **Analyze › Compare Means › Paired Yuen Test**. Add one or more pairs to **Paired Variables**, choosing the first measure and then the second. The difference is first minus second.

## Options
- **Proportion trimmed from each end**: default 0.2 (20% from each tail), the usual choice.
- **Confidence interval (%)**: default 95.

## What you get
- **Paired Yuen test**: one row per pair with the trimmed mean of each measure, their difference, its SE, t, df, p and the confidence interval of the difference. A small p value says the typical scores differ between the two measures.

A case is used when both measures in the pair are valid. Trimming keeps a handful of extreme cases from deciding the result, while the paired design still removes the stable differences between people.

## Checked against
R's `WRS2::yuend()`.
