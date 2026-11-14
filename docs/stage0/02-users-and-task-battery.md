# First-Release Users and Task Battery (T0-01)

Draft 1 · November 10, 2026 · Spec §2, §5

## 1. Users in scope for 1.0

| Group | What they're doing | What 1.0 must let them do | Explicitly not promised in 1.0 |
|---|---|---|---|
| **Undergraduate student** | Intro or research-methods course, often a first stats package | Open a course dataset, find and run a standard test without code, read diagnostics, write and check an interpretation, hand in a project | Secure exams; methods beyond the 1.0 catalog |
| **Instructor** | Teaches a term with a fixed release | Hand out data and tasks, keep menus stable all term, open a student's submitted copy, replay it, see student vs. generated text | Assignment authoring and auto-grading (Stage 6) |
| **Applied researcher** | Survey or experimental study, thesis or paper | Import XLSX/SAV/DTA/SAS7BDAT, Google Sheets or `.jasp` data with labels intact, score scales, run regressions with diagnostics, export an editable report, reproduce it later | Multilevel, SEM, survey-design inference, imputation |
| **Professional analyst** | Repeats the same analysis on new data | Saved workflow with input roles, batch runner with exit codes, version-stamped outputs | Scheduling, servers, regulated-use validation |
| **Migrating user** (any of the above) | Coming from a menu-driven or command-driven package, R Commander, JASP or PSPP | Find familiar places for data, variables, procedures, output and syntax; understand where defaults differ | Running their old scripts or opening their old output files |

Specialists are **out of scope** for 1.0 users. Their needs shape the method registry and result schema only.

## 2. Task battery

These tasks are used three ways: in Stage 0 with clickable mockups (discovery only), in Stage 2 with the vertical slice, and in Stage 4 pilots with the full build. Each task has an ID that stays fixed across stages so results can be compared.

| ID | Task (as given to the participant) | Users | Stage first testable | Success means |
|---|---|---|---|---|
| TB-01 | "Open `wellbeing.sav` and tell me what the value 9 means in the *employment* variable." | All | 0 (mockup) | Finds Variable View or the value-label toggle; reads the label and sees 9 is a declared missing code |
| TB-02 | "Compare stress scores between the two treatment groups." | Migrating | 0 / 2 | Reaches the independent-samples dialog via menu or search, assigns test and grouping variables, defines the groups, runs it |
| TB-03 | "Same question, but you aren't sure which test to use." | Novice | 0 / 2 | Uses *Help me choose*, answers the design questions, ends in the same dialog with roles filled in |
| TB-04 | "Is the result different if you assume equal variances?" | Migrating | 2 | Changes to Student, reruns, and can see both versions side by side |
| TB-05 | "Explain this result in two sentences, including how sure we can be." | Novice | 2 | Mentions direction, size with interval, and one limitation; meets the interpretation rubric (§3) |
| TB-06 | "Show me exactly what was run so a colleague could repeat it." | All | 2 | Opens Paste/Syntax; can tell the native record apart from the R export |
| TB-07 | "Save, close, reopen, and re-run the analysis." | All | 1 / 2 | Same numbers; no warnings left unexplained |
| TB-08 | "Undo the recode you just did. Which outputs are now out of date?" | All | 2 / 3 | Undo works; points to the stale markers |
| TB-09 | "Find the procedures for descriptives, reliability, correlation and regression." | All | 0 | Each found in under 20 seconds (median) by menu or search |
| TB-10 | "Reverse-score items 3 and 7 on this 1–5 scale and make a mean score that needs at least 4 of 6 answers." | Researcher | 3 | Bounds entered as 1–5 rather than observed; minimum-answered rule set; preview checked |
| TB-11 | "Export the result as a Word document for your instructor." | All | 2 | DOCX opens; tables are real tables; N and missing data stated |
| TB-12 | Instructor: "Open Sam's submission and check that the numbers reproduce." | Instructor | 3 | Opens in review mode; replay matches; original untouched |
| TB-13 | Repeat TB-02 and TB-11 using **only the keyboard** (plus screen reader where applicable). | Accessibility participants | 2 | No step requires a pointer; status changes are announced |
| TB-14 | "Run last month's workflow on this new file." (file is missing one variable) | Professional | 3 | Run stops with a clear validation failure; no partial report |

## 3. Success rubrics

**Task completion:** *Unaided* (no help), *Assisted* (one hint), or *Failed* (two or more hints, gave up, or a wrong result accepted as right). Also record time, errors, the path taken (menu, search or guidance), and the participant's own confidence (1–5).

**Interpretation rubric (TB-05).** One point each, with 3 of 4 counting as meeting the rubric:
1. Correct direction (which group is higher).
2. An estimate with its interval, or uncertainty described in words.
3. No claim that a non-significant result means "no difference," and no causal claim the design doesn't support.
4. Names a relevant limitation (sample, design, assumption).

**Targets** (from spec §2, to confirm at G0 and measure at G4):
- First familiar analysis (TB-02, experienced users): 90% unaided within 5 minutes.
- Beginner completion (TB-03): 85% complete, no unresolved critical interpretation error.
- Discovery (TB-09): median under 20 seconds.
- Guidance optional: TB-02 with guidance hidden needs no extra steps and changes no settings.
- Interpretation (TB-05): 80% meet the rubric.
- Recovery (TB-07, TB-08): 90% with no data loss.
- Accessibility (TB-13): every essential step possible without a pointer.

## 4. Datasets needed for the battery

Teaching datasets need documented provenance and redistribution permission (TEACH-01). Plan: **create synthetic datasets** for Stage 0–2 testing, so there are no permission issues.

| File | Contents | Used by |
|---|---|---|
| `wellbeing` (.sav, .dta, .csv, .xlsx) | ~240 cases; two-group treatment; stress 1–5 items with two reverse-keyed; employment with value labels and missing codes 8/9; Unicode names; a date | TB-01–11, TB-13 |
| `wellbeing_paired` | Pre/post version of the same | QA03 guidance mismatch |
| `three_groups` | Same outcome, three-level group | QA04 |
| `wellbeing_next_month` | Same schema minus one variable | TB-14 |
