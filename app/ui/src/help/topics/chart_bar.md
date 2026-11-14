# Bar Chart

Draws one bar per category of a variable, showing counts, percentages, or the mean of another variable with its confidence interval.

## When to use it
- To show how cases are spread across the categories of a variable, such as condition or region.
- To compare group means of a numeric outcome, with intervals that show their precision.
- For a numeric variable's distribution, use a **Histogram**. To show the full spread of scores by group, a **Box Plot** reveals more than bars of means.

## How to run it
Choose **Graphs › Bar Chart**. Put the grouping variable in **Category axis**. For bars of means, also put a numeric variable in **Mean of**.

## Options
- **Bars show**: **Counts (default)**, **Percentages**, or **Means with 95% confidence intervals** of the variable under **Mean of**.

## What you get
- A bar chart drawn as an APA-style figure.
- For counts or percentages, a **Counts** table with each category's count and percent of the valid cases.
- For means, a **Means** table with each category's N, mean, SD and the 95% confidence interval of the mean (t-based), which the error bars show.

Intervals that barely overlap don't necessarily mean a significant difference, and overlapping ones don't prove there is none; use a test such as **One-Way ANOVA** to compare means. User-missing codes and empty cells are left out and counted.

## Checked against
R, using `table()` for counts and t-based intervals as in `t.test()` for the means.
