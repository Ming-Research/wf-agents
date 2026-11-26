import { describe, expect, it } from 'vitest';

import type {
  Back,
  Block,
  CallRow,
  Cause,
  CellNow,
  Item,
  Now,
  Pauser,
  Question,
  Round,
  RoundSession,
  Run,
  Step,
  Summary,
  Wait,
  Writer,
} from '../works-api';
import { outlineOf } from '../testing';
import { FLOW, fixtureWork } from './fixtures';
import { type HistoryRow, LAST_FIRST, orderWorks, placeSteps, runState, sessionsBegun, standingRun } from './placing';
import { partsText } from './words';

/// The outline of the fixtures' flow, as a work on its version is given it.
const OUTLINE = outlineOf(FLOW);

const SPENT = {
  usdMicros: 0,
  inputTokens: 0,
  cacheWrittenTokens: 0,
  unsplitTokens: 0,
  cachedTokens: 0,
  outputTokens: 0,
  workingMs: 0,
  rounds: 0,
};

/// A round of a run, ended with what came back, in the session of its line where one is given.
function round(number: number, back: Back | null, session: RoundSession | null = null, began = 0): Round {
  return {
    number,
    began,
    ended: back === null ? null : began + 1,
    given: { kind: 'task' },
    back,
    spent: null,
    context: null,
    compactions: 0,
    session,
    tries: [],
    missed: null,
    estimated: false,
    judged: { cost: null, time: null },
  };
}

/// A run at this place among the steps, for these reasons, from these versions; finished at the place after it unless
/// it is said to be open or abandoned, with its rounds and its calls where given.
function run(
  n: number,
  cell: string,
  at: number,
  because: Cause[],
  basis: Run['basis'],
  o: { end?: 'finished' | 'open' | 'abandoned'; rounds?: Round[]; calls?: CallRow[] } = {},
): Step & { kind: 'run' } {
  const end = o.end ?? 'finished';
  return {
    kind: 'run',
    run: n,
    cell,
    at,
    time: at * 1000,
    because,
    basis,
    rounds: o.rounds ?? [],
    calls: o.calls ?? [],
    end:
      end === 'open'
        ? null
        : end === 'abandoned'
          ? { kind: 'abandoned', at: at + 1, time: at * 1000 + 500 }
          : { kind: 'finished', at: at + 1, time: at * 1000 + 500, writes: [] },
    spent: SPENT,
  };
}

/// A write of a block, by whom, at this place.
const wrote = (block: string, at: number, by: Writer): Step => ({
  kind: 'wrote',
  at,
  time: at * 1000,
  block,
  by,
  changed: true,
  value: null,
  chars: 5,
});

/// The versions of each block, each `[place, writer]` in order, numbered from one, each changing the block's meaning.
function blocksOf(written: Record<string, [number, Writer][]>): Block[] {
  return Object.entries(written).map(([name, versions]) => ({
    name,
    versions: versions.map(([at, by], i) => ({
      at,
      number: i + 1,
      time: at * 1000,
      by,
      meaning: i + 1,
      value: null,
      saidTo: null,
      chars: 5,
      empty: false,
    })),
  }));
}

/// A work in brief that does this now, its cells standing as given, waiting on the owner for what the engine's list says.
function summary(now: Now, cells: CellNow[] = [], waits: Wait[] = [], paused: Pauser | null = null): Summary {
  return { number: 7, item: 7, flow: 'engineer', added: 0, paused, now, waits, cells, spent: SPENT, last: null };
}

const OWNER: Writer = { kind: 'owner' };
const changed = (block: string, behind = false): Cause => ({ kind: 'changed', block, workedFrom: null, behind });

/// A row as the history draws it: what it names, the line it is or the column and state of its run, the column its
/// arrow starts from, and what it says on pointing at it.
const drawn = (r: HistoryRow) => ({
  names: r.names.map((n) => (n.mark === null ? n.name : `${n.name} (${n.mark})`)),
  divider: r.divider,
  run: r.run === null ? null : `${r.run.column} ${r.run.state}`,
  from: r.from?.column ?? null,
  did: partsText(r.did),
});

