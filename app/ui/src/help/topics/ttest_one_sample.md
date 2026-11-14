# One-Sample T Test

Tests whether the mean of a numeric variable differs from a fixed value you choose, such as a scale midpoint or a published norm.

## When to use it
- You have one group and a meaningful comparison value: "Is our average satisfaction different from the neutral point of 4?"
- For scores that are badly skewed, a **Bootstrap** interval of the median may describe the typical score better.
- To compare two groups, use **Independent-Samples T Test**; for two measures of the same people, **Paired-Samples T Test**.

## How to run it
Choose **Analyze › Compare Means › One-Sample T Test**. Move one or more numeric variables into **Test Variables** (one result each), set the test value, and press **Run**.

## Options
- **Test value**: the mean is compared with this value; default 0.
- **Hypothesis**: **Two-sided (default)**, **One-sided: the mean is above the test value**, or **One-sided: the mean is below the test value**.
- **Bayes factor**: BF₁₀ with R's BayesFactor default prior; off by default.
- **Confidence interval (%)**: default 95.

## What you get
- **One-sample statistics**: N, mean, SD and standard error of each variable.
- **One-sample t test**: t, df, p, the difference between the mean and the test value, and the confidence interval of that difference. If the interval excludes zero, the mean differs from the test value at the matching level.
- **Effect size**: Cohen's d (the mean difference divided by the SD), with Hedges' g beside it, which corrects d's small-sample bias.

Each test variable uses its own valid values, so N can differ between variables.

## Checked against
R's `t.test(x, mu = ...)` and BayesFactor's `ttestBF()`.
