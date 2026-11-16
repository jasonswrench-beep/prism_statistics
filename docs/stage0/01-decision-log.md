# Decision Log (Stage 0)

Draft 1 · November 10, 2026 · Mirrors the *Decisions* sheet of the tracker; this file is the source of truth once approved.

**Status values:** Open → Proposed → Approved (or Rejected / Superseded). To approve, fill in *Decision record* with date, decider and any changes, then copy the status to the tracker.

---

## D01 Initial audience and version 1 scope
- **Before gate:** G0 · **Owner:** Product owner · **Spec:** §2, §3, §10
- **Proposal:** Version 1 serves undergraduate teaching and applied social-science research: one-sample, paired and independent t tests; one-way, Welch and two-factor between-subjects ANOVA; chi-square, 2×2 Fisher and McNemar; four rank tests; Pearson, Spearman, Kendall and partial correlation; alpha and item analysis; linear and binary logistic regression; power for the listed tests. Full list in [05-v1-scope-and-defaults.md](05-v1-scope-and-defaults.md).
- **Change from spec:** move **omega** out of 1.0 and into Stage 6 alongside CFA, unless a narrow one-factor model is separately funded and validated. Reason: the spec requires a validated measurement model for omega, and that model is otherwise a Stage 6 capability.
- **Alternatives:** (a) keep omega via a minimal one-factor ML fit with its own validation corpus; (b) smaller 1.0 that drops two-factor ANOVA and logistic regression to reach a pilot sooner.
- **To close:** product owner sign-off on the catalog; statistical reviewer agrees on the omega question.
- **Status:** Approved · **Decision record:** 2026-11-10, product owner: **omega moves to Stage 6** with CFA; 1.0 ships alpha and item analysis. 2026-11-11, product owner: **the rest of the catalog is approved as written** (two-factor ANOVA and logistic regression stay in 1.0).

## D02 Default workspace and panel presets
- **Before gate:** G0 · **Owner:** Product owner + UX lead · **Spec:** §1, §6, FAM-01–04
- **Proposal:** Open into a conventional workspace: menu bar File/Edit/View/Data/Transform/Analyze/Graphs/Results/Tools/Help, Data/Output/Syntax/Report document tabs, optional docked Variables, Properties and History. Three layout presets (*Standard*, *More help*, *More scripting*) change panel visibility only. Visual styling follows the shared Prism look (see D13). Detail in [03-workspace-and-menu-map.md](03-workspace-and-menu-map.md).
- **To close:** prototype passes the T0-02 discovery tasks with migrating users.
- **Status:** Approved · **Decision record:** 2026-11-11, product owner: approved as proposed. User testing can still change details of the menu map.

## D03 Engine and desktop shell
- **Before gate:** G0 · **Owner:** Technical lead · **Spec:** §17
- **Proposal:** Build two time-boxed spikes against the same thin slice and choose with the scorecard in [06-architecture-evaluation.md](06-architecture-evaluation.md). Leading candidate: Rust engine + Tauri 2 shell + TypeScript UI. Challenger: Tauri/TypeScript UI over a bundled statistical runtime (Python with NumPy/SciPy, or embedded R).
- **Also weigh:** what PrismQDA is built on. Sharing a shell, component library and exchange code with PrismQDA is a real cost saving and helps the "same feel" goal (D13). This needs a look at PrismQDA's actual implementation (T1-13 starts that early).
- **New evidence (2026-11-11):** PrismQDA (github.com/jasonswrench-beep/prismqda) is already built on **Rust + Tauri 2 + React 19 with React Aria + Vite**, Apache-2.0, with a dependency licence checker. Its `prismqda-stats` crate (about 6,000 lines) already implements and validates against R: distributions, descriptives, t tests, one-way ANOVA with post hoc tests, chi-square and Fisher, correlations including partial, the rank tests, Shapiro-Wilk, linear and logistic regression, and the bootstrap. Its importer already reads `.sav` and `.zsav`.
- **Revised recommendation:** adopt PrismQDA's stack and move `prismqda-stats` (and later the `.sav` reader) into a **shared crate both apps depend on**, instead of comparing two new engines. One short spike confirms the thin slice (06 §2) on that stack; the challenger runtime is dropped unless the spike fails.
- **To close:** product owner agrees to the shared-crate approach; confirming spike passes.
- **Settled (2026-11-15, development list item 4):** one statistics crate for both apps, published from the shared
  core (openstage-core), versioned by tag, and both apps pin the same tag. A fix is made once, in the core, and reaches
  each app when it moves its pin and its own reference tests pass. The core does not hold the crate yet, so it is asked
  for in `CORE-REQUESTS.md` (R5) rather than built here. Until then PrismStatistics keeps its pin to PrismQDA commit
  56a161e, and `tools/check_shared_stats.py` (run by every release build) says when PrismQDA's copy has changed since
  the pin. On 2026-11-15 it had not: both apps compute with the same code.
