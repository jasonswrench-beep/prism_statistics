# Frequencies

Counts how often each value of a variable occurs, with percentages, so you can see the shape of categorical or short-scale data at a glance.

## When to use it
- To describe categorical variables such as condition, sex, region or a Likert item.
- To check codes before an analysis: stray values, typos and how many answers are missing.
- For numeric variables with many distinct values, **Descriptives** or **Explore** give a more useful summary.

## How to run it
Choose **Analyze › Descriptive Statistics › Frequencies** (or the Frequencies button on the toolbar). Select variables in **Variables in the data** and press the arrow to move them into **Variables**. Click a chosen variable to move it back. Any variable type can be used. Press **Run**, or **Paste** to add the command to your script without running it.

## Options
This dialog has no options. User-missing codes and empty cells are always listed as missing, never as categories. **Reset** clears the Variables box.

## What you get
One table per variable. Each row is a value, with its value label, then:
- **Count**: how many cases have that value.
- **Percent**: the share of all cases, including the missing ones.
- **Valid percent**: the share of the cases with a valid answer. This is usually the number to report.
- **Cumulative percent**: the running total of valid percent, handy for ordered scales ("62% answered 3 or lower").

Below the valid values, each user-missing code and the empty cells appear as their own rows, so you can see exactly why cases drop out of later analyses.

## Checked against
R's `table()` and the percentages computed from it, with the same missing-value exclusions written out by hand.
