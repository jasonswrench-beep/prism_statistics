// Analyze › Power Analysis…: the sample size a planned study needs, or the power a planned sample has. Needs no data.
// The effect is entered as a standardized size or in raw units (05 §1: no "small, medium, large" shortcuts).

import { useState } from 'react';
import { call } from '../bridge';
import type { OutputItem } from '../types';
import { Modal } from './Modal';

type Test = 'one_sample' | 'paired' | 'independent' | 'anova' | 'correlation' | 'one_proportion' | 'two_proportions';

const TESTS: { value: Test; label: string }[] = [
  { value: 'independent', label: 'Independent-samples t test (two groups)' },
  { value: 'paired', label: 'Paired-samples t test' },
  { value: 'one_sample', label: 'One-sample t test' },
  { value: 'anova', label: 'One-way ANOVA' },
  { value: 'correlation', label: 'Pearson correlation' },
  { value: 'two_proportions', label: 'Two proportions' },
  { value: 'one_proportion', label: 'One proportion' },
];

const isT = (t: Test) => t === 'independent' || t === 'paired' || t === 'one_sample';
const isP = (t: Test) => t === 'one_proportion' || t === 'two_proportions';
const grouped = (t: Test) => t === 'independent' || t === 'anova' || t === 'two_proportions';

