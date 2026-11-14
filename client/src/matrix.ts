// The chart as a matrix: the blocks as rows, the cells as columns in the order of the walk, and at each crossing what
// the cell does with the block; over the blocks, a row for each line the cells run in and for each condition they
// name. Everything the chart draws is worked out here, from the interface's JSON alone, so that it can be tested
// without drawing: the order of the rows, the marks and the ways back.

import type { Block, Cell, Condition, Flow, InputKind, Line, Test } from './api';

/// What a cell does with a block: a new version of it runs the cell, the cell is given it, may write it as an output,
/// reads its value in a key of its `when` (the block's own key, or a named test that reads it), or in a key of its
/// `requires`.
export interface Mark {
  runs: boolean;
  given: boolean;
  write: boolean;
  condition: boolean;
  precondition: boolean;
  /// Of a datum the cell is given, the kind it is given in, where the cell says one.
  kind?: InputKind | null;
}

export const NO_MARK: Mark = {
  runs: false,
  given: false,
  write: false,
  condition: false,
  precondition: false,
};

export const MARK_PARTS = ['runs', 'given', 'write', 'condition', 'precondition'] as const;
export type MarkPart = (typeof MARK_PARTS)[number];

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

/// The mark of a cell on a block.
export function markOf(cell: Cell | undefined, block: string): Mark {
  if (!cell) return NO_MARK;
  return {
    runs: cell.runsOn.includes(block),
    given: cell.given.includes(block),
    write: cell.outputs.includes(block),
    condition: cell.when.some((key) => keyReads(key, block)),
    precondition: cell.requires.some((key) => keyReads(key, block)),
  };
}

export function hasMark(mark: Mark): boolean {
  return mark.runs || mark.given || mark.write || mark.condition || mark.precondition;
}

/// The order of the blocks' rows. A block stands where it is first written in the walk, a cell's outputs together
/// in the order the cell lists them; a block no cell writes (what people or the outside write) stands just before the
/// first cell that reads it: the blocks its `when` reads first, then those its `requires` reads, then those it
/// names, that run it or that it is given, in their order; a block no cell writes or reads stands last. Ties go by
/// name. The marks then step down to the right like a staircase, each cell's writes one under another and next to the
/// reads that follow from them, and a write right of a read stands out as a way back.
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
      return [first, 0, mark.condition ? -2 : mark.precondition ? -1 : cell.inputs.indexOf(block.name)];
    }
    return [Infinity, 0, 0];
  };
  // The rows of what stands only for a trigger come first, apart from the data's.
  return [...flow.blocks]
    .map((block) => ({
      name: block.name,
      key: [block.trigger ? 0 : 1, ...key(block)],
    }))
    .sort((a, b) => {
      for (let i = 0; i < 4; i += 1) {
        if (a.key[i] !== b.key[i]) return a.key[i] < b.key[i] ? -1 : 1;
      }
      return a.name < b.name ? -1 : a.name > b.name ? 1 : 0;
    })
    .map((b) => b.name);
}

/// A row: a trigger or a datum.
export interface BlockRow {
  /// What the row is called: a trigger's name, or the block's where each cell has its own trigger of one name; a
  /// datum's name.
  name: string;
  /// Of a trigger's row, the trigger's name; none of a datum's.
  trigger?: string;
  /// Of a trigger's row, what the trigger is, as its file writes it.
  what?: string;
  /// What the row is about: the block the trigger stands on, or the group's name of a group's trigger; the datum.
  on: string;
  /// Of a group's trigger, the group's blocks, a new version of any of which fires it.
  group?: string[];
  /// The block the row is about; none of a group's trigger, which stands on no one block.
  block?: Block;
}

/// How a row's line runs through one crossing: from the crossing's middle to the right (towards later cells), to the
/// left, both, or not at all.
export type Segment = 'none' | 'right' | 'left' | 'through';

