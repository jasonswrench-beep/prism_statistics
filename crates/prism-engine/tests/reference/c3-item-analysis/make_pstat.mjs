// Writes cases/<name>/<name>.pstat for every case, in the OpenAssess item results 1 profile (PSTAT-1 section 7.1):
// the file OpenAssess would send to PrismStatistics for that class. Run from this folder: node make_pstat.mjs
// The same case files always make the same bytes, and check_set.mjs fails if a .pstat file no longer matches.

import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { PROFILE_ITEM_RESULTS, writePstat } from "../../packages/pstat/src/index.js";

const HERE = fileURLToPath(new URL(".", import.meta.url));
export const APP_VERSION = "OpenAssess item-analysis test set 1.0.0";

/** Reads a case's scores.csv and case.json. */
export function readCase(name) {
  const dir = join(HERE, "cases", name);
  const info = JSON.parse(readFileSync(join(dir, "case.json"), "utf8"));
  const [header, ...lines] = readFileSync(join(dir, "scores.csv"), "utf8").trim().split("\n");
  const cols = header.split(",");
  const rows = lines.map((line) => {
    const cells = line.split(",");
    return Object.fromEntries(cols.map((c, i) => [c, cells[i] === "" || cells[i] === undefined ? null : Number(cells[i])]));
  });
  return { info, rows };
}

/** The project.json of a case, as OpenAssess would write it. */
export function projectFor(name) {
  const { info, rows } = readCase(name);
  const items = info.items;
  const width = Math.max(2, String(items.length).length);
  const versions = [...new Set(rows.map((r) => r.version))].sort((a, b) => a - b);
  const variables = [
    { id: 1, name: "sheet", label: "Sheet number", measure: "nominal", value_labels: [], missing_values: [] },
    { id: 2, name: "version", label: "Version", measure: "nominal", value_labels: versions.map((v) => ({ value: v, label: String.fromCharCode(64 + v) })), missing_values: [] },
    ...items.map((it, j) => ({ id: 3 + j, name: `q${String(j + 1).padStart(width, "0")}`, label: `Question ${j + 1}`, measure: "scale", value_labels: [], missing_values: [] })),
    { id: 3 + items.length, name: "total", label: "Total score", measure: "scale", value_labels: [], missing_values: [] },
  ];
  const columns = [
    { type: "numeric", values: rows.map((r) => r.sheet) },
    { type: "numeric", values: rows.map((r) => r.version) },
    ...items.map((it) => ({ type: "numeric", values: rows.map((r) => r[it.id]) })),
    { type: "numeric", values: rows.map((r) => items.reduce((s, it) => s + (r[it.id] ?? 0), 0)) },
  ];
  const notes = items.filter((it) => it.not_counted_on_versions).map((it) => `${it.id} does not count on version ${it.not_counted_on_versions.map((v) => String.fromCharCode(64 + v)).join(", ")}, so its cells there are empty.`);
  return {
    format: "PrismStatistics project",
    version: 1,
    app_version: APP_VERSION,
    profile: PROFILE_ITEM_RESULTS,
    dataset: { name, variables, columns, next_id: variables.length + 1 },
    source: { file_name: `${name} results`, report: { rows: rows.length, variables: variables.length, renamed: [], short_rows: 0, notes } },
    operations: [],
    results: [],
  };
}

export const caseNames = () => readdirSync(join(HERE, "cases"), { withFileTypes: true }).filter((d) => d.isDirectory()).map((d) => d.name).sort();

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  for (const name of caseNames()) {
    writeFileSync(join(HERE, "cases", name, `${name}.pstat`), writePstat(projectFor(name)));
    console.log(`wrote cases/${name}/${name}.pstat`);
  }
}
