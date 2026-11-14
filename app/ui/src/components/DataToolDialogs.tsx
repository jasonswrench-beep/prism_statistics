// Data › Merge Files (Add Cases…, Add Variables…), Data › Aggregate…, and Data › Restructure…: changes to the shape of
// the data. Each is recorded like any other change, so results that used the old data are marked out of date.

import { useState } from 'react';
import { call } from '../bridge';
import { fileToBase64 } from '../files';
import type { DatasetInfo } from '../types';
import { Modal } from './Modal';

type Done = (message: string) => void;
type Paste = (command: string, args: unknown) => void;
type OtherVar = { name: string; label: string; type: string };
type Other = { file_name: string; rows: number; variables: OtherVar[]; base64: string };

function useRun(onDone: Done, onClose: () => void, say: (r: { report: Record<string, unknown>; dataset: { rows: number } }) => string) {
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const run = async (command: string, args: unknown) => {
    setRunning(true);
    setError(null);
    try {
      const r = await call<{ report: Record<string, unknown>; dataset: { rows: number } }>(command, args);
      onDone(say(r));
      onClose();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setRunning(false);
    }
  };
  return { error, setError, running, run };
}

function Footer({ onClose, onRun, onPaste, disabled, running, action }: { onClose: () => void; onRun: () => void; onPaste?: () => void; disabled: boolean; running: boolean; action: string }) {
  return (
    <>
      {onPaste && (
        <button type="button" className="btn" disabled={disabled} onClick={onPaste} title="Add this change to the script without making it">
          Paste
        </button>
      )}
      <button type="button" className="btn" onClick={onClose}>
        Cancel
      </button>
      <button type="button" className="btn primary" disabled={disabled || running} onClick={onRun}>
        {running ? 'Working…' : action}
      </button>
    </>
  );
}

/** Choosing the other file: reads it and lists its variables. */
function OtherFile({ other, setOther, setError }: { other: Other | null; setOther: (o: Other) => void; setError: (e: string | null) => void }) {
  const [reading, setReading] = useState(false);
  const choose = async (f: File | undefined) => {
    if (!f) return;
    setReading(true);
    setError(null);
    try {
      const base64 = await fileToBase64(f);
      const r = await call<Omit<Other, 'base64'>>('inspect_file', { file_name: f.name, base64 });
      setOther({ ...r, base64 });
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setReading(false);
    }
  };
  return (
    <div>
      <label className="fieldlabel" htmlFor="otherFile">
        The other data file <span className="fieldhint">CSV, .xlsx, .sav, .dta, .sas7bdat or .jasp</span>
      </label>
      <input id="otherFile" type="file" accept=".csv,.tsv,.txt,.xlsx,.xlsm,.sav,.zsav,.dta,.sas7bdat,.jasp" onChange={(e) => choose(e.target.files?.[0])} />
      {reading && <div className="fieldhint">Reading…</div>}
      {other && (
        <div className="fieldhint">
          {other.file_name}: {other.rows} cases, {other.variables.length} variables.
        </div>
      )}
    </div>
  );
}

const lower = (s: string) => s.toLowerCase();

