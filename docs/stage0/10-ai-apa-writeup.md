# AI-Assisted APA Write-Up in the Report Builder (D14)

Draft 1 · November 10, 2026 · Spec GUIDE-06, GUIDE-07, GUIDE-08, OUT-03, OUT-05, SEC-01, TEACH-02, TEACH-03

## 1. What it does

In the Report Builder, the user chooses **Write up with AI** for one or more analyses. A small language model running on their own computer reads the analysis results and drafts an APA-style results paragraph, or a methods and participants description from descriptive results (§3a). The draft lands in the report as an **AI draft** block. The user reviews it, then accepts, edits or discards it.

## 2. Ground rules from the spec

GUIDE-08 already sets the rules for any language-model feature. This design follows them:

| Spec rule (GUIDE-08) | How this design meets it |
|---|---|
| Optional and **off by default** | Off until the user turns it on, in one step, from the Report Builder or Settings › AI writing. No instructor or administrator is needed (decided 2026-11-11). The model is a separate download, not part of the installer |
| Explains verified output; **never calculates authoritative statistics** | The model never writes a number itself. It writes slots like `{t}` and `{df}`, and PrismStatistics fills them from the result object (§4) |
| Identifies model/provider, data sent, output provenance and review status | Every draft shows the model name and version, "ran on this computer", the result versions it read, and *Draft / Reviewed / Edited* status |
| Local only when an actual local model is configured | No cloud fallback. If no model is installed, the button explains how to install one and offers the template writer instead |
| A local model isn't assumed accurate because it's private | Every draft goes through the checks in §5 and needs human review before a final export |

## 3. Two writers, one result

| | Template writer (1.0 baseline, GUIDE-07) | AI writer (this feature) |
|---|---|---|
| How | Fixed, versioned sentence templates per method | Small local model, constrained by a schema |
| Numbers | Bound fields | Bound fields (same) |
| Prose | Correct but formulaic | More natural; can combine several analyses, order findings, and vary wording |
| Always available | Yes | Only when switched on and a model is installed |
| Validation | Unit tests per template | Automated checks every time (§5) plus an evaluation suite before release (§7) |

The template writer stays as the fallback and the reference. If the AI draft fails a check, the user is shown the failure and offered the template version.

## 3a. Methods and participant descriptions (decided 2026-11-11)

The writer can also describe the sample from **Frequencies** and **Descriptives** results. The user picks the variables (for example age, sex and ethnicity), and the paragraph reports whatever statistics they chose for each one.

- **Categorical variables** (from `desc.frequencies`): each category as a percentage with its count, for example "Of the participants, {sex.female.pct}% were female (*n* = {sex.female.n}), {sex.male.pct}% were male (*n* = {sex.male.n}), and {sex.other.pct}% identified otherwise or did not respond (*n* = {sex.other.n})." Missing and "prefer not to say" stay their own category; they are never folded into another one.
- **Scale variables** (from `desc.descriptives`): only the statistics that were run, for example "Participants' ages ranged from {age.min} to {age.max} years (*M* = {age.mean}, *SD* = {age.sd}, *Mdn* = {age.median})." Range, median and the rest appear only if the user asked for them.
- **Extra checks:** category percentages must add to 100 within rounding; the counts must add to the analyzed N; the paragraph states the N it describes; a statistic that wasn't computed can't be mentioned.
- **Template fallback:** the template writer gets the same participant sentences, so the feature works without a model.

## 4. How the model reads output and writes numbers

```
ResultBundle (verified)          Model input (JSON, no raw data)          Model output
──────────────────────          ─────────────────────────────────        ───────────────────────────────
estimates, CIs, df, p,    ──►   method + settings, variable labels,  ──►  "…was lower in the {g2.label} group
n, exclusions, diagnostics      group labels, field IDs with values,      (M = {g2.mean}, SD = {g2.sd}) …
states, design answers          diagnostic states, design answers,        t({df}) = {t}, p {p}, …"
                                APA rules for this method
                                                    │
                                                    ▼
                                PrismStatistics fills every {slot} from the result
                                object with APA formatting → checks (§5) → AI draft block
```

- **No raw data is sent to the model.** It only sees the result summary, labels and settings: the same information already on screen.
- **Slots only.** The output grammar (constrained decoding, e.g. a GBNF grammar in llama.cpp) only allows numbers inside `{slot}` references to fields that exist in the input. A literal digit that isn't part of a slot fails the check.
- **Formatting stays deterministic.** PrismStatistics applies APA formatting when filling slots: italic statistic symbols, no leading zero for p and r, "< .001", decimals. This keeps one formatting rule set for both writers and for tables.
- **Bound like template text.** Filled values stay linked to the result version. If the result changes, the block is marked *Needs review* (GUIDE-07, QA07).

## 5. Checks on every draft (before the user sees it as ready)

| Check | Rule | On failure |
|---|---|---|
| Numbers | Every number comes from a slot that matches a field in the cited result version | Reject; offer the template text |
| Coverage | The required APA elements for the method are present (e.g. t test: means and SDs per group, difference with CI, t, df, p, effect size) | Flag the missing element |
| Sample totals | Participant paragraphs: percentages add to 100 within rounding, counts add to the analyzed N, and only computed statistics are mentioned | Reject; offer the template text |
| Direction | Words like "higher" and "lower" agree with the sign of the estimate | Reject |
| Significance language | No "no effect" or "no difference" from a non-significant result; no "proves"; no importance claim from p alone | Highlight phrase; user must edit or regenerate |
| Causal language | "Caused", "led to", "effect of" only allowed if the recorded design is randomized | Highlight phrase |
| Failed or limited estimates | No interpretation of a failed or suppressed estimate (e.g. logistic separation) | Reject |
| Stale inputs | Results must be current | Block; ask to rerun |
| Diagnostics | Any *Review needed* diagnostic is mentioned or acknowledged | Flag |

