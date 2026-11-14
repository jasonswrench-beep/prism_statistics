# Factor Analysis

Explores how a set of numeric variables group together: it extracts a small number of factors (or principal components) that account for their correlations, and rotates them to make the pattern easier to read.

## When to use it
- Developing a scale, or finding out how many dimensions a set of items measures.
- Reducing many correlated variables to a few components.
- To test a structure you already expect, use **Analyze › Structural Equation Modeling › Factor Analysis of a Scale (CFA)**. For a scale's internal consistency, see **Reliability Analysis**.

## How to run it
Choose **Analyze › Dimension Reduction › Factor Analysis**. Move three or more numeric variables into **Variables**. Cases with every variable present are used.

## Options
- **Extraction**: Maximum likelihood (default), Principal axis factoring, Minimum residual, or Principal components (components, not factors).
- **Rotation**: Oblimin (default; oblique, factors may correlate), Promax (oblique), Varimax (orthogonal), or None.
- **Number of factors**: leave empty to let parallel analysis decide.
- **Parallel analysis**: on by default.
- **Scree plot**: on by default.
- **Hide loadings below**: 0; set it to, say, 0.3 to make the table easier to read (the loadings are still computed).

## What you get
- Adequacy: the Kaiser-Meyer-Olkin (KMO) measure, overall and per variable (above about .6 is usually adequate), and Bartlett's test of sphericity (a small p means the variables are correlated enough to factor).
- Eigenvalues with the percentage of variance and cumulative percentage, and with parallel analysis, the 95th percentile of eigenvalues from random data. Keep the factors whose eigenvalues beat the random ones.
- The scree plot.
- Communalities: how much of each variable's variance the factors explain.
- Loadings with their sums of squares; for oblique rotations, the pattern matrix, the structure matrix and the factor correlations. Loadings of about .4 or more are usually treated as meaningful.
- A Heywood-case warning if a communality reaches 1, a sign of too many factors or too few cases.

## Checked against
R's factanal and psych (fa), with GPArotation for oblimin.
