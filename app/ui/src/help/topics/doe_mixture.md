# Mixture Model (Design of Experiments)

Fits a Scheffé mixture model to an experiment where the factors are proportions of a blend that add up to a fixed total, such as the ingredients of a recipe or a formulation.

## When to use it
- After a simplex lattice, simplex centroid or extreme-vertices design (see **Create Design**).
- When you want to know how each component and each blend of components affects the response.
- Ordinary regression doesn't fit here, because the components always sum to the same total; for independent numeric factors, use **Response Surface**.

## How to run it
Choose **Analyze › Design of Experiments › Mixture Model**. Move the measured outcome into **Response** and at least two components into **Mixture components**. Components can be proportions, or amounts with the same total in every run (they are rescaled to proportions).

## Options
- **Scheffé model**: Quadratic (default; pure components and the binary blending terms), Linear (linear blending only), or Special cubic (adds the ternary blending terms; three or more components).
- **Confidence interval (%)**: 95.

## What you get
- A coefficients table without an intercept, with standard errors, t tests and intervals. Each pure-component coefficient is the predicted response for that component alone. A positive blending coefficient (such as A×B) means the two together do better than their average (synergy); a negative one means they work against each other.
- Model fit: R² and the F test, relative to the mean of the response, as for an ordinary model, and lack of fit when blends were replicated.

## Checked against
R's lm without an intercept.
