import { test } from 'node:test';
import assert from 'node:assert/strict';
import { arrowTarget } from './dialogfocus';

test('the arrow keys move through a list box and stop at its ends', () => {
  const opts = ['a', 'b', 'c'].map((id) => ({ id }) as unknown as HTMLElement);
  assert.equal(arrowTarget('ArrowDown', opts, opts[0]), opts[1]);
  assert.equal(arrowTarget('ArrowDown', opts, opts[2]), opts[2]);
  assert.equal(arrowTarget('ArrowUp', opts, opts[0]), opts[0]);
  assert.equal(arrowTarget('End', opts, opts[0]), opts[2]);
  assert.equal(arrowTarget('Home', opts, opts[2]), opts[0]);
  assert.equal(arrowTarget('Tab', opts, opts[1]), null);
  assert.equal(arrowTarget('ArrowDown', opts, {} as HTMLElement), null);
});
