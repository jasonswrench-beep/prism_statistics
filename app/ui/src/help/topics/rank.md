# Rank Cases

Makes a new variable holding each case's rank on a variable: 1 for the smallest value, 2 for the next, and so on.

## When to use it
- To turn scores into ranks, for example to report a student's position in a class or to prepare data for a rank-based method by hand.
- To rank within groups, such as each student's rank within their own section.
- The rank-based tests (Mann-Whitney, Kruskal-Wallis, Spearman) rank the data themselves, so you don't need this for them.

## How to run it
Choose **Transform › Rank Cases…**. Tick the variables to rank; each makes a new variable `r_<name>`. Choose **Rank**.

## Options
- **Rank 1 goes to**: **The smallest value (default)** or **The largest value**.
- **Tied values get**:
  - **The mean of their ranks (default)**: two values tied for 2nd and 3rd both get 2.5.
  - **The lowest of their ranks**: both get 2.
  - **The highest of their ranks**: both get 3.
  - **Ranks in case order**: they get 2 and 3 in the order the cases come.
- **Rank within groups of**: optional. Ranks start again at 1 in each group. Cases with a missing group get no rank.

## What you get
New ordinal variables labeled "Rank of …". Empty cells and missing-value codes get no rank.

## Checked against
R's `rank(…, ties.method = "average" / "min" / "max" / "first", na.last = "keep")`.
