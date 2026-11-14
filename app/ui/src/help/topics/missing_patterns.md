# Missing Data Patterns

Describes the missing values in a set of variables: how much is missing in each, which combinations of missing values occur and how often, and whether the data could be missing completely at random.

## When to use it
- Before an analysis, to see how many cases listwise deletion would drop and where the gaps are.
- To decide whether to use multiple imputation (**Regression with Multiple Imputation**) or full-information estimation (**Missing values** in **Structural Equation Model**).
- For a broader check of the data's quality, see **Data › Data Health**.

## How to run it
Choose **Analyze › Missing Values › Missing Data Patterns**. Move the variables into **Variables**. User-missing codes count as missing, as they do in every analysis.

## Options
None.

## What you get
- **Missing values by variable**: the number and percentage missing in each variable.
- **Missing-data patterns**: each combination of present and missing variables that occurs, with the number of cases showing it. The pattern with nothing missing is the complete cases.
- **Little's MCAR test**: a chi-square test of whether the numeric variables' means differ across the missing-data patterns, computed from maximum likelihood means and covariances found by the EM algorithm.

A small p value for Little's test means the data are probably not missing completely at random (MCAR), so complete-case analyses may be biased. A large p value is consistent with MCAR but doesn't prove it. Data that are missing at random given other variables (MAR) are handled well by multiple imputation or full-information maximum likelihood.

## Checked against
R's naniar (mcar_test), with norm's EM estimates.
