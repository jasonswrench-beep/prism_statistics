# Accessibility and Migration User Testing Plan (T0-09)

Draft 1 · November 10, 2026 · Spec §2, §4, §18 (ACC-01–05)

## 1. Rounds

| Round | When | What's tested | Material | Participants |
|---|---|---|---|---|
| R0 Discovery | Stage 0 | Menu map, workspace layout, dialog pattern, PrismQDA-style visuals | Clickable mockups (HTML) | 5 per package × 5 packages + 5 novices = 30 |
| R2 Vertical slice | Stage 2 | TB-01–08, TB-11, TB-13 | Working slice | 3 per package + 5 novices + 3 assistive-technology users |
| R4 Pilot | Stage 4 | Full battery | Release candidate | Classroom pilot (one course section) + 5 professional users |
| R-A Accessibility review | End, before release (D12, decided 2026-11-11) | Keyboard model, focus order, naming, screen readers, tagged PDF | Release candidate | 2–3 screen-reader users; 1 keyboard-only; 1 low-vision |

The spec calls five per group a **formative** target. These sessions find usability problems; they don't produce representative statistics. Add sessions when problems stay unresolved.

## 2. Recruiting

- **Migrating users:** people who've used the package in the last 12 months. Record the package version and how long they've used it.
- **Novices:** students who haven't finished a statistics course with software.
- **Assistive technology:** NVDA and JAWS users on Windows, a Narrator check, a keyboard-only user, and a 200% zoom / high-contrast user.
- **Consent and data:** synthetic datasets only (see 02 §4). Record screen and audio only with consent. Store notes without names.
- **Ethics:** if the sessions are run through a university, check whether an IRB/HREB determination is needed before recruiting.

## 3. What gets recorded per task

Package background; version; platform; assistive technology used; outcome (unaided / assisted / failed); time; path (menu / search / guidance); errors; the participant's own words at points of confusion; and their confidence rating.

## 4. Accessibility checks built into every round

| Area | Check | Spec |
|---|---|---|
| Keyboard | Every essential task with no pointer; visible focus; no focus traps; variable transfer by keyboard | ACC-02 |
| Grid | Row/column position, variable label, value or missing status, and edit state are announced | ACC-03 |
| Visual | Text and non-text contrast; status never shown by color alone; 200% zoom; high contrast; reduced motion | ACC-04 |
| Output and charts | Tables have headers; charts have a summary and a data table; exported DOCX/HTML tested with a screen reader | ACC-05 |
| Announcements | Run started, finished and failed use polite live regions; errors are focusable | ACC-04 |

Automated tools (axe-core on the web UI, Accessibility Insights for Windows) run in CI from Stage 1. They support manual testing; they don't replace it.

## 5. How findings are handled

Each finding becomes a tracker item with a severity: **Critical** (blocks a task or causes a wrong conclusion), **High**, **Medium**, **Low**. If the interface contributed to a misunderstanding, it's logged as a product defect, not as user error (spec §21, Stage 4). Gates G2 and G4 require that no critical or high findings are left open.
