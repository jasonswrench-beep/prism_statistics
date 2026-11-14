# Gradient Boosting

Builds a prediction model one small tree at a time: each new tree corrects the errors of the ones before, and its contribution is shrunk so the model learns slowly and generalizes well.

## When to use it
- You want strong predictive accuracy for a numeric or two-category outcome from many predictors.
- Your predictors have missing values; boosting handles them without dropping cases.
- For a model that needs less tuning, try **Random Forest**; for readable rules, **Decision Tree**.

## How to run it
Choose **Analyze › Classify › Gradient Boosting**. Move the outcome into **Outcome** (numeric, or two categories) and the predictors into **Predictors** (numeric or categorical; missing values allowed). To use your own folds, move a numeric variable into **Fold variable**.

## Options
- **Loss**: From the outcome (default; Bernoulli for a categorical outcome), Squared error, or Bernoulli (logistic).
- **Trees**: 100, the most trees grown.
- **Splits per tree (interaction depth)**: 1. One split per tree gives an additive model; more allow interactions.
- **Shrinkage (learning rate)**: 0.1. Smaller values learn more slowly and usually need more trees.
- **Bag fraction**: 0.5, the share of cases sampled for each tree; 1 makes the fit deterministic.
- **Smallest node**: 10 cases.
- **Cross-validation folds to choose the trees (0: none)**: 5.
- **Seed**: 20260101.

## What you get
- The number of trees used: chosen by cross-validation when folds are on, which guards against overfitting.
- Model performance at that number: training and cross-validated RMSE and R² for a numeric outcome, or Bernoulli deviance and accuracy for a two-category one.
- A chart of the error by number of trees, training and cross-validated. The cross-validated curve's low point is the best number of trees.
- Relative influence: each predictor's share (%) of the total improvement from its splits. Larger means more useful for prediction; it doesn't show direction.

## Checked against
R's gbm.
