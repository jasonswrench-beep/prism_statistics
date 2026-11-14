# Binary Logistic Regression

Binary logistic regression predicts a two-category outcome (yes or no, pass or fail) from numeric and categorical predictors, reporting odds ratios.

## When to use it
- The outcome has exactly two categories among the cases used.
- For an outcome with ordered categories, use **Ordinal Logistic**; with three or more unordered categories, **Multinomial Logistic**. For a numeric outcome, use **Linear**. With many predictors and a focus on prediction, consider **Penalized (Lasso, Ridge, Elastic Net)**.

## How to run it
Choose **Analyze › Regression › Binary Logistic**. Put the outcome in **Dependent** and choose the category modeled (the event); it is the second category, the higher code, unless you change it. Add **Numeric predictors** and **Categorical predictors** (dummy-coded against a reference you choose by code). Optionally add **Block 2: numeric predictors** and **Block 2: categorical predictors**, tested by likelihood-ratio change.

## Options
- **Interactions**: optional pairs of predictors, such as hours*cond. They enter as a final block.
- **Mean-center numeric predictors in interactions**: off by default.
- **ROC curve**: with the area under the curve and its DeLong interval; off by default.
- **Confidence interval (%)**: default 95.

## What you get
- **Outcome**: which category is the event and how many cases are in each.
- **Test of the model** and **Model summary**: the likelihood-ratio chi-square, −2 log likelihood, Cox and Snell, Nagelkerke and McFadden R², and AIC.
- **Coefficients**: B, SE, Wald, p and the **odds ratio** with its interval (profile likelihood where possible). An odds ratio above 1 means higher odds of the event as the predictor rises.
- **Likelihood-ratio tests of each predictor**, a **Classification table** at a cut-off of .5, the Hosmer-Lemeshow **Calibration** test, **Events per predictor**, **Influential cases**, and **Model change** for blocks.

Perfect separation is reported as a failure, never as huge coefficients.

## Checked against
R's `glm(family = binomial)` and pROC for the ROC curve.
