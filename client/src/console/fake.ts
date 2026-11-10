import type { CellNow } from '../works-api';
import type { Shown } from './items';

// A made-up tree of items for trying the console's drawing at scale: shallow and wide, as a project's items are, a few
// roots and at most ten items side by side under any one. The same count always gives the same tree. From the browser's
// console, `consoleScene.show(consoleFake(2000))` draws one.
//
// Brought over from the console of the engine on `main` (`client/src/console/fake.ts`); its made-up items that showed
// every state the drawing has are `fixtures.ts` now, made as the interface gives items.

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
  const cells = ['research', 'design', 'implement', 'test', 'review', 'merge'];
  // A made-up item running, waiting for the owner, or idle, by its made-up state; its work started an hour ago.
  const work = (i: FakeItem): Shown['work'] =>
    i.state === 'done' || i.state === 'new'
      ? null
      : {
          number: i.id,
          now:
            i.state === 'working'
              ? { kind: 'round', cell: 'implement', run: 1, round: 2, since: Date.now() - 60000 }
              : {
                  kind: 'waits',
                  at: 'implement',
                  on: [{ kind: 'call', call: 1, who: 'owner', what: { kind: 'question', who: 'owner', question: '?' } }],
                },
          paused: false,
          cells: cells.map(
            (cell, k): CellNow => ({
              cell,
              state: k < 2 ? 'done' : k === 2 ? (i.state === 'working' ? 'running' : 'resting') : 'notYet',
              finished: k < 2 ? 1 : 0,
              record: null,
              open: null,
            }),
          ),
          here: 'implement',
          from: Date.now() - 3600000,
          usdMicros: 1_250_000,
          last: Date.now(),
        };
  const shown = (i: FakeItem): Shown => ({
    id: i.id,
    title: i.title,
    standing: i.state === 'done' ? 'done' : 'open',
    asks: i.state === 'waiting' ? 1 : 0,
    blocked: 0,
    shadow: false,
    work: work(i),
    children: i.children.map(shown),
  });
  return fakeTree(count).map(shown);
}
