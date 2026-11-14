# Dynamic Panel Regression (GMM)

Fits a dynamic panel model, in which the outcome depends on its own earlier values, by Arellano and Bond's difference GMM.

## When to use it
- Panel data with many individuals and few periods, where this period's outcome depends on last period's (habits, persistence, momentum).
- Ordinary fixed effects are biased when a lagged outcome is a predictor; GMM uses deeper lags as instruments to avoid that bias.
- Without a lagged outcome, **Panel Data** is simpler and easier to explain.

## How to run it
Choose **Analyze › Regression › Dynamic Panel (GMM)**. Put the outcome in **Outcome**, optional **Covariates** (treated as exogenous), the panel unit in **Individual** and the time variable in **Period**. The data are in long form, one row per individual and period.

## Options
- **Estimator**: **Two-step GMM (default)**, with Windmeijer-corrected robust standard errors, or **One-step GMM**.
- **Effects**: **Individual and time (default)**, which adds time dummies, or **Individual only**.
- **Lags of the outcome**: default 1.
- **Lags of each covariate (0: current only)**: default 0.
- **Instruments: from lag**: default 2. **Instruments: to lag**: default 99.
- **Collapse the instruments**: one column per lag, for fewer instruments; off by default.
- **Robust standard errors**: on by default.
- **Confidence interval (%)**: default 95.

## What you get
- **Coefficients (differenced equation)**: estimates with SE, z, p and intervals. The lagged-outcome coefficient shows how persistent the outcome is.
- **Specification tests**: the Sargan test of the instruments (a small p value questions them), the Arellano-Bond AR(1) and AR(2) tests of the differenced residuals, and Wald tests of the coefficients and time dummies. AR(1) is expected; a significant AR(2) means lag 2 of the outcome is not a valid instrument, and a **Second-order autocorrelation** warning says so.
- **Instrument count** and **Weight matrix** notes. Too many instruments weaken the Sargan test; collapsing helps.

## Checked against
R's `plm::pgmm()` with the "d" (difference) transformation.
