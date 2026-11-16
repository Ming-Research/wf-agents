// The chart as a matrix: the data as rows, the cells as columns in the order of the walk, and at each crossing what
// the cell does with the datum, by the keys of its file: `watches`, `reads`, `outputs`. Over the data, a row for each
// line the cells run in, for each condition they name, for each of the engine's tools they may call, and for each name
// `[every_agent]` writes. Everything the chart draws is worked out here, from the interface's JSON alone, so that it can
// be tested without drawing: the order of the rows, the marks and the ways back.

import type { Block, Cell, Condition, Flow, InputKind, Line, Test } from './api';

/// What a cell does with a datum: a new version of it runs the cell (`watches`), the cell is given it and it runs
/// nothing (`reads`), the cell may write it (`outputs`); the kind the cell is given it in, where it is given it; and
/// whether a key of its `when` or of its `requires` reads its value.
export interface Mark {
  write: boolean;
  watch: boolean;
  read: boolean;
  kind: InputKind | null;
  condition: boolean;
  precondition: boolean;
}

export const NO_MARK: Mark = {
  write: false,
  watch: false,
  read: false,
  kind: null,
  condition: false,
  precondition: false,
};

/// Whether two cells can never both run in one work, by their conditions as written: each one's `when` names a block or
/// a datum the other's names with another value. A named test, or keys on different blocks, can both hold. No way back
/// is drawn, counted or listed between two such cells: a later cell's write cannot send the work back to a cell that
/// cannot run where the later one did.
export function exclusive(a: Cell | undefined, b: Cell | undefined): boolean {
  if (!a || !b) return false;
  return a.when.some(
    (x) => x.on === 'block' && b.when.some((y) => y.on === 'block' && y.block === x.block && y.value !== x.value),
  );
}

/// Whether a key of a `when` or a `requires` reads the block: names it, or is a named test that reads it.
export function keyReads(key: Condition, block: string): boolean {
  return key.on === 'block' ? key.block === block : key.test.reads.includes(block);
}

/// Whether a name a cell takes stands for one datum of that name, not for a group's blocks or the data a name of
/// `[every_agent]` holding `{each}` stands for.
export function namesItself(n: { name: string; blocks: string[] }): boolean {
  return n.blocks.length === 1 && n.blocks[0] === n.name;
}

/// The kind a cell is given a block in; none where it is not given it.
export function kindOf(cell: Cell, block: string): InputKind | null {
  return cell.given.find((g) => g.block === block)?.kind ?? null;
}

/// The mark of a cell on a datum by any list that names it, its own file's, a group's or `[every_agent]`'s: a new
/// version of it runs the cell, or the cell is only given it. This is what a work's history marks under each cell a
/// write reaches; the matrix puts a group's watcher on the group's row and leaves what every agent's cell reads to the
/// words of its band.
export function markOf(cell: Cell | undefined, block: string): Mark {
  if (!cell) return NO_MARK;
  const watch = cell.watches.some((w) => w.blocks.includes(block));
  const kind = kindOf(cell, block);
  return {
    write: cell.outputs.includes(block),
    watch,
    read: !watch && kind !== null,
    kind,
    condition: cell.when.some((key) => keyReads(key, block)),
    precondition: cell.requires.some((key) => keyReads(key, block)),
  };
}

export function hasMark(mark: Mark): boolean {
  return mark.write || mark.watch || mark.read || mark.condition || mark.precondition;
}

/// The blocks a cell takes, in the order it takes them: those that run it, then those it is only given.
function takenBy(cell: Cell): string[] {
  return [...new Set([...cell.watches.flatMap((w) => w.blocks), ...cell.given.map((g) => g.block)])];
}

/// The order of the blocks' rows. A block stands where it is first written in the walk, a cell's outputs together
/// in the order the cell lists them; a block no cell writes (what people or the outside write) stands just before the
/// first cell that takes it: the blocks its `when` reads first, then those its `requires` reads, then those that run
/// it, then those it is only given, in their order; a block no cell writes or takes stands last. Ties go by name. The
/// marks then step down to the right like a staircase, each cell's writes one under another and next to the watches
/// that follow from them, and a write right of a watch stands out as a way back.
export function blockOrder(flow: Flow): string[] {
  const place = new Map(flow.cells.map((c, i) => [c.id, i]));
  const key = (block: Block): [number, number, number] => {
    const writer = block.writtenBy.cells[0];
    if (writer !== undefined) {
      const cell = flow.cells[place.get(writer) ?? 0];
      return [place.get(writer) ?? 0, 1, cell.outputs.indexOf(block.name)];
    }
    const first = flow.cells.findIndex((cell) => hasMark(markOf(cell, block.name)));
    if (first !== -1) {
      const cell = flow.cells[first];
      const mark = markOf(cell, block.name);
      return [first, 0, mark.condition ? -2 : mark.precondition ? -1 : takenBy(cell).indexOf(block.name)];
    }
    return [Infinity, 0, 0];
  };
  return [...flow.blocks]
    .map((block) => ({ name: block.name, key: key(block) }))
    .sort((a, b) => {
      for (let i = 0; i < 3; i += 1) {
        if (a.key[i] !== b.key[i]) return a.key[i] < b.key[i] ? -1 : 1;
      }
      return a.name < b.name ? -1 : a.name > b.name ? 1 : 0;
    })
    .map((b) => b.name);
}

