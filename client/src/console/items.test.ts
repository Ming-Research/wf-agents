import { describe, expect, it } from 'vitest';

import type { CellNow, Item, Items, Now, Summary, Wait } from '../works-api';
import { asksOf, findItems, itemPaths, pathWords, STANDINGS, shownTree, type Shown } from './items';
import { indexTree } from './tree';

const CELLS = ['plan', 'implement', 'merge'];

/// A work of item `item` doing `now`, its cells as `states` say; its first round begun where a round is under way or a
/// cell has run.
function work(item: number, now: Now, states: CellNow['state'][] = ['notYet', 'notYet', 'notYet']): Summary {
  const begun = now.kind === 'round' || states.some((s) => s !== 'notYet' && s !== 'skipped');
  return {
    from: begun ? 1 : null,
    number: item * 10,
    item,
    flow: 'small',
    added: 0,
    paused: null,
    now,
    cells: CELLS.map((cell, i) => ({
      cell,
      state: states[i],
      finished: states[i] === 'done' ? 1 : 0,
      record: null,
      open: states[i] === 'running' || states[i] === 'resting' ? 1 : null,
    })),
    spent: { usdMicros: 1_500_000, inputTokens: 0, cachedTokens: 0, outputTokens: 0, workingMs: 0, rounds: 3 },
    last: null,
  };
}

const NOW: Record<'open' | 'done' | 'cancelled', Now> = {
  open: { kind: 'next', cell: 'plan' },
  done: { kind: 'merged' },
  cancelled: { kind: 'cancelled' },
};

function item(number: number, standing: 'open' | 'done' | 'cancelled', dependsOn: number[] = []): Item {
  const history = { first: { title: `t${number}`, level: '2', dependsOn }, changes: [] };
  return {
    number,
    title: `t${number}`,
    level: '2',
    project: 'wf-agents',
    added: 0,
    dependsOn,
    history,
    work: work(number, NOW[standing]),
  };
}

/// A tree as lines: each item's number, indented by its depth, a shadow's marked.
const lines = (roots: Shown[], depth = 0): string[] =>
  roots.flatMap((r) => [`${'  '.repeat(depth)}${r.id}${r.shadow ? ' shadow' : ''}`, ...lines(r.children, depth + 1)]);

/// A tree as the console draws it: as `lines`, without what a folded item folds away, the folded item marked with how
/// many stand under it by how they stand.
const drawnLines = (roots: Shown[], depth = 0): string[] =>
  roots.flatMap((r) => {
    const head = `${'  '.repeat(depth)}${r.id}${r.shadow ? ' shadow' : ''}`;
    if (r.fold?.folded !== true) return [head, ...drawnLines(r.children, depth + 1)];
    const under = r.fold.under;
    const counts = STANDINGS.filter((s) => under[s] > 0).map((s) => ` ${s} ${under[s]}`);
    if (under.elsewhere > 0) counts.push(` elsewhere ${under.elsewhere}`);
    return [`${head} folded${counts.join('')}`];
  });

const ALL = new Set(STANDINGS);

