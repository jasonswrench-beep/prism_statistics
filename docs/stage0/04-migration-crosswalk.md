# Migration Crosswalks (T0-03)

Draft 2 · November 11, 2026 · Spec §4 (MIG-01–06), source refs S4–S8

These crosswalks are **design references**. They describe where a migrating user will expect to find things and where the numbers may differ. They don't promise identical behavior, and Prism doesn't copy anyone's interface.

> **Naming rule (product owner, 2026-11-11):** project documents never name a proprietary software package, except when naming a file type (for example `.sav` or `.dta`). Open-source packages (R, R Commander, JASP, PSPP) may be named. Commercial packages are described by their style: *menu-driven* (a data editor, dialogs and a separate output viewer) or *command-driven* (a command line, a results log and script files).

> **Verification status:** the external defaults in §3 come from working knowledge. Each row is checked by running the same data through the reference packages (D08, as decided 2026-11-11) and recording the version used. Rows are marked *Verify* until then.

## 1. Five levels of compatibility (MIG-01)

Every migration guide states these separately, so "opens .sav files" is never read as "runs the syntax that came with them."

| Level | Meaning | 1.0 position |
|---|---|---|
| Interface familiarity | The user can find the same kinds of things in the same kinds of places | **Yes**, tested in Stage 0, 2 and 4 |
| Data compatibility | The file opens with values and metadata preserved | **Yes** for CSV/TSV/XLSX, Google Sheets, and the published `.sav`/`.dta`/`.sas7bdat` matrix (D06) |
| Statistical equivalence | Same numbers given *matched* settings | **Yes**, within published tolerances, once settings are matched (§3) |
| Project compatibility | Their output or project files open | **Partly**: the data inside `.jasp` files opens (D06); their analyses don't. `.spv`, `.smcl`, `.ster` and `.RData` aren't imported (MIG-05) |
| Syntax compatibility | Their scripts run | **No**: aliases help with search only |

## 2. Concept crosswalks

### Menu-driven packages
| What the user knows | PrismStatistics | Notes for the guide |
|---|---|---|
| Data editor with Data View / Variable View | Data tab: Data View / Variable View | Variable View columns: Name, Type, Format, Label, Values, Missing, Measure, Role, plus Units and Notes |
| Measure: Nominal / Ordinal / Scale | Same | Interval/ratio kept as optional detail |
| User-missing values | User-missing, kept distinct from system missing | Never collapsed silently (MIG-03) |
| Analyze menu families | Analyze menu, same family names where they apply | |
| Dialog: OK / Paste | Run / Paste | Paste shows the **Prism operation**, clearly labeled; it isn't another package's syntax |
| Output viewer with an outline | Output tab with an outline | Clicking a heading reopens the settings, which may be new to these users |
| Select Cases / Split File / Weight Cases | Data menu, same names | Always visible in the context bar; only frequency weights in 1.0 |
| Syntax editor | Syntax tab (History + native operations + R export) | No foreign command language |
| Transform > Compute / Recode | Transform > Compute / Recode | Recode "into same" asks for confirmation and preview |

### PSPP
| PSPP | PrismStatistics | Notes |
|---|---|---|
| Data View / Variable View | Same | Shares most content with the menu-driven guide |
| Drop-down procedure dialogs + Paste | Same pattern | |
| Syntax window | Syntax tab | Paste destination is labeled "Prism operation" |
| `.sav` files | `.sav` import with a preservation report | The report lists anything not carried over |

### Command-driven packages
| What the user knows | PrismStatistics | Notes |
|---|---|---|
| Results window or log | Output tab | Structured tables instead of a text log |
| Command line | Search box (Ctrl+K) + Syntax tab | Typing `ttest` *finds* the procedure; it doesn't run foreign syntax |
| Review / history window | History panel (docked) | Same idea: rerun a selected step with Ctrl+Enter |
| Variables window | Variables panel | |
| Properties window | Properties panel | |
| Script files (such as `.do`) | Saved workflows + batch runner | Native records, not foreign scripts |
| Data browser / editor | Data View with a *browse only* switch | |
| Value labels; extended missing `.a`–`.z` in `.dta` files | Value labels; tagged missing preserved | **Key difference:** some command-driven packages treat missing as larger than any number in comparisons; Prism expressions treat missing explicitly (see DAT-07 rules) |
| `by` / `if` / `in` qualifiers | Split file / Select cases rules | Qualifiers become recorded, reversible selection rules |

### R Commander
| R Commander | PrismStatistics | Notes |
|---|---|---|
| Active dataset button | Active dataset in the context bar | |
| Menu dialogs | Analyze menu dialogs | |
| R Script tab (generated code) | Syntax tab → **R export preview** | Prism doesn't run that R; it's export only (D07) |
| Output window | Output tab | |
| Messages window | Separate messages area in the Output and Syntax tabs | Warnings are kept distinct from results |
| Factors / levels | Nominal/ordinal variables with value labels | First level as reference in R; Prism shows and stores the reference explicitly |

