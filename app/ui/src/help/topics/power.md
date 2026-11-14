# Power Analysis

Works out the sample size a planned study needs, or the power a planned sample size gives, for common tests. It needs no data.

## When to use it
- Before collecting data, to plan a study that can detect the effect you care about.
- Don't use it to compute "observed power" for a finished study; that number adds nothing beyond the p value.

## How to run it
Choose **Analyze › Power Analysis**. Pick the **Test**: **Independent-samples t test (two groups)** (default), **Paired-samples t test**, **One-sample t test**, **One-way ANOVA**, **Pearson correlation**, **Two proportions** or **One proportion**. Under **Find**, choose **The sample size needed** (default) or **The power of a sample size**.

## Options
- **Give the effect as**: in raw units (**A mean difference and SD**, **Group means and the SD within groups**, or **Proportions**) or as a standardized size (**Cohen's d**, **f** or **h**). Raw units are the default.
- The effect fields for the chosen test, such as **Difference between the group means** (default 5) and **SD within each group** (default 10), **Expected group means** (default 10, 12, 14), **Proportion in group 1** and **Proportion in group 2** (0.6 and 0.4), **Expected correlation (r)** (0.3), or a standardized size (0.5) with **Number of groups** (3) for ANOVA.
- **Alpha (two-sided)**: default 0.05.
- **Power wanted**: default 0.80, or **N per group**, **Number of pairs** or **N** (default 50) when finding power.

## What you get
- **Power analysis**: the sample size (per group and total) or the power, with the effect size it assumes.
- **Power at other sample sizes**: how power rises with N, to see the trade-off.
- Reminders that the answer is only as good as the assumed effect, and that you should recruit extra cases to allow for dropout.

Base the effect on earlier studies or the smallest effect that would matter, not on a label like "medium." Groups are assumed to be the same size.

## Checked against
The formulas of R's `pwr` package (noncentral t and F, Fisher's z, and Cohen's h).
