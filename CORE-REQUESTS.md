# Requests to the shared core

**Status, 2026-11-15 (core's second round, commit 13b210c):** R1, R2, R3, R5 and R6 are done in the core and taken up
here (PSTAT-1 revision 1.1, test set 1.1.0, `prismqda-stats` 1.0.0). R4 waited on the core going public, which it now is (2026-11-15).
What PrismStatistics did with each is under "Taken up" at the end.

Things PrismStatistics needs from `openstage-core` that the core does not offer yet. PrismStatistics does not build its
own version of any of these; each waits here until the core publishes it. Newest last. Core as read: commit 6c0b84b
(PSTAT-1 revision 1.0, item-analysis test set 1.0.0, `@openstage/pstat` 1.0.0).

## R1. A results profile that carries the answers, the key and the item details

**For:** development list item 3 ("open the .pstat file OpenAssess writes, reading its key and item details").

Profile 7.1, `OpenAssess item results 1`, carries only the points earned per question and the total. The development
list's link section describes more: the letters marked, the key with its adjustments, and the details of each item.
PrismStatistics now reads profile 1 (it offers item analysis on the points, reliability and descriptives when one
opens) and can already score letters against a key itself (Analyze › Scale › Test Scoring and Item Analysis). What it
cannot do is read the key or the item details from the file, because the format does not say where they are.

Asked: a new profile version (or a second profile) in PSTAT-1 that adds, keyed to bank question ids rather than to
positions on a version:

- **Responses:** per sheet, the letter or letters marked on each question (blank kept as an empty cell), as text
  variables beside or instead of the points. Select-all answers written as the letters in order (`AC`).
- **Key:** per question id, the correct options, the points, the option labels, and any adjustment (dropped, also
  right, credit all).
- **Item details:** per question id, the objective, chapter, Bloom level, difficulty label, question type and version
  history number.
- **Test details:** test id, administration date, source (scanned sheets or an LMS export) and the counts left out.
- Optional, off by default: an instructor's student id column. No names or scan images.

PSTAT-1 already lets a reader ignore unknown zip entries and members, so one possible home is a second entry such as
`assessment.json` beside `project.json`, which older readers skip. Whatever layout the core chooses, PrismStatistics
will:

- map each key entry onto its item-analysis key (`correct`, `points`, `adjustment`: none, dropped, credit_all), so the
  letters are scored with distractor counts by score group;
- put the item details into variable labels (or a variable attribute the spec names), so items can be grouped by
  objective or Bloom level.

## R2. Item-analysis figures beyond the current test set

**For:** development list item 2 ("each checked against R").

Test set 1.0.0 covers difficulty, the point-biserial with the question in the total, the upper-lower difference, KR-20
or alpha, and PrismStatistics's Reliability. PrismStatistics now also reports these, checked against its own R
reference (`crates/prism-engine/tests/reference/make_item_analysis_reference.R`, psych and base R), which OpenAssess is
not checked against:

- KR-21 (variances divided by *n*), and the standard error of measurement;
- each group's own difficulty (upper and lower) as well as the difference;
- option counts overall and by upper, middle and lower group, with blanks (needs R1's responses in the set's files);
- scoring letters against a key with two right answers, select-all, dropped and credited questions.

Asked: a minor version of the set (1.1.0) adding these figures to `reference.json`, so both tools match on them. The
definitions PrismStatistics uses are in `crates/prism-engine/src/analysis/item_analysis.rs` and its help page.

## R3. `check_set.mjs` does not run in a copy of the set

**For:** keeping a copy of the set in this repository (CORE-NOTES, "Changing the set").

The README says a tool keeping a copy "copies the whole folder at one set version, so that `check_set.mjs` passes there
too". In a copy it fails at once: it imports `../../packages/pstat/src/index.js`, a path that exists only inside the core
repository. PrismStatistics keeps the folder byte for byte (marked `-text` in `.gitattributes`) and checks the copy with
`sha256sum -c SHA256SUMS` instead, which passes.

Asked: make the check self-contained in the set folder (or say in the README that a copy is checked with `SHA256SUMS`
alone).

## R4. Release-check commands that take an R path and a build path from one place

**For:** "Add both checks to the release checks."

PrismStatistics now runs the C3 set inside its own Rust reference tests (`c3_reference`), reading the copy's
`reference.json`, so it does not need Node for that check. The reader conformance check
(`specs/pstat/conformance/check_reader.mjs --prism <build>`) still needs the core repository on disk. `tools/release.py`
runs it when it is given the core's folder (`--core <path>`), and says it was skipped otherwise.

Asked: once the core repository is public, a tagged release with the conformance files, so a release check can fetch
them by tag rather than needing a clone at a known path.

## R5. The statistics crate, shared from the core

**For:** development list item 4 (decision D03, "settle the shared statistics library with PrismQDA").

PrismStatistics's core numbers come from `prismqda-stats`, pinned to PrismQDA commit 56a161e. PrismQDA is also Rust, so
the two can share code (unlike OpenAssess, which shares only the test set). Today a fix in PrismQDA's copy does not
reach PrismStatistics, and a fix PrismStatistics needs has to be made in PrismQDA's repository.

Asked: move the crate into openstage-core as a published Rust crate (say `crates/prism-stats`), with:

- tagged releases, so each app pins a tag (`git = ".../openstage-core", tag = "prism-stats-v1.0.0"`), not a commit of
  another app;
- the crate's own R reference tests in the core, run by `tools/check_all.mjs` or a `cargo test` beside it;
- the rule that an app moves its pin only with its own reference tests passing (for PrismStatistics, every reference
  test in `python tools/validate.py`).

What PrismStatistics uses from it today: `dist` (distribution functions and quantiles), `describe`, `ttest`,
`anova`, `regression`, `logistic`, `correlation`, `nonparametric`, `discrete`, `contingency`, `rank`, `adjust`, `rng`,
`formula` and `charts`. Until the crate is in the core, `tools/check_shared_stats.py` reports any change to PrismQDA's
copy since the pin (none on 2026-11-15).

## R6. A PSTAT-1 profile for PrismQDA's case table

**For:** development list item 5 (decision D10, "the case-table exchange with PrismQDA").

Mixed-methods projects need survey numbers and coded text in one place. PrismQDA already builds the table (its
`stats.variables` command: one row per case, case attributes, and for each code its presence and its count of
excerpts). The `.pstat` format can carry it as it stands; what is missing is a fixed layout, so PrismStatistics can
rely on the names and say where each column came from.

Asked: profile 7.2 of PSTAT-1, say `PrismQDA case table 1`, fixing:

- one case per PrismQDA case, in case order, with a `case_id` variable (text or whole number, as PrismQDA keeps it)
  that a survey file can be matched on, and the case's name;
- one variable per case attribute, with PrismQDA's measure (nominal, ordinal, scale) and value labels where it has
  them;
- for each code exported, two variables (`code_<n>_present`, 0 or 1, and `code_<n>_count`), labelled with the code's
  path in the codebook (`Barriers › Cost`), named by a stable number rather than the code's text, since code names change
  and may not be valid variable names;
- provenance in `source`: the PrismQDA project's name and id, the export date, the codebook version, and whether
  sub-codes were counted with their parent (PrismQDA's `include_descendants`);
- whether excerpt text is ever written (suggested: never, as the item-results profile leaves out question text).

PrismStatistics already reads any `.pstat` file in Add Variables and Add Cases (development list item 5, first part),
so a case table merges onto survey data by `case_id` today. Once the profile is fixed, PrismStatistics will recognise
it on opening, as it does the item-results profile, and offer the joint analyses (crosstabs of a code by a group,
a code's presence as a predictor). The return direction, PrismStatistics results into PrismQDA, is PrismQDA's to read.

## Taken up (2026-11-15, second round)

- **R1 (profile 2):** `OpenAssess item results 2` files open with an offer to analyse the letters marked with the
  file's own key (`item_analysis::key_from_assessment`); a question whose key differs by version or gives partial
  credit is analysed from its points, and the banner names it. `assessment.json` is kept byte for byte when saving
  (rule Z4). Question details (objective, Bloom level and the rest) are kept in the part but not yet used to group
  items.
- **R2 (set 1.1.0):** copied whole; `c3_reference.rs` now also checks `item_analysis_listwise` for every case, each
  group's own share, and `answers` (answers by score group) for `letters-60`, scored with the file's key.
- **R3:** the copy passes `node check/check_set.mjs` in place.
- **R5:** `prismqda-stats` comes from the core at tag `prismqda-stats-v1.0.0` (source identical to the old pin), fetched
  with `git-fetch-with-cli`. The core went public the same day, so the public `prism_statistics` copy builds from the
  same tag; R4 (tagged releases with the conformance files) is unblocked for the core.
- **R6:** `PrismQDA case table 1` files open with offers (how many cases each code appears in, excerpts per case) and a
  pointer to Add Variables on `case_id`; `case-table.json` is kept when saving.

## Taken up (2026-11-16, third round: PSTAT-1 1.3, set 2.0.0, A11Y-1)

- **The two rulings (PSTAT-1 revision 1.3, set 2.0.0, from the core's tags `pstat-v1.3.0` and `item-analysis-v2.0.0`):**
  on a question credited to everyone, a sheet with no answer (left blank, or marks never settled) is counted
  credited, never blank: `item.<i>.blank` is 0 and those sheets are in the option row `credited` (shown as "(no
  answer, credited)"). A question of type `other` has no `r` variable and is analysed from its points; the banner now
  says it is scored by a person. The set is copied whole at 2.0.0 and `c3_reference.rs` checks `credited-40` on the
  letters as well; the spec's example `item-results-2-boxes.pstat` is in `tests/reference/pstat-examples` with a test.
  Against a build of this commit: the reader conformance check passes 34 of 34, and `check_prismstatistics.mjs` 1,851
  figures match and none differ. `tools/release.py` names `pstat-v1.3.0` in its examples.
- **Question details** (objective, chapter, section, Bloom level, difficulty level) are now used: Test Scoring and Item
  Analysis gives subscores by a label per question, and the banner offers subscores by each detail that splits the
  questions. A set case whose `assessment.json` fills the details would let the set check subscores too (the
  definitions are in `item_analysis.rs`, `subscores`); not a request yet.
- **A11Y-1 (tag `a11y-v0.1.0`):** prism-api's tests run the engine on a report exported as Word and as a web page: no
  rule fails and nothing needs review. PDFs wait on A11Y-1 section 6.6.
