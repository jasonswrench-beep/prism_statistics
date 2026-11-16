// Help › Check for Updates: a check made when the person presses the button (or, if they turned it on, at most once a day),
// an install that needs a second go-ahead, and Return to the previous version (app/src-tauri/src/updates.rs; docs/UPDATES.md).
// The same design as Prism Reference Manager's Updates card.

import { useEffect, useState } from 'react';
import { call, inTauri } from '../bridge';
import { Modal } from './Modal';
import { progressPercent } from '../updates';
import { getSettings, saveSettings } from '../settings';

interface UpdateState {
  version: string;
  set_up: boolean;
  host: string;
  rollback: boolean;
}

interface UpdateFound {
  available: boolean;
  version?: string;
  notes?: string;
  date?: string;
}

type Phase = 'idle' | 'checking' | 'found' | 'current' | 'installing' | 'failed' | 'back_checking' | 'back_found' | 'back_none';

/** The time of a check, for "Last checked". */
function when(iso: string): string {
  const t = Date.parse(iso);
  return Number.isNaN(t) ? 'never' : new Date(t).toLocaleString();
}

export function UpdatesDialog({ onClose }: { onClose: () => void }) {
  const desktop = inTauri();
  const [state, setState] = useState<UpdateState | null>(null);
  const [phase, setPhase] = useState<Phase>('idle');
  const [found, setFound] = useState<UpdateFound | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [percent, setPercent] = useState<number | null>(null);
  const [daily, setDaily] = useState(getSettings().updateDaily);
  const [lastCheck, setLastCheck] = useState(getSettings().updateLastCheck);

  useEffect(() => {
    if (!desktop) return;
    call<UpdateState>('update.state').then(setState, (e: Error) => setError(e.message));
  }, [desktop]);

  const checked = () => {
    const now = new Date().toISOString();
    saveSettings({ ...getSettings(), updateLastCheck: now });
    setLastCheck(now);
  };

  const check = async () => {
    setError(null);
    setPhase('checking');
    try {
      const r = await call<UpdateFound>('update.check');
      checked();
      setFound(r);
      setPhase(r.available ? 'found' : 'current');
    } catch (e) {
      setError((e as Error).message);
      setPhase('failed');
    }
  };

  const install = async (back: boolean) => {
    setError(null);
    setPhase('installing');
    let stop: (() => void) | undefined;
    try {
      const { listen } = await import('@tauri-apps/api/event');
      stop = await listen<{ received: number; total: number | null }>('update-progress', (e) => setPercent(progressPercent(e.payload.received, e.payload.total)));
      const r = await call<{ installed: boolean; version?: string }>(back ? 'update.rollback' : 'update.install');
      if (!r.installed) setPhase(back ? 'back_none' : 'current');
    } catch (e) {
      setError((e as Error).message);
      setPhase('failed');
    } finally {
      stop?.();
    }
  };

  const checkBack = async () => {
    setError(null);
    setPhase('back_checking');
    try {
      const r = await call<UpdateFound>('update.rollback_check');
      setFound(r);
      setPhase(r.available ? 'back_found' : 'back_none');
    } catch (e) {
      setError((e as Error).message);
      setPhase('failed');
    }
  };

  const setDailyCheck = (on: boolean) => {
    setDaily(on);
    saveSettings({ ...getSettings(), updateDaily: on });
  };

  const busy = phase === 'checking' || phase === 'installing' || phase === 'back_checking';
  return (
    <Modal
      title="Check for Updates"
      onClose={onClose}
      width={560}
      footer={
        <>
          {phase === 'found' && (
            <button type="button" className="btn primary" onClick={() => install(false)}>
              Download and install {found?.version}
            </button>
          )}
          {phase === 'back_found' && (
            <button type="button" className="btn primary" onClick={() => install(true)}>
              Install {found?.version}
            </button>
          )}
          {desktop && state?.set_up && phase !== 'found' && phase !== 'back_found' && (
            <button type="button" className="btn primary" onClick={check} disabled={busy}>
              {phase === 'checking' ? 'Checking…' : 'Check now'}
            </button>
          )}
          <span style={{ flex: 1 }} />
          <button type="button" className="btn" onClick={onClose} disabled={phase === 'installing'}>
            Close
          </button>
        </>
      }
    >
      {!desktop ? (
        <p>Updates can only be checked in the installed program.</p>
      ) : !state ? (
        error ? <div className="errbox" role="alert">{error}</div> : <p>Reading the version…</p>
      ) : (
        <>
          <p>
            This is PrismStatistics {state.version}.{' '}
            {state.set_up
              ? `A check asks ${state.host} whether a newer version has been published, sending only the program's version and the kind of computer. Nothing is downloaded or installed without your go-ahead.`
              : 'This copy is not set up to check for updates. Download new versions from the project’s releases page.'}
          </p>
          {state.set_up && (
            <>
              <label className="check">
                <input type="checkbox" checked={daily} onChange={(e) => setDailyCheck(e.target.checked)} /> Check for updates once a day
              </label>
              <p className="fieldhint">Off unless you turn it on. When a newer version is found it is only announced; nothing is downloaded. Last checked: {when(lastCheck)}.</p>
            </>
          )}
          <div role="status" aria-live="polite">
            {phase === 'current' && <p>You have the newest version.</p>}
            {phase === 'found' && found && (
              <>
                <p>
                  <b>PrismStatistics {found.version}</b> is available{found.date ? ` (published ${found.date.slice(0, 10)})` : ''}. Installing it downloads the installer, checks
                  that it is signed by the maintainer, copies any unsaved work to the recovery folder, then closes PrismStatistics and runs the installer.
                </p>
                {found.notes && (
                  <details open>
                    <summary>What’s new</summary>
                    <pre className="updatenotes">{found.notes}</pre>
                  </details>
                )}
              </>
            )}
            {phase === 'back_checking' && <p>Looking for the previous version…</p>}
            {phase === 'back_none' && <p>No earlier version is offered.</p>}
            {phase === 'back_found' && found && (
              <p>
                <b>PrismStatistics {found.version}</b>, the previous version, can be installed in place of this one. Its installer is checked against the maintainer's signature in the same way, and
                unsaved work is copied to the recovery folder first.
              </p>
            )}
            {phase === 'installing' && <p>Downloading{percent === null ? '…' : ` (${percent}%)`}. PrismStatistics will close and the installer will start.</p>}
          </div>
          {state.rollback && phase !== 'back_found' && (
            <p>
              <button type="button" className="btn" onClick={checkBack} disabled={busy}>
                Return to the previous version…
              </button>
            </p>
          )}
          {error && (
            <div className="errbox" role="alert">
              {error}
            </div>
          )}
        </>
      )}
    </Modal>
  );
}
