// A flow's state machine as the chart draws it, before it is laid out: the cells, each in the part of the chart it is
// drawn in and the stage box around it; an arrow for each cell's own answers to another cell in the same part, named,
// through a node for what the engine does on the rows, in the box of the cell the line goes into and just before it
// (lines with the same actions into the same cell share one);
// a small circle beside a cell for a row to another part, and a shaded one beside the cell it enters; the actions a
// cell does every time beside it; the rules beside the cells that are given them; the interjection beside the engine's
// steps that take one; and what the engine does on a row back to whichever cell called, which is not drawn, beside
// the cell it leaves. What an agent's cell may do of its own (run tools, search, ask, pause, answer, redirect) is
// drawn in three tiers by how many of the cells that could do it do: every one, in the legend; more than half, in the
// legend with a mark on each cell that does not; half or fewer, as a line from each cell that does. The way back from
// such a move is drawn for no cell: an answer back to whichever cell asked goes only into the cells whose own answers
// asked. What the engine does when a round ends in no return is the same for every cell and not drawn. The places the
// engine keeps what it writes are drawn apart (`stores.tsx`), and beside each cell, as a small box of its own, each
// store the cell reads or writes. Pure, so that it is tested on its own.

import type { Atom, CompiledCell, CompiledGraph, CompiledRow, EngineNames, GraphView, Layer, RoundSummary } from '../api';

export type NodeKind = 'cell' | 'out' | 'in' | 'rule' | 'interjection' | 'action' | 'does' | 'store';

/// How a cell uses a store beside it: reads it, writes it, or both.
export type StoreUse = 'read' | 'write' | 'both';

export type ChartNode = {
  id: string;
  kind: NodeKind;
  /// The part of the chart it is laid out in: `main`, `asking`, or the id of a cell drawn apart.
  part: string;
  /// The stage box it sits in, or the asking part's box; none for no box.
  group: string | null;
  /// The cell it is, or sits beside; for the actions on a line, the cell the line leads into.
  cell: string;
  /// For a circle, the cell it leads to or comes from; for a rule, the rule; for a store beside a cell, the store.
  other: string | null;
  /// For the actions on a line and those a cell does every time: the actions, by name, in order.
  actions: string[];
  /// For a store beside a cell: how the cell uses it; none for every other node.
  use: StoreUse | null;
};

export type ChartEdge = {
  id: string;
  source: string;
  target: string;
  /// The cell whose rows the arrow draws, and the rows, by their place in its transition table; none for the arrow
  /// out of the actions on a line, of a rule, of the interjection, to what a cell does every time or to or from a store
  /// beside it.
  cell: string | null;
  rows: number[];
  /// The rows of every cell the arrow out of the actions on a line draws: those through it.
  through: Array<{ cell: string; row: number }>;
  /// The rows' names; none for an arrow out of the actions on a line, which its arrows in name.
  label: string;
  /// Some row of it is on the main path: a cell's own answer the flow marks so, never a move.
  main: boolean;
  kind: 'row' | 'rule' | 'interjection' | 'does' | 'store';
};

/// A box around a stage's cells or the asking part: its label, and the letter its cells' codes start with, if they share one.
export type ChartGroup = { id: string; label: string; letter: string; part: string };

/// What a cell's box says besides its id and title: for a step the engine decides, what the engine's values its rows
/// switch on read; and the moves of the second tier the cell does not make.
export type CellNotes = { reads: string | null; lacks: Move[] };

export type Chart = {
  nodes: ChartNode[];
  edges: ChartEdge[];
  groups: ChartGroup[];
  parts: string[];
  moves: Move[];
  /// The actions every agent's cell does every time it returns what sets them off, which the legend names.
  shared: SharedAction[];
  notes: Map<string, CellNotes>;
};

export const MAIN = 'main';
export const ASKING = 'asking';

const SELF = '@self';
const CALLER = '@caller';
const ASKER = '@asker';
const STOPPED = '@stopped';

/// The id of a cell's node.
export const cellNode = (id: string) => `c:${id}`;

