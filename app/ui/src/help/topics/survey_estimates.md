# Survey Estimates

Estimates means, proportions or totals from survey data, with standard errors that account for the sampling weights, strata and clusters of a complex sample design.

## When to use it
- Your data come from a stratified or cluster sample with sampling weights, such as a national survey.
- You want population estimates rather than sample summaries.
- For ordinary unweighted summaries, use **Analyze › Descriptive Statistics › Descriptives** or **Frequencies**. Note that **Data › Weight Cases** sets frequency weights (whole-number counts), which are not the same as sampling weights.

## How to run it
Choose **Analyze › Complex Samples › Survey Estimates**. Move the variables to estimate into **Variables** (categorical ones give the proportion in each category), the weight into **Sampling weight**, and, if the design has them, the stratum variable into **Strata** and the primary sampling unit into **Clusters (PSUs)**.

## Options
- **Estimate**: Means and proportions (default), or Totals.
- **Confidence interval (%)**: 95.

## What you get
A table of weighted estimates with design-based standard errors and confidence intervals, and a note giving the weight, the number of PSUs and strata, and the design degrees of freedom (PSUs minus strata), which the intervals use.

Variances are computed by linearization with the "ultimate cluster" method, treating first-stage sampling as with replacement (no finite population correction). With no clusters, each case is its own PSU; with no strata, there is a single stratum. Design-based standard errors are usually larger than naive ones when cases within a cluster resemble each other.

## Checked against
R's survey package (svydesign, svymean and svytotal).
