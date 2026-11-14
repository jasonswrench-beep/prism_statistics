# Standardize (z-scores)

Makes a z-score for each chosen variable: how many standard deviations each value is above or below the mean.

## When to use it
- To put variables measured on different scales on a common one before comparing or combining them.
- To find unusual values: a z beyond about ±3 is far from the rest of the data.
- To center and scale predictors, so that a regression coefficient is per standard deviation.

## How to run it
Choose **Transform › Standardize (z-scores)…**, tick the variables (each makes `z_<name>`), and choose **Standardize**.

## What you get
- New scale variables labeled "z-score of …", with z = (value − mean) / SD.
- The mean and the sample standard deviation (n − 1) come from the cases with a valid value. Missing values stay missing.
- A z-score's mean is 0 and its SD is 1.
- A variable whose valid values are all the same has no spread, and isn't standardized.

## Checked against
R's `scale()`.
