# Independent-Samples T Test

Compares the means of two separate groups, such as a treatment and a control group, on a numeric outcome.

## When to use it
- Two groups made of different people, and one numeric outcome per comparison.
- The default Welch test doesn't assume the groups have the same spread, so it is a safe first choice.
- If the outcome is badly skewed or has outliers, consider **Yuen's Test (Trimmed Means)** or **Mann-Whitney U**. For more than two groups, use **One-Way ANOVA**. If the same people are measured twice, use **Paired-Samples T Test**.

## How to run it
Choose **Analyze › Compare Means › Independent-Samples T Test** (or the toolbar button). Move outcomes into **Test Variables** (outcomes to compare; one result each) and the group variable into **Grouping Variable**. Then pick the two groups to compare by code from the two lists; each shows the label, code and number of cases.

## Options
- **Test**: **Welch, unequal variances** (default) or **Student, pooled variance**, which assumes equal spread.
- **Hypothesis**: **Two-sided** (default), **One-sided, first group higher**, or **One-sided, first group lower**.
- **Confidence interval**: **Level (%)**, default 95. A one-sided test gives a one-sided interval.
- **Bayes factor**: adds BF₁₀ with R's BayesFactor default prior; off by default.

## What you get
- **Group statistics**: N, mean, SD and standard error for each group.
- **Independent-samples t test**: t, df, p, the mean difference and its confidence interval. A small p value says the means differ; the interval shows by how much.
- **Effect size**: Hedges' g with the pooled SD, the difference in SD units. Judge its size against earlier studies in your field.
- Checks on **group sizes**, **unequal spread** and **shape and outliers**, so you know whether the result needs a second look.

Groups are chosen by code, never by label or position, so relabeling a group can't change which cases are compared.

## Checked against
R's `t.test()` (both the Welch and pooled versions) and BayesFactor's `ttestBF()`.
