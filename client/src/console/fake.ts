import type { Waits, WorkState } from '../api';
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
  'Make the engineer flow right, fast and lean',
  'Cut rework',
  'Keep long rounds in hand',
  'Merge returns of a kind',
  'Keep late interjections',
  'Hand long rounds to a new session',
  'Record faults found after the merge',
  'Watch what builds and tests cost',
  'Install without stopping works',
  'Give health numbers past their line an owner',
  "Gather a project's own measures",
  'Watch anomalies and settle them',
  'Count slot waits in the stops',
  'Open pages fast',
  'Undo the slowdown of the new index',
  'Show the system page read-only',
  'Add prices, timezone',
  'Review each ended work',
  'Trial a candidate flow',
  'Fix guard line',
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
  // A made-up item running, waiting for the owner, or idle, by its made-up state; its work started an hour ago.
  const work = (i: FakeItem): Shown['work'] =>
    i.state === 'done' || i.state === 'new'
      ? null
      : {
          state: i.state === 'working' ? 'running' : 'asking',
          waits: i.state === 'waiting' ? { kind: 'answer', ask: 0 } : null,
          stage: 'implement',
          stages: ['research', 'design', 'implement', 'test', 'merge', 'end'],
          bar: 'mmmtt..mmm',
          from: Date.now() - 3600000,
          until: Date.now(),
          sentBack: i.id % 5 === 0 ? 2 : 0,
        };
  const shown = (i: FakeItem): Shown => ({
    id: i.id,
    title: i.title,
    standing: i.state === 'done' ? 'done' : 'open',
    asks: i.state === 'waiting' ? 1 : 0,
    unread: i.id % 9 === 0 ? 2 : 0,
    unseen: i.id % 7 === 0,
    shadow: false,
    work: work(i),
    children: i.children.map(shown),
  });
  return fakeTree(count).map(shown);
}

/// Made-up items that show everything the console draws, each titled by what it shows: a work in each of its states,
/// of each age and with questions and returns sent back; items open, done and cancelled; branches with work at their
/// ends and without; and the shadow of an item under another item that depends on it. The console draws them at
/// `/console/demo`.
export function demoShown(): Shown[] {
  const now = Date.now();
  const minutes = 60000;
  const stages = ['research', 'design', 'implement', 'test', 'merge', 'end'];
  let next = 1;
  const item = (
    title: string,
    children: Shown[] = [],
    standing: Shown['standing'] = 'open',
    unseen = false,
  ): Shown => ({
    id: next++,
    title,
    standing,
    asks: 0,
    unread: 0,
    unseen,
    shadow: false,
    work: null,
    children,
  });
  // A shadow of an item, under another item that depends on it besides its main one.
  const shadow = (of: Shown): Shown => ({
    ...of,
    asks: 0,
    unread: 0,
    unseen: false,
    shadow: true,
    work: null,
    children: [],
  });
  const work = (
    title: string,
    state: WorkState,
    o: {
      waits?: Waits;
      stage?: number;
      age?: number;
      asks?: number;
      unread?: number;
      sentBack?: number;
      unseen?: boolean;
    } = {},
  ): Shown => ({
    ...item(title, [], 'open', o.unseen ?? false),
    asks: o.asks ?? 0,
    unread: o.unread ?? 0,
    work: {
      state,
      waits: o.waits ?? null,
      stage: stages[o.stage ?? 2],
      stages,
      bar: 'mmmtt..mmmtmm.ttmm',
      from: now - (o.age ?? 12) * minutes,
      until: now,
      sentBack: o.sentBack ?? 0,
    },
  });
  const running = work('Running for 12 minutes', 'running');
  return [
    item('What a work does', [
      running,
      work('Sent back twice, with messages unread', 'running', { age: 45, sentBack: 2, stage: 3, unread: 3 }),
      work('Its next step due', 'ready', { stage: 1 }),
      work('New, and waiting for your answer', 'asking', {
        waits: { kind: 'answer', ask: 1 },
        asks: 1,
        age: 90,
        unseen: true,
      }),
      work('Waiting for your ruling for 2 days', 'asking', {
        waits: { kind: 'ruling', design_card: 1 },
        asks: 2,
        unread: 1,
        age: 2 * 24 * 60,
        stage: 1,
      }),
      work('Paused', 'paused', { age: 5 * 60 }),
      work('Halted by the engine', 'halted', { age: 3 * 60, stage: 3 }),
      work('Waiting for a free slot', 'waiting_for_slot', { stage: 4 }),
      work('Waiting for another item', 'waiting', { waits: { kind: 'items', items: [1] }, age: 30 }),
      work('Waiting for its architect', 'asking', { waits: { kind: 'architect', matter: 1 }, stage: 0 }),
      work('Stopping', 'stopping', { stage: 5 }),
    ]),
    item('What an item is', [
      item('Open, nothing started'),
      item('New to you', [], 'open', true),
      item('Done', [], 'done'),
      item('Cancelled, with an open item under it', [item('Open under a cancelled item')], 'cancelled'),
    ]),
    item('A long branch at work', [
      item('One step down', [
        item('Two steps down', [work('Running at the end of the branch', 'running', { age: 8 }), shadow(running)]),
      ]),
      item('Beside it, nothing at work'),
    ]),
    item('Nothing at work under it', [item('Open, not started'), item('Open, not started either'), shadow(running)]),
  ];
}
