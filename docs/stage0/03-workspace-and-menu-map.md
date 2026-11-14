# Workspace and Menu Map (T0-02)

Draft 1 · November 10, 2026 · Spec FAM-01–04, §6, §7 · Visual treatment: [09-visual-design-alignment.md](09-visual-design-alignment.md)

This map is frozen **for the prototype only**, not for the final product. Items marked **Later** show in search and help with an availability label but never appear as working commands (spec §6).

## 1. Workspace layout

```
┌────────────────────────────────────────────────────────────────────────────┐
│ [logo] PrismStatistics   Wellbeing Study.prsm ● saved          – ▢ ✕      │  navy title bar
├────────────────────────────────────────────────────────────────────────────┤
│ File Edit View Data Transform Analyze Graphs Results Tools Help  [Search ⌃K]│  menu bar
│ [Open][Save][Undo][Redo] | [Variables][Run][Help me choose] | [Explain]    │  compact toolbar
├──────┬───────────────┬────────────────────────────────────┬────────────────┤
│ Home │ History       │ [Data] [Output] [Syntax] [Report]  │ Variables      │
│ Data │ (dock L/btm)  │                                    │ Properties     │
│ Out- │               │   main document area               │ Explain this   │
│ put  │               │                                    │ (dock right)   │
│ ...  │               │                                    │                │
├──────┴───────────────┴────────────────────────────────────┴────────────────┤
│ Dataset: wellbeing v3 · Filter: none · Weight: none · Split: off · N=240   │  context bar (UX-01)
└────────────────────────────────────────────────────────────────────────────┘
```

- **Left rail** (shared with PrismQDA): Home, Data, Output, Syntax, Report, Learn; Settings at the bottom. This is the spec's "optional project navigator." It can be collapsed to icons or hidden, and it mirrors the document tabs rather than replacing the menus.
- **Document tabs:** Data (with Data View / Variable View toggle), Output, Syntax, Report.
- **Docked panels:** Variables and Properties on the right; History on the left or bottom; *Explain this result* and *Help me choose* also dock on the right.
- **Context bar** is always visible: active dataset and version, filter, weight, split, included N (UX-01).
- **Narrow windows:** panels collapse into tabs in this order: History → Properties → Variables. The menu bar and document area never collapse.
- **Layout presets** (View menu): *Standard*, *More help*, *More scripting*, *Reset layout*. They change panels only, never statistical settings (UX-06).

## 2. Menu map

