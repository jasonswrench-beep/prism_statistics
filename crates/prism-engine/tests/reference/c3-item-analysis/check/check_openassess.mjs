// Runs OpenAssess's item statistics (itemStatistics in packages/assess/src/sheets/stats.ts) on every case and compares
// its figures with reference.json.
//
//   node check/check_openassess.mjs --assess <OpenAssess clone>
//
// Each case's scores.csv becomes the scored sheets OpenAssess's statistics read: one complete sheet per row, each
// question scored with the points in its cell, and a question with an empty cell not counting on that sheet. Needs
// Node 23.6 or later, which runs OpenAssess's TypeScript source directly. Exit code 0 when every figure matches.

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

for (const [name, want] of Object.entries(ref.cases)) {
  const { info, rows } = readCase(name);
  const questions = info.items.map((it) =>
    it.partial_credit
      ? { id: it.id, type: "multipleResponse", points: it.possible, body: { options: [], partialCredit: true } }
      : { id: it.id, type: "multipleChoice", points: it.possible, body: { options: [] } },
  );
  const record = { questions };
  const scores = rows.map((r) => {
    const qs = info.items.map((it, j) => {
      const earned = r[it.id];
      const counts = earned !== null;
      return { number: j + 1, questionId: it.id, outcome: counts ? (earned === it.possible ? "right" : earned === 0 ? "wrong" : "partial") : "notCounted", earned: counts ? earned : 0, possible: counts ? it.possible : 0, marked: [] };
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
  for (const [q, w] of Object.entries(want.item)) {
    const item = got.items.find((x) => x.questionId === q);
    if (!item) {
      tally.failed.push(`${name} ${q}: no figures`);
      continue;
    }
    tally.same(`${name} ${q} sheets`, item.sheets, w.sheets);
    tally.number(`${name} ${q} difficulty`, item.difficulty, w.difficulty, TOLERANCE);
    tally.number(`${name} ${q} point-biserial (item in total)`, item.discrimination.pointBiserial, w.point_biserial_item_included, TOLERANCE);
    tally.number(`${name} ${q} upper-lower 27%`, item.discrimination.upperLower, w.upper_lower_27, TOLERANCE);
    if (want.groups_27.split) tally.same(`${name} ${q} group size`, item.discrimination.groupSize, want.groups_27.size);
  }
}
tally.skip("option counts and blanks: counting, not computing; OpenAssess's own tests cover them, and the set holds scores, not marks");
tally.skip("alpha listwise, Feldt's interval, standardized alpha, corrected item-total r and alpha if deleted: PrismStatistics's figures, which OpenAssess does not compute");
console.log(tally.report());
process.exit(tally.failed.length === 0 ? 0 : 1);
