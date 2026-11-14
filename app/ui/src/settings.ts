// Tools › Settings: preferences that belong to the person using this computer, not to a project. They are kept in the
// window's own storage, so they survive restarts; anything the storage can't give back falls back to the defaults.
//
// Output decimals are not a setting: tables follow APA rules for each statistic, so the window and every export always
// show the same numbers.

export interface Settings {
  /** Interface text size, in percent. */
  textSize: 100 | 115 | 130;
  /** Data View shows value labels in place of codes when it opens. */
  valueLabels: boolean;
  /** Data View opens browse-only (editing switched off). */
  browseOnly: boolean;
  /** The confidence level new dialogs start with, in percent. */
  level: number;
}

export const DEFAULTS: Settings = { textSize: 100, valueLabels: false, browseOnly: true, level: 95 };

const KEY = 'prismstatistics.settings';

/** Settings from stored text: unknown keys are dropped and bad values replaced by the defaults. */
export function parseSettings(text: string | null): Settings {
  let raw: Record<string, unknown> = {};
  try {
    const v = text ? JSON.parse(text) : {};
    if (v && typeof v === 'object' && !Array.isArray(v)) raw = v;
  } catch {
    // Damaged settings: start from the defaults.
  }
  const size = raw.textSize === 115 || raw.textSize === 130 ? raw.textSize : 100;
  const level = typeof raw.level === 'number' && raw.level >= 50 && raw.level < 100 ? raw.level : DEFAULTS.level;
  return {
    textSize: size,
    valueLabels: typeof raw.valueLabels === 'boolean' ? raw.valueLabels : DEFAULTS.valueLabels,
    browseOnly: typeof raw.browseOnly === 'boolean' ? raw.browseOnly : DEFAULTS.browseOnly,
    level,
  };
}

let current: Settings | null = null;
const listeners = new Set<(s: Settings) => void>();

export function getSettings(): Settings {
  if (!current) {
    let text: string | null = null;
    try {
      text = globalThis.localStorage?.getItem(KEY) ?? null;
    } catch {
      text = null;
    }
    current = parseSettings(text);
  }
  return current;
}

export function saveSettings(s: Settings) {
  current = parseSettings(JSON.stringify(s));
  try {
    globalThis.localStorage?.setItem(KEY, JSON.stringify(current));
  } catch {
    // Storage refused: the settings last until the window closes.
  }
  listeners.forEach((f) => f(current!));
}

export function onSettings(f: (s: Settings) => void): () => void {
  listeners.add(f);
  return () => listeners.delete(f);
}
