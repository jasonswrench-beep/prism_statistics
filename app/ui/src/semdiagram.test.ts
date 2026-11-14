import { test } from 'node:test';
import assert from 'node:assert/strict';
import { inferEdge, modelText, paramName } from './semdiagram';

test('the model text is rebuilt from a diagram, extras kept', () => {
  const nodes = [
    { name: 'f', kind: 'latent' as const },
    { name: 'g', kind: 'composite' as const },
    { name: 'pending', kind: 'latent' as const },
  ];
  const text = modelText(
    nodes,
    [
      { from: 'f', to: 'x1', kind: 'loading' },
      { from: 'f', to: 'x2', kind: 'loading', modifier: 'a' },
      { from: 'x3', to: 'g', kind: 'composite' },
      { from: 'f', to: 'y', kind: 'regression' },
      { from: 'g', to: 'y', kind: 'regression', modifier: 'b' },
      { from: 'x1', to: 'x2', kind: 'covariance' },
    ],
    ['ab := a*b'],
  );
  assert.equal(text, 'f =~ x1 + a*x2\ng <~ x3\ny ~ f + b*g\nx1 ~~ x2\nab := a*b');
});

test('an arrow means what its ends make it', () => {
  const lat = { name: 'f', kind: 'latent' as const };
  const obs = { name: 'x', kind: 'observed' as const };
  const comp = { name: 'c', kind: 'composite' as const };
  assert.equal(paramName(inferEdge(lat, obs)), 'f =~ x');
  assert.equal(paramName(inferEdge(obs, comp)), 'c <~ x');
  assert.equal(paramName(inferEdge(obs, lat)), 'f ~ x');
  assert.equal(paramName(inferEdge(lat, lat)), 'f ~ f');
});
