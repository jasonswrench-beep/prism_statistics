// Data › Select Cases…, Split File…, Weight Cases…: which cases the analyses run from now on use. Each result keeps the
// settings it ran with, so changing them never makes earlier results out of date.

import { useState } from 'react';
import { call } from '../bridge';
import type { DatasetInfo } from '../types';
import { Modal } from './Modal';

export interface DataSettings {
  filter?: string;
  split?: string;
  weight?: string;
}

/** The status-bar wording for each setting. */
export const settingChips = (s: DataSettings) => [s.filter ? `Filter: ${s.filter}` : 'No filter', s.weight ? `Weighted by ${s.weight}` : 'No weights', s.split ? `Split by ${s.split}` : 'No split'];

export function DataSettingsDialog({ dataset, initial, focus, onClose, onDone, onPaste }: { dataset: DatasetInfo; initial: DataSettings; focus: 'filter' | 'split' | 'weight'; onClose: () => void; onDone: (s: DataSettings) => void; onPaste?: (command: string, args: unknown) => void }) {
  const [filter, setFilter] = useState(initial.filter ?? '');
  const [split, setSplit] = useState(initial.split ?? '');
  const [weight, setWeight] = useState(initial.weight ?? '');
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const args = { filter: filter.trim() || null, split: split || null, weight: weight || null };
  const apply = async () => {
    setRunning(true);
    setError(null);
    try {
      onDone(await call<DataSettings>('set_data_settings', args));
      onClose();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setRunning(false);
    }
  };
  const numeric = dataset.variables.filter((v) => v.type === 'numeric');
  return (
    <Modal
      title="Select, Split and Weight Cases"
      onClose={onClose}
      width={620}
      footer={
        <>
          {onPaste && (
            <button type="button" className="btn" onClick={() => onPaste('set_data_settings', args)} title="Add this change to the script without making it">
              Paste
            </button>
          )}
          <button type="button" className="btn" onClick={() => (setFilter(''), setSplit(''), setWeight(''))}>
            Clear all
          </button>
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <button type="button" className="btn primary" disabled={running} onClick={apply}>
            {running ? 'Checking…' : 'Apply'}
          </button>
        </>
      }
    >
      <p className="fieldhint" style={{ marginTop: 0 }}>
        These apply to analyses run from now on. Each result keeps the settings it ran with and says so in its method note.
      </p>
      <label className="fieldlabel" htmlFor="dsFilter">
        Select cases <span className="fieldhint">a condition in R’s syntax, such as age &gt;= 18 &amp; site == "North"; empty for all cases</span>
      </label>
      <input id="dsFilter" className="textinput" autoFocus={focus === 'filter'} value={filter} onChange={(e) => setFilter(e.target.value)} style={{ fontFamily: 'var(--mono, monospace)' }} />
      <div className="fieldhint">Cases where the condition is false or missing are left out. User-missing codes count as missing.</div>
      <label className="fieldlabel" htmlFor="dsSplit">
        Split file <span className="fieldhint">run every analysis separately for each category</span>
      </label>
      <select id="dsSplit" autoFocus={focus === 'split'} value={split} onChange={(e) => setSplit(e.target.value)}>
        <option value="">Don’t split</option>
        {dataset.variables.map((v) => (
          <option key={v.name} value={v.name}>
            {v.name}
            {v.label ? ` (${v.label})` : ''}
          </option>
        ))}
      </select>
      <label className="fieldlabel" htmlFor="dsWeight">
        Weight cases <span className="fieldhint">frequency weights: each case counts as many times as its weight (whole numbers)</span>
      </label>
      <select id="dsWeight" autoFocus={focus === 'weight'} value={weight} onChange={(e) => setWeight(e.target.value)}>
        <option value="">Don’t weight</option>
        {numeric.map((v) => (
          <option key={v.name} value={v.name}>
            {v.name}
            {v.label ? ` (${v.label})` : ''}
          </option>
        ))}
      </select>
      {error && <div className="errbox" role="alert">{error}</div>}
    </Modal>
  );
}
