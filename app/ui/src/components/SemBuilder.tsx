// Analyze › Structural Equation Modeling: the model builder (checklist C3) for covariance-based SEM and PLS-SEM.
//
// The diagram and the text model are two views of one model, kept in step both ways: typing in the text redraws the
// diagram (the engine parses it and places new variables), and drawing on the diagram rewrites the text. Variables
// are rectangles, latent variables ovals and composites hexagons; drag a variable from the list onto the canvas, or
// add it with the keyboard. The Path and Covariance tools draw arrows between two clicks; a selected arrow can be
// labeled or fixed. Everything can also be done without a mouse: the text model, the relation list with its Add form,
// and on the canvas Tab to a variable, arrow keys to move it, Delete to remove it.

import { HelpButton } from './HelpDialog';
import { useEffect, useMemo, useRef, useState } from 'react';
import { call } from '../bridge';
import type { AnalysisDef, Collected, OptionValue } from '../analyses';
import type { DatasetInfo, OutputItem } from '../types';
import { boundary, inferEdge, modelText, paramName, size, validName, type DEdge, type DNode, type Diagram, type NodeKind } from '../semdiagram';
import { OptionList } from './AnalysisDialog';
import { Icon } from './Icon';

type Tool = 'select' | 'path' | 'cov';
type Selection = { node: string } | { edge: number } | null;

const RELATIONS = {
  sem: [
    { op: '=~', label: 'is measured by (=~)' },
    { op: '~', label: 'is regressed on (~)' },
    { op: '~~', label: 'covaries with (~~)' },
  ],
  pls: [
    { op: '=~', label: 'is measured by, mode A (=~)' },
    { op: '<~', label: 'is formed by, mode B (<~)' },
    { op: '~', label: 'is regressed on (~)' },
  ],
};

