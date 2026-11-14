# Bootstrap

Builds confidence intervals by resampling your cases many times, without assuming a normal distribution.

## When to use it
- When a statistic has no simple formula for its interval, such as a median or a standard deviation.
- When the data are skewed and you'd rather not lean on normal theory.
- When you want resampling-based intervals for correlations or regression coefficients.

## How to run it
Choose **Analyze › Descriptive Statistics › Bootstrap**. Put numeric variables in **Variables**. For regression coefficients, those variables are the predictors and the outcome goes in **Dependent** (used only for a regression).

## Options
- **Statistic**: **Mean** (default), **Median**, **5% trimmed mean**, **Standard deviation**, **Correlations** (every pair of the variables) or **Regression coefficients** (the dependent on the variables).
- **Correlation coefficient**: **Pearson** (default) or **Spearman**, for correlations.
- **Resamples**: how many bootstrap samples to draw; default 2000, between 100 and 100,000.
- **Seed**: default 20260101. The same seed gives the same intervals on any computer.
- **Confidence interval (%)**: default 95.

## What you get
One **Bootstrap** table with a row per variable, pair or term:
- **Estimate**: the statistic in your actual data.
- **Bias**: the average resampled value minus the estimate.
- **SE**: the standard deviation of the resampled values, the bootstrap standard error.
- **Percentile** interval: the middle of the resampled values (the middle 95% by default).
- **BCa** interval: corrected for bias and skew; usually the better one to report.
- **Resamples used**: the resamples in which the statistic could be computed.

More resamples make the interval ends more stable. Report the seed so others can repeat your numbers.

## Checked against
The interval arithmetic follows R's `boot` package; the estimates are checked against R's `mean()`, `median()`, `sd()`, `cor()` and `lm()`.
