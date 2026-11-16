import { describe, expect, it } from 'vitest';

import type { Cell, EveryAgent, Flow, GivenBlock, Watch } from './api';
import { carried, cellNode, chartOf, contextOf, outsideNode } from './graph/model';
import { NO_MARK, markOf, matrix } from './matrix';
import { type Declared, NO_EVERY_AGENT, cell, everyAgent, flowOf, small } from './testing';
import { cellSays, everyIs, everyMarkWords, everySays, groupSays, markWords } from './words';

// The edges of the Flow page drawn on what each cell `watches` and `reads`: a group and one of its blocks named by one
// cell, groups that share a block, a group that holds a datum `[every_agent]` watches, and what a cell's own file says
// of a datum `[every_agent]` names. Each is held against the design (design document `data-and-context`, 5 to 12;
// decisions a-way-back-is-the-only-line-along-a-row and every-agent-is-a-band-of-its-own) and against the flowchart, which
// counts the names its arrows carry.

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

/// The flow as the compiler gives it (`crates/flow/src/compile.rs`, `watching`): each agent cell watches first each
/// block of what `[every_agent]` watches, and is given first each block of what it watches and reads, but a block its
/// own file names by the block's own name; a group its file names does not make its blocks the cell's own. Each block
/// so taken is watched by its own name, in the kind `[every_agent]` gives the name that stands for it.
function compiled(listed: Cell[], blocks: Record<string, Declared>, all: EveryAgent): Flow {
  const flow = flowOf(listed, blocks, 'small', {}, NO_EVERY_AGENT);
  const cells = flow.cells.map((c): Cell => {
    if (c.kind !== 'agent') return c;
    const own = [...c.watches.filter((w) => w.list === 'watches'), ...c.reads].map((n) => n.name);
    const taken = (names: { name: string; blocks: string[] }[]) =>
      names.flatMap((n) => n.blocks.filter((b) => !own.includes(b)).map((block) => ({ name: n.name, block })));
    const runs: Watch[] = taken(all.watches).map(({ name, block }) => ({
      name: block,
      blocks: [block],
      do: all.watches.find((w) => w.name === name)?.do ?? null,
      list: 'every_agent' as const,
    }));
    const alike: GivenBlock[] = [];
    for (const { name, block } of [...taken(all.watches), ...taken(all.reads)]) {
      if (!alike.some((g) => g.block === block)) alike.push({ block, kind: all.kinds[name] ?? 'latest', list: 'every_agent' });
    }
    const rest = c.given.filter((g) => !alike.some((a) => a.block === g.block));
    return { ...c, watches: [...runs, ...c.watches], given: [...alike, ...rest] };
  });
  return { ...flow, cells, everyAgent: all };
}

const GUIDE = everyAgent({
  watches: [['guide', ['guide']]],
  kinds: { guide: 'lastAndLatest' },
  does: { guide: 'Work under it.' },
});

