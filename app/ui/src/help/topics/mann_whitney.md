# Mann-Whitney U (2 independent samples)

A rank-based test of whether values in one group tend to be higher than in another. It is the usual nonparametric alternative to the independent-samples t test.

## When to use it
- Two separate groups, and an outcome that is ordinal (such as ratings) or numeric but skewed or full of outliers.
- It compares whole distributions through ranks; it is not strictly a test of medians unless the groups have the same shape.
- If the data are roughly normal, the **Independent-Samples T Test** is more familiar and usually more powerful. For a robust test of typical values, try **Yuen's Test (Trimmed Means)**. For three or more groups, use **Kruskal-Wallis**.

## How to run it
Choose **Analyze › Nonparametric Tests › Mann-Whitney U (2 independent samples)**. Put outcomes in **Test Variables** (one result each) and the group variable in **Grouping Variable**, then choose the two groups to compare by code.

## Options
- **Hypothesis**: **Two-sided (default)**, **One-sided: the first group tends to be higher**, or **One-sided: the first group tends to be lower**.

## What you get
- **Ranks**: N, median, mean rank and sum of ranks in each group. The group with the higher mean rank tends to have higher values.
- **Mann-Whitney U test**: U, z and p, with the method used for p. The p value is exact when both groups have fewer than 50 values and there are no ties; otherwise it uses the normal approximation with continuity and tie corrections.
- **Effect size**: the rank-biserial correlation, from −1 to +1, which is the difference between the share of pairs where the first group is higher and the share where it is lower.

Groups are chosen by code, so relabeling can't change which cases are compared.

## Checked against
R's `wilcox.test()`.
