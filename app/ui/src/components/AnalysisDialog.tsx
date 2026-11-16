// The variable-transfer dialog (FAM-04) for every analysis described in analyses.ts: a source list on the left, one box
// per role on the right with an arrow button each, two-group pickers chosen by code, reference categories, and the
// options with their defaults. Double-click (or Enter twice) moves a variable into the first box that takes it.

import { HelpButton } from './HelpDialog';
import { useEffect, useMemo, useState } from 'react';
import { call } from '../bridge';
import { optionDefault, optionShown, problem, visibleRoles, type AnalysisDef, type Collected, type OptionValue, type RoleDef } from '../analyses';
import type { DatasetInfo, GroupCode, OutputItem, VariableInfo } from '../types';
import { Icon } from './Icon';

function defaults(def: AnalysisDef): Record<string, OptionValue> {
  return Object.fromEntries(def.options.map((o) => [o.key, optionDefault(o)]));
}

export function AnalysisDialog({ def, dataset, initial, onClose, onDone, onPaste }: { def: AnalysisDef; dataset: DatasetInfo; initial?: Collected; onClose: () => void; onDone: (items: OutputItem[]) => void; onPaste?: (command: string, args: unknown) => void }) {
  const byName = useMemo(() => new Map(dataset.variables.map((v) => [v.name, v])), [dataset]);
  const [selected, setSelected] = useState<string | null>(null);
  // A reopened result keeps only the variables that still exist.
  const keep = (names: string[]) => names.filter((n) => dataset.variables.some((v) => v.name === n));
  const [roles, setRoles] = useState<Record<string, string[]>>(() => Object.fromEntries(Object.entries(initial?.roles ?? {}).map(([k, v]) => [k, keep(v)])));
  const [pairs, setPairs] = useState<Record<string, [string, string][]>>(() => Object.fromEntries(Object.entries(initial?.pairs ?? {}).map(([k, v]) => [k, v.filter((p) => keep(p).length === 2)])));
  const [pending, setPending] = useState<Record<string, string | null>>({});
  const [codes, setCodes] = useState<Record<string, GroupCode[]>>({});
  const [codeErrors, setCodeErrors] = useState<Record<string, string>>({});
  const [groups, setGroups] = useState<Record<string, [number, number]>>(initial?.groups ?? {});
  const [references, setReferences] = useState<Record<string, Record<string, string>>>(initial?.references ?? {});
  const [options, setOptions] = useState<Record<string, OptionValue>>(() => ({ ...defaults(def), ...(initial?.options ?? {}) }));
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);

  const used = new Set([...Object.values(roles).flat()]);
  const accepts = (r: RoleDef, v: VariableInfo) => r.accepts === 'any' || v.type === 'numeric';
  const shown = visibleRoles(def, options);
  const source = dataset.variables.filter((v) => !used.has(v.name) && shown.some((r) => accepts(r, v)));

  // Group codes for two-group pickers and reference categories, fetched whenever a variable lands in such a role.
  const needCodes = def.roles.filter((r) => r.twoGroups || r.reference || r.event).flatMap((r) => roles[r.key] ?? []);
  const needKey = needCodes.join('\u0001');
  useEffect(() => {
    for (const name of needCodes) {
      if (codes[name] || codeErrors[name]) continue;
      if (byName.get(name)?.type !== 'numeric') {
        setCodes((c) => ({ ...c, [name]: [] }));
        continue;
      }
      call<GroupCode[]>('group_codes', { name })
        .then((c) => setCodes((all) => ({ ...all, [name]: c })))
        .catch((e: Error) => setCodeErrors((all) => ({ ...all, [name]: e.message })));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [needKey]);
  useEffect(() => {
    for (const r of def.roles.filter((r) => r.twoGroups)) {
      const name = roles[r.key]?.[0];
      const c = name ? codes[name] : undefined;
      if (c && c.length >= 2 && !groups[r.key]) setGroups((g) => ({ ...g, [r.key]: [c[0].code, c[1].code] }));
    }
  }, [codes, roles, def.roles, groups]);

  const moveTo = (r: RoleDef, name: string | null = selected) => {
    const v = name ? byName.get(name) : undefined;
    if (!v || used.has(v.name) || !accepts(r, v)) return false;
    if (r.kind === 'pairs') {
      const first = pending[r.key];
      if (!first) setPending({ ...pending, [r.key]: v.name });
      else if (first !== v.name) {
        setPairs({ ...pairs, [r.key]: [...(pairs[r.key] ?? []), [first, v.name]] });
        setPending({ ...pending, [r.key]: null });
      }
      setSelected(null);
      return true;
    }
    const now = roles[r.key] ?? [];
    if (r.kind === 'one' && now.length) return false;
    setRoles({ ...roles, [r.key]: [...now, v.name] });
    setSelected(null);
    return true;
  };
  const moveFirst = (name: string) => shown.some((r) => moveTo(r, name));
  const remove = (r: RoleDef, name: string) => {
    setRoles({ ...roles, [r.key]: (roles[r.key] ?? []).filter((n) => n !== name) });
    if (r.twoGroups) setGroups(({ [r.key]: _, ...rest }) => rest);
  };

  // Hidden roles send nothing.
  const collected: Collected = { roles: Object.fromEntries(def.roles.map((r) => [r.key, shown.includes(r) ? (roles[r.key] ?? []) : []])), pairs, groups, references, options };
  const firstProblem = problem(def, collected) ?? (def.roles.some((r) => r.kind === 'pairs' && pending[r.key]) ? 'Choose the second variable of the pair.' : null);
  const codeError = needCodes.map((n) => codeErrors[n]).find(Boolean);

  const run = async () => {
    if (firstProblem || codeError) return;
    setRunning(true);
    try {
      onDone(await call<OutputItem[]>(def.command, def.build(collected)));
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setRunning(false);
    }
  };

  const reset = () => {
    setRoles({});
    setPairs({});
    setPending({});
    setGroups({});
    setReferences({});
    setOptions(defaults(def));
    setError(null);
  };

  const varRow = (v: VariableInfo, onPick: () => void, picked: boolean, onDouble?: () => void) => (
    <li
      key={v.id}
      role="option"
      aria-label={v.label ? `${v.name}, ${v.label}` : v.name}
      aria-selected={picked}
      tabIndex={0}
      onClick={onPick}
      onDoubleClick={onDouble}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          if (picked && onDouble) onDouble();
          else onPick();
        }
      }}
    >
      <span>{v.name}</span>
      <span className="lab">{v.label}</span>
      <span className="measure">
        <Icon name={v.measure} />
      </span>
    </li>
  );
  const codeLabel = (c: GroupCode) => `${c.label ?? c.code} (${c.code}, n = ${c.count})`;
  const id = (s: string) => `${def.id}-${s}`;

  const roleBox = (r: RoleDef) => {
    const names = roles[r.key] ?? [];
    return (
      <div key={r.key} className="rolerow">
        <button type="button" className="arrowbtn" aria-label={`Move to ${r.label}`} onClick={() => moveTo(r)} disabled={!selected}>
          <Icon name="right" />
        </button>
        <div className="rolebox">
          <span className="fieldlabel" id={id(r.key)}>
            {r.label} {r.hint && <span className="fieldhint">{r.hint}</span>}
          </span>
          {r.kind === 'pairs' ? (
            <ul className="listbox" role="listbox" aria-labelledby={id(r.key)} style={{ minHeight: 96 }}>
              {(pairs[r.key] ?? []).map(([a, b], i) => (
                <li key={`${a}-${b}`} role="option" aria-selected={false} tabIndex={0} aria-label={`Pair ${i + 1}: ${a} and ${b}. Press Enter to remove.`} onClick={() => setPairs({ ...pairs, [r.key]: (pairs[r.key] ?? []).filter((_, j) => j !== i) })} onKeyDown={(e) => e.key === 'Enter' && setPairs({ ...pairs, [r.key]: (pairs[r.key] ?? []).filter((_, j) => j !== i) })}>
                  <span>
                    {a} − {b}
                  </span>
                  <span className="lab">pair {i + 1}</span>
                </li>
              ))}
              {pending[r.key] && (
                <li className="empty" aria-live="polite">
                  {pending[r.key]} − <i>choose the second variable</i>
                </li>
              )}
              {!(pairs[r.key] ?? []).length && !pending[r.key] && <li className="empty">Move two variables, first then second</li>}
            </ul>
          ) : (
            <ul className="listbox" role="listbox" aria-labelledby={id(r.key)} style={{ minHeight: r.kind === 'one' ? 0 : 96 }}>
              {names.length ? names.map((n) => varRow(byName.get(n)!, () => remove(r, n), false)) : <li className="empty">{r.accepts === 'numeric' ? 'A numeric variable' : 'Any variable'}</li>}
            </ul>
          )}
          {r.twoGroups &&
            names.map((n) =>
              codeErrors[n] ? null : (codes[n]?.length ?? 0) >= 2 ? (
                <div className="groupsdef" key={n}>
                  {[0, 1].map((k) => (
                    <label key={k}>
                      Group {k + 1}{' '}
                      <select value={groups[r.key]?.[k] ?? ''} onChange={(e) => setGroups({ ...groups, [r.key]: (k === 0 ? [Number(e.target.value), groups[r.key]?.[1] ?? 0] : [groups[r.key]?.[0] ?? 0, Number(e.target.value)]) as [number, number] })}>
                        {codes[n].map((c) => (
                          <option key={c.code} value={c.code}>
                            {codeLabel(c)}
                          </option>
                        ))}
                      </select>
                    </label>
                  ))}
                </div>
              ) : null,
            )}
          {r.reference &&
            names.map((n) =>
              (codes[n]?.length ?? 0) >= 2 ? (
                <label className="groupsdef" key={n}>
                  Reference for {n}{' '}
                  <select value={references[r.key]?.[n] ?? String(codes[n][0].code)} onChange={(e) => setReferences({ ...references, [r.key]: { ...(references[r.key] ?? {}), [n]: e.target.value } })}>
                    {codes[n].map((c) => (
                      <option key={c.code} value={String(c.code)}>
                        {codeLabel(c)}
                      </option>
                    ))}
                  </select>
                </label>
              ) : null,
            )}
          {r.event &&
            names.map((n) =>
              (codes[n]?.length ?? 0) >= 2 ? (
                <label className="groupsdef" key={n}>
                  Category modeled (the event){' '}
                  <select value={references[r.key]?.[n] ?? String(codes[n][codes[n].length - 1].code)} onChange={(e) => setReferences({ ...references, [r.key]: { ...(references[r.key] ?? {}), [n]: e.target.value } })}>
                    {codes[n].map((c) => (
                      <option key={c.code} value={String(c.code)}>
                        {codeLabel(c)}
                      </option>
                    ))}
                  </select>
                </label>
              ) : null,
            )}
          {names.length > 0 && r.kind !== 'one' && <span className="fieldhint">Click a variable to move it back.</span>}
        </div>
      </div>
    );
  };

  return (
    <div className="overlay open" onKeyDown={(e) => e.key === 'Escape' && onClose()}>
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby={id('title')}>
        <header>
          <h2 id={id('title')}>{def.title}</h2>
          <HelpButton />
          <button type="button" className="iconbtn x" aria-label="Close" onClick={onClose}>
            <Icon name="close" />
          </button>
        </header>
        <div className="dbody">
          <div className="transfer two">
            <div>
              <span className="fieldlabel" id={id('src')}>
                Variables
              </span>
              <ul className="listbox tall" role="listbox" aria-labelledby={id('src')}>
                {source.length ? source.map((v) => varRow(v, () => setSelected(v.name), selected === v.name, () => moveFirst(v.name))) : <li className="empty">No variables left</li>}
              </ul>
            </div>
            <div className="roles">{shown.map(roleBox)}</div>
          </div>

          <OptionList def={def} options={options} setOptions={setOptions} />

          {error && (
            <div className="errbox" role="alert">
              {error}
            </div>
          )}
          {codeError && (
            <div className="inline-err" role="alert">
              <Icon name="warn" />
              {codeError}
            </div>
          )}
          {firstProblem && !codeError && (
            <div className="inline-err" aria-live="polite">
              <Icon name="info" />
              {firstProblem}
            </div>
          )}
        </div>
        <footer>
          <button type="button" className="btn" onClick={reset}>
            Reset
          </button>
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          {onPaste && (
            <button type="button" className="btn" disabled={!!firstProblem || !!codeError} onClick={() => onPaste(def.command, def.build(collected))} title="Add this analysis to the script without running it">
              Paste
            </button>
          )}
          <button type="button" className="btn primary" disabled={!!firstProblem || !!codeError || running} onClick={run}>
            <Icon name="run" />
            {running ? 'Running…' : 'Run'}
          </button>
        </footer>
      </div>
    </div>
  );
}

