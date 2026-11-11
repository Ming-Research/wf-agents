// The items as the console shows them: a tree by their dependencies, each item under the item whose dependency on it is
// its main one, at the top the items nothing depends on; under each other item that depends on it, after the items
// standing there, a shadow of it that points to its place. Done and cancelled items are left out, unless something
// under them is still open. Each item carries its own state, its work while it goes on, whether the work has begun, and
// how many of the things its work waits for wait for the owner. Pure, so that it is tested on its own.
//
// Brought over from the console of the engine on `main` (`client/src/console/items.ts`), reading the items as
// `works-api.ts` gives them. The interface does not say which of an item's dependents is its main one: the first of
// them by number is.

import type { CellNow, Item, Items, Now, Summary, Waited } from '../works-api';
import { itemMock } from './mock';

/// What the item is to deliver: not yet, delivered, or given up.
export type ItemStanding = 'open' | 'done' | 'cancelled';

/// The work going on on an item, as its node shows it: what it does now, how each cell of its flow stands, when it was
/// added, what it has taken, and when something last happened to it.
export type ShownWork = {
  number: number;
  now: Now;
  /// No round of it begins; a round under way goes on.
  paused: boolean;
  cells: CellNow[];
  /// When its first round began, from which its age is counted; when it was added, before it has a round or from an
  /// engine that does not say.
  from: number;
  /// Where its time went, in equal cells (`Summary.bar`); empty where the engine does not say.
  bar: string;
  /// How many of its returns were sent back.
  sentBack: number;
  usdMicros: number;
  last: number | null;
};

export type Shown = {
  id: number;
  title: string;
  standing: ItemStanding;
  /// The things its work is stopped for that the owner gives: answers, preconditions only they can make hold, a run
  /// let on.
  asks: number;
  /// The questions that wait for the owner's answer and do not stop its work yet (mock: `mock.ts`).
  questions: number;
  /// How many items it depends on are not merged yet.
  blocked: number;
  /// New to the owner, or changed since they last read it (mock: `mock.ts`).
  unseen: boolean;
  /// Messages from others the owner has not read (mock: `mock.ts`).
  unread: number;
  /// Whether its work has begun: until it has, the card says how many items it waits for.
  begun: boolean;
  /// A shadow of the item under an item that depends on it by a dependency that is not its main one: it points to the
  /// item's place, and carries none of its work, its questions or the items under it.
  shadow: boolean;
  /// Its work while it goes on and says something: once it has begun, or while it is paused, at fault or waits.
  work: ShownWork | null;
  children: Shown[];
};

/// What an item's work makes of it.
export function standingOf(i: Item | undefined): ItemStanding {
  const now = i?.work.now;
  return now?.kind === 'merged' ? 'done' : now?.kind === 'cancelled' ? 'cancelled' : 'open';
}

/// Who gives what a work waits for: the person a call or a precondition names, the owner for a run let on at its
/// limit, none where the engine gives it.
export function whoGives(w: Waited): 'owner' | 'architect' | null {
  if (w.kind === 'limit') return 'owner';
  // The wait for the items it depends on is nobody's to give: the engine names the owner for every test that does not
  // hold, though the owner cannot make this one hold. Until the engine says who can, the test is known by its name.
  if (w.kind === 'precondition' && w.test === 'may_start') return null;
  return w.who;
}

/// How many of the things a work waits for the owner gives.
export const asksOf = (now: Now): number =>
  now.kind === 'waits' ? now.on.filter((w) => whoGives(w) === 'owner').length : 0;

/// Whether a work has begun: a round is under way, or a cell of it has run.
export const started = (w: Summary): boolean =>
  w.now.kind === 'round' || w.cells.some((c) => c.finished > 0 || c.open !== null);

/// Whether a work still goes on: not merged and not cancelled.
const live = (w: Summary) => w.now.kind !== 'merged' && w.now.kind !== 'cancelled';

