# Structural Equation Model

Fits a covariance-based structural equation model: latent variables, regression paths and covariances. Draw the model as a diagram or type it in lavaan syntax; the two stay in step.

## When to use it
- Path analysis, latent-variable models, mediation, and multigroup, two-level or latent growth models.
- For a plain CFA, **Factor Analysis of a Scale (CFA)** is quicker; for composite models, use **PLS-SEM**.

## How to run it
Choose **Analyze › Structural Equation Modeling › Structural Equation Model**. Drag variables from **Variables** onto the canvas (or use **Add to diagram**), make latent variables with **New latent variable** and **Add oval**, and draw arrows with the **Path** and **Covariance** tools. Or type in **Text model**: `f =~ x1 + x2 + x3`, `y ~ f`, `a ~~ b`, labels like `a*x`, fixed values like `1*x`, and defined effects like `ab := a*b`. Under **Groups and clusters**, pick a grouping variable for a multigroup model or a cluster variable for a two-level model.

## Options
- **Estimator**: Maximum likelihood (default); robust MLM or MLR; WLSMV for ordinal indicators; or Bayesian (MCMC) with blavaan's default priors.
- **Missing values**: Complete cases only (default) or Full-information ML.
- **Identify factors by fixing their variances at 1** (off), **Estimate intercepts and means** (off), **Latent growth model** (off), **Fix observed predictors at their sample moments** (on).
- **Held equal across groups** and **Free across groups anyway**: equality constraints for multigroup models.
- **Composite reliability, AVE and HTMT**: on. **Modification indices**: on.
- **Bootstrap resamples**: 0 (none); 1000 or more for indirect-effect intervals, optionally **Bias-corrected bootstrap intervals**.
- **Seed (bootstrap and MCMC)**, and for Bayesian runs **MCMC chains** (3), **Warm-up draws per chain** (500) and **Kept draws per chain** (1000).
- **Confidence interval (%)**: 95.

## What you get
Fit indices (chi-square, CFI, TLI, RMSEA with its 90% interval, SRMR), unstandardized and standardized estimates with tests, R² for each outcome, defined effects such as indirect effects, and modification indices. Bayesian runs report posterior summaries, PPP, DIC and WAIC.

## Checked against
lavaan, and blavaan for the Bayesian estimator.
