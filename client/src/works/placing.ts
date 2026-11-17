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
import { STATE, isWayRound, stateOf } from './words';

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

/// What the list of works can be ordered by, each a column of it: the item's number, its title, the work's state, the
/// level its rounds run at and the level of its cell, what it cost, how long its rounds were under way, and when
/// something last happened to it.
export type OrderBy = 'item' | 'title' | 'state' | 'level' | 'cellLevel' | 'cost' | 'time' | 'last';

/// Which way an order runs: its smallest value first, or its largest.
export type Way = 'ascending' | 'descending';

/// Which way a column orders the list at the first press: the item's number, what was spent and when, the largest
/// first, the levels the highest first; the title from A to Z, and the states in their order (`stateRank`).
export const FIRST_WAY: Record<OrderBy, Way> = {
  item: 'descending',
  title: 'ascending',
  state: 'ascending',
  level: 'descending',
  cellLevel: 'descending',
  cost: 'descending',
  time: 'descending',
  last: 'descending',
};

/// An order of the list: by a column, the way its first press gives, or turned by a second.
export interface Order {
  by: OrderBy;
  turned: boolean;
}

/// The list's order until the owner picks another: what happened last first.
export const LAST_FIRST: Order = { by: 'last', turned: false };

/// The way an order runs.
export const wayOf = (order: Order): Way =>
  (FIRST_WAY[order.by] === 'ascending') !== order.turned ? 'ascending' : 'descending';

/// Where a work's state stands in the order of states: waiting for the owner, halted, running, waiting for another,
/// paused, merged, cancelled.
function stateRank(work: Summary): number {
  const { tone, word } = stateOf(work);
  if (tone === 'owner') return 0;
  if (tone === 'failed') return 1;
  if (tone === 'running') return 2;
  if (tone === 'done') return 5;
  if (word === STATE.cancelled) return 6;
  if (word === STATE.paused) return 4;
  return 3;
}

/// A level as the engine orders the levels: by number where it is one, by name otherwise, a name below every number;
/// none where the engine gives none.
function levelOf(level: string | null | undefined): number | string | null {
  if (level === null || level === undefined) return null;
  return /^\d+$/.test(level) ? Number(level) : level;
}

/// What a work is ordered by in a column: none where it has no value there, a work with no level; a work to which
/// nothing has happened yet is the oldest.
function keyOf({ item, work }: ListedWork, by: OrderBy): number | string | null {
  switch (by) {
    case 'item':
      return item.number;
    case 'title':
      return item.title;
    case 'state':
      return stateRank(work);
    case 'level':
      return levelOf(work.level);
    case 'cellLevel':
      return levelOf(work.cellLevel);
    case 'cost':
      return work.spent.usdMicros;
    case 'time':
      return work.spent.workingMs;
    case 'last':
      return work.last ?? 0;
  }
}

/// Two keys, the smaller first: numbers by their size, words alphabetically, a word below a number.
function compareKeys(a: number | string, b: number | string): number {
  if (typeof a === 'number' && typeof b === 'number') return a - b;
  if (typeof a === 'string' && typeof b === 'string') return a.localeCompare(b);
  return typeof a === 'string' ? -1 : 1;
}

/// The work of every item, in this order: works with no value in its column last, whichever way it runs, and works of
/// one value as the list's first order has them, what happened last first. Nothing of a work's state orders it but
/// the column of states.
export function orderWorks(items: Item[], order: Order = LAST_FIRST): ListedWork[] {
  const sign = wayOf(order) === 'ascending' ? 1 : -1;
  const latest = (a: ListedWork, b: ListedWork) =>
    (b.work.last ?? 0) - (a.work.last ?? 0) || b.item.number - a.item.number;
  return items
    .map((item) => ({ item, work: item.work }))
    .sort((a, b) => {
      const [x, y] = [keyOf(a, order.by), keyOf(b, order.by)];
      if (x === null || y === null) return x === y ? latest(a, b) : x === null ? 1 : -1;
      return sign * compareKeys(x, y) || latest(a, b);
    });
}
