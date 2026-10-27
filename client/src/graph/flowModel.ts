// A flow's state machine as the chart draws it, before it is laid out: the cells, each in the part of the chart it is
// drawn in and the stage box around it; an arrow for the rows from one cell to another in the same part, numbered and
// named; a small circle beside a cell for a row to another part, and a shaded one beside the cell it enters; one
// circle beside each agent's cell for the rows every agent's cell shares; the rules beside the cells that are given
// them; the interjection beside the engine's steps that take one. Pure, so that it is tested on its own.

import type { CompiledCell, CompiledGraph, GraphView, RoundSummary } from '../api';

export type NodeKind = 'cell' | 'shared' | 'out' | 'in' | 'rule' | 'interjection';

export type ChartNode = {
  id: string;
  kind: NodeKind;
  /// The part of the chart it is laid out in: `main`, `asking`, or the id of a cell drawn apart.
  part: string;
  /// The stage box it sits in, or the asking part's box; none for no box.
  group: string | null;
  /// The cell it is, or sits beside.
  cell: string;
  /// For a circle, the cell it leads to or comes from; for a rule, the rule.
  other: string | null;
  /// For the shared circle, the rows it carries, by their place in the cell's transition table.
  rows: number[];
};

export type ChartEdge = {
  id: string;
  source: string;
  target: string;
  /// The cell whose rows the arrow draws, and the rows, by their place in its transition table; none for the arrow
  /// of a rule or the interjection into its cell.
  cell: string | null;
  rows: number[];
  label: string;
  /// Some row of it is on the main path.
  main: boolean;
  kind: 'row' | 'shared' | 'rule' | 'interjection';
};

export type ChartGroup = { id: string; label: string; part: string };

export type Chart = { nodes: ChartNode[]; edges: ChartEdge[]; groups: ChartGroup[]; parts: string[] };

export const MAIN = 'main';
export const ASKING = 'asking';

const CIRCLED = '①②③④⑤⑥⑦⑧⑨⑩⑪⑫⑬⑭⑮⑯⑰⑱⑲⑳';

/// A row's number as the chart and the transition table write it: circled, from ①.
export function circled(i: number): string {
  return i < CIRCLED.length ? CIRCLED[i] : `(${i + 1})`;
}

const SELF = '@self';
const CALLER = '@caller';
const ASKER = '@asker';
const STOPPED = '@stopped';

/// The rows an agent's cell shares with every agent's cell, which the shared circle beside it carries: those that
/// lead elsewhere, and those that come back to the cell itself (a pause, answers, a return sent back, a run that
/// ended without a return).
export function sharedRows(c: CompiledCell): number[] {
  if (c.kind !== 'agent') return [];
  return c.rows.flatMap((r, i) => (r.common ? [i] : []));
}

/// Rows' numbers, circled, three or more in a row written as the first and the last.
export function numbers(rows: number[]): string {
  const out: string[] = [];
  let i = 0;
  while (i < rows.length) {
    let j = i;
    while (j + 1 < rows.length && rows[j + 1] === rows[j] + 1) j += 1;
    out.push(j - i >= 2 ? `${circled(rows[i])}–${circled(rows[j])}` : rows.slice(i, j + 1).map(circled).join(''));
    i = j + 1;
  }
  return out.join('');
}

/// The part each cell is drawn in. The asking part is a part of its own; a cell outside it that no row leads to but
/// the rows every agent's cell shares and the rows back to where the work came from (such as the cell that runs the
/// tools agents ask for) is drawn apart; every other cell is in the main part.
export function partsOf(g: CompiledGraph, v: GraphView): Map<string, string> {
  const asking = new Set(g.asking);
  const cells = new Map(g.cells.map((c) => [c.id, c]));
  const out = new Map<string, string>();
  for (const cv of v.cells) {
    if (asking.has(cv.id)) {
      out.set(cv.id, ASKING);
      continue;
    }
    const wayIn = cv.entered_from.some((r) => {
      const row = cells.get(r.cell)?.rows[r.row];
      return row !== undefined && !row.common && !row.to.startsWith('@');
    });
    out.set(cv.id, cv.id === g.start || wayIn ? MAIN : cv.id);
  }
  return out;
}

