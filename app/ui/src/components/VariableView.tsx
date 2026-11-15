// Variable View: name, type, label, value labels, missing codes and measure for each variable (FAM-03).

import { useState } from 'react';
import { call } from '../bridge';
import { parseCodes, parseValueLabels } from '../format';
import type { DatasetInfo, Measure, VariableInfo } from '../types';
import { Icon } from './Icon';

export function VariableView({ dataset, onChange }: { dataset: DatasetInfo; onChange: (d: DatasetInfo) => void }) {
  const [editing, setEditing] = useState<VariableInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  const update = async (name: string, patch: Record<string, unknown>) => {
    try {
      setError(null);
      onChange(await call<DatasetInfo>('update_variable', { name, ...patch }));
    } catch (e) {
      setError((e as Error).message);
    }
  };

  return (
    <div className="gridwrap">
      {error && <div className="errbox" role="alert">{error}</div>}
      <div className="gridscroll">
        <table className="dg vv" aria-label="Variable View">
          <thead>
            <tr>
              <th className="rn" scope="col">
                #
              </th>
              <th scope="col">Name</th>
              <th scope="col">Type</th>
              <th scope="col">Label</th>
              <th scope="col">Values</th>
              <th scope="col">
                Missing
                <small id="missingHint"> codes, separated by commas</small>
              </th>
              <th scope="col">Measure</th>
              <th scope="col">Empty cells</th>
            </tr>
          </thead>
          <tbody>
            {dataset.variables.map((v, i) => (
              <tr key={v.id}>
                <th className="rn" scope="row">
                  {i + 1}
                </th>
                <td>{v.name}</td>
                <td>{v.type === 'numeric' ? 'Numeric' : 'Text'}</td>
                <td style={{ minWidth: 220 }}>
                  <input aria-label={`Label for ${v.name}`} defaultValue={v.label} key={`${v.id}-${v.label}`} onBlur={(e) => e.target.value !== v.label && update(v.name, { label: e.target.value })} onKeyDown={(e) => e.key === 'Enter' && (e.target as HTMLInputElement).blur()} />
                </td>
                <td>
                  <button type="button" className="tb" disabled={v.type !== 'numeric'} onClick={() => setEditing(v)} aria-label={`Value labels for ${v.name}`}>
                    {v.value_labels.length ? v.value_labels.slice(0, 2).map((l) => `${l.value} = ${l.label}`).join(', ') + (v.value_labels.length > 2 ? ', …' : '') : 'None'}
                  </button>
                </td>
                <td>
                  <input
                    aria-label={`Missing-value codes for ${v.name}`}
                    placeholder="None (e.g. -9, 99)"
                    title="One or more codes, separated by commas, such as -9, 99"
                    aria-describedby="missingHint"
                    disabled={v.type !== 'numeric'}
                    defaultValue={v.missing_values.join(', ')}
                    key={`${v.id}-${v.missing_values.join(',')}`}
                    onBlur={(e) => {
                      const { codes, error: err } = parseCodes(e.target.value);
                      if (err) return setError(`${v.name}: ${err}`);
                      if (codes.join(',') !== v.missing_values.join(',')) update(v.name, { missing_values: codes });
                    }}
                    onKeyDown={(e) => e.key === 'Enter' && (e.target as HTMLInputElement).blur()}
                  />
                  {v.missing_ranges?.length ? <span className="fieldhint">{v.missing_values.length ? ' and ' : ''}{v.missing_ranges.map(([lo, hi]) => `${lo} to ${hi}`).join(', ')}</span> : null}
                </td>
                <td>
                  <span className="measure">
                    <Icon name={v.measure} />
                    <select aria-label={`Measure for ${v.name}`} value={v.measure} onChange={(e) => update(v.name, { measure: e.target.value as Measure })}>
                      <option value="nominal">Nominal</option>
                      <option value="ordinal">Ordinal</option>
                      <option value="scale" disabled={v.type !== 'numeric'}>
                        Scale
                      </option>
                    </select>
                  </span>
                </td>
                <td className="num">{v.empty}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {editing && <ValueLabelsDialog variable={editing} onClose={() => setEditing(null)} onSave={(labels) => (update(editing.name, { value_labels: labels }), setEditing(null))} />}
    </div>
  );
}

function ValueLabelsDialog({ variable, onClose, onSave }: { variable: VariableInfo; onClose: () => void; onSave: (l: { value: number; label: string }[]) => void }) {
  const [text, setText] = useState(variable.value_labels.map((l) => `${l.value} = ${l.label}`).join('\n'));
  const [error, setError] = useState<string | null>(null);
  const save = () => {
    const r = parseValueLabels(text);
    if (r.error) return setError(r.error);
    onSave(r.labels);
  };
  return (
    <div className="overlay open" onKeyDown={(e) => e.key === 'Escape' && onClose()}>
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby="vlTitle" style={{ width: 'min(480px,100%)' }}>
        <header>
          <h2 id="vlTitle">Value labels: {variable.name}</h2>
          <button type="button" className="iconbtn x" aria-label="Close" onClick={onClose}>
            <Icon name="close" />
          </button>
        </header>
        <div className="dbody vlabels">
          <label className="fieldlabel" htmlFor="vlText">
            One per line, code first <span className="fieldhint">for example 1 = Urban</span>
          </label>
          <textarea id="vlText" autoFocus value={text} onChange={(e) => setText(e.target.value)} />
          {error && (
            <div className="inline-err" role="alert">
              <Icon name="warn" />
              {error}
            </div>
          )}
          <p className="fieldhint">Labels are stored by code, so changing a label never changes which cases an analysis uses.</p>
        </div>
        <footer>
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <button type="button" className="btn primary" onClick={save}>
            Save labels
          </button>
        </footer>
      </div>
    </div>
  );
}