describe('a group that holds a datum every agent cell watches', () => {
  const listed = () => [
    cell('A', { watches: [{ group: 'G', blocks: ['guide', 'x'] }], kinds: { guide: 'all', x: 'latest' } }),
    cell('R', { reads: [{ group: 'H', blocks: ['guide', 'y'] }], kinds: { guide: 'all', y: 'latest' } }),
    cell('N', { reads: ['guide'], kinds: { guide: 'latest' } }),
    cell('B', { outputs: ['guide'] }),
  ];
  const blocks = { guide: { outside: 'guide' }, x: { people: true }, y: { people: true } };

  // Catches the tests' own builder of flows standing for a shape the route never gives: flowOf takes a block of a group a
  // cell's file names as the cell's own, so that `[every_agent]` no longer watches it for that cell, while the compiler
  // keeps it out only where the file names the block by the block's own name (compile.rs, `watching`, `its_own`).
  it('is watched by every agent cell all the same where the tests build the flow, as the compiler gives it', () => {
    const built = flowOf(listed(), blocks, 'small', {}, GUIDE);
    const given = compiled(listed(), blocks, GUIDE);
    const taken = (f: Flow) => f.cells.map((c) => ({ id: c.id, watches: c.watches, given: c.given }));
    expect(taken(built)).toEqual(taken(given));
  });

  // Catches a way back of one write drawn on one name's row and not the other's, or counted apart from the names the
  // flowchart's arrow carries: A is run by a new guide both through G and as every agent cell.
  it('draws the way back on the group’s row and on the band’s row, one for each name, as the flowchart carries them', () => {
    const f = compiled([listed()[0], listed()[3]], { guide: blocks.guide, x: blocks.x }, GUIDE);
    const m = matrix(f);
    expect(m.blocks.map((b) => [b.name, b.group ?? null])).toEqual([
      ['G', ['guide', 'x']],
      ['x', null],
    ]);
    expect(row(m, 'G').map((s) => [s.mark, s.back, s.backTo])).toEqual([
      [mark({ watch: true }), 'right', true],
      [mark({ write: true }), 'left', false],
    ]);
    expect(band(m, 'guide').spots.map((s) => [s.mark, s.back, s.backTo])).toEqual([
      [mark({ watch: true, kind: 'lastAndLatest' }), 'right', true],
      [mark({ watch: true, write: true, kind: 'lastAndLatest' }), 'left', false],
    ]);
    expect(carried(f, cellIn(f, 'B'), cellIn(f, 'A')).map((c) => c.name)).toEqual(['guide', 'G']);
    expect(m.waysBack).toBe(2);
    expect(drawnBack(f)).toBe(2);
    expect(chartOf(f).waysBack).toBe(m.waysBack);
  });
});

describe('a cell that watches a group and one of its blocks by its own name', () => {
  const f = () =>
    flowOf(
      [
        cell('A', { watches: [{ group: 'G', blocks: ['acc', 'x'] }, 'acc'], kinds: { acc: 'all', x: 'latest' } }),
        cell('B', { outputs: ['acc'] }),
      ],
      { acc: {}, x: { people: true } },
    );

  // Catches a way back drawn on one of the two names alone, or counted once where the flowchart's arrow carries two.
  it('has a ring and a way back on each row, counted for each name, as the flowchart counts the names it carries', () => {
    const m = matrix(f());
    expect(m.blocks.map((b) => b.name)).toEqual(['G', 'x', 'acc']);
    expect(markAt(m, 'A', 'G')).toEqual(mark({ watch: true }));
    expect(markAt(m, 'A', 'acc')).toEqual(mark({ watch: true, kind: 'all' }));
    expect(markAt(m, 'A', 'x')).toEqual(mark({}));
    expect(row(m, 'G').map((s) => s.backTo)).toEqual([true, false]);
    expect(row(m, 'acc').map((s) => s.backTo)).toEqual([true, false]);
    expect(carried(f(), cellIn(f(), 'B'), cellIn(f(), 'A')).map((c) => c.name)).toEqual(['G', 'acc']);
    expect(m.waysBack).toBe(2);
    expect(drawnBack(f())).toBe(2);
  });

  // Catches the words of a crossing and of the column naming the group on the block's row, or dropping the kind.
  it('says each crossing by the key and the name its file writes', () => {
    const m = matrix(f());
    const A = cellIn(f(), 'A');
    const B = cellIn(f(), 'B');
    expect(markWords(A, 'G', markAt(m, 'A', 'G'), ['acc', 'x'])).toBe('A watches G');
    expect(markWords(A, 'acc', markAt(m, 'A', 'acc'))).toBe('A watches acc, all');
    expect(markWords(B, 'G', markAt(m, 'B', 'G'), ['acc', 'x'])).toBe('B outputs acc');
    expect(groupSays(A, 'G', ['acc', 'x'])).toBe('watches');
    expect(cellSays(A, 'acc')).toBe('watches all');
  });
});

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

