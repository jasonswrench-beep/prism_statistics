# Response Surface

Fits a response-surface regression to an experiment with numeric factors, tests for curvature and lack of fit, and finds the stationary point: the settings where the fitted response is at a maximum, minimum or saddle.

## When to use it
- After a central composite or Box-Behnken design (see **Create Design**) when you want to optimize a process.
- A first-order model points the direction of steepest ascent during early exploration.
- For two-level screening designs, use **Factorial Effects**.

## How to run it
Choose **Analyze › Design of Experiments › Response Surface**. Move the measured outcome into **Response** and the factors, in coded units (such as −1 to +1), into **Factors**. If the design was run in blocks, move the block variable into **Block**; blocks are entered first.

## Options
- **Model**: Second order (default; linear, interactions and squares, and finds the stationary point) or First order (for steepest ascent).
- **Confidence interval (%)**: 95.

## What you get
- A coefficients table in coded units, with standard errors, t tests and intervals.
- A sequential analysis of variance: the first-order terms, two-way interactions and pure quadratic terms each tested in turn. Significant quadratic terms mean the surface curves.
- Lack of fit against pure error, when some settings were replicated. A small p value means the model misses something systematic.
- For a second-order model, the stationary point and its canonical analysis: the eigenvalues. All negative means a maximum, all positive a minimum, and mixed signs a saddle. A stationary point far outside the region you studied is an extrapolation.

## Checked against
R's rsm.
