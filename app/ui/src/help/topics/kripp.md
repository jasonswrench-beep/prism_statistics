# Krippendorff's Alpha

Measures agreement among any number of raters, at any level of measurement, and copes with missing ratings: a unit rated by only some of the raters still counts.

## When to use it
- Content analysis or coding projects where not every coder rated every unit.
- Ratings that are ordinal, interval or ratio, not just categories.
- For two raters with complete nominal codes, **Cohen's Kappa** is the familiar choice; for numeric ratings by the same raters, **Intraclass Correlation** is common too.

## How to run it
Choose **Analyze › Scale › Krippendorff's Alpha**. Move one variable per rater into **Raters** (at least two). Each case is one rated unit; ratings are numeric codes, and missing ratings are allowed. Every unit with two or more ratings is used.

## Options
- **Level of the ratings**: Nominal (default), Ordinal, Interval, or Ratio. This sets how much each kind of disagreement counts: for nominal ratings any difference counts fully; for interval ratings, larger differences count more.

## What you get
**Krippendorff's alpha**: alpha, the metric used, the number of units used, the number of pairable values (ratings that could be compared with another rating of the same unit), and the number of raters.

Alpha is 1 for perfect agreement and 0 for agreement no better than chance; it can be negative when disagreement is systematic. Krippendorff suggests relying on data with alpha of .80 or more, and drawing only tentative conclusions from .667 to .80.

Alpha is computed from the coincidence matrix of the ratings, so the raters don't need to rate the same units.

## Checked against
R's irr (kripp.alpha), for every level of measurement.
