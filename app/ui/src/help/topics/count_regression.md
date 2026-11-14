# Poisson and Negative Binomial Regression

Count regression predicts an outcome that counts events (visits, errors, citations) from numeric and categorical predictors, reporting rate ratios.

## When to use it
- The outcome is a whole number of events, zero or more, often skewed with many low values.
- Choose **Negative binomial** when the counts vary more than a Poisson model allows (overdispersion), which is common in practice.
- If people were observed for different lengths of time, enter that time as an **Exposure**. For a numeric outcome that isn't a count, use **Linear**.

## How to run it
Choose **Analyze › Regression › Poisson and Negative Binomial**. Put the count in **Dependent** and add **Numeric predictors** and **Categorical predictors** (dummy-coded against a reference you choose by code). Optionally add an **Exposure** variable (time at risk), entered as a log offset so the model describes rates.

## Options
- **Model**: **Poisson (default)** or **Negative binomial**, for overdispersed counts.
- **Confidence interval (%)**: default 95.

## What you get
- **Coefficients**: b, SE, z, p and the **rate ratio** with its interval. A rate ratio of 1.2 means each one-unit rise in the predictor goes with 20% more events (or a 20% higher rate with an exposure).
- **Model fit**: deviance, log-likelihood and related statistics, and for the negative binomial its dispersion parameter θ.
- An **Overdispersion** check for the Poisson model. If it flags a problem, rerun with the negative binomial; Poisson standard errors would be too small and the p values too optimistic.

The negative binomial's variance is μ + μ²/θ, so a smaller θ means more extra variation.

## Checked against
R's `glm(family = poisson)` and `MASS::glm.nb()`.
