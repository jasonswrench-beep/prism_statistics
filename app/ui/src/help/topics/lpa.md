# Latent Profile Analysis

Finds hidden groups (profiles) of cases with similar patterns on several continuous measures, by fitting Gaussian mixture models with different numbers of classes and covariance structures and comparing them.

## When to use it
- Your indicators are continuous (scale scores, test results, physiological measures) and you suspect distinct profiles of people.
- You want the data to help choose the number of classes.
- For categorical indicators, use **Latent Class Analysis**.

## How to run it
Choose **Analyze › Classify › Latent Profile Analysis**. Move the continuous variables into **Indicators**. Cases with every indicator present are used.

## Options
- **Fewest classes**: 1 by default.
- **Most classes**: 6 by default (up to 12).
- **Covariance structures**: which models to fit; all four are on.
  - **EEI**: equal variances, no covariances (the classic profile model).
  - **VVI**: variances differ by class, no covariances.
  - **EEE**: equal variances and covariances.
  - **VVV**: variances and covariances differ by class.
- **Random starts per model**: 50 by default; more starts guard against local maxima.
- **Seed**: 20260101.

## What you get
- A model comparison table for every structure and number of classes: log-likelihood, parameters, BIC, AIC, ICL, entropy and the smallest class's share. BIC here is "larger is better" (2 log L − p log n), as mclust reports it.
- For the model with the best BIC: each class's share, the number of cases assigned, and the average posterior probability of belonging to the assigned class (values near 1 mean clear classification); the class means and standard deviations, and within-class correlations where the model has covariances.
- A profile plot of the class means across the indicators.
- A local-maxima check: how many starts reached the best log-likelihood.

Choose the number of profiles with both the fit statistics and whether each profile is large enough and makes sense.

## Checked against
R's mclust.
