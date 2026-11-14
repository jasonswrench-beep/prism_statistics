// Explain this result reads the statistics a result reports, in order, with each interval attached to the estimate it
// belongs to, and adds the checks and cautions that apply. The bundles here copy real results' tables.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { ciSentence, explain } from './explain';
import { GLOSSARY, glossaryPage, pText } from './glossary';
import type { ResultBundle, ResultCell, ResultTable } from './types';

const n = (value: number, decimals = 2): ResultCell => ({ kind: 'number', value, decimals });
const p = (value: number): ResultCell => ({ kind: 'p', value });
const t = (text: string): ResultCell => ({ kind: 'text', text });
const e: ResultCell = { kind: 'empty' };
const table = (title: string, columns: string[], rows: ResultCell[][]): ResultTable => ({ id: title, title, columns, rows, footnotes: [] });
const bundle = (method_id: string, tables: ResultTable[], extra: Partial<ResultBundle> = {}): ResultBundle => ({
  method_id,
  method_version: 1,
  title: method_id,
  settings: {},
  status: { state: 'ok' },
  cases: { total: 120, analyzed: 120, excluded: [] },
  tables,
  diagnostics: [],
  method_note: '',
  fields: {},
  ...extra,
});

const ttest = bundle('ttest.independent', [
  table('Group statistics', ['Group', 'N', 'Mean', 'SD', 'SE of mean'], [[t('Lecture'), { kind: 'count', value: 60 }, n(65.67), n(13.33), n(1.72)]]),
  table('Independent-samples t test', ['Test', 't', 'df', 'p (two-sided)', 'Mean difference', 'SE', '95% CI lower', '95% CI upper'], [[t("Welch's unequal-variances t test"), n(-2.29), n(117.27), p(0.0238), n(-5.37), n(2.34), n(-10.01), n(-0.73)]]),
  table('Effect size', ['Measure', 'Estimate', '95% CI lower', '95% CI upper'], [
    [t("Hedges' g (pooled SD)"), n(-0.416), n(-0.774), n(-0.055)],
    [t("Cohen's d (pooled SD)"), n(-0.418), n(-0.779), n(-0.056)],
  ]),
]);

