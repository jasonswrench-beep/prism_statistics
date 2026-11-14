# Explore

A fuller look at numeric variables, overall or within the groups of a factor: descriptives, percentiles, normality tests, box plots and Q-Q plots in one run.

## When to use it
- Before choosing an analysis, to check shape, outliers and whether groups look roughly normal.
- To compare the distribution of a score across groups.
- For a quick table of means and SDs only, **Descriptives** is simpler.

## How to run it
Choose **Analyze › Descriptive Statistics › Explore**. Put one or more numeric outcomes in **Dependent List** (one result each). Optionally put one grouping variable in **Factor List** to describe each group separately.

## Options
- **Percentiles**: adds a table of percentiles; on by default.
- **Normality tests**: Shapiro-Wilk and Kolmogorov-Smirnov with Lilliefors' correction; on by default.
- **Box plots and Q-Q plots**: on by default.
- **Confidence interval (%)**: the level for the interval of the mean; default 95.

## What you get
- **Descriptive statistics**: the mean with its confidence interval, the 5% trimmed mean, the median, SD and range, and skewness and kurtosis with their standard errors. A trimmed mean far from the mean hints at outliers.
- **Percentiles**, using R's default rule (type 7).
- **Tests of normality**: a small p value says the data depart from a normal distribution. With large samples these tests flag trivial departures, so read them together with the plots.
- **Box plots** and **normal Q-Q plots**: points that bend away from the line show skew or heavy tails.

## Checked against
R's `quantile()` (type 7), `shapiro.test()`, and the Lilliefors test as in `nortest::lillie.test` (also checked with `DescTools::LillieTest`).