/** The options of an analysis, each in a collapsible section with its current value in the summary. */
export function OptionList({ def, options, setOptions, skip = [] }: { def: AnalysisDef; options: Record<string, OptionValue>; setOptions: (o: Record<string, OptionValue>) => void; skip?: string[] }) {
  const id = (s: string) => `${def.id}-${s}`;
  return (
    <>
    {def.options.filter((o) => !skip.includes(o.key) && optionShown(o, options)).map((o) => {
      const v = options[o.key];
      const set = (x: OptionValue) => setOptions({ ...options, [o.key]: x });
      let current = '';
      if (o.type === 'radio') current = o.choices.find((c) => c.value === v)?.label ?? '';
      if (o.type === 'checks') current = o.choices.filter((c) => (v as string[]).includes(c.value)).map((c) => c.label).join(', ') || 'none';
      if (o.type === 'number' || o.type === 'text') current = String(v);
      if (o.type === 'textarea') current = String(v).split('\n').filter((l) => l.trim()).length + ' lines';
      if (o.type === 'toggle') current = v ? 'On' : 'Off';
      return (
        <details className="opt" key={o.key} open={o.type === 'radio' || o.type === 'textarea'}>
          <summary>
            {o.label} <span className="cur">{current}</span>
          </summary>
          <div className="optbody" role={o.type === 'radio' ? 'radiogroup' : undefined} aria-label={o.label}>
            {o.type === 'radio' &&
              o.choices.map((c) => (
                <label className="radio" key={c.value}>
                  <input type="radio" name={id(o.key)} checked={v === c.value} onChange={() => set(c.value)} />
                  <span>
                    <b>{c.label}</b>
                    {c.hint && `: ${c.hint}`}
                  </span>
                </label>
              ))}
            {o.type === 'checks' &&
              o.choices.map((c) => (
                <label className="check" key={c.value}>
                  <input type="checkbox" checked={(v as string[]).includes(c.value)} onChange={(e) => set(e.target.checked ? [...(v as string[]), c.value] : (v as string[]).filter((x) => x !== c.value))} />
                  <span>
                    {c.label}
                    {c.hint && <span className="fieldhint"> ({c.hint})</span>}
                  </span>
                </label>
              ))}
            {o.type === 'number' && (
              <>
                <label>
                  {o.label}{' '}
                  <input className="textinput" inputMode="decimal" style={{ width: 90 }} value={v as string} onChange={(e) => set(e.target.value)} />
                </label>
                {o.hint && <span className="fieldhint">{o.hint}</span>}
              </>
            )}
            {o.type === 'text' && (
              <>
                <label>
                  {o.label}{' '}
                  <input className="textinput" style={{ width: 220 }} value={v as string} onChange={(e) => set(e.target.value)} />
                </label>
                {o.hint && <span className="fieldhint">{o.hint}</span>}
              </>
            )}
            {o.type === 'textarea' && (
              <>
                <label className="fieldlabel" htmlFor={id(o.key)}>
                  {o.label}
                </label>
                <textarea id={id(o.key)} className="textinput code" rows={8} spellCheck={false} style={{ width: '100%', fontFamily: 'var(--mono, monospace)' }} value={v as string} onChange={(e) => set(e.target.value)} />
                {o.hint && <span className="fieldhint">{o.hint}</span>}
              </>
            )}
            {o.type === 'toggle' && (
              <label className="check">
                <input type="checkbox" checked={v as boolean} onChange={(e) => set(e.target.checked)} />
                <span>
                  {o.label}
                  {o.hint && <span className="fieldhint"> {o.hint}</span>}
                </span>
              </label>
            )}
          </div>
        </details>
      );
    })}
    </>
  );
}