/// One crossing of a row and a column.
export interface Spot {
  /// What the cell does with the block.
  mark: Mark;
  /// The row's line, which the eye follows along the cells: from the first mark that no way back reaches to the
  /// last mark. A mark left of every write that a way back reaches is reached by the way back alone, so the line
  /// starts at the first cell that writes the block, the way the block goes on from there; where no cell writes it, at
  /// the first mark.
  line: Segment;
  /// The way back, on a trigger's row: from a later cell's write of the block the trigger stands on, or of one of the
  /// group's, leftwards to the cells before it that the trigger runs and that can run where the writer does (not
  /// `exclusive`). A block a cell is only given, or whose value its condition reads, does not run it, so no way back
  /// reaches it.
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

/// The matrix of a flow.
export interface Matrix {
  /// The rows, top to bottom.
  blocks: BlockRow[];
  /// The columns, left to right: the order of the walk.
  cells: Cell[];
  /// By block, then by cell.
  spots: Spot[][];
  /// The rows of the lines the agents' cells name, at the top, each where its first cell stands.
  lines: LineRow[];
  /// The rows of the conditions the cells name, under the lines', each where the first cell that names it stands.
  conditions: ConditionRow[];
  /// How many ways back the flow has: pairs of a cell a trigger runs and a later cell's write of the block the trigger
  /// stands on, or of one of its group's, of two cells that can both run in one work.
  waysBack: number;
}

/// Works out from a flow everything its chart draws.
export function matrix(flow: Flow): Matrix {
  const cells = flow.cells;
  const blockIn = (name: string) => flow.blocks.find((b) => b.name === name);
  // The triggers' rows first, each a trigger a cell names, by the block it stands on, in the order of those blocks, a
  // group's where the first of its blocks stands; then the data's rows. A trigger stands for one event: one each cell
  // has its own of is a row for each cell.
  const order = blockOrder(flow);
  const pairs: { name: string; block: string; what?: string; group?: string[] }[] = [];
  for (const cell of cells) {
    for (const t of cell.triggers ?? []) {
      if (!pairs.some((p) => p.name === t.name && p.block === t.block)) {
        const group = t.group.length > 0 ? t.group : undefined;
        pairs.push({ name: t.name, block: t.block, what: t.what ?? undefined, group });
      }
    }
  }
  const at = (p: (typeof pairs)[number]) => Math.min(...(p.group ?? [p.block]).map((b) => order.indexOf(b)));
  pairs.sort((x, y) => at(x) - at(y));
  const blocksOf = (name: string) => new Set(pairs.filter((p) => p.name === name).map((p) => p.block)).size;
  const row = (name: string, on: string, trigger?: string, what?: string, group?: string[]): BlockRow => ({
    name,
    trigger,
    what,
    on,
    group,
    block: group ? undefined : blockIn(on),
  });
  const blocks: BlockRow[] = [
    ...pairs.map((p) => row(blocksOf(p.name) > 1 ? p.block : p.name, p.block, p.name, p.what, p.group)),
    ...order.filter((name) => !blockIn(name)?.trigger).map((name) => row(name, name)),
  ];

  // Of a trigger's row: whether the cell runs on the trigger, and whether it writes the block the trigger stands on,
  // or one of the group's. Of a datum's row: whether the cell is given it and in which kind, or writes it. What a
  // cell's `when` and `requires` read is drawn once, on the rows of their keys.
  const markIn = (cell: Cell, r: BlockRow): Mark => {
    if (r.trigger !== undefined) {
      return {
        ...NO_MARK,
        runs: (cell.triggers ?? []).some((t) => t.name === r.trigger && t.block === r.on),
        write: (r.group ?? [r.on]).some((b) => cell.outputs.includes(b)),
      };
    }
    const mark = markOf(cell, r.on);
    return {
      ...mark,
      runs: false,
      condition: false,
      precondition: false,
      kind: mark.given ? (cell.input[r.on]?.given?.kind ?? null) : null,
    };
  };
  const spots: Spot[][] = blocks.map((r) =>
    cells.map((cell) => ({
      mark: markIn(cell, r),
      line: 'none',
      back: 'none',
      backTo: false,
    })),
  );

  // Lines and ways back only on the triggers' rows: a datum in a context runs no cell.
  let waysBack = 0;
  spots.forEach((row, r) => {
    if (blocks[r].trigger === undefined) return;
    const marks = row.map((spot) => spot.mark);
    const marked = marks.flatMap((m, c) => (hasMark(m) ? [c] : []));
    const reads = marks.flatMap((m, c) => (m.runs ? [c] : []));
    const writes = marks.flatMap((m, c) => (m.write ? [c] : []));
    // Of a read, the later writes that send the work back to it: those of a cell that can run where it does.
    const backFrom = (c: number) => writes.filter((w) => w > c && !exclusive(cells[c], cells[w]));
    const reached = reads.filter((c) => backFrom(c).length > 0);
    paint(
      row,
      'line',
      marked.find((c) => !reached.includes(c) || marks[c].write),
      marked[marked.length - 1],
    );
    for (const c of reached) row[c].backTo = true;
    waysBack += reads.reduce((n, c) => n + backFrom(c).length, 0);
    if (reached.length > 0) {
      paint(row, 'back', reached[0], Math.max(...reached.flatMap(backFrom)));
    }
  });

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
  const named: { name: string; test?: Test; block?: string }[] = [];
  for (const c of cells) {
    for (const key of [...c.when, ...c.requires]) {
      const row = key.on === 'block' ? { name: key.block, block: key.block } : { name: key.test.name, test: key.test };
      if (!named.some((r) => r.name === row.name && (r.test === undefined) === (row.test === undefined))) {
        named.push(row);
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
  const conditions: ConditionRow[] = named.map((row) => ({
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

  return { blocks, cells, spots, lines, conditions, waysBack };
}

/// How a line runs along a row of `n` crossings from the middle of the one at `from` to the middle of the one at `to`.
function span(n: number, from?: number, to?: number): Segment[] {
  return Array.from({ length: n }, (_, c): Segment => {
    if (from === undefined || to === undefined || from >= to || c < from || c > to) return 'none';
    return c === from ? 'right' : c === to ? 'left' : 'through';
  });
}

/// Paints a line along a row from the middle of the crossing at `from` to the middle of the one at `to`.
function paint(row: Spot[], which: 'line' | 'back', from?: number, to?: number): void {
  span(row.length, from, to).forEach((segment, c) => {
    if (segment !== 'none') row[c][which] = segment;
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
