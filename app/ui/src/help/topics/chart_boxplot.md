# Box Plot

Summarizes numeric variables with boxes showing the median, quartiles, whiskers and outliers, either side by side or split by the categories of a grouping variable.

## When to use it
- To compare the spread and center of a score across groups at a glance.
- To spot outliers and skew before choosing an analysis.
- For the detailed shape of one distribution, a **Histogram** shows more. **Explore** gives box plots together with descriptive statistics and normality tests.

## How to run it
Choose **Graphs › Box Plot**. Put one or more numeric variables in **Variables**. Without a category variable you get a box for each variable in one chart. Optionally put a variable in **Category axis** to get a box per category, with a chart for each variable.

## Options
This dialog has no options.

## What you get
- A box plot drawn as an APA-style figure. The box runs from the first quartile (Q1) to the third (Q3), so it holds the middle half of the values, and the line inside it is the median. Whiskers reach the most extreme values within 1.5 interquartile ranges of the box; values beyond the whiskers are drawn as circles and flagged as outliers.
- A **Box plot values** table with N, lower whisker, Q1, median, Q3, upper whisker and the outlying values for each box.

A box that is off-center around its median, or one long whisker, suggests skew. Outliers are worth checking but not automatically wrong. Quartiles use R's default rule (type 7), and each variable uses its own valid values.

## Checked against
R's `boxplot.stats()` with type 7 quartiles.