describe('the tree the console shows', () => {
  // 1 needs 2 and 3, and 4 needs 3 too, its main dependent 1, the first by number; 3 needs 5, which is cancelled; 6 and
  // 7, both done, are a tree of their own; 8 is done and stands under 4, and 9, still open, under 8; 10 is done and
  // stands under 2, and 4 needs it too.
  const data: Items = {
    items: [
      item(1, 'open', [2, 3]),
      item(2, 'open', [10]),
      item(3, 'open', [5]),
      item(4, 'open', [3, 8, 10]),
      item(5, 'cancelled'),
      item(6, 'done', [7]),
      item(7, 'done'),
      item(8, 'done', [9]),
      item(9, 'open'),
      item(10, 'done'),
    ],
  };

  it('puts each item under its main dependent and a shadow of it under each other, after the items there, and leaves out what is done or cancelled with nothing open under it', () => {
    expect(lines(shownTree(data))).toEqual(['1', '  2', '  3', '4', '  8', '    9', '  3 shadow']);
  });

  it('indexes the tree by level, each item with its place among its level and the items under it', () => {
    const { tree, items } = indexTree(shownTree(data), (s) => s.children);
    expect(items.map((s) => s.id)).toEqual([1, 2, 3, 4, 8, 9, 3]);
    expect(tree.columns.map((c) => c.map((i) => items[i].id))).toEqual([[1, 4], [2, 3, 8, 3], [9]]);
    expect(Array.from(tree.childStart)).toEqual([0, 0, 0, 2, 0, 0, 1]);
  });

  it('shows the items that stand as one of the kinds ticked, and what stands over them, shadows only of items it shows', () => {
    const shows = (...kinds: ('open' | 'done' | 'cancelled')[]) => lines(shownTree(data, new Set(kinds)));
    expect(shows('open')).toEqual(lines(shownTree(data)));
    expect(shows('done')).toEqual(['1', '  2', '    10', '4', '  8', '  10 shadow', '6', '  7']);
    expect(shows('cancelled')).toEqual(['1', '  3', '    5']);
    expect(shows('open', 'done', 'cancelled')).toEqual([
      '1',
      '  2',
      '    10',
      '  3',
      '    5',
      '4',
      '  8',
      '    9',
      '  3 shadow',
      '  10 shadow',
      '6',
      '  7',
    ]);
    expect(shows()).toEqual([]);
  });

  it("gives a work going on the lines it changed, and an item whose work ended its work's, none where it changed none", () => {
    const changed = (i: Item, added: number, removed: number): Item => ({
      ...i,
      work: { ...i.work, changed: { files: 2, added, removed } },
    });
    // A work that has begun: its first cell runs.
    const going = changed({ ...item(1, 'open'), work: work(1, NOW.open, ['running', 'notYet', 'notYet']) }, 210, 40);
    const [open, done, none] = shownTree(
      { items: [going, changed(item(2, 'done'), 7, 3), changed(item(3, 'done'), 0, 0)] },
      new Set(['open', 'done']),
    );
    expect(open.work?.lines).toEqual({ added: 210, removed: 40, files: 2 });
    expect(open.endedLines).toBeNull();
    expect(done.work).toBeNull();
    expect(done.endedLines).toEqual({ added: 7, removed: 3, files: 2 });
    expect(none.endedLines).toBeNull();
  });

  it("takes an item's standing from its work: merged is done, cancelled is cancelled, else open", () => {
    const shown = shownTree({ items: [item(8, 'done', [9, 10]), item(9, 'open'), item(10, 'cancelled', [11]), item(11, 'open')] });
    expect(shown.map((s) => [s.id, s.standing])).toEqual([[8, 'done']]);
    expect(shown[0].children.map((s) => [s.id, s.standing])).toEqual([
      [9, 'open'],
      [10, 'cancelled'],
    ]);
  });
});