describe('a cell that watches a group and reads one of its blocks by its own name', () => {
  const f = () =>
    flowOf(
      [
        cell('A', { watches: [{ group: 'G', blocks: ['acc', 'x'] }], reads: ['acc'], kinds: { acc: 'latest', x: 'all' } }),
        cell('B', { outputs: ['acc'] }),
      ],
      { acc: {}, x: { people: true } },
    );

  // Catches the reads ring dropped for the group's, or a way back drawn on the block's row to a cell that reads it there.
  it('is drawn as its file writes it: the ring on the group’s row, the reads ring with its kind on the block’s', () => {
    const m = matrix(f());
    expect(m.blocks.map((b) => b.name)).toEqual(['G', 'x', 'acc']);
    expect(markAt(m, 'A', 'G')).toEqual(mark({ watch: true }));
    expect(markAt(m, 'A', 'acc')).toEqual(mark({ read: true, kind: 'latest' }));
    expect(row(m, 'G').map((s) => s.backTo)).toEqual([true, false]);
    expect(row(m, 'acc').every((s) => s.back === 'none' && !s.backTo)).toBe(true);
    expect(m.waysBack).toBe(1);
    expect(drawnBack(f())).toBe(1);
  });

  // Catches the column of words saying only the group's key on a row whose crossing is the reads ring: with the cell
  // chosen, the column says what its file says of the row by its keys, and its file reads acc.
  it('says on the block’s row the key its crossing shows, beside the group it watches it through', () => {
    const m = matrix(f());
    const A = cellIn(f(), 'A');
    expect(markWords(A, 'acc', markAt(m, 'A', 'acc'))).toBe('A reads acc, latest');
    const says = cellSays(A, 'acc');
    expect(says).toContain('watches G');
    expect(says).toContain('reads');
  });
});