/// Whether a card draws the work: while it goes on, once it has begun, or before that while it is paused, at fault or
/// waits, which every work added paused is.
const drawn = (w: Summary) =>
  live(w) && (started(w) || w.now.kind === 'paused' || w.now.kind === 'fault' || w.now.kind === 'waits');

export function shownTree(data: Items): Shown[] {
  const byId = new Map(data.items.map((i) => [i.number, i]));
  const standing = (id: number): ItemStanding => standingOf(byId.get(id));
  // Each item's main dependent: the first by number of the items that depend on it.
  const main = new Map<number, number>();
  for (const i of [...data.items].sort((a, b) => a.number - b.number))
    for (const d of i.dependsOn) if (byId.has(d) && !main.has(d)) main.set(d, i.number);
  const under = new Map<number, number[]>();
  const besides = new Map<number, number[]>();
  const above = new Set<number>();
  for (const i of data.items)
    for (const d of new Set(i.dependsOn)) {
      if (!byId.has(d) || d === i.number) continue;
      if (main.get(d) !== i.number) {
        besides.set(i.number, [...(besides.get(i.number) ?? []), d]);
        continue;
      }
      under.set(i.number, [...(under.get(i.number) ?? []), d]);
      above.add(d);
    }
  // An item is kept when it or something under it is open. An item met again on its own way down, which only a cycle of
  // dependencies does, is not gone down again.
  const kept = new Map<number, boolean>();
  const keep = (id: number): boolean => {
    const known = kept.get(id);
    if (known !== undefined) return known;
    kept.set(id, standing(id) === 'open');
    const children = (under.get(id) ?? []).map(keep);
    const k = standing(id) === 'open' || children.some((c) => c);
    kept.set(id, k);
    return k;
  };
  const making = new Set<number>();
  const make = (id: number): Shown => {
    const i = byId.get(id);
    const w = i?.work;
    making.add(id);
    const mock = i === undefined ? null : itemMock(i);
    // MOCK rule, until the engine says what kind an item is: an item with open items under it whose own work has not
    // begun is a deliverable, made of what is under it. It shows no state of its own: no row of its work.
    const deliverable =
      w !== undefined && !started(w) && (under.get(id) ?? []).some((c) => keep(c) && !making.has(c));
    const shown: Shown = {
      id,
      title: i?.title ?? '',
      standing: standing(id),
      asks: w === undefined ? 0 : asksOf(w.now),
      questions: mock?.openQuestions ?? 0,
      blocked: (i?.dependsOn ?? []).filter((d) => byId.has(d) && standing(d) !== 'done').length,
      // New to the owner exactly when something is unread: one fact, not two.
      unseen: (mock?.unread ?? 0) > 0,
      unread: mock?.unread ?? 0,
      begun: w !== undefined && started(w),
      shadow: false,
      work:
        w !== undefined && drawn(w) && !deliverable
          ? {
              number: w.number,
              now: w.now,
              paused: w.paused,
              cells: w.cells,
              from: w.from ?? w.added,
              bar: w.bar ?? '',
              sentBack: w.sentBack ?? 0,
              usdMicros: w.spent.usdMicros,
              last: w.last,
            }
          : null,
      children: [
        ...[...(under.get(id) ?? [])]
          .filter((c) => keep(c) && !making.has(c))
          .sort((a, b) => a - b)
          .map(make),
        ...[...(besides.get(id) ?? [])]
          .filter(keep)
          .sort((a, b) => a - b)
          .map(shadow),
      ],
    };
    making.delete(id);
    return shown;
  };
  const shadow = (id: number): Shown => ({
    id,
    title: byId.get(id)?.title ?? '',
    standing: standing(id),
    asks: 0,
    questions: 0,
    blocked: 0,
    unseen: false,
    unread: 0,
    begun: false,
    shadow: true,
    work: null,
    children: [],
  });
  return data.items
    .filter((i) => !above.has(i.number) && keep(i.number))
    .sort((a, b) => a.number - b.number)
    .map((i) => make(i.number));
}
