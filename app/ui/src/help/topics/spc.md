# Control Charts (Quality Control)

Draws control charts: a process measure plotted over time against a center line and control limits, flagging points and runs that suggest the process has changed, with optional capability indices.

## When to use it
- Monitoring a process to tell ordinary variation from a real shift, and checking it against its specification limits.

## How to run it
Choose **Analyze › Quality Control › Control Charts** and pick a **Chart**. The variable slots change with the chart:
- X̄ and R, X̄ and S: one **Measurement** variable with a **Subgroup** variable, or one variable per measurement in a subgroup (one row per subgroup).
- Individuals and moving range: one **Measurement**.
- p, np, c and u: the **Count** of defectives or defects, and for p, np and u the **Sample size**.
- CUSUM and EWMA: one **Measurement**, with an optional **Subgroup**.

## Options
- **Chart**: X̄ and R (default), X̄ and S, Individuals and moving range, p, np, c, u, CUSUM, or EWMA.
- **Limits at ± this many standard deviations**: 3.
- **Flag runs of this many points on one side (0: off)**: 7.
- CUSUM: **Decision interval h (standard errors)** 5, **Shift to detect (standard errors)** 1, **Head start** 0.
- EWMA: **Smoothing weight λ (0 to 1)** 0.2.
- CUSUM and EWMA: **Target mean** and **Process standard deviation**, estimated from the data when left empty.
- X̄ and individuals charts: **Lower specification limit**, **Upper specification limit** and **Target** for capability; all optional.

## What you get
- The chart or charts, with the center line, the control limits, and points beyond the limits or in long runs marked.
- For each chart, a table of every sample's value, limits and flag, and a summary of the control limits and the estimated process standard deviation.
- A statistical-control check: either no point is flagged, or the flagged points to look into for special causes.
- With specification limits: Cp, Cp_l, Cp_u, Cpk and Cpm with intervals, Pp and Ppk, and the expected and observed shares outside the limits. A Cpk of 1.33 or more is a common target.

## Checked against
R's qcc.
