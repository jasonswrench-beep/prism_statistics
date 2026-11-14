# Linear Regression

Linear regression predicts a numeric outcome from one or more predictors, with optional blocks, interactions, robust standard errors and diagnostic plots.

## When to use it
- To estimate how much the outcome changes with each predictor while holding the others constant.
- To test whether a second block of predictors adds to the first (hierarchical regression).
- For a yes/no outcome, use **Binary Logistic**; for counts, **Poisson and Negative Binomial**; for curved relationships with one predictor, **Curve Estimation**.

## How to run it
Choose **Analyze › Regression › Linear**. Put the outcome in **Dependent**, numeric predictors in **Numeric predictors** and categorical ones in **Categorical predictors** (dummy-coded; choose each one's reference category by code, the lowest by default). Optionally add **Block 2: numeric predictors** and **Block 2: categorical predictors**, tested by R² change.

## Options
- **Interactions**: optional pairs of predictors, such as hours*cond. They enter as a final block.
- **Mean-center numeric predictors in interactions**: off by default.
- **Residual plots**: residuals against predicted values, plus a Q-Q plot and histogram of the standardized residuals; off by default.
- **Standard errors**: **Usual (default)**, **Robust (HC3)** or **Robust (HC1)**.
- **Bayes factor**: the model against no predictors (numeric predictors only); off by default.
- **Confidence interval (%)**: default 95.

## What you get
- **Model summary**: R², adjusted R² and the standard error of the estimate. R² is the share of the outcome's variance the model explains.
- **Analysis of variance**: the overall F test of the model.
- **Coefficients**: each B with its SE, t, p and interval, plus the standardized beta. B is the expected change in the outcome for a one-unit change in that predictor.
- **Model change** for blocks, and **Collinearity**, **Influential cases**, **Equal error variance** and residual checks.

A predictor that is a combination of others is named and the model is refused rather than quietly dropping a coefficient.

## Checked against
R's `lm()`, `sandwich::vcovHC()` and `lmtest::coeftest()`.
