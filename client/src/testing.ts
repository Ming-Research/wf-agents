// What the page's tests and the works page's fixtures build definitions from: cells written in short, and the flow they make as the interface would
// give it, with who writes and reads each block worked out as the compiler does.

import type { Block, Cell, Condition, Flow, Input, Limits, Line, Test, TestKind } from './api';

const LIMITS: Limits = { runs: 3, returns: 2, calls: 40 };

/// A cell of an agent in the line `engineer`, told to do its part, that reads and writes nothing, with these fields over
/// it. What runs it and what it is given are worked out as the compiler does: from `runsOn` and `given` where they are
/// set, and otherwise from `inputs` as from a file in the old form, each block of it running the cell and given to an
/// agent's cell; `inputs` and `input` follow from them, `input` with what `fields` says over it. Each block that runs
/// it does so by a trigger named apart from the block, `on_` and its name, as the flow's triggers are.
export function cell(id: string, fields: Partial<Cell> = {}): Cell {
  const kind = fields.kind ?? 'agent';
  const named = fields.inputs ?? [];
  const runsOn = fields.runsOn ?? (fields.given ? [] : named);
  const given = fields.given ?? (fields.runsOn || kind !== 'agent' ? [] : named);
  const inputs = [...new Set([...runsOn, ...given])];
  const input: Record<string, Input> = Object.fromEntries(
    inputs.map((b) => [
      b,
      {
        runs: runsOn.includes(b) ? { why: null } : null,
        given: given.includes(b) ? { kind: null, why: null } : null,
      },
    ]),
  );
  return {
    id,
    title: id,
    kind,
    line: 'engineer',
    task: `Do the part of ${id}.`,
    taskFile: null,
    act: null,
    effort: 'high',
    takesMain: false,
    tools: [],
    triggers: runsOn.map((b) => ({ name: `on_${b}`, what: null, block: b, read: [], do: null })),
    when: [],
    requires: [],
    outputs: [],
    mustWrite: [],
    limits: LIMITS,
    ...fields,
    inputs,
    runsOn,
    given,
    input: { ...input, ...fields.input },
  };
}

/// Keys of a cell's `when` or `requires`, each one of these named tests.
export function tested(...tests: Test[]): Condition[] {
  return tests.map((test) => ({ on: 'test', test }));
}

/// A named test of these blocks, in a file of its own.
export function test(name: string, words: string, kind: TestKind, reads: string[], is = 'yes'): Test {
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
/// its own, with what `lines` writes of it; the flow's tools those its cells may ask for.
export function flowOf(
  cells: Cell[],
  blocks: Record<string, Declared>,
  name = 'small',
  lines: Record<string, Partial<Line>> = {},
): Flow {
  const keys = (c: Cell) => [...c.when, ...c.requires];
  const used = cells.flatMap((c) => keys(c).flatMap((k) => (k.on === 'test' ? [k.test] : [])));
  const tests = [...new Map(used.map((t) => [t.name, t])).values()].sort((a, b) => (a.name < b.name ? -1 : 1));
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
        cells: cells.filter((c) => c.inputs.includes(b)).map((c) => c.id),
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
  return { name, limits: LIMITS, lines: declared, tests, tools, cells, blocks: all };
}

/// The small flow of `crates/web/tests/flows/small`: a plan a review can send back, a design when the review asks for
/// one, answers to comments, a check with the latest main and the merge.
export function small(): Flow {
  return flowOf(
    [
      cell('plan', {
        title: 'Plan',
        task: 'Write the plan from the request.\nSay what it leaves out.',
        inputs: ['request', 'opinions'],
        outputs: ['plan'],
      }),
      cell('judge', {
        title: 'Judge the plan',
        line: 'review',
        task: 'Judge the plan, and say whether it needs a design.',
        inputs: ['plan'],
        outputs: ['needs_design', 'opinions'],
      }),
      cell('design', {
        title: 'Design',
        task: 'Design what the plan asks for.',
        when: [{ on: 'block', block: 'needs_design', value: 'yes' }],
        inputs: ['plan'],
        outputs: ['design'],
      }),
      cell('answer', {
        title: 'Answer comments',
        task: 'Answer the comments on the plan.',
        when: tested(test('has_comment', 'The item has a comment', 'any', ['commented'])),
        requires: tested(test('plan_confirmed', 'The owner confirmed the plan', 'all', ['confirmed'])),
        inputs: ['comments', 'plan'],
        outputs: ['reply'],
        limits: { ...LIMITS, runs: 5 },
      }),
      cell('check', {
        title: 'Check with the latest main',
        kind: 'engine',
        line: null,
        task: null,
        act: 'check',
        inputs: ['main', 'design'],
        outputs: ['ci'],
      }),
      cell('merge', {
        title: 'Merge',
        kind: 'engine',
        line: null,
        task: null,
        act: 'merge',
        requires: tested(test('may_merge', 'The work may merge', 'none', ['ci'], 'red')),
        inputs: ['ci'],
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
