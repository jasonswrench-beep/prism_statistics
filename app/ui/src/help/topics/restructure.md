# Restructure

Changes the layout of the data between wide (one row per case, with repeated measures in separate variables) and long (one row per case and occasion).

## When to use it
- Wide to long: before a **Linear Mixed Model**, **Generalized Estimating Equations** or a long-format chart, which need one row per measurement.
- Long to wide: before **Repeated-Measures ANOVA** or a paired test, which need each measurement in its own variable.

## How to run it
Choose **Data › Restructure** and pick the **Direction**.

**Wide to long**: for each stack, give the new variable a name and tick its variables in order (for example score1, score2, score3 stacked into `score`). Use **Add another stack** for more; every stack needs the same number of variables (at least two). Set the **Index variable** name (default `index`), which holds the position 1, 2, 3, and the **Case identifier**: an existing variable, or a new one numbered 1, 2, … (default name `id`).

**Long to wide**: tick the **Identifier variable(s)** (one row each), choose the **Index variable**, whose values name the new variables (score_1, score_2, …), and tick the **Variables to spread** (none ticked: all the others).

## Options
- **Leave out rows whose stacked values are all missing** (wide to long): off by default.

## What you get
- Wide to long: one row per case and position, case by case, with the identifier, the index and each stacked variable. Variables not stacked are repeated on every row of their case.
- Long to wide: one row per identifier, sorted by identifier, with one variable per index value for each spread variable.

Variables keep their labels, value labels, measure and missing codes. The restructured data replace the data shown, so save first if you need the original. The change is recorded like any other, so results that used the old data are marked out of date.

## Checked against
Base R: wide to long (with and without dropping missing rows) and back to wide match datasets computed independently in R.