/// Whether a row is of the engine's fallback: what it does with a round that ends in no return the cell takes.
export function fallback(r: CompiledRow): boolean {
  return r.fallback === true;
}

/// Whether a row is one the engine takes itself for the cell's line, as when its session would start afresh.
export function engineRow(r: CompiledRow): boolean {
  return r.if.some((a) => a[0] === 'engine');
}

/// Whether a row is on the main path: only a cell's own answer is, never a move or the engine's fallback, whatever the
/// row says.
export function mainPath(r: CompiledRow): boolean {
  return r.main && !r.common;
}

/// Whether a row is a move of an agent's cell of its own, which every cell that has it shares: one its agent chooses,
/// or one the engine takes for its line.
export function moveRow(r: CompiledRow): boolean {
  return r.common && !fallback(r);
}

/// The field of the return a shared row turns on: the first part of the place its first condition names, as `ask` of
/// `ask.to`; none for a row that names no place.
export function rowField(r: CompiledRow): string | null {
  const a = r.if[0];
  if (a === undefined) return null;
  if (a[0] === 'has') return a[1].split('.')[0];
  if (a[0] === 'is' || a[0] === 'any' || a[0] === 'none') return a[1].split('.')[0];
  return null;
}

/// How the chart draws a move: in the legend alone, every cell that could make it making it; in the legend with a mark
/// on each cell that does not, more than half making it; or as a line from each cell that makes it.
export type Tier = 'legend' | 'marked' | 'lines';

/// One of the moves an agent's cell makes of its own, as the rows every cell that has it shares: its name and where it
/// goes, the cells that have it and those that could. A cell could have a move when the form of the return it turns
/// on is for every agent's cell or for the cell's line, outside the asking part; no cell makes a move to itself. A move
/// the engine takes for a line could be the move of every cell of that line.
export type Move = {
  row: string;
  to: string;
  /// The engine takes it for the cell's line; the agent chooses every other.
  engine: boolean;
  /// The lines it is for, when only some.
  lines: string[] | null;
  cells: string[];
  could: string[];
  tier: Tier;
};

export function moves(g: CompiledGraph): Move[] {
  const agents = g.cells.filter((c) => c.kind === 'agent');
  const asking = new Set(g.asking);
  const formLines = (field: string): string[] | null => {
    for (const c of agents) {
      for (const f of c.forms ?? []) if (f.field === field) return f.lines ?? null;
    }
    return null;
  };
  const found = new Map<string, { to: string; engine: boolean; field: string | null; cells: string[] }>();
  for (const c of agents) {
    for (const r of c.rows) {
      if (!moveRow(r)) continue;
      const m = found.get(r.name) ?? { to: r.to, engine: engineRow(r), field: rowField(r), cells: [] };
      m.cells.push(c.id);
      found.set(r.name, m);
    }
  }
  return [...found].map(([row, m]) => {
    const has = new Set(m.cells);
    const lines: string[] | null = m.engine
      ? [...new Set(agents.filter((c) => has.has(c.id)).flatMap((c) => (c.line != null ? [c.line] : [])))]
      : m.field !== null
        ? formLines(m.field)
        : null;
    const could = agents
      .filter((c) => {
        if (c.id === m.to) return false;
        if (lines === null) return true;
        return c.line != null && lines.includes(c.line) && (m.engine || !asking.has(c.id));
      })
      .map((c) => c.id);
    for (const id of m.cells) if (!could.includes(id)) could.push(id);
    const tier: Tier = m.cells.length === could.length ? 'legend' : m.cells.length * 2 > could.length ? 'marked' : 'lines';
    return { row, to: m.to, engine: m.engine, lines, cells: m.cells, could, tier };
  });
}

/// An action every agent's cell that could does every time it returns the place that sets it off: every agent's cell,
/// or, for a place in one of the return's forms, every cell that has the form.
export type SharedAction = { action: string; at: string };

