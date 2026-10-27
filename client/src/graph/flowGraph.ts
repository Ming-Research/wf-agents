// A flow's state machine as nodes and edges, and a work's path over it. Pure, so that it is tested on its own; the
// chart lays it out and draws it.

import type { CompiledCell, CompiledGraph, RoundSummary } from '../api';

/// What a work did at a cell.
export type Overlay = {
  rounds: number;
  sentBack: number;
  failed: number;
  running: boolean;
  current: boolean;
};

export type FlowNode = { id: string; cell: CompiledCell; overlay: Overlay | null };

/// The rows from one cell to another, drawn as one edge.
export type FlowEdge = {
  id: string;
  source: string;
  target: string;
  rows: string[];
  /// Every row of it is one every agent's cell has.
  common: boolean;
  main: boolean;
  /// How many times the work went this way.
  taken: number;
};

/// What the work's rounds did at each cell; `at` is the cell the work is at.
export function overlays(rounds: RoundSummary[], at: string | null): Map<string, Overlay> {
  const out = new Map<string, Overlay>();
  const get = (cell: string) => {
    let o = out.get(cell);
    if (!o) {
      o = { rounds: 0, sentBack: 0, failed: 0, running: false, current: false };
      out.set(cell, o);
    }
    return o;
  };
  for (const r of rounds) {
    if (r.cell === null) continue;
    const o = get(r.cell);
    o.rounds += 1;
    if (r.outcome === 'sent_back') o.sentBack += 1;
    if (r.outcome === 'failed' || r.outcome === 'interrupted') o.failed += 1;
    if (r.outcome === 'running') o.running = true;
  }
  if (at !== null) get(at).current = true;
  return out;
}

const edgeId = (source: string, target: string) => `${source}->${target}`;

/// The ways the work went, as edge ids with how many times, and the row names each was taken by. A row to a fixed
/// cell goes there; a row to where the work came from or was asked goes to the cell of the next round.
export function taken(g: CompiledGraph, rounds: RoundSummary[]): Map<string, { times: number; rows: Set<string> }> {
  const cells = new Map(g.cells.map((c) => [c.id, c]));
  const out = new Map<string, { times: number; rows: Set<string> }>();
  rounds.forEach((r, i) => {
    if (r.cell === null || r.transition === null) return;
    const row = cells.get(r.cell)?.rows.find((x) => x.name === r.transition);
    const target =
      row && cells.has(row.to) ? row.to : (rounds.slice(i + 1).find((n) => n.cell !== null)?.cell ?? null);
    if (target === null || target === r.cell) return;
    const id = edgeId(r.cell, target);
    const e = out.get(id) ?? { times: 0, rows: new Set<string>() };
    e.times += 1;
    e.rows.add(r.transition);
    out.set(id, e);
  });
  return out;
}

/// The nodes and edges to draw: every cell, and the rows between cells; rows every agent's cell has only when
/// `common` asks for them, or when the work went that way. A row to the cell itself, or to where the work came from,
/// is not an edge: the cell's definition lists it.
export function build(
  g: CompiledGraph,
  opts: { common: boolean; rounds?: RoundSummary[]; at?: string | null },
): { nodes: FlowNode[]; edges: FlowEdge[] } {
  const rounds = opts.rounds ?? [];
  const over = opts.rounds ? overlays(rounds, opts.at ?? null) : null;
  const went = taken(g, rounds);
  const ids = new Set(g.cells.map((c) => c.id));
  const edges = new Map<string, FlowEdge>();
  for (const c of g.cells) {
    for (const r of c.rows) {
      if (!ids.has(r.to) || r.to === c.id) continue;
      const id = edgeId(c.id, r.to);
      const e = edges.get(id);
      if (e) {
        e.rows.push(r.name);
        e.common = e.common && r.common;
        e.main = e.main || r.main;
      } else {
        edges.set(id, { id, source: c.id, target: r.to, rows: [r.name], common: r.common, main: r.main, taken: 0 });
      }
    }
  }
  for (const [id, w] of went) {
    const e = edges.get(id);
    if (e) {
      e.taken = w.times;
    } else {
      const [source, target] = id.split('->');
      edges.set(id, { id, source, target, rows: [...w.rows], common: false, main: false, taken: w.times });
    }
  }
  return {
    nodes: g.cells.map((cell) => ({ id: cell.id, cell, overlay: over?.get(cell.id) ?? null })),
    edges: [...edges.values()].filter((e) => opts.common || !e.common || e.taken > 0),
  };
}
