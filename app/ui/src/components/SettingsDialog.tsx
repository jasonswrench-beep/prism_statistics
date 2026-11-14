// Tools › Settings: text size, how Data View opens, and the confidence level new dialogs start with (settings.ts).

import { useState } from 'react';
import { getSettings, saveSettings, DEFAULTS, type Settings } from '../settings';
import { Modal } from './Modal';

export function SettingsDialog({ onClose, onAi, onAssistant }: { onClose: () => void; onAi: () => void; onAssistant: () => void }) {
  const [s, setS] = useState<Settings>(getSettings());
  const [level, setLevel] = useState(String(s.level));
  const levelOk = /^\d+(\.\d+)?$/.test(level.trim()) && Number(level) >= 50 && Number(level) < 100;
  const save = () => {
    saveSettings({ ...s, level: Number(level) });
    onClose();
  };
  return (
    <Modal
      title="Settings"
      onClose={onClose}
      width={520}
      footer={
        <>
          <button type="button" className="btn" onClick={() => (setS(DEFAULTS), setLevel(String(DEFAULTS.level)))}>
            Restore defaults
          </button>
          <span style={{ flex: 1 }} />
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <button type="button" className="btn primary" disabled={!levelOk} onClick={save}>
            Save
          </button>
        </>
      }
    >
      <div className="settings">
        <fieldset>
          <legend>Text size</legend>
          {([100, 115, 130] as const).map((n) => (
            <label key={n} className="radio">
              <input type="radio" name="textSize" checked={s.textSize === n} onChange={() => setS({ ...s, textSize: n })} />
              {n === 100 ? 'Standard' : n === 115 ? 'Larger (115%)' : 'Largest (130%), for a projector'}
            </label>
          ))}
        </fieldset>
        <fieldset>
          <legend>Data View</legend>
          <label className="switch">
            <input type="checkbox" checked={s.valueLabels} onChange={(e) => setS({ ...s, valueLabels: e.target.checked })} />
            Show value labels when it opens
          </label>
          <label className="switch">
            <input type="checkbox" checked={s.browseOnly} onChange={(e) => setS({ ...s, browseOnly: e.target.checked })} />
            Open browse-only, so cells can't be changed by accident
          </label>
        </fieldset>
        <fieldset>
          <legend>Analyses</legend>
          <label className="field">
            <span>Confidence level new dialogs start with (%)</span>
            <input inputMode="decimal" value={level} onChange={(e) => setLevel(e.target.value)} aria-invalid={!levelOk} style={{ width: 90 }} />
          </label>
          {!levelOk && <p className="fieldhint">Type a level of at least 50 and under 100, such as 95 or 99.</p>}
          <p className="fieldhint">Each dialog can still change it. Results already run keep the level they used. Decimal places follow APA style for each statistic, so the output and every export always match.</p>
        </fieldset>
        <fieldset>
          <legend>Other settings</legend>
          <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
            <button type="button" className="btn" onClick={onAi}>
              AI writing…
            </button>
            <button type="button" className="btn" onClick={onAssistant}>
              Connect an AI assistant…
            </button>
          </div>
        </fieldset>
        <p className="fieldhint">Settings belong to you on this computer, not to a project, and apply from the next dialog or Data View you open.</p>
      </div>
    </Modal>
  );
}
