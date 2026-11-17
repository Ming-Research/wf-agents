import { describe, expect, it } from 'vitest';

import { NO_MARK, blockOrder, canRun, markOf, matrix } from './matrix';
import { cell, everyAgent, flowOf, small, test, tested } from './testing';

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

  it('puts a condition’s block before what its cell watches and reads, and a block no cell uses last, by name', () => {
    const flow = flowOf(
      [
        cell('a', { outputs: ['x'] }),
        cell('b', { when: [{ on: 'block', block: 'choice', value: 'yes' }], watches: ['given', 'x'], reads: ['read'] }),
      ],
      {
        x: {},
        given: { people: true },
        read: { people: true },
        choice: { people: true, values: ['yes', 'no'] },
        zz: { shown: true, people: true },
        shown: { shown: true, people: true },
      },
    );
    expect(blockOrder(flow)).toEqual(['x', 'choice', 'given', 'read', 'shown', 'zz']);
  });
});

/// The row of a datum, or of a group.
const rowAt = (m: ReturnType<typeof matrix>, name: string) => m.blocks.findIndex((b) => b.name === name);

/// The mark of a cell on the row of a datum or a group.
const markAt = (m: ReturnType<typeof matrix>, cellId: string, name: string) =>
  m.spots[rowAt(m, name)][m.cells.findIndex((c) => c.id === cellId)].mark;

/// A mark, the rest of it none.
const mark = (fields: Partial<typeof NO_MARK>) => ({ ...NO_MARK, ...fields });

