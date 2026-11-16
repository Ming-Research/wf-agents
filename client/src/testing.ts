// What the page's tests and the works page's fixtures build definitions from: cells written in short, and the flow they
// make as the interface would give it, with who writes and reads each block, and what each cell is given, worked out as
// the compiler does.

import type {
  Block,
  Cell,
  Condition,
  EveryAgent,
  Flow,
  GivenBlock,
  Group,
  InputKind,
  Limits,
  Line,
  Read,
  Test,
  TestKind,
  Watch,
} from './api';

const LIMITS: Limits = { runs: 3, returns: 2, calls: 40 };

/// A name a cell's file writes in `watches` or `reads`, in short: a datum by its name, or a group with its blocks.
export type Named = string | { group: string; blocks: string[] };

/// A cell written in short: the fields of a cell over it, and what its file names, in short.
export type CellIn = Omit<Partial<Cell>, 'watches' | 'reads' | 'given'> & {
  /// What its file's `watches` names, in order.
  watches?: Named[];
  /// What its file's `reads` names, in order.
  reads?: Named[];
  /// What it does when a name it watches is new (`[do]`), by the name.
  does?: Record<string, string>;
  /// The kind of each block it is given (`[kinds]`), by the block; the last version it worked from and the latest
  /// where none is said.
  kinds?: Record<string, InputKind>;
};

/// A name as the route gives it: a datum stands for itself, a group for its blocks.
function taken(n: Named): { name: string; blocks: string[] } {
  return typeof n === 'string' ? { name: n, blocks: [n] } : { name: n.group, blocks: n.blocks };
}

/// A cell of an agent in the line `builder`, told to do its part, that watches, reads and writes nothing, with these
/// fields over it. What it is given is worked out as the compiler does: an agent's cell is given what its file reads,
/// then what it watches, each block once, in the kind its `kinds` says; an engine's cell is given nothing. What every
/// agent cell is given is added by [`flowOf`].
export function cell(id: string, fields: CellIn = {}): Cell {
  const { watches = [], reads = [], does = {}, kinds = {}, ...rest } = fields;
  const kind = rest.kind ?? 'agent';
  const watched: Watch[] = watches.map((n) => ({ ...taken(n), do: does[taken(n).name] ?? null, list: 'watches' as const }));
  const read: Read[] = reads.map(taken);
  const given: GivenBlock[] = [];
  if (kind === 'agent') {
    for (const [list, names] of [
      ['reads', read],
      ['watches', watched],
    ] as const) {
      for (const block of names.flatMap((n) => n.blocks)) {
        if (!given.some((g) => g.block === block)) {
          given.push({ block, kind: kinds[block] ?? 'lastAndLatest', list });
        }
      }
    }
  }
  return {
    id,
    title: id,
    kind,
    line: 'builder',
    task: `Do the part of ${id}.`,
    taskFile: null,
    act: null,
    effort: 'high',
    takesMain: false,
    tools: [],
    when: [],
    requires: [],
    outputs: [],
    mustWrite: [],
    limits: LIMITS,
    ...rest,
    watches: watched,
    reads: read,
    given,
  };
}

/// The blocks a cell's own file names, in what it watches and what it reads.
export function named(c: Cell): string[] {
  const own = c.watches.filter((w) => w.list === 'watches');
  return [...new Set([...own, ...c.reads].flatMap((n) => n.blocks))];
}

/// What `[every_agent]` names, in short: each name it watches and reads, with the data it stands for, and the kind of
/// each and what a cell does when one it watches is new, by the name.
export function everyAgent(fields: {
  watches?: [string, string[]][];
  reads?: [string, string[]][];
  kinds?: Record<string, InputKind>;
  does?: Record<string, string>;
}): EveryAgent {
  const { watches = [], reads = [], kinds = {}, does = {} } = fields;
  return {
    watches: watches.map(([name, blocks]) => ({ name, blocks, do: does[name] ?? null, list: 'every_agent' as const })),
    reads: reads.map(([name, blocks]) => ({ name, blocks })),
    kinds,
  };
}

/// What a flow with no `[every_agent]` names.
export const NO_EVERY_AGENT: EveryAgent = { watches: [], reads: [], kinds: {} };

/// Keys of a cell's `when` or `requires`, each one of these named tests.
export function tested(...tests: Test[]): Condition[] {
  return tests.map((test) => ({ on: 'test', test }));
}

/// A named test of these blocks, in a file of its own.
export function test(name: string, words: string, kind: TestKind, reads: string[], is: string | null = 'yes'): Test {
  return { name, words, file: `tests/${name}.toml`, kind, reads, is };
}

/// What a block is besides who of the cells and the tests writes and reads it; `outside` names the source outside
/// the work that writes it.
export type Declared = Partial<{
  values: string[];
  default: string;
  people: boolean;
  note: boolean;
  outside: string;
  shown: boolean;
  what: string;
}>;

