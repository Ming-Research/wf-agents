// Made-up items for looking at the console with no server behind it (`console.html?fixtures`), and for its tests: some
// twenty items, each with its one work, depending on each other in four layers, their works in every state a work can
// be in, on one made-up flow of an engineer's work. What the owner does to a work changes it here, and the console is
// told as the interface's stream of changes would tell it.

import type { Block, Cell, Flow, Limits, Test } from '../api';
import type {
  Act,
  CellDetail,
  CellNow,
  CellState,
  Changed,
  Item,
  Items,
  Now,
  Spent,
  Summary,
  Waited,
  WorkPage,
} from '../works-api';
import type { Source } from './source';

const LIMITS: Limits = { runs: 3, returns: 2, calls: 40 };

type CellIn = {
  id: string;
  title: string;
  line?: string | null;
  /// What an agent's cell's agent is told to do.
  task?: string;
  /// The engine's act that carries out an engine's cell.
  act?: string;
  inputs: string[];
  outputs: string[];
  condition?: Cell['condition'];
  preconditions?: Test[];
};

/// The flow of these cells in this order, with these blocks, who writes and reads each worked out as the compiler does.
function flowOf(
  name: string,
  cells: CellIn[],
  blocks: Record<string, { values?: string[]; people?: boolean; outside?: string; shown?: boolean }>,
): Flow {
  const all: Cell[] = cells.map((c) => ({
    id: c.id,
    title: c.title,
    kind: c.line === null ? 'engine' : 'agent',
    line: c.line === undefined ? 'engineer' : c.line,
    task: c.task ?? null,
    act: c.act ?? null,
    condition: c.condition ?? null,
    preconditions: c.preconditions ?? [],
    inputs: c.inputs,
    outputs: c.outputs,
    limits: LIMITS,
  }));
  const tests = [...new Map(all.flatMap((c) => c.preconditions).map((t) => [t.name, t])).values()];
  const named: Block[] = Object.keys(blocks)
    .sort()
    .map((b) => ({
      name: b,
      values: blocks[b].values ?? null,
      writtenBy: {
        cells: all.filter((c) => c.outputs.includes(b)).map((c) => c.id),
        people: blocks[b].people ?? false,
        outside: blocks[b].outside ?? null,
      },
      readBy: {
        cells: all.filter((c) => c.inputs.includes(b)).map((c) => c.id),
        conditions: all.filter((c) => c.condition?.on === 'block' && c.condition.block === b).map((c) => c.id),
        tests: tests.filter((t) => t.reads.includes(b)).map((t) => t.name),
        shown: blocks[b].shown ?? false,
      },
    }));
  const lines = [...new Set(all.flatMap((c) => (c.line ? [c.line] : [])))].sort().map((l) => ({
    name: l,
    cells: all.filter((c) => c.line === l).map((c) => c.id),
  }));
  return { name, limits: LIMITS, lines, tests, cells: all, blocks: named };
}

/// A named test that holds when each of these blocks, which the owner sets, says yes.
const test = (name: string, words: string, reads: string[]): Test => ({ name, words, kind: 'all', reads, is: 'yes' });

const PLAN_CONFIRMED = test('plan_confirmed', 'The owner confirmed the plan', ['plan_confirmed']);
const REQUEST_READ = test('request_read', 'The owner read what the item asks for', ['request_read']);
const MAY_MERGE = test('may_merge', 'The reviews are content and the owner lets it merge', ['content', 'merge_allowed']);

/// A precondition not met that the owner makes hold.
const owners = (t: Test): Waited => ({ kind: 'precondition', test: t.name, words: t.words, who: 'owner' });

