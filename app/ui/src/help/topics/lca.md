# Latent Class Analysis

Finds hidden groups (latent classes) of people who answer a set of categorical questions in similar ways, and estimates how large each class is and how its members tend to answer.

## When to use it
- Your indicators are categorical (yes/no items, symptom checklists, multiple-choice answers) and you suspect distinct types of respondents.
- You want to assign each case to its most likely class.
- For continuous indicators, use **Latent Profile Analysis**. For a continuous trait measured by items, consider **Analyze › Scale › Item Response Theory** or **Factor Analysis**.

## How to run it
Choose **Analyze › Classify › Latent Class Analysis**. Move two or more categorical variables into **Indicators**. Cases with every indicator answered are used.

## Options
- **Number of classes**: 2 by default (1 to 20).
- **Also compare 1 to that many classes (BIC)**: fits every number of classes up to the one chosen and compares them; off by default.
- **Random starts**: 30 by default. Each start begins the estimation from a different random place, and the best result is kept; more starts guard against local maxima.
- **Seed**: 20260101, so the same starts are used each time.

## What you get
- The class shares (how large each class is), largest first, with standard errors.
- For each indicator and class, the probability of each response category, with standard errors. These describe what each class is like.
- Fit: the log-likelihood, number of free parameters, AIC and BIC, G² and Pearson chi-square with their residual degrees of freedom, and the relative entropy (near 1 means the classes are clearly separated).
- How many starts reached the best log-likelihood. If only a few did, try more starts.
- With the comparison on, a table of fit by number of classes. A lower BIC is better, but also ask whether the classes make substantive sense.

Cases are assigned to their most probable class.

## Checked against
R's poLCA.
