# Changelog

## Unreleased

### Large data

Timed with simulated files of up to 1,000,000 cases and up to 500 variables.

- Data View draws only the cells in view, so wide and long data scroll and edit as quickly as small data. Scrolling 500 cases by 500 variables went from about a quarter of a second per step to a few hundredths.
- Undo keeps the columns a change didn't touch shared with the data, so each change keeps a copy of one column rather than of the whole dataset. Memory after 15 changes to 1,000,000 cases by 20 variables went from 5.8 GB to 0.7 GB.
- Out-of-date checks are kept until the data an analysis reads change, and run several at a time. Adding a result, opening a project and exporting no longer run every earlier analysis again; a saved project keeps each result's check.
- An analysis that takes more than a second is marked out of date when its data change, instead of being run again to check.
- Mixed models stop searching when the fit stops improving relative to its size; on 100,000 cases a model that took over nine minutes takes about a second.
- Scatterplots, box plot outliers and ROC curves draw a mark once for each spot on the chart, so a chart of 1,000,000 cases is 1.8 MB instead of 45 MB, and looks the same.

### Keyboard

- Every dialog takes the keyboard focus when it opens and gives it back when it closes, and Tab and Shift+Tab stay inside it.
- Each variable list is one tab stop: the arrow keys (and Home and End) move through it, Enter picks a variable and Enter again moves it, and the focus stays in the list after a move.
- After Run, the focus is on the new result's heading.

### Fixes

- The "In total, N cases were excluded" sentence appears once in the independent-samples t test paragraph, not twice.
- The Mann-Whitney note no longer reads "a Online value"; it describes pairs of cases, one from each group.
- Every method note ends "Computed by PrismStatistics." (some named the shared statistics crate and its version).
- Opening a text file says how many rows were short, how many had extra fields (which were left out) and how many blank rows were skipped.
- A document, a JSON file or another file that isn't data is refused with a message listing the types that open, instead of opening as an empty or one-column dataset. A .txt file is read with the delimiter its first line uses.
- A numeric column that looks like an identifier (a different whole number for nearly every case) opens as nominal, not scale, matching Data Health.
- With value labels on, a user-missing code shows its label (in amber italics), such as "Prefers not to say".
- The status bar names the data as the title bar does.
- Variable View's Missing and Measure columns are wide enough for their contents again, and the AI writer names the model folder without the account name.

## 0.1.1 (teaching preview)

Fixes for faults found while writing the user manual. The installer and the program are code-signed.

### Fixes

- Closing Help while an analysis dialog was open left a second copy of that dialog that couldn't be closed. Help now closes cleanly and returns focus to the dialog.
- The item response theory paragraph could give the BIC as the largest item difficulty. The range now comes from the difficulties alone.
- Panel regression: when the Hausman statistic is negative (the covariance difference isn't positive definite), the test is reported as not computed, with a note, instead of a negative chi-square.
- The meta-regression note on the test of moderators now gives its p value.
- Descriptive statistics name the variable that caused cases to be excluded ("excluded: empty in exam").
- Turning off Browse only in Data View no longer freezes the program on large data. The grid has one tab stop, and the arrow keys move between cells (Enter edits).
- Bar charts slant long category names, cutting them at 28 characters, instead of printing them over each other.
- Variable View's Missing column says it takes codes separated by commas, and a range such as 90-99 gets a message that explains this.
- Messages and the recovery window name a file without its folders, so the account name in a path isn't shown on screen.
- Help and the changelog say exactly when a result goes out of date, and name the batch runner by its installed file, `prismstatistics-app.exe run`.

## 0.1.0 (teaching preview)

The first release for classes and research, on Windows. The installer and the program are code-signed.

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
- Results go out of date when a change to the data would change their numbers (an edited value the analysis used, a new missing-value code, a recode into the same variable), and can be run again with the same settings. A change the analysis does not use, such as a recode into a new variable, leaves them as they are.
- Report Builder.
- Export to Word, HTML, tagged PDF (PDF/UA) and R script. A single figure can be saved as PNG or SVG, and a single table copied or saved as CSV.
- Scripts (Paste, Run all, a project's history as a script) and a batch runner (`prismstatistics-app.exe run`, the installed program file).
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

- The .xlsx reader's XML library (quick-xml 0.38) has two denial-of-service advisories (RUSTSEC-2026-0194, -0195). It will be updated when the reader moves to a fixed version; workbooks are size-checked before reading meanwhile.
- A security advisory for the `glib` library (GitHub Dependabot alert 1) affects only Linux builds of the desktop shell. The Windows build doesn't include that library. It will be resolved when the Tauri framework moves to the fixed version.
