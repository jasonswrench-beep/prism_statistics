# Random Forest

Grows hundreds of decision trees, each on a bootstrap sample of the cases and trying a random subset of predictors at each split, and averages their predictions. Forests usually predict far better than a single tree.

## When to use it
- You want accurate prediction of a numeric or categorical outcome from many predictors, with interactions and nonlinear effects found automatically.
- You want a ranking of which predictors matter most.
- For a single, readable set of rules, use **Decision Tree**. For a boosted alternative that is often even more accurate when tuned, use **Gradient Boosting**.

## How to run it
Choose **Analyze › Classify › Random Forest**. Move the outcome into **Outcome** and the predictors into **Predictors**; both can be numeric or categorical.

## Options
- **Forest**: From the outcome (default; classification for a text or nominal outcome), Regression, or Classification.
- **Trees**: 500.
- **Predictors tried at each split**: left empty, p/3 for regression or √p for classification, where p is the number of predictors.
- **Largest node left unsplit**: left empty, 5 for regression or 1 for classification.
- **Permutation importance**: on by default.
- **Seed**: 20260101. Forests rely on random draws, so the seed makes a run repeatable.

## What you get
- Out-of-bag performance: each case is predicted only by the trees that did not see it, which gives an honest error estimate without a separate test set. For regression, the out-of-bag mean squared error and the percentage of variance explained; for classification, the out-of-bag error rate and the confusion matrix with each class's error.
- Variable importance: the mean decrease in accuracy when a predictor's values are shuffled (scaled), and the total increase in node purity (or mean decrease in Gini). Larger values mean the predictor matters more for prediction; they do not show the direction of its effect.

## Checked against
R's randomForest.
