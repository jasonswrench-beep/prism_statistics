# Split File

Runs every analysis separately for each category of a variable, such as each country, each sex or each wave, so you get one set of results per group in a single run.

## When to use it
- To repeat the same analysis in each group and compare the results side by side.
- To analyze only one group, use **Select Cases** instead.
- To test whether groups differ, use a test that compares them directly, such as **Analyze › Compare Means › One-Way ANOVA**; split results show each group's own estimates, not a test of the difference.

## How to run it
Choose **Data › Split File**. A dialog opens that holds all three case settings (select, split and weight), with the focus on **Split file**. Pick the variable whose categories define the groups, then choose **Apply**.

## Options
- **Split file**: the grouping variable; "Don't split" by default.
- **Clear all**: empties the select, split and weight settings together.
- **Paste**: adds the setting to the script without applying it.

A split can be combined with **Select Cases** and **Weight Cases** in the same dialog.

## What you get
The status bar shows "Split by" and the variable (or "No split"). Every analysis run from now on gives a separate result for each category of the split variable, and its method note says it was split.

Each result keeps the settings it ran with, so changing the split never makes earlier results out of date. Choose "Don't split" to go back to analyzing all cases together.

## Checked against
Base R: each group's result matches R run on that group's cases alone.
