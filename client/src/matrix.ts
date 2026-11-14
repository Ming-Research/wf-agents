// The chart as a matrix: the blocks as rows, the cells as columns in the order of the walk, and at each crossing what
// the cell does with the block. Everything the chart draws is worked out here, from the interface's JSON alone, so
// that it can be tested without drawing: the order of the rows, the marks, the ways back and what changed.
//
// When there is a definition to compare against, the matrix holds the cells and blocks of both: the one whose version
// is shown in its order, each that only the other has standing after the one it followed there (of those that did not
// move), so that switching between the two versions keeps every row and column that both have where the owner looks.

import type { Block, Cell, Change, Flow, InputKind } from './api';

/// What a cell does with a block: a new version of it runs the cell, the cell is given it, may write it as an output,
/// reads its value in its condition (directly, or by the named test that is its condition), or reads its value in a
/// named test that is one of its preconditions.
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

/// The mark of a cell on a block.
export function markOf(cell: Cell | undefined, block: string): Mark {
  if (!cell) return NO_MARK;
  const condition = cell.condition;
  return {
    runs: cell.runsOn.includes(block),
    given: cell.given.includes(block),
    write: cell.outputs.includes(block),
    condition:
      condition !== null &&
      (condition.on === 'block' ? condition.block === block : condition.test.reads.includes(block)),
    precondition: cell.preconditions.some((test) => test.reads.includes(block)),
  };
}

export function hasMark(mark: Mark): boolean {
  return mark.runs || mark.given || mark.write || mark.condition || mark.precondition;
}

/// The order of the blocks' rows. A block stands where it is first written in the walk, a cell's outputs together
/// in the order the cell lists them; a block no cell writes (what people or the outside write) stands just before the
/// first cell that reads it: the blocks its condition reads first, then those its preconditions read, then those it
/// names, that run it or that it is given, in their order; a block no cell writes or reads stands last. Ties go by name. The marks then step down to the right
/// like a staircase, each cell's writes one under another and next to the reads that follow from them, and a write
/// right of a read stands out as a way back.
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

/// The names of both orders: `shown`'s in its order, each name only `other` has standing after the nearest name before
/// it there that `shown` has too and that is not in `moved`, or first when there is none.
export function merged(shown: string[], other: string[], moved: ReadonlySet<string> = new Set()): string[] {
  const kept = new Set(shown);
  const first: string[] = [];
  const following = new Map<string, string[]>();
  let anchor: string | undefined;
  for (const name of other) {
    if (kept.has(name)) {
      if (!moved.has(name)) anchor = name;
    } else if (anchor === undefined) {
      first.push(name);
    } else {
      following.set(anchor, [...(following.get(anchor) ?? []), name]);
    }
  }
  const all = [...first];
  for (const name of shown) {
    all.push(name, ...(following.get(name) ?? []));
  }
  return all;
}

/// Where a thing of the matrix stands between the two versions.
export type Status = 'same' | 'added' | 'removed' | 'changed';

/// A column: a cell.
export interface CellColumn {
  id: string;
  /// The cell in the version shown; none when only the other version has it.
  cell?: Cell;
  /// The cell in the other version, when comparing.
  other?: Cell;
  /// From the version compared against to the one it is compared with.
  status: Status;
  /// The fields that changed, by the kind of their change (`title`, `condition`, `runsOnAdded`...).
  changed: Set<Change['kind']>;
}

/// A row: a block.
export interface BlockRow {
  /// What the row is called: a trigger's name, or the block's where each cell has its own trigger of one name; a
  /// datum's name.
  name: string;
  /// Of a trigger's row, the trigger's name; none of a datum's.
  trigger?: string;
  /// Of a trigger's row, what the trigger is, as its file writes it.
  what?: string;
  /// The block the row is about: the one the trigger stands on, or the datum.
  on?: string;
  block?: Block;
  other?: Block;
  status: Status;
  changed: Set<Change['kind']>;
}

/// How a row's line runs through one crossing: from the crossing's middle to the right (towards later cells), to the
/// left, both, or not at all.
export type Segment = 'none' | 'right' | 'left' | 'through';

