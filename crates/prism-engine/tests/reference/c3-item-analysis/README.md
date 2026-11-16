# Item-analysis test set (C3)

**Set version 1.0.0, 2026-11-15.**

OpenAssess (TypeScript) and PrismStatistics (Rust) both compute item statistics. They share no code. Instead, both run
this set, and neither ships a release that breaks it. Each case is a class's scored answer sheets. Every figure in
`reference.json` was computed in base R from the case files alone, so neither tool is checked against the other.

## What is here

| Path | What it is |
|---|---|
| `cases/<name>/scores.csv` | One row per complete sheet: `sheet`, `version`, then the points earned on each question. An empty cell means the question does not count on that sheet, which is not the same as 0. |
| `cases/<name>/case.json` | What each question is worth and whether it gives partial credit. |
| `cases/<name>/<name>.pstat` | The same sheets as the `.pstat` file OpenAssess sends to PrismStatistics (profile `OpenAssess item results 1`, [PSTAT-1](../../specs/pstat/PSTAT-1.md) section 7.1). |
| `reference.json` | R's figures for every case, named as below. |
| `make_cases.R` | Made the case files once. They are kept, so a change in R's random numbers cannot move a reference value. |
| `make_reference.R` | Computes `reference.json` from the case files (`Rscript make_reference.R`). Base R only. |
| `make_pstat.mjs` | Writes the `.pstat` files from the case files (`node make_pstat.mjs`). |
| `check/check_openassess.mjs` | Runs OpenAssess's item statistics on every case and compares. |
| `check/check_prismstatistics.mjs` | Runs PrismStatistics's Reliability and Descriptives on every `.pstat` file and compares. |
| `check/check_set.mjs` | Checks the set itself: every `.pstat` file is what `make_pstat.mjs` makes, follows PSTAT-1, and every file matches `SHA256SUMS`. |

## The cases

