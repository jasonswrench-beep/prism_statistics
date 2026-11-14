// Data › Sort Cases…, and Transform › Rank Cases…, Standardize…, Reverse Score… and Date and Time…. Each is recorded
// like any other change, can be undone, pasted into a script, and is repeated by the exported R script.

import { useState } from 'react';
import { call } from '../bridge';
import type { DatasetInfo } from '../types';
import { Modal } from './Modal';
import { Footer, Pick, useRun, type Done } from './TransformDialogs';

type Paste = (command: string, args: unknown) => void;

function Buttons({ onPaste, command, args, problem, onClose, run, running, action }: { onPaste?: Paste; command: string; args: unknown; problem: string | null; onClose: () => void; run: (c: string, a: unknown) => void; running: boolean; action: string }) {
  return (
    <>
      {onPaste && (
        <button type="button" className="btn" disabled={!!problem} onClick={() => onPaste(command, args)}>
          Paste
        </button>
      )}
      <Footer onClose={onClose} onRun={() => run(command, args)} disabled={!!problem} running={running} action={action} />
    </>
  );
}

export function SortDialog({ dataset, onClose, onDone, onPaste }: { dataset: DatasetInfo; onClose: () => void; onDone: Done; onPaste?: Paste }) {
  const [keys, setKeys] = useState<{ name: string; descending: boolean }[]>([{ name: '', descending: false }]);
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const chosen = keys.filter((k) => k.name);
  const args = { keys: chosen };
  const problem = chosen.length ? null : 'Choose a variable to sort by.';
  const run = async (command: string, a: unknown) => {
    setRunning(true);
    setError(null);
    try {
      const r = await call<{ cases: number }>(command, a);
      onDone(`Sorted ${r.cases} cases by ${chosen.map((k) => `${k.name}${k.descending ? ' (largest first)' : ''}`).join(', then ')}.`);
      onClose();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setRunning(false);
    }
  };
  const set = (i: number, k: Partial<{ name: string; descending: boolean }>) => setKeys((ks) => ks.map((x, j) => (j === i ? { ...x, ...k } : x)));
  return (
    <Modal title="Sort Cases" onClose={onClose} width={560} footer={<Buttons onPaste={onPaste} command="sort_cases" args={args} problem={problem} onClose={onClose} run={run} running={running} action="Sort" />}>
      <p className="fieldhint">Puts the cases in order. Cases that tie on the first variable are ordered by the next. Empty cells and missing-value codes go last.</p>
      {keys.map((k, i) => (
        <div key={i} style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap', marginBottom: 6 }}>
          <span className="fieldlabel" style={{ minWidth: 64 }}>{i === 0 ? 'Sort by' : 'then by'}</span>
          <select aria-label={i === 0 ? 'Sort by' : `Then by (${i + 1})`} value={k.name} onChange={(e) => set(i, { name: e.target.value })}>
            <option value="">Choose a variable</option>
            {dataset.variables.map((v) => (
              <option key={v.id} value={v.name}>
                {v.name}
                {v.label ? ` (${v.label})` : ''}
              </option>
            ))}
          </select>
          <label className="radio">
            <input type="radio" checked={!k.descending} onChange={() => set(i, { descending: false })} /> Smallest first
          </label>
          <label className="radio">
            <input type="radio" checked={k.descending} onChange={() => set(i, { descending: true })} /> Largest first
          </label>
          {keys.length > 1 && (
            <button type="button" className="iconbtn" aria-label={`Remove sort variable ${i + 1}`} onClick={() => setKeys((ks) => ks.filter((_, j) => j !== i))}>
              ×
            </button>
          )}
        </div>
      ))}
      <button type="button" className="btn" onClick={() => setKeys((ks) => [...ks, { name: '', descending: false }])}>
        Add a variable
      </button>
      <p className="fieldhint">Text sorts by character code: capitals before lowercase. Results that depend on the order of the cases, such as time series and the runs test, are marked out of date.</p>
      {error && <div className="errbox" role="alert">{error}</div>}
    </Modal>
  );
}

