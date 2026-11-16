// Runs PrismStatistics on every case's .pstat file and compares its figures with reference.json.
//
//   node check/check_prismstatistics.mjs --prism <prism.exe or prismstatistics-app.exe> [--work <empty folder>]
//                                        [--reference <file>]
//
// For each case it writes a script that opens the case's .pstat file and runs Reliability on the questions,
// Descriptives on the total and Item Analysis on the points; for a case with letters (profile 2) also Item Analysis on
// the answers, scored against the key in the file's assessment.json. It runs the script with the batch runner
// (`run <script> --project <out.pstat>`) and reads the figures from the saved project. It needs no change to
// PrismStatistics: a script and a saved project are its own documented ways in and out. Exit code 0 when every figure
// matches.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { readPart, readPstat } from "../../../packages/pstat/src/index.js";
import { readEntry } from "../../../packages/pstat/src/zip.js";
import { readCase } from "../make_pstat.mjs";
import { flags, reference, SET, Tally } from "./common.mjs";

const TOLERANCE = 1e-9; // unrounded figures, as PrismStatistics's own reference tests use
const TOLERANCE_CI = 1e-8; // Feldt's limits go through the F quantile

const args = flags(process.argv.slice(2));
if (!args.prism) {
  console.error("usage: node check/check_prismstatistics.mjs --prism <prism.exe or prismstatistics-app.exe> [--work <folder>]");
  process.exit(2);
}
const work = args.work ? resolve(args.work) : mkdtempSync(join(tmpdir(), "c3-prism-"));
mkdirSync(work, { recursive: true });

const ref = reference(args.reference);
const tally = new Tally("PrismStatistics");
const field = (bundle, key) => {
  const v = bundle.fields?.[key];
  return typeof v === "number" ? v : v === null ? null : undefined;
};

