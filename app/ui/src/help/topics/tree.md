# Decision Tree

Grows a classification or regression tree: a set of simple yes-or-no splits on the predictors that sorts cases into groups with similar outcomes, then prunes it by cross-validation so it doesn't just memorize the data.

## When to use it
- You want easy-to-explain rules ("if age is over 40 and income below 30,000, then...").
- You want to see which predictors matter and how they interact, with missing predictor values handled.
- For more accurate predictions at the cost of a single readable tree, use **Random Forest** or **Gradient Boosting**.

## How to run it
Choose **Analyze › Classify › Decision Tree**. Move the outcome into **Outcome** and the candidate predictors into **Predictors**; both can be numeric or categorical. To use your own cross-validation folds, move a numeric fold variable into **Fold variable**.

## Options
- **Tree**: From the outcome (default; classification for a categorical outcome), Regression tree, or Classification tree.
- **Size**: Prune by the one-SE rule (default: the smallest tree about as good as the best), Prune to the smallest cross-validated error, or Grow to the complexity parameter only.
- **Smallest node to split**: 20.
- **Complexity parameter (cp)**: 0.01; a split must improve the fit by this share.
- **Surrogate splits per split**: 5; cases missing a split's predictor are sent by the best-agreeing other predictors.
- **Maximum depth**: 30.
- **Cross-validation folds**: 10.
- **Seed**: 20260101, for the folds.

## What you get
- The tree diagram, and a table of its nodes: each split, the number of cases, the prediction and the error.
- The complexity table: for each tree size, the relative error in the training data and the cross-validated error with its standard error. This is what pruning uses.
- Surrogate splits, when cases have missing predictor values: which other predictor stands in at each split, and how well it agrees.
- Predictor importance: each predictor's share of the risk reduction, counting its surrogate role too.
- Accuracy (classification) or error (regression) in the training data and by cross-validation, compared with always predicting the largest class.

## Checked against
R's rpart.
