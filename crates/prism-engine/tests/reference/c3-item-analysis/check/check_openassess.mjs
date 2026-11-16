// Runs OpenAssess's item statistics (itemStatistics in packages/assess/src/sheets/stats.ts) on every case and compares
// its figures with reference.json.
//
//   node check/check_openassess.mjs --assess <OpenAssess clone> [--reference <file>]
//
// Each case's scores.csv becomes the scored sheets OpenAssess's statistics read: one complete sheet per row, each
// question scored with the points in its cell, and a question with an empty cell not counting on that sheet. For a
// case with letters (answers.csv) each sheet also carries the answers marked, as OpenAssess's scoring keeps them (the
// bank's option ids, here the version A letters; true and false for a true/false question). Needs Node 23.6 or later,
// which runs OpenAssess's TypeScript source directly. Exit code 0 when every figure matches.

import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { readCase } from "../make_pstat.mjs";
import { flags, reference, Tally } from "./common.mjs";

// OpenAssess shows its figures to 4 places, so a figure within half a unit of the 4th place matches.
const TOLERANCE = 0.00005 + 1e-12;

const args = flags(process.argv.slice(2));
if (!args.assess) {
  console.error("usage: node check/check_openassess.mjs --assess <OpenAssess clone>");
  process.exit(2);
}
const stats = await import(pathToFileURL(join(resolve(args.assess), "packages", "assess", "src", "sheets", "stats.ts")).href);

const ref = reference(args.reference);
const tally = new Tally("OpenAssess");
let groupFigures = false;

/** The bank question OpenAssess would hold for a case's question. */
function questionOf(it) {
  if (it.type === "true_false") return { id: it.id, type: "trueFalse", points: it.possible, body: { correct: it.correct[0] === "A" } };
  const options = (it.options ?? []).map((l) => ({ id: l, content: "", correct: (it.correct ?? []).includes(l) }));
  if (it.type === "select_all" || it.partial_credit) return { id: it.id, type: "multipleResponse", points: it.possible, body: { shuffle: false, partialCredit: it.partial_credit, options } };
  return { id: it.id, type: "multipleChoice", points: it.possible, body: { shuffle: false, options } };
}
/** An answer cell as OpenAssess's scoring keeps it: the bank's ids of the options marked. */
// "~" (marks never settled, PSTAT-1 1.3) marks nothing.
const markedOf = (it, answer) => (answer === "~" ? [] : it.type === "true_false" ? [...answer].map((l) => (l === "A" ? "true" : "false")) : [...answer]);

for (const [name, want] of Object.entries(ref.cases)) {
  const { info, rows, answers } = readCase(name);
  const record = { questions: info.items.map(questionOf) };
  const scores = rows.map((r, k) => {
    const qs = info.items.map((it, j) => {
      const earned = r[it.id];
      const counts = earned !== null;
      const answer = answers?.[k][it.id];
      const outcome = !counts ? "dropped" : it.adjustment === "credit_all" ? "credited" : answer === "" ? "blank" : earned === it.possible ? "right" : earned === 0 ? "wrong" : "partial";
      return { number: j + 1, questionId: it.id, outcome, earned: counts ? earned : 0, possible: counts ? it.possible : 0, marked: counts && answer ? markedOf(it, answer) : [] };
    });
    const earned = qs.reduce((s, q) => s + q.earned, 0);
    const possible = qs.reduce((s, q) => s + q.possible, 0);
    return { number: r.sheet, version: r.version, earned, possible, settled: qs.filter((q) => q.possible > 0).length, unsettled: 0, complete: true, problems: [], questions: qs };
  });
  const got = stats.itemStatistics(record, scores);

  tally.same(`${name} sheets`, got.sheets, want.sheets);
  tally.same(`${name} small group (under ${stats.SMALL_GROUP})`, got.unstable, want.small_group);
  tally.number(`${name} test mean`, got.mean, want.test.mean, TOLERANCE);
  tally.number(`${name} test SD (n)`, got.sd, want.test.sd_population, TOLERANCE);
  const rel = want.reliability_every_sheet;
  if (rel.value === null) tally.same(`${name} reliability`, got.reliability, undefined);
  else {
    tally.same(`${name} reliability kind`, got.reliability?.kind, rel.kind);
    tally.same(`${name} reliability items`, got.reliability?.items, rel.items);
    tally.number(`${name} ${rel.kind}`, got.reliability?.value, rel.value, TOLERANCE);
  }
  // A question that counts on no sheet has no figures (PSTAT-1 section 7.1 leaves it out of the file).
  tally.same(`${name} questions with figures`, got.items.map((x) => x.questionId), Object.keys(want.item));
  for (const [q, w] of Object.entries(want.item)) {
    const item = got.items.find((x) => x.questionId === q);
    if (!item) continue;
    tally.same(`${name} ${q} sheets`, item.sheets, w.sheets);
    tally.number(`${name} ${q} difficulty`, item.difficulty, w.difficulty, TOLERANCE);
    tally.number(`${name} ${q} point-biserial (item in total)`, item.discrimination.pointBiserial, w.point_biserial_item_included, TOLERANCE);
    tally.number(`${name} ${q} upper-lower 27%`, item.discrimination.upperLower, w.upper_lower_27, TOLERANCE);
    if (want.groups_27.split) tally.same(`${name} ${q} group size`, item.discrimination.groupSize, want.groups_27.size);
    // Each group's own share, once OpenAssess reports it (set 1.1.0 holds the figures).
    if ("upper" in item.discrimination || "lower" in item.discrimination) {
      groupFigures = true;
      tally.number(`${name} ${q} upper 27%`, item.discrimination.upper, w.upper_27, TOLERANCE);
      tally.number(`${name} ${q} lower 27%`, item.discrimination.lower, w.lower_27, TOLERANCE);
    }
    // The letters marked: each option's marks (a sheet with two letters marks two options) and the blanks.
    const a = want.answers?.[q];
    if (a) {
      tally.same(`${name} ${q} blanks`, item.blanks, a.blank);
      const it = info.items.find((x) => x.id === q);
      for (const [letter, n] of Object.entries(a.by_letter)) {
        const id = it.type === "true_false" ? (letter === "A" ? "true" : "false") : letter;
        tally.same(`${name} ${q} option ${letter} marked`, item.options.find((o) => o.answer === id)?.chosen, n);
      }
    }
  }
}
if (!groupFigures) tally.skip("each 27 percent group's own share (upper_27, lower_27): OpenAssess works them out for the upper-lower difference but does not report them yet");
tally.skip("the scale analyses (alpha listwise, Feldt's interval, standardized alpha, corrected item-total r, alpha if deleted, KR-21, SEM, each answer by score group): PrismStatistics's figures, which OpenAssess does not compute");
console.log(tally.report());
process.exit(tally.failed.length === 0 ? 0 : 1);
