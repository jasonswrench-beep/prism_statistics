# Item-analysis test set (C3)

**Set version 1.1.0, 2026-11-15.**

OpenAssess (TypeScript) and PrismStatistics (Rust) both compute item statistics. They share no code. Instead, both run
this set, and neither ships a release that breaks it. Each case is a class's scored answer sheets. Every figure in
`reference.json` was computed in base R from the case files alone, so neither tool is checked against the other.

## What is here

| Path | What it is |
|---|---|
| `cases/<name>/scores.csv` | One row per complete sheet: `sheet`, `version`, then the points earned on each question. An empty cell means the question does not count on that sheet, which is not the same as 0. |
| `cases/<name>/case.json` | What each question is worth and whether it gives partial credit. For a case with letters, also each question's type, options, the answers that earn its points (`correct`) and any adjustment (`none`, `credit_all`, `dropped`). |
| `cases/<name>/answers.csv` | A case with letters only: one row per sheet, the letters marked on each question in version A's lettering, an empty cell for a blank. |
| `cases/<name>/<name>.pstat` | The same sheets as the `.pstat` file OpenAssess sends to PrismStatistics: profile `OpenAssess item results 1` ([PSTAT-1](../../specs/pstat/PSTAT-1.md) section 7.1), or profile 2 with its `assessment.json` (section 7.2) for a case with letters. |
| `reference.json` | R's figures for every case, named as below. |
| `make_cases.R` | Made the case files once. They are kept, so a change in R's random numbers cannot move a reference value. |
| `make_reference.R` | Computes `reference.json` from the case files (`Rscript make_reference.R`). Base R only. |
| `make_pstat.mjs` | Writes the `.pstat` files from the case files (`node make_pstat.mjs`). |
| `check/check_openassess.mjs` | Runs OpenAssess's item statistics on every case and compares. |
| `check/check_prismstatistics.mjs` | Runs PrismStatistics's Reliability, Descriptives and Item Analysis (on the points, and on the letters against the file's key) on every `.pstat` file and compares. |
| `check/check_set.mjs` | Checks the set itself: every file matches `SHA256SUMS`, and, in the core repository, every `.pstat` file is what `make_pstat.mjs` makes and follows PSTAT-1. |

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
| `omitted-25` | 25 | (1.1.0) q03 was dropped on every version, so it counts on no sheet: it has no figures, and the `.pstat` file leaves it out, naming the others `q01`, `q02`, `q04` ... by their number on version A. |
| `letters-60` | 60 | (1.1.0) The letters marked, scored against a key, in profile 2: one-answer questions (q07 with a wrong answer that draws the high scorers), q08 with a second answer also accepted, q09 select-all, q10 true/false, q11 credited to everyone, q12 dropped. Blanks, and two letters marked on a one-answer question. |

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

