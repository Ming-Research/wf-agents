import { describe, expect, it } from 'vitest';

import type { Flow } from './api';
import { carried, cellNode, chartOf, contextOf, outsideNode } from './graph/model';
import { NO_MARK, matrix } from './matrix';
import { cell, everyAgent, flowOf, small } from './testing';
import { cellSays, everyIs, everyMarkWords, everySays, markWords } from './words';

// The edges of the Flow page drawn on what each cell `watches` and `reads`: groups that share a block, and what a cell's
// own file says of a datum `[every_agent]` names. A cell that takes a block by two names, by itself and through a group
// or through two groups, the compiler refuses (decision a-cell-names-a-block-once). Each is held against the design
// (design document `data-and-context`, 5 to 12; decisions a-way-back-is-the-only-line-along-a-row and
// every-agent-is-a-band-of-its-own) and against the flowchart, which counts the names its arrows carry.

type M = ReturnType<typeof matrix>;

const rowAt = (m: M, name: string) => m.blocks.findIndex((b) => b.name === name);
const colAt = (m: M, id: string) => m.cells.findIndex((c) => c.id === id);
const markAt = (m: M, id: string, name: string) => m.spots[rowAt(m, name)][colAt(m, id)].mark;
const row = (m: M, name: string) => m.spots[rowAt(m, name)];
const band = (m: M, name: string) => {
  const found = m.every.find((r) => r.name === name);
  if (!found) throw new Error(`no row ${name} in the band of every_agent`);
  return found;
};
const mark = (fields: Partial<typeof NO_MARK>) => ({ ...NO_MARK, ...fields });
const cellIn = (flow: Flow, id: string) => {
  const found = flow.cells.find((c) => c.id === id);
  if (!found) throw new Error(`no cell ${id}`);
  return found;
};

/// The ways back the flowchart draws, counted from its arrows alone and not from `Chart.waysBack` (which is the
/// matrix's count copied): one for each name an arrow back up the walk carries, and of one drawn into a fork, the names
/// of each cell it leads to.
function drawnBack(flow: Flow): number {
  return chartOf(flow)
    .edges.filter((e) => e.kind === 'arrow' && e.back)
    .reduce((n, e) => {
      const writer = cellIn(flow, e.from ?? '');
      const reached = e.reaches.length > 0 ? e.reaches : [e.to];
      return n + reached.reduce((k, id) => k + carried(flow, writer, cellIn(flow, id)).length, 0);
    }, 0);
}

describe('two groups that share a block, each watched', () => {
  const f = () =>
    flowOf(
      [
        cell('w', { outputs: ['a', 'b', 'c'] }),
        cell('r1', { watches: [{ group: 'G1', blocks: ['a', 'b'] }] }),
        cell('r2', { watches: [{ group: 'G2', blocks: ['b', 'c'] }] }),
        cell('v', { outputs: ['a', 'b'] }),
      ],
      { a: {}, b: {}, c: {} },
    );

  // Catches a second group's row put above the first group's or under its block, a shared block naming one group, and a
  // later cell that writes two blocks of one group drawing or counting its way back twice.
  it('has a row for each above its first block, the shared block naming both, one way back for each group', () => {
    const m = matrix(f());
    expect(m.blocks.map((b) => [b.name, b.of])).toEqual([
      ['G1', []],
      ['a', ['G1']],
      ['G2', []],
      ['b', ['G1', 'G2']],
      ['c', ['G2']],
    ]);
    expect(row(m, 'G1').map((s) => [s.back, s.backTo])).toEqual([
      ['none', false],
      ['right', true],
      ['through', false],
      ['left', false],
    ]);
    expect(row(m, 'G2').map((s) => [s.back, s.backTo])).toEqual([
      ['none', false],
      ['none', false],
      ['right', true],
      ['left', false],
    ]);
    for (const name of ['a', 'b', 'c']) {
      expect(row(m, name).every((s) => s.back === 'none' && !s.backTo)).toBe(true);
    }
    expect(m.waysBack).toBe(2);
    expect(drawnBack(f())).toBe(2);
  });
});

describe('a group only read', () => {
  const f = () =>
    flowOf(
      [
        cell('w', { outputs: ['a'] }),
        cell('t', { reads: [{ group: 'G', blocks: ['a', 'b'] }], watches: ['z'], kinds: { a: 'all', b: 'latest' } }),
        cell('v', { outputs: ['a'] }),
      ],
      { a: {}, b: { people: true }, z: { people: true } },
    );

  // Catches a row for a group no cell watches, its name put on its blocks' rows, a way back drawn to a cell that only
  // reads, and the words of a block read through a group.
  it('has no row of its own; each block has the reads ring in its own kind, and a later write runs nothing', () => {
    const flow = f();
    const m = matrix(flow);
    expect(m.blocks.map((b) => [b.name, b.of])).toEqual([
      ['a', []],
      ['z', []],
      ['b', []],
    ]);
    expect(markAt(m, 't', 'a')).toEqual(mark({ read: true, kind: 'all' }));
    expect(markAt(m, 't', 'b')).toEqual(mark({ read: true, kind: 'latest' }));
    expect(m.waysBack).toBe(0);
    expect(drawnBack(flow)).toBe(0);
    const t = cellIn(flow, 't');
    expect(markWords(t, 'a', markAt(m, 't', 'a'))).toBe('t reads a, all');
    expect(cellSays(t, 'a')).toBe('reads G, all');
    expect(cellSays(t, 'b')).toBe('reads G, latest');
  });
});

