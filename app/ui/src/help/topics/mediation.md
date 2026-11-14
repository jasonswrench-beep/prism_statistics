# Mediation Analysis

Tests whether a predictor X affects an outcome Y through one or more mediators, estimating each indirect effect with a bootstrap confidence interval.

## When to use it
- You have a theory of how X works: X changes M, and M in turn changes Y (for example, training raises confidence, which raises performance).
- Several mediators can be tested side by side (in parallel).
- To ask *when* or *for whom* X matters, rather than *how*, use **Moderation**. For latent variables or more complex paths, a structural equation model is the better tool.

## How to run it
Choose **Analyze › Regression › Mediation**. Put the predictor in **X (predictor)**, one or more numeric variables in **Mediators**, the outcome in **Y (outcome)**, and optionally numeric **Covariates**.

## Options
- **Bootstrap resamples**: default 5000.
- **Seed**: default 20260101. The same seed gives the same intervals.
- **Confidence interval (%)**: default 95.

## What you get
- **Paths**: the a paths (each mediator regressed on X), the b paths and the direct effect c′ (Y regressed on X and every mediator), and the total effect c (Y regressed on X), each with its b, SE, t, p and interval.
- **Indirect effects**: each a × b, and their total, with a bootstrap SE and a percentile bootstrap interval. If the interval excludes zero, there is evidence of mediation through that mediator.
- The models of each mediator and of Y.

The total effect equals the direct effect plus the indirect effects. A significant indirect effect doesn't need a significant total effect. Mediation is a causal claim, so the design (timing, randomization) matters as much as the numbers.

This follows Hayes's regression approach to simple and parallel mediation, with ordinary least squares and the usual standard errors.

## Checked against
R's `lm()` for every path and indirect effect.
