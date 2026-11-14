# Moderation Analysis

Tests whether the effect of a predictor X on an outcome Y depends on a third variable W, the moderator, and shows the slope of X at chosen values of W.

## When to use it
- Your question is "for whom" or "when": does the effect of study hours on scores depend on prior knowledge?
- X and W are numeric (a 0/1 variable works as either).
- To ask *how* X affects Y through another variable, use **Mediation**. For interactions among several predictors, use **Linear** regression with its **Interactions** option.

## How to run it
Choose **Analyze › Regression › Moderation**. Put the predictor in **X (predictor)**, the moderator in **W (moderator)**, the outcome in **Y (outcome)** and optionally numeric **Covariates**.

## Options
- **Mean-center X and W**: off by default. Centering changes the meaning of the lower-order terms (they become effects at the mean), not the interaction.
- **Probe the slope of X at**: **16th, 50th and 84th percentiles of W (default)** or **Mean of W and ± 1 SD**.
- **Johnson-Neyman values**: on by default.
- **Simple slopes plot**: on by default.
- **Confidence interval (%)**: default 95.

## What you get
- **Model of Y**: coefficients for X, W and X × W with SE, t, p and intervals. A significant X × W term means the slope of X changes with W.
- **Slope of X at values of W**: the conditional effect of X at each probe value, with its test and interval.
- **Johnson-Neyman values**: the values of W at which the slope of X crosses into or out of significance, which shows the range of W where X matters.
- **Simple slopes** plot: lines of predicted Y against X at the probe values of W.

The model uses ordinary least squares with the usual standard errors, in the manner of Hayes's model 1.

## Checked against
R's `lm()` and `interactions::johnson_neyman()`.
