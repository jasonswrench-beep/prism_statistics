# Kaplan-Meier Survival

Estimates the survival curve: the probability that the event (death, relapse, dropout, a machine failing) has not yet happened by each time, using cases followed for different lengths of time. It can compare groups with the log-rank test.

## When to use it
- Your outcome is a time until an event, and some cases are censored (the event had not happened when follow-up ended).
- You want median survival times, or survival curves by group.
- To adjust for several predictors at once, use **Cox Regression**.

## How to run it
Choose **Analyze › Survival › Kaplan-Meier**. Move the follow-up time into **Time** and the variable that says whether the event happened into **Status**. To compare groups, move a grouping variable into **Factor**.

## Options
- **Status codes that mean the event happened**: 1 by default; give several separated by commas, such as `1, 2`. Every other code counts as censored.
- **Survival curves**: draws the curves; on by default.
- **Confidence interval (%)**: 95.

## What you get
- For each group (or all cases): the number of cases and events, and the median survival time with its confidence interval. A blank median means survival never fell below 0.5.
- The survival table: at each event time, the number at risk, the events, the survival probability, its standard error and interval.
- With a factor, the log-rank test: observed and expected events in each group and a chi-square test of whether the curves differ. A small p value means the groups' survival differs.
- The survival curves, which step down at each event time.

Standard errors are Greenwood's, and intervals are on the log scale.

## Checked against
R's survival package (survfit and survdiff).
