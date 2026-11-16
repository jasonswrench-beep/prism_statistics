# Checks only you can make

Everything else is checked by the tests (`cargo test --workspace`, `npm test` in `app/ui`) and by
`python tools/validate.py`, which writes `docs/validation-report.md`. Items are grouped by the development list item
they belong to (November 2026 list). Tick a box when it's done; add what you found underneath.

## Item 1. Sign-off, student testing, an outside PDF/UA check

- [ ] **Sign each validation suite.** Read its rows in `docs/validation-report.md`, then put your name and the date in
  its row of `docs/validation-signoff.md`. Two suites are new this round: `c3_reference` (the shared item-analysis test
  set, the same files OpenAssess is checked against) and `item_analysis_reference` (test scoring from letters, against
  psych and base R). Every row is still unsigned.
- [ ] **User testing with real students.** The plan is `docs/stage0/07-user-testing-plan.md` (round R4, a classroom
  pilot of one course section). Before recruiting, check whether your university needs an IRB/HREB determination
  (the plan's section 2). Use synthetic data only. Suggested tasks for this round, each timed, with the outcome
  (unaided, assisted, failed) and the student's own words where they got stuck:
  1. Open `crates/prism-engine/tests/reference/item_letters.csv`, score it with the key
     `B, A, D/B, C, B, A, C, D, B, AC, C, D` (Analyze › Scale › Test Scoring and Item Analysis) and say which question
     works worst and why.
  2. Open `crates/prism-engine/tests/reference/c3-item-analysis/cases/class-40/class-40.pstat` and run the item
     analysis the banner offers.
  3. Any three tasks from the task battery (`docs/stage0/02-users-and-task-battery.md`).
- [ ] **An outside PDF/UA check.** PrismStatistics's PDF writer refuses to write a file that fails PDF/UA-1, but no
  independent validator has read one yet. Export a report as PDF (any result, File › Export › PDF), then check it with
  an independent PDF/UA validator. veraPDF is open source (its PDF/UA-1 profile), and PAC is free for Windows. Note
  which validator and version you used and anything it reports. A screen-reader pass over the same PDF (NVDA) belongs
  to the end-of-project accessibility review.

## Item 2. Test scoring and item analysis

- [ ] **Run it in the window once.** Open `crates/prism-engine/tests/reference/item_letters.csv`, choose Analyze ›
  Scale › Test Scoring and Item Analysis, move q01 to q12 into Items and type the key
  `B, A, D/B, C, B, A, C, D, B, AC, C, D`. Expected: N = 119, KR-20 = .62, KR-21 = .58, and the checks name q12 as
  correlating negatively with the rest of the test. The figures are tested; the dialog has been checked only by its
  unit tests, not by eye.
- [ ] **Decide the two labels.** The shared test set asks for "Point-biserial (total includes the question)" and
  "Corrected item-total r (total leaves the item out)". Item Analysis shows both, and Reliability Analysis now shows
  the second. If you want other words, change them in openstage-core first (its TESTING-TODO asks the same).
- [ ] **Look at the key syntax.** `D/B` for two right answers, `AC` for select-all, `-` to drop a question and `*` to
  credit everyone. Say if any of these would confuse a colleague.

## Item 3. Opening OpenAssess's .pstat files

- [ ] **Double-click a results file.** Double-click
  `crates/prism-engine/tests/reference/c3-item-analysis/cases/small-ties/small-ties.pstat`. Expected: Data View shows
  `sheet`, `version` (A and B), `q01` to `q06` labelled Question 1 to Question 6, and `total`, with a banner above it
  offering "Item analysis of the questions", "Reliability of the questions" and "Descriptives of the total". Each
  button should open its dialog filled in; Reliability should give alpha = .47. This has been run through the engine
  and the API, not in the window.
- [ ] **The key and item details.** The locked profile (PSTAT-1 section 7.1) carries points only: no letters, no key,
  no objectives or Bloom levels. The request for a profile that carries them is R1 in `CORE-REQUESTS.md`; decide it in
  openstage-core.

## Items 4 and 5. The shared statistics library and the case-table exchange

- [ ] **Decide where the shared statistics crate lives.** The settled direction is written into decision D03
  (`docs/stage0/01-decision-log.md`): one crate, published from openstage-core and pinned by tag in both apps. Moving
  `prismqda-stats` there is R5 in `CORE-REQUESTS.md`, for a core session. Until then `python tools/check_shared_stats.py`
  (run by every release build) says when PrismQDA's copy has changed; on 2026-11-15 it had not.
- [ ] **Decide the case-table layout.** R6 in `CORE-REQUESTS.md` proposes a `.pstat` profile for PrismQDA's case table.
  Meanwhile Data › Add Variables takes a `.pstat` file, so a case table saved as a project merges onto survey data by
  its case id (tested through the API, not yet in the window).
- [ ] **Try Add Variables with a project in the window once.** Open any survey CSV with a `case_id` column, then Data ›
  Add Variables and choose a `.pstat` file that also has `case_id`. The file picker should now list `.pstat` files.

## Item 6. Item fit and M2

- [ ] **Look at the new tables once.** Open `crates/prism-engine/tests/reference/lsat7.csv`, run Analyze › Scale › Item
  Response Theory on i1 to i5 with the 2PL model. Expected: a "Limited-information model fit" table with M2 = 11.94,
  df = 5, RMSEA = .037 [.009, .065], SRMSR = .032, TLI = .937, CFI = .968 (mirt's values), and an "Item fit" table with
  S-X² for each item. With `science.csv` (four items, four categories) the M2 table is missing and a check says M2 could
  not be computed, which is what mirt does too.
- [ ] **The wording of the fit check.** It flags RMSEA or SRMSR above .08 as "Review needed" and names items whose S-X²
  is significant at .05 without adjusting for the number of items. Say if you want a different cut or an adjustment
  (mirt's itemfit takes one).

## Item 7. Training evaluation

- [ ] **Run the report once in the window.** File › Open Example Data › Training evaluation, then Analyze › Training
  Evaluation (Kirkpatrick Levels): react_1 to react_4 into Reaction; pre_test then post_test into Learning before and
  after; behav_pre then behav_post into Behavior before and after; errors_pre then errors_post into Results; 20 as the
  test points possible. Expected: Learning mean gain 3.88 points, d_z = 1.69, normalized gain .35; Behavior N = 54 (six
  not rated at follow-up). The figures are tested against R; the dialog has not been tried by eye, and it has eight
  boxes, so say whether it is clear enough or should be split by level.
- [ ] **Try the blank template.** The same dialog's "Blank template" button opens the variables with no cases. Check
  that you can type cases into Data View from there.
- [ ] **A real xAPI export.** Only hand-made statements have been read. If any of your LMSs or an LRS can export
  statements as JSON, open one (File › Open data, choose the .json file) and check the verbs, activities and scores.
  Learners show as "Learner 1", "Learner 2"; nothing identifying is read.
- [ ] **Matched learning gains from OpenAssess.** Two OpenAssess results files can be compared as two groups today
  (Add Cases, then the two-groups boxes). Revision 1.2 lets the files carry `student_id`, so each student's before and
  after can be matched (Add Variables on `student_id`); OpenAssess doesn't hold ids yet, so no real file has one.

## Item 8. Exporting the data

- [ ] **Open an exported .sav file in your menu-driven package once.** File › Export › Data as .sav… from any dataset
  with value labels and missing codes, then open the file there. The file is read back exactly by PrismStatistics's own
  .sav reader (tested); no other program has opened one yet.

## Item 9. A Mac build

Deferred (2026-11-15): Mac builds come later, for every tool together. The two checks below wait for that round.

- [x] **Decide whether to run the Mac build workflow.** Decided 2026-11-15: not yet; every tool gets a Mac build in one later round, and this workflow is the starting point then. `.github/workflows/macos.yml` builds and tests on GitHub's
  macOS runners, for Apple silicon and Intel, and keeps an unsigned .dmg. It runs only when you start it (GitHub,
  Actions, Mac build, Run workflow), because macOS minutes on a private repository count against your plan at a
  higher rate. It has never run, so expect to fix a few things on its first runs (the llama.cpp build in particular).
- [ ] **Try the .dmg on a Mac.** Open it, drag PrismStatistics to Applications, and open it with right-click, Open (it
  is not signed). Check opening a .pstat file from Finder, the file dialogs, and VoiceOver on the menus and a dialog.
- [ ] **Signing and notarization.** A Mac build that opens without warnings needs an Apple Developer ID ($99 a year)
  and notarization. That is your decision; nothing is set up for it.

## Core's second round (PSTAT-1 1.1, test set 1.1.0)

- [ ] **Open a profile-2 file in the window.** Double-click
  `crates/prism-engine/tests/reference/c3-item-analysis/cases/letters-60/letters-60.pstat`. The banner should offer
  "Item analysis of the answers marked, with the file's key" first; it should open with r01 to r11 and the key
  `C, A, D, B, A, D, B, B/E, AC, B, *` filled in, and the result should show the options by score group.
- [ ] **Open a case table.** Double-click `crates/prism-engine/tests/reference/pstat-examples/case-table.pstat` (the
  spec's example): the banner should offer code frequencies and excerpts per case.
- [x] **Publishing and the core.** The core went public on 2026-11-15, so the public `prism_statistics` copy builds
  its statistics from the core's tag like this one. Publishing waits only on your word.
- [x] **Student ids in profile 2.** Answered 2026-11-15: ids and names are both fine, since the files stay on the
  instructor's computer. PSTAT-1 revision 1.2 has `student_id` and `student_name` as optional text variables after the
  `r` variables. Matching each student's before and after: Data › Add Variables on `student_id`, then the paired
  Learning boxes in Training Evaluation.

