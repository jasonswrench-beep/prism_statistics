// The Output tab: one card per result with its tables, diagnostics, method note, case accounting and the template
// APA paragraph. A failed result shows its reason and no interpretation (defaults table, row 16).

import { useState } from 'react';
import { call } from '../bridge';
import { cellText, italicRuns, plainText } from '../format';
import { chooseItemPath, download } from '../files';
import type { DiagnosticState, OutputItem, ResultTable } from '../types';
import { Icon } from './Icon';
import { AiDraftBlock } from './AiWriter';
import { ExplainBlock } from './ExplainBlock';

const STATE: Record<DiagnosticState, { words: string; cls: string; icon: string }> = {
  no_concern_detected: { words: 'No concern detected', cls: 'ok', icon: 'check' },
  review_needed: { words: 'Review needed', cls: 'review', icon: 'warn' },
  unsupported_condition: { words: 'Unsupported condition', cls: 'stale', icon: 'warn' },
  not_assessed: { words: 'Not assessed', cls: 'na', icon: 'minus' },
};

function Italic({ text }: { text: string }) {
  return (
    <>
      {italicRuns(text).map((r, i) => (r.italic ? <i key={i}>{r.text}</i> : r.sub ? <sub key={i}>{r.text}</sub> : <span key={i}>{r.text}</span>))}
    </>
  );
}

/** Clicking an arrow of a path diagram (an element with data-param, such as "visual =~ x1" or "y ~ x") selects the
 *  table row of that parameter in the same result: by its name, as "x → y", or by its construct and indicator. */
function showParam(e: React.MouseEvent<HTMLDivElement>) {
  const g = (e.target as Element).closest('[data-param]');
  const result = (e.currentTarget as HTMLElement).closest('.tabpanel');
  if (!g || !result) return;
  const param = g.getAttribute('data-param') ?? '';
  const squash = (t: string | null) => (t ?? '').replace(/\s+/g, '');
  const m = /^(.+?)\s*(=~|<~|~~|~)\s*(.+)$/.exec(param);
  const wanted = new Set([squash(param)]);
  if (m && m[2] === '~') wanted.add(squash(`${m[3]} → ${m[1]}`));
  if (m && m[2] === '<~') wanted.add(squash(`${m[3]} → ${m[1]}`));
  for (const row of Array.from(result.querySelectorAll<HTMLTableRowElement>('table.apa tbody tr'))) {
    const c0 = squash(row.cells[0]?.textContent);
    const c1 = squash(row.cells[1]?.textContent);
    if (wanted.has(c0) || (m && (m[2] === '=~' || m[2] === '<~') && c0 === squash(m[1]) && c1 === squash(m[3]))) {
      result.querySelectorAll('tr.flash').forEach((r) => r.classList.remove('flash'));
      row.classList.add('flash');
      row.scrollIntoView({ block: 'center', behavior: 'smooth' });
      row.focus({ preventScroll: true });
      return;
    }
  }
}

