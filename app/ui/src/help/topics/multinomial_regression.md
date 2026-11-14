# Multinomial Logistic Regression

Multinomial logistic regression predicts an outcome with three or more unordered categories, such as preferred transport or political party, by comparing each category with a reference category.

## When to use it
- The outcome's categories have no natural order.
- For ordered categories, **Ordinal Logistic** is more economical. For two categories, use **Binary Logistic**.

## How to run it
Choose **Analyze › Regression › Multinomial Logistic**. Put the outcome in **Dependent** and choose its reference category by code; every other category is compared against it. Add **Numeric predictors** and **Categorical predictors** (dummy-coded against a reference you choose by code).

## Options
- **Confidence interval (%)**: default 95.

## What you get
- **Coefficients (each category against the reference)**: for every non-reference category, each predictor's b, SE, z, p and **odds ratio** with a Wald interval. An odds ratio of 1.5 means a one-unit rise in the predictor multiplies the odds of being in that category rather than the reference by 1.5.
- **Likelihood-ratio tests**: each predictor tested by leaving it out of the model, covering all its comparisons at once. Use these to decide whether a predictor matters overall before reading its separate coefficients.

Pick a reference category that makes the comparisons meaningful, such as the most common choice or a natural baseline. The model is fitted by maximum likelihood.

## Checked against
R's `nnet::multinom()`.
