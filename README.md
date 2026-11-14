<p align="center"><img src="PrismStatistics.svg" alt="PrismStatistics logo" width="120"></p>

# PrismStatistics

A planned desktop statistics application for undergraduate learning, applied research and professional analysis. It will have the familiar workspace of established statistics packages such as JASP, PSPP and R Commander, with optional guidance, and it is designed to match its companion product, PrismQDA. Both are part of the PrismOpenResearch suite.

> **Status: building the spring 2027 teaching preview** ([docs/stage0/11-spring-2027-release.md](docs/stage0/11-spring-2027-release.md)). Working now: opening CSV, TSV, `.xlsx`, `.sav`/`.zsav`, `.dta`, `.sas7bdat` (with value labels from its `.sas7bcat` format catalog) and JASP (`.jasp`, data only) files and Google Sheets shared links (with variable labels, value labels and missing-value settings); Data and Variable View with labels and missing codes; every teaching-preview analysis (Frequencies, Descriptives, Crosstabs with chi-square; one-sample, paired and independent-samples t tests; one-way ANOVA with Tukey and Games-Howell; Pearson, Spearman and Kendall correlation; Mann-Whitney, Wilcoxon and Kruskal-Wallis with Dunn; linear regression) and, since the preview cut, factorial ANOVA with Type III sums of squares, binary logistic regression, reliability (Cronbach's alpha), Fisher's exact test, McNemar, Friedman, partial and canonical correlation, and power analysis (sample size or power for t tests, one-way ANOVA, correlation and proportions, needing no data), each checked against R; reopening a result's dialog with its settings; Data View editing behind a browse-only switch; a Paste button in every dialog and a Script screen, with a batch runner (`PrismStatistics.exe run script.pscript --data … --out report.docx`); File › Export › R script, which repeats the project's analyses in base R; and APA-style charts (Graphs: histogram, box plot, bar chart, scatterplot, normal Q-Q plot) in the window and in Word, HTML and PDF exports, each with alternative text and the plotted numbers as a table; and a Report Builder (the Report screen) to choose, order and trim results and add your own headings and text before exporting and each with APA template text; saving and reopening projects (`.pstat`), with results marked out of date when the data changes; and exporting the output to Word (.docx), HTML or tagged, accessible PDF (PDF/UA-1) with APA-style tables; and an optional, off-by-default AI writer (Tools › AI writing) that drafts results paragraphs with a local model whose numbers come only from the checked results; and an optional, off-by-default connection for AI assistants (Tools › Connect an AI assistant, using the Model Context Protocol) that lets an assistant open data, run analyses and export results with the same numbers.

## Start here

- [PLAN.md](PLAN.md) is the working plan: stage order, deliverables and open issues.
- [docs/stage0/](docs/stage0/README.md) holds the Stage 0 documents: decision log, users and test tasks, menu map, migration crosswalks, version 1 scope and defaults, architecture evaluation, user testing, roles, PrismQDA design alignment, and the AI-assisted APA write-up.
- [mockups/](mockups/README.md) has the clickable concept mockups. Open `mockups/index.html` in a browser.
- [docs/source/](docs/source/) holds the product specification (v0.1) and the development tracker. Names of commercial software packages are generalized in these copies, following the project's naming rule.

## Building and testing

You need Rust (1.92 or newer), Node.js, CMake and LLVM (for libclang). The last two build llama.cpp, the AI writer's engine; on Windows `winget install Kitware.CMake LLVM.LLVM` installs both, and `.cargo/config.toml` points to LLVM's usual folder. The statistics come from PrismQDA's `prismqda-stats` crate, which Cargo fetches from the public PrismQDA repository at a pinned commit.

```
cargo test --workspace
```

The interface (from `app/ui`):

```
npm ci
npm test
npm run build
```

Try the interface in a browser against the real engine:

```
cargo run -p prism-cli -- serve --port 4317 --ui app/ui/dist
```

then open http://127.0.0.1:4317. The desktop window (from `app/src-tauri`): `npx --prefix ../ui tauri dev`. For the Windows installer and how to install it, see [docs/install-windows.md](docs/install-windows.md).

The R reference values for the tests are made by the scripts in `crates/prism-engine/tests/reference/` (R 4.6.1).

## Validation and releases

- `python tools/validate.py` runs every reference suite and writes [docs/validation-report.md](docs/validation-report.md): what each suite checks, the R script and package versions behind it, and each test's outcome. Add `--regenerate` to re-run the R scripts first and report any reference value that moved. The product owner signs suites in [docs/validation-signoff.md](docs/validation-signoff.md).
- `python tools/release.py check` confirms the version is the same in every file, and `bump <version>` changes it everywhere.
- `python tools/release.py build` runs the tests and the validation report, then makes an **unsigned** Windows installer in `release/<version>/`, with its SHA-256 and the release notes from [CHANGELOG.md](CHANGELOG.md). Code signing is a separate step for the product owner.

The in-app help (Help menu, F1, the ? in each dialog) lives in `app/ui/src/help`: the user guide in `guide/` and one page per dialog in `topics/`.

## Layout

- `crates/prism-engine`: datasets, imports, analyses, result bundles and the template APA writer
- `crates/prism-api`: the command layer the interface talks to
- `crates/prism-cli`: the `prism` command-line harness and test server
- `app/ui`: the React interface; `app/src-tauri`: the desktop window

## License

Apache License 2.0. See [LICENSE](LICENSE).