export function RankDialog({ dataset, onClose, onDone, onPaste }: { dataset: DatasetInfo; onClose: () => void; onDone: Done; onPaste?: Paste }) {
  const [vars, setVars] = useState<string[]>([]);
  const [ties, setTies] = useState<'mean' | 'low' | 'high' | 'sequential'>('mean');
  const [descending, setDescending] = useState(false);
  const [by, setBy] = useState('');
  const { error, running, run } = useRun(onDone, onClose);
  const ordered = dataset.variables.map((v) => v.name).filter((n) => vars.includes(n));
  const args = { variables: ordered, ties, descending, by: by || null };
  const problem = ordered.length ? null : 'Choose at least one variable.';
  return (
    <Modal title="Rank Cases" onClose={onClose} width={600} footer={<Buttons onPaste={onPaste} command="rank_cases" args={args} problem={problem} onClose={onClose} run={run} running={running} action="Rank" />}>
      <Pick dataset={dataset} chosen={vars} onChange={setVars} numericOnly label="Variables to rank (each makes r_<name>)" />
      <fieldset style={{ border: 0, padding: 0 }}>
        <legend className="fieldlabel">Rank 1 goes to</legend>
        <label className="radio">
          <input type="radio" checked={!descending} onChange={() => setDescending(false)} /> The smallest value (default)
        </label>
        <label className="radio">
          <input type="radio" checked={descending} onChange={() => setDescending(true)} /> The largest value
        </label>
      </fieldset>
      <fieldset style={{ border: 0, padding: 0 }}>
        <legend className="fieldlabel">Tied values get</legend>
        {(
          [
            ['mean', 'The mean of their ranks (default)', 'two values tied for 2nd and 3rd both get 2.5'],
            ['low', 'The lowest of their ranks', 'both get 2'],
            ['high', 'The highest of their ranks', 'both get 3'],
            ['sequential', 'Ranks in case order', '2 and 3, in the order the cases come'],
          ] as const
        ).map(([v, l, h]) => (
          <label key={v} className="radio">
            <input type="radio" checked={ties === v} onChange={() => setTies(v)} /> {l} <span className="fieldhint">{h}</span>
          </label>
        ))}
      </fieldset>
      <label className="fieldlabel" htmlFor="rankBy">
        Rank within groups of <span className="fieldhint">optional: ranks start at 1 in each group</span>
      </label>
      <select id="rankBy" value={by} onChange={(e) => setBy(e.target.value)}>
        <option value="">No groups</option>
        {dataset.variables.map((v) => (
          <option key={v.id} value={v.name}>
            {v.name}
          </option>
        ))}
      </select>
      <p className="fieldhint">Empty cells and missing-value codes get no rank. The new variables are ordinal.</p>
      {error && <div className="errbox" role="alert">{error}</div>}
    </Modal>
  );
}

export function StandardizeDialog({ dataset, onClose, onDone, onPaste }: { dataset: DatasetInfo; onClose: () => void; onDone: Done; onPaste?: Paste }) {
  const [vars, setVars] = useState<string[]>([]);
  const { error, running, run } = useRun(onDone, onClose);
  const ordered = dataset.variables.map((v) => v.name).filter((n) => vars.includes(n));
  const args = { variables: ordered };
  const problem = ordered.length ? null : 'Choose at least one variable.';
  return (
    <Modal title="Standardize" onClose={onClose} width={560} footer={<Buttons onPaste={onPaste} command="standardize" args={args} problem={problem} onClose={onClose} run={run} running={running} action="Standardize" />}>
      <p className="fieldhint">Makes a z-score for each variable: z = (value − mean) / standard deviation, with the sample standard deviation (n − 1). A z of 1.5 is one and a half standard deviations above the mean.</p>
      <Pick dataset={dataset} chosen={vars} onChange={setVars} numericOnly label="Variables (each makes z_<name>)" />
      <p className="fieldhint">The mean and SD come from the cases with a valid value. Missing values stay missing.</p>
      {error && <div className="errbox" role="alert">{error}</div>}
    </Modal>
  );
}