/// A made-up flow of an engineer's work: the initial research, deep research when it asks for it, the review of its
/// plan, the implementation once the owner confirmed the plan, the tests, the review of the code, a check with the
/// latest main, and the merge.
export const FLOW: Flow = flowOf(
  'engineer',
  [
    {
      id: 'research',
      title: 'Initial research',
      task: 'Research what the item asks for, and write the plan.\nSay whether it needs deep research.',
      preconditions: [REQUEST_READ],
      inputs: ['request', 'plan_opinions'],
      outputs: ['plan', 'needs_deep_research'],
    },
    {
      id: 'deep_research',
      title: 'Deep research',
      line: 'deep_research',
      task: 'Research what the plan leaves open, and write the solution.',
      condition: { on: 'block', block: 'needs_deep_research', value: 'yes' },
      inputs: ['plan', 'plan_review_on_deep_research'],
      outputs: ['solution'],
    },
    {
      id: 'review_plan',
      title: 'Review the plan',
      task: 'Review the solution and the plan against what the item asks for.',
      line: 'review',
      condition: { on: 'block', block: 'needs_deep_research', value: 'yes' },
      inputs: ['plan', 'solution'],
      outputs: ['plan_review_on_deep_research'],
    },
    {
      id: 'implement',
      title: 'Implement',
      task: 'Implement the plan and the solution.',
      preconditions: [PLAN_CONFIRMED],
      inputs: ['plan', 'solution', 'code_opinions'],
      outputs: ['code'],
    },
    {
      id: 'test',
      title: 'Write the tests',
      task: 'Write the tests of the code.',
      inputs: ['code', 'test_opinions'],
      outputs: ['tests'],
    },
    {
      id: 'review',
      title: 'Review the code',
      task: 'Review the code and its tests, and say whether you are content.',
      line: 'review',
      inputs: ['code', 'tests'],
      outputs: ['code_opinions', 'content'],
    },
    { id: 'check', title: 'Check with the latest main', line: null, act: 'check', inputs: ['code', 'main'], outputs: ['ci'] },
    {
      id: 'merge',
      title: 'Merge',
      line: null,
      act: 'merge',
      preconditions: [MAY_MERGE],
      inputs: ['ci', 'content'],
      outputs: [],
    },
  ],
  {
    request: { people: true, shown: true },
    plan_opinions: { people: true },
    plan: { shown: true },
    needs_deep_research: { values: ['yes', 'no'] },
    solution: { shown: true },
    plan_review_on_deep_research: { people: true },
    code: {},
    code_opinions: { people: true },
    tests: {},
    test_opinions: { people: true },
    content: { values: ['yes', 'no'] },
    main: { outside: 'main' },
    plan_confirmed: { values: ['yes', 'no'], people: true },
    request_read: { values: ['yes', 'no'], people: true },
    merge_allowed: { values: ['yes', 'no'], people: true },
    ci: { values: ['green', 'red'] },
  },
);

const ORDER = FLOW.cells.map((c) => c.id);
const MINUTE = 60_000;
const HOUR = 60 * MINUTE;

/// How each cell of the flow stands for a work at `at` in state `state`: the cells before it done, the deep research and
/// the review of its plan skipped unless `deep`, those after it not yet run.
function cellsAt(at: string | null, state: CellState, deep = true): CellNow[] {
  const here = at === null ? ORDER.length : ORDER.indexOf(at);
  return ORDER.map((cell, i): CellNow => {
    const skipped = !deep && (cell === 'deep_research' || cell === 'review_plan') && here > ORDER.indexOf('research');
    if (skipped) return { cell, state: 'skipped', finished: 0, record: null, open: null };
    if (i < here) return { cell, state: 'done', finished: 1, record: 10 + i, open: null };
    if (i === here) {
      const open = state === 'running' || state === 'resting' ? 40 + i : null;
      return { cell, state, finished: state === 'again' ? 1 : 0, record: state === 'again' ? 10 + i : null, open };
    }
    return { cell, state: 'notYet', finished: 0, record: null, open: null };
  });
}

const spentOf = (dollars: number, hours: number, rounds: number): Spent => ({
  usdMicros: Math.round(dollars * 1e6),
  inputTokens: rounds * 41_000,
  cachedTokens: rounds * 30_000,
  outputTokens: rounds * 3_200,
  workingMs: Math.round(hours * HOUR),
  rounds,
});

