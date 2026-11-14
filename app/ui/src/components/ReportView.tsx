// The Report screen (Report Builder): which results go into the document, in what order, which of their parts, and the
// writer's own headings and text between them. The layout is saved in the project; Export writes it as Word, HTML or
// tagged PDF with the same APA formatting as the Output exports.

import { useEffect, useState } from 'react';
import { call } from '../bridge';
import type { OutputItem } from '../types';
import { Icon } from './Icon';

type Block =
  | { kind: 'heading'; text: string }
  | { kind: 'text'; text: string }
  | { kind: 'result'; id: number; charts: boolean; tables: boolean; checks: boolean; paragraph: boolean; method: boolean };

interface Report {
  title: string;
  blocks: Block[];
}

const PARTS: { key: 'charts' | 'tables' | 'checks' | 'paragraph' | 'method'; label: string }[] = [
  { key: 'charts', label: 'Figures' },
  { key: 'tables', label: 'Tables' },
  { key: 'checks', label: 'Checks' },
  { key: 'paragraph', label: 'Paragraph' },
  { key: 'method', label: 'Method note' },
];

export function ReportView({ results, onExport }: { results: OutputItem[]; onExport: (format: 'docx' | 'html' | 'pdf') => void }) {
  const [report, setReport] = useState<Report | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [adding, setAdding] = useState('');

  useEffect(() => {
    call<Report>('report_get', {})
      .then(setReport)
      .catch((e: Error) => setError(e.message));
  }, [results]);

  const save = async (r: Report) => {
    setReport(r);
    try {
      await call('report_set', r);
    } catch (e) {
      setError((e as Error).message);
    }
  };

  if (!report) return <div className="reportview">{error && <p className="errbox">{error}</p>}</div>;
  const byId = new Map(results.map((r) => [r.id, r]));
  const included = new Set(report.blocks.flatMap((b) => (b.kind === 'result' ? [b.id] : [])));
  const left = results.filter((r) => !included.has(r.id));
  const update = (i: number, b: Block) => save({ ...report, blocks: report.blocks.map((x, k) => (k === i ? b : x)) });
  const move = (i: number, by: number) => {
    const j = i + by;
    if (j < 0 || j >= report.blocks.length) return;
    const blocks = [...report.blocks];
    [blocks[i], blocks[j]] = [blocks[j], blocks[i]];
    save({ ...report, blocks });
  };
  const remove = (i: number) => save({ ...report, blocks: report.blocks.filter((_, k) => k !== i) });
  const add = (b: Block) => save({ ...report, blocks: [...report.blocks, b] });

  return (
    <div className="reportview">
      <div className="reportbar">
        <label className="reporttitle">
          <span>Report title</span>
          <input className="textinput" value={report.title} onChange={(e) => setReport({ ...report, title: e.target.value })} onBlur={() => save(report)} />
        </label>
        <button type="button" className="btn primary" onClick={() => onExport('docx')}>
          <Icon name="text" />
          Export to Word
        </button>
        <button type="button" className="btn" onClick={() => onExport('html')}>
          Export to HTML
        </button>
        <button type="button" className="btn" onClick={() => onExport('pdf')}>
          Export to PDF
        </button>
      </div>
      <p className="fieldhint" style={{ margin: '0 0 10px' }}>
        Arrange the report: move blocks up or down, leave out parts of a result, and add your own headings and text (write *word* for italics). It is saved with the project. Tables and figures are numbered in order when exported.
      </p>
      {error && <p className="errbox">{error}</p>}
      <ol className="reportblocks">
        {report.blocks.map((b, i) => (
          <li key={i} className={`rblock ${b.kind}`}>
            <div className="rbtools">
              <button type="button" className="iconbtn" aria-label="Move up" disabled={i === 0} onClick={() => move(i, -1)}>
                ↑
              </button>
              <button type="button" className="iconbtn" aria-label="Move down" disabled={i === report.blocks.length - 1} onClick={() => move(i, 1)}>
                ↓
              </button>
              <button type="button" className="iconbtn" aria-label="Remove from the report" onClick={() => remove(i)}>
                <Icon name="close" />
              </button>
            </div>
            {b.kind === 'heading' && (
              <input className="textinput rbheading" aria-label="Heading" value={b.text} placeholder="Heading" onChange={(e) => setReport({ ...report, blocks: report.blocks.map((x, k) => (k === i ? { ...b, text: e.target.value } : x)) })} onBlur={() => save(report)} />
            )}
            {b.kind === 'text' && (
              <textarea className="rbtext" aria-label="Text" rows={3} value={b.text} placeholder="Your text" onChange={(e) => setReport({ ...report, blocks: report.blocks.map((x, k) => (k === i ? { ...b, text: e.target.value } : x)) })} onBlur={() => save(report)} />
            )}
            {b.kind === 'result' && (
              <div className="rbresult">
                <b>{byId.get(b.id)?.bundle.title ?? `Result ${b.id}`}</b>
                {byId.get(b.id)?.stale && <span className="status review">Out of date</span>}
                <div className="rbparts">
                  {PARTS.filter((p) => p.key !== 'charts' || (byId.get(b.id)?.bundle.charts?.length ?? 0) > 0).map((p) => (
                    <label key={p.key} className="check">
                      <input type="checkbox" checked={b[p.key]} onChange={(e) => update(i, { ...b, [p.key]: e.target.checked })} />
                      <span>{p.label}</span>
                    </label>
                  ))}
                </div>
              </div>
            )}
          </li>
        ))}
      </ol>
      {!report.blocks.length && <p className="muted">The report is empty. Add results, headings and text below.</p>}
      <div className="reportadd">
        <button type="button" className="btn" onClick={() => add({ kind: 'heading', text: '' })}>
          Add heading
        </button>
        <button type="button" className="btn" onClick={() => add({ kind: 'text', text: '' })}>
          Add text
        </button>
        {left.length > 0 && (
          <>
            <select className="textinput" aria-label="Result to add" value={adding} onChange={(e) => setAdding(e.target.value)}>
              <option value="">Choose a result to add…</option>
              {left.map((r) => (
                <option key={r.id} value={r.id}>
                  {r.bundle.title}
                </option>
              ))}
            </select>
            <button type="button" className="btn" disabled={!adding} onClick={() => (add({ kind: 'result', id: Number(adding), charts: true, tables: true, checks: true, paragraph: true, method: true }), setAdding(''))}>
              Add result
            </button>
          </>
        )}
      </div>
    </div>
  );
}
