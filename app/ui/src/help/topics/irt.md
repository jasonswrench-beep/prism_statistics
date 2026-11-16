# Item Response Theory

Fits an item response model to test or questionnaire items: it places each person on a latent trait and describes each item by how well it discriminates and where on the trait it is hardest or easiest to endorse.

## When to use it
- Analyzing a test or scale item by item: which items are informative, and at what trait levels.
- Items scored right or wrong (two categories) or on ordered rating scales.
- For a quick reliability summary, use **Reliability Analysis**; to group people rather than place them on a continuum, use **Latent Class Analysis**.

## How to run it
Choose **Analyze › Scale › Item Response Theory**. Move at least two items into **Items**. Each item needs two or more ordered categories; cases with every item answered are used.

## Options
- **Model**: 2PL / graded response (default; a discrimination for each item), or Rasch (two-category items; equal discriminations).
- **Confidence interval (%)**: 95.

## What you get
- Model summary: the model fitted (two-parameter logistic, graded response or Rasch), the log-likelihood, the number of free parameters, AIC and BIC, and whether the estimates converged.
- An item table: each item's discrimination a with its standard error, and its difficulty b (two categories) or thresholds b₁, b₂, … (ordered categories), with standard errors. Higher a means the item separates people more sharply; b is the trait level where a person has a 50% chance of answering correctly, or of reaching the next category. In the Rasch model every discrimination is fixed at 1 and the variance of the trait is estimated.
- Item characteristic curves (or expected item scores for ordered items) across the trait.
- The test information curve: where on the trait the test measures most precisely.
- **Limited-information model fit**: M2 (M2* for ordered items), its df and p value, RMSEA with its 90% interval, SRMSR (the average difference between the observed and the model's correlations), and TLI and CFI against a model in which the items are independent. M2 needs more means and cross-products than parameters: four items with four categories each have too few, and the table says it could not be computed.
- **Item fit**: S-X² for each item, with its df, RMSEA and p value. It compares the answers to the item at each summed score with what the model expects, pooling sparse cells as mirt does. The p values are not adjusted for testing every item, so expect about one in twenty to fall below .05 by chance.

## Checked against
R's mirt: the estimates, and its M2() and itemfit() for the fit statistics.
