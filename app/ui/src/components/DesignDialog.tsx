// Analyze › Design of Experiments › Create Design…: generates a factorial, fractional factorial, Plackett-Burman,
// central composite, Box-Behnken, mixture (constrained or not) or D- or I-optimal design and opens it as the data, with an empty response column to fill in as the runs are done. Needs no
// data (and replaces any that is open, after the usual offer to save).

import { useState } from 'react';
import { Modal } from './Modal';

type Kind = 'fractional' | 'pb' | 'full' | 'ccd' | 'bbd' | 'lattice' | 'centroid' | 'vertices' | 'optimal';

interface Row {
  name: string;
  low: string;
  high: string;
  levels: string;
}

const KINDS: { value: Kind; label: string; hint: string }[] = [
  { value: 'fractional', label: 'Two-level fractional factorial', hint: 'screening many factors in few runs (minimum aberration)' },
  { value: 'pb', label: 'Plackett-Burman', hint: 'screening main effects in 12, 20, 24, 36, 44 or 48 runs' },
  { value: 'full', label: 'Full factorial', hint: 'every combination of the factor levels' },
  { value: 'ccd', label: 'Central composite', hint: 'response surface, 2–6 factors' },
  { value: 'bbd', label: 'Box-Behnken', hint: 'response surface, 3–5 factors' },
  { value: 'lattice', label: 'Simplex lattice (mixture)', hint: 'blends in steps of 1/m of each component' },
  { value: 'centroid', label: 'Simplex centroid (mixture)', hint: 'every equal blend of 1, 2, … components' },
  { value: 'vertices', label: 'Extreme vertices (constrained mixture)', hint: 'components with lower and upper limits' },
  { value: 'optimal', label: 'Optimal (D or I)', hint: 'the best runs for a chosen model and number of runs' },
];
const PB_RUNS = [12, 20, 24, 36, 44, 48];

const letter = (i: number) => 'ABCDEFGHJKLMNOPQRSTUVWXYZ'[i] ?? `X${i + 1}`;

