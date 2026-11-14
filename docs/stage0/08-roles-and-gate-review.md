# Roles, Naming Review and G0 Gate (T0-10, T0-11)

Draft 2 · November 11, 2026 · Spec §21, §23

## 1. Team and review roles

The team is **two people: the product owner and Claude** (product owner, 2026-11-11). The spec's roles are still useful as a checklist of responsibilities, but they map onto the two of us, and independent human review is replaced by evidence that can be checked.

| Responsibility | Approves | Held by | How it's checked |
|---|---|---|---|
| Product owner | Scope, priorities, every decision, every gate | Jason Wrench (confirmed 2026-11-11; sole final decider) | — |
| Statistical validation | Method contracts, defaults (D08), validation records | Claude prepares; product owner signs | Every method and default is checked by running the same data in **R and the other reference packages the product owner has**, within published tolerances (decided 2026-11-11). Reference scripts and outputs are committed with the tests |
| Technical lead | Architecture (D03, D05, D07), build and release | Claude proposes; product owner decides | Spike scorecard; automated tests |
| UX / design | Workspace, dialogs, visual identity (D13) | Claude drafts; product owner approves | Mockups; side-by-side review with PrismQDA |
| Accessibility | ACC requirements | Claude builds to the requirements and runs automated checks; simple questions go to the product owner | **Major accessibility review is deferred to the end** (before release), decided 2026-11-11. Keyboard-complete dialogs, names and focus order are still built from the start so the late review doesn't force rework |
| APA evaluation (D14) | AI write-up quality | Product owner | Ratings against the template writer (10 §7) |
| QA | Test evidence, QA01–QA16 | Claude | CI results |
| Data / import (D06) | Import adapters | Claude | Fixture manifests |
| Integration (D10) | PrismQDA exchange | Product owner (also owns PrismQDA) | Exchange-file round-trip tests |

**Claude's part:** drafting documents, writing code and tests, and preparing evidence. Claude doesn't count as an independent reviewer, which is why the statistical sign-off rests on reference-package results rather than on Claude's judgment.

**Deviation from spec:** spec §21 asks for an independent statistical reviewer and an accessibility reviewer before Stage 2. This plan replaces the first with reference-package validation and moves the second to the end. The risk is that a method can match the reference packages and still use a questionable default; the defaults comparison (04 §3) and the method notes are the guard against that.

## 2. Naming (D11)

Decided 2026-11-11: the product is **PrismStatistics**, part of the **PrismOpenResearch** suite with PrismQDA. The product owner doesn't consider name confusion a concern, so no trademark search is planned.

## 3. G0 review checklist (T0-11)

G0 passes when all of these exist and the product owner signs them:

- [ ] Decision log: D01, D02, D03, D04, D05, D07, D08, D11 and D13 are **Approved**; D06, D09, D10 and D12 have target gates
- [ ] Approved v1 scope (05 §1–2)
- [ ] Defaults table signed (05 §3), including #8, with the reference-package check planned for each line
- [ ] Migration crosswalk external defaults verified by reference runs (04 §3)
- [ ] R0 prototype results: discovery and familiarity findings, with critical issues fixed in the menu map
- [ ] Architecture rationale (06 §4), now informed by PrismQDA's existing stack (D03)

**Record:** date, decision (pass / pass with conditions / not yet), conditions, and links to evidence. Copy the result to the Roadmap sheet's *Gate review* column.
