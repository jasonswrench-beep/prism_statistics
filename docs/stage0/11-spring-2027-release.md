# Spring 2027 Release Cut

Draft 1 · November 11, 2026 · Decisions D01, D03, D05, D12

## 1. Why a cut

The product owner wants to use PrismStatistics in **spring 2027**, on Windows and on macOS or the web (D12). The full stage plan won't fit in that time. This document defines the **teaching preview**: the smallest release that is genuinely usable for teaching and simple research, built so that nothing in it has to be thrown away for 1.0.

The cut is possible because PrismQDA already has a Tauri desktop shell, a React interface in the shared look, file importers, and an R-validated statistics crate (`prismqda-stats`). PrismStatistics reuses that crate rather than writing a second engine (D03).

## 2. What the teaching preview includes

| Area | In the preview | After the preview (toward 1.0) |
|---|---|---|
| Platforms | **Windows** installer | macOS build (Tauri builds from the same code; needs a Mac to sign and test), or a web build of the same interface |
| Data | CSV/TSV, XLSX/XLSM, `.sav`/`.zsav`, `.dta`, `.sas7bdat`, `.jasp` data and Google Sheets (shared link) import; Data View and Variable View with labels, value labels, missing codes and measure; save and reopen a project | **Built after the cut:** `.sas7bdat` value labels from a format catalog (`.sas7bcat`), read automatically from beside the file or chosen in File › Import. Still to come: more `.sav`/`.dta` edge cases (D06) |
| Analyses | Frequencies, Descriptives, Crosstabs with chi-square; one-sample, paired and independent t tests; one-way ANOVA with Tukey and Games-Howell; Pearson, Spearman and Kendall tau-b correlation; Mann-Whitney, Wilcoxon, Kruskal-Wallis; linear regression | **Built after the cut (D16):** factorial ANOVA (Type III), binary logistic regression, reliability (Cronbach's alpha), Fisher's exact test, McNemar, Friedman, partial and canonical correlation, and power analysis |
| Output | Output tab with tables, method notes and diagnostics; DOCX, HTML and tagged PDF (PDF/UA-1, D09) export | **Built after the cut:** charts (histogram, box plot, bar, scatter, Q-Q) in every export, R script export, Paste and scripts with a batch runner, reopening a result's settings, and the Report Builder. Still to come: independent PDF/UA validation |
| Writing | Template APA writer for every included method, plus participant descriptions (10 §3a); optional connection for AI assistants (MCP, D15) | AI writer (D14), after its evaluation passes |
| Reproducibility | Every analysis recorded as a native operation; History; rerun | Batch runner; R export |
| Validation | Every included method checked against R in the test suite; defaults table signed for the included methods | Remaining methods and reference-package checks |

Every analysis in the preview is already implemented and validated against R in `prismqda-stats`, so the preview's risk is in the workspace, data model and output, not in the numbers.

## 3. Build order

1. **Workspace and engine slice** (now): Cargo workspace sharing `prismqda-stats`; dataset model with variables, value labels and missing codes; CSV import; independent-samples t test end to end with a structured result; reference test against R.
2. **Desktop shell**: Tauri + React app in the shared Prism look; Data View and Variable View; the t test dialog; Output tab.
3. **Project file**: save and reopen, with the operation record and replay.
4. **Breadth**: the remaining preview analyses, each with its dialog, output, template text and R reference test.
5. **Export**: DOCX and HTML.
6. **Installer** and a classroom dry run.

## 4. What this changes in the plan

- Stage gates G1 to G4 are compressed into the preview. Each method still meets its contract (05 §5) before it's shown to students.
- The full user-testing rounds (07) shrink to the product owner's own classes as the first pilot.
- 1.0 remains the target for the full catalog; the preview is a subset of 1.0, not a separate product.
