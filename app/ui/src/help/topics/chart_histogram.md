# Histogram

Draws the distribution of a numeric variable as bars of equal width, with an optional normal curve, and lists the counts behind the bars.

## When to use it
- To see the shape of a variable: symmetric, skewed, flat, or with more than one peak.
- To spot outliers and impossible values before analysis.
- To judge normality more precisely, a **Normal Q-Q Plot** is easier to read. To compare groups side by side, use a **Box Plot**.

## How to run it
Choose **Graphs › Histogram**. Move one or more numeric variables into **Variables**; you get one chart each.

## Options
- **Normal curve**: overlays the normal distribution with the same mean and SD; off by default. It helps you see where the data depart from normal, such as a long right tail.

## What you get
- A histogram of each variable, drawn as an APA-style figure.
- A **Bins** table beside it with each bar's lower edge, upper edge and count, so the chart can be checked and read without sight.

Bins have equal widths at round numbers, chosen by Sturges' rule. Each bin holds the values above its lower edge up to and including its upper edge. The number of bins changes how the shape looks, so don't over-read small bumps. Each variable uses its own valid values; user-missing codes and empty cells are left out and counted.

## Checked against
R's `hist()`.
