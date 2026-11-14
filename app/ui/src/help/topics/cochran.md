# Cochran’s Q (k related yes/no measures)

Tests whether the share of "yes" differs across three or more yes-or-no measures of the same cases, such as whether students pass each of several tasks.

## When to use it
- The same people give a yes/no answer on several occasions or items, coded with the same two codes throughout.
- It extends McNemar's test to more than two measures.
- For two measures, use **McNemar**. For ranked or numeric measures, use **Friedman**.

## How to run it
Choose **Analyze › Nonparametric Tests › Cochran’s Q (k related yes/no measures)**. Put two or more numeric yes/no variables in **Test Variables**. They must all use the same two codes; the higher code is counted as "yes."

## Options
This dialog has no options. Cases are used only when every measure has a valid value (listwise).

## What you get
- **Cases coded ...**: for each measure, N, the count with the "yes" code and its percent. Compare these percentages to see where the differences lie.
- **Cochran's Q test**: N, Q, df (the number of measures minus 1) and p. Q is compared with the chi-square distribution. A small p value says the share of "yes" is not the same across all the measures.

The test doesn't say which measures differ. To follow up, compare pairs of measures with **McNemar** and adjust for the number of comparisons. If the variables have more than two distinct values, the test stops and tells you.

## Checked against
R, using the standard Cochran's Q formula.
