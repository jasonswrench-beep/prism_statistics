# Handoff to the openstage-core session

> **Answered** by the core at 13b210c (2026-11-15, second round): R1, R2, R3, R5 and R6 done, R4 waiting on the
> repository going public. What PrismStatistics took up is at the end of `CORE-REQUESTS.md`. Kept as sent.

**From:** the PrismStatistics session, 2026-11-15. **Core as read:** `jasonswrench-beep/openstage-core` at 6c0b84b
(PSTAT-1 revision 1.0, item-analysis test set 1.0.0, `@openstage/pstat` 1.0.0).
**PrismStatistics state:** this round's work is on the `item-analysis` branch of the private `prismstatistics`
repository, to be fast-forwarded into `main` once its validation run passes. The full requests, with the reasoning, are
in `CORE-REQUESTS.md` beside this file; this memo is the short version, in the order to take them.

## 1. What PrismStatistics did with what the core published

Everything in "For the PrismStatistics session" in CORE-NOTES.md, except the build-level checks:

| Core asked | Done | Where |
|---|---|---|
| Cite PSTAT-1 revision 1.0; drop "provisional" | Yes | `crates/prism-engine/src/project.rs` |
| Run the C3 set in the release checks | Yes, ported into the Rust reference tests: all seven cases, every figure within 1e-9 (Feldt 1e-8), run by `cargo test` and `tools/validate.py` | `crates/prism-engine/tests/c3_reference.rs`; the set copied whole at 1.0.0 to `tests/reference/c3-item-analysis/` (marked `-text` so the bytes match `SHA256SUMS`) |
| Run `check_prismstatistics.mjs` and `check_reader.mjs` against a build | Wired, **not yet run**: `python tools/release.py build --core <clone>` runs both against the program it just built; without `--core` it says they were skipped | `tools/release.py` |
| Label the column "Corrected item-total r (total leaves the item out)" | Yes, in Reliability and in the new item analysis, which also shows "Point-biserial (total includes the question)" | `crates/prism-engine/src/analysis/item_analysis.rs` (`CORRECTED_LABEL`, `POINT_BISERIAL_LABEL`) |
| Fix the newer-version message wording | Yes: the program's name is added only when `app_version` starts with a digit ("This project was saved by OpenAssess 0.2.0 …") | `project.rs`, `newer_message` |
| Possible later: recognise the item-results profile | Yes: opening one shows a banner offering item analysis and reliability on `q01…` and descriptives of `total` | `project::profile_suggestions`, `open_project` reply |
| Possible later: keep `profile` when saving | Yes | `Project.profile` |
| Possible later: unrounded item means and SDs as fields | Yes: `item.<i>.mean`, `item.<i>.sd` in Reliability | `reliability.rs` |

**The core can confirm** by running, with a PrismStatistics build from that branch:

```bash
node testsets/item-analysis/check/check_prismstatistics.mjs --prism <prismstatistics-app.exe>
node specs/pstat/conformance/check_reader.mjs --prism <prismstatistics-app.exe>
```

Both should pass unchanged. The reader is a superset now (it also opens xAPI `.json` and takes `.pstat` in Add
Variables), but nothing it refused before is opened now.

New PrismStatistics item statistics the set does not cover yet (definitions in `item_analysis.rs` and its help page):
letter scoring against a key (`B`, `D/B`, `AC`, `-` dropped, `*` credit all), each group's own difficulty, D, option
counts by upper, middle and lower group, KR-21 with variances divided by *n*, and the standard error of measurement.
Groups use the set's rule exactly: *g* = ⌊(27*n* + 50)/100⌋, highest total first, ties by sheet order, formed only when
*g* ≥ 2 and 2*g* ≤ *n*. These are checked against PrismStatistics's own R reference
(`tests/reference/make_item_analysis_reference.R`, psych and base R), not against OpenAssess.

## 2. Requests, in the order to take them

### R1. Item-results profile 2: answers, key, item details, optional student id (spec change, M)

