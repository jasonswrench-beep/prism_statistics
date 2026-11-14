// Transform › Compute Variable…, Recode…, and Scale Score…: changes that make new variables from old ones. Each is
// recorded like any other change, so results that used the old values are marked out of date.

import { useMemo, useState } from 'react';
import { call } from '../bridge';
import type { DatasetInfo } from '../types';
import { Modal } from './Modal';

export type Done = (message: string) => void;
type Report = { report: { names: string[]; valid: number; missing: number; unreadable?: [number, string] | null } };

const said = (r: Report) =>
  `${r.report.names.join(', ')}: ${r.report.valid} cases with a value, ${r.report.missing} missing.${r.report.unreadable ? ` ${r.report.unreadable[0]} value${r.report.unreadable[0] === 1 ? '' : 's'} couldn't be read as a date, such as "${r.report.unreadable[1]}"; ${r.report.unreadable[0] === 1 ? 'it was' : 'they were'} left missing.` : ''}`;

export function Footer({ onClose, onRun, disabled, running, action }: { onClose: () => void; onRun: () => void; disabled: boolean; running: boolean; action: string }) {
  return (
    <>
      <button type="button" className="btn" onClick={onClose}>
        Cancel
      </button>
      <button type="button" className="btn primary" disabled={disabled || running} onClick={onRun}>
        {running ? 'Working…' : action}
      </button>
    </>
  );
}

/** A list of variables to tick. */
export function Pick({ dataset, chosen, onChange, numericOnly, label }: { dataset: DatasetInfo; chosen: string[]; onChange: (v: string[]) => void; numericOnly?: boolean; label: string }) {
  const vars = dataset.variables.filter((v) => !numericOnly || v.type === 'numeric');
  return (
    <fieldset className="pickbox" style={{ maxHeight: 180, overflowY: 'auto', border: '1px solid var(--line, #ccc)', padding: 8 }}>
      <legend className="fieldlabel">{label}</legend>
      {vars.map((v) => (
        <label key={v.name} className="check" style={{ display: 'block' }}>
          <input type="checkbox" checked={chosen.includes(v.name)} onChange={(e) => onChange(e.target.checked ? [...chosen, v.name] : chosen.filter((c) => c !== v.name))} /> {v.name}
          {v.label ? <span className="fieldhint"> {v.label}</span> : null}
        </label>
      ))}
    </fieldset>
  );
}

export function useRun(onDone: Done, onClose: () => void) {
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const run = async (command: string, args: unknown) => {
    setRunning(true);
    setError(null);
    try {
      const r = await call<Report>(command, args);
      onDone(said(r));
      onClose();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setRunning(false);
    }
  };
  return { error, running, run };
}

export function ComputeDialog({ dataset, onClose, onDone, onPaste }: { dataset: DatasetInfo; onClose: () => void; onDone: Done; onPaste?: (command: string, args: unknown) => void }) {
  const [target, setTarget] = useState('');
  const [formula, setFormula] = useState('');
  const [label, setLabel] = useState('');
  const [replace, setReplace] = useState(false);
  const { error, running, run } = useRun(onDone, onClose);
  const exists = dataset.variables.some((v) => v.name === target.trim());
  const args = { target: target.trim(), formula, label: label || null, replace };
  return (
    <Modal
      title="Compute Variable"
      onClose={onClose}
      width={640}
      footer={
        <>
          {onPaste && (
            <button type="button" className="btn" disabled={!target.trim() || !formula.trim()} onClick={() => onPaste('compute_variable', args)} title="Add this change to the script without making it">
              Paste
            </button>
          )}
          <Footer onClose={onClose} onRun={() => run('compute_variable', args)} disabled={!target.trim() || !formula.trim() || (exists && !replace)} running={running} action={exists ? 'Replace values' : 'Compute'} />
        </>
      }
    >
      <label className="fieldlabel" htmlFor="cTarget">
        Target variable
      </label>
      <input id="cTarget" className="textinput" autoFocus value={target} onChange={(e) => setTarget(e.target.value)} />
      {exists && (
        <label className="check">
          <input type="checkbox" checked={replace} onChange={(e) => setReplace(e.target.checked)} /> {target.trim()} exists: replace its values
        </label>
      )}
      <label className="fieldlabel" htmlFor="cFormula">
        Formula <span className="fieldhint">R’s syntax: (pre + post) / 2, log(income + 1), ifelse(age &gt;= 18, 1, 0), rowmean(i1, i2, i3)</span>
      </label>
      <textarea id="cFormula" className="textinput" rows={3} value={formula} onChange={(e) => setFormula(e.target.value)} style={{ fontFamily: 'var(--mono, monospace)' }} />
      <div className="fieldhint">
        Variables: {dataset.variables.map((v) => v.name).join(', ')}. User-missing codes count as missing; a missing value makes the result missing. True and false become 1 and 0.
      </div>
      <label className="fieldlabel" htmlFor="cLabel">
        Label <span className="fieldhint">optional</span>
      </label>
      <input id="cLabel" className="textinput" value={label} onChange={(e) => setLabel(e.target.value)} />
      {error && <div className="errbox" role="alert">{error}</div>}
    </Modal>
  );
}

