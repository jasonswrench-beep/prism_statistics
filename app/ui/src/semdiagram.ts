// The path diagram of a model in lavaan's syntax, as the engine's `sem_diagram` command returns it, and the model text
// rebuilt from a diagram. The text is the source of truth: the engine parses it (and lays out new variables), and
// edits made on the diagram are written back as text. Statements a diagram doesn't draw (defined parameters,
// constraints, intercepts, variances) come back as `extras` and are kept at the end.

export type NodeKind = 'observed' | 'latent' | 'composite';
export type EdgeKind = 'loading' | 'composite' | 'regression' | 'covariance';

export interface DNode {
  name: string;
  kind: NodeKind;
  x: number;
  y: number;
}

export interface DEdge {
  from: string;
  to: string;
  kind: EdgeKind;
  modifier?: string | null;
}

export interface Diagram {
  nodes: DNode[];
  edges: DEdge[];
  extras: string[];
}

export const OBS = { w: 84, h: 40 };
export const LAT = { w: 104, h: 60 };

export const size = (k: NodeKind) => (k === 'observed' ? OBS : LAT);

/** The parameter an edge is, as the results name it. */
export function paramName(e: DEdge): string {
  switch (e.kind) {
    case 'loading':
      return `${e.from} =~ ${e.to}`;
    case 'composite':
      return `${e.to} <~ ${e.from}`;
    case 'regression':
      return `${e.to} ~ ${e.from}`;
    default:
      return `${e.from} ~~ ${e.to}`;
  }
}

const term = (name: string, modifier?: string | null) => (modifier ? `${modifier}*${name}` : name);

/** The model text of a diagram: blocks first (in node order), then regressions, covariances and the extras. */
export function modelText(nodes: { name: string; kind: NodeKind }[], edges: DEdge[], extras: string[]): string {
  const lines: string[] = [];
  for (const n of nodes) {
    if (n.kind === 'latent') {
      const ind = edges.filter((e) => e.kind === 'loading' && e.from === n.name);
      if (ind.length) lines.push(`${n.name} =~ ${ind.map((e) => term(e.to, e.modifier)).join(' + ')}`);
    }
    if (n.kind === 'composite') {
      const ind = edges.filter((e) => e.kind === 'composite' && e.to === n.name);
      if (ind.length) lines.push(`${n.name} <~ ${ind.map((e) => term(e.from, e.modifier)).join(' + ')}`);
    }
  }
  const outcomes: string[] = [];
  for (const e of edges) if (e.kind === 'regression' && !outcomes.includes(e.to)) outcomes.push(e.to);
  for (const y of outcomes) {
    lines.push(`${y} ~ ${edges.filter((e) => e.kind === 'regression' && e.to === y).map((e) => term(e.from, e.modifier)).join(' + ')}`);
  }
  for (const e of edges.filter((e) => e.kind === 'covariance')) lines.push(`${e.from} ~~ ${term(e.to, e.modifier)}`);
  lines.push(...extras);
  return lines.join('\n');
}

/** What an edge drawn from `a` to `b` means, from the kinds of its ends. */
export function inferEdge(a: { name: string; kind: NodeKind }, b: { name: string; kind: NodeKind }): DEdge {
  if (a.kind === 'latent' && b.kind === 'observed') return { from: a.name, to: b.name, kind: 'loading' };
  if (a.kind === 'observed' && b.kind === 'composite') return { from: a.name, to: b.name, kind: 'composite' };
  return { from: a.name, to: b.name, kind: 'regression' };
}

/** Where the line from a node's center toward (tx, ty) leaves its outline. */
export function boundary(n: DNode, tx: number, ty: number): [number, number] {
  const dx = tx - n.x;
  const dy = ty - n.y;
  const { w, h } = size(n.kind);
  let t: number;
  if (n.kind === 'observed') {
    const sx = Math.abs(dx) > 1e-9 ? w / 2 / Math.abs(dx) : Infinity;
    const sy = Math.abs(dy) > 1e-9 ? h / 2 / Math.abs(dy) : Infinity;
    t = Math.min(sx, sy);
  } else {
    t = 1 / Math.sqrt((dx / (w / 2)) ** 2 + (dy / (h / 2)) ** 2);
  }
  t = Math.min(t, 1);
  return [n.x + dx * t, n.y + dy * t];
}

export const validName = (s: string) => /^[A-Za-z_.][A-Za-z0-9_.]*$/.test(s);
