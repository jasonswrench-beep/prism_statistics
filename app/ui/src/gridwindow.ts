// Data View draws only the cells in view (and a margin around them), so a dataset of any width or length stays quick
// to scroll and to edit: a page holding every cell of 500 cases by 500 variables took a quarter of a second to lay out
// on each scroll step. These are the sums that decide which rows and columns are in view.

import type { CellValue, VariableInfo } from './types';

/** Rows and columns drawn beyond the edge of the view, so a quick scroll doesn't show blank cells. */
export const OVERSCAN_ROWS = 12;
export const OVERSCAN_COLS = 4;

const PAD = 26; // the cell's padding and border, with room to spare
const MIN = 64;
const MAX = 260;

/** A column's width in pixels, from its name, its label and the longest of the values given (a sample of the cases). */
export function columnWidth(v: Pick<VariableInfo, 'name' | 'label'>, values: Iterable<CellValue>): number {
  let chars = Math.max(v.name.length * 1.08, (v.label ?? '').length * 0.85);
  for (const c of values) {
    const text = c === null ? '' : typeof c === 'object' ? String(c.missing) : String(c);
    if (text.length > chars) chars = text.length;
  }
  return Math.round(Math.min(MAX, Math.max(MIN, chars * 8 + PAD)));
}

/** The left edge of each column, plus the total width at the end (length n + 1). */
export function offsets(widths: number[]): number[] {
  const out = [0];
  for (const w of widths) out.push(out[out.length - 1] + w);
  return out;
}

/** The first column at or after pixel x (in the scrolled area past the row numbers). */
function columnAt(edges: number[], x: number): number {
  let lo = 0;
  let hi = edges.length - 1;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (edges[mid + 1] <= x) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/** The columns to draw: [first, end), for a view `width` pixels wide scrolled `left` pixels past the row numbers. */
export function visibleColumns(edges: number[], left: number, width: number, overscan = OVERSCAN_COLS): [number, number] {
  const n = edges.length - 1;
  if (n <= 0) return [0, 0];
  const first = Math.max(0, columnAt(edges, Math.max(0, left)) - overscan);
  const end = Math.min(n, columnAt(edges, Math.max(0, left + width)) + 1 + overscan);
  return [first, end];
}

/** The rows to draw: [first, end) of `count` rows each `height` pixels high, for a view scrolled `top` pixels. */
export function visibleRows(count: number, height: number, top: number, viewHeight: number, overscan = OVERSCAN_ROWS): [number, number] {
  if (count <= 0 || height <= 0) return [0, Math.max(0, count)];
  const first = Math.min(count, Math.max(0, Math.floor(Math.max(0, top) / height) - overscan));
  const end = Math.min(count, Math.ceil((Math.max(0, top) + viewHeight) / height) + overscan);
  return [first, Math.max(first, end)];
}
