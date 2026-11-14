# Panel Data Regression

Fits linear regression models to panel data: the same individuals (people, firms, countries) observed over several periods, in long form with one row per individual and period.

## When to use it
- You want to control for everything stable about each individual, measured or not (fixed effects), or to model individual differences as random.
- If the outcome depends on its own past values, use **Dynamic Panel (GMM)**. For other repeated-measures designs, a linear mixed model is an alternative.

## How to run it
Choose **Analyze › Regression › Panel Data**. Put the outcome in **Dependent**, predictors in **Numeric predictors** and **Categorical predictors** (dummy-coded), the unit identifier in **Individual (unit)** and the time variable in **Period**.

## Options
- **Model**: **Fixed effects (within) (default)**, **Random effects** (Swamy-Arora; assumes the effects are unrelated to the predictors), **Pooled OLS**, **First differences**, or **Between (individual means)**.
- **Effects for**: **Individuals (default)**, **Periods**, or **Both (two-way)**; shown for fixed and random effects. Two-way random effects need balanced panels.
- **Cluster-robust standard errors (by individual)**: off by default.
- **F, Hausman and Breusch-Pagan tests**: on by default.
- **Confidence interval (%)**: default 95.

## What you get
- **Panel structure**: the number of individuals and periods, and whether the panel is balanced.
- **Coefficients**: estimates with SE, t, p and intervals. In a fixed-effects model they describe change within individuals.
- **Specification tests**: the F test for individual effects (pooled against fixed), Hausman's test (fixed against random; a small p value favors fixed effects) and the Breusch-Pagan test for random effects.
- A **Serial correlation** reminder when you use the usual standard errors: errors are often correlated within individuals over time, and cluster-robust standard errors guard against that.

## Checked against
R's `plm()` and its `vcovHC()` (method "arellano").