Profile 7.1 carries points only. The development lists (OpenAssess and PrismStatistics, "The .pstat export") describe
five parts: Responses, Scores, Key, Item details, Test details, all keyed to bank question ids, and an optional student
id column the instructor adds. Without them PrismStatistics cannot read the key or item details from a file (its
item 3), count distractors from OpenAssess files, or match each student's before and after (its item 7).

Asked of the core: profile `OpenAssess item results 2` in PSTAT-1 (or a second profile), a test-set case written to it,
and the `@openstage/pstat` checks for it. One possible layout is a second zip entry such as `assessment.json` for the
key and item details (PSTAT-1 already lets readers ignore unknown entries, so profile-1 readers are unaffected) plus
text variables for the letters marked. **Done when:** the profile is in PSTAT-1, a case in the set uses it, and
`checkPstat` validates it. PrismStatistics will then map each key entry to its item-analysis key and the item details
to variable labels; OpenAssess writes it.

### R2. Test set 1.1.0: the figures above, and letter cases (minor version, S, after R1)

Add the figures listed at the end of section 1 to `reference.json`, with at least one case carrying letters and a key
(needs R1's layout). **Done when:** both tools' checkers cover them. PrismStatistics will compare at 1e-9.

### R3. `check_set.mjs` fails in a copy of the set (S)

The README says a tool's copy should pass `check_set.mjs`; in a copy it stops at once, because it imports
`../../packages/pstat/src/index.js`, which exists only in the core. PrismStatistics checks its copy with
`sha256sum -c SHA256SUMS` instead (passes). **Done when:** the check runs inside the set folder alone, or the README
says a copy is checked with `SHA256SUMS`.

### R5. The statistics crate, shared from the core (M)

PrismStatistics's core numbers come from PrismQDA's `prismqda-stats`, pinned to PrismQDA commit 56a161e; a fix in one app
doesn't reach the other. Decision D03 (PrismStatistics `docs/stage0/01-decision-log.md`) now settles the direction: one
Rust crate in the core, tagged, both apps pinning the same tag, each moving its pin only with its own reference tests
passing. **Done when:** the crate is in the core with its R reference tests and a first tag. PrismStatistics then
swaps its Cargo dependency. Until then `tools/check_shared_stats.py` reports drift; on 2026-11-15 there was none.
Modules PrismStatistics uses: `dist`, `describe`, `ttest`, `anova`, `regression`, `logistic`, `correlation`,
`nonparametric`, `discrete`, `contingency`, `rank`, `adjust`, `rng`, `formula`, `charts`.

### R6. PSTAT-1 profile for PrismQDA's case table (spec change, M)

Decision D10's exchange, as a `.pstat` profile (`PrismQDA case table 1`). PrismQDA already builds the table
(`stats.variables`: one row per case, attributes as `attr:Name`, codes as `code:ID:present` and `code:ID:count`).
Proposed fixed points: a matchable `case_id`; one variable per attribute with its measure and value labels;
`code_<n>_present` (0/1) and `code_<n>_count` named by a stable number and labelled with the code's path; provenance in
`source` (project name and id, export date, codebook version, whether sub-codes count with their parent); no excerpt
text. **Done when:** the profile is in PSTAT-1 with an example file. PrismStatistics already merges any `.pstat` onto
survey data by key (Add Variables); with the profile fixed it will recognise the file on opening.

### R4. Tagged core releases (S, when the repository goes public)

So a release check can fetch the conformance files by tag instead of needing a clone at a known path.

## 3. Rules PrismStatistics kept, so the core can rely on them

- No item-math code shared with OpenAssess; the shared thing is the test set.
- No change to a format or test-set copy in PrismStatistics without a core revision first; the C3 copy is byte-for-byte.
- No tool described as an alternative to a named product.
- Nothing core-owned was built in PrismStatistics: profile 2, the shared crate and the case-table profile all wait on
  the requests above.

## 4. Decisions only the product owner makes

- The wording of the two point-biserial labels (the core's TESTING-TODO asks the same).
- When the core repository goes public (affects R4 and installing `@openstage/pstat` by URL).
- Whether profile 2 carries a student id at all, and whether it is off by default (the development list says
  instructor-added, for their own records).
