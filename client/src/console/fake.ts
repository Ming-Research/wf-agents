import type { Shown } from './items';

// A made-up tree of items for trying the console's drawing at scale: shallow and wide, as a project's items are, a few
// roots and at most ten items side by side under any one. The same count always gives the same tree.

export type FakeItem = {
  id: number;
  title: string;
  /// One of a few states, so that the drawing shows the colours it will show.
  state: 'working' | 'waiting' | 'new' | 'done';
  children: FakeItem[];
};

/// Words the titles are made of, of the length real titles have.
const WORDS = [
  '把工程师流程做得对快省', '减少返工', '让长轮不再失控', '合并同类退回', '不退回迟答的插话', '长轮交接给新会话',
  '记下合并后才发现的错', '看住编译和测试的代价', '装新版本不打断工作', '让越线的健康数字有人管', '收项目自己的量',
  '监测异常并定结局', '把等名额算进停住统计', '让页面打开快', '修新索引带来的变慢', '让系统页只读显示的时段',
  'Add prices, timezone', 'Review each ended work', 'Trial a candidate flow', 'Fix guard line',
];

const STATES: FakeItem['state'][] = ['working', 'waiting', 'new', 'new', 'done', 'done', 'done'];

/// A tree of `count` items as uneven as real ones: shallow and wide, at most seven levels and ten items under any
/// one, but grown by chance where it has grown before, so that a few branches take most of the items while others
/// stay small, and leaves come at every depth.
export function fakeTree(count: number): FakeItem[] {
  let seed = 7;
  const rand = () => {
    seed = (seed * 1103515245 + 12345) % 2147483648;
    return seed / 2147483648;
  };
  type Grown = { item: FakeItem; depth: number; parent: Grown | null; under: Grown[] };
  const all: Grown[] = [];
  const make = (depth: number, parent: Grown | null): Grown => {
    const item: FakeItem = {
      id: all.length + 1,
      title: WORDS[Math.floor(rand() * WORDS.length)],
      state: STATES[Math.floor(rand() * STATES.length)],
      children: [],
    };
    const g: Grown = { item, depth, parent, under: [] };
    all.push(g);
    if (parent !== null) {
      parent.item.children.push(item);
      parent.under.push(g);
    }
    return g;
  };
  const roots = [make(0, null), make(0, null), make(0, null), make(0, null)];
  while (all.length < count) {
    // An item picked at random, or half the time the item above it: an item with many under it is picked through
    // them, so the more it has the more it gets. One with ten under it hands the new item down to one of them; one too
    // deep hands it up.
    let at: Grown | null = all[Math.floor(rand() * all.length)];
    if (rand() < 0.5 && at.parent !== null) at = at.parent;
    while (at !== null && at.depth >= 6) at = at.parent;
    while (at !== null && at.item.children.length >= 10) {
      const below = at.under.filter((g) => g.depth < 6 && g.item.children.length < 10);
      at = below.length > 0 ? below[Math.floor(rand() * below.length)] : null;
    }
    if (at !== null) make(at.depth + 1, at);
  }
  return roots.map((r) => r.item);
}

/// The made-up tree as the console shows items, for trying the drawing at scale from the browser's console.
export function fakeShown(count: number): Shown[] {
  const shown = (i: FakeItem): Shown => ({ id: i.id, title: i.title, children: i.children.map(shown) });
  return fakeTree(count).map(shown);
}
