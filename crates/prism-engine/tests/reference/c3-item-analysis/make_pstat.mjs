// Writes cases/<name>/<name>.pstat for every case: the file OpenAssess would send to PrismStatistics for that class.
// A case with letters (answers.csv) is written in the OpenAssess item results 2 profile with its assessment.json
// (PSTAT-1 section 7.2); every other case in profile 1 (section 7.1). Run from this folder: node make_pstat.mjs
// The same case files always make the same bytes, and check_set.mjs fails if a .pstat file no longer matches.

import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { PROFILE_ITEM_RESULTS, PROFILE_ITEM_RESULTS_2, writePstat } from "../../packages/pstat/src/index.js";

const HERE = fileURLToPath(new URL(".", import.meta.url));
export const SET_VERSION = "2.0.0";
export const APP_VERSION = `OpenAssess item-analysis test set ${SET_VERSION}`;

const csv = (path, cell) => {
  const [header, ...lines] = readFileSync(path, "utf8").trim().split("\n");
  const cols = header.split(",");
  return lines.map((line) => {
    const cells = line.split(",");
    return Object.fromEntries(cols.map((c, i) => [c, c === "sheet" || c === "version" ? Number(cells[i]) : cell(cells[i] ?? "")]));
  });
};

/** Reads a case's scores.csv, case.json and, when it has letters, answers.csv. */
export function readCase(name) {
  const dir = join(HERE, "cases", name);
  const info = JSON.parse(readFileSync(join(dir, "case.json"), "utf8"));
  const rows = csv(join(dir, "scores.csv"), (s) => (s === "" ? null : Number(s)));
  const answers = existsSync(join(dir, "answers.csv")) ? csv(join(dir, "answers.csv"), (s) => s) : null;
  return { info, rows, answers };
}

const letter = (v) => String.fromCharCode(64 + v);
const variable = (id, name, label, measure, value_labels = []) => ({ id, name, label, measure, value_labels, missing_values: [] });

/** The project.json of a case, as OpenAssess would write it. */
export function projectFor(name) {
  const { info, rows, answers } = readCase(name);
  const width = Math.max(2, String(info.items.length).length);
  const number = (it) => info.items.indexOf(it) + 1;
  const tag = (it) => String(number(it)).padStart(width, "0");
  // A question that counts on no sheet is left out, and the names keep version A's numbers (PSTAT-1 section 7.1).
  const items = info.items.filter((it) => rows.some((r) => r[it.id] !== null));
  const versions = [...new Set(rows.map((r) => r.version))].sort((a, b) => a - b);
  // A question of type other has no letters, so no answer variable (PSTAT-1 1.3).
  const lettered = items.filter((it) => it.type !== "other");
  let id = 1;
  const variables = [
    variable(id++, "sheet", "Sheet number", "nominal"),
    variable(id++, "version", "Version", "nominal", versions.map((v) => ({ value: v, label: letter(v) }))),
    ...items.map((it) => variable(id++, `q${tag(it)}`, `Question ${number(it)}`, "scale")),
    variable(id++, "total", "Total score", "scale"),
    ...(answers ? lettered.map((it) => variable(id++, `r${tag(it)}`, `Question ${number(it)} answer`, "nominal")) : []),
  ];
  const columns = [
    { type: "numeric", values: rows.map((r) => r.sheet) },
    { type: "numeric", values: rows.map((r) => r.version) },
    ...items.map((it) => ({ type: "numeric", values: rows.map((r) => r[it.id]) })),
    { type: "numeric", values: rows.map((r) => items.reduce((s, it) => s + (r[it.id] ?? 0), 0)) },
    // "~" in answers.csv is marks never settled: an empty answer cell beside the points (PSTAT-1 1.3).
    ...(answers ? lettered.map((it) => ({ type: "text", values: rows.map((r, k) => (r[it.id] === null || answers[k][it.id] === "~" ? null : answers[k][it.id])) })) : []),
  ];
  const notes = [
    ...info.items.filter((it) => !items.includes(it)).map((it) => `Question ${number(it)} counts on none of the sheets written (it was dropped), so it is left out.`),
    ...items.filter((it) => it.not_counted_on_versions).map((it) => `q${tag(it)} does not count on version ${it.not_counted_on_versions.map(letter).join(", ")}, so its cells there are empty.`),
  ];
  return {
    format: "PrismStatistics project",
    version: 1,
    app_version: APP_VERSION,
    profile: answers ? PROFILE_ITEM_RESULTS_2 : PROFILE_ITEM_RESULTS,
    dataset: { name, variables, columns, next_id: variables.length + 1 },
    source: { file_name: `${name} results`, report: { rows: rows.length, variables: variables.length, renamed: [], short_rows: 0, notes } },
    operations: [],
    results: [],
  };
}

// Ids in OpenAssess's form (a letter, _ and 26 Crockford characters), made from the case so they never change.
const ulid = (prefix, n) => `${prefix}_01KA9C3T00${String(n).padStart(16, "0")}`;

/** The assessment.json of a case with letters (PSTAT-1 section 7.2), or undefined for a case without. */
export function assessmentFor(name) {
  const { info, answers } = readCase(name);
  if (!answers) return undefined;
  const project = projectFor(name);
  const byName = new Map(project.dataset.variables.map((v) => [v.name, v.id]));
  const width = Math.max(2, String(info.items.length).length);
  return {
    part: "OpenAssess assessment",
    version: 1,
    test: { id: ulid("t", 0), name, given: "2026-11-10", source: "scanned sheets", versions: Array.from({ length: info.versions }, (_, k) => letter(k + 1)), questions: info.items.length, sheets_left_out: 0 },
    questions: info.items.map((it, j) => {
      const tag = String(j + 1).padStart(width, "0");
      const scored = it.adjustment === "none";
      return {
        number: j + 1,
        bank_id: ulid("q", j + 1),
        revision: 1,
        points_variable: byName.get(`q${tag}`) ?? null,
        answer_variable: byName.get(`r${tag}`) ?? null,
        type: it.type,
        options: it.options,
        option_ids: it.type === "true_false" ? ["true", "false"] : it.options.map((_, k) => `o${k + 1}`),
        option_labels: it.type === "true_false" ? { A: "True", B: "False" } : null,
        key: [{ correct: scored ? it.correct : [], points: it.possible, adjustment: it.adjustment, partial_credit: it.partial_credit }],
        details: { objective: null, chapter: null, section: null, page: null, bloom: null, difficulty: null, tags: [] },
      };
    }),
  };
}

/** The bytes of a case's .pstat file. */
export const fileFor = (name) => writePstat(projectFor(name), assessmentFor(name));

export const caseNames = () => readdirSync(join(HERE, "cases"), { withFileTypes: true }).filter((d) => d.isDirectory()).map((d) => d.name).sort();

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  for (const name of caseNames()) {
    writeFileSync(join(HERE, "cases", name, `${name}.pstat`), fileFor(name));
    console.log(`wrote cases/${name}/${name}.pstat`);
  }
}
