# Add Variables

Merges another data file's variables into the open data, matching cases on one or more key variables that both files share, such as a participant ID.

## When to use it
- Measures for the same people are in separate files (a survey file and a lab file, or wave 1 and wave 2 kept side by side).
- One-to-many merges: adding household variables to every person in the household.
- To stack files with the same variables for different people, use **Add Cases**.

## How to run it
Choose **Data › Merge Files › Add Variables**. Choose **The other data file** (.csv, .xlsx, .sav, .dta, .sas7bdat or .jasp). Tick one or more **Key variables (in both files)**: the dialog offers the names the files share. Choose **Merge**.

A key may repeat in one file (one-to-many or many-to-one) but not in both. A case whose key is missing matches nothing.

## Options
- **Cases to keep**:
  - **Every case in this file** (default): unmatched cases get missing values for the new variables.
  - **Only cases with a match**.
  - **Every case in both files**: the other file's unmatched cases are added too.

## What you get
The other file's variables are added to the matching cases. When one file has a key several times, its matching cases each receive the other file's values. A variable whose name clashes with one already open gets the suffix `_2`. Variables keep their labels, value labels, measure and missing-value codes.

The change is recorded like any other, so results that used the old data are marked out of date. Save the project first if you may need the data as they were.

## Checked against
Base R: left, inner, all-cases and one-to-many merges match the datasets computed independently in R (with match).
