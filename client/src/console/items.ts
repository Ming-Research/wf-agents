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

/// What the item is to deliver: not yet, delivered, or given up.
export type ItemStanding = 'open' | 'done' | 'cancelled';

/// The work going on on an item, as its node shows it: what it does now, how each cell of its flow stands, the cell it
/// is at, when it was added, what it has taken, and when something last happened to it.
export type ShownWork = {
  number: number;
  now: Now;
  /// No round of it begins; a round under way goes on.
  paused: boolean;
  cells: CellNow[];
  /// The cell it is at: where a round is under way, where it waits, or what it does next; none where nothing says.
  here: string | null;
  from: number;
  usdMicros: number;
  last: number | null;
};

export type Shown = {
  id: number;
  title: string;
  standing: ItemStanding;
  /// The things its work waits for that the owner gives: answers, preconditions only they can make hold, a run let on.
  asks: number;
  /// How many items it depends on are not merged yet.
  blocked: number;
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
  return w.who;
}

/// How many of the things a work waits for the owner gives.
export const asksOf = (now: Now): number =>
  now.kind === 'waits' ? now.on.filter((w) => whoGives(w) === 'owner').length : 0;

/// The cell a work is at: where its round is under way, where it waits, or what it does next; else, as the walk would
/// find it, the first cell in the flow's order that is running, at rest, to run again or not yet run.
export function hereOf(w: Pick<Summary, 'now' | 'cells'>): string | null {
  const now = w.now;
  if (now.kind === 'round' || now.kind === 'next') return now.cell;
  if (now.kind === 'waits') return now.at;
  if (now.kind === 'merged' || now.kind === 'cancelled') return null;
  return w.cells.find((c) => c.state !== 'done' && c.state !== 'skipped')?.cell ?? null;
}

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
    const shown: Shown = {
      id,
      title: i?.title ?? '',
      standing: standing(id),
      asks: w === undefined ? 0 : asksOf(w.now),
      blocked: (i?.dependsOn ?? []).filter((d) => byId.has(d) && standing(d) !== 'done').length,
      begun: w !== undefined && started(w),
      shadow: false,
      work:
        w !== undefined && drawn(w)
          ? {
              number: w.number,
              now: w.now,
              paused: w.paused,
              cells: w.cells,
              here: hereOf(w),
              from: w.added,
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
    blocked: 0,
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
