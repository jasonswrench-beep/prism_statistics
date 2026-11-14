# PrismStatistics Concept Mockups (Stage 0, round R0)

Open `index.html` in a browser. It's one self-contained file that works offline, except for the Inter web font, which falls back to Segoe UI.

- **Edit** `src/index.html`, then run `python mockups/build.py` to rebuild `index.html` with the logo embedded. It also writes `dist/artifact.html` (not committed), the version published as the shareable link.
- **Screens button** (bottom right) jumps to each screen for facilitators: Welcome, Data View / Variable View, the t-test dialog, Output, Help me choose, Syntax, Report builder, and Write up with AI.
- **Look and feel** follows [docs/stage0/09-visual-design-alignment.md](../docs/stage0/09-visual-design-alignment.md) (shared with PrismQDA).

## What's real and what isn't

- **Data** is synthetic: 240 cases generated from a fixed seed. It includes value labels, user-missing codes (8, 9) and two cases with no group.
- **Numbers** in the output, interpretation and report are computed from that data, so every screen agrees. The Welch and Student results were cross-checked against SciPy's `ttest_ind` on November 10, 2026. This is still mockup arithmetic, **not** a validated implementation, and it's labeled as placeholders in the UI.
- **Commands** not covered by the test tasks show a short message instead of doing anything. Items planned for later releases appear disabled with a "Later release" label, as the spec requires.

## Things participants can try (task battery IDs from 02)

TB-01 (value label for code 9) · TB-02 (find and run the t test via menu or search) · TB-03 (Help me choose) · TB-04 (switch Welch ⇄ Student and run again) · TB-05 (write an interpretation, then reveal the generated one) · TB-06 (Paste → Syntax tab) · TB-08-style stale result ("Simulate data edit") · TB-09 (search aliases) · TB-11 (report export checks) · TB-13 (keyboard only: F10 for menus, arrow keys, T/G in the dialog's variable list)

## Write up with AI (screen 8, decision D14)

In the Report builder, **Write up with AI** opens a side panel. *Generate draft* simulates a small local model writing an APA results paragraph. No model actually runs in the mockup. The draft shows the intended design: the model writes only wording and `{slots}`, PrismStatistics fills every number from the verified result, and the draft passes visible checks before the user can Accept, Edit, Discard or switch back to the template version. The **Show what the model wrote** switch reveals the slots. A final export is blocked while an AI draft is unreviewed. Design: [docs/stage0/10-ai-apa-writeup.md](../docs/stage0/10-ai-apa-writeup.md).
