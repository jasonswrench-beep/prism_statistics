// Tools › Connect an AI assistant (D15): the switch for the MCP connection, the private-rows switch, and the setting
// to paste into the assistant. Off by default. The assistant starts its own copy of PrismStatistics in the background.

import { useEffect, useRef, useState } from 'react';
import { call } from '../bridge';
import { Modal } from './Modal';

interface McpStatus {
  enabled: boolean;
  rows: boolean;
  command: string;
  args: string[];
  config: string;
}

export function AssistantDialog({ onClose }: { onClose: () => void }) {
  const [s, setS] = useState<McpStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const box = useRef<HTMLTextAreaElement>(null);
  const act = async (cmd: string, args: unknown = {}) => {
    setError(null);
    try {
      setS(await call<McpStatus>(cmd, args));
    } catch (e) {
      setError((e as Error).message);
    }
  };
  useEffect(() => {
    act('mcp_status');
  }, []);
  const copy = async () => {
    if (!s) return;
    try {
      await navigator.clipboard.writeText(s.config);
      setCopied(true);
    } catch {
      box.current?.select();
    }
  };
  return (
    <Modal title="Connect an AI assistant" onClose={onClose} width={640} footer={<button type="button" className="btn primary" onClick={onClose}>Done</button>}>
      <p style={{ marginTop: 0 }}>
        An AI assistant that supports the Model Context Protocol (MCP) can use PrismStatistics to open data files, run analyses and export the results. It gets the same numbers you see here, already rounded the APA way, so it doesn't need to calculate anything itself.
      </p>
      <p>
        The assistant's provider decides where what it reads goes. Check its privacy terms: if it runs online, the variable names, labels and results it reads are sent to that service.
      </p>
      {s && (
        <>
          <label className="check" style={{ margin: '12px 0' }}>
            <input type="checkbox" checked={s.enabled} onChange={(e) => act('mcp_set', { enabled: e.target.checked })} />
            <span>
              <b>Allow AI assistants to use PrismStatistics</b> <span className="fieldhint">(off by default; turning it off stops a connected assistant at once)</span>
            </span>
          </label>
          <label className="check" style={{ margin: '12px 0' }}>
            <input type="checkbox" checked={s.rows} disabled={!s.enabled} onChange={(e) => act('mcp_set', { rows: e.target.checked })} />
            <span>
              <b>Also let the assistant read individual data rows</b> <span className="fieldhint">(off by default; without it the assistant sees variable information and results, never single cases)</span>
            </span>
          </label>
          <h3 style={{ fontSize: 14, marginBottom: 6 }}>Add PrismStatistics to your assistant</h3>
          <p style={{ marginTop: 0 }}>
            In your assistant's MCP or connector settings, add a local (stdio) server with this command and argument, or paste this into its configuration file:
          </p>
          <textarea ref={box} readOnly value={s.config} rows={8} aria-label="Assistant configuration" style={{ width: '100%', font: '12px var(--mono)', resize: 'vertical' }} onFocus={(e) => e.currentTarget.select()} />
          <div style={{ display: 'flex', gap: 10, alignItems: 'center', marginTop: 8 }}>
            <button type="button" className="btn" onClick={copy}>
              Copy
            </button>
            <span aria-live="polite" className="fieldhint">
              {copied ? 'Copied.' : ''}
            </span>
          </div>
          <p className="fieldhint">
            The assistant works in its own session. To see its results here, ask it to save a project (.pstat) and open that file. It writes files only where you or it name them, and asks before replacing one.
          </p>
        </>
      )}
      {error && (
        <p role="alert" className="errbox">
          {error}
        </p>
      )}
    </Modal>
  );
}