describe('the folds of the tree', () => {
  // 10, open, depends on 11 and 12. 11, done, depends on 13 and 14, both done. 12, done, depends on 15, done, which
  // depends on 16, done, and 17, cancelled. 18, open, depends on 19, open, and 20, done. 21, cancelled, depends on 22,
  // open.
  const data: Items = {
    items: [
      item(10, 'open', [11, 12]),
      item(11, 'done', [13, 14]),
      item(12, 'done', [15]),
      item(13, 'done'),
      item(14, 'done'),
      item(15, 'done', [16, 17]),
      item(16, 'done'),
      item(17, 'cancelled'),
      item(18, 'open', [19, 20]),
      item(19, 'open'),
      item(20, 'done'),
      item(21, 'cancelled', [22]),
      item(22, 'open'),
    ],
  };

  it('fold an item where neither it nor anything under it is open, and leave the others unfolded, each counting what stands under it by how it stands', () => {
    expect(drawnLines(shownTree(data, ALL))).toEqual([
      '10',
      '  11 folded done 2',
      '  12 folded done 2 cancelled 1',
      '18',
      '  19',
      '  20',
      '21',
      '  22',
    ]);
    // A folded item still holds what stands under it; an item with nothing under it has no fold.
    const [ten] = shownTree(data, ALL);
    expect(ten.fold).toEqual({ folded: false, under: { open: 0, done: 6, cancelled: 1, elsewhere: 0 } });
    expect(ten.children[0].children.map((c) => c.id)).toEqual([13, 14]);
    expect(ten.children[0].children[0].fold).toBeNull();
  });

  it('fold or unfold an item as the owner last chose, over their own rule', () => {
    const chosen = new Map([
      [12, false],
      [21, true],
    ]);
    expect(drawnLines(shownTree(data, ALL, chosen))).toEqual([
      '10',
      '  11 folded done 2',
      '  12',
      '    15 folded done 1 cancelled 1',
      '18',
      '  19',
      '  20',
      '21 folded open 1',
    ]);
  });

  it('fold the shadows under an item as well, an item with shadows alone under it too, counting them apart', () => {
    // 1, open, depends on 2 and 3, both open, which stand under it; 4, done, depends on 2, and 5, open, on 3: a shadow
    // of each stands under them, alone. 6, done, depends on 7, done, and on 2, its shadow beside 7.
    const shadowed: Items = {
      items: [
        item(1, 'open', [2, 3]),
        item(2, 'open'),
        item(3, 'open'),
        item(4, 'done', [2]),
        item(5, 'open', [3]),
        item(6, 'done', [7, 2]),
        item(7, 'done'),
      ],
    };
    expect(drawnLines(shownTree(shadowed, ALL))).toEqual([
      '1',
      '  2',
      '  3',
      '4 folded elsewhere 1',
      '5',
      '  3 shadow',
      '6 folded done 1 elsewhere 1',
    ]);
    const five = shownTree(shadowed, ALL).find((s) => s.id === 5);
    expect(five?.fold).toEqual({ folded: false, under: { open: 0, done: 0, cancelled: 0, elsewhere: 1 } });
  });

  it('count and fold only what the boxes ticked show, and hide what is under an item whatever the boxes show', () => {
    expect(drawnLines(shownTree(data))).toEqual(['10', '18', '  19', '21', '  22']);
    expect(drawnLines(shownTree(data, new Set(['done'])))).toEqual([
      '10',
      '  11 folded done 2',
      '  12 folded done 2',
      '18',
      '  20',
    ]);
    expect(drawnLines(shownTree(data, new Set(['open']), new Map([[18, true]])))).toEqual([
      '10',
      '18 folded open 1',
      '21',
      '  22',
    ]);
  });
});

