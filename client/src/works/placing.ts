// A work's history as the chart draws it, and the order of the list of works, worked out from the interface's JSON
// alone so that they can be tested without drawing.
//
// The chart's columns are the cells of the work's flow in the order of the walk, and its rows the steps the owner reads
// the work by, in the order they happened, each with one line of what it did. A run is a row, its rounds folded into it,
// with its mark in its cell's column. An act of the owner's (a write: an answer, words to a line, a block the owner
// writes; a let-go, a pause, a let-on, a cancel, a reopening) is a row, the owner's sign under each cell it ran, a
// let-go's under the cell let go, an act that ran no cell yet with no sign. A write of the outside's is a row only where
// it ran a cell, with the outside's sign under each; one that ran none is the engine's own bookkeeping. A pause of the
// engine's is a row, since a stop matters to the owner. A write of a run is no row: it is that run's, said in its row.
// A new session of a line is no row: it is said with the run that began it. A run of a cell the flow no longer has
// keeps its row, in no column. A run that is a way round is drawn as the flow's chart draws a way back: a line along
// its row from the furthest cell after it whose write it ran for, to its mark, with an arrow there.
//
// A write ran a cell where a run of the cell is for a change of the block whose meaning that write gave: the records
// name a run's cause by its block, and the run's basis names the version it started from, whose meaning names the
// version that last changed it.

import type { Block, Item, Run, Step, Summary, WorkPage } from '../works-api';
import { type RunMark } from './state-marks';
import { STATE, isWayRound, plain, sentence, stateOf, stoppedAt } from './words';

/// A piece of a row's line: the page's own words, or a name or a value as the files write it, drawn in monospace.
export type Part = string | { name: string };

/// A line of pieces as one text, for the pointer's tip.
export const partsText = (parts: Part[]): string => parts.map((p) => (typeof p === 'string' ? p : p.name)).join('');

/// The mark a row has under a cell: a run's, how it ended or stands; the owner's sign, of an act that ran the cell or
/// let it go on; the outside's sign, of a write that ran the cell.
export type RowMark = { kind: 'run'; mark: RunMark; run: number } | { kind: 'owner' } | { kind: 'outside' };

/// A new session of a line, as a step.
export type SessionStep = Step & { kind: 'session' };

export interface HistoryRow {
  /// Unique among the rows: the step's kind and place.
  key: string;
  /// The step's place among the steps.
  index: number;
  step: Step;
  /// The marks under the cells' columns, by column.
  marks: Map<number, RowMark>;
  /// Whether the row's day differs from the day of the row before, so that the day is shown again.
  newDay: boolean;
  /// What the step did, in one line.
  did: Part[];
  /// Of a write, the runs it ran, in the order they began.
  ran: Run[];
  /// Of a run that is a way round, the column of its cell and those of the cells after it whose writes it ran for, in
  /// order, along which the way back is drawn; none otherwise, or where none of them can be found.
  back: { to: number; from: number[] } | null;
}