Results of the checks are shown in the AI panel and in **Export checks**. A final export needs every AI block to be *Reviewed* or *Edited* (OUT-05).

## 6. Model and runtime (proposed, to verify)

> **Spike results, 2026-11-11 (built in `crates/prism-ai`).**
> - **Runtime:** llama.cpp (MIT), built from source through the `llama-cpp-2` Rust bindings. Building needs CMake and libclang (LLVM), installed on the development PC with the product owner's approval.
>   - The first version used candle (pure Rust). It took 2–2.5 minutes per paragraph because it doesn't batch 4-bit work when reading the prompt.
>   - llama.cpp writes the same t test paragraph in about 17 seconds. Other analyses took 13–46 seconds on the 32-thread development PC.
> - **Model:** Qwen3-4B, 4-bit (Q4_K_M), 2.5 GB, Apache-2.0 weights, pinned revision, SHA-256 checked on download. The tokenizer is inside the model file. Qwen3-1.7B (also Apache-2.0) left out required statistics in the first test.
> - **Constrained decoding:** only English prose characters are allowed, numbers enter only through real `{slot}` names, and spelled-out numbers (including ordinals and run-together words) and asterisk runs are refused. Two slots, `{level}` ("95%") and `{alpha}` (".05"), give the model a legitimate way to state those numbers.
> - **Ways the model tried to get numbers in, all now blocked or caught:** spelling them in words after seeing values in the prompt (so values are never shown), full-width and Chinese numerals, "nine-five per cent", "Ninth", "ninefive", and runs of asterisks.
> - **Checks added after live drafts:**
>   - Significance is judged per cited *p* (correlation pairs, post hoc pairs, regression terms).
>   - A notation check catches a value under the wrong symbol (*t* for an *F*).
>   - Garbled phrases ("was = .020", "= =") are flagged.
>   - The model is told which pairs and terms are significant.
> - **Quality so far:** across t test, ANOVA, correlation, regression and chi-square drafts, every number was right, and every false significance claim and mislabeled statistic was caught. Drafts often need light editing (missing elements, repetitive wording). Participant paragraphs (§3a) still use the template writer. The §7 evaluation hasn't been run.

- **Runtime:** llama.cpp (MIT licence) through Rust bindings, running in a worker process the user can cancel (ARCH-07). It runs on CPU by default and uses the GPU if one is available.
- **Model size:** about 1–4 billion parameters, 4-bit quantized, which is about 1–2.5 GB on disk. That should run on the 16 GB baseline laptop and, slowly, on the 8 GB one. Exact figures are measured in the spike.
- **Candidate models:** small instruction-tuned open-weight models whose **weights are released under MIT or Apache-2.0 only** (product owner decision, 2026-11-10). Models under custom community licences with use policies or attribution terms are excluded. Each candidate's licence is checked on its actual release before testing.
- **Fine-tuning:** start with prompting plus constrained output. Fine-tune (LoRA) on a synthetic corpus of result → APA paragraph pairs only if the evaluation shows it's needed. The corpus can be built from our own template writer plus expert-written examples, so no student or participant data is used.
- **Download:** an optional pack from Settings, with signature and checksum verification (SEC-05). Model name, version and hash are recorded in the project with each draft (REP-05).

## 7. Evaluation before release

- **Corpus:** at least 200 result bundles across every 1.0 method, including edge cases: non-significant results, negative effects, failed estimators, *Review needed* diagnostics, three-group designs.
- **Hard requirements:** 0 number errors (enforced by design); 0 direction errors; 0 failed-estimate interpretations in the corpus.
- **Quality:** the product owner, as APA evaluator (decided 2026-11-11), rates drafts against the template output for accuracy, completeness, APA conformity and readability. Numerical accuracy doesn't depend on the rater, because every number comes from a reference-checked result. The AI writer ships only if it's rated better than the template and has no critical errors.
- **Performance:** time to the first sentence and to a full paragraph on the PERF-01 baseline machines.

## 8. Teaching mode (TEACH-02, TEACH-03)

- AI writing is off by default for everyone, and any user can turn it on themselves (decided 2026-11-11). There's no instructor lock in 1.0.
- The process record keeps student text, template text and AI text separate, and logs when each AI draft was generated, edited and accepted.
- This is a teaching control, not exam security (spec §5).

## 9. Where it fits in the stages

| Stage | Work |
|---|---|
| 0 (now) | Approve D14 and this contract; mockup in the Report Builder |
| 1 | Make sure the result schema has stable field IDs the slots can refer to (T1-11) |
| 2 | Template writer for the t test (T2-06). **AI spike** in parallel: one model, t test only, checks from §5, run against a 50-bundle mini corpus |
| 3 | Extend slots, checks and corpus to every 1.0 method |
| 4 | Full evaluation (§7). The AI panel joins the end-of-project accessibility review (D12) |
| Release | Ship as an optional, off-by-default feature in 1.0 if §7 passes. Otherwise ship 1.0 with the template writer only and release the AI writer in a 1.x update |
