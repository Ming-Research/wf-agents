// Made-up items for looking at the console with no server behind it (`console.html?fixtures`), and for its tests: some
// thirty items, each with its one work, depending on each other in four layers, their works in every state a work can
// be in, on one made-up flow of an engineer's work; most of the project `wf-agents`, two of another, and one, added
// before items had projects, of none. What the owner does to a work changes it here, and the console is told as the
// interface's stream of changes would tell it.

import type { Block, Cell, Flow, Limits, Test } from '../api';
import type {
  Act,
  CellDetail,
  CellNow,
  CellState,
  Changed,
  Ends,
  Item,
  Items,
  Now,
  Pauser,
  Question,
  Spent,
  Summary,
  Wait,
  Waited,
  WorkPage,
} from '../works-api';
import { outlineOf } from '../testing';
import { workName } from '../works/words';
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
  /// The blocks among its outputs that every hand-in of it includes; none where it is left out.
  mustWrite?: string[];
  condition?: Cell['when'][number];
  preconditions?: Test[];
};

/// The flow of these cells in this order, with these blocks, who writes and reads each worked out as the compiler does.
function flowOf(
  name: string,
  cells: CellIn[],
  blocks: Record<string, { values?: string[]; people?: boolean; note?: boolean; outside?: string; shown?: boolean }>,
): Flow {
  const all: Cell[] = cells.map((c) => ({
    id: c.id,
    title: c.title,
    kind: c.line === null ? 'engine' : 'agent',
    line: c.line === undefined ? 'builder' : c.line,
    task: c.task ?? null,
    taskFile: null,
    act: c.act ?? null,
    effort: c.line === null ? null : 'high',
    takesMain: false,
    tools: [],
    when: c.condition ? [c.condition] : [],
    requires: (c.preconditions ?? []).map((test) => ({ on: 'test', test })),
    watches: c.inputs.map((b) => ({ name: b, blocks: [b], do: null })),
    reads: [],
    given:
      c.line === null ? [] : c.inputs.map((b) => ({ block: b, kind: 'lastAndLatest' as const, list: 'watches' as const })),
    outputs: c.outputs,
    mustWrite: c.mustWrite ?? [],
    limits: LIMITS,
  }));
  const tests = [
    ...new Map(
      all.flatMap((c) => c.requires.flatMap((k) => (k.on === 'test' ? [k.test] : []))).map((t) => [t.name, t]),
    ).values(),
  ];
  const named: Block[] = Object.keys(blocks)
    .sort()
    .map((b) => ({
      name: b,
      values: blocks[b].values ?? null,
      default: null,
      what: null,
      writtenBy: {
        cells: all.filter((c) => c.outputs.includes(b)).map((c) => c.id),
        people: blocks[b].people ?? false,
        note: blocks[b].note ?? false,
        outside: blocks[b].outside ?? null,
      },
      readBy: {
        cells: all.filter((c) => c.watches.some((w) => w.blocks.includes(b))).map((c) => c.id),
        conditions: all.filter((c) => c.when.some((k) => k.on === 'block' && k.block === b)).map((c) => c.id),
        tests: tests.filter((t) => t.reads.includes(b)).map((t) => t.name),
        shown: blocks[b].shown ?? false,
      },
    }));
  const lines = [...new Set(all.flatMap((c) => (c.line ? [c.line] : [])))].sort().map((l) => ({
    name: l,
    cells: all.filter((c) => c.line === l).map((c) => c.id),
    levelsAbove: 0,
    file: `lines/${l}.toml`,
    readsOnly: false,
    system: [],
    rules: [],
  }));
  return {
    name,
    limits: LIMITS,
    lines,
    tests,
    tools: [],
    groups: [],
    card: [],
    everyAgent: { reads: [], kinds: {} },
    cells: all,
    blocks: named,
  };
}

/// A named test that holds when each of these blocks, which the owner sets, says yes.
const test = (name: string, words: string, reads: string[]): Test => ({
  name,
  words,
  file: `tests/${name}.toml`,
  kind: 'all',
  reads,
  is: 'yes',
});

const PLAN_CONFIRMED = test('plan_confirmed', 'The owner confirmed the plan', ['plan_confirmed']);
const REQUEST_READ = test('request_read', 'The owner read what the item asks for', ['request_read']);
const MAY_MERGE = test('may_merge', 'The reviews are content and the owner lets it merge', ['content', 'merge_allowed']);