**Item analysis as a scale analysis** (`item_analysis_listwise`, PrismStatistics's Item Analysis on the points)

The sheets with a value on every question (as `reliability_listwise`), each question's points out of what it is worth
(`case.json`). Variances divide by *n*.

- `n`, `k` (questions), `possible` (the points of all *k*), `mean` (of the totals), `sd` (*n* − 1), `sd_n` (*n*),
  `mean_p` (`mean` ÷ `possible`).
- `right_or_wrong`: every cell is 0 or the question's points. `reliability` is then KR-20, otherwise Cronbach's alpha,
  with variances divided by *n* (the same value as `reliability_listwise.alpha`).
- `kr21`: KR-21, only when every question is right or wrong and worth the same points *w*. With *m* the mean total
  ÷ *w* and *v* the variance of the totals (÷ *n*) ÷ *w*²: *k* ÷ (*k* − 1) × (1 − *m*(*k* − *m*) ÷ (*k v*)).
  Undefined otherwise.
- `sem`: the standard error of measurement, `sd` (*n* − 1) × √(1 − `reliability`), with 1 − `reliability` taken as 0
  when it is below 0.
- The 27 percent groups over these sheets: `group_size`, `split`, `upper`, `lower` (the same rule as `groups_27`).
- For each question (`item.<q>`): `p` (mean share of its points), `mean` (of its points), the point-biserial with the
  question in the total, the corrected item-total r, `alpha_if_deleted` (with *k* > 2), and `upper`, `lower` (each
  group's mean share) and `d` (`upper` − `lower`).

**Each 27 percent group's own share** (`item.<q>.upper_27` and `lower_27`, 1.1.0): the mean share of the question's
points in the upper and the lower group of `groups_27`, on the group sheets it counts on. `upper_lower_27` is their
difference.

**The letters marked** (`answers.<q>`, a case with letters only), over the sheets the question counts on:

- `blank`: the sheets left blank on it.
- `by_answer`: for each answer written (`B`, `AB`, `ACD`), each of the question's options, each answer in its key, and
  `blank`: how many sheets gave it (`n`), and how many of them are in the upper, middle and lower group of
  `item_analysis_listwise` (the middle group is every sheet in neither). A whole answer is one row: `AC` is not counted
  under `A`. **PrismStatistics's option counts.** A row with `n` 0 may be left out by a tool.
- `by_letter`: for each option, the sheets whose answer marks it, so a sheet that marked `AC` counts once for `A` and
  once for `C`. **OpenAssess's option counts.**

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
5. **Option counts** (`letters-60`): OpenAssess counts the marks on each option (`by_letter`), PrismStatistics each
   whole answer by score group (`by_answer`). Both are kept.
6. **A credited question left blank**: OpenAssess calls such a sheet credited, not blank; PrismStatistics counts it as a
   blank. No case holds one until the two agree what it is (`letters-60`'s credited q11 is never blank).
7. **Which sheets an item analysis uses**: OpenAssess's figures for a question use the sheets it counts on;
   PrismStatistics's Item Analysis, like its Reliability, the sheets with a value on every question. They differ only
   where a question does not count on every sheet (`adjusted-30`).

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
| 2026-11-15 | OpenAssess | source at commit a4c7097 (alpha 27, not released), set 1.0.0 | 349 of 349 match |
| 2026-11-15 | PrismStatistics | `prism.exe` built from the 0.1.1 tree, 2026-11-15 16:07, set 1.0.0 | 176 of 176 match |
| 2026-11-15 | OpenAssess | source at commit ca56055, set 1.1.0 | 512 of 512 match |
| 2026-11-15 | PrismStatistics | `prism.exe` built from commit 10cee4b (its item-analysis round), set 1.1.0 | 1,497 of 1,497 match |

Both checkers were also run against a copy of `reference.json` with figures changed on purpose (six at 1.0.0, eight
at 1.1.0): each reported every change that concerned it and exited with an error.

## Changing the set

- A new case, or a new figure for an existing case, is a minor version (as 1.1.0 was). Run `make_pstat.mjs` and
  `make_reference.R`, then `check/check_set.mjs --write` to update `SHA256SUMS`.
- A changed definition or a changed reference value is a major version (2.0.0). Say why in this file, and run both tools
  against it before the change is merged.
- A tool that keeps a copy of the set in its own repository copies the whole folder at one set version and marks it
  `-text` in `.gitattributes`, so git never changes a byte. `node check/check_set.mjs` then passes in the copy too: there
  it checks every file against `SHA256SUMS` (which covers every file of the folder, the checkers and this README
  included), and says that the PSTAT-1 checks and the rebuild of the `.pstat` files run in the core repository, where
  `@openstage/pstat` is. `sha256sum -c SHA256SUMS` in the folder checks the same thing.

## Changes

- **1.1.0 (2026-11-15).** Two cases (`omitted-25`, `letters-60`), the letters case's `answers.csv`, and new figures for
  every case: `item_analysis_listwise` (KR-21, the standard error of measurement, each group's share, D), each group's
  own share in `item.<q>`, and for `letters-60` the answer counts. No 1.0.0 figure changed (all 770 compared equal).
  The `.pstat` files carry `app_version` `OpenAssess item-analysis test set 1.1.0`, and `SHA256SUMS` now covers every
  file of the folder, so a copy can be checked on its own.
- **1.0.0 (2026-11-15).** Seven cases.