export function AddCasesDialog({ dataset, onClose, onDone }: { dataset: DatasetInfo; onClose: () => void; onDone: Done }) {
  const [other, setOther] = useState<Other | null>(null);
  const [source, setSource] = useState('');
  const { error, setError, running, run } = useRun(onDone, onClose, (r) => `Appended ${r.report.added_cases} cases: the data now has ${r.dataset.rows}.`);
  const here = dataset.variables.map((v) => lower(v.name));
  const there = other?.variables.map((v) => lower(v.name)) ?? [];
  const both = dataset.variables.filter((v) => there.includes(lower(v.name)));
  const onlyThere = other?.variables.filter((v) => !here.includes(lower(v.name))) ?? [];
  const onlyHere = dataset.variables.filter((v) => !there.includes(lower(v.name)));
  const clash = both.filter((v) => other?.variables.find((o) => lower(o.name) === lower(v.name))?.type !== v.type);
  return (
    <Modal
      title="Add Cases"
      onClose={onClose}
      width={640}
      footer={<Footer onClose={onClose} onRun={() => other && run('append_cases', { file_name: other.file_name, base64: other.base64, source: source.trim() || null })} disabled={!other || clash.length > 0} running={running} action="Add cases" />}
    >
      <p className="fieldhint">Adds another file’s cases below these, matching variables by name.</p>
      <OtherFile other={other} setOther={setOther} setError={setError} />
      {other && (
        <div className="fieldhint" style={{ marginTop: 8 }}>
          <div>
            <b>In both ({both.length}):</b> {both.map((v) => v.name).join(', ') || 'none'}
          </div>
          <div>
            <b>Only in {other.file_name} ({onlyThere.length}):</b> {onlyThere.map((v) => v.name).join(', ') || 'none'}: added, missing for these cases
          </div>
          <div>
            <b>Only here ({onlyHere.length}):</b> {onlyHere.map((v) => v.name).join(', ') || 'none'}: missing for the new cases
          </div>
          {clash.length > 0 && <div className="errbox">{clash.map((v) => v.name).join(', ')} hold numbers in one file and text in the other; make them the same type first.</div>}
        </div>
      )}
      <label className="fieldlabel" htmlFor="srcVar">
        Record the source file in a new variable <span className="fieldhint">optional: 1 for these cases, 2 for the new ones</span>
      </label>
      <input id="srcVar" className="textinput" placeholder="source" value={source} onChange={(e) => setSource(e.target.value)} />
      {error && <div className="errbox" role="alert">{error}</div>}
    </Modal>
  );
}

export function AddVariablesDialog({ dataset, onClose, onDone }: { dataset: DatasetInfo; onClose: () => void; onDone: Done }) {
  const [other, setOther] = useState<Other | null>(null);
  const [keys, setKeys] = useState<string[]>([]);
  const [join, setJoin] = useState<'left' | 'inner' | 'all'>('left');
  const { error, setError, running, run } = useRun(onDone, onClose, (r) => {
    const rep = r.report as { matched: number; unmatched_here: number; unmatched_other: number; added_variables: string[]; renamed: [string, string][] };
    const ren = rep.renamed.length ? ` Renamed: ${rep.renamed.map(([a, b]) => `${a} → ${b}`).join(', ')}.` : '';
    const k = rep.added_variables.length;
    return `Added ${k} variable${k === 1 ? '' : 's'}; ${rep.matched} case${rep.matched === 1 ? '' : 's'} matched, ${rep.unmatched_here} here and ${rep.unmatched_other} there had no match. The data now has ${r.dataset.rows} cases.${ren}`;
  });
  const there = other?.variables.map((v) => lower(v.name)) ?? [];
  const candidates = dataset.variables.filter((v) => there.includes(lower(v.name)));
  return (
    <Modal
      title="Add Variables (Merge)"
      onClose={onClose}
      width={640}
      footer={<Footer onClose={onClose} onRun={() => other && run('merge_variables', { file_name: other.file_name, base64: other.base64, keys, join })} disabled={!other || keys.length === 0} running={running} action="Merge" />}
    >
      <p className="fieldhint">Adds another file’s variables, matching cases on key variables that both files share (such as an ID). A key can repeat in one file (one-to-many) but not in both.</p>
      <OtherFile other={other} setOther={setOther} setError={setError} />
      {other && (
        <fieldset className="pickbox" style={{ maxHeight: 160, overflowY: 'auto', border: '1px solid var(--line, #ccc)', padding: 8, marginTop: 8 }}>
          <legend className="fieldlabel">Key variables (in both files)</legend>
          {candidates.length === 0 && <div className="fieldhint">The files share no variable names.</div>}
          {candidates.map((v) => (
            <label key={v.name} className="check" style={{ display: 'block' }}>
              <input type="checkbox" checked={keys.includes(v.name)} onChange={(e) => setKeys(e.target.checked ? [...keys, v.name] : keys.filter((k) => k !== v.name))} /> {v.name}
            </label>
          ))}
        </fieldset>
      )}
      <fieldset className="cfaest">
        <legend>Cases to keep</legend>
        {(
          [
            ['left', 'Every case in this file (default)', 'unmatched cases get missing values for the new variables'],
            ['inner', 'Only cases with a match', ''],
            ['all', 'Every case in both files', 'the other file’s unmatched cases are added too'],
          ] as const
        ).map(([v, l, h]) => (
          <label key={v} className="radio">
            <input type="radio" name="join" checked={join === v} onChange={() => setJoin(v)} />
            <span>
              {l} {h && <span className="fieldhint">{h}</span>}
            </span>
          </label>
        ))}
      </fieldset>
      {error && <div className="errbox" role="alert">{error}</div>}
    </Modal>
  );
}

