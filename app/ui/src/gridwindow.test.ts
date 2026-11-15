import { test } from 'node:test';
import assert from 'node:assert/strict';
import { columnWidth, offsets, visibleColumns, visibleRows } from './gridwindow';

test('a column is as wide as its name or its longest value, within limits', () => {
  assert.equal(columnWidth({ name: 'id', label: '' }, [1, 2, 3]), 64);
  const wide = columnWidth({ name: 'satisfaction_overall', label: '' }, [4]);
  assert.ok(wide > 150 && wide < 200, String(wide));
  assert.equal(columnWidth({ name: 'comment', label: '' }, ['x'.repeat(300)]), 260);
  assert.ok(columnWidth({ name: 'q', label: '' }, [{ missing: -999999 }]) > 64);
});

test('only the columns in view, and a margin, are drawn', () => {
  const edges = offsets(Array(500).fill(100));
  assert.equal(edges.length, 501);
  assert.equal(edges[500], 50_000);
  assert.deepEqual(visibleColumns(edges, 0, 1000, 0), [0, 11]);
  assert.deepEqual(visibleColumns(edges, 0, 1000), [0, 15]);
  assert.deepEqual(visibleColumns(edges, 25_050, 1000, 2), [248, 263]);
  assert.deepEqual(visibleColumns(edges, 49_500, 1000, 2), [493, 500], 'never past the last column');
  assert.deepEqual(visibleColumns(offsets([]), 0, 1000), [0, 0]);
});

test('only the rows in view, and a margin, are drawn', () => {
  assert.deepEqual(visibleRows(1_000_000, 31, 0, 620, 0), [0, 20]);
  assert.deepEqual(visibleRows(1_000_000, 31, 31_000, 620, 10), [990, 1030]);
  assert.deepEqual(visibleRows(100, 31, 99_999, 620, 10), [100, 100], 'scrolled past the end: nothing');
  assert.deepEqual(visibleRows(5, 31, 0, 620), [0, 5]);
});
