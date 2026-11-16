// Runs PrismStatistics on every case's .pstat file and compares its figures with reference.json.
//
//   node check/check_prismstatistics.mjs --prism <prism.exe or prismstatistics-app.exe> [--work <empty folder>]
//
// For each case it writes a script that opens the case's .pstat file and runs Reliability on the questions and
// Descriptives on the total, runs it with the batch runner (`run <script> --project <out.pstat>`), and reads the
// figures from the saved project. It needs no change to PrismStatistics: a script and a saved project are its own
// documented ways in and out. Exit code 0 when every figure matches.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { readPstat } from "../../../packages/pstat/src/index.js";
import { readEntry } from "../../../packages/pstat/src/zip.js";
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
  const project = readPstat(readFileSync(join(SET, "cases", name, `${name}.pstat`)));
  const items = project.dataset.variables.map((v) => v.name).filter((n) => /^q\d+$/.test(n));
  const script = join(work, `${name}.pscript`);
  const out = join(work, `${name}.out.pstat`);
  writeFileSync(
    script,
    [
      `# C3 item-analysis test set: ${name}`,
      `open_project ${JSON.stringify({ path: join(SET, "cases", name, `${name}.pstat`) })}`,
      `reliability ${JSON.stringify({ items })}`,
      `descriptives ${JSON.stringify({ variables: ["total"], statistics: ["mean", "sd"] })}`,
      "",
    ].join("\n"),
  );
  try {
    execFileSync(args.prism, ["run", script, "--project", out], { stdio: ["ignore", "ignore", "pipe"] });
  } catch (e) {
    tally.failed.push(`${name}: the batch run failed: ${String(e.stderr ?? e.message).trim()}`);
    continue;
  }
  const saved = JSON.parse(new TextDecoder().decode(readPstatEntry(out)));
  const bundles = saved.results.map((r) => r.bundle);
  const rel = bundles.find((b) => b.method_id === "scale.reliability");
  const desc = bundles.find((b) => b.method_id !== "scale.reliability");
  if (!rel) {
    tally.failed.push(`${name}: no reliability result`);
    continue;
  }
  const lw = want.reliability_listwise;
  tally.number(`${name} reliability n`, field(rel, "n"), lw.n, 0);
  tally.number(`${name} alpha`, field(rel, "alpha"), lw.alpha, TOLERANCE);
  tally.number(`${name} Feldt 95% lower`, field(rel, "alpha.ci.lower"), lw.feldt_95_lower, TOLERANCE_CI);
  tally.number(`${name} Feldt 95% upper`, field(rel, "alpha.ci.upper"), lw.feldt_95_upper, TOLERANCE_CI);
  tally.number(`${name} standardized alpha`, field(rel, "alpha.std"), lw.standardized_alpha, TOLERANCE);
  tally.number(`${name} mean inter-item r`, field(rel, "r.mean"), lw.mean_inter_item_r, TOLERANCE);
  items.forEach((q, i) => {
    const w = lw.item[q];
    tally.number(`${name} ${q} corrected item-total r`, field(rel, `item.${i + 1}.r_it`), w.corrected_item_total_r, TOLERANCE);
    tally.number(`${name} ${q} alpha if deleted`, field(rel, `item.${i + 1}.alpha_deleted`), w.alpha_if_deleted, TOLERANCE);
  });
  if (desc) {
    tally.number(`${name} total mean`, field(desc, "total.mean"), want.test.mean, TOLERANCE);
    tally.number(`${name} total SD (n - 1)`, field(desc, "total.sd"), want.test.sd_sample, TOLERANCE);
  } else tally.failed.push(`${name}: no descriptives result`);
}
tally.skip("item means and SDs: PrismStatistics shows them in the Item statistics table (to 3 places) but keeps no unrounded field for them");
tally.skip("difficulty, the point-biserial with the item in the total, the 27 percent groups and KR-20 as the class figure: OpenAssess's figures, which PrismStatistics does not compute");
console.log(tally.report());
console.log(`(scripts and saved projects in ${work})`);
process.exit(tally.failed.length === 0 ? 0 : 1);

/** project.json of a project PrismStatistics saved, as raw bytes: its results are its own, so they are not checked. */
function readPstatEntry(path) {
  const entry = readEntry(readFileSync(path), "project.json");
  if (!entry) throw new Error("the saved project has no project.json");
  return entry;
}