for (const [name, want] of Object.entries(ref.cases)) {
  const file = join(SET, "cases", name, `${name}.pstat`);
  const bytes = readFileSync(file);
  const project = readPstat(bytes);
  const part = readPart(bytes, project);
  const { info } = readCase(name);
  const items = project.dataset.variables.map((v) => v.name).filter((n) => /^q\d+$/.test(n));
  const points = items.map((q) => info.items.find((it) => it.id === q).possible);
  const lines = [
    `# C3 item-analysis test set: ${name}`,
    `open_project ${JSON.stringify({ path: file })}`,
    `reliability ${JSON.stringify({ items })}`,
    `descriptives ${JSON.stringify({ variables: ["total"], statistics: ["mean", "sd"] })}`,
    `item_analysis ${JSON.stringify({ items, key: points.map((p) => ({ correct: [], points: p })), scored: true, group_fraction: 0.27 })}`,
  ];
  // Profile 2: the answers, scored against the file's own key (PSTAT-1 section 7.2).
  const answered = part ? part.questions.filter((q) => q.answer_variable !== null) : [];
  if (answered.length > 0) {
    const nameOf = new Map(project.dataset.variables.map((v) => [v.id, v.name]));
    const key = answered.map((q) => ({ correct: q.key[0].correct, points: q.key[0].points, adjustment: q.key[0].adjustment }));
    lines.push(`item_analysis ${JSON.stringify({ items: answered.map((q) => nameOf.get(q.answer_variable)), key, group_fraction: 0.27 })}`);
  }
  const script = join(work, `${name}.pscript`);
  const out = join(work, `${name}.out.pstat`);
  writeFileSync(script, [...lines, ""].join("\n"));
  try {
    execFileSync(args.prism, ["run", script, "--project", out], { stdio: ["ignore", "ignore", "pipe"] });
  } catch (e) {
    tally.failed.push(`${name}: the batch run failed: ${String(e.stderr ?? e.message).trim()}`);
    continue;
  }
  const saved = JSON.parse(new TextDecoder().decode(readPstatEntry(out)));
  const bundles = saved.results.map((r) => r.bundle);
  const rel = bundles.find((b) => b.method_id === "scale.reliability");
  const desc = bundles.find((b) => b.method_id === "desc.descriptives");
  const [scored, letters] = bundles.filter((b) => b.method_id === "scale.item_analysis");

  // Reliability.
  const lw = want.reliability_listwise;
  if (!rel) tally.failed.push(`${name}: no reliability result`);
  else {
    tally.number(`${name} reliability n`, field(rel, "n"), lw.n, 0);
    tally.number(`${name} alpha`, field(rel, "alpha"), lw.alpha, TOLERANCE);
    tally.number(`${name} Feldt 95% lower`, field(rel, "alpha.ci.lower"), lw.feldt_95_lower, TOLERANCE_CI);
    tally.number(`${name} Feldt 95% upper`, field(rel, "alpha.ci.upper"), lw.feldt_95_upper, TOLERANCE_CI);
    tally.number(`${name} standardized alpha`, field(rel, "alpha.std"), lw.standardized_alpha, TOLERANCE);
    tally.number(`${name} mean inter-item r`, field(rel, "r.mean"), lw.mean_inter_item_r, TOLERANCE);
    items.forEach((q, i) => {
      const w = lw.item[q];
      tally.number(`${name} ${q} item mean`, field(rel, `item.${i + 1}.mean`), w.mean, TOLERANCE);
      tally.number(`${name} ${q} item SD (n - 1)`, field(rel, `item.${i + 1}.sd`), w.sd, TOLERANCE);
      tally.number(`${name} ${q} corrected item-total r`, field(rel, `item.${i + 1}.r_it`), w.corrected_item_total_r, TOLERANCE);
      tally.number(`${name} ${q} alpha if deleted`, field(rel, `item.${i + 1}.alpha_deleted`), w.alpha_if_deleted, TOLERANCE);
    });
  }

  // Descriptives.
  if (desc) {
    tally.number(`${name} total mean`, field(desc, "total.mean"), want.test.mean, TOLERANCE);
    tally.number(`${name} total SD (n - 1)`, field(desc, "total.sd"), want.test.sd_sample, TOLERANCE);
  } else tally.failed.push(`${name}: no descriptives result`);

  // Item Analysis, on the points and (profile 2) on the letters: the same figures, since the key made the points.
  const itemAnalysis = (bundle, how) => {
    const ia = want.item_analysis_listwise;
    if (!bundle) return void tally.failed.push(`${name}: no item analysis ${how}`);
    const at = `${name} item analysis ${how}`;
    for (const [k, f, tol] of [["n", "n", 0], ["k", "k", 0], ["possible", "possible", TOLERANCE], ["mean", "mean", TOLERANCE], ["sd", "sd", TOLERANCE], ["sd_n", "sd.n", TOLERANCE], ["mean_p", "mean.p", TOLERANCE], ["reliability", "reliability", TOLERANCE], ["kr21", "kr21", TOLERANCE], ["sem", "sem", TOLERANCE]])
      tally.number(`${at} ${k}`, field(bundle, f), ia[k], tol);
    // KR-20 for right-or-wrong questions, otherwise alpha: the field says which.
    tally.number(`${at} ${ia.right_or_wrong ? "KR-20" : "alpha"}`, field(bundle, ia.right_or_wrong ? "kr20" : "alpha"), ia.reliability, TOLERANCE);
    tally.same(`${at} names only ${ia.right_or_wrong ? "KR-20" : "alpha"}`, field(bundle, ia.right_or_wrong ? "alpha" : "kr20"), undefined);
    if (ia.split) tally.number(`${at} group size`, field(bundle, "group.size"), ia.group_size, 0);
    Object.keys(ia.item).forEach((q, i) => {
      const w = ia.item[q];
      const f = (k) => field(bundle, `item.${i + 1}.${k}`);
      tally.number(`${at} ${q} p`, f("p"), w.p, TOLERANCE);
      tally.number(`${at} ${q} mean`, f("mean"), w.mean, TOLERANCE);
      tally.number(`${at} ${q} point-biserial (item in total)`, f("rpb"), w.point_biserial_item_included, TOLERANCE);
      tally.number(`${at} ${q} corrected item-total r`, f("rpb_corrected"), w.corrected_item_total_r, TOLERANCE);
      tally.number(`${at} ${q} alpha if deleted`, f("alpha_deleted"), w.alpha_if_deleted, TOLERANCE);
      tally.number(`${at} ${q} upper group`, f("upper"), w.upper, TOLERANCE);
      tally.number(`${at} ${q} lower group`, f("lower"), w.lower, TOLERANCE);
      tally.number(`${at} ${q} D`, f("D"), w.d, TOLERANCE);
      const a = want.answers?.[q];
      if (how === "on the letters" && a) {
        tally.number(`${at} ${q} blanks`, f("blank"), a.blank, 0);
        const prefix = `item.${i + 1}.opt.`;
        for (const [answer, c] of Object.entries(a.by_answer))
          for (const k of ["n", "upper", "middle", "lower"]) tally.number(`${at} ${q} answer ${answer} ${k}`, field(bundle, `${prefix}${answer}.${k}`) ?? 0, c[k], 0);
        // Every answer PrismStatistics counts is one the set counts.
        for (const key of Object.keys(bundle.fields).filter((k) => k.startsWith(prefix) && k.endsWith(".n"))) {
          const answer = key.slice(prefix.length, -2);
          if (!(answer in a.by_answer) && field(bundle, key) !== 0) tally.failed.push(`${at} ${q}: PrismStatistics counts ${field(bundle, key)} answers ${answer}, which the set does not have`);
        }
      }
    });
  };
  itemAnalysis(scored, "on the points");
  if (answered.length > 0) itemAnalysis(letters, "on the letters");
}
tally.skip("difficulty, the point-biserial and the 27 percent groups over the sheets each question counts on, KR-20 as the class figure, and the marks for each letter: OpenAssess's figures (they equal PrismStatistics's listwise figures wherever every question counts on every sheet)");
console.log(tally.report());
console.log(`(scripts and saved projects in ${work})`);
process.exit(tally.failed.length === 0 ? 0 : 1);

/** project.json of a project PrismStatistics saved, as raw bytes: its results are its own, so they are not checked. */
function readPstatEntry(path) {
  const entry = readEntry(readFileSync(path), "project.json");
  if (!entry) throw new Error("the saved project has no project.json");
  return entry;
}
