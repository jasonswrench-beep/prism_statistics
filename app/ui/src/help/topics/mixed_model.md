# Linear Mixed Model

Fits a regression for data with groups, such as repeated measures on the same people or students within schools, by adding random effects that let each group have its own intercept and, optionally, its own slopes.

## When to use it
- Observations within a group are more alike than observations from different groups, so ordinary regression would understate the standard errors.
- You want to know how much of the variation lies between groups, or whether a predictor's effect varies by group.
- For population-averaged effects with robust standard errors, or for binary and count outcomes, use **Generalized Estimating Equations**.

## How to run it
Choose **Analyze › Mixed Models › Linear Mixed Model**. Move the continuous outcome into **Dependent**, numeric predictors into **Numeric fixed effects**, and categorical ones into **Categorical fixed effects** (dummy-coded; choose each one's reference). Move the variable that defines the groups (such as participant or school) into **Grouping variable**. To let a numeric predictor's slope vary by group, also move it into **Random slopes**.

## Options
- **Restricted maximum likelihood (REML)**: on by default. Turn it off for maximum likelihood when you want to compare models that differ in their fixed effects.
- **Reference for** each categorical fixed effect: the first category by default.
- **Confidence interval (%)**: 95.

## What you get
- **Fixed effects**: each coefficient with its standard error, t test using Satterthwaite's degrees of freedom, p value and confidence interval. They are read like ordinary regression coefficients.
- **Random effects**: the variance and SD of each random intercept and slope and of the residual, with the correlation between intercept and slope noted.
- **Model**: the number of observations and groups, the REML criterion (or −2 log-likelihood under ML) and AIC.
- A **Singular fit** note when a variance is estimated as exactly zero; the model may be more complex than the data support.

## Checked against
R's lme4 (lmer), with lmerTest for the degrees of freedom.