/// A precondition not met that the owner makes hold.
const owners = (t: Test): Waited => ({ kind: 'precondition', test: t.name, words: t.words, by: [{ kind: 'owner' }] });

/// What the engine says a work waits on the owner for, made up in the words beside the flows.
const wait = (key: string, why: string, ends: Ends): Wait => ({ key, why, ends });

/// The block a test reads that the owner writes, keeping the cell's `requires` from holding, as the engine says it.
const ruling = (t: Test, block: string): Wait =>
  wait(`write-${block}`, `\`${block}\` keeps \`test = ${t.name}\` from holding.`, {
    kind: 'write',
    block,
    value: 'yes',
  });

/// A made-up flow of an engineer's work: the initial research, deep research when it asks for it, the review of its
/// plan, the implementation once the owner confirmed the plan, the tests, the review of the code, a check with the
/// latest main, and the merge.
export const FLOW: Flow = flowOf(
  'building',
  [
    {
      id: 'research',
      title: 'Initial research',
      task: 'Research what the item asks for, and write the plan.\nSay whether it needs deep research.',
      preconditions: [REQUEST_READ],
      inputs: ['request', 'plan_opinions'],
      outputs: ['outline', 'needs_deep_research'],
    },
    {
      id: 'deep_research',
      title: 'Deep research',
      line: 'deep_research',
      task: 'Research what the plan leaves open, and write the solution.',
      condition: { on: 'block', block: 'needs_deep_research', value: 'yes' },
      inputs: ['outline', 'plan_review_on_deep_research'],
      outputs: ['solution'],
    },
    {
      id: 'review_plan',
      title: 'Review the plan',
      task: 'Review the solution and the plan against what the item asks for.',
      line: 'judge',
      condition: { on: 'block', block: 'needs_deep_research', value: 'yes' },
      inputs: ['outline', 'solution'],
      outputs: ['plan_review_on_deep_research'],
    },
    {
      id: 'implement',
      title: 'Implement',
      task: 'Implement the plan and the solution.',
      preconditions: [PLAN_CONFIRMED],
      inputs: ['outline', 'solution', 'code_opinions'],
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
      id: 'review_code',
      title: 'Review the code',
      task: 'Review the code and its tests, and say whether you are content.',
      line: 'judge',
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
    outline: { shown: true },
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
      const open = state === 'running' || state === 'resting' || state === 'calling' || state === 'failed' ? 40 + i : null;
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

/// Where made-up works' time went, one for each in turn.
const BARS = [
  'mmmmmmtt..mmmmmmmmtttt....mmmmmm',
  'mmm.....mmmmmmmmmmtttmmmmmm..mmm',
  'mmmmtttttttt....mmmmmmmmm...mmmm',
];

/// Whether a made-up work has begun: a round is under way, or a cell of it has run.
const begun = (now: Now, cells: CellNow[]) =>
  now.kind === 'round' || cells.some((c) => c.state !== 'notYet' && c.state !== 'skipped');

/// A made-up item: its project is `wf-agents` where it names none.
type Made = { title: string; project?: string | null; dependsOn: number[]; work: Omit<Summary, 'item' | 'flow'> };

/// The made-up items, as they stand when the page opens; `start` is the time it is taken from.
export function fixtureItems(start = Date.now()): Items {
  let next = 1;
  /// A work added `hours` ago, last touched `quiet` minutes ago.
  const work = (
    now: Now,
    cells: CellNow[],
    o: { hours?: number; dollars?: number; rounds?: number; quiet?: number; paused?: Pauser; waits?: Wait[] } = {},
  ): Made['work'] => {
    const hours = o.hours ?? 2;
    return {
      number: next++,
      added: start - hours * HOUR,
      paused: o.paused ?? null,
      now,
      cells,
      waits: o.waits ?? [],
      spent: spentOf(o.dollars ?? 1.2, Math.min(hours, 1.5), o.rounds ?? 9),
      last: start - (o.quiet ?? 3) * MINUTE,
      // Its first round a little after it was added, and where its time went since, made up; none before it began.
      ...(begun(now, cells)
        ? { from: start - hours * HOUR * 0.9, bar: BARS[next % BARS.length] }
        : { from: null, bar: '' }),
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
        { kind: 'waits', at: 'research', on: [{ kind: 'answer', question: 'stale-runs' }] },
        cellsAt('research', 'resting'),
        {
          hours: 9,
          dollars: 0.84,
          rounds: 4,
          quiet: 95,
          waits: [
            wait('answer-stale-runs', 'Should a stale run be abandoned at once or kept a day?', {
              kind: 'answer',
              question: 'stale-runs',
              version: 2,
            }),
          ],
        },
      ),
    },
    2: {
      title: 'See every work at one glance',
      dependsOn: [8, 9, 22],
      work: work(round('review_code', 6), cellsAt('review_code', 'running'), { hours: 30, dollars: 7.31, rounds: 61 }),
    },
    3: {
      title: 'Keep what each run costs',
      dependsOn: [11, 12],
      work: work({ kind: 'paused', at: ORDER[0], by: 'owner', why: null }, notStarted, {
        hours: 1,
        dollars: 0,
        rounds: 0,
        paused: 'owner',
      }),
    },
    4: {
      title: 'Let the owner write to any cell',
      dependsOn: [13, 9],
      // Paused by the engine: the implementation's last round failed.
      work: work(
        {
          kind: 'paused',
          at: 'implement',
          by: 'engine',
          why: "the agent's program exited with an error: the session could not be resumed",
        },
        cellsAt('implement', 'failed'),
        {
          hours: 50,
          dollars: 3.02,
          rounds: 22,
          paused: 'engine',
          waits: [
            wait(
              'pause',
              "The engine paused the work. The agent's program exited with an error: the session could not be resumed.",
              { kind: 'resume' },
            ),
          ],
        },
      ),
    },
    5: {
      title: 'Restart a lost session',
      dependsOn: [14, 15],
      work: work(round('implement', 18, 3), cellsAt('implement', 'running', false), {
          hours: 0.7,
          dollars: 2.4,
          rounds: 17,
          paused: 'owner',
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
          { hours: 3, dollars: 1.9, rounds: 14, quiet: 40, waits: [ruling(PLAN_CONFIRMED, 'plan_confirmed')] },
        ),
    },
    8: {
      title: 'Draw the items as a tree',
      project: 'Whitefoot',
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
              { kind: 'call', call: 3, who: null, what: { kind: 'tool', name: 'ci', input: 'all' } },
            ],
          },
          cellsAt('research', 'calling'),
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
          waits: [
            wait('limit', 'The cell ran 3 times in a row for what a cell after it wrote, which is its limit.', {
              kind: 'letGo',
              cell: 'review_plan',
            }),
          ],
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
          { hours: 7, dollars: 5.05, rounds: 44, quiet: 25, waits: [ruling(MAY_MERGE, 'merge_allowed')] },
        ),
    },
    16: {
      title: 'Name the tests a precondition reads',
      dependsOn: [],
      work: work({ kind: 'cancelled' }, cellsAt('implement', 'notYet'), { hours: 12, dollars: 0.9, rounds: 8 }),
    },
    17: {
      title: 'Snap the columns as they scroll',
      project: 'Whitefoot',
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
              { kind: 'call', call: 12, who: 'owner', what: { kind: 'atLimit', limit: { of: 'returns', sentBack: 2 } } },
              owners(PLAN_CONFIRMED),
            ],
          },
          cellsAt('implement', 'resting'),
          {
            hours: 2.5,
            dollars: 2.2,
            rounds: 19,
            quiet: 12,
            waits: [
              wait('limit', "The cell's run had 2 returns sent back, which is its limit.", { kind: 'goOn', call: 12 }),
              ruling(PLAN_CONFIRMED, 'plan_confirmed'),
            ],
          },
        ),
    },
    21: {
      title: 'Store the versions of each block',
      // Added before items had projects.
      project: null,
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
        waits: [ruling(REQUEST_READ, 'request_read')],
      }),
    },
    // Items that ended, each under the item that depends on it, and folds. 26 is open, its work not begun; under it
    // 27, done, folded as nothing under it is open; 28, open, unfolded over 32, whose work runs; and 29, cancelled,
    // folded over 33, done, and 34, cancelled. 35 is cancelled, and no item depends on it.
    26: {
      title: 'Fold the items under an item',
      dependsOn: [27, 28, 29],
      work: work({ kind: 'next', cell: 'research' }, notStarted, { hours: 40, dollars: 0, rounds: 0 }),
    },
    27: {
      title: 'Count the items under a fold',
      dependsOn: [30, 31],
      work: work({ kind: 'merged' }, cellsAt(null, 'done', false), { hours: 30, dollars: 3.4, rounds: 25 }),
    },
    28: {
      title: 'Keep a cancelled item where it stood',
      dependsOn: [32],
      work: work({ kind: 'next', cell: 'research' }, notStarted, { hours: 6, dollars: 0, rounds: 0 }),
    },
    29: {
      title: 'Hide the cancelled roots',
      dependsOn: [33, 34],
      work: work({ kind: 'cancelled' }, cellsAt('deep_research', 'notYet'), { hours: 50, dollars: 0.6, rounds: 4 }),
    },
    30: {
      title: 'Draw a fold as a dot',
      dependsOn: [],
      work: work({ kind: 'merged' }, cellsAt(null, 'done', false), { hours: 36, dollars: 2.1, rounds: 18 }),
    },
    31: {
      title: 'Keep what the owner folds',
      dependsOn: [],
      work: work({ kind: 'merged' }, cellsAt(null, 'done'), { hours: 34, dollars: 1.7, rounds: 15 }),
    },
    32: {
      title: 'Keep the dependencies on a cancelled item',
      dependsOn: [],
      work: work(round('implement', 9), cellsAt('implement', 'running'), { hours: 5, dollars: 2.6, rounds: 21 }),
    },
    33: {
      title: 'List the cancelled items apart',
      dependsOn: [],
      work: work({ kind: 'merged' }, cellsAt(null, 'done', false), { hours: 60, dollars: 1.2, rounds: 10 }),
    },
    34: {
      title: 'Show the cancelled items in a column of their own',
      dependsOn: [],
      work: work({ kind: 'cancelled' }, cellsAt('research', 'notYet'), { hours: 70, dollars: 0.2, rounds: 2 }),
    },
    35: {
      title: 'Try the tree as a flat list',
      dependsOn: [],
      work: work({ kind: 'cancelled' }, cellsAt('research', 'notYet'), { hours: 80, dollars: 0.3, rounds: 2 }),
    },
  };
  return {
    items: Object.entries(items).map(
      ([n, m]): Item => ({
        number: Number(n),
        title: m.title,
        level: '2',
        project: m.project === undefined ? 'wf-agents' : m.project,
        added: m.work.added,
        dependsOn: m.dependsOn,
        history: { first: { title: m.title, level: '2', dependsOn: m.dependsOn }, changes: [] },
        work: { ...m.work, item: Number(n), flow: FLOW.name },
      }),
    ),
  };
}

