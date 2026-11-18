// The flow as a flowchart, before it is laid out: how a work moves through the cells, drawn from the flow's definition
// alone, nothing placed by hand and nothing said of one cell that is not said of every cell. A node for each cell, in
// the order of the walk; the main path, the walk as a work goes through the cells, from each cell to the next one that
// can run after it, parting where the next cell's `when` may or may not hold at a fork, one node for the decision with
// a branch for each value, and joining again (`mainPath`); a cell's `requires` as a gate before it, which the path
// waits at; an arrow from a cell that writes a block to each cell that watches it, into the cell itself, one arrow for
// each pair of cells with the name of every datum or group it runs the cell by, an arrow back up the walk being a way
// back, one way back from a cell to every cell a fork's branches lead to drawn into the fork, and an arrow that runs
// along the main path between the same two cells folded into it; and what runs a cell from outside the cells (people,
// or a source of the engine's such as main, the tree or the session) as a tag beside it with a dashed arrow into it,
// of what the cell's own file watches: what `[every_agent]` watches comes into no cell from outside, the view's line
// naming it. What a cell's own file gives it is not drawn until the cell is chosen (`contextOf`). Pure, so that it is
// tested without drawing.

import type { Block, Cell, Condition, Flow, InputKind } from '../api';
import type { MarkKey } from '../marks';
import { canRun, matrix, namesItself } from '../matrix';

/// The marks the flowchart draws, in the order its legend lists them (`Legend` in marks.tsx): its kinds of line, the
/// dashed arrow from a tag, the way into a cell, and the matrix's own marks and signs where the flowchart uses them;
/// the way back with how many the flow has.
export const FLOWCHART_MARKS: MarkKey[] = [
  'mainPath',
  'wakes',
  'wayBack',
  'fromOutside',
  'when',
  'requires',
  'watches',
  'reads',
  'latest',
  'lastAndLatest',
  'all',
  'writer',
  'people',
  'outside',
];

/// The ids of the nodes: a cell's, the gate before it, the fork before it, and a tag of what the outside writes for
/// it.
export const cellNode = (cell: string) => `c:${cell}`;
export const wayInNode = (cell: string) => `w:${cell}`;
export const forkNode = (cell: string) => `f:${cell}`;
export const outsideNode = (cell: string, name: string) => `o:${cell}:${name}`;
/// A tag of a block of a condition that no cell writes, before the fork or the gate (`node`) it belongs to.
export const conditionTagNode = (node: string, block: string) => `o:${node}:${block}`;

/// `way-in`, the gate of a cell's `requires`; `fork`, where the main path parts on a cell's `when`.
export type NodeKind = 'cell' | 'way-in' | 'fork' | 'outside';

/// Who writes a block besides the cells, as its file says: people, and the source outside the work.
export interface Outside {
  people: boolean;
  /// The sources outside the work, by name.
  sources: string[];
}

export interface ChartNode {
  id: string;
  kind: NodeKind;
  /// The cell it is, the cell the gate leads into, the first cell the fork leads into, or the cell the tag stands
  /// beside or before.
  cell: string;
  /// Of a fork: the keys it decides on, those of the cell's `when` that may or may not hold there.
  when: Condition[];
  /// Of a gate: the cell's `requires`, every key in the order written.
  requires: Condition[];
  /// Of a fork on one block: the block. Of a tag: the datum or group the cell watches, or the block of a condition,
  /// and who writes it.
  name: string | null;
  group: boolean;
  outside: Outside | null;
}

/// A name an arrow carries: a datum the cell it runs watches, or a group.
export interface Carried {
  name: string;
  group: boolean;
}

export type EdgeKind = 'arrow' | 'step' | 'outside' | 'in' | 'tie';

