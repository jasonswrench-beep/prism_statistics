# Curve Estimation

Fits one or more curves (linear, quadratic, cubic, logarithmic and others) of a numeric outcome on one predictor, so you can compare their shapes and fit.

## When to use it
- A scatterplot suggests the relationship bends, levels off or grows faster than a straight line.
- You want a quick comparison of common curve shapes for one predictor.
- For several predictors at once, use **Linear** regression (you can add squared terms with **Compute Variable**).

## How to run it
Choose **Analyze › Regression › Curve Estimation**. Put one or more numeric outcomes in **Dependent(s)** (one result each) and the numeric predictor in **Independent**.

## Options
- **Models**: tick at least one of **Linear** (default), **Quadratic**, **Cubic**, **Logarithmic** (needs positive x), **Inverse** (needs nonzero x), **Power** (needs positive x and y), **Exponential** (needs positive y) and **S** (needs nonzero x and positive y).
- **Plot the curves over the data**: on by default.
- **Confidence interval (%)**: default 95.

## What you get
- **Model summary and parameter estimates**: for each curve, R², the F test, its degrees of freedom and p, and the fitted constants. A higher R² means a closer fit, but prefer the simplest curve that fits about as well and makes sense for your data.
- **Coefficients** with standard errors and confidence intervals.
- A plot of the outcome against the predictor with each fitted curve drawn over the points.

Each curve is the least-squares fit of the transformed outcome on the transformed predictor. The power, exponential and S curves are fitted on ln(y), so their R² is on that scale; R² on the outcome's own scale is shown beside it for a fair comparison. Cases are used when both variables are valid.

## Checked against
R's `lm()` on the transformed variables.
