# PrismStatistics: Working Plan

Draft 1 · November 10, 2026 · Built from Product Specification v0.1 and the Development Checklist (105 tasks)

This is my proposed plan for getting from the specification to a working version 1. It covers what to do first, what each early stage produces, and what has to be true before moving on. Nothing has been decided or built yet. Every decision below is a **proposal** until you record it in the decision log.

## The short version

1. **Stage 0 (now):** write down the decisions, the scope, the menu map, and the migration crosswalks, then test the direction with real users. These are the documents in [`docs/stage0/`](docs/stage0/README.md). No product code is written until gate G0 is approved.
2. **Stage 0 spike (inside Stage 0):** build two small, throwaway prototypes of the same thin workflow, one Rust/Tauri and one runtime-backed, so the engine choice (D03) rests on evidence. This is the only code in Stage 0.
3. **Stage 1:** build the foundations: the project format, a data model with stable IDs, the operation log, transactional save, the worker and job lifecycle, CSV/XLSX import, and the numerical reference harness. Build the plain Data View and Variable View shell.
4. **Stage 2:** build one complete workflow, the independent-samples t test, from import all the way to an exported report and a replay after reopening. Test it with novices and with migrating users before adding more methods.
5. **Stage 3 onward:** fill in the version 1 catalog one method at a time. Each method must meet its full contract (setup, computation, diagnostics, explanation, reporting, replay, accessibility, validation) before the next one starts.

## Why this order

The spec's biggest risks are a new numerical engine giving plausible but wrong numbers, metadata lost on import, and an interface that experienced users don't recognize. Each of those gets cheaper to fix the earlier it shows up. So:

- **Familiarity is tested before code.** The menu map and dialogs are checked with users of menu-driven and command-driven packages, R Commander, JASP and PSPP in Stage 0, using clickable mockups.
- **The engine choice uses a real comparison.** Both spikes have to handle value labels, user-missing codes, Welch and Student t, replay and packaging. Each gets measured on validation effort, installer size, startup time, accessibility and licensing.
- **The validation harness comes before the methods.** The reference harness is a Stage 1 deliverable, so the first real procedure is checked against independent values from day one.
- **One slice is finished before the catalog grows.** Stage 2 proves the architecture end to end. Shared pieces (result objects, bound interpretation, stale states, export checks) are built once, and every later method reuses them.

## Stage 0 deliverables and status

| Task | Deliverable | Document | Status |
|---|---|---|---|
| T0-01 | First-release users and task battery | [02-users-and-task-battery.md](docs/stage0/02-users-and-task-battery.md) | Draft |
| T0-02 | Familiar menu and workspace map | [03-workspace-and-menu-map.md](docs/stage0/03-workspace-and-menu-map.md) · [mockups/](mockups/README.md) | Draft + clickable mockups |
| T0-03 | Migration crosswalks (5 packages) | [04-migration-crosswalk.md](docs/stage0/04-migration-crosswalk.md) | Draft, external defaults still to verify |
| T0-04 | Frozen version 1 procedure scope | [05-v1-scope-and-defaults.md](docs/stage0/05-v1-scope-and-defaults.md) | Draft |
| T0-05 | Statistical defaults and naming | [05-v1-scope-and-defaults.md](docs/stage0/05-v1-scope-and-defaults.md) | Draft, needs reference checks and your sign-off |
| T0-06 | Runtime and engine comparison | [06-architecture-evaluation.md](docs/stage0/06-architecture-evaluation.md) | Plan only; spikes not built |
| T0-07 | Distribution, licensing and platforms | [01-decision-log.md](docs/stage0/01-decision-log.md) (D04, D05) | Proposed |
| T0-08 | Scripting and R export boundaries | [06-architecture-evaluation.md](docs/stage0/06-architecture-evaluation.md) §5 | Proposed |
| T0-09 | Accessibility and migration user testing plan | [07-user-testing-plan.md](docs/stage0/07-user-testing-plan.md) | Draft |
| T0-10 | Naming review and reviewer responsibilities | [08-roles-and-gate-review.md](docs/stage0/08-roles-and-gate-review.md) | Draft |
| T0-11 | Direction gate review | [08-roles-and-gate-review.md](docs/stage0/08-roles-and-gate-review.md) §3 | Not started |
| (new) D13 | Same look and feel as PrismQDA | [09-visual-design-alignment.md](docs/stage0/09-visual-design-alignment.md) | Draft |
| (new) D14 | AI-assisted APA write-up in the Report Builder | [10-ai-apa-writeup.md](docs/stage0/10-ai-apa-writeup.md) | Draft |

## Look and feel: matching PrismQDA

PrismStatistics will **look like PrismQDA and work like a statistics package**. It shares PrismQDA's navy title bar, violet accent, pale lavender background, white rounded cards, left module rail, Inter type, outline icon set and spectrum colors. It keeps the classic menu bar, Data View / Variable View and variable-transfer dialogs that users of other statistics packages expect. I've added this as a new decision, **D13**, and recommend pulling the shared tokens, icons and base components into one package both apps use, so they stay matched. Details: [09-visual-design-alignment.md](docs/stage0/09-visual-design-alignment.md).


