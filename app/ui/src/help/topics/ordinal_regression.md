# Ordinal Logistic Regression

Ordinal logistic regression (the proportional-odds model) predicts an outcome with ordered categories, such as "disagree, neutral, agree", from numeric and categorical predictors.

## When to use it
- The outcome's categories have a natural order but the distances between them aren't known to be equal (ratings, stages, levels).
- For two categories, use **Binary Logistic**. For unordered categories, use **Multinomial Logistic**. If the ordered scale has many points and behaves like a number, **Linear** regression may be adequate.

## How to run it
Choose **Analyze › Regression › Ordinal Logistic**. Put the outcome in **Dependent**; its categories are taken as ordered in code order, so check the codes run from lowest to highest. Add **Numeric predictors** and **Categorical predictors** (dummy-coded against a reference category you choose by code).

## Options
- **Confidence interval (%)**: default 95.

## What you get
- **Model fit**: −2 log-likelihood, the likelihood-ratio chi-square of the model with its df and p, and McFadden's and Nagelkerke's R².
- **Coefficients**: b, SE, z, p and the **odds ratio** with its interval. A positive coefficient (odds ratio above 1) means higher categories become more likely as the predictor rises.
- **Thresholds**: the cut points between adjacent categories on the logit scale. They place the categories along the underlying scale and are rarely interpreted on their own.

The model assumes one set of coefficients works for every cut point (proportional odds). If you suspect a predictor matters more at the top of the scale than at the bottom, compare with a multinomial model.

## Checked against
R's `MASS::polr()`, whose parametrization (logit P(Y ≤ j) = θⱼ − xβ) the app uses.
