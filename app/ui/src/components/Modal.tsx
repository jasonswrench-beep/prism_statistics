// Small dialogs: a choice among options, a confirmation, and a path prompt (browser mode only).

import { HelpButton } from './HelpDialog';
import { useState, type ReactNode } from 'react';
import { Icon } from './Icon';

export function Modal({ title, children, footer, onClose, width = 480 }: { title: string; children: ReactNode; footer: ReactNode; onClose: () => void; width?: number }) {
  return (
    <div className="overlay open" onKeyDown={(e) => e.key === 'Escape' && onClose()}>
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby="modalTitle" style={{ width: `min(${width}px,100%)` }}>
        <header>
          <h2 id="modalTitle">{title}</h2>
          <HelpButton />
          <button type="button" className="iconbtn x" aria-label="Close" onClick={onClose}>
            <Icon name="close" />
          </button>
        </header>
        <div className="dbody">{children}</div>
        <footer>{footer}</footer>
      </div>
    </div>
  );
}

export function SheetDialog({ fileName, sheets, onPick, onClose }: { fileName: string; sheets: string[]; onPick: (s: string) => void; onClose: () => void }) {
  const [sheet, setSheet] = useState(sheets[0]);
  return (
    <Modal
      title="Choose a sheet"
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <button type="button" className="btn primary" onClick={() => onPick(sheet)}>
            Open sheet
          </button>
        </>
      }
    >
      <p style={{ margin: 0 }}>
        <b>{fileName}</b> has {sheets.length} sheets. Each sheet opens as its own dataset; the first row names the variables.
      </p>
      <div className="choice" role="radiogroup" aria-label="Sheets">
        {sheets.map((s) => (
          <button type="button" key={s} role="radio" aria-checked={sheet === s} aria-pressed={sheet === s} onClick={() => setSheet(s)} onDoubleClick={() => onPick(s)}>
            <Icon name="data" />
            {s}
          </button>
        ))}
      </div>
    </Modal>
  );
}

export function UnsavedDialog({ onSave, onDiscard, onClose }: { onSave: () => void; onDiscard: () => void; onClose: () => void }) {
  return (
    <Modal
      title="Save changes first?"
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <button type="button" className="btn" onClick={onDiscard}>
            Don't save
          </button>
          <button type="button" className="btn primary" onClick={onSave} autoFocus>
            Save project
          </button>
        </>
      }
    >
      <p style={{ margin: 0 }}>This project has changes that haven't been saved. If you continue without saving, they will be lost.</p>
    </Modal>
  );
}

/** Asks for the link of an online spreadsheet shared as "Anyone with the link". No sign-in, ever. */
export function GoogleSheetDialog({ onOpen, onClose }: { onOpen: (url: string) => Promise<void>; onClose: () => void }) {
  const [url, setUrl] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const go = async () => {
    if (!url.trim() || busy) return;
    setBusy(true);
    setError(null);
    try {
      await onOpen(url.trim());
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <Modal
      title="Open an online spreadsheet"
      onClose={onClose}
      width={600}
      footer={
        <>
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <button type="button" className="btn primary" disabled={!url.trim() || busy} onClick={go}>
            {busy ? 'Downloading…' : 'Open'}
          </button>
        </>
      }
    >
      <label className="fieldlabel" htmlFor="gsLink">
        Link to the spreadsheet
      </label>
      <input id="gsLink" className="textinput" autoFocus placeholder="https://docs.google.com/spreadsheets/d/…" value={url} onChange={(e) => setUrl(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && go()} />
      <p className="fieldhint" style={{ marginTop: 10 }}>
        In the spreadsheet, choose <b>Share</b>, set <b>General access</b> to <b>Anyone with the link</b> (Viewer), and copy the link, which starts with https://docs.google.com/spreadsheets/. PrismStatistics downloads a copy; there is no sign-in and nothing else is sent. Later changes to the sheet don’t appear until you open the link again.
      </p>
      {error && (
        <div className="errbox" role="alert" style={{ marginTop: 10 }}>
          {error}
        </div>
      )}
    </Modal>
  );
}

export function PathDialog({ title, action, initial, onSubmit, onClose }: { title: string; action: string; initial: string; onSubmit: (path: string) => void; onClose: () => void }) {
  const [path, setPath] = useState(initial);
  return (
    <Modal
      title={title}
      onClose={onClose}
      width={560}
      footer={
        <>
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <button type="button" className="btn primary" disabled={!path.trim()} onClick={() => onSubmit(path.trim())}>
            {action}
          </button>
        </>
      }
    >
      <label className="fieldlabel" htmlFor="pathInput">
        File path <span className="fieldhint">the desktop app shows the usual file dialog here</span>
      </label>
      <input id="pathInput" className="textinput" autoFocus value={path} onChange={(e) => setPath(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && path.trim() && onSubmit(path.trim())} />
    </Modal>
  );
}
