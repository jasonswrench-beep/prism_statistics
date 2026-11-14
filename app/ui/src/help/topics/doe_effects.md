# Factorial Effects (Design of Experiments)

Estimates the main effects and interactions in a two-level factorial or fractional factorial experiment, and shows which ones stand out, even when the design has no replicate runs.

## When to use it
- You ran a two-level design (for example one made with **Create Design**) and want to know which factors and interactions move the response.
- Screening experiments with few or no degrees of freedom left for error.
- For designs with three or more levels per factor and curvature, use **Response Surface**; for general factorial data, **Analyze › Compare Means › Factorial ANOVA (Type III)**.

## How to run it
Choose **Analyze › Design of Experiments › Factorial Effects**. Move the measured outcome into **Response** and the factors into **Two-level factors**. Each factor needs two settings (center points halfway between are allowed) or two categories; they are coded −1 and +1.

## Options
- **Highest interaction order (empty: all)**: limit the model to, say, 2 for main effects and two-factor interactions. Empty includes every interaction the design allows.
- **Confidence interval (%)**: 95.

## What you get
- An effects table: each effect (twice the regression coefficient on the coded factor, so it is the change from low to high), with Lenth's test. When the design leaves residual degrees of freedom, t tests too.
- Lenth's pseudo standard error (PSE), margin of error (ME) and simultaneous margin of error (SME). Effects beyond the ME are likely real; beyond the SME, very likely even after allowing for many effects.
- A half-normal plot of the absolute effects. Most effects fall on a line near zero; the ones that stand off it are the active effects.
- The intercept (grand mean). A term aliased with an earlier one (it can't be told apart from it in this design) is left out.

## Checked against
R's BsMD (Lenth's method) and lm.