/// The actions each agent's cell does every time that every cell that could does too, which the legend names; and for
/// each cell, by its id, the actions it does every time of its own, those every work's end does included.
export function cellActions(g: CompiledGraph): { shared: SharedAction[]; own: Map<string, string[]> } {
  const agents = g.cells.filter((c) => c.kind === 'agent');
  const withForm = (field: string) => agents.filter((c) => (c.forms ?? []).some((f) => f.field === field));
  const doing = new Map<string, { action: string; at: string; cells: string[] }>();
  for (const c of agents) {
    for (const a of c.always ?? []) {
      if (a.do == null) continue;
      const key = `${a.do}\u0000${a.at}`;
      const d = doing.get(key) ?? { action: a.do, at: a.at, cells: [] };
      d.cells.push(c.id);
      doing.set(key, d);
    }
  }
  const shared: SharedAction[] = [];
  const sharedKeys = new Set<string>();
  for (const [key, d] of doing) {
    const head = d.at.split(/[.[]/)[0];
    const form = withForm(head);
    const could = form.length > 0 ? form : agents;
    if (d.cells.length > 1 && d.cells.length === could.length) {
      shared.push({ action: d.action, at: d.at });
      sharedKeys.add(key);
    }
  }
  const own = new Map<string, string[]>();
  for (const c of g.cells) {
    const list = [...(c.actions ?? [])];
    for (const a of c.always ?? []) {
      if (a.do == null || (c.kind === 'agent' && sharedKeys.has(`${a.do}\u0000${a.at}`))) continue;
      if (!list.includes(a.do)) list.push(a.do);
    }
    if (list.length > 0) own.set(c.id, list);
  }
  return { shared, own };
}

/// The names of the actions on a row, in order.
export function rowActions(r: CompiledRow): string[] {
  return (r.do ?? []).map((d) => (typeof d === 'string' ? d : d.do));
}

/// An action's name as words: `post_design_card` reads "post design card".
export function actionWords(name: string): string {
  return name.replaceAll('_', ' ');
}

/// The engine's values a cell's own rows switch on, in the order they first come.
export function valuesOf(c: CompiledCell, known: Set<string>): string[] {
  const out: string[] = [];
  const walk = (a: Atom) => {
    if (a[0] === 'or') a[1].forEach(walk);
    else if (a[0] === 'inp' && known.has(a[1]) && !out.includes(a[1])) out.push(a[1]);
  };
  for (const r of c.rows) if (!r.common && !fallback(r)) r.if.forEach(walk);
  return out;
}

/// The part of the flow a store is kept for, as its box among the stores says it: the project, an item, or a work, its
/// rounds' included.
export function scopeOf(layer: Layer): 'project' | 'item' | 'work' {
  return layer === 'round' ? 'work' : layer;
}

/// The stores a cell reads: what its inputs read, what the values its rows switch on read, and what its step reads.
export function readsOf(engine: EngineNames, cell: CompiledCell): string[] {
  const stores = engine.stores ?? {};
  const out: string[] = [];
  const add = (s: string) => {
    if (s in stores && !out.includes(s)) out.push(s);
  };
  for (const x of cell.inputs) {
    for (const src of x.from) {
      if (typeof src !== 'object' || Array.isArray(src)) continue;
      for (const [name, s] of Object.entries(stores)) {
        if ('db' in src && (s.relations ?? []).includes(src.db)) add(name);
        if ('engine' in src && (s.inputs ?? []).includes(src.engine)) add(name);
      }
    }
  }
  for (const v of valuesOf(cell, new Set(Object.keys(engine.values)))) (engine.reads?.[v]?.stores ?? []).forEach(add);
  if (cell.step != null) (engine.reads?.[cell.step]?.stores ?? []).forEach(add);
  return out;
}

/// The stores the actions or steps named write, as the engine's table says.
export function writesOf(engine: EngineNames, names: string[]): string[] {
  const out: string[] = [];
  for (const n of names) for (const s of engine.writes?.[n] ?? []) if (!out.includes(s)) out.push(s);
  return out;
}

/// The stores a cell reads or writes, each with how, in the order the engine lists its stores: what it reads
/// (`readsOf`), and what its step and `actions` write, those it does every time and those on the rows the chart draws
/// out of it.
export function cellStores(engine: EngineNames, cell: CompiledCell, actions: string[]): Array<[string, StoreUse]> {
  const reads = new Set(readsOf(engine, cell));
  const writes = new Set(writesOf(engine, [...(cell.step != null ? [cell.step] : []), ...actions]));
  return Object.keys(engine.stores ?? {}).flatMap((s): Array<[string, StoreUse]> => {
    if (reads.has(s) && writes.has(s)) return [[s, 'both']];
    if (reads.has(s)) return [[s, 'read']];
    if (writes.has(s)) return [[s, 'write']];
    return [];
  });
}

/// The part each cell is drawn in. The asking part is a part of its own; a cell outside it that no row the chart draws
/// leads to, but the moves every agent's cell shares and the rows back to where the work came from (such as the cell
/// that runs the tools agents ask for), is drawn apart; every other cell is in the main part. `drawn` names the moves
/// drawn as lines.
export function partsOf(g: CompiledGraph, v: GraphView, drawn: Set<string> = new Set()): Map<string, string> {
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
      if (row === undefined || fallback(row) || row.to.startsWith('@')) return false;
      return !row.common || drawn.has(row.name);
    });
    out.set(cv.id, cv.id === g.start || wayIn ? MAIN : cv.id);
  }
  return out;
}

