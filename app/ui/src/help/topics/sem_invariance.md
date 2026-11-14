# Measurement Invariance

Tests whether a measurement model works the same way in different groups, by fitting it with more and more parameters held equal and comparing each step with the one before.

## When to use it
- Before comparing groups (men and women, countries, conditions) on a scale's means or relations.
- To find which items behave differently across groups (partial invariance).
- To fit one multigroup model with your own constraints, use **Structural Equation Model** with a grouping variable.

## How to run it
Choose **Analyze › Structural Equation Modeling › Measurement Invariance**. Move the grouping variable into **Groups**, and type the model in **Measurement model** using lavaan syntax, one line per factor, such as `f1 =~ x1 + x2 + x3`.

## Options
- **Steps**: Configural; Metric (equal loadings); Scalar (and intercepts); Strict (and residual variances). All four are on; choose at least two.
- **Free in every step (partial invariance)**: parameters left free in every group, comma-separated, such as `x3 ~1`.
- **Identify factors by fixing their variances at 1**: off by default.
- **Estimator**: Maximum likelihood (default), Robust: Satorra-Bentler (MLM), or Robust: Huber-White and Yuan-Bentler (MLR).
- **Missing values**: Complete cases only (default) or Full-information ML.

## What you get
A table with one row per step: its chi-square, degrees of freedom, CFI and RMSEA, and the comparison with the step before, both as a chi-square difference (likelihood-ratio) test and as the change in CFI and RMSEA. With MLM or MLR the difference test is the Satorra-Bentler scaled one, and CFI and RMSEA are the robust versions.

A nonsignificant difference test, or a drop in CFI smaller than about .01, is usually read as support for that level of invariance. Metric invariance lets you compare relations between factors across groups; scalar invariance is needed before comparing factor means.

## Checked against
lavaan (group.equal constraints and lavTestLRT).
