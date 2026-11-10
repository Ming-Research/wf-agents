import { describe, expect, it } from 'vitest';

import type { Change } from './api';
import { NO_MARK, blockOrder, matrix, merged } from './matrix';
import { cell, flowOf, small } from './testing';

describe('the order of the blocks', () => {
  it('puts each block where it is first written, a cell’s outputs together, and what no cell writes before its first reader', () => {
    expect(blockOrder(small())).toEqual([
      'request',
      'plan',
      'needs_design',
      'opinions',
      'design',
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
      m.spots[m.rows.findIndex((r) => r.id === cellId)][m.columns.findIndex((c) => c.name === block)].mark;
    expect(at('plan', 'request')).toEqual({ read: true, write: false, condition: false });
    expect(at('plan', 'plan')).toEqual({ read: false, write: true, condition: false });
    expect(at('design', 'needs_design')).toEqual({ read: false, write: false, condition: true });
    expect(at('merge', 'plan')).toEqual(NO_MARK);
    expect(m.rows.map((r) => r.id)).toEqual(['plan', 'judge', 'design', 'answer', 'check', 'merge']);
    expect(m.rows.every((r) => r.status === 'same')).toBe(true);
  });

  it('draw a cell that reads and writes a block as both', () => {
    const flow = flowOf([cell('a', { inputs: ['x'], outputs: ['x'] }), cell('b', { inputs: ['x'] })], { x: { people: true } });
    const m = matrix({ after: flow }, 'after');
    expect(m.spots[0][0].mark).toEqual({ read: true, write: true, condition: false });
    // A cell does not run for its own write: no way back.
    expect(m.waysBack).toBe(0);
    expect(m.spots.map((row) => row[0].back)).toEqual(['none', 'none']);
  });
});

describe('the ways back', () => {
  it('run up a column from a later cell’s write to each read above it, which they point at', () => {
    const m = matrix({ after: small() }, 'after');
    const column = m.columns.findIndex((c) => c.name === 'opinions');
    expect(m.spots.map((row) => row[column].back)).toEqual(['down', 'up', 'none', 'none', 'none', 'none']);
    expect(m.spots.map((row) => row[column].backTo)).toEqual([true, false, false, false, false, false]);
    expect(m.waysBack).toBe(1);
  });

  it('leave a column whose writes all stand above its reads to its plain line', () => {
    const m = matrix({ after: small() }, 'after');
    const column = m.columns.findIndex((c) => c.name === 'plan');
    expect(m.spots.map((row) => row[column].line)).toEqual(['down', 'through', 'through', 'up', 'none', 'none']);
    expect(m.spots.every((row) => row[column].back === 'none' && !row[column].backTo)).toBe(true);
  });

  it('count a read by a condition, and every pair of a write below a read', () => {
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
    expect(m.spots.map((row) => row[0].backTo)).toEqual([true, true, false, true, false]);
    expect(m.spots.map((row) => row[0].back)).toEqual(['down', 'through', 'through', 'through', 'up']);
    // a, b and d each read it with a write below: a and b twice, d once.
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
    { kind: 'inputAdded', cell: 'plan', block: 'notes' },
    { kind: 'cellAdded', cell: 'new', place: 1 },
    { kind: 'cellRemoved', cell: 'old', place: 1 },
    { kind: 'blockAdded', block: 'notes' },
  ];

  it('holds the cells and blocks of both versions, each marked as put in, taken out, changed or the same', () => {
    const m = matrix({ after, before, changes }, 'after');
    // What was taken out stands after what it followed, before what was put in there.
    expect(m.rows.map((r) => [r.id, r.status, r.cell !== undefined])).toEqual([
      ['plan', 'changed', true],
      ['old', 'removed', false],
      ['new', 'added', true],
      ['merge', 'same', true],
    ]);
    expect([...m.rows[0].changed]).toEqual(['title', 'inputAdded']);
    expect(m.columns.map((c) => [c.name, c.status])).toEqual([
      ['request', 'same'],
      ['notes', 'added'],
      ['plan', 'same'],
    ]);
  });

  it('marks what the change put in and took out at each crossing', () => {
    const m = matrix({ after, before, changes }, 'after');
    const notes = m.columns.findIndex((c) => c.name === 'notes');
    const plan = m.columns.findIndex((c) => c.name === 'plan');
    expect(m.spots[0][notes].added).toEqual({ read: true, write: false, condition: false });
    expect(m.spots[2][plan].added).toEqual({ read: true, write: false, condition: false });
    // The cell taken out is not in the version shown, and its read is drawn as taken out.
    expect(m.spots[1][plan].mark).toEqual(NO_MARK);
    expect(m.spots[1][plan].removed).toEqual({ read: true, write: false, condition: false });
    expect(m.spots[3][plan].added).toEqual(NO_MARK);
    expect(m.spots[3][plan].removed).toEqual(NO_MARK);
  });

  it('as it was, puts the rows in the order of the version compared against and shows its marks', () => {
    const m = matrix({ after, before, changes }, 'before');
    expect(m.rows.map((r) => [r.id, r.status, r.cell !== undefined])).toEqual([
      ['plan', 'changed', true],
      ['new', 'added', false],
      ['old', 'removed', true],
      ['merge', 'same', true],
    ]);
    expect(m.rows[0].cell?.title).toBe('plan');
    const notes = m.columns.findIndex((c) => c.name === 'notes');
    expect(m.spots[0][notes].mark).toEqual(NO_MARK);
    expect(m.spots[0][notes].added).toEqual({ read: true, write: false, condition: false });
  });

  it('keeps a moved cell where the version shown has it', () => {
    const was = flowOf([cell('a'), cell('b'), cell('c')], {});
    const is = flowOf([cell('c'), cell('a'), cell('b')], {});
    const moved: Change[] = [{ kind: 'cellMoved', cell: 'c', before: 2, after: 0 }];
    expect(matrix({ after: is, before: was, changes: moved }, 'after').rows.map((r) => [r.id, r.status])).toEqual([
      ['c', 'changed'],
      ['a', 'same'],
      ['b', 'same'],
    ]);
    expect(matrix({ after: is, before: was, changes: moved }, 'before').rows.map((r) => r.id)).toEqual(['a', 'b', 'c']);
  });
});
