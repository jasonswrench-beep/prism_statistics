# Working with data

## Data View

Each row is a case (a person, a school, a day) and each column is a variable.

Data View opens **browse-only**, so a stray keystroke can't change your data. Turn **Browse only** off to edit: double-click a cell (or select it and press Enter), type, and press Enter. The change is recorded like any other operation, so results that used the old value are marked out of date. **Tools › Settings** sets whether Data View opens browse-only and whether it shows value labels.

You can paste a block of cells copied from a spreadsheet. PrismStatistics shows a preview first and flags any value that doesn't fit its variable, such as text in a numeric column. It asks before adding new cases.

## Finding a value or a case

- **Edit › Find…** (Ctrl+F) looks for a value in every variable or in one. Numbers match as Data View shows them, and missing-value codes are found too. With **Show value labels** on, a label such as "Agree" matches its code. Tick **Whole cell** to match only whole values (so 1 doesn't find 10). The arrows step through the matches, and the cell is outlined in amber.
- **Edit › Go to Case or Variable…** (Ctrl+G) jumps to a case number, a variable, or both.

## Undoing a change

**Edit › Undo** (Ctrl+Z) takes back the last change, and the menu names it, such as **Undo Recode**. **Edit › Redo** (Ctrl+Y) puts it back. Up to 30 changes can be undone. This covers:

- editing and pasting cells
- changes in Variable View
- Data Health fixes
- Compute, Recode and Scale Score
- the Data menu's commands
- Select, Split and Weight Cases
- saving regression values as variables
- running a script
- clearing the output

An undone change is also taken out of the project's record, so the history and any script made from it never repeat it. Results you ran after the change stay in the output. If their numbers depend on the change, they are marked **Out of date**. Undoing a script also removes the results the script made.

Running an analysis only adds a result, so it isn't on the list. To keep a result out of your report, leave it out in the Report Builder. Opening other data or another project starts the list again. In a text box, Ctrl+Z and Ctrl+Y undo your typing as usual.

## Variable View

Variable View has one row per variable, with its name, label, type, measure, value labels and missing-value codes.

- **Measure** decides how a variable is offered in dialogs. Nominal and ordinal variables can define groups. Scale variables are measured quantities.
- **Value labels** name codes (1 = "Agree"). Results show the labels.
- **Missing values** are codes that mean "no answer" (−99, 999). Every analysis leaves them out.

## Cleaning and reshaping

- **Data › Data Health…** finds problems such as numbers stored as text, likely missing-value codes, identifiers measured as scale, spellings that differ only in case or spaces, and duplicate cases. You see a preview before any fix is applied, and nothing is removed or merged unless you choose it.
- **Transform › Compute Variable…, Recode… and Scale Score…** make new variables.
- **Transform › Reverse Score…** reverses items worded the other way round, **Rank Cases…** turns values into ranks, **Standardize (z-scores)…** makes z-scores, and **Date and Time…** makes years, months, weekdays or ages from dates stored as text or as other programs' date numbers.
- **Data › Sort Cases…** puts the cases in order of one or more variables.
- **Data › Merge Files › Add Cases…** appends another file's cases. **Add Variables…** joins another file's variables by a key.
- **Data › Aggregate…** summarizes cases into groups.
- **Data › Restructure…** turns wide data long, or long data wide.

## Choosing the cases an analysis uses

- **Data › Select Cases…** keeps only the cases that meet a condition.
- **Data › Split File…** repeats each analysis for every group.
- **Data › Weight Cases…** weights cases by a variable (frequency weights).

These settings show as chips above the output. Each analysis records the settings it ran with, so earlier results keep theirs.

## Results that are out of date

Results remember the data they were made from. Sometimes the data change afterwards, for example through an edited cell or a new missing-value code. The affected results are then marked **Out of date**. Their **Run again** button repeats the analysis with the same settings.