/// One crossing of a row and a column.
export interface Spot {
  /// What the cell does with the block in the version shown.
  mark: Mark;
  /// Of the marks of either version, those only the version compared with has: put in by the change.
  added: Mark;
  /// Those only the version compared against has: taken out by the change.
  removed: Mark;
  /// The row's line, which the eye follows along the cells: from the first mark that no way back reaches to the
  /// last mark. A mark left of every write that a way back reaches is reached by the way back alone, so the line
  /// starts at the first cell that writes the block, the way the block goes on from there; where no cell writes it, at
  /// the first mark.
  line: Segment;
  /// The way back: from a later cell's write leftwards to the cells before it that the same block runs, or whose
  /// condition reads it. A block a cell is only given does not run it, so no way back reaches it.
  back: Segment;
  /// A cell that a write right of it sends the work back to.
  backTo: boolean;
}

/// Where a tool's row crosses a cell's column: whether the cell may ask for the tool in the version shown, and
/// whether the change put that in or took it out.
export interface ToolSpot {
  asks: boolean;
  added: boolean;
  removed: boolean;
}

/// A row of a tool of the engine's, which an agent's cell may ask for in its return.
export interface ToolRow {
  name: string;
  /// By cell, in the order of the columns.
  spots: ToolSpot[];
}

/// The tools of the engine's, in the order their rows stand; a tool a definition names that is not among them stands
/// after them.
export const TOOLS = ['query', 'run_tests'];

export type Version = 'after' | 'before';

/// The matrix of one version, holding the cells and blocks of the other too when comparing.
export interface Matrix {
  version: Version;
  /// The rows, top to bottom.
  blocks: BlockRow[];
  /// The columns, left to right: the order of the walk.
  cells: CellColumn[];
  /// By block, then by cell.
  spots: Spot[][];
  /// The rows of the tools that a cell of either version may ask for, over the blocks' rows.
  tools: ToolRow[];
  /// How many ways back the version shown has: pairs of a cell a block runs, or whose condition reads it, and a later
  /// cell's write of it.
  waysBack: number;
}

/// The two versions to draw: the one shown and, when comparing, the one compared against with the changes between.
export interface Versions {
  after: Flow;
  before?: Flow;
  changes?: Change[];
}

/// The things a change is about.
export function changeCell(change: Change): string | undefined {
  return 'cell' in change ? change.cell : undefined;
}

export function changeBlock(change: Change): string | undefined {
  return 'block' in change && !('cell' in change) ? change.block : undefined;
}

