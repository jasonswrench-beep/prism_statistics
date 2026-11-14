# Friedman (k related samples)

A rank-based test of whether three or more measures of the same cases differ, the nonparametric alternative to repeated-measures ANOVA.

## When to use it
- The same people rate or score several conditions, times or products, and the scores are ordinal or numeric but skewed.
- Each case's values are ranked among themselves, so stable differences between people don't matter.
- If the scores are roughly normal, **Repeated-Measures ANOVA** gives more detail. For two measures, use **Wilcoxon Signed-Rank**. For yes/no measures, use **Cochran's Q**.

## How to run it
Choose **Analyze › Nonparametric Tests › Friedman (k related samples)**. Put two or more numeric measures of the same cases in **Test Variables**.

## Options
This dialog has no options. Cases are used only when they have a value on every measure (listwise).

## What you get
- **Ranks**: for each measure, N, the median and the mean rank. Within each case the measures are ranked from lowest to highest, so a higher mean rank means that measure tends to score higher.
- **Friedman test**: N, the chi-square statistic, df and p, corrected for ties. A small p value says the measures don't all have the same typical level.
- **Kendall's W**: the effect size, from 0 (no agreement in how cases rank the measures) to 1 (every case ranks them the same way).

The test does not say which measures differ. To follow up, compare pairs with **Wilcoxon Signed-Rank** and adjust for the number of comparisons.

## Checked against
R's `friedman.test()`.