describe('the rows of a history', () => {
  it("makes a row of each run, in its cell's column, named by what woke it, with an arrow from the cell whose run wrote it", () => {
    const rows = placeSteps(fixtureWork(41));
    // The writes of the owner and of the outside are no rows: the runs they woke name them.
    expect(rows.map((r) => r.step.kind)).toEqual(Array(12).fill('run'));
    expect(rows.map((r) => r.names.map((n) => n.name).join(' '))).toEqual([
      'request',
      'plan',
      'solution',
      'opinions_plan',
      'plan',
      'plan',
      'code',
      'opinions_implement',
      'code',
      'opinions_implement',
      'code',
      'main',
    ]);
    expect(rows.map((r) => r.run?.column)).toEqual([0, 1, 2, 0, 1, 2, 3, 2, 3, 2, 3, 4]);
    // The arrow starts in the column of the cell whose run wrote the latest of what woke the run: none where the owner
    // or the outside wrote it.
    expect(rows.map((r) => r.from?.column ?? null)).toEqual([null, 0, 1, null, 0, 0, 2, 3, 2, 3, 2, null]);
    expect(rows[7].from?.words).toBe('review: outputs opinions_implement v1');
    // The owner's request and the main the outside wrote are named with the sign of who wrote them.
    expect(rows[0].names).toEqual([{ name: 'request', mark: 'people' }]);
    expect(rows[11].names).toEqual([{ name: 'main', mark: 'outside' }]);
    // A run of the implementation abandoned runs again; each other run handed in.
    expect(rows.map((r) => r.run?.state)).toEqual(['handedIn', 'handedIn', 'again', ...Array(9).fill('handedIn')]);
    // On pointing at its names, everything that woke it, the latest first.
    expect(partsText(rows[2].did)).toBe('woken by solution v1 from deep_research; plan v1 from plan');
  });

  it("draws the work's pauses, resumes, cancels and reopenings as lines between the runs, none before the first run", () => {
    const steps: Step[] = [
      wrote('request', 1, OWNER),
      // A work is added paused and let on to begin: nothing lies before its first run.
      { kind: 'paused', at: 2, time: 2000, by: 'owner', why: null },
      { kind: 'resumed', at: 3, time: 3000 },
      run(1, 'plan', 4, [{ kind: 'noRecord' }], [{ block: 'request', version: 1 }]),
      { kind: 'paused', at: 6, time: 6000, by: 'engine', why: 'The agent could not start' },
      { kind: 'resumed', at: 7, time: 7000 },
      { kind: 'paused', at: 8, time: 8000, by: 'owner', why: null },
      { kind: 'cancelled', at: 9, time: 9000, why: 'Joined into #3' },
      { kind: 'reopened', at: 10, time: 10000, why: 'It is wanted after all' },
      // A new session is no row: the run's Step says it.
      { kind: 'session', at: 11, time: 11000, line: 'builder', why: { kind: 'owner' } },
    ];
    const rows = placeSteps({
      steps,
      outline: OUTLINE,
      blocks: blocksOf({ request: [[1, OWNER]] }),
      summary: summary({ kind: 'paused', ran: true, at: 'plan', by: 'owner', why: null }),
    });
    expect(rows.map(drawn)).toEqual([
      { names: ['request (people)'], divider: null, run: '0 handedIn', from: null, did: 'woken by request v1 from the owner' },
      { names: [], divider: 'paused', run: null, from: null, did: 'the engine paused: The agent could not start.' },
      { names: [], divider: 'resumed', run: null, from: null, did: 'let on again' },
      { names: [], divider: 'paused', run: null, from: null, did: 'paused by the owner' },
      { names: [], divider: 'cancelled', run: null, from: null, did: 'cancelled: Joined into #3' },
      { names: [], divider: 'reopened', run: null, from: null, did: 'reopened: It is wanted after all' },
    ]);
    // A pause of the engine's with no reason says so.
    const silent = placeSteps({
      steps: [steps[3], { kind: 'paused', at: 6, time: 6000, by: 'engine', why: null }],
      outline: OUTLINE,
      blocks: [],
      summary: summary({ kind: 'merged' }),
    });
    expect(partsText(silent[1].did)).toBe('the engine paused: it gave no reason.');
  });

  it("names after what woke a run last each other write of the owner's since the cell's run before, an answer by its question, and the results of calls by their tool", () => {
    const review: Writer = { kind: 'run', run: 4, cell: 'review' };
    const tests: CallRow = {
      call: 1,
      what: { kind: 'tool', name: 'run_tests', input: '' },
      made: 16_000,
      done: 17_000,
      chars: 3,
      time: null,
      size: null,
    };
    const scope: Question = {
      name: 'scope',
      cell: 'plan',
      version: 1,
      engine: false,
      waits: true,
      tree: false,
      withdrawn: null,
      ask: 'Which scope?',
      options: [{ does: 'All', follows: '' }],
      recommends: 1,
      answer: { option: 1, at: 18, why: null },
    };
    const steps: Step[] = [
      wrote('request', 1, OWNER),
      run(1, 'plan', 2, [{ kind: 'noRecord' }], [{ block: 'request', version: 1 }]),
      wrote('opinions_plan', 8, OWNER),
      wrote('request', 9, OWNER),
      run(2, 'plan', 10, [changed('opinions_plan'), changed('request')], [
        { block: 'request', version: 2 },
        { block: 'opinions_plan', version: 1 },
      ]),
      run(3, 'implement', 11, [changed('plan')], [{ block: 'plan', version: 1 }]),
      run(4, 'review', 13, [changed('code')], [{ block: 'code', version: 1 }]),
      run(5, 'implement', 15, [changed('opinions_implement', true)], [{ block: 'opinions_implement', version: 1 }], {
        end: 'open',
        calls: [tests],
      }),
      run(6, 'check', 17, [{ kind: 'results', calls: [1] }], []),
      // The engine writes the owner's answer into the questions of the question's cell as it takes it.
      wrote('opinions_plan', 19, OWNER),
      run(7, 'plan', 20, [changed('opinions_plan')], [{ block: 'opinions_plan', version: 2 }]),
      run(8, 'gone', 22, [{ kind: 'noRecord' }], []),
    ];
    const blocks = blocksOf({
      request: [
        [1, OWNER],
        [9, OWNER],
      ],
      opinions_plan: [
        [8, OWNER],
        [19, OWNER],
      ],
      plan: [[3, { kind: 'run', run: 1, cell: 'plan' }]],
      code: [[12, { kind: 'run', run: 3, cell: 'implement' }]],
      opinions_implement: [[14, review]],
    });
    const rows = placeSteps({
      steps,
      outline: OUTLINE,
      blocks,
      summary: summary({ kind: 'waits', at: 'implement', on: [] }),
      questions: [scope],
    });
    expect(rows).toHaveLength(8);
    expect(rows.slice(0, 7).map(drawn)).toEqual([
      { names: ['request (people)'], divider: null, run: '0 handedIn', from: null, did: 'woken by request v1 from the owner' },
      // The request written last woke it, and the owner's words since the plan's first run are named after it.
      {
        names: ['request (people)', 'opinions_plan (people)'],
        divider: null,
        run: '0 handedIn',
        from: null,
        did: 'woken by request v2 from the owner; opinions_plan v1 from the owner',
      },
      { names: ['plan'], divider: null, run: '2 handedIn', from: 0, did: 'woken by plan v1 from plan' },
      { names: ['code'], divider: null, run: '3 handedIn', from: 2, did: 'woken by code v1 from implement' },
      // What the review wrote ran the implementation again: its arrow starts in a later cell's column.
      {
        names: ['opinions_implement'],
        divider: null,
        run: '2 atRest',
        from: 3,
        did: 'woken by opinions_implement v1 from review',
      },
      {
        names: ['run_tests (tools)'],
        divider: null,
        run: '4 handedIn',
        from: null,
        did: 'woken by the results of the tool run_tests',
      },
      // The owner's answer to the plan's question woke it: it is named by the question it answered.
      {
        names: ['scope (people)'],
        divider: null,
        run: '0 handedIn',
        from: null,
        did: 'woken by opinions_plan v2 from the owner',
      },
    ]);
    // A run of a cell the outline has no column for keeps its row, in no column.
    const gone = rows[7];
    expect(gone.run).toBeNull();
    expect(partsText(gone.did)).toBe('gone, which the outline does not have · it had not run before');
  });
});

