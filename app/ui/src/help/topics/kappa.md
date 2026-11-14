# Cohen's Kappa

Measures how well two raters agree when they sort the same cases into categories, after allowing for the agreement you would expect by chance.

## When to use it
- Two coders classified the same interviews, images or patients.
- With ordered categories (such as severity ratings), use linear or quadratic weights so near-misses count partly.
- For three or more raters, use **Fleiss' Kappa** or **Krippendorff's Alpha**; for numeric ratings, **Intraclass Correlation**.

## How to run it
Choose **Analyze › Scale › Cohen's Kappa**. Move the first rater's codes into **Rater 1** and the second rater's into **Rater 2**. Each case is one rated unit, and categories are numeric codes. Cases need both ratings.

## Options
- **Weights**: Unweighted (default), Linear, or Quadratic. The weighted forms are for ordered categories: a disagreement of one step costs less than a disagreement of three.
- **Confidence interval (%)**: 95.

## What you get
- **Cohen's kappa**: kappa with its asymptotic standard error (ASE) and confidence interval, a z test of whether agreement beats chance, the observed agreement, the agreement expected by chance, and N.
- A reminder about prevalence and bias: kappa falls when one category is very common or the raters use the categories at different rates, even when raw agreement is high, so read it alongside the observed agreement.

Kappa is 1 for perfect agreement and 0 for agreement no better than chance; it can be negative. A common rough guide (Landis and Koch) calls .41 to .60 moderate, .61 to .80 substantial and above .80 almost perfect.

The test uses the standard error under no agreement, and the interval the asymptotic standard error (Fleiss, Cohen and Everitt, 1969).

## Checked against
R's irr (kappa2) and psych (cohen.kappa).
