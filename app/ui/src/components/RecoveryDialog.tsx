// Offered at start-up when an earlier window closed or crashed with unsaved work: open the copy (as unsaved work, to be
// saved where the user chooses) or discard it.

import { useState } from 'react';
import { call } from '../bridge';
import { Modal } from './Modal';
import type { DatasetInfo, OutputItem, ProjectState } from '../types';

export type RecoveryCopy = { id: string; name: string; path: string | null; saved_at: number };
type Recovered = { state: ProjectState; dataset: DatasetInfo; results: OutputItem[] };

export function RecoveryDialog({ copies, onClose, onRecovered }: { copies: RecoveryCopy[]; onClose: () => void; onRecovered: (r: Recovered) => void }) {
  const [list, setList] = useState(copies);
  const [error, setError] = useState<string | null>(null);
  const when = (t: number) => new Date(t * 1000).toLocaleString();
  const recover = async (c: RecoveryCopy) => {
    try {
      onRecovered(await call<Recovered>('recover', { id: c.id }));
    } catch (e) {
      setError((e as Error).message);
    }
  };
  const discard = async (c: RecoveryCopy) => {
    try {
      const r = await call<{ recovery: RecoveryCopy[] }>('discard_recovery', { id: c.id });
      setList(r.recovery);
      if (r.recovery.length === 0) onClose();
    } catch (e) {
      setError((e as Error).message);
    }
  };
  return (
    <Modal
      title="Recover unsaved work"
      onClose={onClose}
      width={600}
      footer={
        <button type="button" className="btn" onClick={onClose}>
          Decide later
        </button>
      }
    >
      {error && <div className="errbox" role="alert">{error}</div>}
      <p>PrismStatistics closed before this work was saved. Open it to keep it, or discard it.</p>
      <ul className="healthlist">
        {list.map((c) => (
          <li key={c.id}>
            <span>
              <strong>{c.name}</strong> <span className="fieldhint">— copied {when(c.saved_at)}{c.path ? `, from ${c.path}` : ', never saved'}</span>
            </span>
            <button type="button" className="btn primary" style={{ marginLeft: 8 }} onClick={() => recover(c)}>
              Open
            </button>
            <button type="button" className="btn" style={{ marginLeft: 8 }} onClick={() => discard(c)}>
              Discard
            </button>
          </li>
        ))}
      </ul>
      <p className="fieldhint">An opened copy is unsaved work: save it to keep it. Each save also keeps the version it replaces as a .backup.pstat file beside the project.</p>
    </Modal>
  );
}
