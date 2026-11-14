// Data › Data Health…: each variable's counts and range, and findings worth a look. A finding with a fix shows a
// preview of exactly what would change; nothing changes until the fix is applied. Outliers, duplicate cases and
// spellings are reported, never removed or merged.

import { useEffect, useState } from 'react';
import { call } from '../bridge';
import { Modal } from './Modal';

type Fix = { kind: string; name: string; codes?: number[]; measure?: string };
type Finding = { kind: string; level: string; variables: string[]; message: string; cases: number[]; fix: Fix | null };
type VarHealth = { name: string; kind: string; measure: string; valid: number; missing: number; user_missing: number; unique: number; min: number | null; max: number | null };
type Health = { rows: number; variables: VarHealth[]; findings: Finding[] };
type Preview = { summary: string; changes: [number, string, string][]; count: number };

const fixLabel = (f: Fix) => (f.kind === 'to_numeric' ? 'Convert to numbers' : f.kind === 'mark_missing' ? 'Mark as missing codes' : 'Set measure to nominal');
const fmt = (v: number | null) => (v === null ? '' : Number.isInteger(v) ? String(v) : v.toPrecision(6).replace(/\.?0+$/, ''));

export function DataHealthDialog({ onClose, onDone }: { onClose: () => void; onDone: (message: string) => void }) {
  const [health, setHealth] = useState<Health | null>(null);
  const [preview, setPreview] = useState<{ fix: Fix; p: Preview } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = () =>
    call<Health>('data_health', {})
      .then(setHealth)
      .catch((e: Error) => setError(e.message));
  useEffect(() => {
    load();
  }, []);

  const show = async (fix: Fix) => {
    setError(null);
    try {
      const r = await call<{ preview: Preview }>('health_fix', { fix, apply: false });
      setPreview({ fix, p: r.preview });
    } catch (e) {
      setError((e as Error).message);
    }
  };
  const apply = async () => {
    if (!preview) return;
    setBusy(true);
    try {
      await call('health_fix', { fix: preview.fix, apply: true });
      onDone(preview.p.summary);
      setPreview(null);
      await load();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const review = health?.findings.filter((f) => f.level === 'review') ?? [];
  const notes = health?.findings.filter((f) => f.level !== 'review') ?? [];
  const list = (fs: Finding[]) => (
    <ul className="healthlist">
      {fs.map((f, i) => (
        <li key={i}>
          <span>{f.message}</span>
          {f.cases.length > 0 && <span className="fieldhint"> Cases: {f.cases.join(', ')}{f.cases.length === 20 ? ', …' : ''}.</span>}
          {f.fix && (
            <button type="button" className="btn" style={{ marginLeft: 8 }} onClick={() => show(f.fix as Fix)}>
              Preview: {fixLabel(f.fix)}…
            </button>
          )}
        </li>
      ))}
    </ul>
  );

  return (
    <Modal
      title="Data Health"
      onClose={onClose}
      width={820}
      footer={
        <button type="button" className="btn primary" onClick={onClose}>
          Close
        </button>
      }
    >
      {error && <div className="errbox" role="alert">{error}</div>}
      {!health && !error && <div className="fieldhint">Checking…</div>}
      {health && (
        <>
          <h3 className="healthhead">{review.length === 0 ? 'Nothing needs review' : `${review.length} finding${review.length === 1 ? '' : 's'} to review`}</h3>
          {review.length > 0 && list(review)}
          {notes.length > 0 && (
            <>
              <h3 className="healthhead">Worth knowing</h3>
              {list(notes)}
            </>
          )}
          <p className="fieldhint">The check never changes the data. Outliers, duplicate cases and spellings are reported, never removed or merged; a fix is applied only when you choose it, after its preview.</p>
          <h3 className="healthhead">Variables ({health.rows} cases)</h3>
          <div style={{ maxHeight: 260, overflow: 'auto' }}>
            <table className="designtable healthtable">
              <thead>
                <tr>
                  <th>Variable</th>
                  <th>Type</th>
                  <th>Measure</th>
                  <th>Valid</th>
                  <th>Empty</th>
                  <th>User-missing</th>
                  <th>Unique</th>
                  <th>Minimum</th>
                  <th>Maximum</th>
                </tr>
              </thead>
              <tbody>
                {health.variables.map((v) => (
                  <tr key={v.name}>
                    <td>{v.name}</td>
                    <td>{v.kind}</td>
                    <td>{v.measure}</td>
                    <td>{v.valid}</td>
                    <td>{v.missing}</td>
                    <td>{v.user_missing}</td>
                    <td>{v.unique}</td>
                    <td>{fmt(v.min)}</td>
                    <td>{fmt(v.max)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
      {preview && (
        <Modal
          title={`Preview: ${fixLabel(preview.fix)}`}
          onClose={() => setPreview(null)}
          width={560}
          footer={
            <>
              <button type="button" className="btn" onClick={() => setPreview(null)}>
                Cancel
              </button>
              <button type="button" className="btn primary" disabled={busy} onClick={apply}>
                {busy ? 'Applying…' : 'Apply'}
              </button>
            </>
          }
        >
          <p>{preview.p.summary}</p>
          {preview.p.changes.length > 0 && (
            <table className="designtable">
              <thead>
                <tr>
                  <th>Case</th>
                  <th>Now</th>
                  <th>Becomes</th>
                </tr>
              </thead>
              <tbody>
                {preview.p.changes.map(([c, a, b]) => (
                  <tr key={c}>
                    <td>{c}</td>
                    <td>{a}</td>
                    <td>{b}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          {preview.p.count > preview.p.changes.length && <p className="fieldhint">The first {preview.p.changes.length} of {preview.p.count} are listed.</p>}
          <p className="fieldhint">The change is recorded like any other, so results that used the old data are marked out of date.</p>
        </Modal>
      )}
    </Modal>
  );
}
