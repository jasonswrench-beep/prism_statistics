# Cox Regression

Fits a proportional hazards model: how several predictors together relate to the risk (hazard) of an event over time, with censored cases handled properly.

## When to use it
- Your outcome is time to an event, some cases are censored, and you want to adjust for several predictors at once.
- To describe and compare survival curves for groups without adjustment, use **Kaplan-Meier**.
- For an outcome measured once as yes or no, without follow-up times, use **Analyze › Regression › Binary Logistic**.

## How to run it
Choose **Analyze › Survival › Cox Regression**. Move the follow-up time into **Time** and the event indicator into **Status**. Move numeric predictors into **Numeric predictors** and categorical ones into **Categorical predictors** (dummy-coded; choose each one's reference category). Add at least one predictor.

## Options
- **Status codes that mean the event happened**: 1 by default; give several separated by commas, such as `1, 2`. Every other code counts as censored.
- **Reference for** each categorical predictor: the category the others are compared with; the first category by default.
- **Confidence interval (%)**: 95.

## What you get
- A coefficients table: each predictor's coefficient, standard error, Wald test, and its hazard ratio with a confidence interval. A hazard ratio of 1.5 means a 50% higher risk at any moment per unit of the predictor (or compared with the reference category); below 1 means lower risk.
- Overall tests of the model: likelihood ratio, Wald, and score (log-rank) tests.
- A reminder about the proportional hazards assumption, which is not tested here: each predictor should multiply the hazard by the same amount at every time. Kaplan-Meier curves that cross suggest it fails.

Tied event times are handled by Efron's method.

## Checked against
R's survival package (coxph, with its default Efron ties).
