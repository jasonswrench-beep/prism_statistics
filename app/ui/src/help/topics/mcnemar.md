# McNemar (2 related yes/no measures)

Tests whether the share of "yes" changes between two yes-or-no measures of the same cases, such as passing before and after a course.

## When to use it
- The same people answer the same yes/no question twice, or matched pairs give one answer each.
- Only the cases that changed (yes to no, or no to yes) carry information about change; McNemar compares those two counts.
- For two unrelated groups, use **Crosstabs**. For three or more yes/no measures of the same people, use **Cochran's Q**.

## How to run it
Choose **Analyze › Nonparametric Tests › McNemar (2 related yes/no measures)**. Add one or more pairs to **Paired Variables**. Both variables of a pair must use the same two codes.

## Options
This dialog has no options. The test is reported with the continuity correction, R's default, and the exact binomial p value beside it.

## What you get
- The two-by-two table of the first measure against the second; its footnote names the two counts of cases that changed.
- **McNemar test**: the chi-square with the continuity correction, df and two-sided p, and the exact binomial p value. A small p value says the share of "yes" changed.
- A **Cases that changed** note when fewer than 25 cases changed: the chi-square approximation is then rough, the exact p value is the better guide, and the written results use it.

A case is used when both measures in the pair are valid. Cases that answered the same way both times are reported but don't affect the test.

## Checked against
R's `mcnemar.test()` and `binom.test()` for the exact p value.
