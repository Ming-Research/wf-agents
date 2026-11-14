import { describe, expect, it } from 'vitest';

import type { CellNow, Item, Items, Now, Summary } from '../works-api';
import { asksOf, shownTree, type Shown } from './items';
import { indexTree } from './tree';

const CELLS = ['plan', 'implement', 'merge'];

/// A work of item `item` doing `now`, its cells as `states` say.
function work(item: number, now: Now, states: CellNow['state'][] = ['notYet', 'notYet', 'notYet']): Summary {
  return {
    number: item * 10,
    item,
    flow: 'small',
    added: 0,
    paused: false,
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

  it("takes an item's standing from its work: merged is done, cancelled is cancelled, else open", () => {
    const shown = shownTree({ items: [item(8, 'done', [9, 10]), item(9, 'open'), item(10, 'cancelled', [11]), item(11, 'open')] });
    expect(shown.map((s) => [s.id, s.standing])).toEqual([[8, 'done']]);
    expect(shown[0].children.map((s) => [s.id, s.standing])).toEqual([
      [9, 'open'],
      [10, 'cancelled'],
    ]);
  });
});

const waits: Now = {
  kind: 'waits',
  at: 'implement',
  on: [
    { kind: 'call', call: 1, who: 'owner', what: { kind: 'question', who: 'owner', question: 'Which?' } },
    { kind: 'call', call: 2, who: 'architect', what: { kind: 'question', who: 'architect', question: 'Where?' } },
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

/// At the check before the merge, which waits for the owner's answers, as the engine says of them.
const merge: Now = {
  kind: 'waits',
  at: 'merge',
  on: [{ kind: 'precondition', test: 'asked = answered', words: 'asked = answered', by: [{ kind: 'outside', source: 'asked' }] }],
};

describe('what a card shows of an item', () => {
  it('counts the questions to the owner that hold the work, and nothing else the owner gives', () => {
    expect(asksOf({ now: waits })).toBe(1);
    expect(asksOf({ now: { kind: 'paused', by: 'owner', why: null } })).toBe(0);
    // At the merge's check, the questions the engine names as holding it.
    expect(asksOf({ now: merge, holding: ['measure', 'read-the-time'] })).toBe(2);
  });

  it('shows the work of an item once it has begun, and while it goes on; until it has begun, the items it waits for', () => {
    const notBegun = item(1, 'open', [2, 3]);
    const begun: Item = { ...item(2, 'open'), work: work(2, waits, ['done', 'resting', 'notYet']) };
    const shown = shownTree({ items: [notBegun, begun, item(3, 'done')] });
    expect(shown[0].work).toBeNull();
    expect(shown[0].begun).toBe(false);
    expect(shown[0].blocked).toBe(1);
    const w = shown[0].children[0];
    expect(w.begun).toBe(true);
    expect(w.asks).toBe(1);
    expect(w.work).toMatchObject({ number: 20, usdMicros: 1_500_000 });
    expect(w.work?.cells.map((c) => c.state)).toEqual(['done', 'resting', 'notYet']);
  });

  it('shows the work of an item that has not begun while it is paused, at fault or waits, as every new item is', () => {
    const paused: Item = { ...item(1, 'open', [4]), work: work(1, { kind: 'paused', by: 'owner', why: null }) };
    const fault: Item = { ...item(2, 'open'), work: work(2, { kind: 'fault', words: 'No session' }) };
    const owner: Now = {
      kind: 'waits',
      at: 'plan',
      on: [{ kind: 'precondition', test: 'read', words: 'The owner read it', by: [{ kind: 'owner' }] }],
    };
    const waiting: Item = { ...item(3, 'open'), work: work(3, owner) };
    const shown = shownTree({ items: [paused, fault, waiting, item(4, 'open')] });
    expect(shown.map((s) => [s.id, s.begun, s.work?.now.kind])).toEqual([
      [1, false, 'paused'],
      [2, false, 'fault'],
      [3, false, 'waits'],
    ]);
    // Not begun, it still says how many items it waits for.
    expect(shown[0].blocked).toBe(1);
    // A precondition the owner makes hold is no question: it marks the card, and counts nothing.
    expect(shown[2].asks).toBe(0);
    expect(shown[2].work?.where).toEqual({ cell: 'plan', title: null, says: [['requires', 'read']] });
  });
});

describe('where a card says its work waits for the owner', () => {
  const titles: Record<string, string> = { plan: 'Initial research', implement: 'Implement', merge: 'Check before the merge' };
  const where = (now: Now, states?: CellNow['state'][]) => {
    const i: Item = { ...item(1, 'open'), work: work(1, now, states) };
    return shownTree({ items: [i] }, (flow, cell) => (flow === 'small' ? (titles[cell] ?? null) : null))[0].work?.where;
  };

  it('names the cell by its id and its title, and what it waits for as keys and values', () => {
    const answer: Now = { kind: 'waits', at: 'plan', on: [{ kind: 'answer', question: 'scope' }] };
    expect(where(answer)).toEqual({ cell: 'plan', title: 'Initial research', says: [['answer', 'scope']] });
    expect(where(merge)).toEqual({
      cell: 'merge',
      title: 'Check before the merge',
      says: [['requires', 'asked = answered']],
    });
    // Of what the work waits for, only what the owner gives.
    expect(where(waits)?.says).toEqual([
      ['question', 'Which?'],
      ['requires', 'confirmed'],
      ['limit', 'runs 3'],
    ]);
  });

  it('says a work the owner paused as paused, at the cell it would go on at, apart from a question', () => {
    expect(where({ kind: 'paused', by: 'owner', why: null }, ['done', 'again', 'notYet'])).toEqual({
      cell: 'implement',
      title: 'Implement',
      says: [['paused', null]],
    });
  });

  it('says nothing of a work that waits for anything else, runs, or that the engine stopped', () => {
    const slot: Now = { kind: 'waits', at: 'plan', on: [{ kind: 'slot' }] };
    const items: Now = {
      kind: 'waits',
      at: 'plan',
      on: [{ kind: 'precondition', test: 'deps', words: 'deps', by: [{ kind: 'outside', source: 'dependencies' }] }],
    };
    for (const now of [slot, items, { kind: 'next', cell: 'plan' } as Now, { kind: 'paused', by: 'engine', why: 'x' } as Now])
      expect(where(now, ['resting', 'notYet', 'notYet'])).toBeNull();
  });
});
