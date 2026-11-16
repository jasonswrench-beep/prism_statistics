// Checks the item-analysis test set itself.
//
//   node check/check_set.mjs           every .pstat file is what make_pstat.mjs makes and follows PSTAT-1, and every
//                                      file matches SHA256SUMS
//   node check/check_set.mjs --write   writes SHA256SUMS after a deliberate change (README.md, "Changing the set")
//
// Exit code 0 when the set is whole.

import { createHash } from "node:crypto";
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";

import { checkPstat, writePstat } from "../../../packages/pstat/src/index.js";
import { caseNames, projectFor } from "../make_pstat.mjs";
import { flags, SET } from "./common.mjs";

const args = flags(process.argv.slice(2));
const problems = [];

for (const name of caseNames()) {
  const file = join(SET, "cases", name, `${name}.pstat`);
  const bytes = readFileSync(file);
  const found = checkPstat(bytes);
  for (const p of found) problems.push(`${name}.pstat breaks PSTAT-1 rule ${p.rule}: ${p.message} at ${p.path}`);
  if (!Buffer.from(writePstat(projectFor(name))).equals(bytes)) problems.push(`${name}.pstat is not what make_pstat.mjs makes from the case files: run node make_pstat.mjs`);
}

// Every file of the set except the sums themselves and the checkers' own scratch.
const files = [];
const walk = (dir) => {
  for (const d of readdirSync(dir, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : 1))) {
    const p = join(dir, d.name);
    if (d.isDirectory()) walk(p);
    else if (d.name !== "SHA256SUMS") files.push(p);
  }
};
walk(join(SET, "cases"));
files.push(join(SET, "reference.json"), join(SET, "make_cases.R"), join(SET, "make_reference.R"), join(SET, "make_pstat.mjs"));
const sums = files.map((f) => `${createHash("sha256").update(readFileSync(f)).digest("hex")}  ${relative(SET, f).replaceAll("\\", "/")}`).join("\n") + "\n";
const sumsFile = join(SET, "SHA256SUMS");
if (args.write) {
  writeFileSync(sumsFile, sums);
  console.log(`wrote SHA256SUMS (${files.length} files)`);
} else {
  let kept = "";
  try {
    kept = readFileSync(sumsFile, "utf8").replaceAll("\r\n", "\n");
  } catch {
    problems.push("SHA256SUMS is missing: run node check/check_set.mjs --write");
  }
  if (kept && kept !== sums) {
    const want = new Map(kept.trim().split("\n").map((l) => [l.slice(66), l.slice(0, 64)]));
    for (const line of sums.trim().split("\n")) {
      const file = line.slice(66);
      if (!want.has(file)) problems.push(`${file} is not in SHA256SUMS`);
      else if (want.get(file) !== line.slice(0, 64)) problems.push(`${file} has changed since SHA256SUMS was written`);
      want.delete(file);
    }
    for (const file of want.keys()) problems.push(`${file} is in SHA256SUMS but missing`);
  }
}

if (problems.length > 0) {
  console.log(problems.map((p) => `PROBLEM  ${p}`).join("\n"));
  process.exit(1);
}
console.log(`The set is whole: ${caseNames().length} cases, ${files.length} files.`);