test('a t test is read as its statistic, p, the difference with its interval, and the effect sizes', () => {
  const x = explain(ttest);
  assert.deepEqual(
    x.readings.map((r) => r.term.id),
    ['t', 'df', 'p', 'diff', 'd', 'g'].filter((id) => GLOSSARY.find((g) => g.id === id)?.read),
  );
  const tt = x.readings.find((r) => r.term.id === 't')!;
  assert.equal(tt.ci, undefined, 'the interval belongs to the difference, not to t');
  assert.match(tt.sentence, /2\.29 times its standard error \(negative/);
  const diff = x.readings.find((r) => r.term.id === 'diff')!;
  assert.deepEqual(diff.ci, { lower: -10.01, upper: -0.73, level: '95%' });
  assert.match(ciSentence(diff)!, /\[−10\.01, −0\.73\].*leaves out 0/);
  const pr = x.readings.find((r) => r.term.id === 'p')!;
  assert.match(pr.sentence, /^p = \.024\. .*about 2 in 100 .*statistically significant/);
  const g = x.readings.find((r) => r.term.id === 'g')!;
  assert.equal(g.label, "Hedges' g (pooled SD)");
  assert.match(g.sentence, /0\.42 standard deviations/);
  assert.ok(g.ci && ciSentence(g)!.includes('leaves out 0'));
  assert.ok(x.cautions.some((c) => c.includes('not the chance that the null hypothesis is true')));
  assert.ok(!x.cautions.some((c) => c.includes('cause')), 'no causation note for a group comparison');
  assert.deepEqual(x.checks, []);
  assert.deepEqual(x.unchecked, []);
  const shape = explain({ ...ttest, diagnostics: [{ id: 'shape', title: 'Shape and outliers', state: 'not_assessed', message: 'Look at the boxplot.' }] });
  assert.deepEqual([shape.checks, shape.unchecked], [[], ['Shape and outliers: Look at the boxplot.']]);
});

test('a regression reads a predictor, not the intercept, and a logistic interval goes with the odds ratio', () => {
  const reg = bundle('reg.linear', [
    table('Model summary', ['R', 'R²', 'Adjusted R²', 'SE of the estimate', 'N'], [[n(0.358), n(0.128), n(0.121), n(12.24), { kind: 'count', value: 120 }]]),
    table('Coefficients', ['Term', 'B', 'SE', 'β', 't', 'p', '95% CI lower', '95% CI upper', 'VIF'], [
      [t('(Intercept)'), n(58.91), n(2.53), e, n(23.3), p(1e-40), n(53.9), n(63.91), e],
      [t('Study hours per week'), n(1.51), n(0.36), n(0.358), n(4.16), p(0.00006), n(0.79), n(2.23), n(1)],
    ]),
  ]);
  const x = explain(reg);
  const b = x.readings.find((r) => r.term.id === 'B')!;
  assert.equal(b.label, 'Study hours per week');
  assert.deepEqual(b.ci, { lower: 0.79, upper: 2.23, level: '95%' });
  assert.match(x.readings.find((r) => r.term.id === 'r2')!.sentence, /12\.8% of the variance/);
  assert.equal(x.readings.find((r) => r.term.id === 'p')!.sentence.slice(0, 9), 'p < .001.');
  assert.ok(x.cautions.some((c) => c.includes('cause')));

  const logit = bundle('reg.logistic', [
    table('Coefficients', ['Term', 'B', 'SE', 'Wald', 'df', 'p', 'Odds ratio', '95% CI lower', '95% CI upper'], [
      [t('(Intercept)'), n(-0.2), n(1.1), n(0.03), { kind: 'count', value: 1 }, p(0.86), n(0.82), e, e],
      [t('x'), n(-0.009), n(0.07), n(0.015), { kind: 'count', value: 1 }, p(0.902), n(0.991), n(0.863), n(1.138)],
    ]),
  ]);
  const y = explain(logit);
  assert.equal(y.readings.find((r) => r.term.id === 'B')!.ci, undefined, 'the interval is the odds ratio’s');
  const or = y.readings.find((r) => r.term.id === 'or')!;
  assert.deepEqual(or.ci, { lower: 0.863, upper: 1.138, level: '95%' });
  assert.match(ciSentence(or)!, /includes 1/);
  assert.match(or.sentence, /multiplies the odds of the outcome by 0\.99 \(1% lower odds\)/);
  assert.match(y.readings.find((r) => r.term.id === 'p')!.sentence, /not statistically significant/);
});

test('a crosstab reads chi-square and Cramér’s V from their rows; checks and lost cases are named', () => {
  const x = explain(
    bundle(
      'desc.crosstabs',
      [
        table('Chi-square tests', ['Test', 'Value', 'df', 'p (two-sided)'], [[t('Pearson chi-square'), n(0.069), { kind: 'count', value: 1 }, p(0.793)]]),
        table('Effect size', ['Measure', 'Value'], [[t('Phi (φ)'), n(0.034)], [t("Cramér's V"), n(0.034)]]),
      ],
      { diagnostics: [{ id: 'expected', title: 'Expected counts', state: 'review_needed', message: '2 cells have expected counts below 5.' }], cases: { total: 120, analyzed: 114, excluded: [{ reason: 'missing', count: 6 }] } },
    ),
  );
  assert.deepEqual(x.readings.map((r) => r.term.id).filter((id) => id === 'chi2' || id === 'cramer'), ['chi2', 'cramer']);
  assert.equal(x.readings.find((r) => r.term.id === 'cramer')!.sentence, 'V = .03 on a scale from 0 (no association) to 1 (complete association).');
  assert.deepEqual(x.checks, ['Expected counts: 2 cells have expected counts below 5.']);
  assert.ok(x.cautions.some((c) => c.startsWith('6 of 120 cases were left out')));
});

test('the glossary page lists every term, and p values are written the APA way', () => {
  const page = glossaryPage();
  for (const g of GLOSSARY) assert.ok(page.includes(`**${g.term}**`), g.term);
  assert.equal(new Set(GLOSSARY.map((g) => g.id)).size, GLOSSARY.length);
  assert.deepEqual([pText(0.0004), pText(0.0238), pText(0.5)], ['< .001', '= .024', '= .500']);
  assert.ok(!/\b(SPSS|Stata|SAS\b|Amos|IBM|Excel|Microsoft|Minitab|JMP|Mplus)/.test(page));
});
