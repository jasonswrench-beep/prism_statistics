# Robust ANOVA (Trimmed Means)

Compares three or more independent groups on their trimmed means, with a test that assumes neither equal variances nor normality.

## When to use it
- One categorical factor and a numeric outcome with outliers, skew or unequal spread across groups.
- If the data look reasonably normal with similar spread, **One-Way ANOVA** is simpler. For a rank-based alternative, use **Kruskal-Wallis**.
- For only two groups, use **Yuen's Test (Trimmed Means)**.

## How to run it
Choose **Analyze › Compare Means › Robust ANOVA (Trimmed Means)**. Put numeric outcomes in **Dependent List** (one result each) and the grouping variable in **Factor**.

## Options
- **Proportion trimmed from each end**: default 0.2 (20% from each tail), the usual choice.

## What you get
- **Trimmed means**: N, the trimmed mean and the winsorized SD for each group.
- **Robust ANOVA**: the F statistic for trimmed means, with its two degrees of freedom (df1, df2) and p. A small p value says at least one group's typical score differs from the others.

This is Wilcox's heteroscedastic one-way ANOVA for trimmed means. It doesn't include post hoc comparisons; to follow up a significant result, compare pairs of groups with **Yuen's Test (Trimmed Means)** and adjust for the number of comparisons.

## Checked against
R's `WRS2::t1way()`.