export interface ChartEdge {
  id: string;
  source: string;
  target: string;
  /// `arrow`, from a cell that writes a block to a cell that watches it, or a way back into a fork; `step`, a step of
  /// the main path, from a cell or a fork to the next fork, gate or cell the path goes to; `outside`, from a tag of
  /// what the outside writes; `in`, from a gate to its cell, or from the fork of a cell the main path does not reach;
  /// `tie`, drawn by nothing, which keeps a cell the main path does not reach under the cell before it in the walk.
  kind: EdgeKind;
  /// Of an arrow, the cell that writes and the cell it runs (of a way back into a fork, the fork's first cell); of a
  /// step, the cell it leaves, or whose fork it leaves, and the cell it goes to.
  from: string | null;
  to: string;
  /// Of a way back into a fork: every cell it runs, those the fork's branches lead to.
  reaches: string[];
  /// Of an arrow, the data and groups it runs the cell by, in the order the cell watches them; of a step, those of the
  /// arrows folded into it.
  carries: Carried[];
  /// Of an arrow, it goes back up the walk: a way back.
  back: boolean;
  /// It is on the main path: a step, or the way through a gate the main path goes through into its cell.
  main: boolean;
  /// Of a step that leaves a fork: the branch, the value of the fork's block it is taken on, or ✓ and ✗ for a fork on
  /// several keys or a named test; said before what is folded into it.
  branch: string | null;
  /// Of a step, the arrows folded into it: each runs along the main path from the cell that writes to the cell it runs,
  /// with no cell between, and is drawn as this step, its names on it.
  folds: ChartEdge[];
  /// What the arrow's label says: the names it carries.
  label: string;
}

export interface Chart {
  /// For each cell in the order of the walk: the tags of its fork and gate, its fork, its gate, the cell, its tags.
  nodes: ChartNode[];
  edges: ChartEdge[];
  /// How many ways back the flow has, as the matrix counts them (`Matrix.waysBack`), so that the two views give one
  /// count: an arrow stands for one for each name it carries, and one drawn into a fork for those of every cell it
  /// leads to.
  waysBack: number;
}

/// A step of the main path: from a cell or a fork to the fork, the gate or the cell the path goes to next.
export interface Step {
  source: string;
  target: string;
  /// The cell it leaves, or whose fork it leaves, and the cell it goes to, or whose fork it goes to.
  from: string;
  to: string;
  /// Of a step out of a fork: the branch it is.
  branch: string | null;
}

/// A fork of the main path: where it parts on the keys of a cell's `when` that may or may not hold.
export interface Fork {
  id: string;
  /// The cell whose `when` it decides first.
  cell: string;
  keys: Condition[];
  /// The block it decides on, where it is one key on a block.
  block: string | null;
}

/// The main path of a flow: its steps, its forks, and the cells with a gate that it goes through into the cell.
export interface MainPath {
  steps: Step[];
  forks: Fork[];
  through: Set<string>;
}

/// Who writes a block besides the cells; none where only cells do.
function outsideOf(blocks: Block[]): Outside | null {
  const people = blocks.some((b) => b.writtenBy.people);
  const sources = [...new Set(blocks.flatMap((b) => (b.writtenBy.outside !== null ? [b.writtenBy.outside] : [])))];
  return people || sources.length > 0 ? { people, sources } : null;
}

/// What an arrow from `writer` to `reader` carries: each datum or group the reader watches, a block of which the writer
/// writes, by its name, once.
export function carried(flow: Flow, writer: Cell, reader: Cell): Carried[] {
  const blocks = new Map(flow.blocks.map((b) => [b.name, b]));
  const out: Carried[] = [];
  for (const w of reader.watches) {
    const writes = w.blocks.some((b) => blocks.get(b)?.writtenBy.cells.includes(writer.id));
    if (writes && !out.some((c) => c.name === w.name)) out.push({ name: w.name, group: !namesItself(w) });
  }
  return out;
}

/// Where the main path and an arrow into a cell end: the gate before it, where it has one.
const entryOf = (c: Cell) => (c.requires.length > 0 ? wayInNode(c.id) : cellNode(c.id));

/// What a branch of the main path knows of the value of each choice: the values it may have there, a `null` among
/// them for none; the whole of it `null` where it may have anything, as a block that is no choice.
type Known = Map<string, Set<string | null> | null>;

/// Whether a key of a condition holds on a branch: surely, surely not, or either.
type Holds = 'yes' | 'no' | 'either';

/// The words of a branch out of a fork on several keys or a named test.
export const HOLDS = '✓';
export const FAILS = '✗';

