# Intraclass Correlation

Measures how consistent numeric ratings or measurements are across raters (or instruments, or occasions), as the share of the total variation that comes from real differences between the rated subjects.

## When to use it
- Several judges score the same performances, or two devices measure the same people, and the ratings are numeric.
- Test-retest reliability of a measure.
- For categorical ratings, use **Cohen's Kappa**, **Fleiss' Kappa** or **Krippendorff's Alpha**.

## How to run it
Choose **Analyze › Scale › Intraclass Correlation**. Move one variable per rater into **Raters** (at least two). Each case is one rated subject. Cases need every rating, and at least two complete cases are needed.

## Options
- **Model**: Two-way (default; the same raters rate every subject) or One-way (different raters for different subjects).
- **Type**: Absolute agreement (default) or Consistency (two-way only). Absolute agreement counts a rater who is always two points higher as disagreeing; consistency only asks whether raters rank subjects the same way.
- **Unit**: Single rater (default) or Average of the raters. Use the average when your final scores will be the mean of the raters.
- **Confidence interval (%)**: 95.

## What you get
**Intraclass correlation**: the chosen form, named as McGraw and Wong (1996) name it, such as ICC(A,1) for two-way absolute agreement with a single rater, with its confidence interval and the F test of whether it exceeds zero (F, df1, df2, p).

A common guide (Koo and Li, 2016) reads values below .50 as poor, .50 to .75 moderate, .75 to .90 good and above .90 excellent; judge by the lower end of the interval as well as the estimate. The average-rater ICC is always at least as large as the single-rater one.

## Checked against
R's irr (icc), for every model, type and unit.