export function ReverseDialog({ dataset, onClose, onDone, onPaste }: { dataset: DatasetInfo; onClose: () => void; onDone: Done; onPaste?: Paste }) {
  const [vars, setVars] = useState<string[]>([]);
  const [lo, setLo] = useState('1');
  const [hi, setHi] = useState('5');
  const [replace, setReplace] = useState(false);
  const { error, running, run } = useRun(onDone, onClose);
  const ordered = dataset.variables.map((v) => v.name).filter((n) => vars.includes(n));
  const args = { variables: ordered, min: Number(lo), max: Number(hi), replace };
  const problem = !ordered.length ? 'Choose at least one variable.' : !(Number.isFinite(Number(lo)) && Number.isFinite(Number(hi)) && Number(lo) < Number(hi)) ? 'Give the lowest and highest possible answers, lowest first.' : null;
  return (
    <Modal title="Reverse Score" onClose={onClose} width={560} footer={<Buttons onPaste={onPaste} command="reverse_score" args={args} problem={problem} onClose={onClose} run={run} running={running} action="Reverse" />}>
      <p className="fieldhint">For items worded the other way round, such as “I feel calm during tests” on an anxiety scale. Each answer becomes lowest + highest − answer, so on a 1 to 5 scale 1 becomes 5 and 4 becomes 2. Value labels move with their values.</p>
      <Pick dataset={dataset} chosen={vars} onChange={setVars} numericOnly label="Items to reverse" />
      <div>
        Possible answers from <input aria-label="Lowest possible answer" className="textinput" style={{ width: 60 }} value={lo} onChange={(e) => setLo(e.target.value)} /> to{' '}
        <input aria-label="Highest possible answer" className="textinput" style={{ width: 60 }} value={hi} onChange={(e) => setHi(e.target.value)} />
      </div>
      <fieldset style={{ border: 0, padding: 0 }}>
        <legend className="fieldlabel">Put the reversed answers</legend>
        <label className="radio">
          <input type="radio" checked={!replace} onChange={() => setReplace(false)} /> In new variables, &lt;name&gt;_r (default)
        </label>
        <label className="radio">
          <input type="radio" checked={replace} onChange={() => setReplace(true)} /> In the same variables
        </label>
      </fieldset>
      <p className="fieldhint">Missing-value codes such as −99 are kept as they are. A value outside the range stops the change, so a mistyped range can’t go unnoticed.</p>
      {error && <div className="errbox" role="alert">{error}</div>}
    </Modal>
  );
}

const SOURCES = [
  ['text_ymd', 'Text, year first', '2026-11-14, 2026/11/14 13:05'],
  ['text_mdy', 'Text, month first', '11/14/2026'],
  ['text_dmy', 'Text, day first', '14/11/2026, 14.11.2026'],
  ['spreadsheet', 'Spreadsheet date numbers', 'days since 30 December 1899, as in .xlsx files'],
  ['seconds1582', 'Seconds since 14 October 1582', 'as .sav files store dates'],
  ['days1960', 'Days since 1 January 1960', 'as .dta and .sas7bdat files store dates'],
  ['seconds1960', 'Seconds since 1 January 1960', 'as .dta and .sas7bdat files store date-times'],
] as const;

const PARTS = [
  ['year', 'Year'],
  ['month', 'Month'],
  ['day', 'Day of the month'],
  ['weekday', 'Day of the week (1 = Monday)'],
  ['day_of_year', 'Day of the year'],
  ['hour', 'Hour'],
  ['minute', 'Minute'],
  ['day_number', 'Day number (days since 1970-01-01)'],
] as const;

