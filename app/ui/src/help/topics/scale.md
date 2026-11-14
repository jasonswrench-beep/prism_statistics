# Scale Score

Scores a questionnaire scale from its items: reverse-scores the items worded the other way, then takes the mean or sum of each case's answered items, as a new variable.

## When to use it
- Making a total or average score for a scale or subscale before analysis.
- Handling a few skipped items without losing the whole case.
- Check the scale's reliability first with **Analyze › Scale › Reliability Analysis**. For other formulas, use **Compute Variable**.

## How to run it
Choose **Transform › Scale Score**. Type the **New variable** name and tick the **Items** (at least two). Under **Reverse-scored items**, tick any that are worded the other way round, and give the **Possible answers from** and **to** values (1 and 5 by default). Choose **Score**.

## Options
- **Reverse-scored items**: none by default. A reversed answer becomes lowest + highest − answer, so on a 1 to 5 scale a 2 becomes a 4.
- **Possible answers from … to …**: the lowest and highest possible answers, used for reversing; 1 and 5. Use the possible range, not the range observed in your data.
- **Score**: **Mean of the answered items** (default), **Sum of the answered items**, or **Prorated sum: the mean times the number of items**. The prorated sum stays on the full scale's range when items are skipped; a plain sum does not.
- **Fewest items answered**: empty (every item must be answered) by default. A case answering fewer items gets a missing score.
- **Paste**: adds the change to the script without making it.

## What you get
The new score variable, with a message saying how many cases got a score and how many were left missing. The items themselves are not changed: reversal happens only inside the score. User-missing codes count as missing.

The change is recorded like any other, so results that used the old values are marked out of date.

## Checked against
R: mean and prorated-sum scores, with a reversed item and a minimum number of answered items, match rowMeans in R.
