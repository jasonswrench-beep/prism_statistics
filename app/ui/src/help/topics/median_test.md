# Median Test (k independent samples)

Tests whether two or more independent groups share the same median, by counting how many cases in each group fall above the overall median.

## When to use it
- Groups compared on an ordinal or skewed numeric outcome when you care about medians specifically.
- It is simple and robust to outliers, but less powerful than rank tests that use every value.
- **Kruskal-Wallis** (or **Mann-Whitney U** for two groups) is usually the stronger choice; for typical values with outliers, consider **Robust ANOVA (Trimmed Means)**.

## How to run it
Choose **Analyze › Nonparametric Tests › Median Test (k independent samples)**. Put outcomes in **Test Variables** (one result each) and the group variable in **Grouping Variable**.

## Options
This dialog has no options.

## What you get
- **Cases above and at or below the median**: a table of each group's count above the pooled median and at or below it. A group with most of its cases above the median tends to score higher.
- **Median test**: the chi-square statistic, df and p for that table. For two groups, Fisher's exact p value is given as well, which is the better guide when counts are small.

A small p value says the groups don't all share the same median. Cases are used when both the outcome and the group are valid.

## Checked against
R's `chisq.test()` (without the continuity correction) on the above/at-or-below table, and `fisher.test()` for two groups.