describe('a cell that reads a group and watches one of its blocks by its own name', () => {
  const f = () =>
    flowOf(
      [
        cell('A', { reads: [{ group: 'G', blocks: ['acc', 'x'] }], watches: ['acc'], kinds: { acc: 'all', x: 'latest' } }),
        cell('B', { outputs: ['acc'] }),
      ],
      { acc: {}, x: { people: true } },
    );

  // Catches the chosen cell's box taking the list a block was first given by (`reads`, given before what is watched)
  // for whether a new version of it runs the cell: the matrix and a work's history ring acc as watched.
  it('is watched on the block’s row, in a work’s history and in the chosen cell’s box alike', () => {
    const flow = f();
    const m = matrix(flow);
    expect(m.blocks.map((b) => b.name)).toEqual(['x', 'acc']);
    expect(markAt(m, 'A', 'acc')).toEqual(mark({ watch: true, kind: 'all' }));
    expect(markAt(m, 'A', 'x')).toEqual(mark({ read: true, kind: 'latest' }));
    expect(markOf(cellIn(flow, 'A'), 'acc')).toEqual(mark({ watch: true, kind: 'all' }));
    expect(m.waysBack).toBe(1);
    expect(contextOf(flow, 'A').map((t) => `${t.block} ${t.kind} ${t.watched ? 'watches' : 'reads'}`)).toEqual([
      'acc all watches',
      'x latest reads',
    ]);
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
    compiled(
      [
        cell('a', { watches: ['x'] }),
        cell('c', { watches: ['x'], reads: ['a_questions'], kinds: { a_questions: 'latest' } }),
        cell('e', { kind: 'engine', line: null, watches: ['x'] }),
      ],
      { a_questions: { outside: 'answer' }, b_questions: { outside: 'answer' }, x: { people: true } },
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

describe('a datum every agent cell watches that one cell’s own file reads', () => {
  const f = () =>
    compiled(
      [
        cell('a', {}),
        cell('r', { reads: ['guide'], watches: ['x'], kinds: { guide: 'all' } }),
        cell('w', { outputs: ['guide'] }),
      ],
      { guide: { outside: 'guide' }, x: { people: true } },
      GUIDE,
    );

  // Catches a cell whose own file reads the guide still drawn, counted or tipped as run by it: a datum its file names
  // itself it takes as its file names it.
  it('runs nothing there: the reads ring in its own kind, and a later write runs only the others again', () => {
    const flow = f();
    const m = matrix(flow);
    const guide = band(m, 'guide');
    expect(guide.spots.map((s) => [s.mark, s.back, s.backTo])).toEqual([
      [mark({ watch: true, kind: 'lastAndLatest' }), 'right', true],
      [mark({ read: true, kind: 'all' }), 'through', false],
      [mark({ watch: true, write: true, kind: 'lastAndLatest' }), 'left', false],
    ]);
    const [a, r, w] = flow.cells;
    expect(carried(flow, w, r)).toEqual([]);
    expect(m.waysBack).toBe(1);
    expect(drawnBack(flow)).toBe(1);
    expect(markOf(r, 'guide')).toEqual(mark({ read: true, kind: 'all' }));
    expect(everyMarkWords(r, guide)).toBe('r reads guide, all');
    expect(everyMarkWords(a, guide)).toBe('a every_agent watches guide, last_and_latest');
    expect(everySays(r, guide)).toBe('reads guide, all');
    expect(everySays(a, guide)).toMatch(/^every_agent watches,? last_and_latest$/);
  });
});

describe('a datum every agent cell watches, written by a later cell', () => {
  const deep = (value: string) => [{ on: 'block' as const, block: 'deep', value }];
  const f = () =>
    compiled(
      [
        cell('f', { outputs: ['deep'] }),
        cell('a', { when: deep('no') }),
        cell('b', {}),
        cell('c', { when: deep('yes'), outputs: ['guide'] }),
        cell('e', { kind: 'engine', line: null, watches: ['deep'] }),
      ],
      { deep: { values: ['yes', 'no'] }, guide: { outside: 'guide' } },
      GUIDE,
    );

  // Catches a way back on the band to a cell that cannot run where the writer does, or to an engine cell, which
  // `[every_agent]` does not reach, and the flowchart's arrows disagreeing with the band's count.
  it('draws a way back on the band to each earlier agent cell that can run where the writer does, as the flowchart', () => {
    const flow = f();
    const m = matrix(flow);
    const guide = band(m, 'guide');
    expect(guide.spots.map((s) => s.backTo)).toEqual([true, false, true, false, false]);
    expect(guide.spots.map((s) => s.back)).toEqual(['right', 'through', 'through', 'left', 'none']);
    expect(guide.spots[4].mark).toEqual(mark({}));
    const [first, a, b, c] = flow.cells;
    expect(carried(flow, c, first)).toEqual([{ name: 'guide', group: false }]);
    expect(carried(flow, c, b)).toEqual([{ name: 'guide', group: false }]);
    expect(chartOf(flow).edges.some((e) => e.kind === 'arrow' && e.from === c.id && e.to === a.id)).toBe(false);
    expect(m.waysBack).toBe(2);
    expect(drawnBack(flow)).toBe(2);
  });
});

describe('a name holding {each} that every agent cell watches', () => {
  const f = () =>
    compiled(
      [cell('a', { outputs: ['a_words'] }), cell('b', { outputs: ['b_words'] })],
      { a_words: { outside: 'words' }, b_words: { outside: 'words' } },
      everyAgent({ watches: [['{each}_words', ['a_words', 'b_words']]], kinds: { '{each}_words': 'all' } }),
    );

  // Catches a later cell's write of its own datum not running again an earlier cell that watches it as every agent
  // cell, and the words of a crossing of the band that both watches and writes.
  it('runs an earlier cell again for a later cell’s write of its own, once a pair, and says each key', () => {
    const flow = f();
    const m = matrix(flow);
    const words = band(m, '{each}_words');
    expect(words.spots.map((s) => [s.mark, s.back, s.backTo])).toEqual([
      [mark({ watch: true, write: true, kind: 'all' }), 'right', true],
      [mark({ watch: true, write: true, kind: 'all' }), 'left', false],
    ]);
    expect(m.waysBack).toBe(1);
    expect(drawnBack(flow)).toBe(1);
    const [a] = flow.cells;
    expect(everyMarkWords(a, words)).toBe('a every_agent watches a_words, b_words, all; outputs a_words');
    expect(everySays(a, words)).toMatch(/^outputs a_words; every_agent watches,? all$/);
  });
});

describe('the box of a chosen cell', () => {
  // Catches what every agent cell is given entering the box, a datum `[every_agent]` watches left out of it though the
  // cell's own file reads it, and another order than the compiled one (what it reads, then what it watches).
  it('lists what its own file reads, then what it watches, and nothing every agent cell is given alike', () => {
    const flow = compiled(
      [
        cell('x', { watches: ['w', 'v'], reads: ['r', 'guide'], kinds: { r: 'latest', guide: 'all', w: 'all' } }),
        cell('y', { outputs: ['w', 'v'] }),
      ],
      { r: { people: true }, w: {}, v: {}, guide: { outside: 'guide' }, x_questions: { outside: 'answer' } },
      everyAgent({
        watches: [['guide', ['guide']]],
        reads: [['{each}_questions', ['x_questions']]],
        kinds: { guide: 'lastAndLatest', '{each}_questions': 'all' },
      }),
    );
    expect(cellIn(flow, 'x').given.map((g) => `${g.block} ${g.list}`)).toEqual([
      'x_questions every_agent',
      'r reads',
      'guide reads',
      'w watches',
      'v watches',
    ]);
    expect(contextOf(flow, 'x').map((t) => [t.block, t.kind, t.watched])).toEqual([
      ['r', 'latest', false],
      ['guide', 'all', false],
      ['w', 'all', true],
      ['v', 'lastAndLatest', true],
    ]);
  });
});

describe('the tags beside a cell on the flowchart', () => {
  // Catches a tag drawn for what every agent cell watches, or none for the same datum where a cell's own file watches it
  // or watches a group that holds it.
  it('come only from what the cell’s own file watches, a group that holds the outside’s datum included', () => {
    const flow = compiled(
      [
        cell('a', { watches: ['request'] }),
        cell('g', { watches: ['guide'], kinds: { guide: 'latest' } }),
        cell('A', { watches: [{ group: 'G', blocks: ['guide', 'x'] }], kinds: { guide: 'all', x: 'all' } }),
      ],
      { request: { people: true }, guide: { outside: 'guide' }, x: {} },
      GUIDE,
    );
    const chart = chartOf(flow);
    const tags = (id: string) =>
      chart.edges
        .filter((e) => e.kind === 'outside' && e.target === cellNode(id))
        .map((e) => chart.nodes.find((n) => n.id === e.source)?.name);
    expect(tags('a')).toEqual(['request']);
    expect(tags('g')).toEqual(['guide']);
    expect(tags('A')).toEqual(['G']);
  });
});

describe('the words of a row of every_agent', () => {
  // Catches the column of words writing the list and the kind of `[every_agent]` otherwise than the column writes a key
  // and its kind where no name stands between them (`watches all`): `every_agent watches last_and_latest`, with no cell
  // chosen and with one.
  it('say its list, then its kind, as a key and its kind are said where no name stands between them', () => {
    const flow = compiled(
      [cell('a', { watches: ['x'] })],
      { guide: { outside: 'guide', what: 'The guide.' }, a_questions: { outside: 'answer' }, x: { people: true } },
      everyAgent({
        watches: [['guide', ['guide']]],
        reads: [['{each}_questions', ['a_questions']]],
        kinds: { guide: 'lastAndLatest', '{each}_questions': 'all' },
      }),
    );
    const m = matrix(flow);
    const [a] = flow.cells;
    expect(everyIs(band(m, 'guide'), 'The guide.')).toBe('every_agent watches last_and_latest: The guide.');
    expect(everyIs(band(m, '{each}_questions'), null)).toBe('every_agent reads all');
    expect(everySays(a, band(m, 'guide'))).toBe('every_agent watches last_and_latest');
    expect(everySays(a, band(m, '{each}_questions'))).toBe('every_agent reads all');
  });
});