export function DesignDialog({ onClose, onCreate }: { onClose: () => void; onCreate: (spec: Record<string, unknown>) => Promise<void> }) {
  const [kind, setKind] = useState<Kind>('fractional');
  const [rows, setRows] = useState<Row[]>(Array.from({ length: 5 }, (_, i) => ({ name: letter(i), low: '', high: '', levels: '' })));
  const [runs, setRuns] = useState('8');
  const [center, setCenter] = useState('0');
  const [centerAxial, setCenterAxial] = useState('');
  const [alpha, setAlpha] = useState<'orthogonal' | 'rotatable' | 'face'>('orthogonal');
  const [replicates, setReplicates] = useState('1');
  const [randomize, setRandomize] = useState(true);
  const [seed, setSeed] = useState('20260101');
  const [response, setResponse] = useState('response');
  const [pbRuns, setPbRuns] = useState('12');
  const [degree, setDegree] = useState('2');
  const [augment, setAugment] = useState(false);
  const [model, setModel] = useState<'linear' | 'interactions' | 'quadratic'>('quadratic');
  const [optRuns, setOptRuns] = useState('');
  const [criterion, setCriterion] = useState<'D' | 'I'>('D');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const setCount = (n: number) => {
    const k = Math.max(1, Math.min(15, Math.round(n) || 1));
    setRows((rs) => (k >= rs.length ? [...rs, ...Array.from({ length: k - rs.length }, (_, i) => ({ name: letter(rs.length + i), low: '', high: '', levels: '' }))] : rs.slice(0, k)));
  };
  const edit = (i: number, key: keyof Row, v: string) => setRows((rs) => rs.map((r, j) => (j === i ? { ...r, [key]: v } : r)));
  const k = rows.length;
  const runsAllowed = [4, 8, 16, 32, 64].filter((r) => r > k && r <= 2 ** k);
  const pbAllowed = PB_RUNS.filter((r) => r - 1 >= k);
  const mixture = kind === 'lattice' || kind === 'centroid' || kind === 'vertices';
  // Mixture bounds: lower limits for lattice and centroid designs (pseudo-components), both for extreme vertices.
  const showLow = true;
  const showHigh = !mixture || kind === 'vertices';
  const showLevels = kind === 'full' || kind === 'optimal';
  // Model terms for a D-optimal design: 1 + k, plus k(k−1)/2 interactions, plus k squares.
  const terms = 1 + k + (model === 'linear' ? 0 : (k * (k - 1)) / 2) + (model === 'quadratic' ? k : 0);

  const build = (): Record<string, unknown> | string => {
    const names = rows.map((r) => r.name.trim());
    if (names.some((n) => !/^[A-Za-z_.][A-Za-z0-9_.]*$/.test(n))) return 'Factor names use letters, digits, _ and . (starting with a letter).';
    if (new Set(names).size !== names.length) return 'Each factor needs its own name.';
    if (!/^[A-Za-z_.][A-Za-z0-9_.]*$/.test(response.trim()) || names.includes(response.trim())) return 'Give the response column a name of its own.';
    const factors = [];
    for (const r of rows) {
      const f: Record<string, unknown> = { name: r.name.trim() };
      if (mixture) {
        if (r.low.trim() !== '') {
          const lo = Number(r.low);
          if (!Number.isFinite(lo) || lo < 0 || lo >= 1) return `Give ${r.name} a lower limit from 0 up to 1, or leave it empty.`;
          f.low = lo;
        }
        if (kind === 'vertices' && r.high.trim() !== '') {
          const hi = Number(r.high);
          if (!Number.isFinite(hi) || hi <= 0 || hi > 1) return `Give ${r.name} an upper limit above 0 and up to 1, or leave it empty.`;
          f.high = hi;
        }
      } else if (r.low.trim() !== '' || r.high.trim() !== '') {
        const lo = Number(r.low);
        const hi = Number(r.high);
        if (!Number.isFinite(lo) || !Number.isFinite(hi) || lo === hi) return `Give ${r.name} two different numeric settings, or leave both empty for coded −1 and +1.`;
        f.low = lo;
        f.high = hi;
      }
      if (kind === 'full') f.levels = Math.max(2, Math.round(Number(r.levels) || 2));
      if (kind === 'optimal' && r.levels.trim() !== '') f.levels = Math.max(2, Math.round(Number(r.levels) || 2));
      factors.push(f);
    }
    if (mixture && (k < 2 || k > 10)) return 'A mixture design here has 2 to 10 components.';
    if (kind === 'ccd' && (k < 2 || k > 6)) return 'A central composite design here has 2 to 6 factors.';
    if (kind === 'bbd' && (k < 3 || k > 5)) return 'A Box-Behnken design here has 3 to 5 factors.';
    const spec: Record<string, unknown> = { kind, factors, center: Math.max(0, Math.round(Number(center) || 0)), replicates: Math.max(1, Math.round(Number(replicates) || 1)), randomize, seed: Number(seed), response: response.trim() };
    if (kind === 'fractional') {
      if (!runsAllowed.length) return k < 3 ? 'With so few factors, use a full factorial.' : 'No fractional design of up to 64 runs fits that many factors.';
      spec.runs = Number(runs);
    }
    if (kind === 'ccd') {
      spec.alpha = alpha;
      if (centerAxial.trim() !== '') spec.center_axial = Math.max(0, Math.round(Number(centerAxial)));
    }
    if (kind === 'pb') {
      if (!pbAllowed.length) return 'A Plackett-Burman design here has room for at most 47 factors.';
      spec.runs = Number(pbAllowed.includes(Number(pbRuns)) ? pbRuns : pbAllowed[0]);
    }
    if (kind === 'lattice' || kind === 'centroid') {
      spec.augment = augment;
      if (kind === 'lattice') spec.degree = Math.max(1, Math.min(6, Math.round(Number(degree) || 2)));
    }
    if (kind === 'vertices') spec.augment = augment;
    if (kind === 'optimal') {
      spec.model = model;
      spec.criterion = criterion;
      const n = optRuns.trim() === '' ? terms + 4 : Math.round(Number(optRuns));
      if (!Number.isFinite(n) || n < terms) return `The model has ${terms} terms, so the design needs at least ${terms} runs.`;
      spec.runs = n;
    }
    return spec;
  };

  const create = async () => {
    const s = build();
    if (typeof s === 'string') return setError(s);
    setBusy(true);
    setError(null);
    try {
      await onCreate(s);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      title="Create Design"
      width={680}
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <button type="button" className="btn primary" disabled={busy} onClick={create}>
            {busy ? 'Creating…' : 'Create design'}
          </button>
        </>
      }
    >
      <fieldset className="cfaest">
        <legend>Design</legend>
        {KINDS.map((d) => (
          <label key={d.value} className="radio">
            <input type="radio" name="design-kind" checked={kind === d.value} onChange={() => setKind(d.value)} />
            <span>
              {d.label} <span className="fieldhint">{d.hint}</span>
            </span>
          </label>
        ))}
      </fieldset>
      <label className="cfacount">
        {mixture ? 'Number of components' : 'Number of factors'}{' '}
        <input className="textinput" type="number" min={1} max={15} style={{ width: 70 }} value={k} onChange={(e) => setCount(Number(e.target.value))} />
      </label>
      <table className="designtable">
        <thead>
          <tr>
            <th>{mixture ? 'Component' : 'Factor'}</th>
            {showLow && <th>{mixture ? 'Lower limit' : 'Low setting'}</th>}
            {showHigh && <th>{mixture ? 'Upper limit' : 'High setting'}</th>}
            {showLevels && <th>Levels</th>}
          </tr>
        </thead>
        <tbody>
          {rows.map((r, i) => (
            <tr key={i}>
              <td>
                <input className="textinput" aria-label={`Name of factor ${i + 1}`} value={r.name} onChange={(e) => edit(i, 'name', e.target.value)} />
              </td>
              {showLow && (
                <td>
                  <input className="textinput" aria-label={`${mixture ? 'Lower limit' : 'Low setting'} of ${r.name}`} placeholder={mixture ? '0' : '−1'} value={r.low} onChange={(e) => edit(i, 'low', e.target.value)} />
                </td>
              )}
              {showHigh && (
                <td>
                  <input className="textinput" aria-label={`${mixture ? 'Upper limit' : 'High setting'} of ${r.name}`} placeholder={mixture ? '1' : '+1'} value={r.high} onChange={(e) => edit(i, 'high', e.target.value)} />
                </td>
              )}
              {showLevels && (
                <td>
                  <input className="textinput" type="number" min={2} aria-label={`Levels of ${r.name}`} placeholder={kind === 'optimal' && model === 'quadratic' ? '3' : '2'} value={r.levels} onChange={(e) => edit(i, 'levels', e.target.value)} style={{ width: 70 }} />
                </td>
              )}
            </tr>
          ))}
        </tbody>
      </table>
      <span className="fieldhint">
        {mixture
          ? kind === 'vertices'
            ? 'Components are proportions of the blend (they add to 1 in every run); the design is every corner of the region the limits allow.'
            : 'Components are proportions of the blend (they add to 1 in every run). With lower limits the design is built in pseudo-components and mapped back.'
          : kind === 'optimal'
            ? 'Leave the settings empty to use coded units (−1 to +1). Levels: the candidate settings of each factor, evenly spaced.'
            : 'Leave the settings empty to use coded units (−1 and +1, or 1, 2, … for more levels).'}
      </span>
      <div className="designopts">
        {kind === 'fractional' && (
          <label>
            Runs{' '}
            <select className="textinput" value={runs} onChange={(e) => setRuns(e.target.value)} aria-label="Runs">
              {runsAllowed.map((r) => (
                <option key={r} value={r}>
                  {r}
                  {r === 2 ** k ? ' (full factorial)' : ` (1/${2 ** k / r} fraction)`}
                </option>
              ))}
            </select>
          </label>
        )}
        {kind === 'pb' && (
          <label>
            Runs{' '}
            <select className="textinput" value={pbRuns} onChange={(e) => setPbRuns(e.target.value)} aria-label="Runs">
              {pbAllowed.map((r) => (
                <option key={r} value={r}>
                  {r}
                </option>
              ))}
            </select>
          </label>
        )}
        {kind === 'lattice' && (
          <label>
            Degree m{' '}
            <input className="textinput" type="number" min={1} max={6} style={{ width: 70 }} value={degree} onChange={(e) => setDegree(e.target.value)} />
          </label>
        )}
        {mixture && (
          <label className="check">
            <input type="checkbox" checked={augment} onChange={(e) => setAugment(e.target.checked)} />
            <span>{kind === 'vertices' ? 'Add the midpoints of the edges and the centroid' : 'Add the overall centroid and axial check blends'}</span>
          </label>
        )}
        {kind === 'optimal' && (
          <>
            <label>
              Criterion{' '}
              <select className="textinput" value={criterion} onChange={(e) => setCriterion(e.target.value as typeof criterion)} aria-label="Criterion">
                <option value="D">D-optimal (precise coefficients)</option>
                <option value="I">I-optimal (precise predictions)</option>
              </select>
            </label>
            <label>
              Model{' '}
              <select className="textinput" value={model} onChange={(e) => setModel(e.target.value as typeof model)} aria-label="Model">
                <option value="quadratic">Quadratic (response surface)</option>
                <option value="interactions">Main effects and two-factor interactions</option>
                <option value="linear">Main effects</option>
              </select>
            </label>
            <label>
              Runs{' '}
              <input className="textinput" type="number" min={terms} style={{ width: 70 }} placeholder={String(terms + 4)} value={optRuns} onChange={(e) => setOptRuns(e.target.value)} aria-label="Runs" />
            </label>
            <span className="fieldhint">{terms} model terms</span>
          </>
        )}
        <label>
          {kind === 'ccd' ? 'Center points (cube block)' : kind === 'bbd' ? 'Center points per block' : mixture ? 'Extra centroid runs' : 'Center points'}{' '}
          <input className="textinput" type="number" min={0} style={{ width: 70 }} value={center} onChange={(e) => setCenter(e.target.value)} />
        </label>
        {kind === 'ccd' && (
          <>
            <label>
              Center points (axial block){' '}
              <input className="textinput" type="number" min={0} style={{ width: 70 }} placeholder={center} value={centerAxial} onChange={(e) => setCenterAxial(e.target.value)} />
            </label>
            <label>
              Axial distance{' '}
              <select className="textinput" value={alpha} onChange={(e) => setAlpha(e.target.value as typeof alpha)} aria-label="Axial distance">
                <option value="orthogonal">Orthogonal blocks (default)</option>
                <option value="rotatable">Rotatable</option>
                <option value="face">Face-centered (α = 1)</option>
              </select>
            </label>
          </>
        )}
        <label>
          Replicates{' '}
          <input className="textinput" type="number" min={1} style={{ width: 70 }} value={replicates} onChange={(e) => setReplicates(e.target.value)} />
        </label>
        <label>
          Response column{' '}
          <input className="textinput" style={{ width: 140 }} value={response} onChange={(e) => setResponse(e.target.value)} />
        </label>
        <label className="check">
          <input type="checkbox" checked={randomize} onChange={(e) => setRandomize(e.target.checked)} />
          <span>Randomize the run order (within blocks)</span>
        </label>
        {randomize && (
          <label>
            Seed <input className="textinput" type="number" style={{ width: 120 }} value={seed} onChange={(e) => setSeed(e.target.value)} />
          </label>
        )}
      </div>
      <p className="fieldhint">The design opens as the data, replacing what is open (you will be offered the chance to save first).</p>
      {error && (
        <div className="errbox" role="alert">
          {error}
        </div>
      )}
    </Modal>
  );
}