/// The letter a cell's code starts with, its stage's, as `T` of `T6`; none for an id that is not such a code.
export function letterOf(id: string): string {
  const m = /^([A-Z]+)\d+$/.exec(id);
  return m ? m[1] : '';
}

/// The chart of a graph; `engine`, the flow's engine part, says what the values a step decides by read.
export function chart(g: CompiledGraph, v: GraphView, engine: EngineNames | null = null): Chart {
  const ms = moves(g);
  const drawn = new Set(ms.filter((m) => m.tier === 'lines').map((m) => m.row));
  const parts = partsOf(g, v, drawn);
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
  const node = (
    n: Omit<ChartNode, 'part' | 'group' | 'actions' | 'use'> & { beside?: string; actions?: string[]; use?: StoreUse },
  ) => {
    if (nodes.has(n.id)) return;
    const at = n.beside ?? n.cell;
    nodes.set(n.id, {
      id: n.id,
      kind: n.kind,
      cell: n.cell,
      other: n.other,
      actions: n.actions ?? [],
      use: n.use ?? null,
      part: parts.get(at) ?? MAIN,
      group: groupOf(at),
    });
  };
  const edges = new Map<string, ChartEdge>();
  const plain = (id: string, source: string, target: string, kind: ChartEdge['kind']) =>
    edges.set(id, { id, source, target, cell: null, rows: [], through: [], label: '', main: false, kind });
  // Rows of one cell to the same place are one arrow.
  const arrow = (source: string, target: string, c: CompiledCell, row: number) => {
    const id = `${source}->${target}`;
    const e = edges.get(id);
    if (e) {
      if (!e.rows.includes(row)) e.rows.push(row);
      e.main = e.main || mainPath(c.rows[row]);
    } else {
      edges.set(id, {
        id,
        source,
        target,
        cell: c.id,
        rows: [row],
        through: [],
        label: '',
        main: mainPath(c.rows[row]),
        kind: 'row',
      });
    }
  };
  // The arrow out of the actions on a line, which draws every row through them.
  const onward = (source: string, target: string, c: CompiledCell, row: number) => {
    const id = `${source}->${target}`;
    const e = edges.get(id);
    const one = { cell: c.id, row };
    if (e) {
      if (!e.through.some((x) => x.cell === c.id && x.row === row)) e.through.push(one);
      e.main = e.main || mainPath(c.rows[row]);
    } else {
      const main = mainPath(c.rows[row]);
      edges.set(id, { id, source, target, cell: null, rows: [], through: [one], label: '', main, kind: 'row' });
    }
  };
  for (const c of g.cells) node({ id: cellNode(c.id), kind: 'cell', cell: c.id, other: null });
  // A row to a cell: an arrow within a part; between parts, a circle beside the cell it leaves, and a shaded one
  // beside the cell it enters when it comes back into the main part. A row the engine acts on goes through the node of
  // its actions: one node for the rows with the same actions into the same cell, beside that cell and in its box, on
  // the way in, so that it is not read as part of another line; to another part, beside the circle it leaves by.
  const toCell = (c: CompiledCell, row: number, t: string) => {
    if (!cells.has(t)) return;
    const same = parts.get(t) === parts.get(c.id);
    const acts = rowActions(c.rows[row]);
    let from = cellNode(c.id);
    if (acts.length > 0) {
      const id = same ? `a:${t}:${acts.join('+')}` : `a:${c.id}>${t}:${acts.join('+')}`;
      node({ id, kind: 'action', cell: t, other: null, beside: same ? t : c.id, actions: acts });
      arrow(cellNode(c.id), id, c, row);
      from = id;
    }
    const step = (source: string, target: string) =>
      source === cellNode(c.id) ? arrow(source, target, c, row) : onward(source, target, c, row);
    if (same) {
      step(from, cellNode(t));
      return;
    }
    const out = `o:${t}:${c.id}`;
    node({ id: out, kind: 'out', cell: c.id, other: t });
    step(from, out);
    if (parts.get(c.id) !== MAIN && parts.get(t) === MAIN) {
      const into = `i:${c.id}:${t}`;
      node({ id: into, kind: 'in', cell: t, other: c.id });
      arrow(into, cellNode(t), c, row);
    }
  };
  // A row back to whichever cell asked is drawn into each cell whose own answer asked. A cell that asked by a move the
  // legend names comes back as from any move, which is drawn for no cell, as a row back to the cell that called is not.
  const askedBy = (t: string, asked: string): boolean =>
    (cells.get(t)?.rows ?? []).some(
      (r, i) => !r.common && !fallback(r) && (views.get(t)?.targets[i] ?? [r.to]).includes(asked),
    );
  // What the engine does on a row back to the cell that called, which is not drawn, is drawn beside the cell.
  const back = new Map<string, string[]>();
  const besideCell = (c: CompiledCell, r: CompiledRow) => {
    const acts = back.get(c.id) ?? [];
    for (const a of rowActions(r)) if (!acts.includes(a)) acts.push(a);
    if (acts.length > 0) back.set(c.id, acts);
  };
  for (const c of g.cells) {
    const cv = views.get(c.id);
    c.rows.forEach((r, i) => {
      if (fallback(r) || (r.common && !drawn.has(r.name))) return;
      if (r.to === SELF || r.to === CALLER) {
        besideCell(c, r);
        return;
      }
      if (r.to === STOPPED) return;
      const targets = r.to === ASKER ? (cv?.targets[i] ?? []).filter((t) => askedBy(t, c.id)) : [r.to];
      if (targets.length === 0) besideCell(c, r);
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
          node({ id: into, kind: 'in', cell: t, other: m.id });
          arrow(into, cellNode(t), m, from.row);
        }
      }
    });
  }
  // What a cell does every time beside it; the rules beside the cells that name them (those every cell of a line is
  // given are listed in each cell's inputs, not drawn); and the interjection beside the engine's steps that take one.
  const acting = cellActions(g);
  for (const c of g.cells) {
    const own = [...(acting.own.get(c.id) ?? []), ...(back.get(c.id) ?? [])];
    if (own.length > 0) {
      const id = `d:${c.id}`;
      node({ id, kind: 'does', cell: c.id, other: null, actions: [...new Set(own)] });
      plain(`${cellNode(c.id)}->${id}`, cellNode(c.id), id, 'does');
    }
    for (const r of (c.rules ?? []).filter((x) => !x.shared)) {
      const id = `r:${r.name}:${c.id}`;
      node({ id, kind: 'rule', cell: c.id, other: r.name });
      plain(`${id}->${cellNode(c.id)}`, id, cellNode(c.id), 'rule');
    }
    if (c.kind !== 'agent' && g.interject && c.rows.some((r) => r.interject)) {
      const id = `j:${c.id}`;
      node({ id, kind: 'interjection', cell: c.id, other: null });
      plain(`${id}->${cellNode(c.id)}`, id, cellNode(c.id), 'interjection');
    }
    // Each store the cell reads or writes, beside it: a read comes into the cell, a write goes out to the store.
    if (engine !== null) {
      const drawnRows = c.rows.filter((r) => !fallback(r) && (!r.common || drawn.has(r.name)));
      for (const [store, use] of cellStores(engine, c, [...own, ...drawnRows.flatMap(rowActions)])) {
        const id = `s:${store}:${c.id}`;
        node({ id, kind: 'store', cell: c.id, other: store, use });
        if (use === 'read') plain(`${id}->${cellNode(c.id)}`, id, cellNode(c.id), 'store');
        else plain(`${cellNode(c.id)}->${id}`, cellNode(c.id), id, 'store');
      }
    }
  }
  for (const e of edges.values()) {
    const c = e.cell !== null ? cells.get(e.cell) : undefined;
    if (!c) continue;
    e.rows.sort((a, b) => a - b);
    e.label = [...new Set(e.rows.map((i) => c.rows[i].name))].join(', ');
  }
  const groups: ChartGroup[] = [];
  const letters = new Map<string, Set<string>>();
  for (const c of g.cells) {
    const id = groupOf(c.id);
    if (id === null) continue;
    if (!letters.has(id)) {
      letters.set(id, new Set());
      groups.push({ id, label: id === ASKING ? ASKING : id.slice('stage:'.length), letter: '', part: parts.get(c.id) ?? MAIN });
    }
    letters.get(id)?.add(letterOf(c.id));
  }
  for (const gr of groups) {
    const all = [...(letters.get(gr.id) ?? [])];
    gr.letter = all.length === 1 ? all[0] : '';
  }
  const notes = new Map<string, CellNotes>();
  const known = new Set(Object.keys(engine?.values ?? {}));
  for (const c of g.cells) {
    const says =
      c.kind === 'agent'
        ? []
        : valuesOf(c, known).flatMap((x) => {
            const s = engine?.reads?.[x]?.says;
            return s != null && s !== '' ? [s] : [];
          });
    const lacks = ms.filter((m) => m.tier === 'marked' && !m.engine && m.could.includes(c.id) && !m.cells.includes(c.id));
    if (says.length > 0 || lacks.length > 0) notes.set(c.id, { reads: says.length > 0 ? says.join('; ') : null, lacks });
  }
  const partList = [MAIN, ...new Set([...parts.values()].filter((p) => p !== MAIN))];
  return {
    nodes: wayIn([...nodes.values()], parts),
    edges: [...edges.values()],
    groups,
    parts: partList,
    moves: ms,
    shared: acting.shared,
    notes,
  };
}

