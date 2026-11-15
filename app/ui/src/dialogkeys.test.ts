// The dialogs in App.tsx are siblings, so their React keys must differ. A help page's id is often the same as its
// dialog's (ttest_one_sample, anova_oneway…), and when the Help window and an analysis dialog were both keyed by the bare
// id, closing Help left a stray, unclosable copy of the dialog behind.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const app = readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

test('keyed dialogs in App.tsx each use their own key prefix', () => {
  const keys = [...app.matchAll(/<(AnalysisDialog|SemBuilder|HelpDialog)\b[^>]*?\bkey=\{([^}]*)\}/gs)].map((m) => [m[1], m[2]]);
  assert.deepEqual(keys.map((k) => k[0]).sort(), ['AnalysisDialog', 'HelpDialog', 'SemBuilder']);
  const prefixes = keys.map(([name, k]) => {
    const p = k.match(/^`(\w+):/)?.[1];
    assert.ok(p, `${name} key={${k}} needs a prefix such as \`help:\``);
    return p;
  });
  assert.equal(new Set(prefixes).size, prefixes.length);
});
