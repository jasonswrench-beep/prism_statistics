# Create an Experimental Design

Builds a designed experiment, a run sheet of factor settings for each run, and opens it as the data, with an empty response column to fill in as the runs are done.

## When to use it
- Planning a screening, factorial, response-surface or mixture experiment. Analyze the results with **Factorial Effects**, **Response Surface** or **Mixture Model**.

## How to run it
Choose **Analyze › Design of Experiments › Create Design**; no data needs to be open. Pick a **Design**, set the **Number of factors** (or components, 1 to 15), and give each a name with its **Low setting** and **High setting** (or **Lower limit** and **Upper limit** for mixtures). Leave settings empty to use coded units (−1 and +1). Then choose **Create design**. The design replaces the open data; you are offered the chance to save first.

## Options
- **Design**: Two-level fractional factorial (default; minimum aberration), Plackett-Burman (12 to 48 runs), Full factorial, Central composite (2 to 6 factors), Box-Behnken (3 to 5 factors), Simplex lattice, Simplex centroid, Extreme vertices (constrained mixture), or Optimal (D or I).
- **Runs**: for fractional factorials (8 by default, with the fraction shown), Plackett-Burman (12), and optimal designs (at least the number of model terms).
- **Levels**: for full factorial and optimal designs.
- **Degree m**: for simplex lattices; 2.
- **Criterion** (D-optimal for precise coefficients, I-optimal for precise predictions) and **Model** (Quadratic, Main effects and two-factor interactions, or Main effects) for optimal designs.
- **Center points**: 0 (for central composite designs, per cube and axial block, with **Axial distance**: Orthogonal blocks by default, Rotatable or Face-centered).
- Mixtures: add the centroid and axial check blends (or edge midpoints for extreme vertices).
- **Replicates**: 1. **Response column**: "response".
- **Randomize the run order (within blocks)**: on, with **Seed** 20260101.

## What you get
The run sheet as the new data: run, standard order, block, the factor settings and an empty response column. The output describes the design; for fractional factorials, its resolution and alias structure (which effects can't be told apart).

## Checked against
R's FrF2, DoE.base, rsm and AlgDesign.