/// The matrix of `version` (the one shown, `after`, or the one compared against, `before`).
export function matrix(versions: Versions, version: Version): Matrix {
  const { after, before, changes = [] } = versions;
  const shown = version === 'before' && before ? before : after;
  const other = before ? (version === 'before' ? after : before) : undefined;

  const moved = new Set(changes.flatMap((c) => (c.kind === 'cellMoved' ? [c.cell] : [])));
  const cellIds = merged(
    shown.cells.map((c) => c.id),
    other?.cells.map((c) => c.id) ?? [],
    moved,
  );
  const blockNames = merged(blockOrder(shown), other ? blockOrder(other) : []);

  const cellChanges = new Map<string, Set<Change['kind']>>();
  const blockChanges = new Map<string, Set<Change['kind']>>();
  for (const change of changes) {
    const cell = changeCell(change);
    const block = changeBlock(change);
    if (cell !== undefined) cellChanges.set(cell, (cellChanges.get(cell) ?? new Set()).add(change.kind));
    if (block !== undefined) blockChanges.set(block, (blockChanges.get(block) ?? new Set()).add(change.kind));
  }
  const status = <T>(inAfter: T | undefined, inBefore: T | undefined, changed: Set<Change['kind']>): Status => {
    if (!before) return 'same';
    if (inAfter && !inBefore) return 'added';
    if (inBefore && !inAfter) return 'removed';
    return changed.size > 0 ? 'changed' : 'same';
  };

  const cellIn = (flow: Flow | undefined, id: string) => flow?.cells.find((c) => c.id === id);
  const blockIn = (flow: Flow | undefined, name: string) => flow?.blocks.find((b) => b.name === name);
  const cells: CellColumn[] = cellIds.map((id) => {
    const changed = cellChanges.get(id) ?? new Set();
    return {
      id,
      cell: cellIn(shown, id),
      other: cellIn(other, id),
      status: status(cellIn(after, id), cellIn(before, id), changed),
      changed,
    };
  });
  // The triggers' rows first, each a trigger a cell names, by the block it stands on, in the order of those blocks;
  // then the data's rows. A trigger stands for one event: one each cell has its own of is a row for each cell.
  const order = blockOrder(shown);
  const pairs: { name: string; block: string; what?: string }[] = [];
  for (const cell of shown.cells) {
    for (const t of cell.triggers ?? []) {
      if (!pairs.some((p) => p.name === t.name && p.block === t.block)) {
        pairs.push({ name: t.name, block: t.block, what: t.what ?? undefined });
      }
    }
  }
  pairs.sort((x, y) => order.indexOf(x.block) - order.indexOf(y.block));
  const blocksOf = (name: string) => new Set(pairs.filter((p) => p.name === name).map((p) => p.block)).size;
  const row = (name: string, on: string, trigger?: string, what?: string): BlockRow => ({
    name,
    trigger,
    what,
    on,
    block: blockIn(shown, on),
    other: blockIn(other, on),
    status: 'same',
    changed: blockChanges.get(on) ?? new Set(),
  });
  const blocks: BlockRow[] = [
    ...pairs.map((p) => row(blocksOf(p.name) > 1 ? p.block : p.name, p.block, p.name, p.what)),
    ...blockNames.filter((name) => !blockIn(shown, name)?.trigger).map((name) => row(name, name)),
  ];

  // Of a trigger's row: whether the cell runs on the trigger, and whether it writes the block the trigger stands on.
  // Of a datum's row: whether the cell is given it and in which kind, writes it, or reads its value in a condition.
  const markIn = (cell: Cell | undefined, r: BlockRow): Mark => {
    if (!cell) return NO_MARK;
    const on = r.on ?? r.name;
    if (r.trigger !== undefined) {
      return {
        ...NO_MARK,
        runs: (cell.triggers ?? []).some((t) => t.name === r.trigger && t.block === on),
        write: cell.outputs.includes(on),
      };
    }
    const mark = markOf(cell, on);
    return {
      ...mark,
      runs: false,
      kind: mark.given ? (cell.input[on]?.given?.kind ?? null) : null,
    };
  };
  const spots: Spot[][] = blocks.map((r) =>
    cells.map((column) => ({
      mark: column.cell ? markIn(column.cell, r) : NO_MARK,
      added: NO_MARK,
      removed: NO_MARK,
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
    const lastWrite = writes[writes.length - 1];
    const reached = reads.filter((c) => lastWrite !== undefined && c < lastWrite);
    paint(
      row,
      'line',
      marked.find((c) => !reached.includes(c) || marks[c].write),
      marked[marked.length - 1],
    );
    for (const c of reached) row[c].backTo = true;
    waysBack += reads.reduce((n, c) => n + writes.filter((w) => w > c).length, 0);
    if (reached.length > 0) {
      const first = reached[0];
      paint(row, 'back', first, Math.max(...writes.filter((w) => w > first)));
    }
  });

  const asks = (flow: Flow | undefined, id: string, tool: string) => cellIn(flow, id)?.tools.includes(tool) ?? false;
  const named = [...new Set([after, ...(before ? [before] : [])].flatMap((f) => f.cells.flatMap((c) => c.tools)))];
  const place = (tool: string) => (TOOLS.includes(tool) ? TOOLS.indexOf(tool) : TOOLS.length);
  const tools: ToolRow[] = named
    .sort((a, b) => place(a) - place(b) || (a < b ? -1 : a > b ? 1 : 0))
    .map((name) => ({
      name,
      spots: cells.map((column) => {
        const a = asks(after, column.id, name);
        const b = before ? asks(before, column.id, name) : a;
        return {
          asks: asks(shown, column.id, name),
          added: a && !b,
          removed: b && !a,
        };
      }),
    }));

  return { version, blocks, cells, spots, tools, waysBack };
}

/// Paints a line along a row from the middle of the crossing at `from` to the middle of the one at `to`.
function paint(row: Spot[], which: 'line' | 'back', from?: number, to?: number): void {
  if (from === undefined || to === undefined || from >= to) return;
  for (let c = from; c <= to; c += 1) {
    row[c][which] = c === from ? 'right' : c === to ? 'left' : 'through';
  }
}

/// Where a block another cell writes or reads stands from a cell: above it, below it, or the cell itself.
export type Where = 'above' | 'below' | 'itself';

export function where(flow: Flow, from: string, to: string): Where {
  const ids = flow.cells.map((c) => c.id);
  const a = ids.indexOf(from);
  const b = ids.indexOf(to);
  return b === a ? 'itself' : b < a ? 'above' : 'below';
}
