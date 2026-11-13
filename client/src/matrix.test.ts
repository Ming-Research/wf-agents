import { describe, expect, it } from 'vitest';

import type { Change } from './api';
import { NO_MARK, blockOrder, matrix, merged } from './matrix';
import { cell, flowOf, small, test } from './testing';

describe('the order of the blocks', () => {
  it('puts each block where it is first written, a cell’s outputs together, and what no cell writes before its first reader', () => {
    expect(blockOrder(small())).toEqual([
      'request',
      'plan',
      'needs_design',
      'opinions',
      'design',
      'commented',
      'confirmed',
      'comments',
      'reply',
      'main',
      'ci',
    ]);
  });

  it('puts a condition’s block before the inputs of its cell, and a block no cell uses last, by name', () => {
    const flow = flowOf(
      [
        cell('a', { outputs: ['x'] }),
        cell('b', { condition: { on: 'block', block: 'choice', value: 'yes' }, inputs: ['given', 'x'] }),
      ],
      { x: {}, given: { people: true }, choice: { people: true, values: ['yes', 'no'] }, zz: { shown: true, people: true }, shown: { shown: true, people: true } },
    );
    expect(blockOrder(flow)).toEqual(['x', 'choice', 'given', 'shown', 'zz']);
  });
});

describe('the marks of named tests', () => {
  const flow = flowOf(
    [
      cell('ask', { outputs: ['x', 'y'] }),
      cell('more', {
        condition: { on: 'test', test: test('either', 'Either is yes', 'any', ['x', 'y']) },
        preconditions: [test('given', 'Given', 'all', ['go'])],
        inputs: ['x', 'request'],
      }),
    ],
    { x: { values: ['yes', 'no'] }, y: { values: ['yes', 'no'] }, go: { people: true, values: ['yes', 'no'] }, request: { people: true } },
  );
  const at = (m: ReturnType<typeof matrix>, cellId: string, block: string) =>
    m.spots[m.blocks.findIndex((b) => b.name === block)][m.cells.findIndex((c) => c.id === cellId)].mark;

  it('put the condition’s mark on each block the test that is the condition reads, and a precondition’s on each its test reads', () => {
    const m = matrix({ after: flow }, 'after');
    expect(at(m, 'more', 'x')).toEqual({ runs: true, given: true, write: false, condition: true, precondition: false });
    expect(at(m, 'more', 'y')).toEqual({ runs: false, given: false, write: false, condition: true, precondition: false });
    expect(at(m, 'more', 'go')).toEqual({ runs: false, given: false, write: false, condition: false, precondition: true });
    expect(at(m, 'more', 'request')).toEqual({ runs: true, given: true, write: false, condition: false, precondition: false });
  });

  it('put a block no cell writes before its first reader: what the condition reads, then the preconditions, then the inputs', () => {
    const order = blockOrder(
      flowOf(
        [cell('a', { condition: { on: 'block', block: 'c', value: 'yes' }, preconditions: [test('t', 'T', 'all', ['p'])], inputs: ['i'] })],
        { i: { people: true }, p: { people: true, values: ['yes', 'no'] }, c: { people: true, values: ['yes', 'no'] } },
      ),
    );
    expect(order).toEqual(['c', 'p', 'i']);
  });
});

describe('two orders merged', () => {
  it('keeps the order shown and puts what only the other has after what it followed there', () => {
    expect(merged(['a', 'b', 'c'], ['a', 'x', 'b', 'y', 'c', 'z'])).toEqual(['a', 'x', 'b', 'y', 'c', 'z']);
    expect(merged(['b', 'c'], ['x', 'b', 'c'])).toEqual(['x', 'b', 'c']);
    expect(merged(['a', 'n', 'b'], ['a', 'b'])).toEqual(['a', 'n', 'b']);
  });

  it('does not put what was taken out after a cell that moved', () => {
    // c moved to the front; x followed it, and stands after a, the cell before it that stayed.
    expect(merged(['c', 'a', 'b'], ['a', 'c', 'x', 'b'], new Set(['c']))).toEqual(['c', 'a', 'x', 'b']);
  });
});