- **Status:** Proposed (recommendation changed 2026-11-11) · **Decision record:** —

## D04 Distribution and source licensing
- **Before gate:** G0 · **Owner:** Product owner · **Spec:** §1, §17
- **Proposal:** Free, local-first, no account. Source under **Apache-2.0** (already on the GitHub repository). Before approval, produce a dependency licence inventory from the D03 winner. Note: a bundled R runtime brings GPL obligations, which affects the challenger option.
- **To close:** licence inventory reviewed; confirm Apache-2.0 is compatible with every bundled component. PrismQDA's `prismqda-licensecheck` tool already fails a build on non-permissive dependencies and can be reused.
- **Status:** Approved · **Decision record:** 2026-11-11, product owner: Apache-2.0; **no bundled R**. R is used only as an export target and as a reference package for validation tests.

## D05 Initial platforms and minimum hardware
- **Before gate:** G0 · **Owner:** Technical lead · **Spec:** §19 PERF-01
- **Proposal:** Windows 10/11 x64 first. Baseline: 4 modern cores, 16 GB RAM, SSD; constrained scenario at 8 GB. macOS next, then Linux, each only after native file, font, accessibility (VoiceOver/Orca) and numerical validation.
- **To close:** baseline machine identified and recorded.
- **Status:** Approved (Windows first) · **Decision record:** 2026-11-11, product owner: Windows first, developed on the product owner's PC. A macOS or web version is also needed by spring 2027 (D12); Tauri builds for macOS from the same code, but needs a Mac to build, sign and test on. That PC (Intel Core i9-13900K, 128 GB RAM, GeForce RTX 3050, Windows 11 Home) is far above the 16 GB and 8 GB baselines, so performance checks (PERF-01) and the AI model timings run under a constrained setup, such as a virtual machine limited to 4 cores and 16 GB, then 8 GB. A real low-end laptop check before release is still recommended.

