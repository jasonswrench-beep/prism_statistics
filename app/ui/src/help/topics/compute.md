# Compute Variable

Creates a new variable, or replaces an existing one's values, from a formula of other variables, such as an average of two measures, a log transform or a yes/no indicator.

## When to use it
- Transforming a variable: `log(income + 1)`, or `income / 1000` to change units.
- Combining variables: `(pre + post) / 2`, `post - pre`.
- Making an indicator: `ifelse(age >= 18, 1, 0)`.
- To score a questionnaire scale with reverse-scored items and a minimum number of answers, **Transform › Scale Score** is easier. To regroup values into categories, use **Recode**.

## How to run it
Choose **Transform › Compute Variable**. Type the **Target variable** name and the **Formula** in R's syntax, for example `(pre + post) / 2`, `log(income + 1)`, `ifelse(age >= 18, 1, 0)` or `rowmean(i1, i2, i3)`. The dialog lists the variable names you can use. Optionally give a **Label**. Choose **Compute**.

## Options
- **Label**: optional, the new variable's label.
- **Replace its values**: appears when the target variable already exists; it must be ticked before the values are replaced (the button then reads **Replace values**). The formula must give the same kind of values (numbers or text) as the variable already holds.
- **Paste**: adds the change to the script without making it.

## What you get
The new or changed variable, with a message saying how many cases got a value and how many were left missing.

User-missing codes count as missing, and a missing value in a formula makes the result missing, as in R. True and false become 1 and 0. A formula that names no variable gives every case the same value. A formula that gives numbers for some cases and text for others is refused.

The change is recorded like any other, so results that used the old values are marked out of date.

## Checked against
R: a computed variable matches the same formula evaluated in R (with ifelse and R's missing-value rules).