export function DateDialog({ dataset, onClose, onDone, onPaste }: { dataset: DatasetInfo; onClose: () => void; onDone: Done; onPaste?: Paste }) {
  const [mode, setMode] = useState<'parts' | 'difference'>('parts');
  const [variable, setVariable] = useState('');
  const [end, setEnd] = useState('');
  const [source, setSource] = useState<(typeof SOURCES)[number][0]>('text_ymd');
  const [parts, setParts] = useState<string[]>(['year', 'month', 'day']);
  const [unit, setUnit] = useState<'days' | 'weeks' | 'months' | 'years'>('years');
  const [target, setTarget] = useState('');
  const { error, running, run } = useRun(onDone, onClose);
  const v = dataset.variables.find((x) => x.name === variable);
  const isText = source.startsWith('text_');
  const kindProblem = v && (v.type === 'text') !== isText ? (isText ? `${v.name} holds numbers; choose which kind of date numbers they are.` : `${v.name} holds text; choose how its dates are written.`) : null;
  const command = mode === 'parts' ? 'date_parts' : 'date_difference';
  const args = mode === 'parts' ? { variable, source, parts: PARTS.map((p) => p[0]).filter((p) => parts.includes(p)) } : { start: variable, end, source, unit, target: target.trim() };
  const problem = !variable
    ? 'Choose the date variable.'
    : kindProblem
      ? kindProblem
      : mode === 'parts'
        ? parts.length
          ? null
          : 'Choose at least one part.'
        : !end
          ? 'Choose the end date.'
          : !target.trim()
            ? 'Name the new variable.'
            : null;
  const varSelect = (id: string, label: string, value: string, set: (s: string) => void) => (
    <>
      <label className="fieldlabel" htmlFor={id}>
        {label}
      </label>
      <select id={id} value={value} onChange={(e) => set(e.target.value)}>
        <option value="">Choose a variable</option>
        {dataset.variables.map((x) => (
          <option key={x.id} value={x.name}>
            {x.name}
            {x.label ? ` (${x.label})` : ''}
          </option>
        ))}
      </select>
    </>
  );
  return (
    <Modal title="Date and Time" onClose={onClose} width={620} footer={<Buttons onPaste={onPaste} command={command} args={args} problem={problem} onClose={onClose} run={run} running={running} action={mode === 'parts' ? 'Make variables' : 'Compute'} />}>
      <div role="radiogroup" aria-label="What to make">
        <label className="radio">
          <input type="radio" checked={mode === 'parts'} onChange={() => setMode('parts')} /> Parts of a date (year, month, day of the week…)
        </label>
        <label className="radio">
          <input type="radio" checked={mode === 'difference'} onChange={() => setMode('difference')} /> Time between two dates (such as age)
        </label>
      </div>
      {varSelect('dateVar', mode === 'parts' ? 'Date variable' : 'Start date', variable, setVariable)}
      {mode === 'difference' && varSelect('dateEnd', 'End date', end, setEnd)}
      <fieldset style={{ border: 0, padding: 0 }}>
        <legend className="fieldlabel">The dates are stored as</legend>
        {SOURCES.map(([s, l, h]) => (
          <label key={s} className="radio">
            <input type="radio" checked={source === s} onChange={() => setSource(s)} /> {l} <span className="fieldhint">{h}</span>
          </label>
        ))}
      </fieldset>
      {mode === 'parts' ? (
        <fieldset style={{ border: 0, padding: 0 }}>
          <legend className="fieldlabel">Make (each as &lt;name&gt;_year, &lt;name&gt;_month…)</legend>
          {PARTS.map(([p, l]) => (
            <label key={p} className="check" style={{ display: 'block' }}>
              <input type="checkbox" checked={parts.includes(p)} onChange={(e) => setParts((x) => (e.target.checked ? [...x, p] : x.filter((y) => y !== p)))} /> {l}
            </label>
          ))}
        </fieldset>
      ) : (
        <>
          <fieldset style={{ border: 0, padding: 0 }}>
            <legend className="fieldlabel">In</legend>
            {(
              [
                ['years', 'Whole years completed (default)', 'as an age is counted'],
                ['months', 'Whole months completed', ''],
                ['weeks', 'Weeks', 'with a fraction'],
                ['days', 'Days', 'with a fraction for the time of day'],
              ] as const
            ).map(([u, l, h]) => (
              <label key={u} className="radio">
                <input type="radio" checked={unit === u} onChange={() => setUnit(u)} /> {l} {h && <span className="fieldhint">{h}</span>}
              </label>
            ))}
          </fieldset>
          <label className="fieldlabel" htmlFor="dateTarget">
            New variable
          </label>
          <input id="dateTarget" className="textinput" value={target} onChange={(e) => setTarget(e.target.value)} placeholder="age" />
        </>
      )}
      <p className="fieldhint">Text must have a four-digit year. A cell that isn’t a real date, such as 31 February, is left missing and counted in the message afterwards.</p>
      {kindProblem && <div className="fieldhint err">{kindProblem}</div>}
      {error && <div className="errbox" role="alert">{error}</div>}
    </Modal>
  );
}