const dayKey = (ms: number) => {
  const d = new Date(ms);
  return `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;
};

/// The runs each write of the owner's or the outside's ran, by the write's place: each run one of whose causes is a
/// change of a block whose version it started from has, as its meaning, the version that write wrote.
export function ranBy(steps: Step[], blocks: Block[]): Map<number, Run[]> {
  const versions = new Map(blocks.map((b) => [b.name, b.versions]));
  const ran = new Map<number, Run[]>();
  for (const step of steps) {
    if (step.kind !== 'run') continue;
    const places = new Set<number>();
    for (const cause of step.because) {
      if (cause.kind !== 'changed') continue;
      const all = versions.get(cause.block) ?? [];
      const started = step.basis.find((b) => b.block === cause.block)?.version ?? null;
      const meaning = all.find((v) => v.number === started)?.meaning ?? null;
      const wrote = all.find((v) => v.number === meaning);
      if (wrote !== undefined && wrote.by.kind !== 'run') places.add(wrote.at);
    }
    for (const at of places) ran.set(at, [...(ran.get(at) ?? []), step]);
  }
  return ran;
}

/// Of a run that is a way round, the column of its cell and those of the cells after it whose writes it ran for: of each
/// block it ran for, the cell whose run wrote the version that gave the meaning of the version it started from.
function backOf(run: Run, column: Map<string, number>, blocks: Block[]): HistoryRow['back'] {
  const to = column.get(run.cell);
  if (to === undefined || !isWayRound(run)) return null;
  const versions = new Map(blocks.map((b) => [b.name, b.versions]));
  const from = run.because.flatMap((cause) => {
    if (cause.kind !== 'changed') return [];
    const all = versions.get(cause.block) ?? [];
    const started = run.basis.find((b) => b.block === cause.block)?.version ?? null;
    const by = all.find((v) => v.number === all.find((x) => x.number === started)?.meaning)?.by;
    const c = by?.kind === 'run' ? column.get(by.cell) : undefined;
    return c !== undefined && c > to ? [c] : [];
  });
  return from.length === 0 ? null : { to, from: [...new Set(from)].sort((a, b) => a - b) };
}

/// The new sessions each run began, by the run's number. A session with an id of its own is begun by the run whose
/// round is the first of its line in it, the line's sessions with ids of their own up to it numbering it; one begun again
/// under its id, or compacted, by the run whose round of its line was under way as it was recorded, the latest to begin
/// by then. A session whose first round has not begun is with no run yet.
export function sessionsBegun(steps: Step[]): Map<number, SessionStep[]> {
  const runs = steps.filter((s): s is Step & { kind: 'run' } => s.kind === 'run');
  const rounds = runs.flatMap((run) => run.rounds.map((round) => ({ run: run.run, round })));
  const begun = new Map<number, SessionStep[]>();
  const counted = new Map<string, number>();
  for (const step of steps) {
    if (step.kind !== 'session') continue;
    const onLine = rounds.filter((r) => r.round.session?.line === step.line);
    let by: number | undefined;
    if (step.why.kind === 'lost' || step.why.kind === 'compacted') {
      by = onLine.filter((r) => r.round.began <= step.time).sort((a, b) => b.round.began - a.round.began)[0]?.run;
    } else {
      const number = (counted.get(step.line) ?? 0) + 1;
      counted.set(step.line, number);
      by = onLine.find((r) => r.round.session?.number === number)?.run;
    }
    if (by !== undefined) begun.set(by, [...(begun.get(by) ?? []), step]);
  }
  return begun;
}

/// How a run ended or stands, as the strip says it of its cell where the run is the cell's open one: it finished, and a
/// version it wrote as it did changed a block's meaning, or none did; it was abandoned; the work is stopped at its cell;
/// a round of it runs, or a call or an act is carried out for it; the round it handed in last was sent back; or else it
/// is under way.
export function runMark(run: Run, summary: Pick<Summary, 'now' | 'cells' | 'waits'>, blocks: Block[]): RunMark {
  const end = run.end;
  if (end?.kind === 'abandoned') return 'abandoned';
  if (end?.kind === 'finished') {
    const changed = blocks.some((b) => b.versions.some((v) => v.at === end.at && v.meaning === v.number));
    return changed ? 'handedIn' : 'nothingNew';
  }
  if (stoppedAt(summary) === run.cell) return 'stopped';
  const now = summary.cells.find((c) => c.cell === run.cell && c.open === run.run)?.state;
  if (now === 'running' || now === 'calling') return now;
  return run.rounds.at(-1)?.back?.kind === 'handedIn' ? 'sentBack' : 'underWay';
}

/// A block as a row names it: with the value its version has, where it has one (`plan_ok = no`), else with its
/// versions (`plan v2`).
const named = (block: string, value: string | null, numbers: number[]): string =>
  value !== null ? `${block} = ${value}` : `${block} ${numbers.map((n) => `v${n}`).join(' ')}`;

/// Names joined by a middle dot, each a name.
const dotted = (names: string[]): Part[] => names.flatMap((name, i) => (i === 0 ? [{ name }] : [' · ', { name }]));

/// What a run wrote, each block once, in the order it first wrote them and then the flow's: with the value its last
/// version has, or its versions.
function wroteParts(run: Run, page: Pick<WorkPage, 'blocks' | 'flow'>): Part[] {
  const order = new Map(page.flow.blocks.map((b, i) => [b.name, i]));
  const written = page.blocks.flatMap((b) => {
    const mine = b.versions.filter((v) => v.by.kind === 'run' && v.by.run === run.run).sort((x, y) => x.at - y.at);
    return mine.length === 0 ? [] : [{ block: b.name, mine }];
  });
  written.sort((a, b) => a.mine[0].at - b.mine[0].at || (order.get(a.block) ?? 0) - (order.get(b.block) ?? 0));
  return dotted(written.map((w) => named(w.block, w.mine[w.mine.length - 1].value, w.mine.map((v) => v.number))));
}

/// The question each answer of the owner's was written for, by the place of the write: the owner's latest answer to
/// each question that has one, which the engine writes into the questions of the question's cell as it takes it, the
/// first write after it. An earlier answer the page is not given, and its write is named as any other.
function answers(page: Pick<WorkPage, 'steps' | 'questions'>): Map<number, { question: string; option: number }> {
  const out = new Map<number, { question: string; option: number }>();
  for (const q of page.questions ?? []) {
    const answer = q.answer;
    if (answer === null) continue;
    const write = page.steps.find((s) => s.kind === 'wrote' && s.at > answer.at);
    if (write !== undefined) out.set(write.at, { question: q.name, option: answer.option });
  }
  return out;
}

/// What a write of the owner's or the outside's did: answered a question, by its name and the option taken; said the
/// owner's words to the cell they reached; or wrote the block, with the value it wrote or its version.
function writeParts(
  step: Step & { kind: 'wrote' },
  page: Pick<WorkPage, 'blocks'>,
  answered: Map<number, { question: string; option: number }>,
): Part[] {
  const version = page.blocks.find((b) => b.name === step.block)?.versions.find((v) => v.at === step.at);
  const answer = answered.get(step.at);
  const saidTo = version?.saidTo ?? null;
  if (step.by.kind === 'owner' && answer !== undefined)
    return ['answered ', { name: answer.question }, ': ', { name: String(answer.option) }];
  if (step.by.kind === 'owner' && saidTo !== null) return ['said to ', { name: saidTo }];
  return ['wrote ', { name: named(step.block, step.value, version === undefined ? [] : [version.number]) }];
}

/// The rows of the history, in the order the steps happened.
export function placeSteps(page: Pick<WorkPage, 'steps' | 'flow' | 'blocks' | 'summary' | 'questions'>): HistoryRow[] {
  const { steps, flow, blocks } = page;
  const column = new Map(flow.cells.map((c, i) => [c.id, i]));
  const ran = ranBy(steps, blocks);
  const answered = answers(page);
  const rows: HistoryRow[] = [];
  let day: string | null = null;
  steps.forEach((step, index) => {
    const marks = new Map<number, RowMark>();
    const mark = (cell: string, m: RowMark) => {
      const c = column.get(cell);
      if (c !== undefined) marks.set(c, m);
    };
    let did: Part[] = [];
    let runs: Run[] = [];
    let back: HistoryRow['back'] = null;
    switch (step.kind) {
      case 'run': {
        mark(step.cell, { kind: 'run', mark: runMark(step, page.summary, blocks), run: step.run });
        back = backOf(step, column, blocks);
        const wrote = wroteParts(step, page);
        did = column.has(step.cell)
          ? wrote
          : [{ name: step.cell }, ', which the flow no longer has', ...(wrote.length > 0 ? [' · ', ...wrote] : [])];
        break;
      }
      case 'wrote': {
        if (step.by.kind === 'run') return;
        runs = ran.get(step.at) ?? [];
        if (step.by.kind === 'outside' && runs.length === 0) return;
        const sign: RowMark = step.by.kind === 'owner' ? { kind: 'owner' } : { kind: 'outside' };
        for (const r of runs) mark(r.cell, sign);
        did = writeParts(step, page, answered);
        break;
      }
      case 'letGo':
        mark(step.cell, { kind: 'owner' });
        did = ['let ', { name: step.cell }, ' go on'];
        break;
      case 'paused':
        did = step.by === 'engine' ? [`the engine paused: ${plain(sentence(step.why ?? 'it gave no reason'))}`] : ['paused'];
        break;
      case 'resumed':
        did = ['resumed'];
        break;
      case 'cancelled':
        did = [step.why ? `cancelled: ${step.why}` : 'cancelled'];
        break;
      case 'reopened':
        did = [`reopened: ${step.why}`];
        break;
      case 'session':
        return;
    }
    const today = dayKey(step.time);
    rows.push({ key: `${step.kind}:${step.at}`, index, step, marks, newDay: today !== day, did, ran: runs, back });
    day = today;
  });
  return rows;
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
