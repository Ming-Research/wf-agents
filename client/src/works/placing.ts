// A work's history as the chart draws it, and the order of the list of works, worked out from the interface's JSON
// alone so that they can be tested without drawing.
//
// The history is the chain of what woke what: a datum written woke a cell, whose run wrote data that woke the next.
// Its columns are the cells of the work's flow in the order of the walk, and its rows, in the order they happened, the
// runs of the cells, the owner's acts that woke none, and the pauses, resumes, cancels and reopenings of the work. A
// run's row names what woke it: of the writes among its causes, the one written last, by the name its row has on the
// flow's matrix, with the owner's sign or the outside's where either wrote it, or the tool whose results came in, with
// the tool's mark; and after it each other write of the owner's that woke it, so that no act of the owner's is lost
// from the history. Where a run of another cell wrote what woke it, that cell's column has the dot of `outputs`, from
// which an arrow runs to the run's own column, back where that cell is the later. A write of the owner's or the
// outside's that woke a cell is no row: the run it woke names it; one of the outside's that woke none is the engine's
// own bookkeeping. A write of a run is no row: it is the dot of the rows of the runs it woke. A new session of a line
// is no row: the run's Step says it. A run of a cell the flow no longer has keeps its row, in no column.
//
// A write woke a run where the run has a change of the block among its causes and the version of the block it started
// from has, as its meaning, the version the write wrote: the records name a run's cause by its block, and the run's
// basis names the version it started from, whose meaning names the version that last changed it.

import type { Cell, Flow } from '../api';
import type { Block, CallRow, Item, Run, Step, Summary, WorkPage, Writer } from '../works-api';
import { type RunState } from './state-marks';
import { type Part, STATE, callWords, causeWords, plain, sentence, stateOf, stoppedAt, writerWords } from './words';

/// A new session of a line, as a step.
export type SessionStep = Step & { kind: 'session' };

/// What a row names at its left: a datum, by the name its row has on the flow's matrix; a tool or a person whose
/// results came in; or an act on the work; with the mark after it: the owner's sign where the owner wrote it, answered
/// or acted, the outside's where the outside wrote it, the tool's mark of a tool, or none.
export interface Named {
  name: string;
  mark: 'people' | 'outside' | 'tools' | null;
}

