# Fleiss' Kappa

Measures agreement among two or more raters who each sort the same cases into categories, after allowing for the agreement expected by chance.

## When to use it
- Several raters classified the same set of cases into nominal categories.
- With exactly two raters and ordered categories, **Cohen's Kappa** with weights may suit better.
- If some ratings are missing, or the ratings are ordinal, interval or ratio, use **Krippendorff's Alpha**. For numeric ratings, use **Intraclass Correlation**.

## How to run it
Choose **Analyze › Scale › Fleiss' Kappa**. Move one variable per rater into **Raters** (at least two). Each case is one rated unit, and categories are numeric codes. Cases need every rating.

## Options
None.

## What you get
**Fleiss' kappa**: kappa, a z test of whether agreement beats chance with its p value, the observed agreement, the agreement expected by chance, the number of raters, and N.

Kappa is 1 for perfect agreement and 0 for agreement no better than chance; it can be negative. A common rough guide (Landis and Koch) calls .41 to .60 moderate, .61 to .80 substantial and above .80 almost perfect, though what counts as enough depends on the stakes of the ratings.

Fleiss' kappa treats categories as unordered, so a near-miss counts the same as a far miss. Like other kappas, it falls when one category dominates, even when raw agreement is high, so report it with the observed agreement.

## Checked against
R's irr (kappam.fleiss).
