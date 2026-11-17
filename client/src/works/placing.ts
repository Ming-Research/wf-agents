// A work's history as the chart draws it, and the order of the list of works, worked out from the interface's JSON
// alone so that they can be tested without drawing.
//
// The chart's columns are the cells of the work's flow in the order of the walk, and its rows the steps in the order
// they happened. A run stands in its cell's column, in the row of its start. What is not a run (a write a return did
// not hand in, by the owner, a cell's question or the outside; a let-go, a pause, a let-on, a cancel) is a row of its
// own: its words at the row's left, and, for a write, a mark in the column of each cell that takes the block, the
// flow's chart's for that cell; for a let-go, a mark in the column of the cell let go. A run that is a way round is
// drawn as the flow's views draw a way back: a line in the accent from the columns of the cells after it whose writes
// it ran for, each marked, to the run, with an arrow there. A run of a cell the flow no longer has keeps its row, in no
// column.

import type { Flow } from '../api';
import type { Block, Item, Run, Step, Summary } from '../works-api';
import { isWayRound } from './words';

/// What a row marks in a cell's column besides a run: the cell reads the block written, its condition reads it, or it
/// is the cell let go on.
export type RowMark = 'read' | 'condition' | 'letGo';

export interface HistoryRow {
  /// Unique among the rows: the step's kind and place.
  key: string;
  /// The step's place among the steps.
  index: number;
  step: Step;
  /// The column of the run's cell; none for a row of its own, or a run of a cell the flow no longer has.
  column: number | null;
  /// The marks in the cells' columns of a row of its own, by column.
  marks: Map<number, RowMark>;
  /// Whether the row's day differs from the row's before, so that the day is shown again.
  newDay: boolean;
  /// For a run that is a way round, the columns of the cells after it whose writes it ran for, in order; empty where
  /// none of them can be found.
  backFrom: number[];
}

const dayKey = (ms: number) => {
  const d = new Date(ms);
  return `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;
};

/// The cell whose write gave a block the meaning it had at the version of this number, where a run wrote it.
function meaningWriter(versions: Block[], block: string, number: number | null): string | null {
  const all = versions.find((b) => b.name === block)?.versions ?? [];
  const version = all.find((v) => v.number === number);
  const by = all.find((v) => v.number === version?.meaning)?.by;
  return by?.kind === 'run' ? by.cell : null;
}

/// The rows of the history, one for each step, in the order the steps happened.
export function placeSteps(steps: Step[], flow: Flow, versions: Block[] = []): HistoryRow[] {
  const column = new Map(flow.cells.map((c, i) => [c.id, i]));
  const blocks = new Map(flow.blocks.map((b) => [b.name, b]));
  return steps.map((step, index) => {
    const marks = new Map<number, RowMark>();
    let col: number | null = null;
    let backFrom: number[] = [];
    if (step.kind === 'run') {
      col = column.get(step.cell) ?? null;
      if (col !== null && isWayRound(step)) {
        const from = step.because.flatMap((cause) => {
          if (cause.kind !== 'changed') return [];
          const started = step.basis.find((b) => b.block === cause.block)?.version ?? null;
          const writer = meaningWriter(versions, cause.block, started);
          const c = writer === null ? undefined : column.get(writer);
          return c === undefined ? [] : [c];
        });
        const run = col;
        backFrom = [...new Set(from)].filter((c) => c > run).sort((a, b) => a - b);
      }
    } else if (step.kind === 'wrote') {
      const block = blocks.get(step.block);
      for (const id of block?.readBy.conditions ?? []) {
        const c = column.get(id);
        if (c !== undefined) marks.set(c, 'condition');
      }
      for (const id of block?.readBy.cells ?? []) {
        const c = column.get(id);
        if (c !== undefined) marks.set(c, 'read');
      }
    } else if (step.kind === 'letGo') {
      const c = column.get(step.cell);
      if (c !== undefined) marks.set(c, 'letGo');
    }
    const before = steps[index - 1];
    return {
      key: `${step.kind}:${step.at}`,
      index,
      step,
      column: col,
      marks,
      newDay: !before || dayKey(before.time) !== dayKey(step.time),
      backFrom,
    };
  });
}

/// The runs among the steps.
export function runsOf(steps: Step[]): (Step & { kind: 'run' })[] {
  return steps.filter((s): s is Step & { kind: 'run' } => s.kind === 'run');
}

/// The run of this number, where there is one.
export function findRun(steps: Step[], run: number): Run | undefined {
  return runsOf(steps).find((r) => r.run === run);
}

export interface ListedWork {
  item: Item;
  work: Summary;
}

/// What the list of works can be ordered by, each a column of it: the item's number, what the work cost, how long its
/// rounds were under way, and when something last happened to it.
export type OrderBy = 'item' | 'cost' | 'time' | 'last';

/// An order of the list: by a column, the largest first (`down`) or the smallest (`up`).
export interface Order {
  by: OrderBy;
  way: 'down' | 'up';
}

/// The list's order until the owner picks another: what happened last first.
export const LAST_FIRST: Order = { by: 'last', way: 'down' };

/// The value a work is ordered by; a work to which nothing has happened yet is the oldest.
function valueOf({ item, work }: ListedWork, by: OrderBy): number {
  switch (by) {
    case 'item':
      return item.number;
    case 'cost':
      return work.spent.usdMicros;
    case 'time':
      return work.spent.workingMs;
    case 'last':
      return work.last ?? 0;
  }
}

/// The work of every item, in this order, works of one value by their items' numbers in the same direction. Nothing of
/// a work's state orders it.
export function orderWorks(items: Item[], order: Order = LAST_FIRST): ListedWork[] {
  const sign = order.way === 'down' ? -1 : 1;
  return items
    .map((item) => ({ item, work: item.work }))
    .sort((a, b) => sign * (valueOf(a, order.by) - valueOf(b, order.by) || a.item.number - b.item.number));
}