export function PowerDialog({ initial, onClose, onDone, onPaste }: { initial?: Record<string, unknown>; onClose: () => void; onDone: (items: OutputItem[]) => void; onPaste?: (command: string, args: unknown) => void }) {
  const a = initial ?? {};
  const given = (k: string) => (a[k] == null ? undefined : String(a[k]));
  const [test, setTest] = useState<Test>((a.test as Test) ?? 'independent');
  const [solve, setSolve] = useState<'n' | 'power'>((a.solve_for as 'n' | 'power') ?? 'n');
  const [raw, setRaw] = useState(a.effect_size == null);
  const [f, setF] = useState<Record<string, string>>({
    es: (a.test !== 'correlation' && given('effect_size')) || '0.5',
    diff: given('mean_difference') ?? '5',
    sd: given('sd') ?? '10',
    means: Array.isArray(a.means) ? a.means.join(', ') : '10, 12, 14',
    groups: given('groups') ?? '3',
    p1: given('p1') ?? '0.6',
    p2: given('p2') ?? '0.4',
    r: (a.test === 'correlation' && given('effect_size')) || '0.3',
    alpha: given('alpha') ?? '0.05',
    power: given('power') ?? '0.80',
    n: given('n') ?? '50',
  });
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const set = (k: string) => (e: React.ChangeEvent<HTMLInputElement>) => setF({ ...f, [k]: e.target.value });
  const field = (k: string, label: string, hint?: string) => (
    <label className="powerfield" key={k}>
      <span>{label}</span>
      <input className="textinput" inputMode="decimal" style={{ width: 120 }} value={f[k]} onChange={set(k)} />
      {hint && <span className="fieldhint">{hint}</span>}
    </label>
  );
  const symbol = isT(test) ? 'd' : test === 'anova' ? 'f' : test === 'correlation' ? 'r' : 'h';

  const build = (): Record<string, unknown> | string => {
    const num = (k: string, what: string) => {
      const v = Number(f[k]);
      if (f[k].trim() === '' || !Number.isFinite(v)) throw new Error(`${what} must be a number.`);
      return v;
    };
    try {
      const args: Record<string, unknown> = { test, solve_for: solve, alpha: num('alpha', 'Alpha') };
      if (solve === 'n') args.power = num('power', 'Power');
      else args.n = Math.round(num('n', 'The sample size'));
      if (test === 'correlation') args.effect_size = num('r', 'The correlation');
      else if (!raw) {
        args.effect_size = num('es', `Cohen's ${symbol}`);
        if (test === 'anova') args.groups = Math.round(num('groups', 'The number of groups'));
      } else if (isT(test)) {
        args.mean_difference = num('diff', 'The mean difference');
        args.sd = num('sd', 'The SD');
      } else if (test === 'anova') {
        const means = f.means.split(/[,;\s]+/).filter(Boolean).map(Number);
        if (means.length < 2 || means.some((m) => !Number.isFinite(m))) throw new Error('Give at least two group means, separated by commas.');
        args.means = means;
        args.sd = num('sd', 'The SD');
      } else if (isP(test)) {
        args.p1 = num('p1', 'The first proportion');
        args.p2 = num('p2', 'The second proportion');
      }
      return args;
    } catch (e) {
      return (e as Error).message;
    }
  };

  const run = async () => {
    const args = build();
    if (typeof args === 'string') return setError(args);
    setRunning(true);
    setError(null);
    try {
      onDone(await call<OutputItem[]>('power', args));
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setRunning(false);
    }
  };

  return (
    <Modal
      title="Power Analysis"
      width={600}
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          {onPaste && (
            <button
              type="button"
              className="btn"
              title="Add this analysis to the script without running it"
              onClick={() => {
                const args = build();
                if (typeof args === 'string') setError(args);
                else onPaste('power', args);
              }}
            >
              Paste
            </button>
          )}
          <button type="button" className="btn primary" onClick={run} disabled={running}>
            {running ? 'Working…' : 'Run'}
          </button>
        </>
      }
    >
      <div className="powergrid">
        <label className="powerfield">
          <span>Test</span>
          <select className="textinput" value={test} onChange={(e) => setTest(e.target.value as Test)}>
            {TESTS.map((t) => (
              <option key={t.value} value={t.value}>
                {t.label}
              </option>
            ))}
          </select>
        </label>
        <fieldset className="powerset">
          <legend>Find</legend>
          <label className="radio">
            <input type="radio" name="solve" checked={solve === 'n'} onChange={() => setSolve('n')} /> <span>The sample size needed</span>
          </label>
          <label className="radio">
            <input type="radio" name="solve" checked={solve === 'power'} onChange={() => setSolve('power')} /> <span>The power of a sample size</span>
          </label>
        </fieldset>
        {test !== 'correlation' && (
          <fieldset className="powerset">
            <legend>Give the effect as</legend>
            <label className="radio">
              <input type="radio" name="raw" checked={raw} onChange={() => setRaw(true)} />{' '}
              <span>{isT(test) ? 'A mean difference and SD' : test === 'anova' ? 'Group means and the SD within groups' : 'Proportions'}</span>
            </label>
            <label className="radio">
              <input type="radio" name="raw" checked={!raw} onChange={() => setRaw(false)} /> <span>Cohen&rsquo;s {symbol}</span>
            </label>
          </fieldset>
        )}
        {test === 'correlation' && field('r', 'Expected correlation (r)')}
        {test !== 'correlation' && !raw && field('es', `Cohen's ${symbol}`)}
        {test === 'anova' && !raw && field('groups', 'Number of groups')}
        {isT(test) && raw && field('diff', test === 'one_sample' ? 'Difference from the test value' : test === 'paired' ? 'Mean of the differences' : 'Difference between the group means')}
        {isT(test) && raw && field('sd', test === 'paired' ? 'SD of the differences' : test === 'independent' ? 'SD within each group' : 'SD of the scores')}
        {test === 'anova' && raw && field('means', 'Expected group means', 'separated by commas')}
        {test === 'anova' && raw && field('sd', 'SD within each group')}
        {isP(test) && raw && field('p1', test === 'one_proportion' ? 'Expected proportion' : 'Proportion in group 1')}
        {isP(test) && raw && field('p2', test === 'one_proportion' ? 'Proportion it is compared with' : 'Proportion in group 2')}
        {field('alpha', 'Alpha (two-sided)')}
        {solve === 'n' ? field('power', 'Power wanted') : field('n', grouped(test) ? 'N per group' : test === 'paired' ? 'Number of pairs' : 'N')}
      </div>
      <p className="fieldhint">Base the effect on earlier studies or on the smallest effect that would matter, not on a label like &ldquo;medium&rdquo;. Groups are assumed to be the same size.</p>
      {error && (
        <p role="alert" className="errbox">
          {error}
        </p>
      )}
    </Modal>
  );
}