/// The questions put to the owner, by the item whose work they are of: one the work waits for at its cell, and one it
/// went on past, still to confirm.
const QUESTIONS: Record<number, Question[]> = {
  1: [
    {
      name: 'stale-runs',
      cell: 'research',
      version: 2,
      engine: false,
      waits: true,
      tree: true,
      withdrawn: null,
      ask: 'Should a stale run be abandoned at once or kept a day?',
      options: [
        { does: 'Abandon it at once', follows: 'Its round is stopped and its cell runs again' },
        { does: 'Keep it a day', follows: 'It is abandoned the next day, unless it goes on' },
      ],
      recommends: 1,
      answer: null,
    },
    {
      name: 'stale-limit',
      cell: 'research',
      version: 1,
      engine: false,
      waits: false,
      tree: false,
      withdrawn: null,
      ask: 'How long may a run rest before it is stale?',
      options: [
        { does: 'An hour', follows: 'Most runs that rest longer were lost' },
        { does: 'A day', follows: 'A run waiting for a person is left alone overnight' },
      ],
      recommends: 2,
      answer: null,
    },
  ],
};

/// How many of `questions` stand unanswered and hold the work at nothing that `waits` lists, as the engine counts a
/// work's questions the owner may answer ahead (`Summary.ahead`).
function aheadOf(questions: Question[], waits: Wait[]): number {
  const open = (q: Question) => q.withdrawn === null && q.ask !== null && q.version !== null && q.answer === null;
  return questions.filter((q) => open(q) && !waits.some((w) => w.key === `answer-${q.name}`)).length;
}

