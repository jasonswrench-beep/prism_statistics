# Select Cases

Limits the analyses you run from now on to the cases that meet a condition, such as adults only, or one site. The data themselves are not changed or deleted.

## When to use it
- To analyze a subgroup (for example `age >= 18 & site == "North"`).
- To leave out cases you've decided to exclude, without removing them from the file.
- To run the same analysis for every category of a variable, use **Split File** instead. To remove or reshape cases permanently, see **Aggregate** or **Restructure**.

## How to run it
Choose **Data › Select Cases**. A dialog opens that holds all three case settings (select, split and weight), with the focus on **Select cases**. Type a condition in R's syntax: `==` for equals, `!=` for not equal, `<`, `<=`, `>`, `>=`, `&` for and, `|` for or, and quotes around text values. Then choose **Apply**. Leave the box empty to use all cases.

## Options
- **Select cases**: the condition; empty (all cases) by default. Cases where the condition is false or missing are left out, and user-missing codes count as missing.
- **Clear all**: empties the select, split and weight settings together.
- **Paste**: adds the setting to the script without applying it.

## What you get
The status bar shows the current filter (or "No filter"). Every analysis run from now on uses only the selected cases, and its method note says which filter it ran with.

Each result keeps the settings it ran with, so changing the filter never makes earlier results out of date, and running an earlier analysis again uses its own cases. Clear the condition to go back to all cases.

## Checked against
Base R subsetting: an analysis on the selected cases matches R on `d[condition %in% TRUE, ]`.