describe('the search of the items', () => {
  // 1 depends on 2 and 3, and 2 on 4: the paths 1-2, 1-3 and 1-2-4. 5, cancelled, and 65 stand alone.
  const named = (i: Item, title: string): Item => ({ ...i, title });
  const data: Items = {
    items: [
      named(item(1, 'open', [2, 3]), 'Find an item'),
      named(item(2, 'done', [4]), 'Show the legend'),
      named(item(3, 'open'), '搜索条目'),
      named(item(4, 'done'), 'Draw the marks'),
      named(item(5, 'cancelled'), 'Find nothing'),
      named(item(65, 'open'), 'Read the record'),
    ],
  };
  const found = (query: string) => findItems(data, query).map((f) => pathWords(f.path));

  it('writes each item by its path in the tree, from its root down, as the tree places it', () => {
    const paths = itemPaths(data);
    const words = [1, 2, 3, 4, 5, 65].map((n) => pathWords(paths.get(n) ?? []));
    expect(words).toEqual(['#1', '1-2', '1-3', '1-2-4', '#5', '#65']);
  });

  it('finds an item by its number, with or without #, and as the number is typed, the items whose number begins so', () => {
    expect(found('65')).toEqual(['#65']);
    expect(found('#65')).toEqual(['#65']);
    expect(found(' 3 ')).toEqual(['1-3']);
    expect(found('6')).toEqual(['#65']);
    expect(found('1')).toEqual(['#1']);
  });

  it('finds the item a path from its root names, and the items under it, in the order of the tree', () => {
    expect(found('1-2')).toEqual(['1-2', '1-2-4']);
    expect(found('1-2-4')).toEqual(['1-2-4']);
    expect(found('1-')).toEqual(['1-2', '1-2-4', '1-3']);
    // A path that does not begin at a root names nothing.
    expect(found('2-4')).toEqual([]);
  });

  it('finds as a path is typed a digit at a time: the last number begun names each number it begins, the very one first', () => {
    // 26 depends on 27, 28 and 29, and 27 on 30.
    const typed: Items = {
      items: [
        item(26, 'open', [27, 28, 29]),
        item(27, 'done', [30]),
        item(28, 'open'),
        item(29, 'cancelled'),
        item(30, 'done'),
      ],
    };
    const paths = (query: string) => findItems(typed, query).map((f) => pathWords(f.path));
    expect(paths('26-2')).toEqual(['26-27', '26-27-30', '26-28', '26-29']);
    expect(paths('26-28')).toEqual(['26-28']);
    expect(paths('26-27-3')).toEqual(['26-27-30']);
    expect(paths('2')).toEqual(['#26', '26-27', '26-28', '26-29']);
  });

  it('finds the items whose title holds what is typed, in any case and as Chinese is typed, open, done or cancelled', () => {
    expect(found('find')).toEqual(['#1', '#5']);
    expect(found('LEGEND')).toEqual(['1-2']);
    expect(found('搜索')).toEqual(['1-3']);
    expect(found('nowhere')).toEqual([]);
  });

  it('finds nothing for nothing typed, and at most eight', () => {
    expect(found('  ')).toEqual([]);
    const many: Items = { items: Array.from({ length: 12 }, (_, k) => named(item(k + 1, 'open'), `Fix ${k + 1}`)) };
    expect(findItems(many, 'fix')).toHaveLength(8);
  });
});

