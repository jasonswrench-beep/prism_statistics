# Test Scoring and Item Analysis

Scores a test's answers against a key and reports how each question worked: how hard it was, how well it separated stronger from weaker test-takers, and which wrong options drew whom. It also reports the test's reliability.

## When to use it
- After a multiple-choice or true-false test, to find questions to keep, revise or drop.
- With letter answers (A, B, C …), select-all answers (AC), or numeric codes, scored against a key you type.
- With points already earned on each question, such as a results file from OpenAssess (open the `.pstat` file it writes, then choose **Points already earned**).
- For a rating scale rather than a test, use **Reliability Analysis**; to model each question on a latent trait, use **Item Response Theory**.

## How to run it
Choose **Analyze › Scale › Test Scoring and Item Analysis**. Move the questions into **Items** in the order of the key, then type the key.

The key has one entry per item, separated by commas or spaces:
- `B` — the right answer.
- `D/B` — either D or B earns the points (a question with two right answers, or one adjusted after the test).
- `AC` — a select-all question: A and C must both be marked, and nothing else.
- `-` — drop the question: it is left out of every figure.
- `*` — credit everyone with the question, whatever they marked.

Answers are compared without regard to case or spaces, and the letters of a select-all answer in any order (`c, a` is the same as `AC`). A blank earns no points and is counted under Blank. A person who left every question blank is left out.

## Options
- **The items hold**: answers to score against a key (default), or points already earned. With points, an empty cell means the question does not count for that person, and the person is left out (listwise), as in Reliability Analysis.
- **Answer key**: as above.
- **Points per item**: optional, in the same order. Empty means one point each, or, with points earned, each question's highest score.
- **Upper and lower groups (%)**: 27 (default).

## What you get
- **Test summary**: the number of people and items, the points possible, the mean and SD of the total (SD uses N − 1), the lowest and highest totals, KR-20 (Cronbach's alpha when some questions give partial credit), KR-21 when every question is right-or-wrong and worth the same, and the standard error of measurement.
- **Item statistics**, for each question:
  - **Difficulty (p)**: the share of its points earned. Higher means easier.
  - **Point-biserial (total includes the question)**: its correlation with the total score, as an answer-sheet reader such as OpenAssess reports it.
  - **Corrected item-total r (total leaves the item out)**: its correlation with the total of the other questions. This is the fairer figure on a short test, because the first one partly correlates the question with itself.
  - The difficulty in the **upper** and **lower** groups (the top and bottom 27 percent by total, ties broken by case order) and **D**, the upper minus the lower.
  - KR-20 if the question were deleted, and the number of blanks.
- **Options by score group**: for each question, how many chose each option, overall and in the upper, middle and lower groups. A working distractor draws more of the lower group than of the upper group.
- Checks: fewer than 30 people (the figures are unstable), questions with a negative or low corrected item-total r, wrong options the upper group chose more than the lower group, and options nobody chose.

## Notes
KR-21 and the variances in KR-20 divide by N, describing the people tested; KR-20 is the same either way. A correlation with a question everyone got right, or everyone got wrong, is undefined and is left empty, never shown as 0.

## Checked against
R: psych's score.multiple.choice for difficulty and the point-biserial, base R for every other figure, and the shared item-analysis test set (C3) that OpenAssess is also checked against.
