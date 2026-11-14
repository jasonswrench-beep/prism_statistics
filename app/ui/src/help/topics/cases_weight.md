# Weight Cases

Applies frequency weights: each case counts as many times as its weight, so a case with weight 3 is treated as three identical cases. This is how you analyze data entered as a table of counts.

## When to use it
- Your data hold one row per combination of categories with a count column, such as a published contingency table.
- Each row stands for several identical observations.
- Frequency weights are not sampling weights. For survey data with sampling weights, strata or clusters, use **Analyze › Complex Samples › Survey Estimates** or **Survey Regression**, which give design-based standard errors.

## How to run it
Choose **Data › Weight Cases**. A dialog opens that holds all three case settings (select, split and weight), with the focus on **Weight cases**. Pick the numeric variable that holds the counts, then choose **Apply**.

## Options
- **Weight cases**: the weight variable; "Don't weight" by default. Weights must be whole numbers; a fractional weight is refused with a message naming the case. A case with a missing, zero or negative weight is left out.
- **Clear all**: empties the select, split and weight settings together.
- **Paste**: adds the setting to the script without applying it.

## What you get
The status bar shows "Weighted by" and the variable (or "No weights"). Every analysis run from now on gives the weighted result, exactly as if each case appeared as many times as its weight, and its method note says which weight it used.

Each result keeps the settings it ran with, so changing the weights never makes earlier results out of date. Choose "Don't weight" to go back to counting each case once.

## Checked against
Base R: weighted results match R run on the data with each row repeated by its weight.
