// The optional AI writer (D14): Tools › AI writing (switch, model download) and the AI draft block on each result.
// Off by default. The model runs on this computer; nothing is sent anywhere. A draft is marked as AI text, shows its
// checks, and stays out of exports until the user accepts or edits it.

import { useEffect, useState } from 'react';
import { call } from '../bridge';
import { italicRuns, plainText, withoutAccount } from '../format';
import type { AiDraft, AiStatus, OutputItem } from '../types';
import { Icon } from './Icon';
import { Modal } from './Modal';

const GB = (n: number) => `${(n / 1e9).toFixed(1)} GB`;

export function AiSettingsDialog({ onClose, onChange }: { onClose: () => void; onChange: (s: AiStatus) => void }) {
  const [s, setS] = useState<AiStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const refresh = async () => {
    try {
      const st = await call<AiStatus>('ai_status');
      setS(st);
      onChange(st);
    } catch (e) {
      setError((e as Error).message);
    }
  };
  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useEffect(() => {
    if (!s?.downloading) return;
    const t = window.setInterval(refresh, 1000);
    return () => window.clearInterval(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [s?.downloading]);
  const act = async (cmd: string, args: unknown = {}) => {
    setError(null);
    try {
      const st = await call<AiStatus>(cmd, args);
      setS(st);
      onChange(st);
    } catch (e) {
      setError((e as Error).message);
    }
  };
  return (
    <Modal title="AI writing" onClose={onClose} width={620} footer={<button type="button" className="btn primary" onClick={onClose}>Done</button>}>
      <p style={{ marginTop: 0 }}>
        PrismStatistics can draft a results paragraph with a small AI model that runs <b>on this computer</b>. Nothing is sent anywhere. The model never writes a number: every number is filled in from the checked result, and each draft is checked for wrong directions, misused significance language and causal claims. You review every draft before it can go into an export.
      </p>
      {s && (
        <>
          <label className="check" style={{ margin: '12px 0' }}>
            <input type="checkbox" checked={s.enabled} onChange={(e) => act('ai_enable', { on: e.target.checked })} />
            <span>
              <b>Turn on AI writing</b> <span className="fieldhint">(off by default; you can turn it off at any time)</span>
            </span>
          </label>
          <div className="card" style={{ padding: 12 }}>
            <div>
              <b>Model:</b> {s.model} · licence {s.license} · {GB(s.size)}
            </div>
            {s.installed ? (
              <div style={{ marginTop: 8, display: 'flex', gap: 10, alignItems: 'center', flexWrap: 'wrap' }}>
                <span className="status ok">
                  <Icon name="check" />
                  Installed
                </span>
                <button type="button" className="btn" onClick={() => act('ai_remove')}>
                  Remove the model ({GB(s.size)})
                </button>
              </div>
            ) : s.downloading ? (
              <div style={{ marginTop: 8 }} aria-live="polite">
                Downloading… {s.total ? Math.floor((100 * s.done) / s.total) : 0}% ({GB(s.done)} of {GB(s.total || s.size)})
                <progress max={s.total || s.size} value={s.done} style={{ width: '100%' }} />
              </div>
            ) : (
              <div style={{ marginTop: 8 }}>
                <button type="button" className="btn primary" disabled={!s.enabled} onClick={() => act('ai_download')}>
                  Download the model ({GB(s.size)})
                </button>
                <p className="fieldhint" style={{ margin: '6px 0 0' }}>
                  A one-time download from the model's publisher, checked against its recorded checksum. It is saved in {withoutAccount(s.folder)}.
                </p>
              </div>
            )}
          </div>
          <p className="fieldhint">Writing a paragraph takes from about 15 seconds to a minute or more, depending on the computer and the analysis.</p>
        </>
      )}
      {(error || s?.error) && (
        <div className="errbox" role="alert">
          {error ?? s?.error}
        </div>
      )}
    </Modal>
  );
}

const CHECK_CLS: Record<string, string> = { pass: 'ok', flag: 'review', fail: 'stale' };

function Rich({ text }: { text: string }) {
  return (
    <>
      {italicRuns(text).map((r, i) => (r.italic ? <i key={i}>{r.text}</i> : r.sub ? <sub key={i}>{r.text}</sub> : <span key={i}>{r.text}</span>))}
    </>
  );
}

/** The AI draft block for one result, with the button that writes it. */
export function AiDraftBlock({ item, ready, onChanged, onCopied }: { item: OutputItem; ready: boolean; onChanged: () => void; onCopied: (msg: string) => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const d: AiDraft | undefined = item.ai ?? undefined;
  const canWrite = ready && item.bundle.status.state === 'ok' && !item.stale;
  const run = async (cmd: string, args: unknown) => {
    setError(null);
    setBusy(cmd === 'ai_draft');
    try {
      await call(cmd, args);
      setEditing(null);
      onChanged();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  if (!d && !canWrite) return null;
  const rejected = d?.checks.some((c) => c.state === 'fail');
  return (
    <div className="gen ai" style={{ marginTop: 12 }}>
      <div className="tag">
        <Icon name="text" />
        {d ? `AI draft · ${d.model} on this computer · ${d.status === 'draft' ? 'not reviewed' : d.status}` : 'AI writing'}
      </div>
      {!d && (
        <button type="button" className="btn" disabled={busy} onClick={() => run('ai_draft', { id: item.id })}>
          <Icon name="text" />
          {busy ? 'Writing… (up to a minute)' : 'Write up with AI'}
        </button>
      )}
      {d && editing === null && (
        <p className="para" style={{ margin: 0 }}>
          <Rich text={d.text} />
        </p>
      )}
      {d && editing !== null && <textarea className="textinput" aria-label="Edit the AI draft" style={{ width: '100%', minHeight: 140 }} value={editing} onChange={(e) => setEditing(e.target.value)} />}
      {d && (
        <ul className="aichecks">
          {d.checks.map((c) => (
            <li key={c.id}>
              <span className={`status ${CHECK_CLS[c.state]}`}>{c.state === 'pass' ? 'OK' : c.state === 'flag' ? 'Look' : 'Failed'}</span> <b>{c.title}:</b> {c.message}
            </li>
          ))}
        </ul>
      )}
      {d && rejected && <div className="inline-err">This draft failed a check, so it can't be accepted as it is. Edit it, discard it, or use the template paragraph.</div>}
      {d && (
        <div style={{ marginTop: 10, display: 'flex', gap: 8, flexWrap: 'wrap' }}>
          {editing === null ? (
            <>
              {d.status === 'draft' && !rejected && (
                <button type="button" className="btn primary" onClick={() => run('ai_review', { id: item.id, action: 'accept' })}>
                  Accept after review
                </button>
              )}
              <button type="button" className="btn" onClick={() => setEditing(d.text)}>
                Edit
              </button>
              <button type="button" className="btn" onClick={async () => (await navigator.clipboard.writeText(plainText(d.text)), onCopied('Copied the AI draft.'))}>
                <Icon name="copy" />
                Copy
              </button>
              <button type="button" className="btn" onClick={() => run('ai_review', { id: item.id, action: 'discard' })}>
                Discard
              </button>
            </>
          ) : (
            <>
              <button type="button" className="btn primary" onClick={() => run('ai_review', { id: item.id, action: 'edit', text: editing })}>
                Save my edits
              </button>
              <button type="button" className="btn" onClick={() => setEditing(null)}>
                Cancel
              </button>
            </>
          )}
        </div>
      )}
      {error && (
        <div className="errbox" role="alert" style={{ marginTop: 8 }}>
          {error}
        </div>
      )}
    </div>
  );
}
