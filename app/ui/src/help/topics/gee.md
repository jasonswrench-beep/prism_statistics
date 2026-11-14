# Generalized Estimating Equations

Fits a regression for clustered or repeated observations (continuous, binary or count outcomes) that estimates population-averaged effects, with robust standard errors that stay valid even if the assumed correlation within clusters is wrong.

## When to use it
- Repeated measures on the same subjects, or people nested in families or clinics, with a binary, count or continuous outcome.
- You care about average effects in the population rather than effects within a particular subject.
- For subject-specific effects and variance components with a continuous outcome, use **Linear Mixed Model**.

## How to run it
Choose **Analyze › Mixed Models › Generalized Estimating Equations**. Move the outcome into **Dependent** (0/1 for binomial, counts for Poisson), numeric predictors into **Numeric predictors**, and categorical ones into **Categorical predictors** (dummy-coded). Move the subject or cluster identifier into **Cluster (subject)**. If occasions aren't simply in row order, move a whole-number occasion variable into **Occasion**.

## Options
- **Outcome**: Continuous (Gaussian, identity link; default), Binary (binomial, logit link: odds ratios), or Count (Poisson, log link: rate ratios).
- **Working correlation**: Exchangeable (default; the same correlation for every pair), AR(1) (decays with the distance between occasions), Unstructured (a correlation for each pair of occasions), or Independence.
- **Reference for** each categorical predictor: the first category by default.
- **Confidence interval (%)**: 95.

## What you get
- **Coefficients**: each estimate with its robust standard error, the model-based standard error alongside, a Wald chi-square test, p value and interval; for binary and count outcomes, odds ratios or rate ratios.
- **Working correlation and fit**: the estimated correlation, the scale parameter, and QIC, QICu and CIC for comparing working correlations (smaller is better).
- **Number of clusters** and their sizes, and a convergence note.

If the robust and model-based standard errors differ a lot, the working correlation may be a poor fit, but the robust ones remain trustworthy with enough clusters.

## Checked against
R's geepack (geeglm and QIC).