/// The main path of a flow: the walk as a work goes through the cells, worked out from the definition alone (the order
/// of the walk, each cell's `when`, `requires` and what it writes, the blocks' values and defaults, and `canRun`, the
/// one rule of whether a cell's write can run another). The walk starts again from the first cell every time, so a
/// block may hold any value a cell writes anywhere in the flow, or people or the outside, or else its default, or none;
/// where the path goes through a cell that writes it, any of its values again. From the first cell, each step goes to
/// the next cell in the walk that can run after the cells the path went through: one whose `when` may hold there and
/// whose conditions exclude none of theirs. Where that cell's `when` surely holds, the path goes into it; where it may
/// or may not, the path parts at a fork: one branch into the cell, labelled with the value it is taken on, and one past
/// it to the cell after, labelled with the values left, each knowing which; and where that branch comes to a cell whose
/// `when` is on the same block and may hold, that is a branch of the same fork. The branches join again where they
/// reach the same node. A `requires` is a gate the path waits at, not a fork: the path goes through it into its cell.
export function mainPath(flow: Flow): MainPath {
  const blocks = new Map(flow.blocks.map((b) => [b.name, b]));
  const cells = flow.cells;
  // What a block may hold as the walk starts.
  const before = (name: string): Set<string | null> | null => {
    const b = blocks.get(name);
    if (b === undefined || b.values === null) return null;
    const first = new Set<string | null>([b.default ?? null]);
    const written = b.writtenBy.cells.length > 0 || b.writtenBy.people || b.writtenBy.outside !== null;
    if (written) for (const v of b.values) first.add(v);
    return first;
  };
  const may = (known: Known, name: string) => (known.has(name) ? (known.get(name) ?? null) : before(name));
  const is = (known: Known, name: string, value: string): Holds => {
    const vs = may(known, name);
    if (vs === null) return 'either';
    if (!vs.has(value)) return 'no';
    return vs.size === 1 ? 'yes' : 'either';
  };
  const holds = (known: Known, key: Condition): Holds => {
    if (key.on === 'block') return is(known, key.block, key.value);
    const { kind, reads, is: value } = key.test;
    // A test of content looks for no value: what a block holds, the definition does not say.
    if (value === null) return 'either';
    const each = reads.map((b) => is(known, b, value));
    const all = (h: Holds) => each.every((x) => x === h);
    if (kind === 'all') return each.includes('no') ? 'no' : all('yes') ? 'yes' : 'either';
    if (kind === 'any') return each.includes('yes') ? 'yes' : all('no') ? 'no' : 'either';
    return each.includes('yes') ? 'no' : all('no') ? 'yes' : 'either';
  };
  // A branch's knowledge once it is known that a key holds, or does not.
  const narrowed = (known: Known, name: string, keep: (v: string | null) => boolean): Known => {
    const vs = may(known, name);
    return vs === null ? known : new Map(known).set(name, new Set([...vs].filter(keep)));
  };
  const assume = (known: Known, key: Condition, held: boolean): Known => {
    if (key.on === 'block') return narrowed(known, key.block, (v) => (v === key.value) === held);
    const { kind, reads, is: value } = key.test;
    if (value === null) return known;
    if (held && kind === 'all') return reads.reduce((k, b) => narrowed(k, b, (v) => v === value), known);
    if (held !== (kind === 'any')) return reads.reduce((k, b) => narrowed(k, b, (v) => v !== value), known);
    return known;
  };
  // A branch's knowledge once a cell has run, which may write any value of each choice it writes.
  const written = (known: Known, cell: Cell): Known => {
    const out = new Map(known);
    for (const name of cell.outputs) {
      const vs = may(known, name);
      const values = blocks.get(name)?.values;
      out.set(name, vs === null || !values ? null : new Set([...vs, ...values]));
    }
    return out;
  };
  const said = (known: Known) =>
    [...known]
      .map(([b, vs]) => `${b}=${vs === null ? '*' : [...vs].map(String).sort().join('/')}`)
      .sort()
      .join(',');

  const steps = new Map<string, Step>();
  const forks = new Map<string, Fork>();
  const through = new Set<string>();
  const walked = new Set<string>();
  /// Where the walk stands: the node it leaves, the cell that node is of, and the fork it leaves by the branch past
  /// that fork's cell, where it does.
  type At = { node: string; cell: string; past: Fork | null };
  const step = (at: At | null, target: string, to: string, branch: string | null) => {
    if (at === null) return;
    const key = `${at.node}>${target}`;
    const was = steps.get(key);
    // Reached on two ways with other values left, a branch says them all.
    const words =
      was?.branch != null && branch !== null && was.branch !== branch
        ? [...new Set([...was.branch.split(', '), ...branch.split(', ')])].join(', ')
        : branch;
    steps.set(key, { source: at.node, target, from: at.cell, to, branch: words });
  };
  // What the branch past a fork is taken on: the values of its block left, or ✗; none where the block is left with no
  // value at all, before anyone writes it.
  const pastWords = (fork: Fork, known: Known): string | null => {
    if (fork.block === null) return FAILS;
    const left = [...(may(known, fork.block) ?? [])].filter((v): v is string => v !== null);
    return left.length > 0 ? left.join(', ') : null;
  };
  const go = (i: number, at: At | null, known: Known, ran: Cell[]) => {
    const key = `${i}|${at?.node ?? ''}|${said(known)}|${ran.filter((c) => c.when.length > 0).map((c) => c.id)}`;
    if (walked.has(key)) return;
    walked.add(key);
    for (let j = i; j < cells.length; j += 1) {
      const cell = cells[j];
      if (ran.some((a) => !canRun(a, cell))) continue;
      const each = cell.when.map((k) => holds(known, k));
      if (each.includes('no')) continue;
      const open = cell.when.filter((_, k) => each[k] === 'either');
      const branchIn = at?.past ? pastWords(at.past, known) : null;
      const onward = (inside: Known) =>
        go(j + 1, { node: cellNode(cell.id), cell: cell.id, past: null }, written(inside, cell), [...ran, cell]);
      if (cell.requires.length > 0) through.add(cell.id);
      if (open.length === 0) {
        step(at, entryOf(cell), cell.id, branchIn);
        onward(known);
        return;
      }
      const block = open.length === 1 && open[0].on === 'block' ? open[0].block : null;
      // A branch past a fork that comes to a cell whose `when` is on the fork's block is a branch of that fork.
      const same = at?.past != null && block !== null && at.past.block === block ? at.past : null;
      const fork = same ?? forks.get(forkNode(cell.id)) ?? { id: forkNode(cell.id), cell: cell.id, keys: open, block };
      forks.set(fork.id, fork);
      if (same === null) step(at, fork.id, cell.id, branchIn);
      const from: At = { node: fork.id, cell: fork.cell, past: null };
      const key0 = open[0];
      step(from, entryOf(cell), cell.id, block !== null && key0.on === 'block' ? key0.value : HOLDS);
      onward(open.reduce((k, key) => assume(k, key, true), known));
      const past = open.length === 1 ? assume(known, open[0], false) : known;
      go(j + 1, { node: fork.id, cell: fork.cell, past: fork }, past, ran);
      return;
    }
  };
  go(0, null, new Map(), []);
  return { steps: [...steps.values()], forks: [...forks.values()], through };
}