describe('how a run ended or stands', () => {
  const failed: Back = { kind: 'failed', why: 'The agent could not start.', failure: 'other', resets: null };
  const handedIn: Back = { kind: 'handedIn', blocks: [] };
  const open = (rounds: Round[]) => run(3, 'implement', 5, [], [], { end: 'open', rounds });
  const cell = (state: CellNow['state']): CellNow => ({ cell: 'implement', state, finished: 0, record: null, open: 3 });
  const waiting: Now = { kind: 'waits', at: 'implement', on: [] };

  it('takes the states of the strip of the console: a run that ended handed in, or runs again where it was abandoned', () => {
    expect(runState(run(3, 'implement', 5, [], []), summary(waiting))).toBe('handedIn');
    expect(runState(run(3, 'implement', 5, [], [], { end: 'abandoned' }), summary(waiting))).toBe('again');
  });

  it('marks an open run as its cell stands: running, waiting for a call, failed, at rest, or stopped where the work is stopped at it', () => {
    expect(runState(open([round(1, null)]), summary(waiting, [cell('running')]))).toBe('running');
    expect(runState(open([round(1, null)]), summary(waiting, [cell('calling')]))).toBe('waiting');
    expect(runState(open([round(1, null)]), summary(waiting, [cell('failed')]))).toBe('failed');
    expect(runState(open([round(1, failed)]), summary(waiting, [cell('failed')]))).toBe('failed');
    expect(runState(open([round(1, handedIn)]), summary(waiting, [cell('failed')]))).toBe('failed');
    expect(runState(open([round(1, { kind: 'asked', asks: [] })]), summary(waiting, [cell('resting')]))).toBe('atRest');
    // The cell's state for another run of it says nothing of this one.
    expect(runState(open([round(1, null)]), summary(waiting, [{ ...cell('running'), open: 9 }]))).toBe('atRest');
    const pause: Wait = { key: 'pause', why: 'The engine paused the work.', ends: { kind: 'resume' } };
    const stopped = summary({ kind: 'paused', ran: true, at: 'implement', by: 'engine', why: 'x' }, [cell('failed')], [pause], 'engine');
    expect(runState(open([round(1, failed)]), stopped)).toBe('stopped');
  });

  // The page does not work out for itself that a run is to be given its round again from what its last round came back
  // as: the engine says it of the cell, for a round that failed and for a return it sends back, a pause holding it or not.
  it('does not read from what the last round came back as whether an open run is to be given again', () => {
    for (const back of [failed, handedIn, { kind: 'asked', asks: [] } as Back, null]) {
      expect(runState(open([round(1, back)]), summary(waiting, [cell('resting')]))).toBe('atRest');
    }
  });

  it("is the run the item's window shows where no row is chosen: the run under way, else the work's latest", () => {
    const steps: Step[] = [run(1, 'plan', 1, [], []), run(2, 'implement', 3, [], []), run(3, 'implement', 5, [], [])];
    expect(standingRun({ steps, summary: summary({ kind: 'round', cell: 'plan', run: 1, round: 2, since: 0 }) })).toBe(1);
    expect(standingRun({ steps, summary: summary(waiting) })).toBe(3);
    // Standing at a cell that has not run, or ended, it shows what ran last, never nothing.
    expect(standingRun({ steps, summary: summary({ kind: 'waits', at: 'review', on: [] }) })).toBe(3);
    expect(standingRun({ steps, summary: summary({ kind: 'merged' }) })).toBe(3);
    expect(standingRun({ steps: [], summary: summary(waiting) })).toBeNull();
  });

  it('gives each new session to the run that began it: by its number on its line, or, lost or compacted, to the round then under way', () => {
    const builder = (number: number): RoundSession => ({ line: 'builder', number });
    const steps: Step[] = [
      run(1, 'plan', 1, [], [], { rounds: [round(1, handedIn, null, 1000)] }),
      { kind: 'session', at: 2, time: 2000, line: 'builder', why: { kind: 'owner' } },
      run(2, 'implement', 3, [], [], {
        rounds: [round(1, handedIn, builder(1), 3000), round(2, handedIn, builder(1), 4000)],
      }),
      { kind: 'session', at: 5, time: 4500, line: 'builder', why: { kind: 'compacted' } },
      // A session whose first round has not begun is with no run yet.
      { kind: 'session', at: 6, time: 6000, line: 'builder', why: { kind: 'limit', context: 600_000, limit: 500_000 } },
    ];
    const begun = sessionsBegun(steps);
    expect([...begun.keys()]).toEqual([2]);
    expect(begun.get(2)?.map((s) => s.why.kind)).toEqual(['owner', 'compacted']);
  });
});

