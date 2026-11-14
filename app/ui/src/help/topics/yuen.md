# Yuen’s Test (Trimmed Means)

Compares two independent groups on their trimmed means, a robust alternative to the t test that resists outliers and skew.

## When to use it
- Two separate groups and a numeric outcome with outliers, long tails or unequal spread.
- Trimmed means describe the typical score in each group without letting a few extreme cases dominate.
- If your data are close to normal, the **Independent-Samples T Test** (Welch) is simpler to report. For more than two groups, use **Robust ANOVA (Trimmed Means)**.

## How to run it
Choose **Analyze › Compare Means › Yuen’s Test (Trimmed Means)**. Put outcomes in **Test Variables** (one result each) and the group variable in **Grouping Variable**, then choose the two groups to compare by code.

## Options
- **Proportion trimmed from each end**: default 0.2, which trims 20% of the values from each tail. This is Wilcox's usual recommendation.
- **Confidence interval (%)**: default 95.

## What you get
- **Trimmed means**: for each group, N, the trimmed mean and the winsorized SD (the spread after pulling the extreme values in).
- **Yuen's test**: the difference in trimmed means, its SE, t, the Welch-type degrees of freedom, p and the confidence interval of the difference. A small p value says the typical scores differ; the interval shows by how much.

Each test variable uses its own valid cases. With 20% trimming the test keeps good power when the data are normal and is much better behaved when they are not.

## Checked against
R's `WRS2::yuen()`.
