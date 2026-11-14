# Architecture Evaluation Plan (T0-06, T0-07, T0-08)

Draft 1 · November 10, 2026 · Spec §15, §17, §18 · Decisions D03, D04, D05, D07

## 1. Goal

Choose the engine and desktop shell (D03) on evidence. Both candidates build the **same thin spike**, are scored on the **same criteria**, and the spikes are thrown away afterwards. Spike code doesn't become product code.

## 2. Candidates

| | A: Rust engine (lead) | B: Bundled statistical runtime (challenger) |
|---|---|---|
| Shell | Tauri 2 | Tauri 2 |
| UI | TypeScript + React, Prism component library | Same |
| Computation | Rust crates (e.g. `nalgebra`/`faer` for linear algebra, `statrs` for distributions), with our own validated implementations where crates fall short | Python (NumPy, SciPy, statsmodels) or R, run as a bundled sidecar process |
| Data | Arrow-style columnar store in Rust | Arrow / pandas in the runtime; IPC to the shell |
| Main appeal | Small installer, fast startup, one language for engine and shell back end, strong typing for the result schema | Mature, well-tested numerical libraries; faster method coverage later |
| Main worry | We must implement and validate more numerical code ourselves (the spec's "new numerical engine" risk) | Installer size (often 150–400 MB); startup time; IPC overhead; R's GPL licence obligations if bundled; packaging complexity |

Keeping Tauri and React fixed across both candidates also keeps the PrismQDA look-and-feel work (D13) independent of the engine choice.

## 3. The spike (same for both, about 2 weeks each)

1. Open `mig_labels.sav` and `mig_missing.dta`: values, value labels, user-missing and extended missing preserved.
2. Show a Data View grid of 100,000 × 20 rows and columns with keyboard navigation and screen-reader announcements (NVDA).
3. Run Welch and Student independent t tests in a **worker**, with cancellation.
4. Return a structured `ResultBundle` (spec §17) and render it as an accessible table.
5. Record the operation, save the project, reopen it, replay, and get an equivalent result.
6. Build a Windows installer.

## 4. Scorecard

| Criterion | Weight | How measured |
|---|---|---|
| Numerical validation effort | 20 | Estimated person-weeks to validate the full 1.0 catalog (§1 of 05), based on what the libraries already give us |
| Correctness on the spike | Pass/fail | Welch/Student match the reference fixtures within tolerance |
| SAV/DTA metadata fidelity | 15 | Fixture manifests reconciled |
| Accessibility of the grid and dialogs | 15 | NVDA + Narrator walk-through of spike steps 2–3 |
| Installer size and cold start | 10 | MB; seconds to the welcome screen (PERF-01 baseline machine) |
| Large-data responsiveness | 10 | Open + scroll the 100k × 20 grid; descriptives time |
| Licensing obligations | 10 | Dependency licence inventory; compatibility with Apache-2.0 distribution |
| Maintainability | 10 | Languages a contributor must know; build reproducibility; dependency count |
| Sharing with PrismQDA | 10 | How much shell, components and exchange code can be shared (needs a look at PrismQDA's actual stack) |

The technical lead writes the rationale into D03. A tie goes to the option with lower validation effort.

## 5. Scripting and R export boundaries (T0-08, D07)

| Item | 1.0 | Later |
|---|---|---|
| Native operation record (JSON, versioned schema) | Executed by Prism; shown in History; replayed; used by the batch runner | Possibly an editable text form |
| Native scripting language | **None.** Nobody has to learn a new language for standard work | Decide after Stage 5 |
| R script export | Generated for supported procedures; includes data reference, transforms, options, packages and exclusions; with a translation report | Compared against pinned R in CI (REP-04) |
| Running R | **Never** by Prism. The user opens the exported file in their own R | Optional configured bridge with isolation, timeouts and "externally computed" labels |
| Other packages' syntax | Not accepted; aliases only help search | Not planned |
| Opening a project with scripts inside | Nothing runs automatically (QA13) | Same |

## 6. Distribution and platform (T0-07)

- **Distribution:** free, offline installer, no account, no telemetry by default (SEC-01). Update checks are a separate, visible preference.
- **Licence:** Apache-2.0 for Prism source (the repo already has it). A dependency inventory is generated from the build (e.g. `cargo-about` / `license-checker`) and reviewed at G0 and at every release.
- **Signing:** Windows Authenticode code signing is needed before public release (Stage 5). The certificate cost and ownership need deciding under D12.
- **Platforms:** Windows 10/11 x64 first. macOS and Linux follow, each with its own validation (G7).
