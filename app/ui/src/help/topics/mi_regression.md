# Regression with Multiple Imputation

Fills in missing values many times over, fits a linear regression to each completed dataset, and pools the results, so that the standard errors reflect the uncertainty about the missing values.

## When to use it
- Your regression would lose many cases to listwise deletion, and the data are plausibly missing at random.
- You have extra variables that predict the missing values but don't belong in the model (auxiliary variables).
- Check the extent and pattern of missingness first with **Missing Data Patterns**. With no missing values, use **Analyze › Regression › Linear**.

## How to run it
Choose **Analyze › Missing Values › Regression with Multiple Imputation**. Move the outcome into **Dependent** and the predictors into **Predictors** (both numeric). Optionally move variables into **Auxiliary variables**: they are used only to impute, not in the regression.

## Options
- **Imputations**: 20, the number of completed datasets (2 to 500).
- **Iterations per imputation**: 10 (1 to 200).
- **Seed**: 20260101. The same seed gives the same imputations.
- **Confidence interval (%)**: 95.

## What you get
A table of pooled coefficients: each estimate, its standard error, t test, degrees of freedom, p value, confidence interval, and the fraction of missing information (FMI).

The pooled estimate is the average across imputations. Its standard error combines the variation within each imputed dataset and the variation between them (Rubin's rules), with Barnard and Rubin's degrees of freedom. The FMI is the share of the uncertainty that comes from the missing values; a large FMI means more imputations may be worthwhile.

Each variable with missing values is imputed by chained equations, drawing from a Bayesian linear regression on the other variables.

## Checked against
R's mice (the "norm" method and pool).