/// A row of the data: a datum, or a group a cell watches by its name.
export interface BlockRow {
  /// The datum's name, or the group's.
  name: string;
  /// Of a group's row, its blocks as `flow.toml` names them; none of a datum's.
  group?: string[];
  /// Of a datum's row, the block as the flow declares it.
  block?: Block;
  /// Of a datum's row, the groups it is of that have rows of their own, whose watchers are marked there.
  of: string[];
}

/// How a row's line runs through one crossing: from the crossing's middle to the right (towards later cells), to the
/// left, both, or not at all.
export type Segment = 'none' | 'right' | 'left' | 'through';

/// One crossing of a row and a column.
export interface Spot {
  /// What the cell does with the row's data.
  mark: Mark;
  /// The way back, the only line along a row of the data: from a later cell's write of the row's datum, of one of its
  /// group's or of the data a name of `[every_agent]` stands for, leftwards to each cell before it that watches it by
  /// the row's name and can run where the writer does (not `exclusive`).
  back: Segment;
  /// A cell that a write right of it sends the work back to.
  backTo: boolean;
}

/// Where a line's row crosses a cell's column: whether the cell is on the line, and the row's line through it.
export interface LineSpot {
  on: boolean;
  /// The row's line, from the first cell on the line to the last: one session going on.
  line: Segment;
}

/// A row of a line: the cells of an agent's that share one session.
export interface LineRow {
  name: string;
  /// The line as the flow declares it.
  line: Line | undefined;
  /// By cell, in the order of the columns.
  spots: LineSpot[];
}

/// Where a condition's row crosses a cell's column: whether a key of the cell's `when` is on the row, so that the cell
/// runs only when it holds and is skipped otherwise, and whether a key of its `requires` is, so that the work waits
/// before the cell until it holds.
export interface ConditionSpot {
  when: boolean;
  /// Of a block's row, the value the cell's `when` waits for.
  value?: string;
  requires: boolean;
  /// Of a block's row, the value the cell's `requires` waits for.
  requiresValue?: string;
}

/// A row of a condition the cells name: a named test, or a block or datum a key of a cell's `when` or `requires`
/// names.
export interface ConditionRow {
  /// The test's name, or the block's.
  name: string;
  /// Of a named test's row, the test as the flow declares it.
  test?: Test;
  /// Of a block's row, the block.
  block?: string;
  /// By cell, in the order of the columns.
  spots: ConditionSpot[];
}

/// A row of one of the engine's tools: by cell, whether the cell may call it.
export interface ToolRow {
  name: string;
  spots: boolean[];
}

/// A row of a name `[every_agent]` writes: whether it watches or reads it, the kind it gives it in, the data it stands
/// for, and under each cell what runs the cell of those data, what its own file reads of them and what it writes; what
/// every agent's cell only reads is said by the row's words.
export interface EveryRow {
  name: string;
  list: 'watches' | 'reads';
  kind: InputKind | null;
  blocks: string[];
  /// The data it stands for, as the flow declares them.
  data: Block[];
  spots: Spot[];
}

/// The matrix of a flow.
export interface Matrix {
  /// The rows of the data, top to bottom: each datum, and each group a cell watches by its name over its first block.
  blocks: BlockRow[];
  /// The columns, left to right: the order of the walk.
  cells: Cell[];
  /// By row of the data, then by cell.
  spots: Spot[][];
  /// The rows of the lines the agents' cells name, at the top, each where its first cell stands.
  lines: LineRow[];
  /// The rows of the conditions the cells name, under the lines', each where the first cell that names it stands.
  conditions: ConditionRow[];
  /// The rows of the engine's tools: the flow's, or else each a cell names, in the order named.
  tools: ToolRow[];
  /// The rows of what `[every_agent]` names, as `flow.toml` writes it: what it watches, then what it reads.
  every: EveryRow[];
  /// How many ways back the flow has: pairs of a cell that watches a name and a later cell's write of a block the name
  /// stands for, of two cells that can both run in one work.
  waysBack: number;
}