| Case | Sheets | What it tests |
|---|---|---|
| `small-ties` | 12 | Written by hand. Totals tie across both 27 percent group boundaries; q06 is answered right mostly by low scorers. Fewer than 30 sheets. |
| `class-40` | 40 | Fifteen right-or-wrong questions, four versions; q15 discriminates the wrong way. |
| `weighted-30` | 30 | q09 and q10 are worth two points (0 or 2). Still KR-20: no question gives partial credit. |
| `partial-35` | 35 | q07 and q08 give partial credit in halves, so the class figure is Cronbach's alpha. |
| `constant-20` | 20 | Everyone got q03 right and q05 wrong: their correlations are undefined. |
| `rounding-150` | 150 | 27 percent of 150 is 40.5, which rounds up to 41 (rounding half to even, R's `round`, would give 40). |
| `adjusted-30` | 30 | q04 does not count on version 2. Shows where the two tools differ on purpose (below). |

## The figures

The names are the keys of `reference.json`. *n* is the number of sheets.

**For the whole test**

- `sheets`: *n*. `small_group`: *n* is under 30; OpenAssess then says the figures are unstable.
- `test.mean`: the mean total.
- `test.sd_population`: the SD of the totals with *n* as the divisor. **OpenAssess's SD**: it describes this class.
- `test.sd_sample`: the SD with *n* − 1 as the divisor. **PrismStatistics's SD** (Descriptives).
- `groups_27`: the upper and lower groups.
  - `size` is *g* = ⌊(27*n* + 50) / 100⌋, which is 27 percent of *n* rounded half up, worked in whole numbers so that
    no rounding of 0.27 × *n* can move it.
  - The groups are formed (`split`) only when *g* ≥ 2 and 2*g* ≤ *n*.
  - Sheets are ordered by total, highest first. A tie puts the lower sheet number first.
  - The upper group is the first *g* sheets and the lower group the last *g*. `upper` and `lower` list the sheet numbers.

**For each question** (`item.<q>`)

- `sheets`: the sheets it counts on.
- `difficulty`: the mean share of its points earned (earned ÷ possible) on those sheets.
- `point_biserial_item_included`: the Pearson correlation of the question's points with the sheet's total, **where the
  total includes the question**, on the sheets it counts on. **OpenAssess's point-biserial.**
- `upper_lower_27`: the mean share of its points earned in the upper group less the lower group's, on the group sheets
  it counts on.

**The class reliability figure** (`reliability_every_sheet`, OpenAssess)

The questions that count on every sheet are used; `left_out` names any others. `value` is Cronbach's alpha of their
points, which is KR-20 when every one is right or wrong. `kind` is `Cronbach alpha` when one of them gives partial
credit, otherwise `KR-20`.

**The scale reliability analysis** (`reliability_listwise`, PrismStatistics's Reliability on q01 to the last question)

- `n`: the sheets with a value on every question (listwise).
- `alpha`, `feldt_95_lower` and `feldt_95_upper`: Cronbach's alpha and Feldt's 95% interval, from the F distribution
  with *n* − 1 and (*n* − 1)(*k* − 1) degrees of freedom.
- `standardized_alpha` and `mean_inter_item_r`: from the mean correlation between pairs of questions. Undefined when a
  question does not vary.
- For each question (`item.<q>`):
  - `mean` and `sd` (*n* − 1).
  - `corrected_item_total_r`: the correlation of the question with the total of the **other** questions, so the
    question is **left out** of the total. **PrismStatistics's corrected item-total r.**
  - `alpha_if_deleted`: alpha without the question.

A figure that is `null` is **undefined**: a correlation with something that does not vary, for example. A tool must
show no figure there, never 0.

## The two correlations

The two tools correlate each question with a different total, so their figures differ even on the same sheets. Each
must label its figure so a reader can tell which it is:

| Tool | Figure | Total | Label to show |
|---|---|---|---|
| OpenAssess | `point_biserial_item_included` | includes the question | **Point-biserial (total includes the question)** |
| PrismStatistics | `corrected_item_total_r` | leaves the question out | **Corrected item-total r (total leaves the item out)** |

Including the question in the total raises the figure, more so on a short test. On `small-ties` (six questions), q01's
point-biserial is 0.60 and its corrected item-total r is 0.33; q06's are 0.10 and −0.19, so only the corrected figure
shows that q06 runs the wrong way.

## Where the tools differ on purpose

1. **The correlation**, above.
2. **The SD**: OpenAssess divides by *n* (the class), PrismStatistics by *n* − 1 (a sample).
3. **A question that does not count on some sheets** (`adjusted-30`): OpenAssess leaves the **question** out of KR-20,
   so every sheet stays in. PrismStatistics leaves out the **sheets** with an empty cell (listwise), so every question
   stays in. Both are right for what they answer; neither tool should quietly switch to the other's rule.
4. **Rounding**: OpenAssess keeps four places; PrismStatistics keeps every digit and rounds only for display.

## Tolerances

- OpenAssess: within 0.00005 of R. It rounds to four places, so this is half a unit in the fourth place.
- PrismStatistics: within 1e−9 of R, and within 1e−8 for Feldt's limits, which go through the F distribution. These are
  the tolerances of its own reference tests.
- A count (sheets, items, group size) must be equal, and an undefined figure must be undefined.

## Running it

```bash
node check/check_set.mjs
node check/check_openassess.mjs --assess <path to an OpenAssess clone>
node check/check_prismstatistics.mjs --prism <path to prism.exe or prismstatistics-app.exe>
```

The PrismStatistics checker uses only the program's own batch runner (a script and a saved project), so it runs
against a build or an installed copy without changing either. Both checkers take `--reference <file>` to run against a
changed copy of `reference.json`, which shows they fail when a figure is wrong.

## Results

| Date | Tool | Build | Figures |
|---|---|---|---|
| 2026-11-15 | OpenAssess | source at commit a4c7097 (alpha 27, not released) | 349 of 349 match |
| 2026-11-15 | PrismStatistics | `prism.exe` built from the 0.1.1 tree, 2026-11-15 16:07 | 176 of 176 match |

Both checkers were also run against a copy of `reference.json` with six figures changed: each reported every change
that concerned it (12 figures for OpenAssess, 3 for PrismStatistics) and exited with an error.

## Changing the set

- A new case, or a new figure for an existing case, is a minor version (1.1.0). Run `make_pstat.mjs` and
  `make_reference.R`, then `check/check_set.mjs --write` to update `SHA256SUMS`.
- A changed definition or a changed reference value is a major version (2.0.0). Say why in this file, and run both tools
  against it before the change is merged.
- A tool that keeps a copy of the set in its own repository copies the whole folder at one set version, so that
  `check_set.mjs` passes there too.
