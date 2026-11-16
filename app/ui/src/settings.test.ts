// Tools › Settings: damaged or missing storage gives the defaults, and the confidence level setting reaches new dialogs.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { DEFAULTS, getSettings, parseSettings, saveSettings } from './settings';
import { ANALYSES, optionDefault, restore } from './analyses';

test('stored settings are read back, and anything unusable falls back to the defaults', () => {
  assert.deepEqual(parseSettings(null), DEFAULTS);
  assert.deepEqual(parseSettings('not json'), DEFAULTS);
  assert.deepEqual(parseSettings('[1,2]'), DEFAULTS);
  assert.deepEqual(parseSettings(JSON.stringify({ textSize: 130, valueLabels: true, browseOnly: false, level: 99, extra: 1 })), { textSize: 130, valueLabels: true, browseOnly: false, level: 99, updateDaily: false, updateLastCheck: '' });
  assert.deepEqual(parseSettings(JSON.stringify({ textSize: 200, valueLabels: 'yes', level: 100 })), DEFAULTS);
  assert.equal(parseSettings(JSON.stringify({ level: 49.9 })).level, 95);
  assert.equal(parseSettings(JSON.stringify({ level: 90 })).level, 90);
});

test('the confidence level setting is where new dialogs start, but reopened results keep theirs', () => {
  const level = ANALYSES.ttest_paired.options.find((o) => o.key === 'level')!;
  assert.equal(optionDefault(level), '95');
  saveSettings({ ...getSettings(), level: 99 });
  try {
    assert.equal(optionDefault(level), '99');
    assert.equal(restore(ANALYSES.ttest_paired, { pairs: [['a', 'b']] }).options.level, '99', 'a dialog started from Help me choose');
    assert.equal(restore(ANALYSES.ttest_paired, { pairs: [['a', 'b']], level: 0.9 }).options.level, '90', 'a result reopened with its settings');
    const other = ANALYSES.ttest_paired.options.find((o) => o.key !== 'level')!;
    assert.equal(optionDefault(other), other.default);
  } finally {
    saveSettings(DEFAULTS);
  }
});
