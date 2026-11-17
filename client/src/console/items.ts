// The items as the console shows them: a tree by their dependencies, each item under the item whose dependency on it is
// its main one, at the top the items nothing depends on; under each other item that depends on it, after the items
// standing there, a shadow of it that points to its place. Done and cancelled items are left out, unless something
// under them is still open or the owner asked to see every item. Each item carries its own state, its work while it
// goes on, whether the work has begun, and how many of the things its work waits for wait for the owner; and a work's
// age and what it has taken, as a card and the item's window say them. Pure, so that it is tested on its own.
//
// Brought over from the console of the engine on `old-main` (`client/src/console/items.ts`), reading the items as
// `works-api.ts` gives them. The interface does not say which of an item's dependents is its main one: the first of
// them by number is.

import type { CellNow, Item, Items, Now, Pauser, Summary, Wait } from '../works-api';
import { mocksShown } from '../mock-data';
import { waitsOf } from '../works/waiting';
import { itemOf } from './real';

/// What the item is to deliver: not yet, delivered, or given up.
export type ItemStanding = 'open' | 'done' | 'cancelled';

/// The work going on on an item, as its node shows it: what it does now, how each cell of its flow stands, when it was
/// added, what it has taken, and when something last happened to it.
export type ShownWork = {
  number: number;
  now: Now;
  /// Who paused it, where it is paused: only a note to a cell is answered; a round under way goes on.
  paused: Pauser | null;
  cells: CellNow[];
  /// What it waits on the owner for, as the engine's one list of it says.
  waits: Wait[];
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
  /// How many things its work waits on the owner for, as the engine's one list of it counts them.
  asks: number;
  /// It is a deliverable: an item that says what is to be delivered and is made of the items under it, with no work
  /// of its own to show. Its flow says so (`deliver`); until the engine has that flow, the page's rule above does.
  deliver: boolean;
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

/// How many things a work waits on the owner for.
export const asksOf = (w: Pick<Summary, 'waits'>): number => waitsOf(w).length;

/// Whether a work has begun: a round is under way, or a cell of it has run.
export const started = (w: Summary): boolean =>
  w.now.kind === 'round' || w.cells.some((c) => c.finished > 0 || c.open !== null);

/// Whether a work still goes on: not merged and not cancelled.
const live = (w: Summary) => w.now.kind !== 'merged' && w.now.kind !== 'cancelled';

/// Whether a card draws the work: while it goes on, once it has begun, or before that while it is paused, at fault,
/// waits or is stalled, which every work added paused is.
const drawn = (w: Summary) =>
  live(w) &&
  (started(w) ||
    w.now.kind === 'paused' ||
    w.now.kind === 'everything' ||
    w.now.kind === 'fault' ||
    w.now.kind === 'waits' ||
    w.now.kind === 'stalled');

/// The tree of the items: of those still open and what stands over them, or, with `every`, of every item.
export function shownTree(data: Items, every = false): Shown[] {
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
  // An item is kept when it or something under it is open, or when every item is shown. An item met again on its own
  // way down, which only a cycle of dependencies does, is not gone down again.
  const kept = new Map<number, boolean>();
  const keep = (id: number): boolean => {
    const known = kept.get(id);
    if (known !== undefined) return known;
    const own = every || standing(id) === 'open';
    kept.set(id, own);
    const children = (under.get(id) ?? []).map(keep);
    const k = own || children.some((c) => c);
    kept.set(id, k);
    return k;
  };
  const making = new Set<number>();
  const make = (id: number): Shown => {
    const i = byId.get(id);
    const w = i?.work;
    making.add(id);
    const mock = i === undefined ? null : itemOf(i);
    // MOCK rule, until the engine says what kind an item is: an item with open items under it whose own work has not
    // begun is a deliverable, made of what is under it. It shows no state of its own: no row of its work.
    // With the mocks off the engine's own word decides alone: the item's flow, which delivers where it has no agent's
    // cell.
    const deliverable =
      mocksShown() &&
      w !== undefined &&
      !started(w) &&
      (under.get(id) ?? []).some((c) => keep(c) && !making.has(c));
    const shown: Shown = {
      id,
      title: i?.title ?? '',
      standing: standing(id),
      asks: w === undefined ? 0 : asksOf(w),
      deliver: deliverable || w?.delivers === true,
      blocked: (i?.dependsOn ?? []).filter((d) => byId.has(d) && standing(d) !== 'done').length,
      // New to the owner exactly when something is unread: one fact, not two.
      unseen: (mock?.unread ?? 0) > 0,
      unread: mock?.unread ?? 0,
      begun: w !== undefined && started(w),
      shadow: false,
      work:
        w !== undefined && drawn(w) && !deliverable && !w.delivers
          ? {
              number: w.number,
              now: w.now,
              paused: w.paused,
              cells: w.cells,
              waits: waitsOf(w),
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
    deliver: false,
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

/// How long a work has gone on, as a short label and the tone of its age: under half an hour, under an hour, or an
/// hour and more, however much more.
export function age(ms: number): {
  label: string;
  tone: 'success' | 'attention' | 'danger';
} {
  const minutes = ms / 60000;
  if (minutes < 60)
    return {
      label: `${Math.max(1, Math.round(minutes))}m`,
      tone: minutes < 30 ? 'success' : 'attention',
    };
  const hours = minutes / 60;
  if (hours < 24) return { label: `${Math.round(hours * 10) / 10}h`, tone: 'danger' };
  return { label: `${Math.round((hours / 24) * 10) / 10}d`, tone: 'danger' };
}

/// What a work has taken, in dollars.
export const dollars = (usdMicros: number) => `$${(usdMicros / 1e6).toFixed(2)}`;
