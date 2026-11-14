# Runs Test

The runs test checks whether a sequence of values is random, by counting runs of consecutive values on the same side of a cut point.

## When to use it
- To check that observations in time or entry order don't cluster or alternate too regularly, such as daily measurements or a quality-control series.
- Too few runs suggest trends or clustering; too many suggest values that swing back and forth.
- For autocorrelation at specific lags in a time series, the time series tools give more detail.

## How to run it
Choose **Analyze › Nonparametric Tests › Runs**. Put one or more numeric variables in **Test Variables**. Cases are taken in their file order, so sort the data first if the order matters.

## Options
- **Cut point**: **Median (default)**, **Mean**, or **A value**, given below.
- **Cut value**: only used when the cut point is a value.

## What you get
**Runs test**: for each variable,
- **Cut point**: the value used to split the series.
- **Cases below** and **Cases at or above** the cut point.
- **Runs**: the number of unbroken stretches on one side, and **Expected runs** if the order were random.
- **z** with **p (exact)** and **p (normal)**. The exact p value is the main result; the normal approximation is shown beside it.

A small p value says the sequence is unlikely to be random. Compare the observed and expected runs to see whether values cluster (fewer runs) or alternate (more runs).

## Checked against
R, by counting the runs and computing the test directly.
