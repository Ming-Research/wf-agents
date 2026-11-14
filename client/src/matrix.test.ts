import { describe, expect, it } from 'vitest';

import { NO_MARK, blockOrder, matrix } from './matrix';
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

/// The row of a trigger, or of a datum, of this name.
const rowAt = (m: ReturnType<typeof matrix>, name: string, trigger = false) =>
  m.blocks.findIndex((b) => b.name === name && (b.trigger !== undefined) === trigger);

/// The mark of a cell on the row of a trigger, or of a datum, of this name.
const markAt = (m: ReturnType<typeof matrix>, cellId: string, name: string, trigger = false) =>
  m.spots[rowAt(m, name, trigger)][m.cells.findIndex((c) => c.id === cellId)].mark;

/// A mark on a datum's row, which says the kind the cell is given the datum in.
const datum = (mark: Partial<typeof NO_MARK>, kind: string | null = null) => ({ ...NO_MARK, ...mark, kind });

describe('the bands over the blocks', () => {
  const ready = test('ready', 'Ready', 'all', ['go']);
  const flow = () =>
    flowOf(
      [
        cell('a', { line: 'first', outputs: ['x'] }),
        cell('b', { line: 'second', condition: { on: 'block', block: 'x', value: 'no' } }),
        cell('c', { line: 'first', condition: { on: 'block', block: 'x', value: 'yes' }, preconditions: [ready] }),
        cell('d', { kind: 'engine', line: null, condition: { on: 'block', block: 'x', value: 'maybe' } }),
        cell('e', { kind: 'engine', line: null, condition: { on: 'test', test: ready }, preconditions: [ready] }),
      ],
      { x: { values: ['yes', 'no', 'maybe'] }, go: { people: true, values: ['yes', 'no'] } },
    );

  it('have a row for each line, where its first cell stands, its cells joined along it', () => {
    const m = matrix(flow());
    expect(m.lines.map((l) => l.name)).toEqual(['first', 'second']);
    expect(m.lines[0].spots.map((s) => s.on)).toEqual([true, false, true, false, false]);
    expect(m.lines[0].spots.map((s) => s.line)).toEqual(['right', 'through', 'left', 'none', 'none']);
    // A line of one cell has nothing to join.
    expect(m.lines[1].spots.map((s) => s.line)).toEqual(['none', 'none', 'none', 'none', 'none']);
    expect(m.lines[0].line?.file).toBe('lines/first.toml');
  });

  it('have a row for each block a cell’s when reads, with the value each cell waits for, and one for each named test', () => {
    const m = matrix(flow());
    expect(m.conditions.map((r) => [r.name, r.test !== undefined])).toEqual([
      ['x', false],
      ['ready', true],
    ]);
    const [x, named] = m.conditions;
    expect(x.spots.map((s) => [s.when, s.value])).toEqual([
      [false, undefined],
      [true, 'no'],
      [true, 'yes'],
      [true, 'maybe'],
      [false, undefined],
    ]);
    expect(named.spots.map((s) => [s.when, s.requires])).toEqual([
      [false, false],
      [false, false],
      [false, true],
      [false, false],
      [true, true],
    ]);
    expect(named.test?.file).toBe('tests/ready.toml');
  });

  it('leave what a condition reads off the blocks’ rows, where it was drawn twice', () => {
    const m = matrix(flow());
    expect(m.spots.flat().some((s) => s.mark.condition || s.mark.precondition)).toBe(false);
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

  it('leave what the test that is the condition reads, and what a precondition’s test reads, off the data’s rows', () => {
    const m = matrix(flow);
    expect(markAt(m, 'more', 'x')).toEqual(datum({ given: true }));
    expect(markAt(m, 'more', 'y')).toEqual(datum({}));
    expect(markAt(m, 'more', 'go')).toEqual(datum({}));
    expect(markAt(m, 'more', 'request')).toEqual(datum({ given: true }));
    // Each test is a row of its own, under the cell that names it.
    expect(m.conditions.map((r) => r.name)).toEqual(['either', 'given']);
    expect(m.conditions[0].spots.map((s) => s.when)).toEqual([false, true]);
    expect(m.conditions[1].spots.map((s) => s.requires)).toEqual([false, true]);
    // What runs the cell is on its triggers' rows, and only there.
    expect(markAt(m, 'more', 'x', true)).toEqual({ ...NO_MARK, runs: true });
    expect(markAt(m, 'more', 'request', true)).toEqual({ ...NO_MARK, runs: true });
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

describe('the marks', () => {
  it('say on a trigger’s row which cells it runs and which write its block, and on a datum’s which are given it, write it or read its value', () => {
    const m = matrix(small());
    expect(markAt(m, 'plan', 'request', true)).toEqual({ ...NO_MARK, runs: true });
    expect(markAt(m, 'plan', 'request')).toEqual(datum({ given: true }));
    expect(markAt(m, 'plan', 'plan', true)).toEqual({ ...NO_MARK, write: true });
    expect(markAt(m, 'plan', 'plan')).toEqual(datum({ write: true }));
    expect(markAt(m, 'design', 'needs_design')).toEqual(datum({}));
    expect(markAt(m, 'merge', 'plan')).toEqual(datum({}));
    // The cells run left to right in the order of the walk; the triggers' rows stand first, by the blocks they stand
    // on, then each datum's.
    expect(m.cells.map((c) => c.id)).toEqual(['plan', 'judge', 'design', 'answer', 'check', 'merge']);
    expect(m.blocks.filter((b) => b.trigger !== undefined).map((b) => b.name)).toEqual([
      'request',
      'plan',
      'opinions',
      'design',
      'comments',
      'main',
      'ci',
    ]);
    expect(m.blocks.filter((b) => b.trigger === undefined).map((b) => b.name)).toEqual(blockOrder(small()));
  });

  it('draw a cell that is run by a block and writes it as both, on its trigger’s row', () => {
    const flow = flowOf([cell('a', { inputs: ['x'], outputs: ['x'] }), cell('b', { inputs: ['x'] })], { x: { people: true } });
    const m = matrix(flow);
    expect(markAt(m, 'a', 'x', true)).toEqual({ ...NO_MARK, runs: true, write: true });
    expect(markAt(m, 'a', 'x')).toEqual(datum({ given: true, write: true }));
    // A cell does not run for its own write: no way back.
    expect(m.waysBack).toBe(0);
    expect(m.spots[rowAt(m, 'x', true)].map((spot) => spot.back)).toEqual(['none', 'none']);
  });
});

describe('what runs a cell and what it is given', () => {
  it('are marked apart, and a block a cell is only given sends no way back to it', () => {
    const flow = flowOf(
      [
        cell('a', { runsOn: ['x', 'ping'], given: ['x', 'record'], outputs: ['record'] }),
        cell('b', { outputs: ['x', 'record', 'ping'] }),
      ],
      { x: {}, record: { people: true }, ping: { people: true } },
    );
    const m = matrix(flow);
    expect(markAt(m, 'a', 'x', true)).toEqual({ ...NO_MARK, runs: true });
    expect(markAt(m, 'a', 'x')).toEqual(datum({ given: true }));
    expect(markAt(m, 'a', 'ping', true)).toEqual({ ...NO_MARK, runs: true });
    expect(markAt(m, 'a', 'ping')).toEqual(datum({}));
    expect(markAt(m, 'a', 'record')).toEqual(datum({ given: true, write: true }));
    // No trigger stands on the record: nothing of it runs a cell.
    expect(rowAt(m, 'record', true)).toBe(-1);
    // b's writes of x and of ping run a again; its write of the record does not.
    expect(m.waysBack).toBe(2);
  });

  it('give on a datum’s row the kind the cell is given it in, where the cell says one', () => {
    const flow = flowOf(
      [cell('a', { outputs: ['x'] }), cell('b', { inputs: ['x'], input: { x: { runs: { why: null }, given: { kind: 'all', why: null } } } })],
      { x: {} },
    );
    const m = matrix(flow);
    expect(markAt(m, 'b', 'x')).toEqual(datum({ given: true }, 'all'));
  });
});

describe('the ways back', () => {
  const row = (m: ReturnType<typeof matrix>, block: string) => m.spots[rowAt(m, block, true)];

  it('run left along a trigger’s row from a later cell’s write to each cell before it that it runs, which they point at', () => {
    const m = matrix(small());
    expect(row(m, 'opinions').map((spot) => spot.back)).toEqual(['right', 'left', 'none', 'none', 'none', 'none']);
    expect(row(m, 'opinions').map((spot) => spot.backTo)).toEqual([true, false, false, false, false, false]);
    expect(m.waysBack).toBe(1);
  });

  it('leave a row whose writes all stand left of its reads to its plain line', () => {
    const m = matrix(small());
    expect(row(m, 'plan').map((spot) => spot.line)).toEqual(['right', 'through', 'through', 'left', 'none', 'none']);
    expect(row(m, 'plan').every((spot) => spot.back === 'none' && !spot.backTo)).toBe(true);
  });

  it('leave the row\'s line out left of the first write, where the way back runs alone', () => {
    // The opinions run the first cell and the second writes them: nothing of them goes forward.
    const m = matrix(small());
    expect(row(m, 'opinions').every((spot) => spot.line === 'none')).toBe(true);
    // Run a and b, written by c, run d, written by e: the line runs from c on.
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
    const spots = row(matrix(flow), 'x');
    expect(spots.map((spot) => spot.line)).toEqual(['none', 'none', 'right', 'through', 'left']);
    expect(spots.map((spot) => spot.back)).toEqual(['right', 'through', 'through', 'through', 'left']);
  });

  it('draw no line and no way back on a datum’s row: a datum a cell is given, or whose value it reads, runs no cell', () => {
    // A precondition of the first cell reads what people and a later cell write.
    const waits = test('ready', 'x is yes', 'all', ['x']);
    const flow = flowOf([cell('a', { preconditions: [waits] }), cell('b', { outputs: ['x'] }), cell('c', { inputs: ['x'] })], {
      x: { people: true, values: ['yes', 'no'] },
    });
    const m = matrix(flow);
    expect(m.spots[rowAt(m, 'x')].every((spot) => spot.line === 'none' && spot.back === 'none')).toBe(true);
    expect(markAt(m, 'a', 'x')).toEqual(datum({}));
    // On the trigger's row the line runs from b's write to c, which it runs.
    expect(row(m, 'x').map((spot) => spot.line)).toEqual(['none', 'right', 'left']);
  });

  it('count every pair of a cell a trigger runs and a later cell’s write of its block, and no read by a condition', () => {
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
    const m = matrix(flow);
    expect(row(m, 'x').map((spot) => spot.backTo)).toEqual([true, false, false, true, false]);
    expect(row(m, 'x').map((spot) => spot.back)).toEqual(['right', 'through', 'through', 'through', 'left']);
    // a is run by x and written after twice, d once; b's condition reads it on the condition's row alone.
    expect(m.waysBack).toBe(3);
    expect(markAt(m, 'b', 'x')).toEqual(datum({}));
  });
});