## D06 Supported import file versions
- **Before gate:** G1 · **Owner:** Data lead · **Spec:** MIG-02, DAT-04
- **Proposal:** Publish a version and metadata matrix before claiming SAV/DTA support. Starting target: SAV (uncompressed and bytecode-compressed; ZSAV as stretch), DTA formats 117–119. Fixtures cover value labels, user-missing ranges and discrete values, long strings, Unicode, dates, and `.dta` extended missing values (.a–.z).
- **Status:** Direction set · **Decision record:** 2026-11-11, product owner: the first import set also includes **`.sas7bdat` data files**, **`.jasp` files (data only; the analyses inside them aren't carried over)** and **Google Sheets**. **Google Sheets, decided 2026-11-11 by the product owner: shared link, no sign-in.** The user pastes the link of a sheet shared as "Anyone with the link"; PrismStatistics downloads it once as `.xlsx` and reads it like any workbook. No account, no sign-in, and nothing but the request for that sheet is sent, so private data stays private and the local-first rule (D04) holds apart from that one download. Private sheets can't be opened; a downloaded `.xlsx`/`.csv` always works offline.
- **Built (2026-11-11):** `.sav`/`.zsav`, `.dta`, `.sas7bdat` (value labels from catalog files not yet read), `.jasp` data (JASP 0.17 and later) and Google Sheets links. Checked against R's haven and, for JASP, the product owner's 118-file JASP data library.

## D07 Scripting and external execution
- **Before gate:** G0 · **Owner:** Technical + statistical leads · **Spec:** §15
- **Proposal:** In 1.0, the native, structured operation record is the only thing Prism executes. R scripts are **export only**: never run by Prism, never accepted as input, and every export carries a translation report. No invented scripting language in 1.0. An editable native language or an R bridge is chosen after Stage 5. Detail in 06 §5.
- **Status:** Approved · **Decision record:** 2026-11-11, product owner: R stays **export only**, and R is also a comparison package for testing. No Python export in 1.0.

## D08 Statistical defaults
- **Before gate:** G0 · **Owner:** Statistical reviewer · **Spec:** §10
- **Proposal:** The defaults table in [05-v1-scope-and-defaults.md](05-v1-scope-and-defaults.md) §3. Key items: 95% intervals; two-sided tests; Welch t by default; listwise deletion per procedure; reference category chosen and stored by code; Type II sums of squares for additive models, with an **open question** on the interaction-model default; explicit multiplicity families; quantile type stated.
- **To close:** each line is checked by reference runs, and the product owner signs it.
- **Status:** Partly decided · **Decision record:** 2026-11-10, product owner preference: **Type III sums of squares (sum-to-zero contrasts) for two-way models with an interaction**, labeled in output with Type II one click away. 2026-11-11, product owner: **there is no separate statistical reviewer**; methods and defaults are validated by running the same data in R and the other reference packages the product owner has, and the product owner signs each line.

## D09 PDF export readiness
- **Before gate:** G3 · **Owner:** Reporting + accessibility leads · **Spec:** OUT-04
- **Proposal:** Ship DOCX and accessible HTML in 1.0. Tagged PDF only after a semantic-tagging and screen-reader check passes. No "print to PDF" presented as accessible output.
- **Status:** Approved (changed) · **Decision record:** 2026-11-11, product owner: **tagged PDF is in 1.0** alongside DOCX and HTML. Its tagging and screen-reader check joins the end-of-project accessibility review (D12); if that check fails, PDF export is labeled *not accessible* rather than removed.
- **Built (2026-11-11):** PDF/UA-1 export typeset by Typst 0.15 (embedded, Apache-2.0), whose exporter refuses to write a file that fails PDF/UA-1. Checked so far: structure tree (headings, tables with header cells, lists, paragraphs), document language and title, embedded fonts. Still to do in the accessibility review: an independent validator and a screen-reader pass.

## D10 PrismQDA integration contract
- **Before gate:** G1 · **Owner:** Integration lead · **Spec:** §16
- **Proposal:** Versioned, file-based case table exchange with provenance (INT-01–03). **New point:** PrismQDA's concept screens already show a *Statistics* module (descriptive, inferential, chi-square). The two products need a stated boundary before G1. Suggested split: PrismQDA keeps quick in-context counts and crosstabs on coded data; anything inferential or model-based hands off to PrismStatistics through the exchange file, and PrismQDA labels its own statistics as descriptive.
- **Status:** Direction set · **Decision record:** 2026-11-10, product owner: **PrismQDA stays descriptive** (counts and crosstabs of coded data). Any test or model hands off to PrismStatistics through the exchange file, so each test has one implementation. The exchange schema itself is still open until G1.
- **Exchange route (2026-11-15, development list item 5):** the case table travels as a `.pstat` file (PSTAT-1, the
  shared core's format), one row per case. PrismStatistics now reads a `.pstat` file in Add Variables and Add Cases, so
  a case table merges onto survey data by its case id. The layout PrismQDA writes (case id, attributes, each code's
  presence and count, and where they came from) is a new profile of PSTAT-1, asked for in `CORE-REQUESTS.md` (R6).
- **Conflict found (2026-11-11), needs the product owner:** PrismQDA's own roadmap puts t tests, one-way ANOVA, correlation, linear regression and the rank tests in *its* 1.0, and its `prismqda-stats` crate already computes them. If both apps share one statistics crate (D03), the "one implementation" goal holds even if PrismQDA keeps running tests. Either PrismQDA's roadmap changes to match this decision, or this decision changes to "shared engine, both apps may run tests".

## D11 Naming and visual identity
- **Before gate:** G0 · **Owner:** Product owner + design lead · **Spec:** §1, §8
- **Proposal:** Keep *PrismStatistics* as the working name pending a name and trademark check. Logo: the supplied prism-histogram mark (`PrismStatistics.svg`). Visual identity is shared with PrismQDA per D13.
- **Status:** Approved · **Decision record:** 2026-11-11, product owner: the name stays **PrismStatistics**. It's one product in the **PrismOpenResearch** suite (suite mark `PrismOpenResearch.svg`), alongside PrismQDA, and the product owner doesn't consider confusion with other products a concern. No trademark check is planned.

## D12 Staffing, funding and calendar estimates
- **Before gate:** G1 · **Owner:** Product owner · **Spec:** §21
- **Proposal:** No dates until G1 evidence exists.
- **Status:** Partly decided · **Decision record:** 2026-11-11, product owner: the team is **the product owner and Claude only**. No separate statistical reviewer (see D08). Major accessibility review is **deferred to the end**; simple accessibility questions go to the product owner. The product owner is the APA evaluator (D14). **Calendar:** the product owner wants to **use the product in spring 2027**, with both a Windows and a macOS (or web) version by then; the current focus is Windows only. This is far sooner than the full stage plan allows, so a reduced spring release has to be defined (see PLAN.md, *Spring 2027 target*).

## D13 Shared Prism look and feel *(new, not in spec v0.1)*
- **Before gate:** G0 · **Owner:** Product owner + design lead
- **Proposal:** PrismStatistics uses the same visual language as PrismQDA: color tokens, typography, icon style, title bar, card and panel treatment, and the left module rail. It keeps the **interaction model** the spec requires for statistics users: the conventional menu bar, Data View/Variable View, and variable-transfer dialogs. Detail in [09-visual-design-alignment.md](09-visual-design-alignment.md).
- **Why:** the products are meant to be used together (§16). A shared look tells users they're one family and lets both share a component library.
- **Status:** Approved · **Decision record:** 2026-11-11, product owner: approved after reviewing the mockups.

## D14 AI-assisted APA write-up *(new, not in spec v0.1)*
- **Before gate:** G0 (direction) · G4 (release decision) · **Owner:** Product owner + statistical reviewer + technical lead · **Spec:** GUIDE-06–08, OUT-03/05, SEC-01
- **Proposal:** An optional, off-by-default **Write up with AI** command in the Report Builder. A small model (about 1–4B parameters) runs **locally** through llama.cpp and drafts APA-style results text from the verified result object. The model never writes numbers: it emits slots that PrismStatistics fills from the result, and every draft passes automated checks (numbers, direction, significance and causal language, failed estimates, staleness) plus human review before final export. No cloud fallback. The deterministic template writer (GUIDE-07) remains the baseline and fallback. Detail in [10-ai-apa-writeup.md](10-ai-apa-writeup.md).
- **Release rule:** ships in 1.0 only if the evaluation in 10 §7 passes; otherwise it moves to a 1.x update without delaying 1.0.
- **To close:** approve the contract; choose candidate models after the licence check under D04.
- **Status:** Proposed (licence rule and evaluator decided) · **Decision record:** 2026-11-10, product owner: candidate models must have **permissive licences only (MIT or Apache-2.0 weights)**. 2026-11-11: the **product owner is the APA evaluator**. Contract approval is still open. **Built as an experimental feature, 2026-11-11** (Tools › AI writing; llama.cpp runtime, about 15–45 s per paragraph on the development PC; Qwen3-4B Apache-2.0, constrained to slots, checks from 10 §5, review before export; see 10 §6 for the spike results). The release decision under §7 is still open. 2026-11-11, product owner: the AI also drafts **methods and participant descriptions** from descriptive results (10 §3a), and it is **off by default but any user can turn it on** in one step; no instructor control is needed.

## D15 Connection for AI assistants (MCP) *(new, not in spec v0.1)*
- **Before gate:** G0 (direction) · **Owner:** Product owner + technical lead · **Spec:** SEC-01, OUT-03
- **Proposal:** PrismStatistics can act as a Model Context Protocol (MCP) server, so an AI assistant the user already uses can open data, run the analyses and export results through the same command layer as the window. The assistant starts `PrismStatistics.exe --mcp` (or `prism mcp`) and talks over standard input and output; nothing listens on the network. The assistant gets the engine's numbers, already rounded the APA way, with the checks and the template paragraph, and is told never to recompute them.
- **Privacy:** off by default (Tools › Connect an AI assistant). Individual data rows need a second switch, also off by default; without it the assistant sees variable information and results only. Both switches are read on every call, so turning one off stops a connected assistant at once. The dialog says plainly that an online assistant sends what it reads to its provider. Files are written only to full paths the assistant names, and an existing file is never replaced unless the call says so (the assistant is told to ask the user first).
- **Prompts:** three ready-made MCP prompts walk the assistant through reporting correctly: *Analyze and report in APA style*, *Check a write-up against the results*, and *Explain a result in plain language*. Each carries the same reporting rules (numbers copied exactly, significance matched to *p*, no causal wording without a randomized design, checks and exclusions reported, out-of-date results rerun).
- **Scope:** each connection is its own session; results reach the window through a saved project (.pstat). No sign-in, no network listener, no AI writer calls (the assistant writes its own prose).
- **Status:** Built, 2026-11-11, at the product owner's request ("we should also have an MCP option"). Tested with the official MCP Python client over stdio against both `prism` and the desktop app.

## D16 Analyses after the preview, and canonical correlation *(scope change)*
- **Before gate:** G1 · **Owner:** Product owner · **Spec:** §3, §10
- **Request:** 2026-11-12, product owner: keep building the analyses listed for after the preview (factorial ANOVA with Type III sums of squares, logistic regression, reliability, Fisher, McNemar, Friedman, partial correlation, power), and add **canonical correlation** "in the correlation area or possibly under regressions".
- **Scope change (spec §3):** canonical correlation is new to the 1.0 catalog. *User task:* relate one set of measures to another (for example several attitude scales with several behavior measures). *Contracts:* `corr.canonical` in 05 §1, under Analyze › Correlate. *Validation:* R's `cancor` for the correlations and coefficients, with the Wilks' lambda tests (Rao's F) worked out in base R. *Dependencies:* none beyond the engine. *Release impact:* small; it adds one dialog.
- **How the others landed:** Fisher's exact test is an option of Crosstabs (two-by-two tables only), not a separate menu item. The two-way ANOVA (`anova.twoway`) became `anova.factorial`, which takes two or three factors with every interaction, Type III sums of squares with sum-to-zero contrasts (05 §3 row 8) and partial eta squared. Logistic regression reports profile-likelihood intervals for the odds ratios (05 §1).
- **Status:** Built, 2026-11-12, each checked against R (`tests/more_reference.rs`). Power analysis (Analyze › Power Analysis) is built too: one-sample, paired and independent t, one-way ANOVA, Pearson r and one or two proportions, solving for the sample size or the power, with the effect entered as a standardized size or in raw units (05 §1). It needs no data; results run before data is opened move into the project when data arrives. The formulas are those of R's `pwr` package, checked against base R's noncentral t and F.
