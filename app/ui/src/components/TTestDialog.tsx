// Analyze › Compare Means › Independent-Samples T Test. The familiar variable-transfer dialog (FAM-04): a source list,
// a Test Variables box, a Grouping Variable box with the two groups chosen by code, and options with their defaults.

import { getSettings } from '../settings';
import { HelpButton } from './HelpDialog';
import { useEffect, useMemo, useState } from 'react';
import { call } from '../bridge';
import type { DatasetInfo, GroupCode, OutputItem, TTestSpec, VariableInfo } from '../types';
import { Icon } from './Icon';

export function TTestDialog({ dataset, initial, onClose, onDone, onPaste }: { dataset: DatasetInfo; initial?: Partial<TTestSpec>; onClose: () => void; onDone: (items: OutputItem[]) => void; onPaste?: (command: string, args: unknown) => void }) {
  const numeric = useMemo(() => dataset.variables.filter((v) => v.type === 'numeric'), [dataset]);
  const [selected, setSelected] = useState<string | null>(null);
  const exists = (n: string) => dataset.variables.some((v) => v.name === n);
  const [tests, setTests] = useState<string[]>((initial?.test_variables ?? []).filter(exists));
  const [group, setGroup] = useState<string | null>(initial?.grouping_variable && exists(initial.grouping_variable) ? initial.grouping_variable : null);
  const [codes, setCodes] = useState<GroupCode[]>([]);
  const [codeError, setCodeError] = useState<string | null>(null);
  const [g1, setG1] = useState<number | null>(null);
  const [g2, setG2] = useState<number | null>(null);
  const [estimator, setEstimator] = useState<'welch' | 'student'>(initial?.estimator ?? 'welch');
  const [level, setLevel] = useState(initial?.level ? String(Math.round(initial.level * 1e6) / 1e4) : String(getSettings().level));
  const [alternative, setAlternative] = useState<'two_sided' | 'greater' | 'less'>(initial?.alternative ?? 'two_sided');
  const [bayes, setBayes] = useState<boolean>(initial?.bayes ?? false);
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);

  const available = numeric.filter((v) => !tests.includes(v.name) && v.name !== group);

  useEffect(() => {
    if (!group) return setCodes([]);
    call<GroupCode[]>('group_codes', { name: group })
      .then((c) => {
        setCodeError(null);
        setCodes(c);
        // Reopened with the same grouping variable: keep the groups chosen before when they still exist.
        const kept = group === initial?.grouping_variable && initial?.groups && initial.groups.every((g) => c.some((x) => x.code === g)) ? initial.groups : null;
        setG1(kept ? kept[0] : (c[0]?.code ?? null));
        setG2(kept ? kept[1] : (c[1]?.code ?? null));
      })
      .catch((e: Error) => {
        setCodes([]);
        setCodeError(e.message);
      });
  }, [group]);

  const toTests = () => {
    if (selected && available.some((v) => v.name === selected)) {
      setTests([...tests, selected]);
      setSelected(null);
    }
  };
  const toGroup = () => {
    if (selected && available.some((v) => v.name === selected)) {
      setGroup(selected);
      setSelected(null);
    }
  };

  const problems: string[] = [];
  if (!tests.length) problems.push('Move at least one variable into Test Variables.');
  if (!group) problems.push('Move a variable into Grouping Variable.');
  else if (codes.length < 2 && !codeError) problems.push('The grouping variable needs at least two groups with data.');
  if (g1 !== null && g1 === g2) problems.push('Choose two different groups.');
  const levelNum = Number(level);
  if (!(levelNum > 0 && levelNum < 100)) problems.push('The confidence level must be between 0 and 100.');

  const run = async () => {
    if (problems.length || codeError || !group || g1 === null || g2 === null) return;
    const spec: TTestSpec = { test_variables: tests, grouping_variable: group, groups: [g1, g2], estimator, alternative, bayes, level: levelNum / 100 };
    setRunning(true);
    try {
      onDone(await call<OutputItem[]>('ttest_independent', spec));
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setRunning(false);
    }
  };

  const label = (c: GroupCode) => `${c.label ?? c.code} (${c.code}, n = ${c.count})`;
  const varRow = (v: VariableInfo, onPick: () => void, picked: boolean) => (
    <li key={v.id} role="option" aria-label={v.label ? `${v.name}, ${v.label}` : v.name} aria-selected={picked} tabIndex={0} onClick={onPick} onKeyDown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), onPick())}>
      <span>{v.name}</span>
      <span className="lab">{v.label}</span>
      <span className="measure">
        <Icon name={v.measure} />
      </span>
    </li>
  );

  return (
    <div className="overlay open" onKeyDown={(e) => e.key === 'Escape' && onClose()}>
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby="ttTitle">
        <header>
          <h2 id="ttTitle">Independent-Samples T Test</h2>
          <HelpButton />
          <button type="button" className="iconbtn x" aria-label="Close" onClick={onClose}>
            <Icon name="close" />
          </button>
        </header>
        <div className="dbody">
          <div className="transfer">
            <div>
              <span className="fieldlabel" id="srcLbl">
                Variables
              </span>
              <ul className="listbox tall" role="listbox" aria-labelledby="srcLbl">
                {available.length ? available.map((v) => varRow(v, () => setSelected(v.name), selected === v.name)) : <li className="empty">No numeric variables left</li>}
              </ul>
            </div>
            <div className="rolecol" style={{ marginTop: 40 }}>
              <button type="button" className="arrowbtn" aria-label="Move to Test Variables" onClick={toTests}>
                <Icon name="right" />
              </button>
              <button type="button" className="arrowbtn" aria-label="Move to Grouping Variable" onClick={toGroup} style={{ marginTop: 120 }}>
                <Icon name="right" />
              </button>
            </div>
            <div className="rolecol">
              <div>
                <span className="fieldlabel" id="testLbl">
                  Test Variables <span className="fieldhint">outcomes to compare</span>
                </span>
                <ul className="listbox" role="listbox" aria-labelledby="testLbl" style={{ minHeight: 120 }}>
                  {tests.length ? (
                    tests.map((n) => {
                      const v = numeric.find((x) => x.name === n)!;
                      return varRow(v, () => setTests(tests.filter((t) => t !== n)), false);
                    })
                  ) : (
                    <li className="empty">Select a variable and press the arrow</li>
                  )}
                </ul>
                {tests.length > 0 && <span className="fieldhint">Click a test variable to move it back.</span>}
              </div>
              <div>
                <span className="fieldlabel" id="grpLbl">
                  Grouping Variable
                </span>
                <ul className="listbox" role="listbox" aria-labelledby="grpLbl">
                  {group ? varRow(numeric.find((x) => x.name === group)!, () => setGroup(null), false) : <li className="empty">Select a variable and press the arrow</li>}
                </ul>
                {codeError && (
                  <div className="inline-err" role="alert">
                    <Icon name="warn" />
                    {codeError}
                  </div>
                )}
                {codes.length >= 2 && (
                  <div className="groupsdef">
                    <label>
                      Group 1{' '}
                      <select value={g1 ?? ''} onChange={(e) => setG1(Number(e.target.value))}>
                        {codes.map((c) => (
                          <option key={c.code} value={c.code}>
                            {label(c)}
                          </option>
                        ))}
                      </select>
                    </label>
                    <label>
                      Group 2{' '}
                      <select value={g2 ?? ''} onChange={(e) => setG2(Number(e.target.value))}>
                        {codes.map((c) => (
                          <option key={c.code} value={c.code}>
                            {label(c)}
                          </option>
                        ))}
                      </select>
                    </label>
                  </div>
                )}
              </div>
            </div>
          </div>

          <details className="opt" open>
            <summary>
              Test <span className="cur">{estimator === 'welch' ? 'Welch (default)' : 'Student'}</span>
            </summary>
            <div className="optbody" role="radiogroup" aria-label="Which t test">
              <label className="radio">
                <input type="radio" name="est" checked={estimator === 'welch'} onChange={() => setEstimator('welch')} />
                <span>
                  <b>Welch, unequal variances</b> (default): doesn't assume the groups have the same spread.
                </span>
              </label>
              <label className="radio">
                <input type="radio" name="est" checked={estimator === 'student'} onChange={() => setEstimator('student')} />
                <span>
                  <b>Student, pooled variance</b>: assumes equal spread. Some packages label this “Equal variances assumed”.
                </span>
              </label>
            </div>
          </details>
          <details className="opt">
            <summary>
              Hypothesis <span className="cur">{alternative === 'two_sided' ? 'Two-sided (default)' : alternative === 'greater' ? 'First group higher' : 'First group lower'}</span>
            </summary>
            <div className="optbody" role="radiogroup" aria-label="Alternative hypothesis">
              <label className="radio">
                <input type="radio" name="alt" checked={alternative === 'two_sided'} onChange={() => setAlternative('two_sided')} />
                <span>
                  <b>Two-sided</b> (default): a difference in either direction.
                </span>
              </label>
              <label className="radio">
                <input type="radio" name="alt" checked={alternative === 'greater'} onChange={() => setAlternative('greater')} />
                <span>
                  <b>One-sided, first group higher</b>: only a higher mean in the first group counts.
                </span>
              </label>
              <label className="radio">
                <input type="radio" name="alt" checked={alternative === 'less'} onChange={() => setAlternative('less')} />
                <span>
                  <b>One-sided, first group lower</b>: only a lower mean in the first group counts.
                </span>
              </label>
            </div>
          </details>
          <details className="opt">
            <summary>
              Confidence interval <span className="cur">{level}%</span>
            </summary>
            <div className="optbody">
              <label>
                Level (%){' '}
                <input className="textinput" inputMode="decimal" style={{ width: 80 }} value={level} onChange={(e) => setLevel(e.target.value)} />
              </label>
              <span className="fieldhint">A one-sided test gives a one-sided interval. Effect size: Hedges' g with the pooled SD.</span>
              <label className="check" style={{ display: 'block', marginTop: 6 }}>
                <input type="checkbox" checked={bayes} onChange={(e) => setBayes(e.target.checked)} /> Bayes factor (BF₁₀, R’s BayesFactor default prior)
              </label>
            </div>
          </details>

          {error && <div className="errbox" role="alert">{error}</div>}
          {problems.length > 0 && (
            <div className="inline-err" aria-live="polite">
              <Icon name="info" />
              {problems[0]}
            </div>
          )}
        </div>
        <footer>
          <button type="button" className="btn" onClick={() => (setTests([]), setGroup(null), setEstimator('welch'), setLevel('95'))}>
            Reset
          </button>
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          {onPaste && (
            <button type="button" className="btn" disabled={problems.length > 0 || !!codeError || !group || g1 === null || g2 === null} onClick={() => group && g1 !== null && g2 !== null && onPaste('ttest_independent', { test_variables: tests, grouping_variable: group, groups: [g1, g2], estimator, alternative, bayes, level: levelNum / 100 })} title="Add this analysis to the script without running it">
              Paste
            </button>
          )}
          <button type="button" className="btn primary" disabled={problems.length > 0 || !!codeError || running} onClick={run}>
            <Icon name="run" />
            {running ? 'Running…' : 'Run'}
          </button>
        </footer>
      </div>
    </div>
  );
}
