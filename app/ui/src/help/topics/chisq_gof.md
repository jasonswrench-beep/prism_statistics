# Chi-Square (goodness of fit)

Tests whether the counts in the categories of one variable match the proportions you expect, either equal shares or proportions you give.

## When to use it
- To check whether a sample matches known population shares (for example, the regional mix of a census).
- To test whether choices are spread evenly across options.
- To compare two categorical variables with each other, use **Crosstabs**. For a variable with only two categories and a single expected share, the **Binomial** test is exact.

## How to run it
Choose **Analyze › Nonparametric Tests › Chi-Square (goodness of fit)**. Put one variable of any type in **Test Variable**.

## Options
- **Expected proportions**: leave empty for equal proportions (the default), or give one number per category in code order, such as 1, 2, 2. The numbers are rescaled to add to 1, so you can enter counts, percentages or ratios.

## What you get
- **Observed and expected counts**: each category's observed N and expected N, both as percentages, and the residual (observed minus expected). Large residuals show which categories are over- or under-represented.
- **Chi-square test**: the chi-square statistic, df (the number of categories minus 1), p and Cohen's w as the effect size. A small p value says the counts don't fit the expected proportions.

Small expected counts are flagged, and categories are never merged automatically. User-missing codes and empty cells are left out.

## Checked against
R's `chisq.test(table(x), p = ...)`.