/// Draws the ways back along a row and counts them: for each block the row stands for, from each later cell that
/// writes it to each earlier cell that watches it by the row's name and can run where the writer does.
function waysBackAlong(
  row: Spot[],
  cells: Cell[],
  blocks: string[],
  watches: (c: number, block: string) => boolean,
  writes: (c: number, block: string) => boolean,
): number {
  let count = 0;
  let first = -1;
  let furthest = -1;
  for (const block of blocks) {
    for (let c = 0; c < cells.length; c += 1) {
      if (!watches(c, block)) continue;
      const from = cells.flatMap((_, w) => (w > c && writes(w, block) && !exclusive(cells[c], cells[w]) ? [w] : []));
      if (from.length === 0) continue;
      count += from.length;
      row[c].backTo = true;
      first = first === -1 ? c : Math.min(first, c);
      furthest = Math.max(furthest, ...from);
    }
  }
  if (first !== -1) paint(row, first, furthest);
  return count;
}

/// Works out from a flow everything its chart draws.
export function matrix(flow: Flow): Matrix {
  const cells = flow.cells;
  const blockIn = (name: string) => flow.blocks.find((b) => b.name === name);
  let waysBack = 0;

  // The data's rows, but those `[every_agent]` names, which its band holds; a group a cell watches by its name stands
  // right above the first of its blocks' rows.
  const every = flow.everyAgent;
  const alike = new Set([...every.watches, ...every.reads].flatMap((n) => n.blocks));
  const order = blockOrder(flow).filter((name) => !alike.has(name));
  const watchesGroup = (cell: Cell, group: string) =>
    cell.watches.some((w) => w.list === 'watches' && w.name === group);
  const groups = flow.groups.filter((g) => cells.some((c) => watchesGroup(c, g.name)));
  const firstOf = (g: { blocks: string[] }) =>
    Math.min(...g.blocks.map((b) => (order.includes(b) ? order.indexOf(b) : Infinity)));
  const blocks: BlockRow[] = [];
  const placed = new Set<string>();
  const placeGroups = (upTo: number) => {
    for (const g of groups.filter((x) => !placed.has(x.name) && firstOf(x) <= upTo)) {
      placed.add(g.name);
      blocks.push({ name: g.name, group: g.blocks, of: [] });
    }
  };
  order.forEach((name, i) => {
    placeGroups(i);
    blocks.push({ name, block: blockIn(name), of: groups.filter((g) => g.blocks.includes(name)).map((g) => g.name) });
  });
  placeGroups(Infinity);

  // A datum's row: what each cell's file says of the datum by its own name, or, of what it reads, through a group; a
  // cell that watches it through a group is marked on the group's row. A group's row: a cell whose `watches` names
  // the group, and a cell that writes one of its blocks. The kind is the one the cell is given the datum in.
  const watchesItself = (cell: Cell, block: string) => cell.watches.some((w) => w.name === block && namesItself(w));
  const markIn = (cell: Cell, r: BlockRow): Mark => {
    if (r.group !== undefined) {
      return { ...NO_MARK, watch: watchesGroup(cell, r.name), write: r.group.some((b) => cell.outputs.includes(b)) };
    }
    const watch = watchesItself(cell, r.name);
    const read = !watch && cell.reads.some((n) => n.blocks.includes(r.name));
    return {
      ...NO_MARK,
      write: cell.outputs.includes(r.name),
      watch,
      read,
      kind: watch || read ? kindOf(cell, r.name) : null,
    };
  };
  const spots: Spot[][] = blocks.map((r) =>
    cells.map((cell): Spot => ({ mark: markIn(cell, r), back: 'none', backTo: false })),
  );
  spots.forEach((row, r) => {
    waysBack += waysBackAlong(
      row,
      cells,
      [blocks[r].name],
      (c) => row[c].mark.watch,
      (w) => row[w].mark.write,
    );
  });

  // The band of `[every_agent]`: under each cell, what runs it of the data a name stands for, each datum by its own name
  // in the cell's file or by `[every_agent]` (a group's watcher is marked on the group's row), what its own file reads
  // of them, and what it writes; what every agent's cell only reads is said by the row's words. Its ways back are
  // drawn datum by datum, to each cell that watches the datum so, and so counted once.
  const named = [
    ...every.watches.map((n) => ({ name: n.name, blocks: n.blocks, list: 'watches' as const })),
    ...every.reads.map((n) => ({ name: n.name, blocks: n.blocks, list: 'reads' as const })),
  ];
  const everyRows: EveryRow[] = named.map(({ name, blocks: of, list }) => {
    const row = cells.map((cell): Spot => {
      const watched = of.filter((b) => watchesItself(cell, b));
      const read = of.filter((b) => !watched.includes(b) && cell.reads.some((n) => n.blocks.includes(b)));
      // The kind drawn is the one of what it reads, inside the ring of `reads`; else of what it watches, for its words.
      const first = read[0] ?? watched[0];
      return {
        mark: {
          ...NO_MARK,
          watch: watched.length > 0,
          read: read.length > 0,
          write: of.some((b) => cell.outputs.includes(b)),
          kind: first === undefined ? null : kindOf(cell, first),
        },
        back: 'none',
        backTo: false,
      };
    });
    waysBack += waysBackAlong(
      row,
      cells,
      of,
      (c, block) => watchesItself(cells[c], block),
      (w, block) => cells[w].outputs.includes(block),
    );
    const data = of.flatMap((b) => blockIn(b) ?? []);
    return { name, list, kind: every.kinds[name] ?? null, blocks: of, data, spots: row };
  });

  // The tools: the flow's, which every agent's cell may call, or else each a cell names, in the order named.
  const toolNames = flow.tools.length > 0 ? flow.tools : [...new Set(cells.flatMap((c) => c.tools))];
  const tools: ToolRow[] = toolNames.map((name) => ({ name, spots: cells.map((c) => c.tools.includes(name)) }));

  // The lines, each where the first cell on it stands, one session going on along its row from that cell to the last.
  const lineNames = [...new Set(cells.flatMap((c) => (c.kind === 'agent' && c.line ? [c.line] : [])))];
  const lines: LineRow[] = lineNames.map((name) => {
    const on = cells.map((c) => c.kind === 'agent' && c.line === name);
    const marked = on.flatMap((has, c) => (has ? [c] : []));
    const line = span(cells.length, marked[0], marked[marked.length - 1]);
    return {
      name,
      line: flow.lines.find((l) => l.name === name),
      spots: on.map((has, c) => ({ on: has, line: line[c] })),
    };
  });

  // The conditions, each where the first cell that names it stands, every key of a cell's `when` first, then of its
  // `requires`, in their order. A block is one row, whatever value each cell waits for; a test and a block of one name
  // are two.
  const conditionsNamed: { name: string; test?: Test; block?: string }[] = [];
  for (const c of cells) {
    for (const key of [...c.when, ...c.requires]) {
      const row = key.on === 'block' ? { name: key.block, block: key.block } : { name: key.test.name, test: key.test };
      if (!conditionsNamed.some((r) => r.name === row.name && (r.test === undefined) === (row.test === undefined))) {
        conditionsNamed.push(row);
      }
    }
  }
  // The key of these that is on the row: of a block's row, by the value it waits for; of a test's row, as ''.
  const keyOn = (row: { name: string; block?: string }, keys: Condition[]): string | undefined => {
    for (const key of keys) {
      if (row.block !== undefined && key.on === 'block' && key.block === row.block) return key.value;
      if (row.block === undefined && key.on === 'test' && key.test.name === row.name) return '';
    }
    return undefined;
  };
  const conditions: ConditionRow[] = conditionsNamed.map((row) => ({
    ...row,
    test: row.test && (flow.tests.find((t) => t.name === row.name) ?? row.test),
    spots: cells.map((c) => {
      const value = keyOn(row, c.when);
      const requiresValue = keyOn(row, c.requires);
      return {
        when: value !== undefined,
        value: row.block !== undefined ? value : undefined,
        requires: requiresValue !== undefined,
        requiresValue: row.block !== undefined ? requiresValue : undefined,
      };
    }),
  }));

  return { blocks, cells, spots, lines, conditions, tools, every: everyRows, waysBack };
}

/// How a line runs along a row of `n` crossings from the middle of the one at `from` to the middle of the one at `to`.
function span(n: number, from?: number, to?: number): Segment[] {
  return Array.from({ length: n }, (_, c): Segment => {
    if (from === undefined || to === undefined || from >= to || c < from || c > to) return 'none';
    return c === from ? 'right' : c === to ? 'left' : 'through';
  });
}

/// Paints the way back along a row from the middle of the crossing at `from` to the middle of the one at `to`.
function paint(row: Spot[], from: number, to: number): void {
  span(row.length, from, to).forEach((segment, c) => {
    if (segment !== 'none') row[c].back = segment;
  });
}

/// Where a block another cell writes or reads stands from a cell: above it, below it, or the cell itself.
export type Where = 'above' | 'below' | 'itself';

export function where(flow: Flow, from: string, to: string): Where {
  const ids = flow.cells.map((c) => c.id);
  const a = ids.indexOf(from);
  const b = ids.indexOf(to);
  return b === a ? 'itself' : b < a ? 'above' : 'below';
}
