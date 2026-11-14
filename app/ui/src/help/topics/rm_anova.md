# Repeated-Measures ANOVA

Compares two or more measures of the same cases, such as scores at three time points, and optionally crosses them with a between-subjects factor (a mixed ANOVA).

## When to use it
- Every case has a value on each measure, and the measures are levels of one within-subjects factor (time, condition, task).
- Add a between-subjects factor to ask whether change over time differs between groups.
- For only two measures, the **Paired-Samples T Test** gives the same answer more simply. For ranked or skewed data, use **Friedman**. If many cases are missing a measure, a linear mixed model handles incomplete cases better.

## How to run it
Choose **Analyze › Compare Means › Repeated-Measures ANOVA**. Put the measures in **Measures** in order; they become the levels of the within-subjects factor. Optionally add a **Between-subjects factor**.

## Options
- **Within-subjects factor name**: default "time"; used in the output labels.
- **Correction reported**: **Greenhouse-Geisser (default)**, **Huynh-Feldt**, or **None (sphericity assumed)**.
- **Paired comparisons of the measures**: **None (default)**, **Bonferroni** or **Holm**.
- **Profile plot**: off by default.
- **Confidence interval (%)**: default 95.

## What you get
- **Descriptive statistics** for each measure (and group).
- **Mauchly's test of sphericity** with the Greenhouse-Geisser and Huynh-Feldt epsilons. Sphericity means the differences between every pair of measures have similar spread; when epsilon is well below 1, the corrected test is the safer one.
- **Tests of effects**: F, the (corrected) degrees of freedom, p, partial eta squared and generalized eta squared for the within factor, the between factor and their interaction.
- **Means across** the within factor, paired comparisons and a profile plot when asked.

Cases are used only when every measure (and the between factor) is valid.

## Checked against
R's `car::Anova()` on the multivariate linear model, and afex's `aov_ez()`.