### JASP
| JASP | PrismStatistics | Notes |
|---|---|---|
| Options panel beside live results | Dockable analysis options panel in the Output tab | |
| Results update as options change | **Explicit Run by default**; live preview is an optional preference (UX-04) | Say this on the first page of the JASP guide. Every change makes a new, versioned result, which is what makes replay reliable |
| Click a result to reopen its options | Same | |
| Duplicate analysis | Results > Duplicate analysis | |
| Spreadsheet-like data view | Data View | |
| `.jasp` files | Data imported; analyses not | Rerun the analyses in Prism; the guide shows where each one lives |

## 3. Statistical defaults compared (MIG-04)

What a migrating user will see if they *don't* change any settings. To reproduce another package's result, match these first. Commercial packages are checked in the validation suite but, under the naming rule, appear here only as "other packages vary" notes.

| Setting | PrismStatistics (proposed, D08) | R / R Commander | JASP | PSPP | Other packages | Status |
|---|---|---|---|---|---|---|
| Independent t estimator | Welch | `t.test`: Welch; Rcmdr dialog: equal variances "No" | Student; Welch is a checkbox | Shows both rows | Some print both rows; some use pooled variance unless asked | Verify |
| t effect size | Named convention, e.g. Hedges' g with stated SD | Not in base R | Cohen's d | None | Often an unnamed or version-dependent d | Verify |
| Two-factor ANOVA sums of squares | Type II (additive); Type III with sum-to-zero contrasts (interaction) | `anova()`: Type I (sequential); Rcmdr uses `car::Anova` Type II | Type III | Type III | Type III is the common menu-driven default | Verify |
| Contrast / reference coding | Shown and stored by category code | Treatment, first level | First level | Varies | Varies; some use the last category | Verify |
| Logistic event category | User chooses; shown | Second factor level = event | Second level | Higher code | Higher code or nonzero | Verify |
| Correlation missing data | Listwise default; pairwise option with per-pair N | `cor()` gives NA unless `use=` is set | Pairwise | Pairwise | Varies by command | Verify |
| Chi-square 2×2 continuity correction | Off; Yates shown when requested | `chisq.test`: Yates on by default for 2×2 | Off (option) | Printed for 2×2 | Varies | Verify |
| Mann-Whitney exact vs. asymptotic | State which; exact when available and requested | `wilcox.test`: exact for small n without ties | Exact when feasible | Asymptotic | Often asymptotic | Verify |
| Percentile definition | Stated per table (proposed: Hyndman-Fan type 7, configurable) | Type 7 | Varies | Type 6 | Varies (type 6 is common) | Verify |
| SD denominator | n − 1 | n − 1 | n − 1 | n − 1 | n − 1 | Verify |
| User-missing codes in imported files | Excluded, and the exclusion reported | Imports may turn them into values or NA depending on the reader | Depends on import | Excluded | Usually excluded | Verify |
| Confidence level | 95%, stored per analysis | 95% | 95% | 95% | 95% | Verify |

## 4. Migration guide outline (MIG-06)

One short guide per style or package (menu-driven, command-driven, R Commander, JASP, PSPP), same structure, about four pages each:

1. **Open your data:** which formats, what's preserved, how to read the import report.
2. **Check your variables:** where Variable View lives, labels, missing codes, measure.
3. **Find a familiar procedure:** menus and search, worked through the independent-samples t test.
4. **Change an option:** e.g. Welch ⇄ Student; why the default may differ from what you're used to.
5. **Read the output:** outline, tabs (Tables, Diagnostics, Interpretation, Method, Syntax).
6. **Export a report:** DOCX/HTML.
7. **Rerun your work:** History, replay, the batch runner, R export.
8. **Things that work differently here:** the 3–5 biggest differences (e.g. missing values in comparisons; JASP live updates; R reference levels).
9. **Shortcut table.**

## 5. Migration fixture collection (for T1-07 and T4-04)

| Fixture | Contains | Formats |
|---|---|---|
| `mig_labels` | Value labels on numeric and string variables; ordered categories; a non-default reference level | SAV, DTA, CSV + codebook |
| `mig_missing` | `.sav` user-missing (discrete values and a range); `.dta` extended missing `.a`, `.b`, `.z`; system missing | SAV, DTA |
| `mig_text` | Unicode (accents, CJK, RTL), strings over 255 bytes, empty strings vs. missing | SAV, DTA, CSV (UTF-8 and Windows-1252), XLSX |
| `mig_dates` | Dates, datetimes, times; `.sav` and `.dta` date formats; `.xlsx` 1900/1904 date systems | SAV, DTA, XLSX |
| `mig_sheets` | XLSX with several sheets, a merged header, a formula cell, a blank header | XLSX |

Each fixture ships with a **manifest**: the expected canonical values and metadata. An import passes only when it reconciles with the manifest, or when every difference is listed in the import report.