/// The label of an arrow: the rows' numbers and names, or only the names when the cell has one row.
function label(c: CompiledCell, rows: number[], names = true): string {
  const nos = c.rows.length > 1 ? numbers(rows) : '';
  const said = names ? [...new Set(rows.map((i) => c.rows[i].name))].join(', ') : '';
  return [nos, said].filter((s) => s !== '').join(' ');
}

export function chart(g: CompiledGraph, v: GraphView): Chart {
  const parts = partsOf(g, v);
  const cells = new Map(g.cells.map((c) => [c.id, c]));
  const views = new Map(v.cells.map((c) => [c.id, c]));
  const groupOf = (id: string): string | null => {
    const part = parts.get(id);
    if (part === ASKING) return ASKING;
    if (part !== MAIN) return null;
    const stage = cells.get(id)?.stage;
    return stage ? `stage:${stage}` : null;
  };
  const nodes = new Map<string, ChartNode>();
  const node = (n: Omit<ChartNode, 'part' | 'group'> & { beside?: string }) => {
    if (nodes.has(n.id)) return;
    const at = n.beside ?? n.cell;
    nodes.set(n.id, {
      id: n.id,
      kind: n.kind,
      cell: n.cell,
      other: n.other,
      rows: n.rows,
      part: parts.get(at) ?? MAIN,
      group: groupOf(at),
    });
  };
  const edges = new Map<string, ChartEdge>();
  // Rows of one cell to the same place are one arrow.
  const arrow = (source: string, target: string, c: CompiledCell, row: number, kind: ChartEdge['kind'] = 'row') => {
    const id = `${source}->${target}`;
    const e = edges.get(id);
    if (e) {
      if (!e.rows.includes(row)) e.rows.push(row);
      e.main = e.main || c.rows[row].main;
    } else {
      edges.set(id, { id, source, target, cell: c.id, rows: [row], label: '', main: c.rows[row].main, kind });
    }
  };
  const cellNode = (id: string) => `c:${id}`;
  for (const c of g.cells) node({ id: cellNode(c.id), kind: 'cell', cell: c.id, other: null, rows: [] });
  // A row to a cell: an arrow within a part; between parts, a circle beside the cell it leaves, and a shaded one
  // beside the cell it enters when it comes back into the main part.
  const toCell = (c: CompiledCell, row: number, t: string) => {
    if (!cells.has(t)) return;
    if (parts.get(t) === parts.get(c.id)) {
      arrow(cellNode(c.id), cellNode(t), c, row);
      return;
    }
    const out = `o:${t}:${c.id}`;
    node({ id: out, kind: 'out', cell: c.id, other: t, rows: [] });
    arrow(cellNode(c.id), out, c, row);
    if (parts.get(c.id) !== MAIN && parts.get(t) === MAIN) {
      const into = `i:${c.id}:${t}`;
      node({ id: into, kind: 'in', cell: t, other: c.id, rows: [] });
      arrow(into, cellNode(t), c, row);
    }
  };
  for (const c of g.cells) {
    const shared = new Set(sharedRows(c));
    const cv = views.get(c.id);
    if (shared.size > 0) {
      const id = `k:${c.id}`;
      node({ id, kind: 'shared', cell: c.id, other: null, rows: [...shared] });
      for (const i of shared) arrow(cellNode(c.id), id, c, i, 'shared');
    }
    c.rows.forEach((r, i) => {
      if (shared.has(i) || r.to === SELF || r.to === CALLER) return;
      if (r.to === STOPPED) return;
      const targets = r.to === ASKER ? (cv?.targets[i] ?? []) : [r.to];
      for (const t of targets) toCell(c, i, t);
    });
  }
  // A row back to where the work stopped is drawn into each cell it can be, from the cell whose row leads to the
  // cell that goes back: that row decided it.
  for (const c of g.cells) {
    const cv = views.get(c.id);
    c.rows.forEach((r, i) => {
      if (r.to !== STOPPED) return;
      for (const t of cv?.targets[i] ?? []) {
        for (const from of views.get(c.id)?.entered_from ?? []) {
          const m = cells.get(from.cell);
          if (!m || from.cell === c.id) continue;
          const into = `i:${m.id}:${t}`;
          node({ id: into, kind: 'in', cell: t, other: m.id, rows: [] });
          arrow(into, cellNode(t), m, from.row);
        }
      }
    });
  }
  // The rules beside the cells that name them (those every cell of a line is given are listed in each cell's inputs,
  // not drawn), and the interjection beside the engine's steps that take one.
  for (const c of g.cells) {
    for (const r of (c.rules ?? []).filter((x) => !x.shared)) {
      const id = `r:${r.name}:${c.id}`;
      node({ id, kind: 'rule', cell: c.id, other: r.name, rows: [] });
      edges.set(`${id}->${cellNode(c.id)}`, {
        id: `${id}->${cellNode(c.id)}`,
        source: id,
        target: cellNode(c.id),
        cell: null,
        rows: [],
        label: '',
        main: false,
        kind: 'rule',
      });
    }
    if (c.kind !== 'agent' && g.interject && c.rows.some((r) => r.interject)) {
      const id = `j:${c.id}`;
      node({ id, kind: 'interjection', cell: c.id, other: null, rows: [] });
      edges.set(`${id}->${cellNode(c.id)}`, {
        id: `${id}->${cellNode(c.id)}`,
        source: id,
        target: cellNode(c.id),
        cell: null,
        rows: [],
        label: '',
        main: false,
        kind: 'interjection',
      });
    }
  }
  for (const e of edges.values()) {
    const c = e.cell !== null ? cells.get(e.cell) : undefined;
    if (!c) continue;
    e.rows.sort((a, b) => a - b);
    e.label = label(c, e.rows, e.kind !== 'shared');
  }
  const groups: ChartGroup[] = [];
  const seen = new Set<string>();
  for (const c of g.cells) {
    const id = groupOf(c.id);
    if (id === null || seen.has(id)) continue;
    seen.add(id);
    groups.push({ id, label: id === ASKING ? ASKING : id.slice('stage:'.length), part: parts.get(c.id) ?? MAIN });
  }
  const partList = [MAIN, ...new Set([...parts.values()].filter((p) => p !== MAIN))];
  return { nodes: [...nodes.values()], edges: [...edges.values()], groups, parts: partList };
}