| Menu | Item | Shortcut | 1.0? | Spec |
|---|---|---|---|---|
| **File** | New dataset | Ctrl+N | ✔ | SCR01 |
| | Open data… (CSV, TSV, XLSX, SAV, DTA) | Ctrl+O | ✔ | SCR02 |
| | Open project… | Ctrl+Shift+O | ✔ | |
| | Open example dataset… | | ✔ | TEACH-01 |
| | Save project / Save As… | Ctrl+S / Ctrl+Shift+S | ✔ | ARCH-03 |
| | Import from PrismQDA… | | ✔ (pilot) | INT-03 |
| | Export ▸ Data (CSV, XLSX) · Tables (CSV) · Report (DOCX, HTML) · Figures (PNG, SVG) · R script · Reproduction package | | ✔ | OUT-04, REP-04/05 |
| | Export ▸ PDF (tagged) | | ✔ (D09) | OUT-04 |
| | Recent ▸ | | ✔ | |
| | Close / Exit | Ctrl+W / Alt+F4 | ✔ | |
| **Edit** | Undo / Redo | Ctrl+Z / Ctrl+Y | ✔ | DAT-14 |
| | Cut / Copy / Paste (with paste preview) | Ctrl+X/C/V | ✔ | SCR03 |
| | Find / Replace | Ctrl+F / Ctrl+H | ✔ | |
| | Go to case / Go to variable | Ctrl+G | ✔ | |
| | Preferences… | Ctrl+, | ✔ | SCR12 |
| **View** | Data View / Variable View | Ctrl+T toggles | ✔ | FAM-03 |
| | Output / Syntax / Report | Ctrl+1 … Ctrl+4 | ✔ | |
| | Show value labels | | ✔ | DAT-03 |
| | Panels ▸ Variables · Properties · History · Explain · Navigator rail | | ✔ | |
| | Layout ▸ Standard · More help · More scripting · Reset | | ✔ | |
| | Zoom in / out / reset | Ctrl+= / Ctrl+- / Ctrl+0 | ✔ | ACC-04 |
| **Data** | Data health… | | ✔ | DAT-05 |
| | Sort cases… | | ✔ | |
| | Select cases… (reversible rule) | | ✔ | DAT-10 |
| | Split file… | | ✔ | DAT-10 |
| | Weight cases… (frequency weights only) | | ✔ | DAT-13 |
| | Merge datasets… / Append cases… | | ✔ | DAT-06, DAT-09 |
| | Aggregate… / Restructure (wide ⇄ long)… | | ✔ | DAT-09 |
| | Identify duplicates… | | ✔ | DAT-06 |
| | Dataset manager… | | ✔ | |
| | Survey design… | | Later | DAT-13 |
| **Transform** | Compute variable… | | ✔ | SCR04, DAT-07 |
| | Recode ▸ Into new variable · Into same variable | | ✔ | DAT-08 |
| | Reverse score… | | ✔ | DAT-08 |
| | Composite score… | | ✔ | DAT-08 |
| | Rank cases… / Standardize (z)… | | ✔ | DAT-07 |
| | Date and time… / Text… | | ✔ | DAT-07 |
| | Random numbers… (seeded) | | ✔ | DAT-07 |
| | Multiple imputation… | | Later | DAT-12 |
| **Analyze** | Descriptive Statistics ▸ Frequencies · Descriptives · Explore · Crosstabs | | ✔ | §10 |
| | Compare Means ▸ Means by group · One-Sample T · Independent-Samples T · Paired-Samples T · One-Way ANOVA | | ✔ | |
| | General Linear Model ▸ Univariate (two-factor between-subjects) | | ✔ | |
| | General Linear Model ▸ Repeated measures · Multivariate · ANCOVA | | Later | |
| | Correlate ▸ Bivariate · Partial | | ✔ | |
| | Regression ▸ Linear · Binary Logistic | | ✔ | |
| | Regression ▸ Ordinal · Multinomial · GLM · Mixed models | | Later | |
| | Nonparametric ▸ Chi-square goodness of fit · Mann-Whitney · Wilcoxon signed-rank · Kruskal-Wallis · Friedman · McNemar | | ✔ | |
| | Scale ▸ Reliability (alpha, item analysis) | | ✔ | |
| | Scale ▸ Omega · Factor analysis | | Later (see D01) | |
| | Power and Sample Size ▸ t tests · One-way ANOVA · Correlation · Proportions | | ✔ | |
| | Help me choose… | Ctrl+Shift+H | ✔ | SCR06 |
| **Graphs** | Chart Builder… | | ✔ | SCR09 |
| | Distribution ▸ Histogram · Density · Boxplot · Dot/strip · Q-Q | | ✔ | CHART-01 |
| | Groups ▸ Bar (with labeled intervals) · Error bars | | ✔ | |
| | Relationships ▸ Scatter · Correlation heatmap · Line | | ✔ | |
| | Model diagnostics ▸ Residual · Influence | | ✔ | |
| **Results** | Go to output item… | | ✔ | SCR07 |
| | Reopen settings | Enter on heading | ✔ | |
| | Run again / Refresh stale | F5 | ✔ | ARCH-05 |
| | Duplicate analysis | | ✔ | |
| | Compare versions… | | ✔ | |
| | Copy / Export item | | ✔ | |
| | Add to report | Ctrl+Shift+R | ✔ | SCR10 |
| **Tools** | Replay project… / Run selected steps | Ctrl+R / Ctrl+Enter | ✔ | REP-02 |
| | Workflow and batch setup… | | ✔ | REP-03 |
| | Data dictionary… | | ✔ | |
| | Method validation records… | | ✔ | VAL-05 |
| | PrismQDA exchange… | | ✔ (pilot) | §16 |
| | Open in external R… | | Later (D07) | |
| **Help** | Help me choose… | | ✔ | |
| | Help for this procedure | F1 | ✔ | |
| | Glossary / Worked examples | | ✔ | TEACH-01 |
| | Migration guides ▸ From menu-driven packages · command-driven packages · R Commander · JASP · PSPP | | ✔ | MIG-06 |
| | Statistical defaults compared… | | ✔ | MIG-04 |
| | Keyboard shortcuts / Accessibility | | ✔ | ACC-02 |
| | About (version, engine, build) | | ✔ | |

