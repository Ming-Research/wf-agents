// What the page's tests and the works page's fixtures build definitions from: cells written in short, and the flow they make as the interface would
// give it, with who writes and reads each block worked out as the compiler does.

import type { Block, Cell, Flow, Given, Limits, RunsOn, Test, TestKind } from './api';
import { named } from './matrix';

const LIMITS: Limits = { runs: 3, returns: 2, calls: 40 };

/// A cell's fields written in short. What runs it and what it is given are either one list, `inputs`, as a cell in the
/// old form names them: each block of it runs the cell, and an agent's cell is given it, the last and the latest; or
/// the two lists apart, each entry a block's name, which says nothing of why or of its kind but the last and the
/// latest, or the whole entry.
export type CellFields = Omit<Partial<Cell>, 'runsOn' | 'given'> & {
  inputs?: string[];
  runsOn?: (string | RunsOn)[];
  given?: (string | Given)[];
};

/// A cell of an agent in the line `engineer`, told to do its part, that nothing runs and that writes nothing, with
/// these fields over it.
export function cell(id: string, fields: CellFields = {}): Cell {
  const { inputs = [], runsOn, given, ...rest } = fields;
  const agent = (rest.kind ?? 'agent') === 'agent';
  const runs = (b: string | RunsOn): RunsOn => (typeof b === 'string' ? { block: b, why: null } : b);
  const gives = (b: string | Given): Given => (typeof b === 'string' ? { block: b, kind: 'lastAndLatest', why: null } : b);
  return {
    id,
    title: id,
    kind: 'agent',
    line: 'engineer',
    task: `Do the part of ${id}.`,
    act: null,
    effort: 'high',
    takesMain: false,
    condition: null,
    preconditions: [],
    outputs: [],
    mustWrite: [],
    tools: [],
    limits: LIMITS,
    ...rest,
    runsOn: (runsOn ?? (given ? [] : inputs)).map(runs),
    given: (given ?? (agent && !runsOn ? inputs : [])).map(gives),
  };
}

/// A named test of these blocks.
export function test(name: string, words: string, kind: TestKind, reads: string[], is = 'yes'): Test {
  return { name, words, kind, reads, is };
}

/// What a block is besides who of the cells and the tests writes and reads it; `outside` names the source outside
/// the work that writes it.
export type Declared = Partial<{ values: string[]; people: boolean; note: boolean; outside: string; shown: boolean; what: string }>;

/// The flow of these cells in this order, with these blocks, and the named tests the cells use.
export function flowOf(cells: Cell[], blocks: Record<string, Declared>, name = 'small'): Flow {
  const used = cells.flatMap((c) => [...(c.condition?.on === 'test' ? [c.condition.test] : []), ...c.preconditions]);
  const tests = [...new Map(used.map((t) => [t.name, t])).values()].sort((a, b) => (a.name < b.name ? -1 : 1));
  const all: Block[] = Object.keys(blocks)
    .sort()
    .map((b) => ({
      name: b,
      values: blocks[b].values ?? null,
      what: blocks[b].what ?? null,
      writtenBy: {
        cells: cells.filter((c) => c.outputs.includes(b)).map((c) => c.id),
        people: blocks[b].people ?? false,
        note: blocks[b].note ?? false,
        outside: blocks[b].outside ?? null,
      },
      readBy: {
        cells: cells.filter((c) => named(c).includes(b)).map((c) => c.id),
        conditions: cells.filter((c) => c.condition?.on === 'block' && c.condition.block === b).map((c) => c.id),
        tests: tests.filter((t) => t.reads.includes(b)).map((t) => t.name),
        shown: blocks[b].shown ?? false,
      },
    }));
  const lines = [...new Set(cells.flatMap((c) => (c.line ? [c.line] : [])))].sort().map((l) => ({
    name: l,
    cells: cells.filter((c) => c.line === l).map((c) => c.id),
    levelsAbove: 0,
  }));
  return { name, limits: LIMITS, lines, tests, cells, blocks: all };
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
        condition: { on: 'block', block: 'needs_design', value: 'yes' },
        inputs: ['plan'],
        outputs: ['design'],
      }),
      cell('answer', {
        title: 'Answer comments',
        task: 'Answer the comments on the plan.',
        condition: { on: 'test', test: test('has_comment', 'The item has a comment', 'any', ['commented']) },
        preconditions: [test('plan_confirmed', 'The owner confirmed the plan', 'all', ['confirmed'])],
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
        preconditions: [test('may_merge', 'The work may merge', 'none', ['ci'], 'red')],
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
