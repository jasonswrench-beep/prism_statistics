# PLS-SEM

Fits a partial least squares path model: constructs built as weighted composites of their indicators, and regression paths among them. It focuses on explaining and predicting the outcome constructs.

## When to use it
- Prediction-oriented models, small samples, or models with formative (mode B) constructs.
- Comparing path coefficients between two groups.
- For a covariance-based model with fit tests, use **Structural Equation Model** instead.

## How to run it
Choose **Analyze › Structural Equation Modeling › PLS-SEM**. The model builder opens: drag indicators onto the canvas, add constructs with **New construct** and **Add (mode A)** (reflective) or **Add (mode B)** (formative), and draw paths with the **Path** tool. Or type the model in **Text model**: `f =~ x1 + x2` (mode A), `f <~ x1 + x2` (mode B), and `y ~ f + g` for paths. The model needs at least one structural path. Under **Groups and clusters**, **Two groups to compare** runs a multigroup analysis.

## Options
- **Inner weighting**: Path weighting (default), Factorial, or Centroid.
- **Bootstrap resamples**: 0 for none; 5000 is usual for final results.
- **Seed**: 20260101, so bootstrap and permutation results can be repeated.
- **Q²predict folds**: 10; 0 for none.
- **Permutations (two groups)**: 1000.
- **Confidence interval (%)**: 95.

## What you get
- Path coefficients, R² and adjusted R², f² effect sizes, and total effects.
- Loadings and outer weights for each indicator, and VIF for each construct's predictors.
- Reliability and validity: Cronbach's alpha, rho_A, rho_C (composite reliability), AVE and HTMT.
- SRMR of the saturated model and Q²predict; a Q²predict above zero means the model predicts better than the mean.
- With bootstrapping, standard errors and percentile intervals; with two groups, a permutation test of each path difference.

## Checked against
seminr.
