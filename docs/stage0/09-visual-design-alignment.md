# Visual Design Alignment with PrismQDA (D13)

Draft 1 · November 10, 2026 · Sources: PrismQDA concept screens 01–08, `Website/refract-kit/assets/refract.css`, PrismQDA Icon Foundation Pack v0.1, PrismQDA Main Logo SVG pack

## 1. The rule

**PrismStatistics looks like PrismQDA and works like a statistics package.**

- *Shared:* colors, type, icons, title bar, left rail, cards, buttons, inputs, callouts, spacing, motion, and light, dark and high-contrast themes.
- *Different on purpose:* the menu bar (File … Help), Data View / Variable View, the variable-transfer dialogs, and the output outline. The spec requires these for migrating users (FAM-01–04). PrismQDA uses a ribbon (Home / Code / Analyze / View); PrismStatistics uses a classic menu bar plus a slim toolbar in the same visual style.

Someone switching between the two apps should feel they're in the same family. Someone coming from a menu-driven package should still find Analyze > Compare Means.

## 2. What PrismQDA's screens establish

| Element | PrismQDA treatment | PrismStatistics use |
|---|---|---|
| Title bar | Deep navy bar: logo, product name, project name, module name; window controls | Same: `[mark] PrismStatistics │ Wellbeing Study │ Data`, plus a saved/unsaved dot |
| Left rail | White rail; icon + label; active item is a lavender pill with violet text and icon; Team / Settings at the bottom | Same component. Items: Home, Data, Output, Syntax, Report, Learn; Settings at the bottom. Collapsible |
| Background | Very pale lavender wash behind content | Same |
| Cards | White, rounded (~12 px), hairline lavender border, soft shadow | Dialogs, output items, welcome tiles, Variable View panels |
| Primary button | Violet, rounded, white label (e.g. *Run Analysis*, *New Project*) | **Run** in every dialog; *New dataset* on Welcome |
| Secondary button | White card with outline icon above or beside the label | Paste, Reset, Cancel, Help; Open data, Open project, Examples |
| Tabs | Text tabs with a violet underline on the active one | Data / Output / Syntax / Report; result tabs (Tables, Diagnostics, Interpretation, Method, Syntax) |
| Info callout | Pale blue box with an ⓘ icon, plain-language sentence | *Explain this result* text; GUIDE-02 status messages (with text labels, never color only) |
| Panels | Headed side panels with close ✕ (Sources, Codes, Coding Inspector) | Variables, Properties, History panels |
| Search | Rounded field, "Search in project (Ctrl+K)" | Same placeholder and shortcut, extended to procedure aliases |
| Data visuals | Violet area/line charts; spectrum donut; large stat numbers on cards | Project overview; but **statistical charts follow CHART-01–05**, not dashboard styling |
| Welcome | Large heading; three big action tiles; Recent Projects list; Get Started checklist | Same layout: New dataset, Open data, Open project, Examples; recent projects; *Get started* (import data → check variables → run an analysis → export a report) |

## 3. Design tokens (shared)

Taken from PrismQDA's `refract.css`. Shared tokens should live in **one package both apps use** (see §6), not be copied.

| Token | Value | Use |
|---|---|---|
| `--prism-navy` | `#151A3D` | Title bar, headings, app icon tile |
| `--prism-deep` | `#0B0F2B` | Dark theme base |
| `--prism-ink` | `#151A3D` | Body text (light theme) |
| `--prism-muted` | `#565C80` | Secondary text |
| `--prism-accent` | `#6555C4` | Primary buttons, active states, focus ring |
| `--prism-paper` | `#FFFFFF` | Cards, panels |
| `--prism-wash` | `#F5F4FB` | App background |
| `--prism-line` | `#E3E2F0` | Borders, grid lines |
| `--prism-c1`…`c6` | `#22C7EE` `#3D7BF2` `#7C5CF0` `#E04FB3` `#FF7A59` `#FFB23E` | Spectrum: brand accents and decorative gradients only |
| `--prism-mono` | Cascadia Mono, Consolas, … | Syntax tab, operation records, numbers in the grid (tabular) |