describe('an engine cell that watches a datum only the outside writes', () => {
  // Catches a kind put on an engine cell's ring (it has no session), a way back with no writer, and the outside's tag
  // lost for a cell that is given nothing.
  it('has the ring with no kind, no way back, its words by the key alone, and the outside’s tag', () => {
    const flow = small();
    const m = matrix(flow);
    const check = cellIn(flow, 'check');
    expect(markAt(m, 'check', 'main')).toEqual(mark({ watch: true }));
    expect(row(m, 'main').every((s) => s.back === 'none' && !s.backTo)).toBe(true);
    expect(markWords(check, 'main', markAt(m, 'check', 'main'))).toBe('check watches main');
    expect(cellSays(check, 'main')).toBe('watches');
    const tag = chartOf(flow).nodes.find((n) => n.id === outsideNode('check', 'main'));
    expect(tag?.outside).toEqual({ people: false, sources: ['main'] });
  });
});

describe('a datum every agent cell reads that one cell’s own file reads in another kind', () => {
  const f = () =>
    flowOf(
      [
        cell('a', { watches: ['x'] }),
        cell('c', { watches: ['x'], reads: ['a_questions'], kinds: { a_questions: 'latest' } }),
        cell('e', { kind: 'engine', line: null, watches: ['x'] }),
      ],
      { a_questions: { outside: 'answer' }, b_questions: { outside: 'answer' }, x: { people: true } },
      'small',
      {},
      everyAgent({ reads: [['{each}_questions', ['a_questions', 'b_questions']]], kinds: { '{each}_questions': 'all' } }),
    );

  // Catches what every agent cell only reads marked under each cell, the cell's own reads ring drawn in the kind
  // `[every_agent]` gives, and words that drop either list.
  it('shows under that cell its own reads ring and kind, under the others nothing, and says both lists', () => {
    const flow = f();
    const questions = band(matrix(flow), '{each}_questions');
    expect(questions.spots.map((s) => s.mark)).toEqual([mark({}), mark({ read: true, kind: 'latest' }), mark({})]);
    const [a, c, e] = flow.cells;
    expect(everyMarkWords(c, questions)).toBe('c reads a_questions, latest');
    expect(everyMarkWords(a, questions)).toBe('');
    expect(everySays(c, questions)).toMatch(/^reads a_questions, latest; every_agent reads,? all$/);
    expect(everySays(a, questions)).toMatch(/^every_agent reads,? all$/);
    expect(everySays(e, questions)).toBe('');
    expect(contextOf(flow, 'c').map((t) => [t.block, t.kind, t.watched])).toEqual([
      ['a_questions', 'latest', false],
      ['x', 'lastAndLatest', true],
    ]);
  });
});

describe('the box of a chosen cell', () => {
  // Catches what every agent cell is given entering the box, and another order than the compiled one (what it reads,
  // then what it watches).
  it('lists what its own file reads, then what it watches, and nothing every agent cell is given alike', () => {
    const flow = flowOf(
      [
        cell('x', { watches: ['w', 'v'], reads: ['r', 'z'], kinds: { r: 'latest', z: 'all', w: 'all' } }),
        cell('y', { outputs: ['w', 'v'] }),
      ],
      { r: { people: true }, w: {}, v: {}, z: { people: true }, x_questions: { outside: 'answer' } },
      'small',
      {},
      everyAgent({ reads: [['{each}_questions', ['x_questions']]], kinds: { '{each}_questions': 'all' } }),
    );
    expect(cellIn(flow, 'x').given.map((g) => `${g.block} ${g.list}`)).toEqual([
      'x_questions every_agent',
      'r reads',
      'z reads',
      'w watches',
      'v watches',
    ]);
    expect(contextOf(flow, 'x').map((t) => [t.block, t.kind, t.watched])).toEqual([
      ['r', 'latest', false],
      ['z', 'all', false],
      ['w', 'all', true],
      ['v', 'lastAndLatest', true],
    ]);
  });
});

describe('the tags beside a cell on the flowchart', () => {
  // Catches a tag drawn for a datum a cell's own file does not watch, or none for the same datum where a cell's own
  // file watches it or watches a group that holds it.
  it('come only from what the cell’s own file watches, a group that holds the outside’s datum included', () => {
    const flow = flowOf(
      [
        cell('a', { watches: ['request'] }),
        cell('g', { watches: ['tree'], kinds: { tree: 'latest' } }),
        cell('A', { watches: [{ group: 'G', blocks: ['tree', 'x'] }], kinds: { tree: 'all', x: 'all' } }),
      ],
      { request: { people: true }, tree: { outside: 'tree' }, x: {} },
    );
    const chart = chartOf(flow);
    const tags = (id: string) =>
      chart.edges
        .filter((e) => e.kind === 'outside' && e.target === cellNode(id))
        .map((e) => chart.nodes.find((n) => n.id === e.source)?.name);
    expect(tags('a')).toEqual(['request']);
    expect(tags('g')).toEqual(['tree']);
    expect(tags('A')).toEqual(['G']);
  });
});

describe('the words of a row of every_agent', () => {
  // Catches the column of words writing the kind of `[every_agent]` otherwise than the column writes a key and its kind
  // where no name stands between them (`reads all`): `every_agent reads all`, with no cell chosen and with one.
  it('say that it is read, then its kind, as a key and its kind are said where no name stands between them', () => {
    const flow = flowOf(
      [cell('a', { watches: ['x'] })],
      { a_questions: { outside: 'answer' }, x: { people: true } },
      'small',
      {},
      everyAgent({ reads: [['{each}_questions', ['a_questions']]], kinds: { '{each}_questions': 'all' } }),
    );
    const m = matrix(flow);
    const [a] = flow.cells;
    expect(everyIs(band(m, '{each}_questions'), 'The questions.')).toBe('every_agent reads all: The questions.');
    expect(everyIs(band(m, '{each}_questions'), null)).toBe('every_agent reads all');
    expect(everySays(a, band(m, '{each}_questions'))).toBe('every_agent reads all');
  });
});
