# Binomial Test

An exact test of whether the proportion of cases in one category of a two-category variable equals a value you choose, such as .5.

## When to use it
- A yes/no, pass/fail or two-choice variable, and a known or hypothesized share to compare against.
- It is exact, so it works with small samples where chi-square approximations fail.
- For more than two categories, use **Chi-Square (goodness of fit)**. To compare two groups' proportions, use **Crosstabs**.

## How to run it
Choose **Analyze › Nonparametric Tests › Binomial**. Put one or more two-category variables, of any type, in **Test Variables** (two categories each).

## Options
- **Test proportion**: default 0.5. It applies to the first category, the one with the lowest code.
- **Confidence interval (%)**: default 95.

## What you get
**Binomial test**: for each variable, one row per category with
- **N** and the **Observed proportion**.
- **Test proportion**: the share you are testing against.
- A Clopper-Pearson confidence interval for the observed proportion. This exact interval is a little wider than simpler ones but never falls short of its stated level.
- **p (exact, two-sided)**: a small value says the observed share differs from the test proportion.

Check which category has the lowest code before setting a test proportion other than .5; the test is always stated for that first category.

## Checked against
R's `binom.test()`.
