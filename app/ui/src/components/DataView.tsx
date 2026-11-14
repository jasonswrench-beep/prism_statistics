// Data View: the cases as a grid. Empty cells and user-missing codes look different from values, so they're never
// mistaken for data. Value labels can be shown in place of codes. The grid is browse-only until that switch is turned
// off; then a double-click (or Enter) edits a cell, and every edit is recorded like any other change.

import { useEffect, useRef, useState } from 'react';
import { call } from '../bridge';
import type { CellValue, DatasetInfo, RowsPage, VariableInfo } from '../types';
import { Modal } from './Modal';
import { getSettings } from '../settings';

type Change = { row: number; name: string; old: string | null; new: string | null; problem: string | null };
type PastePlan = { changes: Change[]; new_cases: number; left_out: number; problems: number };

const PAGE = 500;

/** Edit › Find (Ctrl+F) or Go to Case or Variable (Ctrl+G); n changes on every request so the same one can repeat. */
export type FindRequest = { mode: 'find' | 'goto'; n: number };
type Found = { matches: { row: number; name: string }[]; total: number };

export function DataView({ dataset, onEdited, onGrew, request }: { dataset: DatasetInfo; onEdited?: () => void; onGrew?: (message: string) => void; request?: FindRequest | null }) {
  const [rows, setRows] = useState<CellValue[][]>([]);
  const [labels, setLabels] = useState(() => getSettings().valueLabels);
  const [browseOnly, setBrowseOnly] = useState(() => getSettings().browseOnly);
  const [editing, setEditing] = useState<{ row: number; col: number; text: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [focus, setFocus] = useState<{ row: number; col: number } | null>(null);
  const [pending, setPending] = useState<{ row: number; name: string; text: string; plan: PastePlan } | null>(null);
  const [applying, setApplying] = useState(false);
  // The find bar.
  const [bar, setBar] = useState<'find' | 'goto' | null>(null);
  const [query, setQuery] = useState('');
  const [within, setWithin] = useState('');
  const [whole, setWhole] = useState(false);
  const [found, setFound] = useState<Found | null>(null);
  const [at, setAt] = useState(0);
  const [caseNo, setCaseNo] = useState('');
  const [gotoVar, setGotoVar] = useState('');
  const [target, setTarget] = useState<{ row: number; col: number } | null>(null);
  const findRef = useRef<HTMLInputElement>(null);
  const gotoRef = useRef<HTMLInputElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!request) return;
    setBar(request.mode);
    window.setTimeout(() => (request.mode === 'find' ? findRef : gotoRef).current?.focus(), 0);
  }, [request]);
  useEffect(() => {
    setFound(null);
    setTarget(null);
  }, [dataset]);
  // Bring the cell being shown into view.
  useEffect(() => {
    if (!target) return;
    const el = scrollRef.current?.querySelector<HTMLElement>(`[data-cell="${target.row}-${target.col}"]`);
    el?.scrollIntoView({ block: 'center', inline: 'center' });
  }, [target, rows.length]);

  /** Loads the cases up to `row` (Data View shows them a page at a time), then shows that cell. */
  const showCell = async (row: number, col: number) => {
    try {
      let have = rows.length;
      const extra: CellValue[][] = [];
      while (have <= row) {
        const p = await call<RowsPage>('rows', { start: have, count: Math.min(2000, row - have + PAGE) });
        if (!p.rows.length) break;
        extra.push(...p.rows);
        have += p.rows.length;
      }
      if (extra.length) setRows((r) => [...r, ...extra]);
      setTarget({ row, col });
    } catch (e) {
      setError((e as Error).message);
    }
  };
  const colOf = (name: string) => dataset.variables.findIndex((v) => v.name === name);
  const runFind = async () => {
    setError(null);
    try {
      const f = await call<Found>('find', { text: query, variable: within || null, whole, labels });
      setFound(f);
      setAt(0);
      if (f.matches.length) await showCell(f.matches[0].row, colOf(f.matches[0].name));
      else setTarget(null);
    } catch (e) {
      setError((e as Error).message);
    }
  };
  const step = async (d: number) => {
    if (!found?.matches.length) return;
    const k = (at + d + found.matches.length) % found.matches.length;
    setAt(k);
    await showCell(found.matches[k].row, colOf(found.matches[k].name));
  };
  const goTo = async () => {
    setError(null);
    const n = caseNo.trim() ? Number(caseNo) : null;
    if (n !== null && (!Number.isInteger(n) || n < 1 || n > dataset.rows)) return setError(`Type a case number from 1 to ${dataset.rows}.`);
    const col = gotoVar ? colOf(gotoVar) : target?.col ?? 0;
    await showCell(n !== null ? n - 1 : target?.row ?? 0, col);
  };
  const closeBar = () => {
    setBar(null);
    setTarget(null);
  };

  useEffect(() => {
    let live = true;
    call<RowsPage>('rows', { start: 0, count: PAGE })
      .then((p) => live && setRows(p.rows))
      .catch((e: Error) => live && setError(e.message));
    return () => {
      live = false;
    };
  }, [dataset]);

  const more = async () => {
    try {
      const p = await call<RowsPage>('rows', { start: rows.length, count: PAGE });
      setRows((r) => [...r, ...p.rows]);
    } catch (e) {
      setError((e as Error).message);
    }
  };

  const raw = (c: CellValue) => (c === null ? '' : typeof c === 'object' ? String(c.missing) : String(c));
  const startEdit = (row: number, col: number) => {
    if (browseOnly) return;
    setError(null);
    setEditing({ row, col, text: raw(rows[row][col]) });
  };
  const commit = async () => {
    if (!editing) return;
    const { row, col, text } = editing;
    setEditing(null);
    if (text === raw(rows[row][col])) return;
    try {
      const p = await call<RowsPage>('set_cell', { row, name: dataset.variables[col].name, value: text });
      setRows((r) => r.map((x, i) => (i === row ? p.rows[0] : x)));
      onEdited?.();
    } catch (e) {
      setError((e as Error).message);
    }
  };

  // Paste (browse-only off): a preview of every cell it changes, then Apply.
  const onPaste = async (e: React.ClipboardEvent) => {
    if (browseOnly || editing || !focus) return;
    const text = e.clipboardData.getData('text/plain');
    if (!text) return;
    e.preventDefault();
    setError(null);
    const name = dataset.variables[focus.col].name;
    try {
      const r = await call<{ plan: PastePlan }>('paste_cells', { row: focus.row, name, text });
      setPending({ row: focus.row, name, text, plan: r.plan });
    } catch (err) {
      setError((err as Error).message);
    }
  };
  const applyPaste = async () => {
    if (!pending) return;
    setApplying(true);
    try {
      await call('paste_cells', { row: pending.row, name: pending.name, text: pending.text, apply: true });
      const p = await call<RowsPage>('rows', { start: 0, count: Math.max(rows.length, PAGE) });
      setRows(p.rows);
      const added = pending.plan.new_cases;
      setPending(null);
      if (added > 0) onGrew?.(`Pasted; ${added} case${added === 1 ? '' : 's'} added.`);
      else onEdited?.();
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setApplying(false);
    }
  };

  const show = (v: VariableInfo, c: CellValue): { text: string; cls: string } => {
    if (c === null) return { text: '', cls: 'miss' };
    if (typeof c === 'object') return { text: String(c.missing), cls: 'umiss' };
    if (typeof c === 'number') {
      if (labels) {
        const l = v.value_labels.find((x) => x.value === c);
        if (l) return { text: l.label, cls: 'lbl' };
      }
      return { text: String(c), cls: 'num' };
    }
    return { text: c, cls: '' };
  };

  return (
    <div className="gridwrap">
      <div className="gridbar">
        <label className="switch">
          <input type="checkbox" checked={labels} onChange={(e) => setLabels(e.target.checked)} />
          Show value labels
        </label>
        <label className="switch" title="Turn off to edit cells: double-click a cell, type, and press Enter; or select a cell and paste">
          <input type="checkbox" checked={browseOnly} onChange={(e) => (setBrowseOnly(e.target.checked), setEditing(null))} />
          Browse only
        </label>
        <span className="muted">
          {dataset.rows.toLocaleString()} cases · {dataset.variables.length} variables · empty cells are blank; <i>italic amber</i> values are user-missing codes
        </span>
      </div>
      {bar && (
        <div className="findbar" role="search" aria-label={bar === 'find' ? 'Find in Data View' : 'Go to case or variable'} onKeyDown={(e) => e.key === 'Escape' && closeBar()}>
          {bar === 'find' ? (
            <form
              onSubmit={(e) => {
                e.preventDefault();
                runFind();
              }}
            >
              <input ref={findRef} type="search" aria-label="Find" placeholder="Find a value" value={query} onChange={(e) => setQuery(e.target.value)} />
              <select aria-label="Look in" value={within} onChange={(e) => setWithin(e.target.value)}>
                <option value="">All variables</option>
                {dataset.variables.map((v) => (
                  <option key={v.id} value={v.name}>
                    {v.name}
                  </option>
                ))}
              </select>
              <label className="switch">
                <input type="checkbox" checked={whole} onChange={(e) => setWhole(e.target.checked)} />
                Whole cell
              </label>
              <button type="submit" className="btn">
                Find
              </button>
              {found && (
                <>
                  <span className="muted" aria-live="polite">
                    {found.total === 0 ? 'Not found' : `${at + 1} of ${found.total.toLocaleString()}${found.total > found.matches.length ? ` (the first ${found.matches.length.toLocaleString()} can be stepped through)` : ''}`}
                  </span>
                  <button type="button" className="btn" disabled={!found.matches.length} onClick={() => step(-1)} aria-label="Previous match">
                    ‹
                  </button>
                  <button type="button" className="btn" disabled={!found.matches.length} onClick={() => step(1)} aria-label="Next match">
                    ›
                  </button>
                </>
              )}
            </form>
          ) : (
            <form
              onSubmit={(e) => {
                e.preventDefault();
                goTo();
              }}
            >
              <input ref={gotoRef} inputMode="numeric" aria-label="Case number" placeholder={`Case (1 to ${dataset.rows})`} value={caseNo} onChange={(e) => setCaseNo(e.target.value)} />
              <select aria-label="Variable" value={gotoVar} onChange={(e) => setGotoVar(e.target.value)}>
                <option value="">Any variable</option>
                {dataset.variables.map((v) => (
                  <option key={v.id} value={v.name}>
                    {v.name}
                    {v.label ? ` (${v.label})` : ''}
                  </option>
                ))}
              </select>
              <button type="submit" className="btn">
                Go
              </button>
            </form>
          )}
          <button type="button" className="btn" onClick={() => setBar(bar === 'find' ? 'goto' : 'find')}>
            {bar === 'find' ? 'Go to…' : 'Find…'}
          </button>
          <button type="button" className="iconbtn" aria-label="Close" onClick={closeBar}>
            ×
          </button>
        </div>
      )}
      {error && <div className="errbox" role="alert">{error}</div>}
      <div className="gridscroll" onPaste={onPaste} ref={scrollRef}>
        <table className="dg" aria-label="Data View" aria-rowcount={dataset.rows + 1}>
          <thead>
            <tr>
              <th className="rn" scope="col">
                #
              </th>
              {dataset.variables.map((v) => (
                <th key={v.id} scope="col" title={v.label || v.name}>
                  {v.name}
                  <small>{v.label || ' '}</small>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((r, i) => (
              <tr key={i}>
                <th className="rn" scope="row">
                  {i + 1}
                </th>
                {r.map((c, j) => {
                  if (editing && editing.row === i && editing.col === j) {
                    return (
                      <td key={j} className="editing">
                        <input
                          autoFocus
                          className="cellinput"
                          aria-label={`Case ${i + 1}, ${dataset.variables[j].name}`}
                          value={editing.text}
                          onChange={(e) => setEditing({ ...editing, text: e.target.value })}
                          onBlur={commit}
                          onKeyDown={(e) => {
                            if (e.key === 'Enter') commit();
                            if (e.key === 'Escape') setEditing(null);
                          }}
                        />
                      </td>
                    );
                  }
                  const s = show(dataset.variables[j], c);
                  return (
                    <td
                      key={j}
                      data-cell={`${i}-${j}`}
                      aria-current={target && target.row === i && target.col === j ? 'true' : undefined}
                      className={s.cls + (browseOnly ? '' : ' editable') + (target && target.row === i && target.col === j ? ' found' : '')}
                      tabIndex={browseOnly ? undefined : 0}
                      onFocus={() => setFocus({ row: i, col: j })}
                      onDoubleClick={() => startEdit(i, j)}
                      onKeyDown={(e) => e.key === 'Enter' && startEdit(i, j)}
                    >
                      {s.text}
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
        {rows.length < dataset.rows && (
          <div className="loadmore">
            <button type="button" className="btn" onClick={more}>
              Show {Math.min(PAGE, dataset.rows - rows.length)} more of {(dataset.rows - rows.length).toLocaleString()} remaining
            </button>
          </div>
        )}
      </div>
      {pending && (
        <Modal
          title="Paste preview"
          onClose={() => setPending(null)}
          width={640}
          footer={
            <>
              <button type="button" className="btn" onClick={() => setPending(null)}>
                Cancel
              </button>
              <button type="button" className="btn primary" disabled={applying || pending.plan.changes.every((c) => c.problem)} onClick={applyPaste}>
                {applying ? 'Pasting…' : 'Paste'}
              </button>
            </>
          }
        >
          <p>
            {pending.plan.changes.filter((c) => !c.problem).length} cell{pending.plan.changes.filter((c) => !c.problem).length === 1 ? '' : 's'} will change
            {pending.plan.new_cases > 0 && `, adding ${pending.plan.new_cases} case${pending.plan.new_cases === 1 ? '' : 's'} at the end`}.
            {pending.plan.problems > 0 && ` ${pending.plan.problems} value${pending.plan.problems === 1 ? ' is' : 's are'} not a number for a numeric variable and will be left as ${pending.plan.problems === 1 ? 'it is' : 'they are'}.`}
            {pending.plan.left_out > 0 && ` ${pending.plan.left_out} value${pending.plan.left_out === 1 ? '' : 's'} past the last variable will be left out.`}
          </p>
          <div style={{ maxHeight: 300, overflow: 'auto' }}>
            <table className="designtable">
              <thead>
                <tr>
                  <th>Case</th>
                  <th>Variable</th>
                  <th>Now</th>
                  <th>Pasted</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {pending.plan.changes.slice(0, 200).map((c, k) => (
                  <tr key={k} className={c.problem ? 'pasteproblem' : ''}>
                    <td>{c.row + 1}</td>
                    <td>{c.name}</td>
                    <td>{c.old ?? ''}</td>
                    <td>{c.new ?? ''}</td>
                    <td className="fieldhint">{c.problem ? 'left as it is: not a number' : ''}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {pending.plan.changes.length > 200 && <p className="fieldhint">The first 200 of {pending.plan.changes.length} cells are listed.</p>}
          <p className="fieldhint">The paste is recorded like any edit, so results that used the old values are marked out of date.</p>
        </Modal>
      )}
    </div>
  );
}
