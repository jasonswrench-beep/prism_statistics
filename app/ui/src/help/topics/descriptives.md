# Descriptives

Summarizes numeric variables with the mean, standard deviation, minimum and maximum, plus any extra statistics you ask for.

## When to use it
- To describe your sample's scores in a results section or a table of measures.
- To spot impossible values (a maximum of 99 on a 1 to 7 scale) before analysis.
- For skewness, percentiles, normality tests and plots, use **Explore**. For categorical variables, use **Frequencies**.

## How to run it
Choose **Analyze › Descriptive Statistics › Descriptives** (or the Descriptives toolbar button). Only numeric variables are listed. Move the ones you want from **Variables in the data** into **Variables** with the arrow, then press **Run**. **Paste** writes the command to your script instead.

## Options
- **Statistics**: tick the statistics to report. The defaults are **Mean**, **Standard deviation**, **Minimum** and **Maximum**. You can add **Range**, **Median**, **Standard error of the mean**, **Variance** and the **95% confidence interval of the mean**. At least one must be ticked.
- **Reset** clears the variables and restores the default statistics.

## What you get
A **Descriptive statistics** table with one row per variable: N (the number of valid values) and each statistic you chose. Each variable uses its own valid values, so N can differ from row to row; user-missing codes and empty cells are left out. The standard deviation uses n − 1, and the median uses R's default quantile rule (type 7). The confidence interval of the mean is the usual t-based interval: the range of population means consistent with your data.

## Checked against
R's `mean()`, `sd()`, `quantile()` (type 7) and the interval from `t.test()`.
