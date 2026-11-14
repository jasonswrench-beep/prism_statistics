// The help pages: every dialog the menus open has a page, pages name no proprietary package, internal links point at
// pages that exist, and the renderer draws headings, lists, tables, code and links.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { Markdown } from './markdown';

const here = new URL('.', import.meta.url);
const read = (rel: string) => readFileSync(new URL(rel, here), 'utf8');
const topics = readdirSync(new URL('topics/', here)).filter((f) => f.endsWith('.md')).map((f) => f.slice(0, -3));
const guide = readdirSync(new URL('guide/', here)).filter((f) => f.endsWith('.md')).map((f) => f.slice(0, -3).replace(/^\d+-/, ''));
const examples = readdirSync(new URL('examples/', here)).filter((f) => f.endsWith('.md')).map((f) => f.slice(0, -3).replace(/^\d+-/, ''));
const pages = [...topics.map((t) => `topics/${t}.md`), ...readdirSync(new URL('guide/', here)).map((g) => `guide/${g}`), ...readdirSync(new URL('examples/', here)).map((e) => `examples/${e}`)];
const datasets = readdirSync(new URL('../../../../crates/prism-api/data/examples/', here)).filter((f) => f.endsWith('.json')).map((f) => f.slice(0, -5));

test('every worked example opens an example dataset', () => {
  assert.ok(examples.length >= 3);
  for (const e of readdirSync(new URL('examples/', here))) assert.match(read(`examples/${e}`), /\]\(example:[a-z_]+\)/, e);
});

test('every dialog the menus open has a help page', () => {
  const app = read('../App.tsx');
  const ids = new Set([...app.matchAll(/(?:open|setDialog)\('([a-z0-9_]+)'/g)].map((m) => m[1]));
  const notHelp = new Set(['about', 'ai', 'mcp', 'sheet', 'google']);
  const missing = [...ids].filter((id) => !notHelp.has(id)).map((id) => (id === 'design' ? 'doe_design' : id)).filter((id) => !topics.includes(id));
  assert.deepEqual(missing, []);
});

test('pages name no proprietary package and link only to pages that exist', () => {
  const banned = /\b(SPSS|Stata|SAS\b(?!7)|Amos|IBM|Excel|Microsoft|Minitab|JMP|Mplus)/;
  for (const p of pages) {
    const text = read(p);
    assert.ok(/^# .+/m.test(text), `${p} has a title`);
    assert.ok(!banned.test(text), `${p}: ${banned.exec(text)?.[0]}`);
    for (const m of text.matchAll(/\]\(topic:([a-z_-]+)\)/g)) assert.ok(topics.includes(m[1]) || guide.includes(m[1]) || examples.includes(m[1]) || m[1] === 'glossary', `${p} links to missing page ${m[1]}`);
    for (const m of text.matchAll(/\]\(example:([a-z_]+)\)/g)) assert.ok(datasets.includes(m[1]), `${p} opens missing example ${m[1]}`);
  }
});

test('the interface names no proprietary product', () => {
  // File formats (.sav, .sas7bdat, .xlsx, .jasp) are fine; product names are not.
  const banned = /\b(SPSS|Stata|SAS\b(?!7)|Amos|IBM|Excel|Microsoft|Minitab|JMP|Mplus|Google Sheets)/;
  const files = ['../App.tsx', ...readdirSync(new URL('../components/', here)).filter((f) => f.endsWith('.tsx')).map((f) => `../components/${f}`)];
  assert.ok(files.includes('../components/DataToolDialogs.tsx'));
  for (const f of files) {
    const text = read(f);
    assert.ok(!banned.test(text), `${f}: ${banned.exec(text)?.[0]}`);
  }
});

test('the renderer draws the parts the pages use', () => {
  const html = renderToStaticMarkup(
    createElement(Markdown, {
      text: '# Title\n\nA **bold** and *slanted* `code` [page](topic:data).\n\n## Options\n- one\n- two\n\n1. first\n2. second\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n```\nrun x\n```\n',
      open: () => undefined,
    }),
  );
  assert.ok(!html.includes('Title'), 'the page title is shown by the viewer, not repeated');
  for (const part of ['<strong>bold</strong>', '<em>slanted</em>', '<code>code</code>', '<h3>Options</h3>', '<ul><li>one</li><li>two</li></ul>', '<ol><li>first</li><li>second</li></ol>', '<th>A</th>', '<td>2</td>', '<pre>run x</pre>', '>page</a>']) {
    assert.ok(html.includes(part), `${part} in ${html}`);
  }
});