export interface HistoryRow {
  /// Unique among the rows: the step's kind and place.
  key: string;
  /// The step's place among the steps.
  index: number;
  step: Step;
  /// What it names at its left: of a run, what woke it; else the act.
  names: Named[];
  /// Of a run of a cell the flow has, its cell's column and how it ended or stands.
  run: { column: number; state: RunState; words: string } | null;
  /// Of a run woken by what a run of another cell the flow has wrote, that cell's column, where the dot stands and the
  /// arrow starts, and what the dot says.
  from: { column: number; words: string } | null;
  /// Whether the row's day differs from the day of the row before, so that the day is shown again.
  newDay: boolean;
  /// What it is, in one line, said on pointing at its names: of a run, everything that woke it; else the act.
  did: Part[];
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

/// The name a datum's row has on the flow's matrix where it runs `cell`: what the cell's `watches` names it by, a
/// group or itself; else a group a cell watches by its name, on whose row the matrix marks its watchers; else its own.
export function rowName(flow: Pick<Flow, 'groups' | 'cells'>, block: string, cell?: Cell): string {
  const own = cell?.watches.find((w) => w.blocks.includes(block));
  if (own !== undefined) return own.name;
  const watched = (group: string) => flow.cells.some((c) => c.watches.some((w) => w.name === group));
  return flow.groups.find((g) => g.blocks.includes(block) && watched(g.name))?.name ?? block;
}

/// The mark of who wrote a version, after its name: the owner's sign, the outside's, or none of a run.
const signOf = (by: Writer): Named['mark'] =>
  by.kind === 'owner' ? 'people' : by.kind === 'outside' ? 'outside' : null;

/// A write, or the results of calls, that woke a run: when it came, what the row names it by and how its tip says it;
/// the cell whose run wrote it, with the run and the version; and whether the owner wrote it.
interface Waking {
  time: number;
  at: number;
  names: Named[];
  words: Part[];
  by: { cell: string; run: number; version: string } | null;
  owner: boolean;
}

/// What woke a run, the latest first: of each block among its causes, the write that gave the version it started from
/// its meaning (of a run its cell ran with no record, each block the cell watches that it started from a version of);
/// and the results of the calls its cell asked for, which came in as the last of them did.
function wakings(run: Run, page: Pick<WorkPage, 'flow' | 'blocks'>, calls: Map<number, CallRow>): Waking[] {
  const cell = page.flow.cells.find((c) => c.id === run.cell);
  const versions = new Map(page.blocks.map((b) => [b.name, b.versions]));
  const changed = new Set(run.because.flatMap((c) => (c.kind === 'changed' ? [c.block] : [])));
  const first = run.because.some((c) => c.kind === 'noRecord') ? (cell?.watches.flatMap((w) => w.blocks) ?? []) : [];
  const out: Waking[] = [];
  for (const block of new Set([...changed, ...first])) {
    const all = versions.get(block) ?? [];
    const started = run.basis.find((b) => b.block === block)?.version ?? null;
    const meaning = all.find((v) => v.number === started)?.meaning ?? null;
    const wrote = all.find((v) => v.number === meaning);
    const name = rowName(page.flow, block, cell);
    if (wrote === undefined) {
      // A change whose write the work does not show is named still, as the oldest, with no one who wrote it.
      if (changed.has(block)) {
        const names: Named[] = [{ name, mark: null }];
        out.push({ time: -Infinity, at: -1, names, words: [{ name: block }], by: null, owner: false });
      }
      continue;
    }
    const version = `${block} v${wrote.number}`;
    out.push({
      time: wrote.time,
      at: wrote.at,
      names: [{ name, mark: signOf(wrote.by) }],
      words: [{ name: version }, ` from ${writerWords(wrote.by)}`],
      by: wrote.by.kind === 'run' ? { cell: wrote.by.cell, run: wrote.by.run, version } : null,
      owner: wrote.by.kind === 'owner',
    });
  }
  for (const cause of run.because) {
    if (cause.kind !== 'results') continue;
    const made = cause.calls.flatMap((id) => calls.get(id) ?? []);
    const names = made.map((c): Named => {
      if (c.what.kind === 'tool') return { name: c.what.name, mark: 'tools' };
      if (c.what.kind === 'question') return { name: c.what.who, mark: 'people' };
      return { name: c.what.kind, mark: null };
    });
    out.push({
      time: Math.max(-Infinity, ...made.map((c) => c.done ?? c.made)),
      at: Infinity,
      names: names.filter((n, i) => names.findIndex((m) => m.name === n.name && m.mark === n.mark) === i),
      words: ['the results of ', made.map((c) => callWords(c.what)).join(' and ') || `calls ${cause.calls.join(', ')}`],
      by: null,
      owner: false,
    });
  }
  return out.sort((a, b) => b.time - a.time || b.at - a.at);
}

/// A run's row: what woke it, the latest and then each other write of the owner's made since the cell's run before, each
/// name once; the column of the cell whose run wrote the latest, where the flow has it and it is another than the run's;
/// and the run's own column and mark.
function runRow(
  run: Run,
  page: Pick<WorkPage, 'flow' | 'blocks' | 'summary'>,
  column: Map<string, number>,
  calls: Map<number, CallRow>,
  /// When the cell's run before this one began; none where this is its first.
  before: number | null,
): Pick<HistoryRow, 'names' | 'run' | 'from' | 'did'> {
  const woke = wakings(run, page, calls);
  const [latest, ...rest] = woke;
  const said = rest.filter((w) => w.owner && (before === null || w.time > before));
  const all = [...(latest?.names ?? []), ...said.flatMap((w) => w.names)];
  const names = all.filter((n, i) => all.findIndex((m) => m.name === n.name && m.mark === n.mark) === i);
  const at = column.get(run.cell);
  const state = runState(run, page.summary, page.blocks);
  const by = latest?.by ?? null;
  const fromAt = by === null ? undefined : column.get(by.cell);
  const why =
    woke.length > 0
      ? ['woken by ', ...woke.flatMap((w, i): Part[] => [...(i === 0 ? [] : ['; ']), ...w.words])]
      : [run.because.map((c) => causeWords(c)).join('; ')];
  return {
    names,
    run: at === undefined ? null : { column: at, state, words: `${run.cell} run ${run.run}` },
    from:
      by === null || fromAt === undefined || at === undefined || fromAt === at
        ? null
        : { column: fromAt, words: `${by.cell} run ${by.run}: outputs ${by.version}` },
    did: at === undefined ? [{ name: run.cell }, ', which the flow no longer has · ', ...why] : why,
  };
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
/// a round of it runs, or it waits for a call or an act carried out for it; its last round failed, or what it handed in
/// last was sent back; or else it is at rest between two rounds.
export function runState(run: Run, summary: Pick<Summary, 'now' | 'cells' | 'waits'>, blocks: Block[]): RunState {
  const end = run.end;
  if (end?.kind === 'abandoned') return 'abandoned';
  if (end?.kind === 'finished') {
    const changed = blocks.some((b) => b.versions.some((v) => v.at === end.at && v.meaning === v.number));
    return changed ? 'handedIn' : 'nothingNew';
  }
  if (stoppedAt(summary) === run.cell) return 'stopped';
  const now = summary.cells.find((c) => c.cell === run.cell && c.open === run.run)?.state;
  if (now === 'running') return 'running';
  if (now === 'calling') return 'waiting';
  const back = run.rounds.at(-1)?.back?.kind;
  return now === 'failed' || back === 'failed' || back === 'handedIn' ? 'failed' : 'atRest';
}

/// A block as a row names it: with the value its version has, where it has one (`plan_ok = no`), else with its
/// versions (`plan v2`).
const named = (block: string, value: string | null, numbers: number[]): string =>
  value !== null ? `${block} = ${value}` : `${block} ${numbers.map((n) => `v${n}`).join(' ')}`;

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

/// What a write of the owner's did: answered a question, by its name and the option taken; said the owner's words to
/// the cell they reached; or wrote the block, with the value it wrote or its version.
function writeParts(
  step: Step & { kind: 'wrote' },
  page: Pick<WorkPage, 'blocks'>,
  answered: Map<number, { question: string; option: number }>,
): Part[] {
  const version = page.blocks.find((b) => b.name === step.block)?.versions.find((v) => v.at === step.at);
  const answer = answered.get(step.at);
  const saidTo = version?.saidTo ?? null;
  if (answer !== undefined) return ['answered ', { name: answer.question }, ': ', { name: String(answer.option) }];
  if (saidTo !== null) return ['said to ', { name: saidTo }];
  return ['wrote ', { name: named(step.block, step.value, version === undefined ? [] : [version.number]) }];
}

/// The owner's sign, after the name of an act of the owner's.
const owners = (name: string): Named[] => [{ name, mark: 'people' }];

/// A row of an act: its name, nothing under the cells, and what it did.
const act = (names: Named[], did: Part[]): Pick<HistoryRow, 'names' | 'run' | 'from' | 'did'> => ({
  names,
  run: null,
  from: null,
  did,
});

/// The rows of the history, in the order the steps happened.
export function placeSteps(page: Pick<WorkPage, 'steps' | 'flow' | 'blocks' | 'summary' | 'questions'>): HistoryRow[] {
  const { steps, flow, blocks } = page;
  const column = new Map(flow.cells.map((c, i) => [c.id, i]));
  const calls = new Map(steps.flatMap((s) => (s.kind === 'run' ? s.calls.map((c) => [c.call, c] as const) : [])));
  const ran = ranBy(steps, blocks);
  const answered = answers(page);
  // When each cell's latest run so far began.
  const began = new Map<string, number>();
  // A step's row, or none where it is no row.
  const rowOf = (step: Step): Pick<HistoryRow, 'names' | 'run' | 'from' | 'did'> | null => {
    switch (step.kind) {
      case 'run': {
        const row = runRow(step, page, column, calls, began.get(step.cell) ?? null);
        began.set(step.cell, step.time);
        return row;
      }
      case 'wrote':
        if (step.by.kind !== 'owner' || (ran.get(step.at) ?? []).length > 0) return null;
        return act(owners(rowName(flow, step.block)), writeParts(step, page, answered));
      case 'letGo':
        return act(owners('let go'), ['let ', { name: step.cell }, ' go on']);
      case 'paused':
        if (step.by === 'owner') return act(owners('paused'), ['paused']);
        return act(
          [{ name: 'paused', mark: null }],
          [`the engine paused: ${plain(sentence(step.why ?? 'it gave no reason'))}`],
        );
      case 'resumed':
        return act(owners('resumed'), ['resumed']);
      case 'cancelled':
        return act(owners('cancelled'), [step.why ? `cancelled: ${step.why}` : 'cancelled']);
      case 'reopened':
        return act(owners('reopened'), [`reopened: ${step.why}`]);
      case 'session':
        return null;
    }
  };
  const rows: HistoryRow[] = [];
  let day: string | null = null;
  steps.forEach((step, index) => {
    const row = rowOf(step);
    if (row === null) return;
    const today = dayKey(step.time);
    rows.push({ key: `${step.kind}:${step.at}`, index, step, newDay: today !== day, ...row });
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
