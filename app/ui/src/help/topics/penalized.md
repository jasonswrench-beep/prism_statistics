# Penalized Regression (Lasso, Ridge, Elastic Net)

Fits linear or logistic regression with a penalty that shrinks coefficients toward zero, choosing the penalty's strength (λ) by cross-validation, to build models that predict well on new data.

## When to use it
- Many predictors, possibly correlated, and the goal is prediction rather than testing each coefficient.
- The lasso can drop predictors entirely, which helps pick a smaller set; ridge keeps them all but tames unstable estimates.
- To test hypotheses about specific predictors, ordinary **Linear** or **Binary Logistic** regression gives p values; this procedure does not.

## How to run it
Choose **Analyze › Regression › Penalized (Lasso, Ridge, Elastic Net)**. Put the outcome in **Dependent** (numeric, or two categories), predictors in **Numeric predictors** and **Categorical predictors** (dummy-coded), and optionally your own folds in **Fold variable**.

## Options
- **Outcome**: **Continuous (linear)** (default) or **Two categories (logistic)**, which models the second category.
- **Penalty**: **Lasso (default)**, **Ridge** or **Elastic net**.
- **Elastic-net α (0 = ridge, 1 = lasso)**: default 0.5.
- **Choose λ by**: **One-standard-error rule (default)**, the simplest model about as good as the best, or **Minimum cross-validated error**.
- **Folds**: default 10. **Repeat the cross-validation**: default 1. **Seed**: default 20260101.

## What you get
- **Choice of λ**: λ at the minimum error and by the one-standard-error rule.
- **Coefficients at** the chosen λ, on the predictors' original scale. Zeros mean the lasso dropped that predictor.
- **Predictive performance**: cross-validated error (and accuracy measures for two categories) from out-of-fold predictions, an honest guide to performance on new data.
- A **Cross-validation curve** and **Coefficient paths** showing how estimates shrink as λ grows, with notes on **Fold size** and on **No predictive signal** when nothing beats the intercept.

## Checked against
R's `glmnet::cv.glmnet()`, with pROC for the area under the curve.