## Spring 2027 target (decided 2026-11-11)

You want to use PrismStatistics in **spring 2027**, on Windows and on macOS or the web. That's about two to five months away, and the full stage plan (G0 to G7, every 1.0 method with its full contract) won't fit. What makes it possible is PrismQDA: its Tauri shell, React interface, importers and R-validated statistics crate already exist (D03).

A realistic spring release would be a **teaching preview**: data import, Data/Variable View, the descriptive and core inferential methods that `prismqda-stats` already validates, Output, DOCX/HTML export and the template APA writer, on Windows first. The AI writer, tagged PDF, the remaining methods and the macOS build follow as the time allows. I'll draft that cut as a proposal for you to approve before Stage 1 starts.

## AI-assisted APA write-up

The Report Builder gets an optional **Write up with AI** command. A small language model running on the user's own computer reads the verified results and drafts the APA-style paragraph. It never writes numbers itself: it writes placeholders that PrismStatistics fills from the result object, and every draft is checked for wrong numbers, wrong direction, overclaiming and causal language before the user reviews it. It is off by default, with no cloud fallback, and the plain template writer stays as the fallback. A spike runs alongside Stage 2; it ships in 1.0 only if it passes its evaluation, otherwise in a 1.x update. This is new decision **D14**; details in [10-ai-apa-writeup.md](docs/stage0/10-ai-apa-writeup.md).

## What I can do and what needs you or other people

Some Stage 0 work needs people I can't stand in for. I've drafted the plans so that work can start, but these items stay open until they happen:

- **Product owner decisions (you):** D01, D02, D04, D11, D12, plus sign-off on scope.
- **Statistical validation:** decided 2026-11-11: no separate reviewer. Each method and default is checked against R and the other reference packages you have, and you sign it. I prepare the reference scripts and comparisons.
- **User sessions:** at least five participants from each of the five migration backgrounds, plus a novice group (T0-02, T0-09).
- **Naming review:** a trademark and name-clash check on "PrismStatistics" before any public release (D11, T0-10).

## Proposed work after G0 (preview)

Stage 1 goes in this order, following the tracker's dependencies:

1. T1-01 project and dataset schemas → T1-02 source protection and data versions → T1-03 operation API and dependency graph
2. T1-05 numerical reference harness (in parallel with 1)
3. T1-04 transactional save and recovery; T1-10 workers and job states
4. T1-06 CSV/XLSX import; T1-07 SAV/DTA/SAS7BDAT preservation fixtures; `.jasp` data and Google Sheets import (D06)
5. T1-08 shell and docked panels → T1-09 Data View and Variable View
6. T1-11 method registry and result schema; T1-12 R export prototype; T1-13 PrismQDA exchange inspection
7. T1-14 G1 review: import, edit, save, reopen, undo and replay; survive a forced interruption; one simple routine passes independent checks

## Issues I found in the spec

Points to settle before G0. Each one is also noted in the relevant document.

1. **Omega in version 1.** *Decided 2026-11-10: moved to Stage 6.* T3-14 and the version 1 table put "model-qualified omega" in 1.0. The spec also says omega needs a validated measurement model with convergence and admissibility checks, but CFA isn't scheduled until Stage 6 (T6-08). Proposal: limit version 1 to alpha and item analysis, and move omega to Stage 6 with CFA. The other option is a narrow one-factor model built and validated only for omega. *(D01/D08)*
2. **"Statistical defaults and compatibility" heading.** In the .docx this heading has no body text; its content is the defaults table at the end. That's fine, but keep the table and the decision log in sync. The decision log is the source of truth.
3. **R export timing.** D07 (scripting boundaries) has to be decided before G0, but the R export prototype is T1-12 in Stage 1. Proposal: decide the *boundary* at G0 (export only, never executed by Prism, no R input), and let T1-12 prove feasibility.
4. **Two-factor ANOVA sums of squares.** *Decided 2026-11-10: Type III for interaction models, pending the reference check.* The spec proposes Type II for additive models and leaves the interaction-model default to the reviewer. JASP and PSPP users, and most users of menu-driven packages, will expect Type III results for interaction models. This is the single default most likely to cause "your numbers don't match" reports, so it needs an explicit D08 entry. *(See 05 §3.)*
5. **Live preview vs. explicit Run.** JASP users expect live updating. The spec makes explicit Run the default and preview optional (UX-04). That's the right call for reproducibility. The JASP migration guide should say so up front.
6. **PrismQDA already shows statistics.** *Decided 2026-11-10: PrismQDA stays descriptive; tests hand off to PrismStatistics.* Its concept screen 06 includes a Statistics module with a chi-square test. Without a stated boundary, users could get two chi-square implementations with different defaults. Proposal: PrismQDA keeps descriptive counts and crosstabs of coded data, and inferential work hands off to PrismStatistics through the exchange file. *(D10)*
7. **Name.** *Decided 2026-11-11:* PrismStatistics stays, as part of the PrismOpenResearch suite; no trademark check is planned. *(D11)*
