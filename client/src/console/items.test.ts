import { describe, expect, it } from 'vitest';

import type { ItemLink, Items, ItemState, ItemSummary } from '../api';
import { shownTree, type Shown } from './items';
import { indexTree } from './tree';

function item(id: number, state: ItemState): ItemSummary {
  return {
    id,
    project: 'p',
    title: `t${id}`,
    deliver: 'change',
    level: '1',
    route_taken: null,
    cancelled: state === 'cancelled' ? 'not wanted' : null,
    state,
    work: null,
    at: null,
    stage: null,
    stages: [],
    imported: null,
    open_asks: 0,
    open_design_cards: 0,
    pr: null,
    blocked_by: [],
  } as unknown as ItemSummary;
}

const link = (from: number, to: number, main: boolean): ItemLink => ({
  item: from,
  depends_on: to,
  needs: '',
  kind: 'blocking',
  main,
});

/// A tree as lines: each item's number, indented by its depth, a shadow's marked.
const lines = (roots: Shown[], depth = 0): string[] =>
  roots.flatMap((r) => [`${'  '.repeat(depth)}${r.id}${r.shadow ? ' shadow' : ''}`, ...lines(r.children, depth + 1)]);

describe('the tree the console shows', () => {
  // 1 needs 2 and 3, and 4 needs 3 too, its main dependency 1's; 3 needs 5, which is cancelled; 6 and 7, both done,
  // are a tree of their own; 8 is done and stands under 4, and 9, still open, under 8; 10 is done and stands under 2,
  // and 4 needs it too.
  const data: Items = {
    items: [
      item(1, 'open'),
      item(2, 'open'),
      item(3, 'open'),
      item(4, 'open'),
      item(5, 'cancelled'),
      item(6, 'done'),
      item(7, 'done'),
      item(8, 'done'),
      item(9, 'open'),
      item(10, 'done'),
    ],
    links: [
      link(1, 2, true),
      link(1, 3, true),
      link(4, 3, false),
      link(3, 5, true),
      link(6, 7, true),
      link(4, 8, true),
      link(8, 9, true),
      link(2, 10, true),
      link(4, 10, false),
    ],
  };

  it('puts each item under its main dependency and a shadow of it under each other, after the items there, and leaves out what is done or cancelled with nothing open under it', () => {
    expect(lines(shownTree(data))).toEqual(['1', '  2', '  3', '4', '  8', '    9', '  3 shadow']);
  });

  it('indexes the tree by level, each item with its place among its level and the items under it', () => {
    const { tree, items } = indexTree(shownTree(data), (s) => s.children);
    expect(items.map((s) => s.id)).toEqual([1, 2, 3, 4, 8, 9, 3]);
    expect(tree.columns.map((c) => c.map((i) => items[i].id))).toEqual([[1, 4], [2, 3, 8, 3], [9]]);
    expect(Array.from(tree.childStart)).toEqual([0, 0, 0, 2, 0, 0, 1]);
  });
});
