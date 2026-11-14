# Add Cases

Appends the cases of another data file below the cases already open, matching variables by name. Use it to combine files with the same variables, such as data collected at different sites or in different waves.

## When to use it
- Two or more files hold the same measures for different people.
- To bring in new variables for the same people instead, use **Add Variables**.

## How to run it
Choose **Data › Merge Files › Add Cases**. Choose **The other data file**: a .csv, .xlsx, .sav, .dta, .sas7bdat or .jasp file. The dialog then lists the variables in both files, those only in the other file, and those only in the open data. Names are matched without regard to capitals. Choose **Add cases**.

## Options
- **Record the source file in a new variable**: optional. Give a name, and the new variable holds 1 for the cases already open and 2 for the added ones.

## What you get
- The other file's cases are added after the last case, and a message gives the number added and the new total.
- A variable found only in the other file is added, and is missing for the original cases. A variable found only in the open data is missing for the new cases.
- Value labels and missing-value codes from both files are combined, and variables keep their labels and measure.
- A variable that holds numbers in one file and text in the other can't be combined; the dialog names it, and you need to make the types match first.

The change is recorded like any other, so results that used the old data are marked out of date. Save the project first if you may need the data as they were.

## Checked against
Base R: the appended data, with a source variable, match the dataset computed independently in R (with rbind).
