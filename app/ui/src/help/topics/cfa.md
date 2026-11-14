# Factor Analysis of a Scale (CFA)

Tests whether your scale items group into the factors you expect. You say how many factors there are and which items belong to each; the app writes the model and runs a confirmatory factor analysis.

## When to use it
- Confirming the structure of a questionnaire with known subscales, with reliability and discriminant validity for a methods section.
- If you don't yet know how the items group, start with **Analyze › Dimension Reduction › Factor Analysis**.
- For cross-loadings, correlated residuals or paths between factors, use **Open in Model Builder** or **Structural Equation Model**.

## How to run it
Choose **Analyze › Structural Equation Modeling › Factor Analysis of a Scale (CFA)**. Set **Number of factors** (1 to 12), give each factor a name, then put each item in its factor's box: drag it, double-click it (it goes to the first factor), or select items and use the factor's **Move here** button. Each factor needs at least two items; three or more is best. Expand **The model this runs** to see the lavaan syntax.

## Options
- **Factors correlate**: lets the factors correlate, as is usual for subscales of one instrument; on by default.
- **Identify each factor by fixing its variance at 1**: instead of fixing each factor's first loading at 1; off by default.
- **Estimator**: Maximum likelihood (default); Robust maximum likelihood (MLR), for items that aren't normal; or Ordinal (WLSMV), for Likert items with few categories.

**Open in Model Builder** carries the same model to the full builder; **Paste** adds it to the script without running it.

## What you get
- Fit: chi-square, CFI, TLI, RMSEA with its 90% interval, and SRMR. Roughly, CFI and TLI of .95 or more and RMSEA and SRMR below .06 to .08 suggest good fit.
- Loadings, unstandardized and standardized, with tests; standardized loadings above about .5 are welcome.
- Factor correlations, residual variances and each item's R².
- Composite reliability (omega), AVE and HTMT.
- Modification indices: the changes that would most improve fit.

## Checked against
lavaan (cfa and sem), with the same defaults.