/// An item whose work runs, last changed then, at this level and cell level, having cost this much.
function listed(
  number: number,
  o: {
    last?: number | null;
    level?: string | null;
    cellLevel?: string | null;
    cost?: number;
    time?: number;
    title?: string;
    unread?: boolean;
  } = {},
): Item {
  const work: Summary = {
    ...summary({ kind: 'round', cell: 'plan', run: 1, round: 1, since: 0 }),
    number,
    item: number,
    last: o.last ?? null,
    level: o.level,
    cellLevel: o.cellLevel,
    spent: { ...SPENT, usdMicros: o.cost ?? 0, workingMs: o.time ?? 0 },
    ...(o.unread === true ? { unread: true, cells: [] } : {}),
  };
  return {
    number,
    title: o.title ?? `${number}`,
    level: '2',
    project: 'wf-agents',
    added: 0,
    dependsOn: [],
    mainDependent: null,
    history: { first: { title: `${number}`, level: '2', dependsOn: [] }, changes: [] },
    work,
  };
}

describe('the order of the list of works', () => {
  const order = (items: Item[], by = LAST_FIRST) => orderWorks(items, by).map((w) => w.work.number);

  it('is what happened last first until the owner picks a column, a work to which nothing has happened the oldest', () => {
    const items = [listed(1, { last: 50 }), listed(2, { last: 90 }), listed(3), listed(4, { last: 50 })];
    // Of two works last changed at once, the later item first, whichever way the column runs.
    expect(order(items)).toEqual([2, 4, 1, 3]);
    expect(order(items, { by: 'last', turned: true })).toEqual([3, 4, 1, 2]);
  });

  it('puts the works with no value in a column last whichever way it runs, and orders levels by number, a named one below them', () => {
    const items = [
      listed(1, { level: '2', last: 1 }),
      listed(2, { level: null, last: 2 }),
      listed(3, { level: '10', last: 3 }),
      listed(4, { level: 'easy', last: 4 }),
      listed(5, { level: '3', last: 5 }),
    ];
    expect(order(items, { by: 'level', turned: false })).toEqual([3, 5, 1, 4, 2]);
    expect(order(items, { by: 'level', turned: true })).toEqual([4, 1, 5, 3, 2]);
    const cells = [listed(1, { cellLevel: '3', last: 1 }), listed(2, { last: 2 }), listed(3, { cellLevel: '2', last: 3 })];
    expect(order(cells, { by: 'cellLevel', turned: false })).toEqual([1, 3, 2]);
  });

  it('orders by cost the dearest first, by title from A to Z, and by item the newest first', () => {
    const items = [
      listed(1, { cost: 20, title: 'Beta' }),
      listed(2, { cost: 90, title: 'alpha' }),
      listed(3, { cost: 50, title: 'Gamma' }),
    ];
    expect(order(items, { by: 'cost', turned: false })).toEqual([2, 3, 1]);
    expect(order(items, { by: 'title', turned: false })).toEqual([2, 1, 3]);
    expect(order(items, { by: 'item', turned: false })).toEqual([3, 2, 1]);
  });

  it('puts a work whose records the engine could not read last in the columns of cost and time, whichever way they run', () => {
    // Its cost and time are not worked out: shown as nothing, they would put it first when the column ran upward.
    const items = [
      listed(1, { cost: 20, time: 200 }),
      listed(2, { cost: 0, time: 0, unread: true }),
      listed(3, { cost: 50, time: 100 }),
    ];
    expect(order(items, { by: 'cost', turned: false })).toEqual([3, 1, 2]);
    expect(order(items, { by: 'cost', turned: true })).toEqual([1, 3, 2]);
    expect(order(items, { by: 'time', turned: false })).toEqual([1, 3, 2]);
    expect(order(items, { by: 'time', turned: true })).toEqual([3, 1, 2]);
  });
});
