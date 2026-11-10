// The chart as a matrix: the cells as rows in the order of the walk, the blocks as columns, and at each crossing what
// the cell does with the block. Everything the chart draws is worked out here, from the interface's JSON alone, so
// that it can be tested without drawing: the order of the columns, the marks, the ways back and what changed.
//
// When there is a definition to compare against, the matrix holds the cells and blocks of both: the one whose version
// is shown in its order, each that only the other has standing after the one it followed there (of those that did not
// move), so that switching between the two versions keeps every row and column that both have where the owner looks.

import type { Block, Cell, Change, Flow } from './api';

/// What a cell does with a block: reads it as an input, may write it as an output, or reads its value in its
/// condition.
export interface Mark {
  read: boolean;
  write: boolean;
  condition: boolean;
}

export const NO_MARK: Mark = { read: false, write: false, condition: false };

export const MARK_PARTS = ['read', 'write', 'condition'] as const;
export type MarkPart = (typeof MARK_PARTS)[number];

/// The mark of a cell on a block.
export function markOf(cell: Cell | undefined, block: string): Mark {
  if (!cell) return NO_MARK;
  return {
    read: cell.inputs.includes(block),
    write: cell.outputs.includes(block),
    condition: cell.condition?.on === 'block' && cell.condition.block === block,
  };
}

export function hasMark(mark: Mark): boolean {
  return mark.read || mark.write || mark.condition;
}

/// The order of the blocks' columns. A block stands where it is first written in the walk, a cell's outputs together
/// in the order the cell lists them; a block no cell writes (what people or the outside write) stands just before the
/// first cell that reads it, its inputs in their order, a condition's block first; a block no cell writes or reads
/// stands last. Ties go by name. The matrix then goes down to the right like a staircase, each cell's writes beside
/// each other and next to the reads that follow from them, and a write below a read stands out as a way back.
export function blockOrder(flow: Flow): string[] {
  const place = new Map(flow.cells.map((c, i) => [c.id, i]));
  const key = (block: Block): [number, number, number] => {
    const writer = block.writtenBy.cells[0];
    if (writer !== undefined) {
      const cell = flow.cells[place.get(writer) ?? 0];
      return [place.get(writer) ?? 0, 1, cell.outputs.indexOf(block.name)];
    }
    const readers = [...block.readBy.cells, ...block.readBy.conditions]
      .map((id) => place.get(id) ?? Infinity)
      .sort((a, b) => a - b);
    const first = readers[0];
    if (first !== undefined && first !== Infinity) {
      const cell = flow.cells[first];
      const isCondition = cell.condition?.on === 'block' && cell.condition.block === block.name;
      return [first, 0, isCondition ? -1 : cell.inputs.indexOf(block.name)];
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

/// A row: a cell.
export interface Row {
  id: string;
  /// The cell in the version shown; none when only the other version has it.
  cell?: Cell;
  /// The cell in the other version, when comparing.
  other?: Cell;
  /// From the version compared against to the one it is compared with.
  status: Status;
  /// The fields that changed, by the kind of their change (`title`, `condition`, `inputAdded`...).
  changed: Set<Change['kind']>;
}

/// A column: a block.
export interface Column {
  name: string;
  block?: Block;
  other?: Block;
  status: Status;
  changed: Set<Change['kind']>;
}

/// How a column's line runs through one crossing: from the crossing's middle down, up, both, or not at all.
export type Segment = 'none' | 'down' | 'up' | 'through';

/// One crossing of a row and a column.
export interface Spot {
  /// What the cell does with the block in the version shown.
  mark: Mark;
  /// Of the marks of either version, those only the version compared with has: put in by the change.
  added: Mark;
  /// Those only the version compared against has: taken out by the change.
  removed: Mark;
  /// The column's line, from its first mark to its last, which the eye follows down the rows.
  line: Segment;
  /// The way back: from a write up to the reads above it of the same block, by other cells.
  back: Segment;
  /// A read that a write below it sends the work back to.
  backTo: boolean;
}

export type Version = 'after' | 'before';

/// The matrix of one version, holding the cells and blocks of the other too when comparing.
export interface Matrix {
  version: Version;
  rows: Row[];
  columns: Column[];
  /// By row, then by column.
  spots: Spot[][];
  /// How many ways back the version shown has: pairs of a block's write and a read above it.
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
  const rowIds = merged(
    shown.cells.map((c) => c.id),
    other?.cells.map((c) => c.id) ?? [],
    moved,
  );
  const columnNames = merged(blockOrder(shown), other ? blockOrder(other) : []);

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
  const rows: Row[] = rowIds.map((id) => {
    const changed = cellChanges.get(id) ?? new Set();
    return {
      id,
      cell: cellIn(shown, id),
      other: cellIn(other, id),
      status: status(cellIn(after, id), cellIn(before, id), changed),
      changed,
    };
  });
  const columns: Column[] = columnNames.map((name) => {
    const changed = blockChanges.get(name) ?? new Set();
    return {
      name,
      block: blockIn(shown, name),
      other: blockIn(other, name),
      status: status(blockIn(after, name), blockIn(before, name), changed),
      changed,
    };
  });

  const spots: Spot[][] = rows.map((row) =>
    columns.map((column) => {
      const mark = row.cell && column.block ? markOf(row.cell, column.name) : NO_MARK;
      const a = markOf(cellIn(after, row.id), column.name);
      const b = before ? markOf(cellIn(before, row.id), column.name) : a;
      const only = (x: Mark, y: Mark): Mark => ({
        read: x.read && !y.read,
        write: x.write && !y.write,
        condition: x.condition && !y.condition,
      });
      return { mark, added: only(a, b), removed: only(b, a), line: 'none', back: 'none', backTo: false };
    }),
  );

  let waysBack = 0;
  columns.forEach((_, c) => {
    const marks = spots.map((row) => row[c].mark);
    const marked = marks.flatMap((m, r) => (hasMark(m) ? [r] : []));
    const reads = marks.flatMap((m, r) => (m.read || m.condition ? [r] : []));
    const writes = marks.flatMap((m, r) => (m.write ? [r] : []));
    paint(spots, c, 'line', marked[0], marked[marked.length - 1]);
    const lowestWrite = writes[writes.length - 1];
    const reached = reads.filter((r) => lowestWrite !== undefined && r < lowestWrite);
    for (const r of reached) spots[r][c].backTo = true;
    waysBack += reads.reduce((n, r) => n + writes.filter((w) => w > r).length, 0);
    if (reached.length > 0) {
      const top = reached[0];
      const bottom = Math.max(...writes.filter((w) => w > top));
      paint(spots, c, 'back', top, bottom);
    }
  });

  return { version, rows, columns, spots, waysBack };
}

/// Paints a line in a column from the middle of row `top` to the middle of row `bottom`.
function paint(spots: Spot[][], column: number, which: 'line' | 'back', top?: number, bottom?: number): void {
  if (top === undefined || bottom === undefined || top >= bottom) return;
  for (let r = top; r <= bottom; r += 1) {
    spots[r][column][which] = r === top ? 'down' : r === bottom ? 'up' : 'through';
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
