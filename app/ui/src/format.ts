// Display formatting shared by the screens. It mirrors crates/prism-engine/src/apa.rs, so a number reads the same in
// a table here as in the exported report.

import type { ResultCell } from './types';

const MINUS = '−';

export function num(x: number, decimals: number): string {
  let s = x.toFixed(decimals);
  if (/^-0(\.0+)?$/.test(s)) s = s.slice(1);
  return s.replace('-', MINUS);
}

export function pCell(p: number): string {
  if (p < 0.001) return '< .001';
  return num(p, 3).replace(/^0\./, '.');
}

export function cellText(c: ResultCell): string {
  switch (c.kind) {
    case 'text':
      return c.text;
    case 'number':
      return Number.isFinite(c.value) ? num(c.value, c.decimals) : '—';
    case 'count':
      return String(c.value);
    case 'p':
      return pCell(c.value);
    case 'empty':
      return '';
  }
}


/** Splits text with *italic* and <sub>subscript</sub> markers into runs. */
export function italicRuns(text: string): { text: string; italic: boolean; sub?: boolean }[] {
  return text
    .split(/(\*[^*]+\*|<sub>[^<]*<\/sub>)/)
    .filter(Boolean)
    .map((part) => {
      if (part.startsWith('<sub>')) return { text: part.slice(5, -6), italic: false, sub: true };
      if (part.startsWith('*') && part.endsWith('*') && part.length > 2) return { text: part.slice(1, -1), italic: true };
      return { text: part, italic: false };
    });
}

/** The paragraph as plain text, for the clipboard. */
export function plainText(text: string): string {
  return text.replace(/\*/g, '').replace(/<\/?sub>/g, '');
}

/** Parses value labels written one per line as "1 = Urban". Returns an error message for the first bad line. */
export function parseValueLabels(text: string): { labels: { value: number; label: string }[]; error?: string } {
  const labels: { value: number; label: string }[] = [];
  const lines = text.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trim();
    if (!line) continue;
    const m = /^(-?\d+(?:\.\d+)?)\s*[=:]\s*(.+)$/.exec(line);
    if (!m) return { labels, error: `Line ${i + 1} should look like "1 = Urban".` };
    const value = Number(m[1]);
    if (labels.some((l) => l.value === value)) return { labels, error: `Code ${m[1]} has two labels.` };
    labels.push({ value, label: m[2].trim() });
  }
  return { labels };
}

/** Parses missing-value codes written as "-99, 9". */
export function parseCodes(text: string): { codes: number[]; error?: string } {
  // A range ("90 to 99", "90-99", "90..99") can't be typed here; say so instead of "is not a number".
  if (/\d\s*(?:to|thru|through|\.\.|–)\s*-?\d|\d-\d/i.test(text)) return { codes: [], error: 'list each code, separated by commas (for example -9, 98, 99). A range of codes can only come from the data file.' };
  const parts = text.split(/[,;\s]+/).filter(Boolean);
  const codes: number[] = [];
  for (const p of parts) {
    const v = Number(p);
    if (!Number.isFinite(v)) return { codes, error: `"${p}" is not a number.` };
    if (!codes.includes(v)) codes.push(v);
  }
  return { codes };
}

/** A file's name without its folders, for messages: "study.pstat" from a full path. Messages and the recovery window show
 * this, so the account name in a user folder's path is not put on screen (or in screenshots). */
export function fileName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}
