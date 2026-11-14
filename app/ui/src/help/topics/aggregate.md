# Aggregate

Summarizes variables within groups, such as the mean score per school or the number of visits per patient. The result is either a new dataset with one row per group, or the group summaries added to every case.

## When to use it
- Moving from individuals to groups (students to classrooms, visits to patients).
- Adding a group mean to each case, for example to center a variable within groups.
- For group summaries in the output without changing the data, use **Analyze › Descriptive Statistics › Explore** or **Split File**.

## How to run it
Choose **Data › Aggregate**. Tick one or more **Break variables (groups)**; each combination of their values is a group. Then build the summaries: for each row choose a **Function**, the variable it applies to (**Of variable**) and a **New name**. Left empty, the name is the variable and function, such as `score_mean`. Use **Add a summary** for more rows. Choose **Aggregate**.

## Options
- **Function**: Mean, Sum, Median, Minimum, Maximum, Standard deviation, Number of valid values, Number missing, Number of cases, First value, or Last value.
- **Result**: A new dataset with one row per group (default; it replaces the data shown, so save first if you need it), or Add the summaries to every case.
- **Cases with a missing break value form a group of their own**: on by default.

User-missing codes count as missing.

## What you get
- With a new dataset: one row per group, sorted by the break values (missing last), holding the break variables and the summaries. A message gives the number of groups.
- With summaries added: the new variables appear on every case, each holding its group's value.

The change is recorded like any other, so results that used the old data are marked out of date.

## Checked against
Base R: all eleven summaries, as a new dataset and added to cases, match datasets computed independently in R (with tapply and ave).
