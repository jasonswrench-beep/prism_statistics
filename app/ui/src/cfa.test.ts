import test from 'node:test';
import assert from 'node:assert/strict';
import { cfaModel, validName } from './cfa';

const fs = [
  { name: 'Visual', items: ['x1', 'x2', 'x3'] },
  { name: 'Textual', items: ['x4', 'x5', 'x6'] },
  { name: 'Speed', items: ['x7', 'x8', 'x9'] },
];

test('correlated factors are one line each', () => {
  assert.equal(cfaModel(fs, true), ['Visual =~ x1 + x2 + x3', 'Textual =~ x4 + x5 + x6', 'Speed =~ x7 + x8 + x9'].join('\n'));
});

test('uncorrelated factors fix every covariance at zero', () => {
  const m = cfaModel(fs, false).split('\n');
  assert.deepEqual(m.slice(3), ['Visual ~~ 0*Textual', 'Visual ~~ 0*Speed', 'Textual ~~ 0*Speed']);
});

test('empty factors are left out', () => {
  assert.equal(cfaModel([fs[0], { name: 'F2', items: [] }], false), 'Visual =~ x1 + x2 + x3');
});

test('factor names follow the model syntax', () => {
  assert.ok(validName('Self.Esteem_2'));
  assert.ok(!validName('2nd'));
  assert.ok(!validName('Self Esteem'));
});
