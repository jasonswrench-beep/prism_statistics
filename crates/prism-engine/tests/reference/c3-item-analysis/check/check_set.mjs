// Checks the item-analysis test set itself.
//
//   node check/check_set.mjs           every file of the set matches SHA256SUMS; inside the core repository, also
//                                      that every .pstat file is what make_pstat.mjs makes and follows PSTAT-1
//   node check/check_set.mjs --write   writes SHA256SUMS after a deliberate change (README.md, "Changing the set")
//
// In a tool's copy of the set (the folder copied whole, without the rest of the core) only the sums are checked: they
// prove the copy is byte for byte the set that passed the full check in the core. Exit code 0 when the set is whole.

import { createHash } from "node:crypto";
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

import { flags, SET } from "./common.mjs";

const args = flags(process.argv.slice(2));
const problems = [];
const cases = readdirSync(join(SET, "cases"), { withFileTypes: true }).filter((d) => d.isDirectory()).map((d) => d.name).sort();

// The PSTAT-1 checks and the rebuild need @openstage/pstat, which lives beside the set only in the core repository.
const PACKAGE = new URL("../../../packages/pstat/src/index.js", import.meta.url);
const inCore = existsSync(fileURLToPath(PACKAGE));
if (inCore) {
  const { checkPstat } = await import(PACKAGE.href);
  const { fileFor } = await import("../make_pstat.mjs");
  for (const name of cases) {
    const bytes = readFileSync(join(SET, "cases", name, `${name}.pstat`));
    for (const p of checkPstat(bytes)) problems.push(`${name}.pstat breaks PSTAT-1 rule ${p.rule}: ${p.message} at ${p.path}`);
    if (!Buffer.from(fileFor(name)).equals(bytes)) problems.push(`${name}.pstat is not what make_pstat.mjs makes from the case files: run node make_pstat.mjs`);
  }
}

// Every file of the set except the sums themselves.
const files = [];
const walk = (dir) => {
  for (const d of readdirSync(dir, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : 1))) {
    const p = join(dir, d.name);
    if (d.isDirectory()) walk(p);
    else if (relative(SET, p) !== "SHA256SUMS") files.push(p);
  }
};
walk(SET);
const sums = files.map((f) => `${createHash("sha256").update(readFileSync(f)).digest("hex")}  ${relative(SET, f).replaceAll("\\", "/")}`).join("\n") + "\n";
const sumsFile = join(SET, "SHA256SUMS");
if (args.write) {
  if (!inCore) {
    console.error("SHA256SUMS is written only in the core repository: a copy of the set is never changed in place.");
    process.exit(2);
  }
  writeFileSync(sumsFile, sums);
  console.log(`wrote SHA256SUMS (${files.length} files)`);
} else {
  let kept = "";
  try {
    kept = readFileSync(sumsFile, "utf8").replaceAll("\r\n", "\n");
  } catch {
    problems.push("SHA256SUMS is missing: run node check/check_set.mjs --write in the core repository");
  }
  if (kept && kept !== sums) {
    const want = new Map(kept.trim().split("\n").map((l) => [l.slice(66), l.slice(0, 64)]));
    for (const line of sums.trim().split("\n")) {
      const file = line.slice(66);
      if (!want.has(file)) problems.push(`${file} is not in SHA256SUMS`);
      else if (want.get(file) !== line.slice(0, 64)) problems.push(`${file} has changed since SHA256SUMS was written${inCore ? "" : " (in a copy, check that git did not change its line endings: mark the folder -text)"}`);
      want.delete(file);
    }
    for (const file of want.keys()) problems.push(`${file} is in SHA256SUMS but missing`);
  }
}

if (problems.length > 0) {
  console.log(problems.map((p) => `PROBLEM  ${p}`).join("\n"));
  process.exit(1);
}
console.log(
  inCore
    ? `The set is whole: ${cases.length} cases, ${files.length} files, every .pstat file rebuilt and checked against PSTAT-1.`
    : `The copy is whole: ${cases.length} cases, ${files.length} files match SHA256SUMS. (The PSTAT-1 checks and the rebuild run in the core repository.)`,
);
