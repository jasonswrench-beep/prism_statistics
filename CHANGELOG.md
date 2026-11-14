# Changelog

## 0.1.0 (teaching preview, not yet released)

The first release for classes and research, on Windows. The installer is not code-signed yet.

### Data

- Opens CSV and TSV, .xlsx and .xlsm, .sav and .zsav, .dta, .sas7bdat (with value labels from a .sas7bcat catalog) and .jasp files, plus online spreadsheets through a shared link.
- Data View and Variable View, with labels, value labels, missing-value codes and measures.
- Paste from a spreadsheet, with a preview that flags values that don't fit.
- Undo and Redo (Ctrl+Z, Ctrl+Y) for up to 30 changes to the data, variables, case selection, scripts and the output. An undone change also leaves the project's record.
- Find a value (Ctrl+F) and go to a case or variable (Ctrl+G) in Data View.
- Data Health: finds numbers stored as text, likely missing-value codes, identifiers measured as scale, differing spellings and duplicate cases. Each fix is previewed before it is applied.
- Compute, Recode and Scale Score.
- Sort Cases, Rank Cases, Standardize (z-scores), Reverse Score, and Date and Time (date parts and the time between two dates, from text dates or other programs' date numbers). Each is checked against R, and the R script export repeats it.
- Select Cases, Split File and Weight Cases.
- Add Cases, Add Variables, Aggregate and Restructure.

### Analyses

77 analyses, each checked against R. The full list is in `docs/statistics-checklist.md` and the evidence in `docs/validation-report.md`.

- Descriptive statistics and crosstabs.
- t tests and ANOVA, including robust trimmed-mean tests.
- Correlation and regression: linear, logistic, ordinal, multinomial, count, penalized, panel and dynamic panel.
- Mediation and moderation.
- Factor analysis and reliability.
- Agreement statistics and item response theory.
- Structural equation models (covariance-based, Bayesian, multilevel and PLS) with a visual model builder.
- Complex samples, time series and survival.
- Classification: ROC, latent class and profile analysis, decision trees, random forests and gradient boosting.
- Quality control charts and design of experiments.
- Meta-analysis (classical, multilevel and Bayesian).
- Permutation tests.
- Missing-data patterns and multiple imputation.
- Mixed models and GEE.
- The nonparametric tests.
- Power analysis.

### Output and writing

- APA tables and figures, with assumption checks and a method note on every result.
- A template APA paragraph on every result, plus an optional AI-written draft whose numbers are checked against the result.
- Results go out of date when the data change, and can be run again with the same settings.
- Report Builder.
- Export to Word, HTML, tagged PDF (PDF/UA) and R script. A single figure can be saved as PNG or SVG, and a single table copied or saved as CSV.
- Scripts (Paste, Run all, a project's history as a script) and a batch runner (`PrismStatistics.exe run`).
- Connection for AI assistants through the Model Context Protocol (MCP). Off by default.

### Reliability

- Saving checks the new file before it replaces the old one, and keeps the replaced version as `<name>.backup.pstat`.
- Unsaved work is copied every 30 seconds and offered for recovery after a crash.
- Long analyses show their progress and can be cancelled. A cancelled analysis adds nothing to the output.

### Settings

- Tools › Settings: text size (up to 130% for a projector), whether Data View opens browse-only and with value labels, and the confidence level new dialogs start with.

### Help

- A user guide and a help page for every dialog, opened from **Help**, the **?** button in each dialog, or **F1**.
- **Explain this result** under each result: what the analysis asks, what each of its numbers means here, the checks that need attention, and what the result can't show.
- Three example datasets (File › Open Example Data), simulated for teaching with full codebooks, and three worked examples in Help that use them.
- A glossary of the terms in the output (Help › Glossary).
- **Help me choose** (Ctrl+Shift+H): questions about the study's design that suggest an analysis, explain why it fits and name the alternatives, then open it with the chosen variables filled in.

### Security

From an independent review before release. A project, data file or script from someone else is treated as untrusted:

- R script export never copies a project's text in as code: formulas and Select Cases conditions are rebuilt from the engine's own reading of them, names and text are quoted, and anything else is left out with a note.
- Chart pictures in a project are rebuilt from the parts the program's own charts use, so they can't carry scripts, links or outside images; drawing reads no file, and oversized pictures are refused. Exported HTML has a strict content security policy.
- Network and device paths (`\\server\share`) are refused everywhere. A script lists the files it will write before it runs and replaces one only when you agree. An AI assistant can't replace files, and the data's own text reaches it marked as data.
- Uninstalling with **Delete the application data** ticked also removes the AI model and settings.
- Projects, workbooks, `.jasp` and `.zsav` files are checked against a 1 GB unpacked size; a project whose data don't add up is refused; an internal failure no longer stops the engine.
- CSV export keeps formula-like text as text.
- The statistics library is fetched from the public PrismQDA repository at a pinned commit, and release builds keep the build folders' paths out of the program.

### Known issues

- The installer is not code-signed, so Windows asks for confirmation the first time it runs.
- The .xlsx reader's XML library (quick-xml 0.38) has two denial-of-service advisories (RUSTSEC-2026-0194, -0195). It will be updated when the reader moves to a fixed version; workbooks are size-checked before reading meanwhile.
- A security advisory for the `glib` library (GitHub Dependabot alert 1) affects only Linux builds of the desktop shell. The Windows build doesn't include that library. It will be resolved when the Tauri framework moves to the fixed version.
