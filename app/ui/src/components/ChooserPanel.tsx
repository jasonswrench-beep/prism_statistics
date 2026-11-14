// Help me choose (Analyze and Help menus, the toolbar, Ctrl+Shift+H): a docked panel that asks about the study's
// design, suggests an analysis with the reason and its alternatives, and opens that analysis's dialog with the
// variables the user picks here already in their roles.

import { useState } from 'react';
import { LEVELS, LEVELS_NOTE, NODES, isEnding, startArgs, type Ending, type Question, type Slot, type SlotKey, type Args } from '../chooser';
import { helpFor } from '../help';
import type { DatasetInfo, VariableInfo } from '../types';
import { Icon } from './Icon';

function SlotPicker({ slot, vars, value, onChange }: { slot: Slot; vars: VariableInfo[]; value: string[]; onChange: (v: string[]) => void }) {
  const fits = vars.filter((v) => !slot.numeric || v.type === 'numeric');
  const preferred = (v: VariableInfo) => (slot.prefer === 'scale' ? v.measure === 'scale' : slot.prefer === 'categorical' ? v.measure !== 'scale' : true);
  const ordered = [...fits.filter(preferred), ...fits.filter((v) => !preferred(v))];
  // The measure from Variable View, so the level shows on the user's own variables.
  const name = (v: VariableInfo) => `${v.label ? `${v.name} (${v.label})` : v.name} · ${v.measure}`;
  const id = `slot-${slot.key}`;
  if (!slot.many) {
    return (
      <label className="slot" htmlFor={id}>
        <span>{slot.label}</span>
        <select id={id} value={value[0] ?? ''} onChange={(e) => onChange(e.target.value ? [e.target.value] : [])}>
          <option value="">Choose in the dialog</option>
          {ordered.map((v) => (
            <option key={v.id} value={v.name}>
              {name(v)}
            </option>
          ))}
        </select>
      </label>
    );
  }
  return (
    <fieldset className="slot">
      <legend>{slot.label}</legend>
      <div className="slotlist">
        {ordered.map((v) => (
          <label key={v.id}>
            <input type="checkbox" checked={value.includes(v.name)} onChange={(e) => onChange(e.target.checked ? [...value, v.name] : value.filter((n) => n !== v.name))} />
            {name(v)}
          </label>
        ))}
        {!ordered.length && <span className="muted">No variables of this kind.</span>}
      </div>
    </fieldset>
  );
}

export function ChooserPanel({ dataset, onOpen, onHelp, onClose }: { dataset: DatasetInfo | null; onOpen: (dialog: string, args: Args) => void; onHelp: (id: string) => void; onClose: () => void }) {
  // The questions answered so far and the node they led to; the last entry is the one shown.
  const [path, setPath] = useState<{ id: string; answer?: string }[]>([{ id: 'start' }]);
  const [chosen, setChosen] = useState<Partial<Record<SlotKey, string[]>>>({});
  const node = NODES[path[path.length - 1].id];
  const vars = dataset?.variables ?? [];
  const measure = (n: string) => vars.find((v) => v.name === n)?.measure;

  const answer = (q: Question, label: string, next: string) => setPath((p) => [...p.slice(0, -1), { id: q.id, answer: label }, { id: next }]);
  const switchTo = (id: string) => setPath((p) => [...p.slice(0, -1), { id }]);

  const ending = (e: Ending) => {
    const help = helpFor(e.dialog);
    const usable = e.noData || !!dataset;
    return (
      <section className="chooseend" aria-labelledby="chooseTitle">
        <p className="eyebrow">Suggested analysis</p>
        <h3 id="chooseTitle">{e.title}</h3>
        <p>{e.why}</p>
        {e.check && (
          <div className="callout">
            <Icon name="info" />
            <div>
              <b>Check: </b>
              {e.check}
            </div>
          </div>
        )}
        {e.slots.length > 0 &&
          (dataset ? (
            <div className="slots">
              <p className="muted">Pick the variables now, or leave them for the dialog.</p>
              {e.slots.map((s) => (
                <SlotPicker key={s.key} slot={s} vars={vars} value={chosen[s.key] ?? []} onChange={(v) => setChosen((c) => ({ ...c, [s.key]: v }))} />
              ))}
            </div>
          ) : (
            <p className="muted">Open data to run it. You can still read about it.</p>
          ))}
        <div className="chooseactions">
          <button type="button" className="btn primary" disabled={!usable} onClick={() => onOpen(e.dialog, startArgs(e, chosen, measure))}>
            Open {e.title}
          </button>
          {help && (
            <button type="button" className="btn" onClick={() => onHelp(help.id)}>
              About this analysis
            </button>
          )}
        </div>
        {e.also && e.also.length > 0 && (
          <div className="alsolist">
            <p className="eyebrow">Also consider</p>
            <ul>
              {e.also.map((id) => {
                const alt = NODES[id] as Ending;
                return (
                  <li key={id}>
                    <button type="button" className="linkbtn" onClick={() => switchTo(id)}>
                      {alt.title}
                    </button>
                  </li>
                );
              })}
            </ul>
          </div>
        )}
      </section>
    );
  };

  const question = (q: Question) => (
    <fieldset className="choosequestion">
      <legend>{q.ask}</legend>
      {q.help && <p className="muted">{q.help}</p>}
      <div className="answers">
        {q.answers.map((a) => (
          <button type="button" key={a.label} className="answer" onClick={() => answer(q, a.label, a.next)}>
            <span>{a.label}</span>
            {a.hint && <small>{a.hint}</small>}
          </button>
        ))}
      </div>
      {q.levels && (
        <details className="levels">
          <summary>Not sure which?</summary>
          <dl>
            {LEVELS.map((l) => (
              <div key={l.level}>
                <dt>{l.level}</dt>
                <dd>
                  {l.means} <span className="muted">For example: {l.examples}.</span>
                </dd>
              </div>
            ))}
          </dl>
          <p>{LEVELS_NOTE}</p>
          {dataset && <p className="muted">The variable lists show each variable’s measure from Variable View.</p>}
        </details>
      )}
    </fieldset>
  );

  return (
    <aside className="dock chooser" aria-label="Help me choose" onKeyDown={(e) => e.key === 'Escape' && onClose()}>
      <header>
        <Icon name="bulb" />
        Help me choose
        <button type="button" className="iconbtn x" aria-label="Close Help me choose" onClick={onClose}>
          <Icon name="close" />
        </button>
      </header>
      <div className="dockbody">
        {path.length > 1 && (
          <ol className="choosetrail" aria-label="Your answers">
            {path.slice(0, -1).map((step, i) => (
              <li key={step.id}>
                <span className="muted">{(NODES[step.id] as Question).ask}</span>
                <span className="ans">{step.answer}</span>
                <button type="button" className="linkbtn" aria-label={`Change: ${(NODES[step.id] as Question).ask}`} onClick={() => setPath((p) => [...p.slice(0, i), { id: step.id }])}>
                  Change
                </button>
              </li>
            ))}
          </ol>
        )}
        {isEnding(node) ? ending(node) : question(node)}
        {path.length > 1 && (
          <button type="button" className="linkbtn startover" onClick={() => setPath([{ id: 'start' }])}>
            Start over
          </button>
        )}
      </div>
    </aside>
  );
}
