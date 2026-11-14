// Analyze › Structural Equation Modeling › Factor Analysis of a Scale (CFA): the quick way to run a confirmatory
// factor analysis of scale items. Say how many factors there are, name them, and put each item in its factor's box
// (drag, double-click, or select and use the factor's Move button); the screen writes the model and runs it. Open in
// Model Builder carries the same model to the full builder for adjustments (a cross-loading, a correlated residual).

import { HelpButton } from './HelpDialog';
import { useMemo, useState } from 'react';
import { call } from '../bridge';
import { ANALYSES, type OptionValue } from '../analyses';
import type { DatasetInfo, OutputItem, VariableInfo } from '../types';
import { Icon } from './Icon';
import { cfaModel, validName, type Factor } from '../cfa';

export function CfaDialog({ dataset, onClose, onDone, onPaste, onOpenBuilder }: { dataset: DatasetInfo; onClose: () => void; onDone: (items: OutputItem[]) => void; onPaste?: (command: string, args: unknown) => void; onOpenBuilder: (args: Record<string, unknown>) => void }) {
  const numeric = useMemo(() => dataset.variables.filter((v) => v.type === 'numeric'), [dataset]);
  const [factors, setFactors] = useState<Factor[]>([
    { name: 'F1', items: [] },
    { name: 'F2', items: [] },
  ]);
  const [picked, setPicked] = useState<string[]>([]);
  const [anchor, setAnchor] = useState<number | null>(null);
  const [correlated, setCorrelated] = useState(true);
  const [stdLv, setStdLv] = useState(false);
  const [estimator, setEstimator] = useState<'ml' | 'mlr' | 'wlsmv'>('ml');
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [say, setSay] = useState('');

  const assigned = new Set(factors.flatMap((f) => f.items));
  const pool = numeric.filter((v) => !assigned.has(v.name));
  const isVar = (n: string) => dataset.variables.some((v) => v.name === n);

  const setCount = (k: number) => {
    const n = Math.max(1, Math.min(12, Math.round(k) || 1));
    setFactors((fs) => (n >= fs.length ? [...fs, ...Array.from({ length: n - fs.length }, (_, i) => ({ name: `F${fs.length + i + 1}`, items: [] }))] : fs.slice(0, n)));
  };
  const moveTo = (fi: number, names: string[] = picked) => {
    if (!names.length) return;
    setFactors((fs) => fs.map((f, i) => ({ ...f, items: i === fi ? [...f.items.filter((x) => !names.includes(x)), ...names.filter((x) => !f.items.includes(x))] : f.items.filter((x) => !names.includes(x)) })));
    setSay(`${names.join(', ')} → ${factors[fi].name}.`);
    setPicked([]);
  };
  const removeFrom = (fi: number, name: string) => {
    setFactors((fs) => fs.map((f, i) => (i === fi ? { ...f, items: f.items.filter((x) => x !== name) } : f)));
    setSay(`${name} back in the list.`);
  };
  // Click picks one item; Ctrl or Cmd adds or removes one; Shift picks a run.
  const pick = (v: VariableInfo, idx: number, e: React.MouseEvent | React.KeyboardEvent) => {
    if (e.shiftKey && anchor !== null) {
      const [a, b] = [Math.min(anchor, idx), Math.max(anchor, idx)];
      setPicked(pool.slice(a, b + 1).map((x) => x.name));
    } else if (e.ctrlKey || e.metaKey) {
      setPicked((p) => (p.includes(v.name) ? p.filter((x) => x !== v.name) : [...p, v.name]));
      setAnchor(idx);
    } else {
      setPicked([v.name]);
      setAnchor(idx);
    }
  };

  const problems: string[] = [];
  const names = factors.map((f) => f.name.trim());
  if (names.some((n) => !validName(n))) problems.push('Factor names use letters, digits, _ and . (starting with a letter).');
  else if (new Set(names).size !== names.length) problems.push('Each factor needs its own name.');
  else if (names.some(isVar)) problems.push(`${names.find(isVar)} is a variable in the data; give that factor another name.`);
  const empty = factors.filter((f) => f.items.length === 0);
  if (empty.length) problems.push(`Put items in ${empty.map((f) => f.name).join(', ')}.`);
  if (factors.some((f) => f.items.length === 1)) problems.push('A factor needs at least two items (three are best).');
  if (factors.length === 1 && factors[0].items.length === 2) problems.push('A single factor with two items is not identified; add a third item.');
  const firstProblem = problems[0] ?? null;
  const two = factors.filter((f) => f.items.length === 2);

  const model = cfaModel(
    factors.map((f) => ({ ...f, name: f.name.trim() })),
    correlated,
  );
  const args = (): Record<string, unknown> => {
    const def = ANALYSES.sem;
    const options: Record<string, OptionValue> = { ...Object.fromEntries(def.options.map((o) => [o.key, o.default])), model, std_lv: stdLv, estimator };
    return def.build({ roles: { group: [], cluster: [] }, pairs: {}, groups: {}, references: {}, options }) as Record<string, unknown>;
  };
  const run = async () => {
    setRunning(true);
    setError(null);
    try {
      onDone(await call<OutputItem[]>('sem', args()));
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setRunning(false);
    }
  };
  const ordinal = factors.flatMap((f) => f.items).length > 0 && factors.flatMap((f) => f.items).every((n) => dataset.variables.find((v) => v.name === n)?.measure === 'ordinal');

  return (
    <div className="overlay open" onKeyDown={(e) => e.key === 'Escape' && onClose()}>
      <div className="dialog wide" role="dialog" aria-modal="true" aria-labelledby="cfa-title">
        <header>
          <h2 id="cfa-title">Factor Analysis of a Scale</h2>
          <HelpButton />
          <button type="button" className="iconbtn x" aria-label="Close" onClick={onClose}>
            <Icon name="close" />
          </button>
        </header>
        <div className="dbody">
          <label className="cfacount">
            Number of factors{' '}
            <input className="textinput" type="number" min={1} max={12} style={{ width: 70 }} value={factors.length} onChange={(e) => setCount(Number(e.target.value))} />
            <span className="fieldhint">Then put each item in its factor’s box: drag it, double-click it (it goes to the first factor), or select items (Ctrl or Shift for several) and use a factor’s Move button.</span>
          </label>
          <div className="cfagrid">
            <div className="cfapool">
              <span className="fieldlabel" id="cfa-items">
                Items
              </span>
              <ul className="listbox tall" role="listbox" aria-multiselectable="true" aria-labelledby="cfa-items">
                {pool.length ? (
                  pool.map((v, i) => (
                    <li
                      key={v.id}
                      role="option"
                      aria-selected={picked.includes(v.name)}
                      tabIndex={0}
                      draggable
                      onDragStart={(e) => e.dataTransfer.setData('text/plain', JSON.stringify(picked.includes(v.name) ? picked : [v.name]))}
                      onClick={(e) => pick(v, i, e)}
                      onDoubleClick={() => moveTo(0, [v.name])}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter' || e.key === ' ') {
                          e.preventDefault();
                          pick(v, i, e);
                        }
                      }}
                    >
                      <span>{v.name}</span>
                      <span className="lab">{v.label}</span>
                    </li>
                  ))
                ) : (
                  <li className="empty">Every item is in a factor</li>
                )}
              </ul>
            </div>
            <div className="cfafactors">
              {factors.map((f, fi) => (
                <div
                  key={fi}
                  className="cfabox"
                  onDragOver={(e) => e.preventDefault()}
                  onDrop={(e) => {
                    e.preventDefault();
                    try {
                      const names = JSON.parse(e.dataTransfer.getData('text/plain')) as string[];
                      moveTo(fi, names.filter(isVar));
                    } catch {
                      /* not ours */
                    }
                  }}
                >
                  <div className="cfahead">
                    <input className="textinput" aria-label={`Name of factor ${fi + 1}`} value={f.name} onChange={(e) => setFactors((fs) => fs.map((x, i) => (i === fi ? { ...x, name: e.target.value } : x)))} />
                    <button type="button" className="btn" disabled={!picked.length} onClick={() => moveTo(fi)}>
                      Move here
                    </button>
                  </div>
                  <ul className="cfaitems" aria-label={`Items of ${f.name}`}>
                    {f.items.map((it) => (
                      <li
                        key={it}
                        draggable
                        onDragStart={(e) => e.dataTransfer.setData('text/plain', JSON.stringify([it]))}
                      >
                        <span>{it}</span>
                        <button type="button" className="iconbtn" aria-label={`Take ${it} out of ${f.name}`} onClick={() => removeFrom(fi, it)}>
                          <Icon name="close" />
                        </button>
                      </li>
                    ))}
                    {!f.items.length && <li className="empty">Drop items here</li>}
                  </ul>
                </div>
              ))}
            </div>
          </div>
          <div className="cfaopts">
            <label className="check">
              <input type="checkbox" checked={correlated} onChange={(e) => setCorrelated(e.target.checked)} />
              <span>Factors correlate (usual for the subscales of one instrument)</span>
            </label>
            <label className="check">
              <input type="checkbox" checked={stdLv} onChange={(e) => setStdLv(e.target.checked)} />
              <span>Identify each factor by fixing its variance at 1 (instead of its first loading)</span>
            </label>
            <fieldset className="cfaest">
              <legend>Estimator</legend>
              {(
                [
                  ['ml', 'Maximum likelihood'],
                  ['mlr', 'Robust maximum likelihood (MLR), for items that aren’t normal'],
                  ['wlsmv', 'Ordinal (WLSMV), for Likert items with few categories'],
                ] as const
              ).map(([v, l]) => (
                <label key={v} className="radio">
                  <input type="radio" name="cfa-est" checked={estimator === v} onChange={() => setEstimator(v)} />
                  <span>{l}</span>
                </label>
              ))}
              {ordinal && estimator !== 'wlsmv' && <span className="fieldhint">Every item is marked ordinal: WLSMV treats them as ordered categories.</span>}
            </fieldset>
          </div>
          <details className="opt">
            <summary>The model this runs</summary>
            <div className="optbody">
              <pre className="cfamodel">{model || '(no items yet)'}</pre>
              <span className="fieldhint">Reliability (ω, AVE) and discriminant validity (HTMT) come with the results. To change the model (a cross-loading, a correlated residual), use Open in Model Builder.</span>
            </div>
          </details>
          {two.length > 0 && !firstProblem && (
            <div className="inline-err" aria-live="polite">
              <Icon name="info" />
              {two.map((f) => f.name).join(', ')} {two.length === 1 ? 'has' : 'have'} two items: identified only through {two.length === 1 ? 'its correlation' : 'their correlations'} with other factors.
            </div>
          )}
          {error && (
            <div className="errbox" role="alert">
              {error}
            </div>
          )}
          {firstProblem && (
            <div className="inline-err" aria-live="polite">
              <Icon name="info" />
              {firstProblem}
            </div>
          )}
          <div className="fieldhint" aria-live="polite">
            {say}
          </div>
        </div>
        <footer>
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <button type="button" className="btn" disabled={!model} onClick={() => onOpenBuilder(args())}>
            Open in Model Builder
          </button>
          {onPaste && (
            <button type="button" className="btn" disabled={!!firstProblem} onClick={() => onPaste('sem', args())}>
              Paste
            </button>
          )}
          <button type="button" className="btn primary" disabled={!!firstProblem || running} onClick={run}>
            <Icon name="run" />
            {running ? 'Running…' : 'Run'}
          </button>
        </footer>
      </div>
    </div>
  );
}