**Shortcut rules:** Windows conventions win over cloning another package. Ctrl+Enter runs the selection in the Syntax tab (familiar from R script editors). All shortcuts are listed in Help and can be remapped, so there's always an alternative when one conflicts with a screen reader.

## 3. Search aliases (FAM-02)

The search box (Ctrl+K, the same as PrismQDA's "Search in project") matches procedure names, aliases and menu paths. Aliases help people find things; they don't mean their syntax runs.

| Procedure | Aliases |
|---|---|
| Independent-samples t test | t test, ttest, t-test, two sample t, independent samples, compare two means, Welch, Student t, `ttest`, `T-TEST` |
| Paired t test | paired, dependent t, repeated t, pre post, matched pairs |
| One-way ANOVA | anova, oneway, F test, compare several means, Welch ANOVA |
| Two-factor ANOVA | two-way anova, factorial anova, univariate GLM, UNIANOVA |
| Chi-square | crosstabs, contingency table, chi2, tabulate, association |
| Mann-Whitney | Wilcoxon rank-sum, U test, ranksum |
| Pearson correlation | correlation, r, correlate, pwcorr, bivariate |
| Reliability | Cronbach, alpha, internal consistency, scale reliability, item analysis |
| Linear regression | regression, OLS, lm, regress, linear model |
| Binary logistic | logit, logistic, glm binomial, odds ratio |
| Descriptives | summary, summarize, mean, sd, describe |
| Frequencies | freq, tab, tabulate, counts, table |

## 4. Procedure dialog pattern (SCR05)

Every 1.0 analysis dialog uses this same layout, so migrating users see the familiar variable-transfer pattern:

```
┌ Independent-Samples T Test ─────────────────────────────── [?] ✕ ┐
│ ┌ Variables ─────────┐      ┌ Test variable(s) ──────────────┐  │
│ │ 🔍 search          │  ▶   │ stress_mean   Scale            │  │
│ │ id         ID      │  ◀   └────────────────────────────────┘  │
│ │ age        Scale   │      ┌ Grouping variable ─────────────┐  │
│ │ group      Nominal │  ▶   │ group (1=Control, 2=Treatment) │  │
│ │ stress_1   Ordinal │      └ [Define groups…] ──────────────┘  │
│ └────────────────────┘                                          │
│ ▸ Options   Estimator: (●) Welch ( ) Student pooled  CI: 95%    │
│ ▸ Missing values   Exclude cases analysis by analysis           │
│ ▸ Effect size      Hedges' g (pooled SD) ▾                       │
│ ⓘ What's the difference between Welch and Student?              │
│                       [Run] [Paste] [Reset] [Cancel] [Help]      │
└─────────────────────────────────────────────────────────────────┘
```

- Arrow buttons and the keyboard (Enter on a variable → focus moves to the role picker) do the role transfer; drag and drop is optional.
- Variables that can't go in a role stay visible but disabled, with a reason on focus. A numeric-coded category can't be a test variable (UX-02).
- Plain-language explanations sit beside the technical labels; they're never substituted for them (FAM-03).
- **Run** computes. **Paste** opens the Syntax tab with the native operation (and the R preview, labeled as R), but computes nothing (FAM-04).

## 5. Open questions for the prototype round

1. Does the left rail help or get in the way for users of menu-driven and command-driven packages? Test with the rail shown and with it collapsed.
2. Is "Results" a clear menu name, or should it be "Output" to match the output viewers users already know?
3. Ctrl+R for *Replay* vs. Ctrl+Enter for *Run selection*: check for screen-reader conflicts (NVDA, JAWS, Narrator).
