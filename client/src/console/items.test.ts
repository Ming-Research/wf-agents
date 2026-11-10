import { describe, expect, it } from 'vitest';

import type { CellNow, Item, Items, Now, Summary } from '../works-api';
import { asksOf, hereOf, shownTree, type Shown } from './items';
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
  return { number, title: `t${number}`, added: 0, dependsOn, works: [work(number, NOW[standing])] };
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

  it("takes an item's standing from its latest work: merged is done, cancelled is cancelled, else open", () => {
    const later: Item = { ...item(8, 'cancelled', [9]), works: [work(8, NOW.cancelled), work(8, NOW.done)] };
    const shown = shownTree({ items: [later, item(9, 'open')] });
    expect(shown.map((s) => [s.id, s.standing])).toEqual([[8, 'done']]);
    expect(shown[0].children.map((s) => [s.id, s.standing])).toEqual([[9, 'open']]);
  });
});

describe('what a card shows of an item', () => {
  const waits: Now = {
    kind: 'waits',
    at: 'implement',
    on: [
      { kind: 'call', call: 1, who: 'owner', what: { kind: 'question', who: 'owner', question: 'Which?' } },
      { kind: 'call', call: 2, who: 'architect', what: { kind: 'question', who: 'architect', question: 'Where?' } },
      { kind: 'call', call: 3, who: null, what: { kind: 'tool', name: 'ci', input: '' } },
      { kind: 'precondition', test: 'confirmed', words: 'The owner confirmed it', who: 'owner' },
      { kind: 'limit', runs: 3 },
    ],
  };

  it('counts the things a work waits for that the owner gives: their questions, their preconditions, a run let on', () => {
    expect(asksOf(waits)).toBe(3);
    expect(asksOf({ kind: 'paused' })).toBe(0);
  });

  it('puts a work at the cell its round, its wait or what it does next names, else at its open run or the first to run', () => {
    expect(hereOf({ now: { kind: 'round', cell: 'merge', run: 1, round: 1, since: 0 }, cells: [] })).toBe('merge');
    expect(hereOf({ now: waits, cells: [] })).toBe('implement');
    expect(hereOf(work(1, { kind: 'paused' }, ['done', 'resting', 'notYet']))).toBe('implement');
    expect(hereOf(work(1, { kind: 'paused' }, ['done', 'again', 'notYet']))).toBe('implement');
    expect(hereOf(work(1, { kind: 'merged' }, ['done', 'done', 'done']))).toBeNull();
  });

  it('shows the work of an item only once it has begun, and while it goes on; until then the items it waits for', () => {
    const notBegun = item(1, 'open', [2, 3]);
    const begun: Item = { ...item(2, 'open'), works: [work(2, waits, ['done', 'resting', 'notYet'])] };
    const shown = shownTree({ items: [notBegun, begun, item(3, 'done')] });
    expect(shown[0].work).toBeNull();
    expect(shown[0].blocked).toBe(1);
    const w = shown[0].children[0];
    expect(w.asks).toBe(3);
    expect(w.work).toMatchObject({ number: 20, here: 'implement', usdMicros: 1_500_000 });
  });
});