type FromKind = 'value' | 'range' | 'missing' | 'else';
type ToKind = 'value' | 'missing' | 'copy';
type RuleRow = { from: FromKind; a: string; b: string; to: ToKind; v: string };

export function RecodeDialog({ dataset, onClose, onDone, onPaste }: { dataset: DatasetInfo; onClose: () => void; onDone: Done; onPaste?: (command: string, args: unknown) => void }) {
  const [vars, setVars] = useState<string[]>([]);
  const [intoNew, setIntoNew] = useState(true);
  const [names, setNames] = useState('');
  const [rules, setRules] = useState<RuleRow[]>([{ from: 'value', a: '', b: '', to: 'value', v: '' }]);
  const { error, running, run } = useRun(onDone, onClose);
  const into = names.split(/[,\s]+/).filter(Boolean);
  const num = (s: string) => (s.trim() === '' ? null : Number(s));
  const problem = useMemo(() => {
    if (!vars.length) return 'Choose at least one variable.';
    if (intoNew && into.length !== vars.length) return `Give ${vars.length} new name${vars.length === 1 ? '' : 's'}, one for each variable.`;
    for (const r of rules) {
      if (r.from === 'value' && !Number.isFinite(num(r.a) ?? NaN)) return 'Each old value must be a number.';
      if (r.from === 'range' && num(r.a) === null && num(r.b) === null) return 'A range needs at least one end.';
      if (r.to === 'value' && !Number.isFinite(num(r.v) ?? NaN)) return 'Each new value must be a number.';
    }
    return null;
  }, [vars, intoNew, into.length, rules]);
  const args = {
    variables: vars,
    into: intoNew ? into : null,
    rules: rules.map((r) => ({
      from: r.from === 'value' ? { kind: 'value', value: num(r.a) } : r.from === 'range' ? { kind: 'range', lo: num(r.a), hi: num(r.b) } : { kind: r.from },
      to: r.to === 'value' ? { kind: 'value', value: num(r.v) } : { kind: r.to },
    })),
  };
  const set = (i: number, patch: Partial<RuleRow>) => setRules((rs) => rs.map((r, j) => (j === i ? { ...r, ...patch } : r)));
  return (
    <Modal
      title="Recode"
      onClose={onClose}
      width={700}
      footer={
        <>
          {onPaste && (
            <button type="button" className="btn" disabled={!!problem} onClick={() => onPaste('recode', args)}>
              Paste
            </button>
          )}
          <Footer onClose={onClose} onRun={() => run('recode', args)} disabled={!!problem} running={running} action="Recode" />
        </>
      }
    >
      <Pick dataset={dataset} chosen={vars} onChange={setVars} numericOnly label="Numeric variables" />
      <div role="radiogroup" aria-label="Where the new values go">
        <label className="radio">
          <input type="radio" checked={intoNew} onChange={() => setIntoNew(true)} /> Into new variables (default): values no rule matches are left empty
        </label>
        <label className="radio">
          <input type="radio" checked={!intoNew} onChange={() => setIntoNew(false)} /> Into the same variables: values no rule matches stay as they are
        </label>
      </div>
      {intoNew && (
        <>
          <label className="fieldlabel" htmlFor="rNames">
            New names <span className="fieldhint">one per variable, in order, separated by commas</span>
          </label>
          <input id="rNames" className="textinput" value={names} onChange={(e) => setNames(e.target.value)} />
        </>
      )}
      <div className="fieldlabel">Rules, applied in order (the first that matches a value wins)</div>
      {rules.map((r, i) => (
        <div key={i} style={{ display: 'flex', gap: 6, alignItems: 'center', flexWrap: 'wrap', marginBottom: 4 }}>
          <select aria-label={`Rule ${i + 1}: old`} value={r.from} onChange={(e) => set(i, { from: e.target.value as FromKind })}>
            <option value="value">Value</option>
            <option value="range">Range</option>
            <option value="missing">Missing</option>
            <option value="else">All other values</option>
          </select>
          {r.from === 'value' && <input aria-label="Old value" className="textinput" style={{ width: 80 }} value={r.a} onChange={(e) => set(i, { a: e.target.value })} />}
          {r.from === 'range' && (
            <>
              <input aria-label="From (empty: lowest)" placeholder="lowest" className="textinput" style={{ width: 80 }} value={r.a} onChange={(e) => set(i, { a: e.target.value })} /> to
              <input aria-label="To (empty: highest)" placeholder="highest" className="textinput" style={{ width: 80 }} value={r.b} onChange={(e) => set(i, { b: e.target.value })} />
            </>
          )}
          →
          <select aria-label={`Rule ${i + 1}: new`} value={r.to} onChange={(e) => set(i, { to: e.target.value as ToKind })}>
            <option value="value">Value</option>
            <option value="missing">Missing (empty)</option>
            <option value="copy">Same value</option>
          </select>
          {r.to === 'value' && <input aria-label="New value" className="textinput" style={{ width: 80 }} value={r.v} onChange={(e) => set(i, { v: e.target.value })} />}
          <button type="button" className="btn" aria-label={`Remove rule ${i + 1}`} disabled={rules.length === 1} onClick={() => setRules((rs) => rs.filter((_, j) => j !== i))}>
            Remove
          </button>
        </div>
      ))}
      <button type="button" className="btn" onClick={() => setRules((rs) => [...rs, { from: 'value', a: '', b: '', to: 'value', v: '' }])}>
        Add rule
      </button>
      {problem && <div className="fieldhint">{problem}</div>}
      {error && <div className="errbox" role="alert">{error}</div>}
    </Modal>
  );
}