describe('what a card shows of an item', () => {
  const waits: Now = {
    kind: 'waits',
    at: 'implement',
    on: [
      { kind: 'call', call: 1, who: 'owner', what: { kind: 'atLimit', limit: { of: 'returns', sentBack: 2 } } },
      { kind: 'call', call: 3, who: null, what: { kind: 'tool', name: 'ci', input: '' } },
      { kind: 'precondition', test: 'confirmed', words: 'The owner confirmed it', by: [{ kind: 'owner' }] },
      {
        kind: 'precondition',
        test: 'may_start',
        words: 'The items it depends on are merged',
        by: [{ kind: 'outside', source: 'dependencies' }],
      },
      { kind: 'precondition', test: 'checked', words: 'The check before the merge passed', by: [{ kind: 'cell', cell: 'M1' }] },
      { kind: 'limit', runs: 3 },
    ],
  };

  it('counts the things a work waits on the owner for as the engine\'s list says, and nothing of what it waits for that the list leaves out', () => {
    const listed = [
      { key: 'limit', why: "The cell's run had 2 returns sent back, which is its limit.", ends: { kind: 'goOn' as const, call: 1 } },
      { key: 'write-confirmed', why: 'implement requires test = confirmed, which confirmed keeps from holding.', ends: { kind: 'write' as const, block: 'confirmed', value: 'yes' } },
      { key: 'answer-which', why: 'Which?', ends: { kind: 'answer' as const, question: 'which', version: 1 } },
    ];
    expect(asksOf({ waits: listed })).toBe(3);
    expect(asksOf({ waits: [] })).toBe(0);
    // A card's mark is the same count.
    const shown = shownTree({ items: [{ ...item(1, 'open'), work: { ...work(1, waits, ['done', 'resting', 'notYet']), waits: listed } }] });
    expect(shown[0].asks).toBe(3);
  });

  it('stands every item an item depends on under it, open, done or cancelled', () => {
    const items = [item(1, 'open', [2, 3, 4]), item(2, 'open'), item(3, 'done'), item(4, 'cancelled')];
    const shown = shownTree({ items }, ALL);
    expect(shown[0].children.map((s) => [s.id, s.standing])).toEqual([
      [2, 'open'],
      [3, 'done'],
      [4, 'cancelled'],
    ]);
  });

  it('shows the work of an item once it has begun, and while it goes on', () => {
    const notBegun = item(1, 'open', [2, 3]);
    const listed: Wait[] = ['limit', 'write-confirmed', 'answer-which'].map((key) => ({ key, why: `${key}.`, ends: { kind: 'resume' } }));
    const begun: Item = { ...item(2, 'open'), work: { ...work(2, waits, ['done', 'resting', 'notYet']), waits: listed } };
    const shown = shownTree({ items: [notBegun, begun, item(3, 'done')] });
    expect(shown[0].work).toBeNull();
    expect(shown[0].begun).toBe(false);
    const w = shown[0].children[0];
    expect(w.begun).toBe(true);
    expect(w.asks).toBe(3);
    expect(w.work).toMatchObject({ number: 20, usdMicros: 1_500_000 });
    expect(w.work?.cells.map((c) => c.state)).toEqual(['done', 'resting', 'notYet']);
  });

  it("draws an item whose work's flow delivers, as the engine says, as a deliverable with no row of its work", () => {
    const delivering: Item = { ...item(1, 'open', [2]), work: { ...work(1, NOW.open), delivers: true } };
    const shown = shownTree({ items: [delivering, item(2, 'open')] });
    expect(shown[0].deliver).toBe(true);
    expect(shown[0].work).toBeNull();
    // A work whose flow does not deliver is no deliverable, whatever its flow is called.
    const making: Item = { ...item(1, 'open', [2]), work: { ...work(1, NOW.open), flow: 'deliver', delivers: false } };
    const made = shownTree({ items: [making, item(2, 'open')] });
    expect(made[0].deliver).toBe(false);
  });

  it('gives a work that has not run a round no second row, its colours only where it is halted or waits for the owner', () => {
    const paused: Item = {
      ...item(1, 'open', [4]),
      work: work(1, { kind: 'paused', at: 'plan', by: 'owner', why: null }),
    };
    const fault: Item = { ...item(2, 'open'), work: work(2, { kind: 'fault', words: 'No session' }) };
    const owner: Now = {
      kind: 'waits',
      at: 'plan',
      on: [{ kind: 'precondition', test: 'read', words: 'The owner read it', by: [{ kind: 'owner' }] }],
    };
    const ruling: Wait = { key: 'write-read', why: 'plan requires test = read, which read keeps from holding.', ends: { kind: 'write', block: 'read', value: 'yes' } };
    const waiting: Item = { ...item(3, 'open'), work: { ...work(3, owner), waits: [ruling] } };
    // Brought over from elsewhere: its first cell stands done, and no round of it ever ran.
    const held: Now = { kind: 'paused', at: 'implement', by: 'owner', why: null };
    const brought: Item = { ...item(5, 'open'), work: { ...work(5, held, ['done', 'notYet', 'notYet']), from: null } };
    const shown = shownTree({ items: [paused, fault, waiting, item(4, 'open'), brought] });
    // Paused before its first round, as the one brought over is, it is drawn as an open item with no work; halted, or
    // waiting for the owner, the card takes the work's state, with no time of a first round and so no second row.
    expect(shown.map((s) => [s.id, s.begun, s.work?.now.kind ?? null, s.work?.from ?? null])).toEqual([
      [1, false, null, null],
      [2, false, 'fault', null],
      [3, false, 'waits', null],
      [5, false, null, null],
    ]);
    // Not begun, it still says what it waits on the owner for.
    expect(shown[2].asks).toBe(1);
    // Once its first round has begun, the same work paused is drawn, with the time of that round.
    const ran = shownTree({ items: [{ ...paused, work: { ...paused.work, from: 1 } }, item(4, 'open')] });
    expect(ran[0].begun).toBe(true);
    expect([ran[0].work?.now.kind, ran[0].work?.from]).toEqual(['paused', 1]);
  });
});
