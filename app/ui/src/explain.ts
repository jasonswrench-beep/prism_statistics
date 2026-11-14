// Explain this result: a plain-language reading of one result, built from its own tables and checks with the glossary's
// definitions (glossary.ts). It finds the statistics the result reports (by column heading, or by row label in
// effect-size tables), says what this result's value of each one means, names the checks that need attention, and adds
// the cautions that apply. It never computes anything new: every number comes from the result.

import { GLOSSARY, fmt, noLead, type Term } from './glossary';
import type { ResultBundle, ResultCell } from './types';

export interface Reading {
  term: Term;
  /** Where the value came from: the row's label (a predictor, a test, a group) if it has one. */
  label: string | null;
  value: number;
  sentence: string;
  /** The row's confidence interval, when the table gives one. */
  ci?: { lower: number; upper: number; level: string };
}

export interface Explanation {
  readings: Reading[];
  checks: string[];
  /** Checks the result doesn't make for you, with what to look at instead. */
  unchecked: string[];
  cautions: string[];
}

const numberOf = (c: ResultCell | undefined): number | null => (c && (c.kind === 'number' || c.kind === 'p' || c.kind === 'count') && Number.isFinite(c.value) ? c.value : null);
const textOf = (c: ResultCell | undefined): string | null => (c && c.kind === 'text' && c.text.trim() ? c.text.trim() : null);

/** The level named in a heading such as "95% CI lower". */
const ciCols = (columns: string[]) => {
  const lo = columns.findIndex((c) => /CI lower$/.test(c));
  const hi = columns.findIndex((c) => /CI upper$/.test(c));
  return lo >= 0 && hi >= 0 ? { lo, hi, level: columns[lo].replace(/\s*CI lower$/, '') || '95%' } : null;
};

export function explain(bundle: ResultBundle, options: { maxReadings?: number } = {}): Explanation {
  const max = options.maxReadings ?? 8;
  const readings: Reading[] = [];
  const seen = new Set<string>();
  const add = (term: Term, label: string | null, value: number, ci?: Reading['ci']) => {
    if (seen.has(term.id) || !term.read) return;
    seen.add(term.id);
    readings.push({ term, label, value, sentence: term.read(value), ci });
  };
  for (const t of bundle.tables) {
    const ci = ciCols(t.columns);
    for (const term of GLOSSARY.filter((g) => g.match && g.read)) {
      const byColumn = term.where !== 'row';
      const byRow = term.where === 'row' || term.where === 'both';
      if (byColumn) {
        const j = t.columns.findIndex((c) => term.match!.test(c.trim()));
        if (j >= 0) {
          // The first row that has a value there; for a regression, a predictor rather than the intercept.
          const rows = t.rows.filter((r) => numberOf(r[j]) !== null);
          const row = rows.find((r) => textOf(r[0]) !== '(Intercept)') ?? (term.id === 'B' || term.id === 'beta' || term.id === 'or' ? undefined : rows[0]);
          if (row) {
            // A row's interval belongs to the estimate it follows: the mean difference, a correlation, a coefficient
            // (the odds ratio's, when the table gives one), never to a test statistic.
            const owner = ['diff', 'r', 'or'].includes(term.id) || (term.id === 'B' && !t.columns.includes('Odds ratio'));
            const rowCi = owner && ci && ci.lo > j && numberOf(row[ci.lo]) !== null && numberOf(row[ci.hi]) !== null ? { lower: numberOf(row[ci.lo])!, upper: numberOf(row[ci.hi])!, level: ci.level } : undefined;
            add(term, j > 0 ? textOf(row[0]) : null, numberOf(row[j])!, rowCi);
          }
        }
      }
      if (byRow && !seen.has(term.id)) {
        const row = t.rows.find((r) => term.match!.test(textOf(r[0]) ?? ''));
        const k = row ? row.findIndex((c, i) => i > 0 && numberOf(c) !== null) : -1;
        if (row && k > 0) {
          const rowCi = ci && numberOf(row[ci.lo]) !== null && numberOf(row[ci.hi]) !== null ? { lower: numberOf(row[ci.lo])!, upper: numberOf(row[ci.hi])!, level: ci.level } : undefined;
          add(term, textOf(row[0]), numberOf(row[k])!, rowCi);
        }
      }
    }
  }
  readings.sort((a, b) => (a.term.rank ?? 50) - (b.term.rank ?? 50));

  const checks = bundle.diagnostics
    .filter((d) => d.state === 'review_needed' || d.state === 'unsupported_condition')
    .map((d) => `${d.title}: ${d.message}`);
  const unchecked = bundle.diagnostics.filter((d) => d.state === 'not_assessed').map((d) => `${d.title}: ${d.message}`);

  const cautions: string[] = [];
  const has = (id: string) => readings.some((r) => r.term.id === id);
  if (has('p')) cautions.push('The p value says how surprising this result would be if there were no effect. It is not the chance that the null hypothesis is true, and it doesn’t say whether the effect is large enough to matter. Read it with the effect size and its confidence interval.');
  if (/^(corr|reg)\./.test(bundle.method_id) || has('r')) cautions.push('A relationship in data that weren’t from an experiment doesn’t show that one variable causes the other: a third variable may drive both, or the cause may run the other way.');
  if (bundle.cases.analyzed < bundle.cases.total) cautions.push(`${bundle.cases.total - bundle.cases.analyzed} of ${bundle.cases.total} cases were left out (see the case summary). If the missing cases differ from the others, the result may not describe everyone.`);

  return { readings: readings.slice(0, max), checks, unchecked, cautions };
}

/** A confidence interval as the explanation words it. */
export function ciSentence(r: Reading): string | null {
  if (!r.ci) return null;
  const ratio = r.term.id === 'or';
  const none = ratio ? 1 : 0;
  const covers = r.ci.lower <= none && r.ci.upper >= none;
  const f = (x: number) => (r.term.id === 'r' ? noLead(x) : fmt(x, 2));
  return `${r.ci.level} CI [${f(r.ci.lower)}, ${f(r.ci.upper)}]: the population value is plausibly anywhere in this range. It ${covers ? 'includes' : 'leaves out'} ${none}, ${covers ? `so “no ${ratio ? 'change in the odds' : 'effect'}” can’t be ruled out at this level` : `so “no ${ratio ? 'change in the odds' : 'effect'}” is ruled out at this level`}.`;
}
