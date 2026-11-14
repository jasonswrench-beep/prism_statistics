# Sort Cases

Puts the cases (rows) in order of one or more variables.

## When to use it
- To read the data in a meaningful order, such as by group and then by score, or to find the highest and lowest values.
- Before analyses that use the order of the cases, such as time series or the runs test, when the rows aren't already in time order.
- Most analyses don't depend on the order of the cases, so sorting doesn't change their results.

## How to run it
Choose **Data › Sort Cases…**. Choose the first variable to sort by and whether its **Smallest first** or **Largest first**. Choose **Add a variable** to break ties with another variable. Then choose **Sort**.

## What you get
- The cases are reordered in Data View, and the message says how many were sorted.
- Cases that tie on every sort variable keep their order.
- Empty cells and missing-value codes go last, whichever way that variable is sorted.
- Text sorts by character code, so capital letters come before lowercase ones and "B" sorts before "a".
- **Edit › Undo** puts the old order back. Results that depend on the order are marked out of date.

## Checked against
R's `order(…, method = "radix", na.last = TRUE)`; the exported R script sorts the same way.
