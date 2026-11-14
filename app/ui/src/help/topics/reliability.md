# Reliability Analysis

Measures the internal consistency of a set of scale items with Cronbach's alpha and McDonald's omega, and shows how each item fits with the rest of the scale.

## When to use it
- Reporting the reliability of a questionnaire scale or subscale.
- Deciding whether an item should be dropped or was reverse-worded.
- To create the scale score itself, use **Transform › Scale Score**. To check a scale's factor structure, use **Factor Analysis** or **Factor Analysis of a Scale (CFA)**.

## How to run it
Choose **Analyze › Scale › Reliability Analysis**. Move items scored in the scale's direction into **Items**, and items worded the other way round into **Reverse-scored items**; both are part of the scale. You need at least two items in total. Reversing happens only within this analysis; the data don't change.

## Options
- **Lowest possible score**: used to reverse items; leave empty to use the lowest value found.
- **Highest possible score**: leave empty to use the highest value found. A reversed item becomes (lowest + highest) − value.
- **Confidence interval (%)**: 95.

## What you get
- **Reliability**: Cronbach's alpha with Feldt's confidence interval, standardized alpha, McDonald's omega (with three or more items), the mean inter-item correlation, the number of items and N. Alpha of about .70 or more is often called acceptable and .80 good, though it depends on the purpose.
- **Item statistics**: each item's mean, SD, corrected item-total correlation (its correlation with the sum of the other items) and alpha if that item were deleted. An item whose deletion raises alpha noticeably may not belong.
- **Item direction**: a check that every item correlates positively with the rest. An item with a negative correlation is flagged, never reversed automatically; it may need reverse scoring.

Cases missing any item are left out (listwise).

## Checked against
R, with Cronbach's alpha and the item statistics computed directly.
