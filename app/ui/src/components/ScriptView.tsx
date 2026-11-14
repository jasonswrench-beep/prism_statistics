// The Script screen: commands pasted from the dialogs (Paste) or taken from the project's history, run in order with
// Run all. A script is plain text, one command per line (the command, then its arguments as JSON); lines starting with
// # are comments. Saved scripts run without the window too: prism run study.pscript --data exam.sav --out report.docx.

import { useRef, useState } from 'react';
import { call, inTauri } from '../bridge';
import { download } from '../files';
import type { OutputItem } from '../types';
import { Icon } from './Icon';
import { Modal } from './Modal';

/** A file a script would write, as script_outputs lists it. */
interface Output {
  line: number;
  command: string;
  path: string;
  exists: boolean;
  refused: string | null;
}

interface RunReply {
  ran: number;
  ids: number[];
  error: { line: number; message: string } | null;
  results: OutputItem[];
}

export function ScriptView({ text, setText, onRan, say }: { text: string; setText: (t: string) => void; onRan: (results: OutputItem[]) => void; say: (msg: string) => void }) {
  const [error, setError] = useState<{ line?: number; message: string } | null>(null);
  const [running, setRunning] = useState(false);
  // The files the script would write, shown before it runs.
  const [outputs, setOutputs] = useState<Output[] | null>(null);
  const [replace, setReplace] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);
  const areaRef = useRef<HTMLTextAreaElement>(null);
  const commands = text.split('\n').filter((l) => l.trim() && !l.trim().startsWith('#')).length;

  /** Run all: when the script writes files, list them first, and replace an existing one only if the user agrees. */
  const run = async () => {
    setError(null);
    try {
      const o = await call<Output[]>('script_outputs', { text });
      if (o.length) {
        setReplace(false);
        setOutputs(o);
        return;
      }
    } catch (e) {
      setError({ message: (e as Error).message });
      return;
    }
    await runNow([]);
  };

  const runNow = async (agreed: string[]) => {
    setOutputs(null);
    setRunning(true);
    setError(null);
    try {
      const r = await call<RunReply>('run_script', { text, replace: agreed });
      onRan(r.results);
      if (r.error) {
        setError(r.error);
        // Put the cursor on the line that failed.
        const area = areaRef.current;
        if (area) {
          const start = text.split('\n').slice(0, r.error.line - 1).join('\n').length + (r.error.line > 1 ? 1 : 0);
          area.focus();
          area.setSelectionRange(start, start + (text.split('\n')[r.error.line - 1]?.length ?? 0));
        }
      } else say(`Ran ${r.ran} command${r.ran === 1 ? '' : 's'}; ${r.ids.length} new result${r.ids.length === 1 ? '' : 's'}.`);
    } catch (e) {
      setError({ message: (e as Error).message });
    } finally {
      setRunning(false);
    }
  };

  const history = async () => {
    try {
      const r = await call<{ text: string }>('history_script', {});
      setText(text.trim() ? `${text.replace(/\n*$/, '\n\n')}${r.text}` : r.text);
    } catch (e) {
      setError({ message: (e as Error).message });
    }
  };

  const save = async () => {
    try {
      if (inTauri()) {
        const { save } = await import('@tauri-apps/plugin-dialog');
        const path = await save({ defaultPath: 'analysis.pscript', filters: [{ name: 'PrismStatistics script', extensions: ['pscript'] }] });
        if (!path) return;
        const r = await call<{ path: string }>('write_script', { path, text });
        say(`Saved the script to ${r.path}.`);
      } else {
        download('analysis.pscript', btoa(unescape(encodeURIComponent(text))), 'text/plain');
      }
    } catch (e) {
      setError({ message: (e as Error).message });
    }
  };

  const open = async () => {
    try {
      if (inTauri()) {
        const { open } = await import('@tauri-apps/plugin-dialog');
        const path = await open({ multiple: false, directory: false, filters: [{ name: 'PrismStatistics script', extensions: ['pscript', 'txt'] }] });
        if (typeof path !== 'string') return;
        setText((await call<{ text: string }>('read_script', { path })).text);
      } else fileRef.current?.click();
    } catch (e) {
      setError({ message: (e as Error).message });
    }
  };

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      say('Copied the script.');
    } catch {
      areaRef.current?.select();
    }
  };

  return (
    <div className="scriptview">
      <div className="scriptbar">
        <button type="button" className="btn primary" onClick={run} disabled={running || !commands}>
          <Icon name="run" />
          {running ? 'Running…' : 'Run all'}
        </button>
        <button type="button" className="btn" onClick={history}>
          Add the project&rsquo;s history
        </button>
        <button type="button" className="btn" onClick={open}>
          Open…
        </button>
        <button type="button" className="btn" onClick={save} disabled={!text.trim()}>
          Save…
        </button>
        <button type="button" className="btn" onClick={copy} disabled={!text.trim()}>
          Copy
        </button>
        <button type="button" className="btn" onClick={() => (setText(''), setError(null))} disabled={!text.trim()}>
          Clear
        </button>
        <span className="muted">{commands} command{commands === 1 ? '' : 's'}</span>
      </div>
      <p className="fieldhint" style={{ margin: '0 0 8px' }}>
        Use <b>Paste</b> in any analysis dialog to add its command here without running it. One command per line; lines starting with # are notes. A saved script also runs without the window: <code>prism run analysis.pscript --data mydata.sav --out report.docx</code>.
      </p>
      {error && (
        <p role="alert" className="errbox">
          {error.line ? `Line ${error.line}: ` : ''}
          {error.message}
          {error.line ? ' The lines before it ran; nothing after it did.' : ''}
        </p>
      )}
      <textarea ref={areaRef} className="scripttext" aria-label="Script" spellCheck={false} value={text} onChange={(e) => setText(e.target.value)} placeholder="# Paste analyses here from their dialogs, or add the project's history." />
      <input
        ref={fileRef}
        type="file"
        accept=".pscript,.txt,text/plain"
        hidden
        aria-label="Choose a script"
        onChange={async (e) => {
          const f = e.target.files?.[0];
          if (f) setText(await f.text());
          e.target.value = '';
        }}
      />
      {outputs && (
        <Modal
          title="This script writes files"
          onClose={() => setOutputs(null)}
          width={620}
          footer={
            <>
              <button type="button" className="btn" onClick={() => setOutputs(null)}>
                Cancel
              </button>
              <button
                type="button"
                className="btn primary"
                disabled={outputs.some((o) => o.refused) || (outputs.some((o) => o.exists) && !replace)}
                onClick={() => runNow(replace ? outputs.filter((o) => o.exists).map((o) => o.path) : [])}
              >
                Run all
              </button>
            </>
          }
        >
          <p>Before it runs, check where the script will save:</p>
          <ul className="outputs">
            {outputs.map((o) => (
              <li key={`${o.line}-${o.path}`}>
                <code>{o.path}</code> <span className="fieldhint">line {o.line}, {o.command === 'export' ? 'export' : 'project'}</span>
                {o.exists && (
                  <>
                    {' '}
                    <span className="status review">already exists</span>
                  </>
                )}
                {o.refused && <div className="fieldhint err">{o.refused}</div>}
              </li>
            ))}
          </ul>
          {outputs.some((o) => o.exists) && (
            <label className="switch">
              <input type="checkbox" checked={replace} onChange={(e) => setReplace(e.target.checked)} />
              Replace the files that already exist
            </label>
          )}
          <p className="fieldhint">A script never replaces a file you haven't agreed to here. Check that every path is one you expect, especially in a script you didn't write.</p>
        </Modal>
      )}
    </div>
  );
}
