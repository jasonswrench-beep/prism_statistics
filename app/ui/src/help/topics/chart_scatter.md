# Scatterplot

Plots one point per case for two numeric variables, with an optional least-squares line, to show how they relate.

## When to use it
- Before computing a correlation or regression, to check that the relationship is roughly straight and to spot outliers or clusters.
- To show a relationship in a report.
- For the numbers behind a relationship, use **Bivariate** correlation or **Linear** regression. For curved patterns, **Curve Estimation** fits several shapes.

## How to run it
Choose **Graphs › Scatterplot**. Put the variable to plot up and down in **Vertical axis (Y)** and the one across in **Horizontal axis (X)**. If one variable is thought to influence the other, it usually goes on the horizontal axis.

## Options
- **Fit line**: draws the least-squares line of Y on X; off by default.

## What you get
- A scatterplot drawn as an APA-style figure, with one point per case that has both values.
- With the fit line, a **Fitted line** table with N, the intercept, the slope and Pearson's r. The slope is the expected change in Y for a one-unit increase in X.
- Without it, a **Cases** table with the number of points plotted.

Look for curves, fan shapes (spread that grows with X) and isolated points; each can mislead a correlation or regression. Cases missing either value are left out and counted.

## Checked against
R's `lm()` for the fitted line and `cor()` for r.