type Made = { title: string; dependsOn: number[]; work: Omit<Summary, 'item' | 'flow'> };

/// The made-up items, as they stand when the page opens; `start` is the time it is taken from.
export function fixtureItems(start = Date.now()): Items {
  let next = 1;
  /// A work added `hours` ago, last touched `quiet` minutes ago.
  const work = (
    now: Now,
    cells: CellNow[],
    o: { hours?: number; dollars?: number; rounds?: number; quiet?: number; paused?: boolean } = {},
  ): Made['work'] => {
    const hours = o.hours ?? 2;
    return {
      number: next++,
      added: start - hours * HOUR,
      paused: o.paused ?? false,
      now,
      cells,
      spent: spentOf(o.dollars ?? 1.2, Math.min(hours, 1.5), o.rounds ?? 9),
      last: start - (o.quiet ?? 3) * MINUTE,
    };
  };
  const round = (cell: string, minutes: number, number = 2): Now => ({
    kind: 'round',
    cell,
    run: 40 + ORDER.indexOf(cell),
    round: number,
    since: start - minutes * MINUTE,
  });
  const notStarted = cellsAt(ORDER[0], 'notYet');
  const items: Record<number, Made> = {
    1: {
      title: 'Run works without anyone watching',
      dependsOn: [5, 6, 7],
      work: work(
          {
            kind: 'waits',
            at: 'research',
            on: [
              {
                kind: 'call',
                call: 7,
                who: 'owner',
                what: { kind: 'question', who: 'owner', question: 'Should a stale run be abandoned at once or kept a day?' },
              },
            ],
          },
          cellsAt('research', 'resting'),
          { hours: 9, dollars: 0.84, rounds: 4, quiet: 95 },
        ),
    },
    2: {
      title: 'See every work at one glance',
      dependsOn: [8, 9, 22],
      work: work(round('review', 6), cellsAt('review', 'running'), { hours: 30, dollars: 7.31, rounds: 61 }),
    },
    3: {
      title: 'Keep what each run costs',
      dependsOn: [11, 12],
      work: work({ kind: 'paused' }, notStarted, { hours: 1, dollars: 0, rounds: 0, paused: true }),
    },
    4: {
      title: 'Let the owner write to any cell',
      dependsOn: [13, 9],
      work: work({ kind: 'paused' }, cellsAt('implement', 'again'), { hours: 50, dollars: 3.02, rounds: 22, paused: true }),
    },
    5: {
      title: 'Restart a lost session',
      dependsOn: [14, 15],
      work: work(round('implement', 18, 3), cellsAt('implement', 'running', false), {
          hours: 0.7,
          dollars: 2.4,
          rounds: 17,
          paused: true,
        }),
    },
    6: {
      title: 'Abandon a stale run',
      dependsOn: [],
      work: work(
          {
            kind: 'waits',
            at: 'check',
            on: [{ kind: 'call', call: 31, who: null, what: { kind: 'tool', name: 'ci', input: 'the full CI' } }],
          },
          cellsAt('check', 'resting', false),
          { hours: 5, dollars: 4.1, rounds: 38 },
        ),
    },
    7: {
      title: 'Walk the flow from its first cell',
      dependsOn: [16, 24],
      work: work(
          { kind: 'waits', at: 'implement', on: [owners(PLAN_CONFIRMED)] },
          cellsAt('implement', 'notYet'),
          { hours: 3, dollars: 1.9, rounds: 14, quiet: 40 },
        ),
    },
    8: {
      title: 'Draw the items as a tree',
      dependsOn: [17, 22],
      work: work(round('deep_research', 2), cellsAt('deep_research', 'running'), { hours: 0.4, dollars: 0.62, rounds: 5 }),
    },
    9: {
      title: 'Read changes as they happen',
      dependsOn: [18],
      work: work(
          { kind: 'fault', words: 'The agent of the line engineer could not be started' },
          cellsAt('test', 'resting'),
          { hours: 20, dollars: 3.3, rounds: 26 },
        ),
    },
    10: {
      title: 'Show the cells a work went through',
      dependsOn: [],
      work: work({ kind: 'merged' }, cellsAt(null, 'done'), { hours: 70, dollars: 6.2, rounds: 48 }),
    },
    11: {
      title: 'Count the tokens of each round',
      dependsOn: [],
      work: work(
          {
            kind: 'waits',
            at: 'research',
            on: [
              {
                kind: 'call',
                call: 3,
                who: 'architect',
                what: { kind: 'question', who: 'architect', question: 'Which table keeps the tokens of a round?' },
              },
            ],
          },
          cellsAt('research', 'resting'),
          { hours: 0.3, dollars: 0.21, rounds: 2 },
        ),
    },
    12: {
      title: 'Show the cost of a work',
      dependsOn: [11],
      work: work({ kind: 'waits', at: 'review_plan', on: [{ kind: 'limit', runs: 3 }] }, cellsAt('review_plan', 'again'), {
          hours: 26,
          dollars: 9.8,
          rounds: 70,
          quiet: 300,
        }),
    },
    13: {
      title: 'Pick the block a remark goes to',
      dependsOn: [19, 25],
      work: work({ kind: 'next', cell: 'test' }, cellsAt('test', 'notYet', false), { hours: 1.5, dollars: 1.1 }),
    },
    14: {
      title: "Keep the session's id with its line",
      dependsOn: [],
      work: work(round('research', 1, 1), cellsAt('research', 'running'), { hours: 0.05, dollars: 0.03, rounds: 0 }),
    },
    15: {
      title: 'Write a lost session as a change of the outside',
      dependsOn: [],
      work: work(
          { kind: 'waits', at: 'merge', on: [owners(MAY_MERGE)] },
          cellsAt('merge', 'notYet'),
          { hours: 7, dollars: 5.05, rounds: 44, quiet: 25 },
        ),
    },
    16: {
      title: 'Name the tests a precondition reads',
      dependsOn: [],
      work: work({ kind: 'cancelled' }, cellsAt('implement', 'notYet'), { hours: 12, dollars: 0.9, rounds: 8 }),
    },
    17: {
      title: 'Snap the columns as they scroll',
      dependsOn: [],
      work: work(round('test', 31, 4), cellsAt('test', 'running'), { hours: 4, dollars: 3.75, rounds: 33 }),
    },
    18: {
      title: 'Send an event for each write',
      dependsOn: [20, 21],
      work: work({ kind: 'merged' }, cellsAt(null, 'done', false), { hours: 40, dollars: 4.4, rounds: 37 }),
    },
    19: {
      title: 'List the blocks people write',
      dependsOn: [],
      work: work(round('research', 4), cellsAt('research', 'running'), { hours: 0.1, dollars: 0.12, rounds: 1 }),
    },
    20: {
      title: 'Keep one stream open for each page',
      dependsOn: [],
      work: work(
          {
            kind: 'waits',
            at: 'implement',
            on: [
              {
                kind: 'call',
                call: 12,
                who: 'owner',
                what: { kind: 'question', who: 'owner', question: 'May a page keep its stream open while hidden?' },
              },
              owners(PLAN_CONFIRMED),
            ],
          },
          cellsAt('implement', 'resting'),
          { hours: 2.5, dollars: 2.2, rounds: 19, quiet: 12 },
        ),
    },
    21: {
      title: 'Store the versions of each block',
      dependsOn: [],
      work: work({ kind: 'merged' }, cellsAt(null, 'done'), { hours: 90, dollars: 8.1, rounds: 59 }),
    },
    22: {
      title: 'Try a second layout of the tree',
      dependsOn: [23],
      work: work({ kind: 'cancelled' }, cellsAt('deep_research', 'notYet'), { hours: 60, dollars: 1.4, rounds: 11 }),
    },
    23: {
      title: 'Measure how long a frame of the tree takes',
      dependsOn: [],
      work: work({ kind: 'next', cell: 'research' }, notStarted, { hours: 0.02, dollars: 0, rounds: 0 }),
    },
    24: {
      title: 'Check a setting before it is saved',
      dependsOn: [],
      work: work({ kind: 'fault', words: 'The flow names a block no cell writes' }, notStarted, {
        hours: 0.2,
        dollars: 0,
        rounds: 0,
      }),
    },
    25: {
      title: 'Say which cell a note goes to',
      dependsOn: [],
      work: work({ kind: 'waits', at: 'research', on: [owners(REQUEST_READ)] }, notStarted, {
        hours: 0.5,
        dollars: 0,
        rounds: 0,
      }),
    },
  };
  return {
    items: Object.entries(items).map(
      ([n, m]): Item => ({
        number: Number(n),
        title: m.title,
        added: m.work.added,
        dependsOn: m.dependsOn,
        work: { ...m.work, item: Number(n), flow: FLOW.name },
      }),
    ),
  };
}

