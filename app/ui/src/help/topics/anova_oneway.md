# One-Way ANOVA

Compares the means of three or more independent groups on a numeric outcome, with optional post hoc comparisons and planned contrasts.

## When to use it
- One categorical factor (such as condition) and one numeric outcome.
- If the groups' spreads differ, choose **Welch's F** and Games-Howell comparisons. If the outcome is skewed or has outliers, consider **Robust ANOVA (Trimmed Means)** or **Kruskal-Wallis**.
- With two or three factors at once, use **Factorial ANOVA (Type III)**.

## How to run it
Choose **Analyze › Compare Means › One-Way ANOVA**. Put numeric outcomes in **Dependent List** (one result each) and the grouping variable in **Factor**. Every category with data is a group, in code order.

## Options
- **Test**: **Conventional F (default)**, with Welch's F shown beside it, or **Welch's F**.
- **Post hoc comparisons**: **None (default)**, **Tukey HSD**, **Games-Howell**, **Bonferroni**, **Holm** or **Scheffé**.
- **Planned contrasts**: optional coefficients, one per group in code order and adding to zero, separated by semicolons (for example 1 -0.5 -0.5; 0 1 -1).
- **Trend contrasts (linear, quadratic, cubic)**: treats the groups as equally spaced; off by default.
- **Adjust the contrasts' p values**: **No (default)**, **Bonferroni** or **Holm**.
- **Means and residual plots**: off by default.
- **Bayes factor**: off by default.
- **Confidence interval (%)**: default 95.

## What you get
- **Descriptive statistics** for each group.
- **Analysis of variance**: F, df and p. A small p value says at least one group mean differs.
- **Effect size**: omega squared (less biased) and eta squared, the share of variance explained by group.
- An **Unequal spread** check using Levene's test (Brown-Forsythe version). The test is never switched for you.
- **Multiple comparisons** and **Planned contrasts** when asked, plus optional means and residual plots.

## Checked against
R's `aov()`, `oneway.test()`, `TukeyHSD()`, `pairwise.t.test()` and emmeans contrasts.