export function SemBuilder({ kind, def, dataset, initial, initialArgs, onClose, onDone, onPaste }: { kind: 'sem' | 'pls'; def: AnalysisDef; dataset: DatasetInfo; initial?: Collected; initialArgs?: Record<string, unknown> | null; onClose: () => void; onDone: (items: OutputItem[]) => void; onPaste?: (command: string, args: unknown) => void }) {
  const numeric = useMemo(() => dataset.variables.filter((v) => v.type === 'numeric'), [dataset]);
  const isVar = (n: string) => dataset.variables.some((v) => v.name === n);
  const [text, setText] = useState<string>(String(initial?.options.model ?? ''));
  const [layout, setLayout] = useState<Record<string, [number, number]>>(() => ((initialArgs?.layout as Record<string, [number, number]>) ?? {}));
  const [diagram, setDiagram] = useState<Diagram>({ nodes: [], edges: [], extras: [] });
  const [pending, setPending] = useState<DNode[]>([]);
  const [parseError, setParseError] = useState<string | null>(null);
  const [tool, setTool] = useState<Tool>('select');
  const [start, setStart] = useState<string | null>(null);
  const [sel, setSel] = useState<Selection>(null);
  const [picked, setPicked] = useState<string | null>(null);
  const [newLatent, setNewLatent] = useState('');
  const [nameError, setNameError] = useState<string | null>(null);
  const [form, setForm] = useState<{ a: string; op: string; b: string }>({ a: '', op: '=~', b: '' });
  const [options, setOptions] = useState<Record<string, OptionValue>>(() => ({ ...Object.fromEntries(def.options.map((o) => [o.key, o.default])), ...(initial?.options ?? {}) }));
  const [group, setGroup] = useState<string>(initial?.roles.group?.[0] ?? '');
  const [cluster, setCluster] = useState<string>(initial?.roles.cluster?.[0] ?? '');
  // Two-level models (level: 1 and level: 2) are edited as text: the diagram shows both levels together and can't
  // write the level blocks back.
  const twoLevel = /^\s*level\s*:/m.test(text);
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [say, setSay] = useState('');
  const svgRef = useRef<SVGSVGElement>(null);
  const drag = useRef<{ name: string; dx: number; dy: number } | null>(null);
  const layoutRef = useRef(layout);
  layoutRef.current = layout;

  // Text → diagram, after a pause in typing.
  useEffect(() => {
    if (!text.trim()) {
      setDiagram({ nodes: [], edges: [], extras: [] });
      setParseError(null);
      return;
    }
    const t = setTimeout(() => {
      call<Diagram>('sem_diagram', { model: text, layout: layoutRef.current })
        .then((d) => {
          setDiagram(d);
          setParseError(null);
          setLayout((l) => {
            const next = { ...l };
            for (const n of d.nodes) if (!next[n.name]) next[n.name] = [n.x, n.y];
            return next;
          });
          setPending((p) => p.filter((n) => !d.nodes.some((m) => m.name === n.name)));
        })
        .catch((e: Error) => setParseError(e.message));
    }, 250);
    return () => clearTimeout(t);
  }, [text]);

  const nodes: DNode[] = [...diagram.nodes, ...pending.filter((p) => !diagram.nodes.some((n) => n.name === p.name))].map((n) => {
    const p = layout[n.name];
    return p ? { ...n, x: p[0], y: p[1] } : n;
  });
  const node = (name: string) => nodes.find((n) => n.name === name);
  const edges = diagram.edges;

  // Diagram → text.
  const commit = (nextEdges: DEdge[], nextNodes: DNode[] = nodes) => {
    if (twoLevel) return setSay('Two-level models are edited in the text model; the diagram shows both levels together.');
    setDiagram((d) => ({ ...d, edges: nextEdges }));
    setText(modelText(nextNodes, nextEdges, diagram.extras));
  };
  const addEdge = (e: DEdge) => {
    if (kind === 'pls' && e.kind === 'covariance') return setSay('PLS-SEM has no covariances.');
    if (edges.some((x) => x.kind === e.kind && ((x.from === e.from && x.to === e.to) || (e.kind === 'covariance' && x.from === e.to && x.to === e.from)))) return setSay(`${paramName(e)} is already in the model.`);
    commit([...edges, e]);
    setSay(`Added ${paramName(e)}.`);
  };
  const removeEdge = (i: number) => {
    const e = edges[i];
    commit(edges.filter((_, k) => k !== i));
    setSel(null);
    setSay(`Removed ${paramName(e)}.`);
  };
  const removeNode = (name: string) => {
    const rest = nodes.filter((n) => n.name !== name);
    setPending((p) => p.filter((n) => n.name !== name));
    commit(
      edges.filter((e) => e.from !== name && e.to !== name),
      rest,
    );
    setSel(null);
    setSay(`Removed ${name}.`);
  };
  const ensureNode = (name: string, k: NodeKind, at?: [number, number]): DNode => {
    const have = node(name);
    if (have) return have;
    const pos = at ?? [80 + ((nodes.length * 97) % 700), 420 + Math.floor(nodes.length / 8) * 60];
    const n: DNode = { name, kind: k, x: pos[0], y: pos[1] };
    setPending((p) => [...p, n]);
    setLayout((l) => ({ ...l, [name]: pos }));
    return n;
  };

  const toSvg = (cx: number, cy: number): [number, number] => {
    const svg = svgRef.current;
    const m = svg?.getScreenCTM();
    if (!svg || !m) return [cx, cy];
    const p = new DOMPoint(cx, cy).matrixTransform(m.inverse());
    return [p.x, p.y];
  };
  const clickNode = (n: DNode) => {
    if (tool === 'select') return setSel({ node: n.name });
    if (!start) {
      setStart(n.name);
      return setSay(`From ${n.name}: now choose the other end.`);
    }
    const a = node(start);
    setStart(null);
    if (!a || a.name === n.name) return;
    addEdge(tool === 'cov' ? { from: a.name, to: n.name, kind: 'covariance' } : inferEdge(a, n));
  };
  const moveNode = (name: string, x: number, y: number) => setLayout((l) => ({ ...l, [name]: [Math.round(x), Math.round(y)] }));

  const addLatent = (k: NodeKind) => {
    const name = newLatent.trim();
    if (!validName(name)) return setNameError('Use letters, digits, _ and . (starting with a letter).');
    if (isVar(name)) return setNameError(`${name} is a variable in the data; a latent variable needs a new name.`);
    if (node(name)) return setNameError(`${name} is already in the diagram.`);
    ensureNode(name, k);
    setNameError(null);
    setNewLatent('');
    setSay(`Added ${name}. Draw arrows to its indicators, or use Add relation below.`);
  };
  const addRelation = () => {
    const { a, op, b } = form;
    if (!a || !b || a === b) return setSay('Choose two different variables.');
    const kindOf = (n: string): NodeKind => node(n)?.kind ?? (isVar(n) ? 'observed' : op === '<~' ? 'composite' : 'latent');
    ensureNode(a, op === '=~' ? 'latent' : op === '<~' ? 'composite' : kindOf(a));
    ensureNode(b, kindOf(b));
    if (op === '=~') addEdge({ from: a, to: b, kind: 'loading' });
    else if (op === '<~') addEdge({ from: b, to: a, kind: 'composite' });
    else if (op === '~') addEdge({ from: b, to: a, kind: 'regression' });
    else addEdge({ from: a, to: b, kind: 'covariance' });
  };
  const relayout = () => {
    if (!text.trim()) return;
    call<Diagram>('sem_diagram', { model: text, layout: {} })
      .then((d) => {
        setDiagram(d);
        setLayout(Object.fromEntries(d.nodes.map((n) => [n.name, [n.x, n.y] as [number, number]])));
        setSay('Laid out again.');
      })
      .catch((e: Error) => setParseError(e.message));
  };

  const selectedEdge = sel && 'edge' in sel ? edges[sel.edge] : undefined;
  const setModifier = (m: string) => {
    if (!sel || !('edge' in sel)) return;
    const next = edges.map((e, k) => (k === sel.edge ? { ...e, modifier: m.trim() || null } : e));
    commit(next);
  };

  // Canvas size from the positions.
  const maxX = Math.max(900, ...nodes.map((n) => n.x + 120));
  const maxY = Math.max(520, ...nodes.map((n) => n.y + 90));
  const minX = Math.min(0, ...nodes.map((n) => n.x - 120));
  const minY = Math.min(0, ...nodes.map((n) => n.y - 90));

  const collected = (): Collected => ({ roles: { group: group ? [group] : [], cluster: cluster ? [cluster] : [] }, pairs: {}, groups: {}, references: {}, options: { ...options, model: text } });
  const args = () => ({ ...(def.build(collected()) as Record<string, unknown>), layout: Object.fromEntries(nodes.map((n) => [n.name, [n.x, n.y]])) });
  const problem = !text.trim() ? 'Draw the model, or type it in the text model.' : parseError;
  const run = async () => {
    setRunning(true);
    setError(null);
    try {
      onDone(await call<OutputItem[]>(def.command, args()));
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setRunning(false);
    }
  };

  const id = (s: string) => `${def.id}-builder-${s}`;
  const relations = RELATIONS[kind];
  const names = [...nodes.map((n) => n.name), ...numeric.map((v) => v.name).filter((v) => !node(v))];

  return (
    <div className="overlay open" onKeyDown={(e) => e.key === 'Escape' && onClose()}>
      <div className="dialog wide" role="dialog" aria-modal="true" aria-labelledby={id('title')}>
        <header>
          <h2 id={id('title')}>{def.title}</h2>
          <HelpButton />
          <button type="button" className="iconbtn x" aria-label="Close" onClick={onClose}>
            <Icon name="close" />
          </button>
        </header>
        <div className="dbody">
          <div className="sembuilder">
            <div className="semvars">
              <span className="fieldlabel" id={id('vars')}>
                Variables
              </span>
              <ul className="listbox tall" role="listbox" aria-labelledby={id('vars')}>
                {numeric.map((v) => (
                  <li
                    key={v.name}
                    role="option"
                    aria-selected={picked === v.name}
                    tabIndex={0}
                    draggable
                    className={node(v.name) ? 'used' : undefined}
                    onDragStart={(e) => e.dataTransfer.setData('text/plain', v.name)}
                    onClick={() => setPicked(v.name)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') ensureNode(v.name, 'observed');
                    }}
                    onDoubleClick={() => ensureNode(v.name, 'observed')}
                  >
                    {v.name}
                  </li>
                ))}
              </ul>
              <button type="button" className="btn" disabled={!picked || !!(picked && node(picked))} onClick={() => picked && (ensureNode(picked, 'observed'), setSay(`Added ${picked}.`))}>
                Add to diagram
              </button>
              <label className="fieldlabel" htmlFor={id('latent')}>
                New {kind === 'pls' ? 'construct' : 'latent variable'}
              </label>
              <input id={id('latent')} className="textinput" value={newLatent} onChange={(e) => setNewLatent(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && addLatent('latent')} placeholder="name" />
              <div className="row">
                <button type="button" className="btn" onClick={() => addLatent('latent')}>
                  {kind === 'pls' ? 'Add (mode A)' : 'Add oval'}
                </button>
                {kind === 'pls' && (
                  <button type="button" className="btn" onClick={() => addLatent('composite')}>
                    Add (mode B)
                  </button>
                )}
              </div>
              {nameError && <span className="fieldhint err">{nameError}</span>}
            </div>

            <div className="semcanvas">
              <div className="semtools" role="toolbar" aria-label="Diagram tools">
                {(
                  [
                    ['select', 'Select and move'],
                    ['path', 'Path (one-headed arrow)'],
                    ...(kind === 'sem' ? [['cov', 'Covariance (two-headed arrow)']] : []),
                  ] as [Tool, string][]
                ).map(([t, label]) => (
                  <button key={t} type="button" className={`btn${tool === t ? ' on' : ''}`} aria-pressed={tool === t} onClick={() => (setTool(t), setStart(null))}>
                    {label}
                  </button>
                ))}
                <button type="button" className="btn" onClick={relayout}>
                  Automatic layout
                </button>
              </div>
              <div className="semscroll">
                <svg
                  ref={svgRef}
                  role="group"
                  aria-label="Path diagram. Tab to a variable; arrow keys move it, Delete removes it, Enter picks it as an arrow's end."
                  viewBox={`${minX} ${minY} ${maxX - minX} ${maxY - minY}`}
                  width={maxX - minX}
                  height={maxY - minY}
                  onPointerMove={(e) => {
                    if (!drag.current) return;
                    const [x, y] = toSvg(e.clientX, e.clientY);
                    moveNode(drag.current.name, x - drag.current.dx, y - drag.current.dy);
                  }}
                  onPointerUp={() => (drag.current = null)}
                  onPointerLeave={() => (drag.current = null)}
                  onClick={(e) => {
                    if (e.target === svgRef.current) setSel(null);
                  }}
                  onDragOver={(e) => e.preventDefault()}
                  onDrop={(e) => {
                    e.preventDefault();
                    const name = e.dataTransfer.getData('text/plain');
                    if (!name || !isVar(name)) return;
                    const at = toSvg(e.clientX, e.clientY);
                    if (node(name)) moveNode(name, at[0], at[1]);
                    else ensureNode(name, 'observed', [Math.round(at[0]), Math.round(at[1])]);
                  }}
                >
                  <defs>
                    <marker id={id('arrow')} viewBox="0 0 10 10" refX="10" refY="5" markerWidth="8" markerHeight="8" orient="auto-start-reverse">
                      <path d="M0,0 L10,5 L0,10 z" fill="currentColor" />
                    </marker>
                  </defs>
                  {edges.map((e, k) => {
                    const a = node(e.from);
                    const b = node(e.to);
                    if (!a || !b) return null;
                    const selected = !!sel && 'edge' in sel && sel.edge === k;
                    let d: string;
                    let lx: number;
                    let ly: number;
                    if (e.kind === 'covariance') {
                      const mx = (a.x + b.x) / 2;
                      const my = (a.y + b.y) / 2;
                      const dx = b.x - a.x;
                      const dy = b.y - a.y;
                      const len = Math.max(Math.hypot(dx, dy), 1e-9);
                      const ext = (n: DNode) => (Math.abs(dy / len) * size(n.kind).w) / 2 + (Math.abs(dx / len) * size(n.kind).h) / 2;
                      const bow = 0.25 * len + 25 + Math.max(ext(a), ext(b));
                      const cx = mx - (dy / len) * bow;
                      const cy = my + (dx / len) * bow;
                      const [sx, sy] = boundary(a, cx, cy);
                      const [ex, ey] = boundary(b, cx, cy);
                      d = `M${sx},${sy} Q${cx},${cy} ${ex},${ey}`;
                      lx = 0.25 * sx + 0.5 * cx + 0.25 * ex;
                      ly = 0.25 * sy + 0.5 * cy + 0.25 * ey;
                    } else {
                      const [sx, sy] = boundary(a, b.x, b.y);
                      const [ex, ey] = boundary(b, a.x, a.y);
                      d = `M${sx},${sy} L${ex},${ey}`;
                      lx = (sx + ex) / 2;
                      ly = (sy + ey) / 2;
                    }
                    return (
                      <g key={k} className={`semedge${selected ? ' sel' : ''}`} onClick={(ev) => (ev.stopPropagation(), setSel({ edge: k }))}>
                        <path d={d} className="hit" />
                        <path d={d} className="line" markerEnd={`url(#${id('arrow')})`} markerStart={e.kind === 'covariance' ? `url(#${id('arrow')})` : undefined} />
                        {e.modifier && (
                          <text x={lx} y={ly - 6} textAnchor="middle" className="mod">
                            {e.modifier}
                          </text>
                        )}
                      </g>
                    );
                  })}
                  {nodes.map((n) => {
                    const { w, h } = size(n.kind);
                    const selected = (!!sel && 'node' in sel && sel.node === n.name) || start === n.name;
                    const missing = n.kind === 'observed' && !isVar(n.name);
                    return (
                      <g
                        key={n.name}
                        className={`semnode${selected ? ' sel' : ''}${missing ? ' missing' : ''}`}
                        tabIndex={0}
                        role="button"
                        aria-label={`${n.name}, ${n.kind === 'observed' ? 'observed variable' : n.kind === 'latent' ? 'latent variable' : 'composite'}${missing ? ', not in the data' : ''}`}
                        onPointerDown={(e) => {
                          if (tool !== 'select') return;
                          const [x, y] = toSvg(e.clientX, e.clientY);
                          drag.current = { name: n.name, dx: x - n.x, dy: y - n.y };
                        }}
                        onClick={(e) => (e.stopPropagation(), clickNode(n))}
                        onKeyDown={(e) => {
                          const step = e.shiftKey ? 50 : 10;
                          const moves: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
                          if (moves[e.key]) {
                            e.preventDefault();
                            moveNode(n.name, n.x + moves[e.key][0], n.y + moves[e.key][1]);
                          } else if (e.key === 'Delete' || e.key === 'Backspace') {
                            e.preventDefault();
                            removeNode(n.name);
                          } else if (e.key === 'Enter' || e.key === ' ') {
                            e.preventDefault();
                            clickNode(n);
                          }
                        }}
                      >
                        {n.kind === 'observed' && <rect x={n.x - w / 2} y={n.y - h / 2} width={w} height={h} />}
                        {n.kind === 'latent' && <ellipse cx={n.x} cy={n.y} rx={w / 2} ry={h / 2} />}
                        {n.kind === 'composite' && <polygon points={[[-w / 2, 0], [-w / 4, -h / 2], [w / 4, -h / 2], [w / 2, 0], [w / 4, h / 2], [-w / 4, h / 2]].map(([dx, dy]) => `${n.x + dx},${n.y + dy}`).join(' ')} />}
                        <text x={n.x} y={n.y + 4.5} textAnchor="middle">
                          {n.name}
                        </text>
                      </g>
                    );
                  })}
                </svg>
              </div>
              <div className="fieldhint" aria-live="polite">
                {say || (tool === 'select' ? 'Drag variables onto the canvas; drag to move them. Choose Path or Covariance, then click two variables.' : start ? `From ${start}: click the other end.` : 'Click the first variable.')}
              </div>
            </div>

            <div className="semside">
              {selectedEdge ? (
                <div className="semprops">
                  <b>{paramName(selectedEdge)}</b>
                  <label>
                    Label or fixed value{' '}
                    <input key={`${sel && 'edge' in sel ? sel.edge : ''}-${selectedEdge.modifier ?? ''}`} className="textinput" defaultValue={selectedEdge.modifier ?? ''} placeholder="a, 1, NA" onBlur={(e) => setModifier(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && setModifier((e.target as HTMLInputElement).value)} />
                  </label>
                  <span className="fieldhint">A name labels the parameter (same names are held equal); a number fixes it; NA frees a loading fixed by default.</span>
                  {node(selectedEdge.from)?.kind === 'latent' && node(selectedEdge.to)?.kind === 'latent' && (
                    <label>
                      Kind{' '}
                      <select
                        value={selectedEdge.kind}
                        onChange={(e) => commit(edges.map((x, k) => (sel && 'edge' in sel && k === sel.edge ? { ...x, kind: e.target.value as DEdge['kind'] } : x)))}
                      >
                        <option value="loading">Loading: a higher-order factor (=~)</option>
                        <option value="regression">Regression (~)</option>
                      </select>
                    </label>
                  )}
                  <button type="button" className="btn" onClick={() => sel && 'edge' in sel && removeEdge(sel.edge)}>
                    Remove arrow
                  </button>
                </div>
              ) : sel && 'node' in sel ? (
                <div className="semprops">
                  <b>{sel.node}</b>
                  <button type="button" className="btn" onClick={() => removeNode(sel.node)}>
                    Remove from model
                  </button>
                </div>
              ) : null}
              <span className="fieldlabel" id={id('rels')}>
                Relations
              </span>
              <ul className="semrels" aria-labelledby={id('rels')}>
                {edges.map((e, k) => (
                  <li key={k}>
                    <button type="button" className="linkish" onClick={() => setSel({ edge: k })}>
                      {e.modifier ? `${paramName(e)} (${e.modifier})` : paramName(e)}
                    </button>
                    <button type="button" className="iconbtn" aria-label={`Remove ${paramName(e)}`} onClick={() => removeEdge(k)}>
                      <Icon name="close" />
                    </button>
                  </li>
                ))}
                {!edges.length && <li className="empty">None yet</li>}
              </ul>
              <fieldset className="semadd">
                <legend>Add relation</legend>
                <select aria-label="First variable" value={form.a} onChange={(e) => setForm({ ...form, a: e.target.value })}>
                  <option value="">Choose…</option>
                  {names.map((n) => (
                    <option key={n}>{n}</option>
                  ))}
                </select>
                <select aria-label="Relation" value={form.op} onChange={(e) => setForm({ ...form, op: e.target.value })}>
                  {relations.map((r) => (
                    <option key={r.op} value={r.op}>
                      {r.label}
                    </option>
                  ))}
                </select>
                <select aria-label="Second variable" value={form.b} onChange={(e) => setForm({ ...form, b: e.target.value })}>
                  <option value="">Choose…</option>
                  {names.map((n) => (
                    <option key={n}>{n}</option>
                  ))}
                </select>
                <button type="button" className="btn" onClick={addRelation}>
                  Add
                </button>
              </fieldset>
            </div>
          </div>

          <details className="opt" open>
            <summary>Text model</summary>
            <div className="optbody">
              <label className="fieldlabel" htmlFor={id('text')}>
                Model (lavaan syntax; kept in step with the diagram)
              </label>
              <textarea id={id('text')} className="textinput code" rows={7} spellCheck={false} value={text} onChange={(e) => setText(e.target.value)} />
              {parseError && (
                <div className="inline-err" role="alert">
                  <Icon name="warn" />
                  {parseError}
                </div>
              )}
            </div>
          </details>
          <details className="opt">
            <summary>
              Groups and clusters <span className="cur">{[group, cluster && `clusters: ${cluster}`].filter(Boolean).join('; ') || 'none'}</span>
            </summary>
            <div className="optbody">
              {kind === 'sem' && (
                <label>
                  Clusters (a two-level model, with level: 1 and level: 2 in the text){' '}
                  <select value={cluster} onChange={(e) => setCluster(e.target.value)}>
                    <option value="">(none)</option>
                    {dataset.variables.map((v) => (
                      <option key={v.name}>{v.name}</option>
                    ))}
                  </select>
                </label>
              )}
              <label>
                {kind === 'pls' ? 'Two groups to compare' : 'A multigroup model across'}{' '}
                <select value={group} onChange={(e) => setGroup(e.target.value)}>
                  <option value="">(none)</option>
                  {dataset.variables.map((v) => (
                    <option key={v.name}>{v.name}</option>
                  ))}
                </select>
              </label>
            </div>
          </details>
          <OptionList def={def} options={options} setOptions={setOptions} skip={['model']} />
          {error && (
            <div className="errbox" role="alert">
              {error}
            </div>
          )}
          {problem && !parseError && (
            <div className="inline-err" aria-live="polite">
              <Icon name="info" />
              {problem}
            </div>
          )}
        </div>
        <footer>
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          {onPaste && (
            <button type="button" className="btn" disabled={!!problem} onClick={() => onPaste(def.command, args())} title="Add this analysis to the script without running it">
              Paste
            </button>
          )}
          <button type="button" className="btn primary" disabled={!!problem || running} onClick={run}>
            <Icon name="run" />
            {running ? 'Running…' : 'Run'}
          </button>
        </footer>
      </div>
    </div>
  );
}
