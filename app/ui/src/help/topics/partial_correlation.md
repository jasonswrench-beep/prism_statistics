# Partial Correlation

Partial correlations measure the relationship between pairs of numeric variables after taking out the linear effect of one or more control variables from both.

## When to use it
- When a third variable could explain a relationship: does study time still relate to exam score once prior grades are controlled?
- To control for variables in a model with a single outcome, **Linear** regression gives coefficients as well. For plain correlations, use **Bivariate**.

## How to run it
Choose **Analyze › Correlate › Partial**. Put at least two numeric variables in **Variables** and one or more numeric controls in **Controlling for**.

## Options
- **Zero-order correlations**: the ordinary correlations of the same cases, shown for comparison; on by default.
- **Confidence interval (%)**: default 95.

## What you get
- **Partial correlations (controlling for ...)**: each pair's partial r, from −1 to +1.
- **Pairs**: each partial r with its t test, degrees of freedom (N − 2 − the number of controls), p and a Fisher-z confidence interval, with the zero-order correlation beside it.

Compare the two: if the correlation shrinks toward zero once the controls are in, the controls account for much of the relationship. If it barely changes, the relationship stands on its own. Cases are deleted listwise across every variable and control, so all coefficients use the same cases. Tests are two-sided.

## Checked against
R: Pearson correlations of the residuals from `lm()` fits on the controls.
