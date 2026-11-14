# Data Health

Checks the open data for common problems and summarizes every variable. The check itself never changes the data; a fix is applied only when you choose it, after a preview.

## When to use it
- Right after opening or importing a file, before any analysis.
- After merging files or entering data by hand.
- For a closer look at missing values across variables, use **Analyze › Missing Values › Missing Data Patterns**.

## How to run it
Choose **Data › Data Health**. The check runs at once. Findings are listed in two groups, the findings to review (or "Nothing needs review") and **Worth knowing**, with the first case numbers involved. Some findings have a **Preview** button.

## Options
There are no settings. Three findings come with a fix:
- **Convert to numbers**: for a variable of numbers stored as text. Text that isn't a number becomes empty, and the preview lists it.
- **Mark as missing codes**: for common codes such as −99, −9, 99, 999 or 9999 sitting far outside a variable's other values and not marked as missing. The values stay in the data; they just count as missing.
- **Set measure to nominal**: for a variable that looks like an identifier but is measured as scale.

Each preview shows a summary and, case by case, the value now and what it becomes. Choose **Apply** to make the change, or **Cancel**.

## What you get
- Findings such as empty or constant variables, numbers stored as text, likely missing-value codes, values without a label in a labeled variable, duplicate identifiers, cases that repeat an earlier case exactly, identifiers measured as scale, and category spellings that differ only in capitals or spaces.
- A **Variables** table: each variable's type, measure, valid count, empty cells, user-missing count, number of unique values, minimum and maximum.

Outliers, duplicate cases and spellings are reported, never removed or merged. When you apply a fix, the change is recorded like any other, so results that used the old data are marked out of date.
