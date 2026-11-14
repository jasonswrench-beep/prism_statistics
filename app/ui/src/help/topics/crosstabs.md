# Crosstabs

Cross-tabulates two categorical variables and tests whether they are related, with effect sizes and optional exact and ordinal statistics.

## When to use it
- To see whether two categorical variables are associated, such as condition by pass or fail.
- To compare percentages across groups.
- To check one variable against a set of expected proportions instead, use **Chi-Square (goodness of fit)**. For two yes/no measures of the same people, use **McNemar**.

## How to run it
Choose **Analyze › Descriptive Statistics › Crosstabs**. Put one variable in **Row** and one in **Column**. Optionally add a **Layer** variable to run the Cochran-Mantel-Haenszel test across its categories (it needs a 2 × 2 table in each layer).

## Options
- **Cell percentages**: **Row** (default), **Column**, **Total**.
- **Chi-square test of independence**: on by default, with Cramér's V (and phi for a 2 × 2 table).
- **Continuity correction (2 × 2 only)**: off by default; stated in the output when on.
- **Expected counts** and **Adjusted residuals**: off by default.
- **Fisher's exact test (2 × 2 only)**: off by default; use it when expected counts are small.
- **Risk estimates (2 × 2 only)**: odds ratio and relative risks with 95% intervals; off by default.
- **Bayes factor for association**: off by default.
- **How the table was sampled (for the Bayes factor)**: **Only the total fixed** (default), **Row totals fixed**, **Column totals fixed**, or **Nothing fixed (Poisson)**.
- **Ordinal measures**: gamma, tau-b, tau-c and Somers' d, for ordered categories; off by default.

## What you get
The table of counts with your chosen percentages, the **Chi-square tests** and the **Effect size**. A small p value means the variables are related; Cramér's V says how strongly (0 is none, 1 is perfect). Small expected counts are flagged, and categories are never merged for you. With a layer you also get the tests across layers and the table within each layer.

## Checked against
R's `chisq.test()`, `fisher.test()` and `mantelhaen.test()`, DescTools for the ordinal measures and the Breslow-Day test, and BayesFactor's `contingencyTableBF()`.
