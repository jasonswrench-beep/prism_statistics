# ROC Curve

Shows how well a score separates two states (sick or healthy, pass or fail) across every possible cutoff, and summarizes it with the area under the curve (AUC).

## When to use it
- To judge a diagnostic test, a screening questionnaire or a risk score against a known outcome.
- To choose a cutoff that balances sensitivity and specificity.
- To compare several test variables on the same cases.
- For an ROC curve of a fitted model's predictions, turn on **ROC curve** in **Analyze › Regression › Binary Logistic**.

## How to run it
Choose **Analyze › Classify › ROC Curve**. Move one or more scores into **Test variables**; higher values should point to the positive state. Move the variable holding the true state into **State variable**.

## Options
- **Code of the positive state**: the code in the state variable that marks a positive case; 1 by default. Every other code counts as negative.
- **ROC curve**: draws the curves; on by default.
- **Confidence interval (%)**: 95.

## What you get
- For each test variable: the AUC with DeLong's standard error, confidence interval and test against 0.5.
- The best cutoff by Youden's J (sensitivity + specificity − 1), with its sensitivity and specificity.
- The ROC curve: sensitivity against 1 − specificity at every cutoff. The closer it bends toward the top left corner, the better the test.

The AUC is the chance that a randomly chosen positive case scores higher than a randomly chosen negative one. An AUC of 0.5 is no better than guessing; about 0.7 is acceptable, 0.8 good and 0.9 or more excellent. If the AUC is below 0.5, lower scores point to the positive state; check the direction.

## Checked against
R's pROC (roc, ci.auc with the DeLong method, and coords with Youden's J).
