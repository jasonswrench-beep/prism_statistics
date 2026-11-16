// What both checkers share: reading reference.json and comparing a figure with the tolerance its tool is held to.
// This is test scaffolding, not item-analysis code: no figure is computed here.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

export const SET = fileURLToPath(new URL("..", import.meta.url));
/** reference.json, or another copy of it (--reference), so the checkers can be shown to fail on a wrong value. */
export const reference = (path) => JSON.parse(readFileSync(path ?? join(SET, "reference.json"), "utf8"));

/**
 * Compares figures and keeps a tally. A reference of null means the figure is undefined; the tool must then give no
 * figure (undefined, null or NaN), never a number.
 */
export class Tally {
  constructor(tool) {
    this.tool = tool;
    this.passed = 0;
    this.failed = [];
    this.skipped = [];
  }

  /**
   * @param {string} where e.g. "small-ties q03 difficulty"
   * @param {number | null | undefined} got
   * @param {number | null} want
   * @param {number} tolerance absolute
   */
  number(where, got, want, tolerance) {
    const none = (v) => v === undefined || v === null || (typeof v === "number" && Number.isNaN(v));
    if (want === null) {
      if (none(got)) this.passed++;
      else this.failed.push(`${where}: undefined in R, but ${this.tool} gives ${got}`);
      return;
    }
    if (none(got)) return void this.failed.push(`${where}: R gives ${want}, ${this.tool} gives none`);
    if (Math.abs(got - want) <= tolerance) this.passed++;
    else this.failed.push(`${where}: R gives ${want}, ${this.tool} gives ${got} (off by ${Math.abs(got - want).toExponential(2)}, allowed ${tolerance})`);
  }

  same(where, got, want) {
    if (JSON.stringify(got) === JSON.stringify(want)) this.passed++;
    else this.failed.push(`${where}: expected ${JSON.stringify(want)}, ${this.tool} gives ${JSON.stringify(got)}`);
  }

  skip(what) {
    this.skipped.push(what);
  }

  report() {
    const lines = [`${this.tool}: ${this.passed} figures match, ${this.failed.length} do not.`];
    for (const f of this.failed) lines.push(`  DIFFERS  ${f}`);
    for (const s of this.skipped) lines.push(`  not checked: ${s}`);
    return lines.join("\n");
  }
}

/** Reads "--name value" flags. */
export function flags(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i++) if (argv[i].startsWith("--")) out[argv[i].slice(2)] = argv[i + 1] && !argv[i + 1].startsWith("--") ? argv[++i] : true;
  return out;
}
