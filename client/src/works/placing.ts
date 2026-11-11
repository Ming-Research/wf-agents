// A work's history as the chart draws it, and the order of the list of works, worked out from the interface's JSON
// alone so that they can be tested without drawing.
//
// The chart's columns are the cells of the work's flow in the order of the walk, and its rows the steps in the order
// they happened. A run stands in its cell's column, in the row of its start. What is not a run (a write by a person or
// the outside, a let-go, a pause, a let-on, a cancel) is a row of its own: its words at the row's left, and, for a
// write, a ring in the column of each cell that reads the block, as the flow's chart marks a read; for a let-go, a mark
// in the column of the cell let go. A run that is a way round is drawn as the flow's chart draws a way back: a line in
// the way back's purple from the columns of the cells after it whose writes it ran for, each marked, to the run, with an
// arrow there. A run of a cell the flow no longer has keeps its row, in no column.

import type { Flow } from '../api';
import type { Block, Item, Run, Step, Summary } from '../works-api';
import { isWayRound, waitsForOwner } from './words';

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

/// The cell whose write gave a block the meaning it had at this version, where a run wrote it.
function meaningWriter(versions: Block[], block: string, at: number | null): string | null {
  const all = versions.find((b) => b.name === block)?.versions ?? [];
  const version = all.find((v) => v.at === at);
  const by = all.find((v) => v.at === version?.meaning)?.by;
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

/// Where a work stands in the list: what waits for the owner first, then a fault or a pause of the engine's, as for a
/// round that failed, then what goes on by itself, then what the owner paused, then what has ended.
export function rank(summary: Summary): number {
  const now = summary.now;
  if (now.kind === 'waits' && now.on.some(waitsForOwner)) return 0;
  if (now.kind === 'fault' || (now.kind === 'paused' && now.by === 'engine')) return 1;
  if (now.kind === 'merged' || now.kind === 'cancelled') return 4;
  if (summary.paused || now.kind === 'paused' || now.kind === 'everything') return 3;
  return 2;
}

export interface ListedWork {
  item: Item;
  work: Summary;
}

/// The work of every item, in the list's order: by rank, and within a rank what happened last first.
export function orderWorks(items: Item[]): ListedWork[] {
  return items
    .map((item) => ({ item, work: item.work }))
    .sort((a, b) => rank(a.work) - rank(b.work) || (b.work.last ?? 0) - (a.work.last ?? 0) || a.work.number - b.work.number);
}