describe('the marks', () => {
  it('say where a cell reads a block, may write it, or reads its value in its condition', () => {
    const m = matrix({ after: small() }, 'after');
    const at = (cellId: string, block: string) =>
      m.spots[m.blocks.findIndex((b) => b.name === block)][m.cells.findIndex((c) => c.id === cellId)].mark;
    expect(at('plan', 'request')).toEqual({ runs: true, given: true, write: false, condition: false, precondition: false });
    expect(at('plan', 'plan')).toEqual({ runs: false, given: false, write: true, condition: false, precondition: false });
    expect(at('design', 'needs_design')).toEqual({ runs: false, given: false, write: false, condition: true, precondition: false });
    expect(at('merge', 'plan')).toEqual(NO_MARK);
    // The cells run left to right in the order of the walk, the blocks down the rows.
    expect(m.cells.map((c) => c.id)).toEqual(['plan', 'judge', 'design', 'answer', 'check', 'merge']);
    expect(m.blocks.map((b) => b.name)).toEqual(blockOrder(small()));
    expect(m.cells.every((c) => c.status === 'same')).toBe(true);
  });

  it('draw a cell that reads and writes a block as both', () => {
    const flow = flowOf([cell('a', { inputs: ['x'], outputs: ['x'] }), cell('b', { inputs: ['x'] })], { x: { people: true } });
    const m = matrix({ after: flow }, 'after');
    expect(m.spots[0][0].mark).toEqual({ runs: true, given: true, write: true, condition: false, precondition: false });
    // A cell does not run for its own write: no way back.
    expect(m.waysBack).toBe(0);
    expect(m.spots[0].map((spot) => spot.back)).toEqual(['none', 'none']);
  });
});

describe('what runs a cell and what it is given', () => {
  it('are marked apart, and a block a cell is only given sends no way back to it', () => {
    const flow = flowOf(
      [
        cell('a', { runsOn: ['x', 'ping'], given: ['x', 'record'], outputs: ['record'] }),
        cell('b', { inputs: ['record'], outputs: ['x', 'record', 'ping'] }),
      ],
      { x: {}, record: { people: true }, ping: { people: true } },
    );
    const m = matrix({ after: flow }, 'after');
    const at = (block: string) => m.spots[m.blocks.findIndex((b) => b.name === block)][0].mark;
    expect(at('x')).toEqual({ runs: true, given: true, write: false, condition: false, precondition: false });
    expect(at('ping')).toEqual({ runs: true, given: false, write: false, condition: false, precondition: false });
    expect(at('record')).toEqual({ runs: false, given: true, write: true, condition: false, precondition: false });
    // b's writes of x and of ping run a again; its write of the record, which a is only given, does not.
    expect(m.waysBack).toBe(2);
    expect(m.spots[m.blocks.findIndex((b) => b.name === 'record')].map((spot) => spot.backTo)).toEqual([false, false]);
  });

  it('put a block no cell writes before its first cell, those that run it before those it is only given', () => {
    const flow = flowOf([cell('a', { runsOn: ['ping'], given: ['notes', 'ping'] })], {
      notes: { people: true },
      ping: { people: true },
    });
    expect(blockOrder(flow)).toEqual(['ping', 'notes']);
  });
});

describe('the rows of the tools', () => {
  const flow = (aTools: string[], bTools: string[]) =>
    flowOf([cell('a', { inputs: ['x'], tools: aTools }), cell('b', { outputs: ['x'], tools: bTools })], { x: {} });

  it('stand in the order of the first cell that may ask for each, with a mark under each cell that may', () => {
    const m = matrix({ after: flow(['run_tests'], ['query', 'run_tests']) }, 'after');
    expect(m.tools.map((t) => [t.name, t.spots.map((s) => s.asks)])).toEqual([
      ['run_tests', [true, true]],
      ['query', [false, true]],
    ]);
    expect(matrix({ after: flow([], []) }, 'after').tools).toEqual([]);
  });

  it('hold the tools of both versions when comparing, each mark the change put in or took out said so', () => {
    const after = flow(['run_tests'], []);
    const before = flow([], ['query']);
    const changes: Change[] = [
      { kind: 'toolAdded', cell: 'a', tool: 'run_tests' },
      { kind: 'toolRemoved', cell: 'b', tool: 'query' },
    ];
    const shown = matrix({ after, before, changes }, 'after');
    expect(shown.tools.map((t) => [t.name, t.spots])).toEqual([
      [
        'run_tests',
        [
          { asks: true, added: true, removed: false },
          { asks: false, added: false, removed: false },
        ],
      ],
      [
        'query',
        [
          { asks: false, added: false, removed: false },
          { asks: false, added: false, removed: true },
        ],
      ],
    ]);
    // As it was, the same rows, each cell marked as it was.
    const was = matrix({ after, before, changes }, 'before');
    expect(was.tools.map((t) => [t.name, t.spots.map((s) => s.asks)])).toEqual([
      ['run_tests', [false, false]],
      ['query', [false, true]],
    ]);
  });
});

