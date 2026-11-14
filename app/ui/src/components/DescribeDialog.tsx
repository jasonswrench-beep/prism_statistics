// Analyze › Descriptive Statistics › Frequencies… and Descriptives…: a source list, a Variables box and, for
// Descriptives, the statistics to report (defaults: mean, SD, minimum, maximum).

import { HelpButton } from './HelpDialog';
import { useMemo, useState } from 'react';
import { call } from '../bridge';
import type { DatasetInfo, OutputItem, VariableInfo } from '../types';
import { Icon } from './Icon';

export type DescribeKind = 'frequencies' | 'descriptives';

const STATS: { id: string; label: string }[] = [
  { id: 'mean', label: 'Mean' },
  { id: 'sd', label: 'Standard deviation' },
  { id: 'min', label: 'Minimum' },
  { id: 'max', label: 'Maximum' },
  { id: 'range', label: 'Range' },
  { id: 'median', label: 'Median' },
  { id: 'se', label: 'Standard error of the mean' },
  { id: 'variance', label: 'Variance' },
  { id: 'ci_mean', label: '95% confidence interval of the mean' },
];
const DEFAULT_STATS = ['mean', 'sd', 'min', 'max'];

export function DescribeDialog({ kind, dataset, initial, onClose, onDone, onPaste }: { kind: DescribeKind; dataset: DatasetInfo; initial?: { variables?: string[]; statistics?: string[] }; onClose: () => void; onDone: (items: OutputItem[]) => void; onPaste?: (command: string, args: unknown) => void }) {
  const eligible = useMemo(() => (kind === 'descriptives' ? dataset.variables.filter((v) => v.type === 'numeric') : dataset.variables), [dataset, kind]);
  const [chosen, setChosen] = useState<string[]>(() => (initial?.variables ?? []).filter((n) => eligible.some((v) => v.name === n)));
  const [selected, setSelected] = useState<string | null>(null);
  const [stats, setStats] = useState<string[]>(initial?.statistics ?? DEFAULT_STATS);
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const title = kind === 'frequencies' ? 'Frequencies' : 'Descriptives';
  const available = eligible.filter((v) => !chosen.includes(v.name));

  const row = (v: VariableInfo, onPick: () => void, picked: boolean) => (
    <li key={v.id} role="option" aria-label={v.label ? `${v.name}, ${v.label}` : v.name} aria-selected={picked} tabIndex={0} onClick={onPick} onKeyDown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), onPick())}>
      <span>{v.name}</span>
      <span className="lab">{v.label}</span>
      <span className="measure">
        <Icon name={v.measure} />
      </span>
    </li>
  );

  const args = kind === 'frequencies' ? { variables: chosen } : { variables: chosen, statistics: stats, level: 0.95 };
  const run = async () => {
    if (!chosen.length) return;
    setRunning(true);
    try {
      onDone(await call<OutputItem[]>(kind, args));
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setRunning(false);
    }
  };

  return (
    <div className="overlay open" onKeyDown={(e) => e.key === 'Escape' && onClose()}>
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby="dsTitle">
        <header>
          <h2 id="dsTitle">{title}</h2>
          <HelpButton />
          <button type="button" className="iconbtn x" aria-label="Close" onClick={onClose}>
            <Icon name="close" />
          </button>
        </header>
        <div className="dbody">
          <div className="transfer">
            <div>
              <span className="fieldlabel" id="dsSrc">
                Variables in the data
              </span>
              <ul className="listbox tall" role="listbox" aria-labelledby="dsSrc">
                {available.length ? available.map((v) => row(v, () => setSelected(v.name), selected === v.name)) : <li className="empty">No variables left</li>}
              </ul>
            </div>
            <div className="rolecol" style={{ marginTop: 40 }}>
              <button
                type="button"
                className="arrowbtn"
                aria-label={`Move to ${title} variables`}
                onClick={() => {
                  if (selected && available.some((v) => v.name === selected)) {
                    setChosen([...chosen, selected]);
                    setSelected(null);
                  }
                }}
              >
                <Icon name="right" />
              </button>
            </div>
            <div>
              <span className="fieldlabel" id="dsChosen">
                Variables <span className="fieldhint">click one to move it back</span>
              </span>
              <ul className="listbox tall" role="listbox" aria-labelledby="dsChosen">
                {chosen.length ? chosen.map((n) => row(eligible.find((v) => v.name === n)!, () => setChosen(chosen.filter((c) => c !== n)), false)) : <li className="empty">Select a variable and press the arrow</li>}
              </ul>
            </div>
          </div>
          {kind === 'descriptives' && (
            <details className="opt" open>
              <summary>
                Statistics <span className="cur">{stats.length === 4 && DEFAULT_STATS.every((s) => stats.includes(s)) ? 'Defaults' : `${stats.length} chosen`}</span>
              </summary>
              <div className="optbody" style={{ gridTemplateColumns: 'repeat(auto-fill,minmax(230px,1fr))' }}>
                {STATS.map((s) => (
                  <label className="radio" key={s.id}>
                    <input type="checkbox" checked={stats.includes(s.id)} onChange={(e) => setStats(e.target.checked ? [...stats, s.id] : stats.filter((x) => x !== s.id))} />
                    <span>{s.label}</span>
                  </label>
                ))}
              </div>
            </details>
          )}
          {kind === 'frequencies' && <p className="fieldhint" style={{ margin: 0 }}>User-missing codes and empty cells are listed as missing, never as categories.</p>}
          {error && <div className="errbox" role="alert">{error}</div>}
        </div>
        <footer>
          <button type="button" className="btn" onClick={() => (setChosen([]), setStats(DEFAULT_STATS))}>
            Reset
          </button>
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          {onPaste && (
            <button type="button" className="btn" disabled={!chosen.length || (kind === 'descriptives' && !stats.length)} onClick={() => onPaste(kind, args)} title="Add this analysis to the script without running it">
              Paste
            </button>
          )}
          <button type="button" className="btn primary" disabled={!chosen.length || running || (kind === 'descriptives' && !stats.length)} onClick={run}>
            <Icon name="run" />
            {running ? 'Running…' : 'Run'}
          </button>
        </footer>
      </div>
    </div>
  );
}