export function ScaleDialog({ dataset, onClose, onDone, onPaste }: { dataset: DatasetInfo; onClose: () => void; onDone: Done; onPaste?: (command: string, args: unknown) => void }) {
  const [target, setTarget] = useState('');
  const [items, setItems] = useState<string[]>([]);
  const [reverse, setReverse] = useState<string[]>([]);
  const [method, setMethod] = useState<'mean' | 'sum' | 'prorated_sum'>('mean');
  const [lo, setLo] = useState('1');
  const [hi, setHi] = useState('5');
  const [minItems, setMinItems] = useState('');
  const { error, running, run } = useRun(onDone, onClose);
  // Items in the data's order, whatever order they were ticked in.
  const ordered = dataset.variables.map((v) => v.name).filter((n) => items.includes(n));
  const rev = ordered.filter((r) => reverse.includes(r));
  const args = {
    target: target.trim(),
    items: ordered,
    method,
    reverse: rev,
    scale_min: rev.length ? Number(lo) : null,
    scale_max: rev.length ? Number(hi) : null,
    min_items: minItems.trim() ? Number(minItems) : null,
  };
  const problem = !target.trim() ? 'Name the new variable.' : items.length < 2 ? 'Choose at least two items.' : rev.length && !(Number.isFinite(Number(lo)) && Number.isFinite(Number(hi))) ? 'Give the lowest and highest possible answers.' : null;
  return (
    <Modal
      title="Scale Score"
      onClose={onClose}
      width={640}
      footer={
        <>
          {onPaste && (
            <button type="button" className="btn" disabled={!!problem} onClick={() => onPaste('scale_score', args)}>
              Paste
            </button>
          )}
          <Footer onClose={onClose} onRun={() => run('scale_score', args)} disabled={!!problem} running={running} action="Score" />
        </>
      }
    >
      <label className="fieldlabel" htmlFor="sTarget">
        New variable
      </label>
      <input id="sTarget" className="textinput" autoFocus value={target} onChange={(e) => setTarget(e.target.value)} />
      <Pick dataset={dataset} chosen={items} onChange={setItems} numericOnly label="Items" />
      {items.length > 0 && (
        <fieldset style={{ border: 0, padding: 0 }}>
          <legend className="fieldlabel">Reverse-scored items</legend>
          {ordered.map((i) => (
            <label key={i} className="check" style={{ marginRight: 12 }}>
              <input type="checkbox" checked={reverse.includes(i)} onChange={(e) => setReverse((r) => (e.target.checked ? [...r, i] : r.filter((x) => x !== i)))} /> {i}
            </label>
          ))}
        </fieldset>
      )}
      {rev.length > 0 && (
        <div>
          Possible answers from <input aria-label="Lowest possible answer" className="textinput" style={{ width: 60 }} value={lo} onChange={(e) => setLo(e.target.value)} /> to{' '}
          <input aria-label="Highest possible answer" className="textinput" style={{ width: 60 }} value={hi} onChange={(e) => setHi(e.target.value)} />
          <span className="fieldhint"> a reversed answer becomes lowest + highest − answer</span>
        </div>
      )}
      <div role="radiogroup" aria-label="Score">
        <label className="radio">
          <input type="radio" checked={method === 'mean'} onChange={() => setMethod('mean')} /> Mean of the answered items (default)
        </label>
        <label className="radio">
          <input type="radio" checked={method === 'sum'} onChange={() => setMethod('sum')} /> Sum of the answered items
        </label>
        <label className="radio">
          <input type="radio" checked={method === 'prorated_sum'} onChange={() => setMethod('prorated_sum')} /> Prorated sum: the mean times the number of items
        </label>
      </div>
      <label className="fieldlabel" htmlFor="sMin">
        Fewest items answered <span className="fieldhint">empty: every item; fewer answered gives a missing score</span>
      </label>
      <input id="sMin" className="textinput" style={{ width: 80 }} value={minItems} onChange={(e) => setMinItems(e.target.value)} />
      {problem && <div className="fieldhint">{problem}</div>}
      {error && <div className="errbox" role="alert">{error}</div>}
    </Modal>
  );
}