/// The flow of these cells in this order, with these blocks, and the named tests the cells use; each line in a file of
/// its own, with what `lines` writes of it; the flow's tools those its cells may ask for, and its groups those its
/// cells name; and what `[every_agent]` names, which every agent cell watches and is given first, each block its own
/// file does not name, by the block's name, in the kind `[every_agent]` gives the name that stands for it.
export function flowOf(
  given: Cell[],
  blocks: Record<string, Declared>,
  name = 'small',
  lines: Record<string, Partial<Line>> = {},
  every: EveryAgent = NO_EVERY_AGENT,
): Flow {
  const cells = given.map((c): Cell => {
    if (c.kind !== 'agent') return c;
    const own = named(c);
    const alike: GivenBlock[] = [];
    const runs: Watch[] = [];
    const fresh = (blocks: string[]) => blocks.filter((b) => !own.includes(b) && !alike.some((g) => g.block === b));
    for (const n of every.watches) {
      for (const block of fresh(n.blocks)) {
        alike.push({ block, kind: every.kinds[n.name] ?? 'lastAndLatest', list: 'every_agent' });
        runs.push({ name: block, blocks: [block], do: n.do, list: 'every_agent' });
      }
    }
    for (const n of every.reads) {
      for (const block of fresh(n.blocks)) {
        alike.push({ block, kind: every.kinds[n.name] ?? 'lastAndLatest', list: 'every_agent' });
      }
    }
    return { ...c, watches: [...runs, ...c.watches], given: [...alike, ...c.given] };
  });
  const keys = (c: Cell) => [...c.when, ...c.requires];
  const used = cells.flatMap((c) => keys(c).flatMap((k) => (k.on === 'test' ? [k.test] : [])));
  const tests = [...new Map(used.map((t) => [t.name, t])).values()].sort((a, b) => (a.name < b.name ? -1 : 1));
  const takes = (c: Cell, b: string) => c.given.some((g) => g.block === b) || c.watches.some((w) => w.blocks.includes(b));
  const all: Block[] = Object.keys(blocks)
    .sort()
    .map((b) => ({
      name: b,
      values: blocks[b].values ?? null,
      default: blocks[b].default ?? null,
      what: blocks[b].what ?? null,
      writtenBy: {
        cells: cells.filter((c) => c.outputs.includes(b)).map((c) => c.id),
        people: blocks[b].people ?? false,
        note: blocks[b].note ?? false,
        outside: blocks[b].outside ?? null,
      },
      readBy: {
        cells: cells.filter((c) => takes(c, b)).map((c) => c.id),
        conditions: cells.filter((c) => keys(c).some((k) => k.on === 'block' && k.block === b)).map((c) => c.id),
        tests: tests.filter((t) => t.reads.includes(b)).map((t) => t.name),
        shown: blocks[b].shown ?? false,
      },
    }));
  const declared = [...new Set(cells.flatMap((c) => (c.line ? [c.line] : [])))].sort().map(
    (l): Line => ({
      name: l,
      cells: cells.filter((c) => c.line === l).map((c) => c.id),
      levelsAbove: 0,
      file: `lines/${l}.toml`,
      readsOnly: false,
      system: [],
      rules: [],
      ...lines[l],
    }),
  );
  const tools = [...new Set(cells.flatMap((c) => c.tools))];
  const groups: Group[] = [];
  for (const n of cells.flatMap((c) => [...c.watches.filter((w) => w.list === 'watches'), ...c.reads])) {
    const group = n.blocks.length !== 1 || n.blocks[0] !== n.name;
    if (group && !groups.some((g) => g.name === n.name)) groups.push({ name: n.name, blocks: n.blocks });
  }
  return {
    name,
    limits: LIMITS,
    lines: declared,
    tests,
    tools,
    groups,
    card: [],
    everyAgent: every,
    cells,
    blocks: all,
  };
}

/// The small flow of `crates/web/tests/flows/small`: a plan a review can send back, a design when the review asks for
/// one, answers to comments, a check with the latest main and the merge.
export function small(): Flow {
  return flowOf(
    [
      cell('plan', {
        title: 'Plan',
        task: 'Write the plan from the request.\nSay what it leaves out.',
        watches: ['request', 'opinions'],
        outputs: ['plan'],
      }),
      cell('judge', {
        title: 'Judge the plan',
        line: 'review',
        task: 'Judge the plan, and say whether it needs a design.',
        watches: ['plan'],
        outputs: ['needs_design', 'opinions'],
      }),
      cell('design', {
        title: 'Design',
        task: 'Design what the plan asks for.',
        when: [{ on: 'block', block: 'needs_design', value: 'yes' }],
        watches: ['plan'],
        outputs: ['design'],
      }),
      cell('answer', {
        title: 'Answer comments',
        task: 'Answer the comments on the plan.',
        when: tested(test('has_comment', 'The item has a comment', 'any', ['commented'])),
        requires: tested(test('plan_confirmed', 'The owner confirmed the plan', 'all', ['confirmed'])),
        watches: ['comments', 'plan'],
        outputs: ['reply'],
        limits: { ...LIMITS, runs: 5 },
      }),
      cell('check', {
        title: 'Check with the latest main',
        kind: 'engine',
        line: null,
        task: null,
        act: 'check',
        watches: ['main', 'design'],
        outputs: ['ci'],
      }),
      cell('merge', {
        title: 'Merge',
        kind: 'engine',
        line: null,
        task: null,
        act: 'merge',
        requires: tested(test('may_merge', 'The work may merge', 'none', ['ci'], 'red')),
        watches: ['ci'],
      }),
    ],
    {
      request: { people: true },
      plan: {},
      opinions: { people: true },
      needs_design: { values: ['yes', 'no'] },
      design: {},
      comments: { people: true },
      reply: { shown: true },
      main: { outside: 'main' },
      ci: { values: ['green', 'red'] },
      commented: { values: ['yes', 'no'], people: true },
      confirmed: { values: ['yes', 'no'], people: true },
    },
  );
}