export function OutputView({ items, onCopied, onChanged, onExport, onEdit, aiReady }: { items: OutputItem[]; onCopied: (msg: string) => void; onChanged: () => void; onExport: (format: 'docx' | 'html' | 'pdf' | 'r') => void; onEdit?: (item: OutputItem) => void; aiReady: boolean }) {
  const [current, setCurrent] = useState<number | null>(null);
  const [sample, setSample] = useState<string | null>(null);
  const [sampleError, setSampleError] = useState<string | null>(null);
  /** One figure (PNG, SVG) or table (CSV) on its own. */
  const saveItem = async (id: number, result: string, item: string, title: string, format: 'png' | 'svg' | 'csv') => {
    try {
      const path = await chooseItemPath(format, `${result} - ${title}`);
      if (path === null) return;
      if (path === undefined) {
        const r = await call<{ file_name: string; base64: string }>('export_item', { id, item, format });
        download(r.file_name, r.base64, format === 'png' ? 'image/png' : format === 'svg' ? 'image/svg+xml' : 'text/csv');
        onCopied(`Saved ${r.file_name}.`);
      } else {
        await call('export_item', { id, item, format, path });
        onCopied(`Saved ${path}.`);
      }
    } catch (e) {
      onCopied((e as Error).message);
    }
  };
  /** A table as tab-separated text, as shown, for pasting into a spreadsheet or document. */
  const copyTable = (t: ResultTable) => {
    const lines = [t.title, t.columns.join('\t'), ...t.rows.map((r) => r.map(cellText).join('\t')), ...(t.footnotes.length ? [`Note. ${t.footnotes.join(' ')}`] : [])];
    navigator.clipboard.writeText(lines.join('\n')).then(
      () => onCopied(`Copied the ${t.title} table. Paste it into a spreadsheet or a document.`),
      () => onCopied('The table could not be copied: the clipboard refused it.'),
    );
  };
  if (!items.length) {
    return (
      <div className="empty-state">
        <Icon name="output" />
        <h2>No output yet</h2>
        <p className="muted">Run an analysis from the Analyze menu. Its tables, checks and a draft APA paragraph appear here.</p>
      </div>
    );
  }
  const copy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(plainText(text));
      onCopied('Paragraph copied');
    } catch {
      onCopied('Copy failed: select the text and press Ctrl+C');
    }
  };
  const describable = items.filter((it) => it.bundle.status.state === 'ok' && (it.bundle.method_id === 'desc.frequencies' || it.bundle.method_id === 'desc.descriptives'));
  const describe = async () => {
    // Every Frequencies result and the latest Descriptives result, in the order they were run.
    const latestDesc = [...describable].reverse().find((it) => it.bundle.method_id === 'desc.descriptives');
    const ids = describable.filter((it) => it.bundle.method_id === 'desc.frequencies' || it === latestDesc).map((it) => it.id);
    try {
      setSampleError(null);
      setSample((await call<{ paragraph: string }>('participants', { ids })).paragraph);
    } catch (e) {
      setSampleError((e as Error).message);
    }
  };
  return (
    <div className="outgrid">
      <nav className="card outline" aria-label="Output outline">
        <ul>
          {items.map((it) => (
            <li key={it.id}>
              <button
                type="button"
                aria-current={current === it.id}
                onClick={() => {
                  setCurrent(it.id);
                  document.getElementById(`result-${it.id}`)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
                }}
              >
                <Icon name="output" />
                {it.bundle.title}
              </button>
            </li>
          ))}
        </ul>
      </nav>
      <div style={{ display: 'grid', gap: 16, alignContent: 'start', minWidth: 0 }}>
        <div className="card" style={{ padding: 14, display: 'flex', gap: 10, alignItems: 'center', flexWrap: 'wrap' }}>
          <button type="button" className="btn" onClick={() => onExport('docx')}>
            <Icon name="text" />
            Export to Word
          </button>
          <button type="button" className="btn" onClick={() => onExport('html')}>
            <Icon name="output" />
            Export to HTML
          </button>
          <button type="button" className="btn" onClick={() => onExport('pdf')}>
            <Icon name="text" />
            Export to PDF
          </button>
          <button type="button" className="btn" onClick={() => onExport('r')} title="An R script that repeats these analyses in R">
            <Icon name="syntax" />
            R script
          </button>
          <span className="fieldhint">Every result below, with APA-style tables, checks, method notes and the template paragraphs. The PDF is tagged for screen readers (PDF/UA-1).</span>
        </div>
        {describable.length > 0 && (
          <div className="card" style={{ padding: 14 }}>
            <div style={{ display: 'flex', gap: 10, alignItems: 'center', flexWrap: 'wrap' }}>
              <button type="button" className="btn" onClick={describe}>
                <Icon name="users" />
                Describe the sample
              </button>
              <span className="fieldhint">Writes a participants paragraph from the Frequencies and Descriptives results above, using only the statistics you ran.</span>
            </div>
            {sampleError && <div className="errbox" role="alert" style={{ marginTop: 10 }}>{sampleError}</div>}
            {sample && (
              <div className="gen" style={{ marginTop: 12 }}>
                <div className="tag">
                  <Icon name="text" />
                  Template text · participants
                </div>
                <p className="para" style={{ margin: 0 }}>
                  <Italic text={sample} />
                </p>
                <div style={{ marginTop: 10 }}>
                  <button type="button" className="btn" onClick={() => copy(sample)}>
                    <Icon name="copy" />
                    Copy paragraph
                  </button>
                </div>
              </div>
            )}
          </div>
        )}
        {items.map((it) => {
          const b = it.bundle;
          const excluded = b.cases.total - b.cases.analyzed;
          return (
            <article className="card result" key={it.id} id={`result-${it.id}`} aria-labelledby={`rt-${it.id}`}>
              <div className="rhead">
                <h2 id={`rt-${it.id}`}>{b.title}</h2>
                {b.status.state === 'failed' ? <span className="status stale">Not computed</span> : null}
                {it.command === 'regression' && b.status.state === 'ok' && (
                  <button
                    type="button"
                    className="btn ghost editsettings"
                    title="Add the predicted values, residuals, standardized residuals, Cook's distances and leverages as new variables"
                    onClick={async () => {
                      try {
                        const r = await call<{ names: string[] }>('save_regression', { id: it.id });
                        onCopied(`Saved ${r.names.join(', ')} to the data.`);
                        onChanged();
                      } catch (e) {
                        onCopied((e as Error).message);
                      }
                    }}
                  >
                    Save values
                  </button>
                )}
                {onEdit && it.command && (
                  <button type="button" className="btn ghost editsettings" onClick={() => onEdit(it)} aria-label={`Open the settings of ${b.title} again`} title="Open this analysis's dialog with the same settings">
                    <Icon name="settings" />
                    Settings…
                  </button>
                )}
                {it.stale && (
                  <span className="ractions">
                    <span className="status review">
                      <Icon name="warn" />
                      Out of date
                    </span>
                    <button type="button" className="btn" onClick={async () => (await call('rerun', { id: it.id }), onChanged())}>
                      <Icon name="refresh" />
                      Run again
                    </button>
                  </span>
                )}
              </div>
              {it.stale && <div className="rmeta" style={{ color: 'var(--warn-ink)' }}>The data changed after this result was run (for example a new missing-value code), so these numbers may no longer match. Run it again to update.</div>}
              <div className="rmeta">
                {b.method_id === 'power' ? 'Planning: no data used.' : `${b.cases.analyzed} of ${b.cases.total} cases analyzed`}
                {excluded > 0 && <> · excluded: {b.cases.excluded.map((e) => `${e.reason} (${e.count})`).join(', ')}</>}
              </div>
              <div className="tabpanel">
                {b.status.state === 'failed' && (
                  <div className="callout warn" role="alert" style={{ marginBottom: 14 }}>
                    <Icon name="warn" />
                    <div>
                      <b>No result to interpret.</b> {b.status.reason}
                    </div>
                  </div>
                )}
                {(b.charts ?? []).map((c) => (
                  // The SVG is drawn by the engine from numbers and escaped labels; it carries role="img" and its alt text.
                  <figure className="chartfig" key={c.id}>
                    <figcaption>{c.title}</figcaption>
                    <div className="chartsvg" dangerouslySetInnerHTML={{ __html: c.svg }} onClick={showParam} />
                    <div className="itemtools">
                      <button type="button" className="btn ghost" onClick={() => saveItem(it.id, b.title, c.id, c.title, 'png')} aria-label={`Save ${c.title} as a PNG image`}>
                        Save PNG
                      </button>
                      <button type="button" className="btn ghost" onClick={() => saveItem(it.id, b.title, c.id, c.title, 'svg')} aria-label={`Save ${c.title} as an SVG image`}>
                        Save SVG
                      </button>
                    </div>
                  </figure>
                ))}
                {b.tables.map((t) => (
                  <div key={t.id} style={{ overflowX: 'auto' }}>
                    <table className="apa">
                      <caption>{t.title}</caption>
                      <thead>
                        <tr>
                          {t.columns.map((c) => (
                            <th key={c} scope="col">
                              {c}
                            </th>
                          ))}
                        </tr>
                      </thead>
                      <tbody>
                        {t.rows.map((r, i) => (
                          <tr key={i} tabIndex={-1}>
                            {r.map((c, j) => (
                              <td key={j}>{cellText(c)}</td>
                            ))}
                          </tr>
                        ))}
                      </tbody>
                    </table>
                    {t.footnotes.length > 0 && <p className="tnote">Note. {t.footnotes.join(' ')}</p>}
                    <div className="itemtools">
                      <button type="button" className="btn ghost" onClick={() => copyTable(t)} aria-label={`Copy the ${t.title} table`}>
                        Copy table
                      </button>
                      <button type="button" className="btn ghost" onClick={() => saveItem(it.id, b.title, t.id, t.title, 'csv')} aria-label={`Save the ${t.title} table as CSV`}>
                        Save CSV
                      </button>
                    </div>
                  </div>
                ))}
                {b.diagnostics.length > 0 && (
                  <>
                    <h3 style={{ margin: '8px 0' }}>Checks</h3>
                    <div className="diaglist">
                      {b.diagnostics.map((d) => (
                        <div className="diag" key={d.id}>
                          <b>{d.title}</b>
                          <span className={`status ${STATE[d.state].cls}`}>
                            <Icon name={STATE[d.state].icon} />
                            {STATE[d.state].words}
                          </span>
                          <p>{d.message}</p>
                        </div>
                      ))}
                    </div>
                  </>
                )}
                <ExplainBlock item={it} />
                {it.paragraph && (
                  <div className="gen" style={{ marginTop: 16 }}>
                    <div className="tag">
                      <Icon name="text" />
                      Template text · numbers bound to this result
                    </div>
                    <p className="para" style={{ margin: 0 }}>
                      <Italic text={it.paragraph} />
                    </p>
                    <div style={{ marginTop: 10 }}>
                      <button type="button" className="btn" onClick={() => copy(it.paragraph!)}>
                        <Icon name="copy" />
                        Copy paragraph
                      </button>
                    </div>
                  </div>
                )}
                <AiDraftBlock item={it} ready={aiReady} onChanged={onChanged} onCopied={onCopied} />
                <p className="tnote" style={{ marginTop: 14 }}>
                  <b>Method.</b> {b.method_note}
                </p>
              </div>
            </article>
          );
        })}
      </div>
    </div>
  );
}