/// What a work did at a cell.
export type Overlay = {
  rounds: number;
  sentBack: number;
  failed: number;
  running: boolean;
  current: boolean;
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

/// How many times the work took each arrow of the chart: a round that took a row counts on every arrow that draws
/// that row of its cell.
export function taken(c: Chart, g: CompiledGraph, rounds: RoundSummary[]): Map<string, number> {
  const rowOf = new Map<string, number>();
  for (const cell of g.cells) cell.rows.forEach((r, i) => rowOf.set(`${cell.id}\u0000${r.name}`, i));
  const byRow = new Map<string, ChartEdge[]>();
  for (const e of c.edges) {
    if (e.cell === null) continue;
    for (const i of e.rows) {
      const k = `${e.cell}\u0000${i}`;
      byRow.set(k, [...(byRow.get(k) ?? []), e]);
    }
  }
  const nodes = new Map(c.nodes.map((n) => [n.id, n]));
  const out = new Map<string, number>();
  rounds.forEach((r, n) => {
    if (r.cell === null || r.transition === null) return;
    const i = rowOf.get(`${r.cell}\u0000${r.transition}`);
    if (i === undefined) return;
    // The cells the work went to next: the next one, or the one after it when the next goes straight back.
    const next = rounds
      .slice(n + 1)
      .filter((x) => x.cell !== null)
      .slice(0, 2)
      .map((x) => x.cell);
    for (const e of byRow.get(`${r.cell}\u0000${i}`) ?? []) {
      // An arrow to one of several cells a row can lead to counts only when the work went there.
      const into = nodes.get(e.target);
      const lands = into?.kind === 'cell' ? into.cell : into?.kind === 'out' ? into.other : null;
      const there = e.kind === 'shared' || lands === null || next.length === 0 || next.includes(lands);
      if (there) out.set(e.id, (out.get(e.id) ?? 0) + 1);
    }
  });
  return out;
}