/// The nodes in the order the chart is laid out by, each node of the actions on lines into a cell of its own part
/// just before that cell, so that the layout, which keeps that order from top to bottom, puts it on the way in.
function wayIn(all: ChartNode[], parts: Map<string, string>): ChartNode[] {
  const into = (n: ChartNode) => n.kind === 'action' && n.part === (parts.get(n.cell) ?? MAIN);
  const out: ChartNode[] = [];
  for (const n of all) {
    if (into(n)) continue;
    if (n.kind === 'cell') out.push(...all.filter((a) => into(a) && a.cell === n.cell));
    out.push(n);
  }
  return out;
}

/// The arrows that light up with the arrow under the pointer: itself; on the way on, the arrow out of the actions it
/// goes into and the half of a row to another part beside the cell it enters; on the way back, the arrows into the
/// actions it comes out of and the half beside the cell it leaves. Nothing else, so that a row drawn into several
/// cells, such as one going back to whichever cell asked, lights only where it is pointed at. None for none.
export function related(edges: ChartEdge[], id: string | null): Set<string> {
  const e = id === null ? undefined : edges.find((x) => x.id === id);
  if (e === undefined) return new Set();
  const lit = new Set([e.id]);
  if (e.kind !== 'row') return lit;
  const forward = (x: ChartEdge): ChartEdge[] => {
    if (x.target.startsWith('a:')) return edges.filter((y) => y.source === x.target);
    if (x.target.startsWith('o:')) {
      const [, t, from] = x.target.split(':');
      return edges.filter((y) => y.id === `i:${from}:${t}->${cellNode(t)}`);
    }
    return [];
  };
  const backward = (x: ChartEdge): ChartEdge[] => {
    if (x.source.startsWith('a:')) return edges.filter((y) => y.target === x.source);
    if (x.source.startsWith('i:')) {
      const [, from, t] = x.source.split(':');
      return edges.filter((y) => y.target === `o:${t}:${from}`);
    }
    return [];
  };
  const follow = (step: (x: ChartEdge) => ChartEdge[]) => {
    let at = [e];
    while (at.length > 0) {
      const next = at.flatMap(step).filter((y) => !lit.has(y.id));
      for (const y of next) lit.add(y.id);
      at = next;
    }
  };
  follow(forward);
  follow(backward);
  return lit;
}