/// The flowchart of a flow.
export function chartOf(flow: Flow): Chart {
  const blocks = new Map(flow.blocks.map((b) => [b.name, b]));
  const place = new Map(flow.cells.map((c, i) => [c.id, i]));
  const path = mainPath(flow);
  const nodes: ChartNode[] = [];
  const edges: ChartEdge[] = [];
  const node = (n: Partial<ChartNode> & Pick<ChartNode, 'id' | 'kind' | 'cell'>): ChartNode => ({
    when: [],
    requires: [],
    name: null,
    group: false,
    outside: null,
    ...n,
  });
  const link = (e: Partial<ChartEdge> & Pick<ChartEdge, 'id' | 'source' | 'target' | 'kind' | 'to'>): ChartEdge => ({
    from: null,
    reaches: [],
    carries: [],
    back: false,
    main: false,
    branch: null,
    folds: [],
    label: '',
    ...e,
  });
  // The blocks of a condition that no cell writes, each a tag before the fork or the gate: what opens it from outside.
  const conditionTags = (cell: Cell, keys: Condition[], at: string) => {
    const read = keys.flatMap((k) => (k.on === 'block' ? [k.block] : k.test.reads));
    for (const name of [...new Set(read)]) {
      const block = blocks.get(name);
      const outside = block && block.writtenBy.cells.length === 0 ? outsideOf([block]) : null;
      if (outside === null) continue;
      const id = conditionTagNode(at, name);
      nodes.push(node({ id, kind: 'outside', cell: cell.id, name, outside }));
      edges.push(link({ id: `x:${id}`, source: id, target: at, kind: 'outside', to: cell.id }));
    }
  };

  const forks = new Map(path.forks.map((f) => [f.cell, f]));
  const entered = new Set(path.steps.map((s) => s.to));
  // A cell with a `when` that the main path never reaches has its condition drawn before it as a fork with no branch.
  const unreached = new Map(
    flow.cells
      .filter((c, i) => c.when.length > 0 && i > 0 && !entered.has(c.id) && !forks.has(c.id))
      .map((c) => [c.id, forkNode(c.id)]),
  );
  for (const cell of flow.cells) {
    const fork = forks.get(cell.id);
    if (fork !== undefined) {
      conditionTags(cell, fork.keys, fork.id);
      nodes.push(node({ id: fork.id, kind: 'fork', cell: cell.id, when: fork.keys, name: fork.block }));
    } else if (unreached.has(cell.id)) {
      const id = forkNode(cell.id);
      conditionTags(cell, cell.when, id);
      nodes.push(node({ id, kind: 'fork', cell: cell.id, when: cell.when }));
      edges.push(link({ id: `in:${id}`, source: id, target: cellNode(cell.id), kind: 'in', to: cell.id }));
    }
    if (cell.requires.length > 0) {
      const id = wayInNode(cell.id);
      conditionTags(cell, cell.requires, id);
      nodes.push(node({ id, kind: 'way-in', cell: cell.id, requires: cell.requires }));
      edges.push(
        link({
          id: `in:${cell.id}`,
          source: id,
          target: cellNode(cell.id),
          kind: 'in',
          to: cell.id,
          main: path.through.has(cell.id),
        }),
      );
    }
    nodes.push(node({ id: cellNode(cell.id), kind: 'cell', cell: cell.id }));
    // What runs it from outside the cells: each datum or group its own file watches that people or a source outside
    // the work writes.
    for (const w of cell.watches) {
      const outside = outsideOf(w.blocks.flatMap((b) => blocks.get(b) ?? []));
      const id = outsideNode(cell.id, w.name);
      if (outside === null || nodes.some((n) => n.id === id)) continue;
      nodes.push(node({ id, kind: 'outside', cell: cell.id, name: w.name, group: !namesItself(w), outside }));
      edges.push(link({ id: `x:${id}`, source: id, target: cellNode(cell.id), kind: 'outside', to: cell.id }));
    }
  }

  // The main path's steps, before the arrows, so that the layout takes them first; the cells they reach.
  const reached = new Set<string>();
  const steps = path.steps.map((s) =>
    link({
      id: `m:${s.source}>${s.target}`,
      source: s.source,
      target: s.target,
      kind: 'step',
      from: s.from,
      to: s.to,
      main: true,
      branch: s.branch,
    }),
  );
  for (const s of steps) reached.add(s.to);
  edges.push(...steps);
  const out = new Map<string, ChartEdge[]>();
  for (const s of steps) out.set(s.source, [...(out.get(s.source) ?? []), s]);
  const isFork = (id: string) => id.startsWith('f:');
  /// The steps an arrow from `writer` into `reader` runs along: the last step of each way the main path goes from the
  /// writer to where the reader is entered through forks alone, with no cell run between.
  const along = (writer: Cell, reader: Cell): ChartEdge[] => {
    const found: ChartEdge[] = [];
    const seen = new Set<string>();
    let at = out.get(cellNode(writer.id)) ?? [];
    while (at.length > 0) {
      const next: ChartEdge[] = [];
      for (const s of at) {
        if (seen.has(s.id)) continue;
        seen.add(s.id);
        if (s.target === entryOf(reader)) found.push(s);
        else if (isFork(s.target)) next.push(...(out.get(s.target) ?? []));
      }
      at = next;
    }
    return found;
  };

  // The arrows: from each writer, in the order of the walk, to each cell it can run, into the cell itself; one that runs
  // along the main path is folded into its step.
  const back: ChartEdge[] = [];
  for (const writer of flow.cells) {
    for (const reader of flow.cells) {
      if (reader.id === writer.id || !canRun(writer, reader)) continue;
      const carries = carried(flow, writer, reader);
      if (carries.length === 0) continue;
      const isBack = (place.get(writer.id) ?? 0) > (place.get(reader.id) ?? 0);
      const arrow = link({
        id: `a:${writer.id}>${reader.id}`,
        source: cellNode(writer.id),
        target: cellNode(reader.id),
        kind: 'arrow',
        from: writer.id,
        to: reader.id,
        carries,
        back: isBack,
        label: carries.map((c) => c.name).join(', '),
      });
      if (isBack) {
        back.push(arrow);
        continue;
      }
      const into = along(writer, reader);
      if (into.length === 0) edges.push(arrow);
      for (const s of into) {
        s.folds.push(arrow);
        for (const c of carries) if (!s.carries.some((x) => x.name === c.name)) s.carries.push(c);
        s.label = s.carries.map((c) => c.name).join(', ');
      }
    }
  }
  // The ways back: one from a cell to every cell a fork's branches lead to is drawn into the fork, the forks taken in
  // the order of the walk, each way back drawn once; every other into the cell it runs.
  const used = new Set<string>();
  const branchCells = (fork: string) => [...new Set((out.get(fork) ?? []).map((s) => s.to))];
  const ordered = [...path.forks].sort((a, b) => (place.get(a.cell) ?? 0) - (place.get(b.cell) ?? 0));
  for (const writer of flow.cells) {
    const mine = back.filter((a) => a.from === writer.id);
    for (const fork of ordered) {
      const to = branchCells(fork.id);
      const each = to.map((c) => mine.find((a) => a.to === c && !used.has(a.id)));
      if (to.length < 2 || each.some((a) => a === undefined)) continue;
      const these = each as ChartEdge[];
      for (const a of these) used.add(a.id);
      const carries: Carried[] = [];
      for (const a of these) for (const c of a.carries) if (!carries.some((x) => x.name === c.name)) carries.push(c);
      edges.push(
        link({
          id: `a:${writer.id}>${fork.id}`,
          source: cellNode(writer.id),
          target: fork.id,
          kind: 'arrow',
          from: writer.id,
          to: fork.cell,
          reaches: to,
          carries,
          back: true,
          label: carries.map((c) => c.name).join(', '),
        }),
      );
    }
  }
  edges.push(...back.filter((a) => !used.has(a.id)));
  // A cell the main path does not reach is kept under the cell before it, so that the cells stand in the walk's order:
  // the arrows a write runs are not laid out, but drawn over the layout (`data.ts`).
  flow.cells.forEach((cell, i) => {
    if (i === 0 || reached.has(cell.id)) return;
    const source = cellNode(flow.cells[i - 1].id);
    const target = unreached.get(cell.id) ?? entryOf(cell);
    edges.push(link({ id: `t:${cell.id}`, source, target, kind: 'tie', to: cell.id }));
  });

  return { nodes, edges, waysBack: matrix(flow).waysBack };
}

