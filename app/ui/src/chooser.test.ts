// Help me choose: every answer leads somewhere, every suggestion can be reached and opens a dialog that exists and has a
// help page, and the variables picked in the panel land in that dialog's roles.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { ENDINGS, LEVELS, LEVELS_NOTE, NODES, QUESTIONS, isEnding, startArgs, type SlotKey } from './chooser';
import { ANALYSES, restore } from './analyses';
import type { Measure } from './types';

const OWN_DIALOGS = new Set(['ttest', 'frequencies', 'descriptives', 'power', 'cfa']);

test('every answer leads to a question or a suggestion, and every node can be reached', () => {
  const seen = new Set<string>();
  const visit = (id: string) => {
    if (seen.has(id)) return;
    const n = NODES[id];
    assert.ok(n, `missing node ${id}`);
    seen.add(id);
    if (isEnding(n)) n.also?.forEach(visit);
    else n.answers.forEach((a) => visit(a.next));
  };
  visit('start');
  const unreached = [...QUESTIONS, ...ENDINGS].map((n) => n.id).filter((id) => !seen.has(id));
  assert.deepEqual(unreached, []);
  for (const q of QUESTIONS) assert.ok(q.answers.length >= 2, `${q.id} offers a choice`);
  assert.equal(new Set([...QUESTIONS, ...ENDINGS].map((n) => n.id)).size, QUESTIONS.length + ENDINGS.length, 'ids are unique');
});

test('every suggestion opens a dialog that exists and has a help page', () => {
  for (const e of ENDINGS) {
    assert.ok(ANALYSES[e.dialog] || OWN_DIALOGS.has(e.dialog), `${e.id}: no dialog ${e.dialog}`);
    assert.ok(existsSync(new URL(`./help/topics/${e.dialog}.md`, import.meta.url)), `${e.id}: no help page for ${e.dialog}`);
    for (const a of e.also ?? []) {
      assert.ok(a !== e.id && isEnding(NODES[a]), `${e.id}: also ${a} is not another suggestion`);
    }
    assert.equal(new Set(e.slots.map((s) => s.key)).size, e.slots.length, `${e.id}: slot keys are unique`);
  }
});

test('the variables picked land in the dialog’s roles', () => {
  for (const e of ENDINGS) {
    const chosen: Partial<Record<SlotKey, string[]>> = {};
    const measures: Record<string, Measure> = {};
    const all: string[] = [];
    e.slots.forEach((s, i) => {
      const names = s.many ? [`${s.key}_a`, `${s.key}_b`] : [`${s.key}_v`];
      chosen[s.key] = names;
      names.forEach((n, j) => (measures[n] = (i + j) % 2 ? 'nominal' : 'scale'));
      all.push(...names);
    });
    const args = startArgs(e, chosen, (n) => measures[n]);
    const def = ANALYSES[e.dialog];
    let placed: string[];
    if (def) {
      const c = restore(def, args);
      const keys = new Set(def.roles.map((r) => r.key));
      for (const k of [...Object.keys(c.roles).filter((k) => c.roles[k].length), ...Object.keys(c.pairs)]) assert.ok(keys.has(k), `${e.id}: role ${k} isn't in ${e.dialog}`);
      placed = [...Object.values(c.roles).flat(), ...Object.values(c.pairs).flat(2)];
      for (const o of def.options) if (args[o.key] !== undefined) assert.deepEqual(c.options[o.key], args[o.key], `${e.id}: option ${o.key}`);
    } else {
      placed = Object.values(args).flatMap((v) => (Array.isArray(v) ? v.map(String) : typeof v === 'string' ? [v] : []));
    }
    // Single-variable slots of paired dialogs take one each; every chosen variable must arrive.
    const missing = all.filter((n) => !placed.includes(n) && !(n.endsWith('_b') && (e.slots.find((s) => n.startsWith(s.key))?.key === 'first' || e.slots.find((s) => n.startsWith(s.key))?.key === 'second')));
    assert.deepEqual(missing, [], `${e.id}: ${JSON.stringify(args)}`);
  }
});

test('the t test and power suggestions start their own dialogs with the right settings', () => {
  const t = ENDINGS.find((e) => e.id === 'ttest')!;
  assert.deepEqual(startArgs(t, { outcome: ['score'], group: ['cond'] }, () => 'scale'), { test_variables: ['score'], grouping_variable: 'cond' });
  assert.deepEqual(startArgs(t, {}, () => undefined), {}, 'nothing picked: the dialog starts empty');
  const p = ENDINGS.find((e) => e.id === 'power_anova')!;
  assert.deepEqual(startArgs(p, {}, () => undefined), { test: 'anova' });
  assert.ok(p.noData);
  // A variable chosen for a slot the suggestion doesn't show is not passed on.
  assert.deepEqual(startArgs(t, { outcome: ['score'], measures: ['a', 'b'] }, () => 'scale'), { test_variables: ['score'] });
});

test('the questions about variables offer the levels of measurement, and ordinal outcomes go to rank-based analyses', () => {
  for (const q of QUESTIONS.filter((q) => q.levels)) {
    const labels = q.answers.map((a) => a.label).join(' | ');
    assert.ok(/nominal/.test(labels) && /interval or ratio/.test(labels), `${q.id}: ${labels}`);
    if (q.id !== 'describe') assert.ok(/ordinal/.test(labels), `${q.id} has an ordinal answer`);
  }
  // Following an ordinal answer never reaches a test of means.
  const means = new Set(['ttest', 'ttest_one_sample', 'ttest_paired', 'anova_oneway', 'rm_anova', 'anova_factorial', 'manova', 'pearson', 'regression']);
  const reach = (id: string, seen = new Set<string>()): Set<string> => {
    if (seen.has(id)) return seen;
    seen.add(id);
    const n = NODES[id];
    if (!isEnding(n)) n.answers.forEach((a) => reach(a.next, seen));
    return seen;
  };
  for (const q of QUESTIONS) {
    for (const a of q.answers.filter((a) => /\(ordinal\)/.test(a.label))) {
      const hit = [...reach(a.next)].filter((id) => means.has(id));
      assert.deepEqual(hit, [], `${q.id} › ${a.label}`);
    }
  }
  // Times with censored cases are scores that reach the survival analyses.
  assert.ok(reach('compare_time').has('kaplan_meier') && reach('predict_kind').has('cox_regression'));
});

test('the questions name no proprietary package', () => {
  const banned = /\b(SPSS|Stata|SAS\b|Amos|IBM|Excel|Microsoft|Minitab|JMP|Mplus)/;
  const text = JSON.stringify([QUESTIONS, LEVELS, LEVELS_NOTE, ENDINGS.map(({ args, ...e }) => e)]);
  assert.ok(!banned.test(text), banned.exec(text)?.[0] ?? '');
});