/// The arrows into a node and out of it, each set by its arrows' ids.
export type Ways = { in: Set<string>; out: Set<string> };

/// The rows into a cell's node and out of it, each whole as `related` follows it from the arrow at the cell: through
/// the actions on a line, into the cell through them from every cell whose rows go there, and through the circles to
/// and from another part. The arrows of a rule, of the interjection and to what the cell does every time are no row,
/// and in neither. None for none.
export function ways(edges: ChartEdge[], node: string | null): Ways {
  const into = new Set<string>();
  const out = new Set<string>();
  if (node === null) return { in: into, out };
  for (const e of edges) {
    if (e.kind !== 'row') continue;
    if (e.source === node) for (const id of related(edges, e.id)) out.add(id);
    if (e.target === node) for (const id of related(edges, e.id)) into.add(id);
  }
  return { in: into, out };
}

/// Why a work waits at the cell it is at while no round of it runs there: in a few words for the tag beside the cell,
/// and in full on hovering.
export type Wait = { tag: string; why: string };

/// What a work did at a cell, and at the cell it is at, whether a round runs there now and, while none does, why the
/// work waits there.
export type Overlay = {
  rounds: number;
  sentBack: number;
  failed: number;
  running: boolean;
  current: boolean;
  waits: Wait | null;
};