describe('the ways back', () => {
  const row = (m: ReturnType<typeof matrix>, block: string) => m.spots[m.blocks.findIndex((b) => b.name === block)];

  it('run left along a row from a later cell’s write to each read before it, which they point at', () => {
    const m = matrix({ after: small() }, 'after');
    expect(row(m, 'opinions').map((spot) => spot.back)).toEqual(['right', 'left', 'none', 'none', 'none', 'none']);
    expect(row(m, 'opinions').map((spot) => spot.backTo)).toEqual([true, false, false, false, false, false]);
    expect(m.waysBack).toBe(1);
  });

  it('leave a row whose writes all stand left of its reads to its plain line', () => {
    const m = matrix({ after: small() }, 'after');
    expect(row(m, 'plan').map((spot) => spot.line)).toEqual(['right', 'through', 'through', 'left', 'none', 'none']);
    expect(row(m, 'plan').every((spot) => spot.back === 'none' && !spot.backTo)).toBe(true);
  });

  it('leave the row\'s line out left of the first write, where the way back runs alone', () => {
    // The opinions are read by the first cell and written by the second: nothing of them goes forward.
    const m = matrix({ after: small() }, 'after');
    expect(row(m, 'opinions').every((spot) => spot.line === 'none')).toBe(true);
    // Read by a and b, written by c, read by d, written by e: the line runs from c on.
    const flow = flowOf(
      [
        cell('a', { inputs: ['x'] }),
        cell('b', { inputs: ['x'] }),
        cell('c', { outputs: ['x'] }),
        cell('d', { inputs: ['x'] }),
        cell('e', { outputs: ['x'] }),
      ],
      { x: {} },
    );
    const spots = matrix({ after: flow }, 'after').spots[0];
    expect(spots.map((spot) => spot.line)).toEqual(['none', 'none', 'right', 'through', 'left']);
    expect(spots.map((spot) => spot.back)).toEqual(['right', 'through', 'through', 'through', 'left']);
  });

  it('start the row\'s line at a mark left of the first write that no way back reaches, so that none stands alone', () => {
    // A precondition of the first cell reads what people and a later cell write: no way back reaches a precondition.
    const waits = test('ready', 'x is yes', 'all', ['x']);
    const flow = flowOf([cell('a', { preconditions: [waits] }), cell('b', { outputs: ['x'] }), cell('c', { inputs: ['x'] })], {
      x: { people: true, values: ['yes', 'no'] },
    });
    const spots = matrix({ after: flow }, 'after').spots[0];
    expect(spots.map((spot) => spot.line)).toEqual(['right', 'through', 'left']);
    expect(spots.every((spot) => spot.back === 'none')).toBe(true);
  });

  it('count a read by a condition, and every pair of a read and a later write', () => {
    const flow = flowOf(
      [
        cell('a', { inputs: ['x'] }),
        cell('b', { condition: { on: 'block', block: 'x', value: 'yes' } }),
        cell('c', { outputs: ['x'] }),
        cell('d', { inputs: ['x'] }),
        cell('e', { outputs: ['x'] }),
      ],
      { x: { people: true, values: ['yes', 'no'] } },
    );
    const m = matrix({ after: flow }, 'after');
    expect(m.spots[0].map((spot) => spot.backTo)).toEqual([true, true, false, true, false]);
    expect(m.spots[0].map((spot) => spot.back)).toEqual(['right', 'through', 'through', 'through', 'left']);
    // a, b and d each read it with a write after: a and b twice, d once.
    expect(m.waysBack).toBe(5);
  });
});