/// A block a cell's own file gives it, by what it reads or what it watches: with the kind it is given in, whether a
/// new version of it runs the cell (its own file watches it, by itself or through a group), the cells that write it and
/// who else does.
export interface ContextTag {
  block: string;
  kind: InputKind | null;
  watched: boolean;
  writers: string[];
  outside: Outside | null;
}

/// What a cell's own file gives it, in the order it is given them: each block of a name its file reads or watches.
/// What `[every_agent]` gives every agent's cell alike, and no name of its file stands for, is the view line's.
export function contextOf(flow: Flow, id: string): ContextTag[] {
  const cell = flow.cells.find((c) => c.id === id);
  if (!cell) return [];
  const blocks = new Map(flow.blocks.map((b) => [b.name, b]));
  const watched = new Set(cell.watches.flatMap((w) => w.blocks));
  const own = new Set([...watched, ...cell.reads.flatMap((r) => r.blocks)]);
  return cell.given
    .filter((g) => own.has(g.block))
    .map((g) => {
      const block = blocks.get(g.block);
      return {
        block: g.block,
        kind: g.kind,
        watched: watched.has(g.block),
        writers: block?.writtenBy.cells ?? [],
        outside: block ? outsideOf([block]) : null,
      };
    });
}

/// The cells lit while a cell is chosen: those that write what its own file gives it, the cell itself left out.
export function litBy(flow: Flow, id: string): Set<string> {
  return new Set(contextOf(flow, id).flatMap((t) => t.writers.filter((w) => w !== id)));
}

/// The arrows a write runs that the chart draws besides the main path and the ways back: those into and out of the
/// cell chosen, as what its own file gives it is shown only while it is chosen; every one where the owner shows them
/// all. Those folded into the main path are drawn as its steps.
export function shownArrows(chart: Chart, chosen: string | null, all: boolean): ChartEdge[] {
  return chart.edges.filter(
    (e) => e.kind === 'arrow' && !e.back && (all || (chosen !== null && (e.from === chosen || e.to === chosen))),
  );
}
