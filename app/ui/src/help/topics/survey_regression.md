# Survey Regression

Fits a linear regression to complex survey data, using the sampling weights for the coefficients and the design (strata and clusters) for the standard errors.

## When to use it
- Your data come from a weighted, stratified or clustered sample and you want population-level regression estimates.
- For ordinary regression on a simple random sample, use **Analyze › Regression › Linear**.
- For weighted means and proportions rather than a model, use **Survey Estimates**.

## How to run it
Choose **Analyze › Complex Samples › Survey Regression**. Move the outcome into **Dependent**, numeric predictors into **Numeric predictors**, and categorical predictors into **Categorical predictors** (they are dummy-coded; choose each one's reference category). Move the weight into **Sampling weight**, and if the design has them, the stratum variable into **Strata** and the primary sampling unit into **Clusters (PSUs)**.

## Options
- **Reference for** each categorical predictor: the category the others are compared with; the first category by default.
- **Confidence interval (%)**: 95.

## What you get
A coefficients table: each estimate (from weighted least squares), its design-based standard error, t test, p value and confidence interval. A note gives the weight, the number of PSUs and strata, and the design degrees of freedom (PSUs minus strata), which the tests use.

Each coefficient is the expected change in the outcome for a one-unit change in that predictor, holding the others constant; for a dummy-coded category, it is the difference from the reference category. Standard errors come from linearization with the ultimate-cluster method, treating first-stage sampling as with replacement.

## Checked against
R's survey package (svyglm).
