# Training Evaluation (Kirkpatrick Levels)

Reports a training program at each of Kirkpatrick's four levels in one result: how people rated it (Reaction), what they learned (Learning), what they do differently on the job (Behavior), and what changed for the organization (Results). Fill in the levels you measured; the others are left out.

## When to use it
- After a workshop, course or onboarding program with a reaction survey, a test before and after, and follow-up measures.
- With answer-sheet results from OpenAssess: open the class's `.pstat` file, add the second administration with Data › Add Cases (it takes `.pstat` files), and use the two-groups boxes for Learning.
- With xAPI statements from a learning record store: open the `.json` export (File › Open data), then Data › Aggregate by learner to get one row per person.
- For one before-and-after comparison on its own, the Paired-Samples T Test gives the same test with more options.

## How to run it
Choose **Analyze › Training Evaluation (Kirkpatrick Levels)**. Move variables into the boxes for the levels you have:
- **Level 1, Reaction: rating items** — the reaction survey's ratings.
- **Level 2, Learning: before and after** — pairs of scores for the same people, the earlier one first. Or, when the people can't be matched, **score of two groups** and **which administration**, with the codes for before and after below.
- **Level 3, Behavior** and **Level 4, Results** — before-and-after pairs, or single measures (such as a follow-up rating or an outcome after the training only).

To get started, open **File › Open Example Data › Training evaluation**: 60 employees measured at every level. The same dialog offers it as a blank template with the variables and their labels, ready for your own data.

## Options
- **Highest rating on the scale** and **Favorable from**: by default, the highest rating given and the top two points of the scale (4 and 5 on a five-point scale).
- **Test points possible**: gives the normalized gain at Level 2.
- **Code for before** and **Code for after**: for two groups.
- **Confidence interval (%)**: 95.

## What you get
- **Summary by level**: one line per measure, with its N.
- **Level 1**: each item's N, mean, SD and percent favorable; the scale mean (each person's mean rating) and Cronbach's alpha of the items.
- **Level 2**: before and after means and SDs, the mean gain with its confidence interval, the paired t test and Cohen's *d_z* (the mean gain over the SD of the gains). With the points possible, Hake's normalized gain, (after − before) / (possible − before): the share of the possible improvement that was made, for the means and as the mean of each person's. For two groups: Welch's t test and Cohen's *d* with the pooled SD.
- **Levels 3 and 4**: the same comparison for pairs, and N, mean, SD, lowest and highest for single measures.

Each level uses the cases complete on its own measures, so the levels can have different N.

## Notes
A rise from before to after shows change, not that the training caused it: other events at work, practice with the test and regression toward the mean move scores too. The report says so in its checks. A comparison group that didn't take the training is what separates the training's effect from the rest.

## Checked against
R: `t.test` (paired and Welch) and base R for the rest, on the Training evaluation example data.
