# Normal Q-Q Plot

Plots each value against where it would fall if the data were normally distributed, so you can judge normality by eye.

## When to use it
- To check whether a variable, or a model's residuals, are close enough to normal for a t test, ANOVA or regression.
- It is usually more informative than a normality test, which flags trivial departures in large samples and misses real ones in small samples.
- **Explore** draws Q-Q plots alongside normality tests and descriptives. A **Histogram** shows the overall shape.

## How to run it
Choose **Graphs › Normal Q-Q Plot**. Move one or more numeric variables into **Variables**; you get one chart each. Each needs at least three valid values.

## Options
This dialog has no options.

## What you get
- A Q-Q plot drawn as an APA-style figure: observed values on the vertical axis against normal quantiles on the horizontal axis, with a reference line through the quartiles.
- A **Reference line** table with N and the line's intercept and slope.

How to read it: points close to the line mean the data are close to normal. A curve that bends upward at the right end shows a long right tail (positive skew); an S shape with both ends pulling away from the line shows heavy tails. A few stray points at the ends are common and rarely a problem with moderate samples. User-missing codes and empty cells are left out.

## Checked against
R's `qqnorm()` positions and `qqline()`.