describe('the bands over the blocks', () => {
  const ready = test('ready', 'Ready', 'all', ['go']);
  const flow = () =>
    flowOf(
      [
        cell('a', { line: 'first', outputs: ['x'] }),
        cell('b', { line: 'second', when: [{ on: 'block', block: 'x', value: 'no' }] }),
        cell('c', { line: 'first', when: [{ on: 'block', block: 'x', value: 'yes' }], requires: tested(ready) }),
        cell('d', { kind: 'engine', line: null, when: [{ on: 'block', block: 'x', value: 'maybe' }] }),
        cell('e', {
          kind: 'engine',
          line: null,
          when: tested(ready),
          requires: [...tested(ready), { on: 'block', block: 'x', value: 'no' }],
        }),
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
    // A key of a requires on a block is on the block's row, with its value.
    expect(x.spots.map((s) => [s.requires, s.requiresValue])).toEqual([
      [false, undefined],
      [false, undefined],
      [false, undefined],
      [false, undefined],
      [true, 'no'],
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

  it('have a row for each of the engine’s tools, marked under each cell that may call it, and none for a flow with none', () => {
    const tooled = flowOf(
      [
        cell('a', { tools: ['query', 'run_tests'] }),
        cell('b', { tools: ['run_tests'] }),
        cell('c', { kind: 'engine', line: null }),
      ],
      {},
    );
    const m = matrix(tooled);
    expect(m.tools.map((t) => t.name)).toEqual(['query', 'run_tests']);
    expect(m.tools.map((t) => t.spots)).toEqual([
      [true, false, false],
      [true, true, false],
    ]);
    expect(matrix(flow()).tools).toEqual([]);
  });
});

describe('the marks on a datum’s row', () => {
  const flow = () =>
    flowOf(
      [
        cell('ask', { outputs: ['x', 'y'] }),
        cell('more', {
          when: tested(test('either', 'Either is yes', 'any', ['x', 'y'])),
          requires: tested(test('given', 'Given', 'all', ['go'])),
          watches: ['x'],
          reads: ['request'],
          kinds: { x: 'all', request: 'latest' },
          outputs: ['y'],
        }),
        cell('act', { kind: 'engine', line: null, watches: ['y'] }),
      ],
      {
        x: { values: ['yes', 'no'] },
        y: { values: ['yes', 'no'] },
        go: { people: true, values: ['yes', 'no'] },
        request: { people: true },
      },
    );

  it('are watches, reads with the kind it is given in, and outputs, each by its key, one ring at most', () => {
    const m = matrix(flow());
    // It watches x: a new version runs it; the kind it is given x in is kept for its words, and drawn on no ring.
    expect(markAt(m, 'more', 'x')).toEqual(mark({ watch: true, kind: 'all' }));
    expect(markAt(m, 'more', 'request')).toEqual(mark({ read: true, kind: 'latest' }));
    expect(markAt(m, 'more', 'y')).toEqual(mark({ write: true }));
    expect(markAt(m, 'ask', 'x')).toEqual(mark({ write: true }));
    // An engine's cell watches with no kind: it has no session.
    expect(markAt(m, 'act', 'y')).toEqual(mark({ watch: true }));
    // What a condition reads is on the condition's row alone.
    expect(markAt(m, 'more', 'go')).toEqual(mark({}));
  });

  it('stand on one row for each datum, and no row stands for what runs a cell apart from its data', () => {
    const m = matrix(small());
    // The datum a condition reads, `needs_design`, has its one row with the condition's.
    expect(m.blocks.map((b) => b.name)).toEqual(blockOrder(small()).filter((b) => b !== 'needs_design'));
    expect(m.conditions.map((c) => c.name)).toContain('needs_design');
    expect(m.blocks.every((b) => b.group === undefined)).toBe(true);
    expect(m.cells.map((c) => c.id)).toEqual(['plan', 'judge', 'design', 'answer', 'check', 'merge']);
  });
});

describe('a group a cell watches by its name', () => {
  const flow = () =>
    flowOf(
      [
        cell('w', { outputs: ['a'] }),
        cell('r', { watches: [{ group: 'planned', blocks: ['a', 'b'] }], kinds: { a: 'latest', b: 'all' } }),
        cell('t', { reads: [{ group: 'planned', blocks: ['a', 'b'] }], kinds: { a: 'all', b: 'latest' } }),
        cell('v', { outputs: ['b'] }),
      ],
      { a: {}, b: {} },
    );

  it('has a row of its own right above its first block, the watches mark on it and a write of any of its blocks', () => {
    const m = matrix(flow());
    expect(m.blocks.map((b) => [b.name, b.group ?? null])).toEqual([
      ['planned', ['a', 'b']],
      ['a', null],
      ['b', null],
    ]);
    const spots = m.spots[rowAt(m, 'planned')];
    expect(spots.map((s) => s.mark)).toEqual([mark({ write: true }), mark({ watch: true }), mark({}), mark({ write: true })]);
  });

  it('leaves its blocks’ rows to those that read it, each with the block’s kind, and names the group on them', () => {
    const m = matrix(flow());
    expect(markAt(m, 'r', 'a')).toEqual(mark({}));
    expect(markAt(m, 't', 'a')).toEqual(mark({ read: true, kind: 'all' }));
    expect(markAt(m, 't', 'b')).toEqual(mark({ read: true, kind: 'latest' }));
    expect(m.blocks[rowAt(m, 'a')].of).toEqual(['planned']);
    expect(m.blocks[rowAt(m, 'planned')].of).toEqual([]);
  });

  it('draws its way back on its own row once, and none on its blocks’ rows', () => {
    const m = matrix(flow());
    expect(m.spots[rowAt(m, 'planned')].map((s) => s.backTo)).toEqual([false, true, false, false]);
    expect(m.spots[rowAt(m, 'planned')].map((s) => s.back)).toEqual(['none', 'right', 'through', 'left']);
    expect(m.spots[rowAt(m, 'b')].every((s) => s.back === 'none' && !s.backTo)).toBe(true);
    expect(m.waysBack).toBe(1);
  });

  it('is drawn by its blocks alone where a cell only reads it', () => {
    const read = flowOf(
      [cell('w', { outputs: ['a'] }), cell('t', { reads: [{ group: 'planned', blocks: ['a', 'b'] }] })],
      { a: {}, b: { people: true } },
    );
    expect(matrix(read).blocks.map((b) => b.name)).toEqual(['a', 'b']);
  });
});

describe('the ways back', () => {
  /// A datum's row: among the data, or, of one a condition reads, its one row with the condition's.
  const row = (m: ReturnType<typeof matrix>, name: string) =>
    m.conditions.find((c) => c.name === name)?.data?.spots ?? m.spots[rowAt(m, name)];

  it('run left along the datum’s row from a later cell’s write to each cell before it that watches it, which they point at', () => {
    const m = matrix(small());
    expect(row(m, 'opinions').map((spot) => spot.back)).toEqual(['right', 'left', 'none', 'none', 'none', 'none']);
    expect(row(m, 'opinions').map((spot) => spot.backTo)).toEqual([true, false, false, false, false, false]);
    expect(m.waysBack).toBe(1);
  });

  it('are the only line along a row: a write going right draws nothing', () => {
    const m = matrix(small());
    expect(row(m, 'plan').every((spot) => spot.back === 'none' && !spot.backTo)).toBe(true);
    expect(m.spots.flat().every((spot) => !('line' in spot))).toBe(true);
  });

  it('reach no cell that only reads the datum, nor one whose condition reads it', () => {
    const waits = test('ready', 'x is yes', 'all', ['x']);
    const flow = flowOf(
      [
        cell('a', { requires: tested(waits) }),
        cell('r', { reads: ['x'] }),
        cell('w', { watches: ['x'] }),
        cell('b', { outputs: ['x'] }),
      ],
      { x: { people: true, values: ['yes', 'no'] } },
    );
    const m = matrix(flow);
    expect(row(m, 'x').map((spot) => spot.backTo)).toEqual([false, false, true, false]);
    expect(row(m, 'x').map((spot) => spot.back)).toEqual(['none', 'none', 'right', 'left']);
    expect(m.waysBack).toBe(1);
  });

  it('do not run a cell again for its own write', () => {
    const flow = flowOf([cell('a', { watches: ['x'], outputs: ['x'] }), cell('b', { watches: ['x'] })], {
      x: { people: true },
    });
    const m = matrix(flow);
    expect(markAt(m, 'a', 'x')).toEqual(mark({ watch: true, write: true, kind: 'lastAndLatest' }));
    expect(m.waysBack).toBe(0);
  });

  it('count every pair of a cell that watches a datum and a later cell’s write of it', () => {
    const flow = flowOf(
      [
        cell('a', { watches: ['x'] }),
        cell('b', { when: [{ on: 'block', block: 'x', value: 'yes' }] }),
        cell('c', { outputs: ['x'] }),
        cell('d', { watches: ['x'] }),
        cell('e', { outputs: ['x'] }),
      ],
      { x: { people: true, values: ['yes', 'no'] } },
    );
    const m = matrix(flow);
    expect(row(m, 'x').map((spot) => spot.backTo)).toEqual([true, false, false, true, false]);
    expect(row(m, 'x').map((spot) => spot.back)).toEqual(['right', 'through', 'through', 'through', 'left']);
    // a is watched and written after twice, d once; x's one row is the condition's that b names, and they run along it.
    expect(m.waysBack).toBe(3);
  });

  it('are drawn for a dialogue on its one row, from the later party to the earlier', () => {
    const flow = flowOf(
      [
        cell('maker', { watches: ['review'], outputs: ['review'], kinds: { review: 'all' } }),
        cell('judge', { watches: ['review'], outputs: ['review'], kinds: { review: 'all' } }),
      ],
      { review: {} },
    );
    const m = matrix(flow);
    expect(row(m, 'review').map((s) => s.mark)).toEqual([
      mark({ watch: true, write: true, kind: 'all' }),
      mark({ watch: true, write: true, kind: 'all' }),
    ]);
    expect(row(m, 'review').map((s) => [s.back, s.backTo])).toEqual([
      ['right', true],
      ['left', false],
    ]);
  });
});

describe('the band of what every agent cell is given alike', () => {
  const every = everyAgent({
    reads: [['{each}_questions', ['a_questions', 'b_questions']]],
    kinds: { '{each}_questions': 'all' },
  });
  const flow = () =>
    flowOf(
      [
        cell('a', { watches: ['a_questions'], outputs: ['a_questions', 'x'], kinds: { a_questions: 'all' } }),
        cell('n', { watches: ['x'] }),
        cell('b', {
          watches: ['b_questions', 'a_questions'],
          outputs: ['b_questions'],
          kinds: { a_questions: 'all', b_questions: 'all' },
        }),
        cell('e', { kind: 'engine', line: null, watches: ['x'] }),
      ],
      { a_questions: { outside: 'answer' }, b_questions: { outside: 'answer' }, x: {} },
      'small',
      {},
      every,
    );

  it('has a row for each name as flow.toml writes it, and the data it stands for have none of their own', () => {
    const m = matrix(flow());
    expect(m.every.map((r) => [r.name, r.kind, r.blocks])).toEqual([
      ['{each}_questions', 'all', ['a_questions', 'b_questions']],
    ]);
    expect(m.blocks.map((b) => b.name)).toEqual(['x']);
  });

  it('marks under each cell what its own file says of the data a row stands for', () => {
    const m = matrix(flow());
    const [questions] = m.every;
    // What every agent cell only reads is said by the row's words, not marked under each cell.
    expect(questions.spots.map((s) => s.mark)).toEqual([
      mark({ watch: true, write: true, kind: 'all' }),
      mark({}),
      mark({ watch: true, write: true, kind: 'all' }),
      mark({}),
    ]);
  });

  it('draws a way back on its row where a later cell writes a datum an earlier one watches', () => {
    const later = flowOf(
      [cell('a', { watches: ['a_questions'], kinds: { a_questions: 'all' } }), cell('b', { outputs: ['a_questions'] })],
      { a_questions: { outside: 'answer' } },
      'small',
      {},
      everyAgent({ reads: [['{each}_questions', ['a_questions']]], kinds: { '{each}_questions': 'all' } }),
    );
    const m = matrix(later);
    expect(m.every[0].spots.map((s) => [s.back, s.backTo])).toEqual([
      ['right', true],
      ['left', false],
    ]);
    expect(m.waysBack).toBe(1);
  });

  it('draws no way back from a later cell’s write of its own to an earlier one that watches its own', () => {
    const own = flowOf(
      [
        cell('a', { watches: ['a_questions'], outputs: ['a_questions'], kinds: { a_questions: 'all' } }),
        cell('b', { watches: ['b_questions'], outputs: ['b_questions'], kinds: { b_questions: 'all' } }),
      ],
      { a_questions: { outside: 'answer' }, b_questions: { outside: 'answer' } },
      'small',
      {},
      everyAgent({ reads: [['{each}_questions', ['a_questions', 'b_questions']]], kinds: { '{each}_questions': 'all' } }),
    );
    const m = matrix(own);
    expect(m.every[0].spots.every((s) => s.back === 'none' && !s.backTo)).toBe(true);
    expect(m.waysBack).toBe(0);
  });

  it('is empty where the flow names nothing every agent cell is given', () => {
    expect(matrix(small()).every).toEqual([]);
  });
});

describe('a way back between two cells that never both run', () => {
  /// A watcher of x and a later writer of it, each with a `when` on `deep` as given.
  const flow = (readerWhen: string | null, writerWhen: string | null) => {
    const on = (value: string | null) => (value === null ? [] : [{ on: 'block' as const, block: 'deep', value }]);
    return flowOf(
      [
        cell('first', { outputs: ['x', 'deep'] }),
        cell('reader', { watches: ['x'], when: on(readerWhen) }),
        cell('writer', { outputs: ['x'], when: on(writerWhen) }),
      ],
      { x: {}, deep: { values: ['yes', 'no'] } },
    );
  };
  const row = (m: ReturnType<typeof matrix>) => m.spots[rowAt(m, 'x')];

  it('is not drawn nor counted where their whens name one block with different values', () => {
    const f = flow('no', 'yes');
    expect(canRun(f.cells[2], f.cells[1])).toBe(false);
    const m = matrix(f);
    expect(m.waysBack).toBe(0);
    expect(row(m).some((spot) => spot.backTo || spot.back !== 'none')).toBe(false);
  });

  it('is drawn and counted where they can both run: the same value, no condition, or one on another block', () => {
    for (const [r, w] of [
      ['yes', 'yes'],
      [null, 'yes'],
      ['no', null],
    ] as const) {
      const f = flow(r, w);
      expect(canRun(f.cells[2], f.cells[1])).toBe(true);
      const m = matrix(f);
      expect(m.waysBack).toBe(1);
      expect(row(m).map((spot) => spot.backTo)).toEqual([false, true, false]);
    }
  });

  it('counts only the writes of cells that can run where the watcher does, drawing the way back to the last', () => {
    const f = flowOf(
      [
        cell('first', { outputs: ['x', 'deep'] }),
        cell('reader', { watches: ['x'], when: [{ on: 'block', block: 'deep', value: 'no' }] }),
        cell('both', { outputs: ['x'] }),
        cell('deeper', { outputs: ['x'], when: [{ on: 'block', block: 'deep', value: 'yes' }] }),
      ],
      { x: {}, deep: { values: ['yes', 'no'] } },
    );
    const m = matrix(f);
    expect(m.waysBack).toBe(1);
    expect(row(m).map((spot) => spot.back)).toEqual(['none', 'right', 'left', 'none']);
  });

  // The writer's own key on a block it writes may no longer hold once it has written it: its write runs the watcher
  // that needs the other value, as the flowchart draws it, by the one rule both decide by.
  it('counts the write of a later cell whose condition is on a block it writes itself, as the flowchart does', () => {
    const f = flowOf(
      [
        cell('first', { outputs: ['deep'] }),
        cell('reader', { watches: ['x'], when: [{ on: 'block', block: 'deep', value: 'no' }] }),
        cell('writer', { outputs: ['x', 'deep'], when: [{ on: 'block', block: 'deep', value: 'yes' }] }),
      ],
      { x: {}, deep: { values: ['yes', 'no'] } },
    );
    const m = matrix(f);
    expect(m.waysBack).toBe(1);
    expect(row(m).map((spot) => spot.backTo)).toEqual([false, true, false]);
  });
});

describe('the mark of a cell on a datum, wherever the page draws one', () => {
  it('is the matrix’s: watches by itself or through a group, else reads with its kind', () => {
    const f = flowOf(
      [
        cell('g', { watches: [{ group: 'both', blocks: ['x', 'y'] }] }),
        cell('r', { reads: ['x'], kinds: { x: 'latest' } }),
        cell('e', { kind: 'engine', line: null, watches: ['x'] }),
      ],
      { x: {}, y: {} },
    );
    expect(markOf(f.cells[0], 'y')).toEqual(mark({ watch: true, kind: 'lastAndLatest' }));
    expect(markOf(f.cells[1], 'x')).toEqual(mark({ read: true, kind: 'latest' }));
    expect(markOf(f.cells[2], 'x')).toEqual(mark({ watch: true }));
    expect(markOf(f.cells[2], 'y')).toEqual(mark({}));
  });
});