/// A work as its page gives it: the details the console does not read are left empty.
function pageOf(item: Item, summary: Summary, questions: Record<number, Question[]>): WorkPage {
  const cells: CellDetail[] = FLOW.cells.map((c) => ({
    cell: c.id,
    needs: [],
    inputs: c.watches.flatMap((w) => w.blocks).map((block) => ({ block, current: null, known: null, changed: false })),
  }));
  return {
    summary,
    title: item.title,
    level: item.level,
    project: item.project,
    history: item.history,
    outline: outlineOf(FLOW),
    flow: FLOW,
    cells,
    blocks: [],
    steps: [],
    questions: questions[item.number] ?? [],
  };
}

/// The made-up items as a source the console reads: what the owner does changes them, and the console is told.
export function fixtureSource(start = Date.now()): Source {
  const data = fixtureItems(start);
  const questions = structuredClone(QUESTIONS);
  const listeners = new Set<(c: Changed) => void>();
  const find = (work: number) => {
    const item = data.items.find((i) => i.work.number === work);
    if (item !== undefined) return { item, summary: item.work };
    throw new Error(`There is no work ${workName(work)}`);
  };
  const told = (work: number) => {
    const c: Changed = { works: [work], items: true, copies: false, accounts: false, observations: false };
    window.setTimeout(() => listeners.forEach((on) => on(c)), 0);
  };
  const change = (s: Summary, what: Act) => {
    const ended = s.now.kind === 'merged' || s.now.kind === 'cancelled';
    switch (what.kind) {
      case 'answer': {
        // The answer is kept; a question the work waited for at its cell holds it no more, and the cell runs.
        const q = (questions[s.item] ?? []).find((x) => x.name === what.question && x.version === what.version);
        if (q === undefined) throw new Error(`There is no question ${what.question} of version ${what.version}`);
        q.answer = { option: what.option, at: Date.now(), why: null };
        s.waits = (s.waits ?? []).filter((w) => w.key !== `answer-${q.name}`);
        if (s.now.kind === 'waits' && s.now.on.some((w) => w.kind === 'answer' && w.question === q.name))
          s.now = { kind: 'next', cell: q.cell };
        return;
      }
      case 'pause':
        if (ended) throw new Error('The work has ended');
        s.paused = 'owner';
        if (s.now.kind !== 'round') {
          const now = s.now;
          const at = now.kind === 'next' ? now.cell : 'at' in now && typeof now.at === 'string' ? now.at : ORDER[0];
          s.now = { kind: 'paused', at, by: 'owner', why: null };
        }
        return;
      case 'resume': {
        if (ended) throw new Error('The work has ended');
        s.paused = null;
        s.waits = (s.waits ?? []).filter((w) => w.ends.kind !== 'resume');
        // A cell whose last round failed has that round again: its run is at rest until it begins.
        s.cells = s.cells.map((c) => (c.state === 'failed' ? { ...c, state: 'resting' } : c));
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
        s.waits = (s.waits ?? []).filter((w) => w.key !== `write-${what.block}`);
        return;
      default:
        throw new Error(`The made-up items do not take ${what.kind}`);
    }
  };
  return {
    items: {
      // Each work with the questions the owner may answer ahead counted as the engine counts them, from the questions
      // as they stand now.
      loadItems: () => {
        for (const i of data.items) i.work.ahead = aheadOf(questions[i.number] ?? [], i.work.waits ?? []);
        return Promise.resolve(structuredClone(data));
      },
      changes: (onChanged) => {
        listeners.add(onChanged);
        return () => listeners.delete(onChanged);
      },
    },
    loadWork: (work) => {
      try {
        const { item, summary } = find(work);
        return Promise.resolve(structuredClone(pageOf(item, summary, questions)));
      } catch (e) {
        return Promise.reject(e instanceof Error ? e : new Error(String(e)));
      }
    },
    // The made-up works have no block with a version.
    loadVersion: (work, block, number) =>
      Promise.reject(new Error(`${workName(work)} has no version ${number} of ${block}.`)),
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
