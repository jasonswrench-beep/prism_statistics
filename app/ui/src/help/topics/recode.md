# Recode

Changes the values of numeric variables by rules, such as grouping ages into bands, collapsing categories, or turning a code into missing. The new values can go into new variables or replace the old ones.

## When to use it
- Collapsing a 7-point scale into three groups, or ages into bands.
- Swapping codes (1 becomes 2, 2 becomes 1), or turning a "don't know" code into a missing value.
- For a formula-based change, use **Compute Variable**. To reverse-score items within a scale score, use **Scale Score**.

## How to run it
Choose **Transform › Recode**. Tick one or more **Numeric variables**. Choose where the new values go. Then add **Rules, applied in order (the first that matches a value wins)**: for each, choose the old value and what it becomes. Choose **Recode**.

## Options
- **Where the new values go**: **Into new variables** (default; values no rule matches are left empty), with **New names**, one per variable, in order, separated by commas; or **Into the same variables** (values no rule matches stay as they are).
- **Old**: **Value** (one number), **Range** (from and to, both included; leave an end empty for lowest or highest), **Missing** (an empty cell or user-missing code), or **All other values**.
- **New**: **Value** (a number), **Missing (empty)**, or **Same value**.
- **Paste**: adds the change to the script without making it.

Because the first matching rule wins, put specific rules (single values, Missing) before broad ones (ranges, All other values).

## What you get
The recoded variables, with a message saying how many cases got a value. Recoding into new variables keeps the originals, so you can check the result, for example with **Analyze › Descriptive Statistics › Crosstabs** of old against new.

The change is recorded like any other, so results that used the old values are marked out of date.

## Checked against
R: a recode matches the same rules written with nested ifelse in R.
