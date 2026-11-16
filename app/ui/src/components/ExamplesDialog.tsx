// File › Open Example Data: the built-in teaching datasets, each with what it holds and what to try with it.

import { useEffect, useState } from 'react';
import { call } from '../bridge';
import { Modal } from './Modal';

export interface Example {
  id: string;
  title: string;
  description: string;
  rows: number;
  variables: number;
  try: string[];
  /** Also offered as a blank template (the variables and labels, no cases). */
  template?: boolean;
}

export function ExamplesDialog({ onClose, onOpen, onWorked }: { onClose: () => void; onOpen: (id: string, blank?: boolean) => void; onWorked: () => void }) {
  const [list, setList] = useState<Example[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    call<Example[]>('examples').then(setList, (e: Error) => setError(e.message));
  }, []);
  return (
    <Modal
      title="Open Example Data"
      onClose={onClose}
      width={640}
      footer={
        <>
          <button type="button" className="btn" onClick={onWorked}>
            Worked examples…
          </button>
          <span style={{ flex: 1 }} />
          <button type="button" className="btn" onClick={onClose}>
            Close
          </button>
        </>
      }
    >
      <p className="fieldhint">Simulated data for learning and teaching, with variable labels, value labels and missing-value codes already set. An example opens as new, unsaved data; your own files are untouched.</p>
      {error && <div className="errbox" role="alert">{error}</div>}
      <div className="examples">
        {(list ?? []).map((x) => (
          <section className="example" key={x.id} aria-labelledby={`ex-${x.id}`}>
            <div className="examplehead">
              <h3 id={`ex-${x.id}`}>{x.title}</h3>
              <span className="muted">
                {x.rows} cases · {x.variables} variables
              </span>
            </div>
            <p>{x.description}</p>
            <ul>
              {x.try.map((t) => (
                <li key={t}>{t}</li>
              ))}
            </ul>
            <button type="button" className="btn primary" onClick={() => onOpen(x.id)}>
              Open {x.title}
            </button>
            {x.template && (
              <button type="button" className="btn" style={{ marginLeft: 8 }} onClick={() => onOpen(x.id, true)}>
                Blank template
              </button>
            )}
          </section>
        ))}
      </div>
    </Modal>
  );
}
