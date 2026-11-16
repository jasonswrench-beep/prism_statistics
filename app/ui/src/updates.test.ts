import { test } from 'node:test';
import assert from 'node:assert/strict';
import { dailyCheckDue, progressPercent } from './updates';
import { DEFAULTS, parseSettings } from './settings';

test('the download shows a whole percent, never more than 100, and none when the size is unknown', () => {
  assert.equal(progressPercent(0, 200), 0);
  assert.equal(progressPercent(199, 200), 99);
  assert.equal(progressPercent(250, 200), 100);
  assert.equal(progressPercent(10, null), null);
  assert.equal(progressPercent(10, 0), null);
});

test('the daily check is off by default, and due only when on and a day has passed', () => {
  assert.equal(DEFAULTS.updateDaily, false);
  assert.equal(parseSettings('{}').updateDaily, false);
  assert.equal(parseSettings('{"updateDaily": "yes"}').updateDaily, false, 'only true turns it on');
  assert.equal(parseSettings('{"updateLastCheck": "not a time"}').updateLastCheck, '');
  const now = new Date('2026-11-16T12:00:00Z');
  assert.equal(dailyCheckDue(false, '', now), false, 'off: never due');
  assert.equal(dailyCheckDue(true, '', now), true, 'on and never checked');
  assert.equal(dailyCheckDue(true, '2026-11-16T00:00:01Z', now), false, 'checked under a day ago');
  assert.equal(dailyCheckDue(true, '2026-11-15T12:00:00Z', now), true, 'a full day ago');
});
