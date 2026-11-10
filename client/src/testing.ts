// What the page's tests build definitions from: cells written in short, and the flow they make as the interface would
// give it, with who writes and reads each block worked out as the compiler does.

import type { Block, Cell, Flow, Limits } from './api';

const LIMITS: Limits = { runs: 3, returns: 2, calls: 40 };

/// A cell of an agent in the line `engineer` that reads and writes nothing, with these fields over it.
export function cell(id: string, fields: Partial<Cell> = {}): Cell {
  return {
    id,
    title: id,
    kind: 'agent',
    line: 'engineer',
    condition: null,
    preconditions: [],
    inputs: [],
    outputs: [],
    limits: LIMITS,
    ...fields,
  };
}

/// What a block is besides who of the cells writes and reads it.
export type Declared = Partial<{ values: string[]; people: boolean; outside: boolean; shown: boolean }>;

/// The flow of these cells in this order, with these blocks.
export function flowOf(cells: Cell[], blocks: Record<string, Declared>, name = 'small'): Flow {
  const all: Block[] = Object.keys(blocks)
    .sort()
    .map((b) => ({
      name: b,
      values: blocks[b].values ?? null,
      writtenBy: {
        cells: cells.filter((c) => c.outputs.includes(b)).map((c) => c.id),
        people: blocks[b].people ?? false,
        outside: blocks[b].outside ?? false,
      },
      readBy: {
        cells: cells.filter((c) => c.inputs.includes(b)).map((c) => c.id),
        conditions: cells.filter((c) => c.condition?.on === 'block' && c.condition.block === b).map((c) => c.id),
        shown: blocks[b].shown ?? false,
      },
    }));
  const lines = [...new Set(cells.flatMap((c) => (c.line ? [c.line] : [])))].sort().map((l) => ({
    name: l,
    cells: cells.filter((c) => c.line === l).map((c) => c.id),
  }));
  return { name, limits: LIMITS, lines, tests: [], cells, blocks: all };
}

/// The small flow of `crates/web/tests/flows/small`: a plan a review can send back, a design when the review asks for
/// one, answers to comments, a check with the latest main and the merge.
export function small(): Flow {
  return flowOf(
    [
      cell('plan', { title: 'Plan', inputs: ['request', 'opinions'], outputs: ['plan'] }),
      cell('judge', { title: 'Judge the plan', line: 'review', inputs: ['plan'], outputs: ['needs_design', 'opinions'] }),
      cell('design', {
        title: 'Design',
        condition: { on: 'block', block: 'needs_design', value: 'yes' },
        inputs: ['plan'],
        outputs: ['design'],
      }),
      cell('answer', {
        title: 'Answer comments',
        condition: { on: 'test', test: 'has_comment', words: 'The item has a comment' },
        preconditions: [{ name: 'plan_confirmed', words: 'The owner confirmed the plan' }],
        inputs: ['comments', 'plan'],
        outputs: ['reply'],
        limits: { ...LIMITS, runs: 5 },
      }),
      cell('check', { title: 'Check with the latest main', kind: 'engine', line: null, inputs: ['main', 'design'], outputs: ['ci'] }),
      cell('merge', {
        title: 'Merge',
        kind: 'engine',
        line: null,
        preconditions: [{ name: 'may_merge', words: 'The work may merge' }],
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
      main: { outside: true },
      ci: { values: ['green', 'red'] },
    },
  );
}