const FUNS = [
  ['mean', 'Mean'],
  ['sum', 'Sum'],
  ['median', 'Median'],
  ['min', 'Minimum'],
  ['max', 'Maximum'],
  ['sd', 'Standard deviation'],
  ['n', 'Number of valid values'],
  ['nmiss', 'Number missing'],
  ['cases', 'Number of cases'],
  ['first', 'First value'],
  ['last', 'Last value'],
] as const;

type SumRow = { source: string; fun: string; target: string };

export function AggregateDialog({ dataset, onClose, onDone, onPaste }: { dataset: DatasetInfo; onClose: () => void; onDone: Done; onPaste?: Paste }) {
  const [breaks, setBreaks] = useState<string[]>([]);
  const numeric = dataset.variables.filter((v) => v.type === 'numeric');
  const [rows, setRows] = useState<SumRow[]>([{ source: numeric[0]?.name ?? '', fun: 'mean', target: '' }]);
  const [add, setAdd] = useState(false);
  const [missingGroups, setMissingGroups] = useState(true);
  const { error, running, run } = useRun(onDone, onClose, (r) => {
    const rep = r.report as { groups: number; names: string[] };
    return add ? `Added ${rep.names.join(', ')} to every case (${rep.groups} groups).` : `The data is now ${rep.groups} groups, one row each.`;
  });
  const name = (r: SumRow) => r.target.trim() || (r.fun === 'cases' ? 'cases' : `${r.source}_${r.fun}`);
  const args = { breaks, summaries: rows.map((r) => ({ source: r.fun === 'cases' ? null : r.source, function: r.fun, target: name(r) })), add_to_cases: add, missing_groups: missingGroups };
  const set = (i: number, k: keyof SumRow, v: string) => setRows((rs) => rs.map((r, j) => (j === i ? { ...r, [k]: v } : r)));
  return (
    <Modal title="Aggregate" onClose={onClose} width={700} footer={<Footer onClose={onClose} onRun={() => run('aggregate', args)} onPaste={onPaste ? () => onPaste('aggregate', args) : undefined} disabled={breaks.length === 0 || rows.length === 0} running={running} action="Aggregate" />}>
      <fieldset className="pickbox" style={{ maxHeight: 150, overflowY: 'auto', border: '1px solid var(--line, #ccc)', padding: 8 }}>
        <legend className="fieldlabel">Break variables (groups)</legend>
        {dataset.variables.map((v) => (
          <label key={v.name} className="check" style={{ display: 'block' }}>
            <input type="checkbox" checked={breaks.includes(v.name)} onChange={(e) => setBreaks(e.target.checked ? [...breaks, v.name] : breaks.filter((b) => b !== v.name))} /> {v.name}
            {v.label ? <span className="fieldhint"> {v.label}</span> : null}
          </label>
        ))}
      </fieldset>
      <div className="fieldlabel" style={{ marginTop: 8 }}>
        Summaries
      </div>
      <table className="designtable">
        <thead>
          <tr>
            <th>Function</th>
            <th>Of variable</th>
            <th>New name</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {rows.map((r, i) => (
            <tr key={i}>
              <td>
                <select className="textinput" aria-label={`Function ${i + 1}`} value={r.fun} onChange={(e) => set(i, 'fun', e.target.value)}>
                  {FUNS.map(([v, l]) => (
                    <option key={v} value={v}>
                      {l}
                    </option>
                  ))}
                </select>
              </td>
              <td>
                {r.fun !== 'cases' && (
                  <select className="textinput" aria-label={`Variable ${i + 1}`} value={r.source} onChange={(e) => set(i, 'source', e.target.value)}>
                    {numeric.map((v) => (
                      <option key={v.name} value={v.name}>
                        {v.name}
                      </option>
                    ))}
                  </select>
                )}
              </td>
              <td>
                <input className="textinput" aria-label={`New name ${i + 1}`} placeholder={name({ ...r, target: '' })} value={r.target} onChange={(e) => set(i, 'target', e.target.value)} />
              </td>
              <td>
                <button type="button" className="btn" aria-label={`Remove summary ${i + 1}`} onClick={() => setRows((rs) => rs.filter((_, j) => j !== i))}>
                  ×
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <button type="button" className="btn" onClick={() => setRows((rs) => [...rs, { source: numeric[0]?.name ?? '', fun: 'mean', target: '' }])}>
        Add a summary
      </button>
      <fieldset className="cfaest">
        <legend>Result</legend>
        <label className="radio">
          <input type="radio" name="aggmode" checked={!add} onChange={() => setAdd(false)} />
          <span>
            A new dataset with one row per group <span className="fieldhint">replaces the data shown; save first if you need it</span>
          </span>
        </label>
        <label className="radio">
          <input type="radio" name="aggmode" checked={add} onChange={() => setAdd(true)} />
          <span>Add the summaries to every case</span>
        </label>
      </fieldset>
      <label className="check">
        <input type="checkbox" checked={missingGroups} onChange={(e) => setMissingGroups(e.target.checked)} /> Cases with a missing break value form a group of their own
      </label>
      <div className="fieldhint">User-missing codes count as missing.</div>
      {error && <div className="errbox" role="alert">{error}</div>}
    </Modal>
  );
}

export function RestructureDialog({ dataset, onClose, onDone, onPaste }: { dataset: DatasetInfo; onClose: () => void; onDone: Done; onPaste?: Paste }) {
  const [mode, setMode] = useState<'long' | 'wide'>('long');
  const [stacks, setStacks] = useState<{ target: string; variables: string[] }[]>([{ target: '', variables: [] }]);
  const [index, setIndex] = useState('index');
  const [idVar, setIdVar] = useState('');
  const [newId, setNewId] = useState('id');
  const [dropMissing, setDropMissing] = useState(false);
  const [ids, setIds] = useState<string[]>([]);
  const [wIndex, setWIndex] = useState('');
  const [spread, setSpread] = useState<string[]>([]);
  const { error, running, run } = useRun(onDone, onClose, (r) => {
    const rep = r.report as { rows: number; variables: number; dropped: string[] };
    return `Restructured: ${rep.rows} rows and ${rep.variables} variables.${rep.dropped.length ? ` Left out: ${rep.dropped.join(', ')}.` : ''}`;
  });
  const longArgs = { stacks: stacks.map((s) => ({ target: s.target.trim(), variables: s.variables })), index: index.trim(), id: idVar || newId.trim() || null, drop_missing: dropMissing };
  const wideArgs = { id: ids, index: wIndex, variables: spread };
  const k = stacks[0]?.variables.length ?? 0;
  const longOk = stacks.every((s) => s.target.trim() && s.variables.length >= 2 && s.variables.length === k) && index.trim() !== '';
  const wideOk = ids.length > 0 && wIndex !== '';
  const toggle = (list: string[], v: string, on: boolean) => (on ? [...list, v] : list.filter((x) => x !== v));
  const box = (title: string, chosen: string[], set: (v: string[]) => void, exclude: string[] = []) => (
    <fieldset className="pickbox" style={{ maxHeight: 150, overflowY: 'auto', border: '1px solid var(--line, #ccc)', padding: 8 }}>
      <legend className="fieldlabel">{title}</legend>
      {dataset.variables
        .filter((v) => !exclude.includes(v.name))
        .map((v) => (
          <label key={v.name} className="check" style={{ display: 'block' }}>
            <input type="checkbox" checked={chosen.includes(v.name)} onChange={(e) => set(toggle(chosen, v.name, e.target.checked))} /> {v.name}
            {chosen.includes(v.name) && mode === 'long' ? <span className="fieldhint"> (position {chosen.indexOf(v.name) + 1})</span> : null}
          </label>
        ))}
    </fieldset>
  );
  return (
    <Modal
      title="Restructure"
      onClose={onClose}
      width={700}
      footer={
        <Footer
          onClose={onClose}
          onRun={() => (mode === 'long' ? run('restructure_long', longArgs) : run('restructure_wide', wideArgs))}
          onPaste={onPaste ? () => (mode === 'long' ? onPaste('restructure_long', longArgs) : onPaste('restructure_wide', wideArgs)) : undefined}
          disabled={mode === 'long' ? !longOk : !wideOk}
          running={running}
          action="Restructure"
        />
      }
    >
      <fieldset className="cfaest">
        <legend>Direction</legend>
        <label className="radio">
          <input type="radio" name="rmode" checked={mode === 'long'} onChange={() => setMode('long')} />
          <span>
            Wide to long <span className="fieldhint">variables such as score1, score2, score3 become rows: one per case and position</span>
          </span>
        </label>
        <label className="radio">
          <input type="radio" name="rmode" checked={mode === 'wide'} onChange={() => setMode('wide')} />
          <span>
            Long to wide <span className="fieldhint">one row per identifier, a variable per index value</span>
          </span>
        </label>
      </fieldset>
      {mode === 'long' ? (
        <>
          {stacks.map((s, i) => (
            <div key={i} style={{ marginBottom: 8 }}>
              <label className="fieldlabel" htmlFor={`stack${i}`}>
                Stack {i + 1}: new variable name <span className="fieldhint">tick its variables in order</span>
              </label>
              <input id={`stack${i}`} className="textinput" placeholder="score" value={s.target} onChange={(e) => setStacks((ss) => ss.map((x, j) => (j === i ? { ...x, target: e.target.value } : x)))} />
              {box(`Variables stacked into ${s.target || 'it'}`, s.variables, (v) => setStacks((ss) => ss.map((x, j) => (j === i ? { ...x, variables: v } : x))))}
            </div>
          ))}
          <button type="button" className="btn" onClick={() => setStacks((ss) => [...ss, { target: '', variables: [] }])}>
            Add another stack
          </button>
          {!longOk && stacks.some((s) => s.variables.length > 0) && <div className="fieldhint">Every stack needs a name and the same number of variables (at least two).</div>}
          <div className="designopts">
            <label>
              Index variable <input className="textinput" style={{ width: 120 }} value={index} onChange={(e) => setIndex(e.target.value)} />
            </label>
            <label>
              Case identifier{' '}
              <select className="textinput" value={idVar} onChange={(e) => setIdVar(e.target.value)} aria-label="Case identifier">
                <option value="">A new variable numbered 1, 2, …</option>
                {dataset.variables.map((v) => (
                  <option key={v.name} value={v.name}>
                    {v.name}
                  </option>
                ))}
              </select>
            </label>
            {!idVar && (
              <label>
                named <input className="textinput" style={{ width: 100 }} value={newId} onChange={(e) => setNewId(e.target.value)} />
              </label>
            )}
            <label className="check">
              <input type="checkbox" checked={dropMissing} onChange={(e) => setDropMissing(e.target.checked)} /> Leave out rows whose stacked values are all missing
            </label>
          </div>
        </>
      ) : (
        <>
          {box('Identifier variable(s): one row each', ids, setIds)}
          <label className="fieldlabel" htmlFor="wIndex">
            Index variable <span className="fieldhint">its values name the new variables (score_1, score_2, …)</span>
          </label>
          <select id="wIndex" className="textinput" value={wIndex} onChange={(e) => setWIndex(e.target.value)}>
            <option value="">Choose…</option>
            {dataset.variables
              .filter((v) => !ids.includes(v.name))
              .map((v) => (
                <option key={v.name} value={v.name}>
                  {v.name}
                </option>
              ))}
          </select>
          {box('Variables to spread (none ticked: all the others)', spread, setSpread, [...ids, wIndex])}
        </>
      )}
      <p className="fieldhint">The restructured data replaces the data shown; save first if you need the original.</p>
      {error && <div className="errbox" role="alert">{error}</div>}
    </Modal>
  );
}