/// How the work goes on now at the cell it is at: whether the engine runs a round of it, and why it waits, if it does.
export type Now = { running: boolean; waits: Wait | null };

/// What the work's rounds did at each cell; `at` is the cell the work is at, where it goes on as `now` says. A round
/// still open is running only while the engine runs it, or a tool of it is under way.
export function overlays(rounds: RoundSummary[], at: string | null, now: Now): Map<string, Overlay> {
  const out = new Map<string, Overlay>();
  const get = (cell: string) => {
    let o = out.get(cell);
    if (!o) {
      o = { rounds: 0, sentBack: 0, failed: 0, running: false, current: false, waits: null };
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
  }
  if (at !== null) {
    const o = get(at);
    o.current = true;
    o.running = now.running;
    o.waits = now.running ? null : now.waits;
  }
  return out;
}

/// A round the halo goes round on a work's chart: its number, its cell, and when the page first saw it, in
/// milliseconds.
export type Flash = { n: number; cell: string; seen: number };

/// How long the halo stays on the cell of a round the page saw come in, counted from when it saw it; and the least it
/// stays on each of several that came in together.
export const HALO_HOLD = 1500;
export const HALO_STEP = 500;

/// Where the halo goes round at `now`, and when that next changes (none when it does not). The rounds the page saw come
/// in (`flashes`, oldest first) each take it in order, for at least HALO_STEP and the last until HALO_HOLD after it was
/// seen, so that a round over before the page could show it running is seen; the newest round while a round of the
/// work runs (`runs`) keeps it as long as it runs; after the last round that ended, it lands on the cell the work is
/// at, `at`, for the rest of the hold. Otherwise it is on `at` while a round runs there, and nowhere while none does.
export function haloAt(
  flashes: Flash[],
  now: number,
  where: { at: string | null; runs: boolean; newest: number | null },
): { cell: string | null; next: number | null } {
  const { at, runs, newest } = where;
  const steps: Array<{ cell: string; from: number; to: number }> = [];
  let prev = -Infinity;
  flashes.forEach((f, i) => {
    const from = Math.max(f.seen, prev + HALO_STEP);
    prev = from;
    const running = runs && f.n === newest;
    const lands = i === flashes.length - 1 && !running && at !== null && at !== f.cell;
    const to = running ? Infinity : lands ? from + HALO_STEP : Math.max(from + HALO_STEP, f.seen + HALO_HOLD);
    steps.push({ cell: f.cell, from, to });
    if (lands && at !== null) steps.push({ cell: at, from: to, to: Math.max(to + HALO_STEP, f.seen + HALO_HOLD) });
  });
  // Each round gives the halo up when the next takes it.
  for (let i = 0; i + 1 < steps.length; i++) steps[i].to = Math.min(steps[i].to, steps[i + 1].from);
  const on = steps.find((s) => s.from <= now && now < s.to);
  const next = Math.min(...steps.flatMap((s) => [s.from, s.to]).filter((x) => x > now));
  return { cell: on !== undefined ? on.cell : runs ? at : null, next: Number.isFinite(next) ? next : null };
}

/// The arrows of the chart that draw the row each round took, by the round's place among the work's rounds: every
/// arrow that draws that row of its cell, an arrow to one of several cells the row can lead to only when the work
/// went there.
function rowArrows(c: Chart, g: CompiledGraph, rounds: RoundSummary[]): (at: number) => string[] {
  const rowOf = new Map<string, number>();
  for (const cell of g.cells) cell.rows.forEach((r, i) => rowOf.set(`${cell.id}\u0000${r.name}`, i));
  const byRow = new Map<string, ChartEdge[]>();
  const index = (cell: string, row: number, e: ChartEdge) => {
    const k = `${cell}\u0000${row}`;
    byRow.set(k, [...(byRow.get(k) ?? []), e]);
  };
  for (const e of c.edges) {
    if (e.cell !== null) for (const i of e.rows) index(e.cell, i, e);
    for (const x of e.through) index(x.cell, x.row, e);
  }
  const nodes = new Map(c.nodes.map((n) => [n.id, n]));
  return (at) => {
    const r = rounds[at];
    if (r === undefined || r.cell === null || r.transition === null) return [];
    const i = rowOf.get(`${r.cell}\u0000${r.transition}`);
    if (i === undefined) return [];
    // The cells the work went to next: the next one, or the one after it when the next goes straight back.
    const next = rounds
      .slice(at + 1)
      .filter((x) => x.cell !== null)
      .slice(0, 2)
      .map((x) => x.cell);
    return (byRow.get(`${r.cell}\u0000${i}`) ?? [])
      .filter((e) => {
        const into = nodes.get(e.target);
        const lands =
          into?.kind === 'cell' || into?.kind === 'action' ? into.cell : into?.kind === 'out' ? into.other : null;
        return lands === null || next.length === 0 || next.includes(lands);
      })
      .map((e) => e.id);
  };
}

/// How many times the work took each arrow of the chart: a round that took a row counts on every arrow that draws
/// that row of its cell.
export function taken(c: Chart, g: CompiledGraph, rounds: RoundSummary[]): Map<string, number> {
  const arrows = rowArrows(c, g, rounds);
  const out = new Map<string, number>();
  rounds.forEach((_, at) => {
    for (const id of arrows(at)) out.set(id, (out.get(id) ?? 0) + 1);
  });
  return out;
}

/// The arrows a round came in by and left by: in, those of the row the round before it took; out, those of the row it
/// took itself, none for a round that has taken no row yet.
export function roundArrows(c: Chart, g: CompiledGraph, rounds: RoundSummary[], n: number): Ways {
  const at = rounds.findIndex((r) => r.n === n);
  if (at < 0) return { in: new Set(), out: new Set() };
  const arrows = rowArrows(c, g, rounds);
  return { in: new Set(at > 0 ? arrows(at - 1) : []), out: new Set(arrows(at)) };
}