/// A work as its page gives it: the details the console does not read are left empty.
function pageOf(item: Item, summary: Summary): WorkPage {
  const cells: CellDetail[] = FLOW.cells.map((c) => ({
    cell: c.id,
    needs: [],
    inputs: c.inputs.map((block) => ({ block, current: null, known: null, changed: false })),
  }));
  return { summary, title: item.title, flow: FLOW, cells, blocks: [], steps: [] };
}

/// The made-up items as a source the console reads: what the owner does changes them, and the console is told.
export function fixtureSource(start = Date.now()): Source {
  const data = fixtureItems(start);
  const listeners = new Set<(c: Changed) => void>();
  const find = (work: number) => {
    const item = data.items.find((i) => i.work.number === work);
    if (item !== undefined) return { item, summary: item.work };
    throw new Error(`There is no work ${work}`);
  };
  const told = (work: number) => {
    const c: Changed = { works: [work], items: true };
    window.setTimeout(() => listeners.forEach((on) => on(c)), 0);
  };
  const change = (s: Summary, what: Act) => {
    const ended = s.now.kind === 'merged' || s.now.kind === 'cancelled';
    switch (what.kind) {
      case 'pause':
        if (ended) throw new Error('The work has ended');
        s.paused = true;
        if (s.now.kind !== 'round') s.now = { kind: 'paused' };
        return;
      case 'resume': {
        if (ended) throw new Error('The work has ended');
        s.paused = false;
        const at = s.cells.find((c) => c.state !== 'done' && c.state !== 'skipped')?.cell ?? ORDER[0];
        if (s.now.kind === 'paused') s.now = { kind: 'next', cell: at };
        return;
      }
      case 'cancel':
        if (ended) throw new Error('The work has ended');
        s.now = { kind: 'cancelled' };
        return;
      case 'write':
        if (!FLOW.blocks.some((b) => b.name === what.block && b.writtenBy.people))
          throw new Error(`People do not write ${what.block}`);
        return;
      default:
        throw new Error(`The made-up items do not take ${what.kind}`);
    }
  };
  return {
    loadItems: () => Promise.resolve(structuredClone(data)),
    loadWork: (work) => {
      try {
        const { item, summary } = find(work);
        return Promise.resolve(structuredClone(pageOf(item, summary)));
      } catch (e) {
        return Promise.reject(e instanceof Error ? e : new Error(String(e)));
      }
    },
    act: (work, what) => {
      try {
        const { summary } = find(work);
        change(summary, what);
        summary.last = Date.now();
        told(work);
        return Promise.resolve(null);
      } catch (e) {
        return Promise.reject(e instanceof Error ? e : new Error(String(e)));
      }
    },
    changes: (on) => {
      listeners.add(on);
      return () => listeners.delete(on);
    },
  };
}