describe('what changed', () => {
  const before = flowOf(
    [cell('plan', { inputs: ['request'], outputs: ['plan'] }), cell('old', { inputs: ['plan'] }), cell('merge', { inputs: ['plan'] })],
    { request: { people: true }, plan: {} },
  );
  const after = flowOf(
    [
      cell('plan', { title: 'Plan it', inputs: ['request', 'notes'], outputs: ['plan'] }),
      cell('new', { inputs: ['plan'] }),
      cell('merge', { inputs: ['plan'] }),
    ],
    { request: { people: true }, plan: {}, notes: { people: true } },
  );
  const changes: Change[] = [
    { kind: 'title', cell: 'plan', before: 'plan', after: 'Plan it' },
    { kind: 'runsOnAdded', cell: 'plan', block: 'notes' },
    { kind: 'cellAdded', cell: 'new', place: 1 },
    { kind: 'cellRemoved', cell: 'old', place: 1 },
    { kind: 'blockAdded', block: 'notes' },
  ];

  it('holds the cells and blocks of both versions, each marked as put in, taken out, changed or the same', () => {
    const m = matrix({ after, before, changes }, 'after');
    // What was taken out stands after what it followed, before what was put in there.
    expect(m.cells.map((c) => [c.id, c.status, c.cell !== undefined])).toEqual([
      ['plan', 'changed', true],
      ['old', 'removed', false],
      ['new', 'added', true],
      ['merge', 'same', true],
    ]);
    expect([...m.cells[0].changed]).toEqual(['title', 'runsOnAdded']);
    expect(m.blocks.map((b) => [b.name, b.status])).toEqual([
      ['request', 'same'],
      ['notes', 'added'],
      ['plan', 'same'],
    ]);
  });

  it('marks what the change put in and took out at each crossing', () => {
    const m = matrix({ after, before, changes }, 'after');
    const notes = m.spots[m.blocks.findIndex((b) => b.name === 'notes')];
    const plan = m.spots[m.blocks.findIndex((b) => b.name === 'plan')];
    expect(notes[0].added).toEqual({ runs: true, given: true, write: false, condition: false, precondition: false });
    expect(plan[2].added).toEqual({ runs: true, given: true, write: false, condition: false, precondition: false });
    // The cell taken out is not in the version shown, and its read is drawn as taken out.
    expect(plan[1].mark).toEqual(NO_MARK);
    expect(plan[1].removed).toEqual({ runs: true, given: true, write: false, condition: false, precondition: false });
    expect(plan[3].added).toEqual(NO_MARK);
    expect(plan[3].removed).toEqual(NO_MARK);
  });

  it('as it was, puts the rows in the order of the version compared against and shows its marks', () => {
    const m = matrix({ after, before, changes }, 'before');
    expect(m.cells.map((c) => [c.id, c.status, c.cell !== undefined])).toEqual([
      ['plan', 'changed', true],
      ['new', 'added', false],
      ['old', 'removed', true],
      ['merge', 'same', true],
    ]);
    expect(m.cells[0].cell?.title).toBe('plan');
    const notes = m.spots[m.blocks.findIndex((b) => b.name === 'notes')];
    expect(notes[0].mark).toEqual(NO_MARK);
    expect(notes[0].added).toEqual({ runs: true, given: true, write: false, condition: false, precondition: false });
  });

  it('keeps a moved cell where the version shown has it', () => {
    const was = flowOf([cell('a'), cell('b'), cell('c')], {});
    const is = flowOf([cell('c'), cell('a'), cell('b')], {});
    const moved: Change[] = [{ kind: 'cellMoved', cell: 'c', before: 2, after: 0 }];
    expect(matrix({ after: is, before: was, changes: moved }, 'after').cells.map((c) => [c.id, c.status])).toEqual([
      ['c', 'changed'],
      ['a', 'same'],
      ['b', 'same'],
    ]);
    expect(matrix({ after: is, before: was, changes: moved }, 'before').cells.map((c) => c.id)).toEqual(['a', 'b', 'c']);
  });
});
