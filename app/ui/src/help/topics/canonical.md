# Canonical Correlation

Canonical correlation finds the strongest correlations between weighted sums of one set of numeric variables and weighted sums of another set.

## When to use it
- You have two groups of measures, such as a set of personality scales and a set of performance scores, and want to know how the sets relate as a whole.
- For relationships between single pairs of variables, use **Bivariate**. For several outcomes compared across groups, **MANOVA** is the better fit. To predict one outcome, use **Linear** regression.

## How to run it
Choose **Analyze › Correlate › Canonical**. Put the numeric variables of one set in **First set** and those of the other in **Second set**.

## Options
This dialog has no options. Cases are deleted listwise across both sets, and the analysis uses the correlation matrix, so the weights are standardized.

## What you get
- **Canonical correlations and dimension-reduction tests**: one row per canonical function. The first function is the strongest link between the sets; each later one is the strongest left over and unrelated to the earlier ones. The Wilks' lambda test of function i (Rao's F approximation) asks whether functions i to the last are all zero, so read down until a test is no longer significant.
- **Standardized canonical coefficients** for each set: the weights that build each canonical variate.
- **Structure coefficients**: each variable's correlation with its own set's canonical variate. These are usually easier to interpret than the weights; variables with large values define what the function means.
- **Cases per variable**, as a check on sample size.

The sign of a function is arbitrary; the app makes the first set's structure coefficients sum to a positive number.

## Checked against
R's `cancor()`.