**Type:** Inter (the wordmark is built from Inter Display), with tabular figures on for every numeric table and grid. Bundle the font so it works offline. Inter is under the SIL Open Font License.

**Icons:** use the PrismQDA icon standard: 24×24, outlined, `stroke="currentColor"`, 1.75 stroke, round caps and joins, with filled `-active` variants for the selected rail item. Shared icons (home, data, report, settings, search) come straight from the PrismQDA pack. Statistics-specific icons (Variable View, compute, recode, t test, regression, chart builder, syntax, replay) are new but follow the same standard.

## 4. What statistics needs that PrismQDA doesn't have

These are new components, designed in the same visual language:

| Component | Notes |
|---|---|
| **Data View grid** | Dense, virtualized. Uses `--prism-line` gridlines, a sticky header, frozen ID column and tabular figures. Missing values show a distinct glyph plus a screen-reader text. Value-label mode uses italics as a second cue. Selection is the accent at low opacity with an accent outline |
| **Variable View** | Same grid component; editable cells; Values and Missing open small card dialogs |
| **Variable-transfer dialog** | Card dialog: variable list on the left, arrow buttons, role boxes on the right, collapsible option groups, button row Run (primary) · Paste · Reset · Cancel · Help |
| **Output outline + result card** | Each analysis is a card with a heading (click to reopen settings), result tabs, and a status chip (*Completed*, *Stale*, *Failed* …) that has a text label and an icon, not color alone (UX-05) |
| **Statistical tables** | APA-like styling: horizontal rules only, no vertical lines; right-aligned decimals; a footnote area for N, missing data and method |
| **Context bar** | Slim bar under the document area: dataset · filter · weight · split · N. Always visible (UX-01) |
| **Syntax tab** | Mono font. Native operations and R export in **separate labeled panes** with different header colors and text labels ("Prism operation", "R export: not run by Prism") |
| **Charts** | Statistical charts aren't decoration. Check the chart palette for color-vision deficiency and contrast; mark intervals with text labels. The spectrum can supply categorical hues only after it passes those checks |

## 5. Things to check before approving D13

1. **Contrast:** measure `--prism-accent` on white and on `--prism-wash`, and `--prism-muted` on `--prism-wash`, against WCAG 2.2 AA (4.5:1 text, 3:1 non-text). Adjust shades if needed in **both** apps.
2. **Logo family:** the PrismStatistics mark (prism columns under a bell curve) uses a violet-to-rainbow palette. The PrismQDA mark is a faceted prism with cyan, blue, violet, pink and orange faces. They read as siblings. Decide whether the PrismStatistics app icon should also sit on the `#151A3D` rounded square, as PrismQDA's `app-icon.svg` does, for a matching taskbar look. I recommend yes.
3. **Wordmark:** make a PrismStatistics wordmark matching PrismQDA's (Inter Display SemiBold "Prism" + Light "Statistics").
4. **PrismQDA's own Statistics module** (screen 06 shows a chi-square test inside PrismQDA). Agree the boundary under D10, so users aren't given two different chi-square implementations with possibly different defaults.
5. **Dark and high-contrast themes:** PrismQDA's icon pack previews light, dark and high-contrast. PrismStatistics should ship all three from the same tokens.

## 6. Recommendation: one shared design package

Pull the tokens, fonts, icons and base components (title bar, rail, card, buttons, tabs, inputs, callout, dialog shell) into a shared package, for example `@prism/ui`, used by both apps. Then a change to the accent color or an icon lands in both products at once, and the "same feel" holds as both grow. Whether this is practical depends on PrismQDA's actual front-end stack, which is part of the D03 scorecard ("Sharing with PrismQDA") and T1-13.

## 7. Stage 0 mockups to build (for round R0)

Clickable HTML mockups in the Prism style, using synthetic data:

1. Welcome / project launcher
2. Data View and Variable View, with the context bar
3. Analyze menu open → Independent-Samples T Test dialog
4. Output with outline, result card, tabs and *Explain this result*
5. Help me choose (docked)
6. Syntax tab with native operation + R export panes
7. Report builder (matching PrismQDA screen 08)

These are what participants see in R0 (07 §1). They're mockups only: no real calculations, and placeholder numbers are labeled as such.
