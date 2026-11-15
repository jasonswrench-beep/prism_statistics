import { test } from 'node:test';
import assert from 'node:assert/strict';
import { cellText, fileName, italicRuns, num, parseCodes, parseValueLabels, pCell, plainText } from './format';

test('numbers and p values read like the engine writes them', () => {
  assert.equal(num(-1.5, 2), '−1.50');
  assert.equal(num(-0.001, 2), '0.00');
  assert.equal(pCell(0.0004), '< .001');
  assert.equal(pCell(0.0451), '.045');
  assert.equal(cellText({ kind: 'p', value: 0.5 }), '.500');
  assert.equal(cellText({ kind: 'number', value: Number.NaN, decimals: 2 }), '—');
});

test('italic markers become runs', () => {
  assert.deepEqual(italicRuns('*t*(10) = 2'), [
    { text: 't', italic: true },
    { text: '(10) = 2', italic: false },
  ]);
  assert.deepEqual(italicRuns('*d*<sub>z</sub> = 1'), [
    { text: 'd', italic: true },
    { text: 'z', italic: false, sub: true },
    { text: ' = 1', italic: false },
  ]);
  assert.equal(plainText('*r*<sub>s</sub>(55) = .44'), 'rs(55) = .44');
});

test('value labels and missing codes parse, and bad lines are named', () => {
  assert.deepEqual(parseValueLabels('1 = Urban\n2: Rural\n\n').labels, [
    { value: 1, label: 'Urban' },
    { value: 2, label: 'Rural' },
  ]);
  assert.match(parseValueLabels('Urban').error ?? '', /Line 1/);
  assert.match(parseValueLabels('1 = A\n1 = B').error ?? '', /two labels/);
  assert.deepEqual(parseCodes('-99, 9 9').codes, [-99, 9]);
  assert.match(parseCodes('x').error ?? '', /not a number/);
  // A range is refused with a message that says how to type codes, and negative codes still work.
  for (const r of ['90 to 99', '90-99', '90..99', '90 thru 99']) assert.match(parseCodes(r).error ?? '', /separated by commas/);
  assert.deepEqual(parseCodes('-9, -99, 1e-3').codes, [-9, -99, 0.001]);
});

test('messages name a file without its folders', () => {
  assert.equal(fileName('C:\\Users\\someone\\Documents\\study.pstat'), 'study.pstat');
  assert.equal(fileName('/home/someone/report.docx'), 'report.docx');
  assert.equal(fileName('study.pstat'), 'study.pstat');
});
