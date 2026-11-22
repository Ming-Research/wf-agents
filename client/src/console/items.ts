// The items as the console shows them: a tree by their dependencies, each item under the item whose dependency on it is
// its main one, at the top the items nothing depends on; under each other item that depends on it, after the items
// standing there, a shadow of it that points to its place, a done or a cancelled item as any other. The tree holds the
// items that stand as one of the kinds the owner ticks, and what stands over them. Each item with items under it folds
// them away, as the owner last chose or else where neither it nor any of them is open, and counts them by how they
// stand, and apart the shadows among them, which fold with them. Each item carries its own state, its work while it
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

/// The kinds of item by how they stand, in the order the console lists them: every item is one of them.
export const STANDINGS: readonly ItemStanding[] = ['open', 'done', 'cancelled'];

/// The work going on on an item, as its node shows it: what it does now, how each cell of its flow stands, when it was
/// added, what it has taken, when something last happened to it, the level its cell runs at, and the lines it changed.
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
  /// The level its rounds run at, none where the engine gives none; and how many levels above it the line of the cell
  /// it stands at runs that cell, none counted as nought.
  level: string | null;
  raise: number;
  /// The lines it added and removed, none where it changed none (`linesOf`).
  lines: Lines | null;
};

/// The lines a work added and removed, and in how many files.
export type Lines = { added: number; removed: number; files: number };

/// How many items stand under an item, at every depth, by how they stand; and apart, how many shadows stand there,
/// each of an item that stands elsewhere and is counted where it stands.
export type Under = Record<ItemStanding, number> & { elsewhere: number };

/// Whether the items under an item are folded away, and how many stand there by how they stand.
export type Fold = { folded: boolean; under: Under };

/// The lines a work added and removed, as its summary counts them; none where it changed no line, or the engine has
/// nothing to tell.
export function linesOf(w: Pick<Summary, 'changed'>): Lines | null {
  const c = w.changed;
  return c == null || c.added + c.removed === 0 ? null : { added: c.added, removed: c.removed, files: c.files };
}

export type Shown = {
  id: number;
  title: string;
  standing: ItemStanding;
  /// Of an item whose work has ended, the lines that work added and removed; none where it changed none, or the item's
  /// work goes on, whose own `lines` say them.
  endedLines?: Lines | null;
  /// How many things its work waits on the owner for, as the engine's one list of it counts them.
  asks: number;
  /// It is a deliverable: an item that says what is to be delivered and is made of the items under it, with no work
  /// of its own to show. Its flow says so (`deliver`); until the engine has that flow, the page's rule above does.
  deliver: boolean;
  /// How many items it depends on are still open: neither merged nor cancelled, which it no longer waits for.
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
  /// Every item under it, whether or not they are folded away.
  children: Shown[];
  /// Of an item with items or shadows under it: whether they are folded away, and how many stand there as the tree
  /// holds them. None for an item with nothing under it, and for a shadow.
  fold?: Fold | null;
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

/// Each item's main dependent, which it stands under in the tree: the first by number of the items that depend on it.
function mainDependents(data: Items): Map<number, number> {
  const known = new Set(data.items.map((i) => i.number));
  const main = new Map<number, number>();
  for (const i of [...data.items].sort((a, b) => a.number - b.number))
    for (const d of i.dependsOn) if (known.has(d) && !main.has(d)) main.set(d, i.number);
  return main;
}

/// Each item's path in the tree: the numbers of the items it stands under, from its root down, and its own last, as the
/// tree places an item, under its main dependent; an item met again on its own way up, which only a cycle of
/// dependencies does, ends the path.
export function itemPaths(data: Items): Map<number, number[]> {
  const main = mainDependents(data);
  return new Map(
    data.items.map((i) => {
      const path = [i.number];
      for (let p = main.get(i.number); p !== undefined && !path.includes(p); p = main.get(p)) path.unshift(p);
      return [i.number, path];
    }),
  );
}

/// An item's path as the page writes it, its numbers joined by `-` from its root down: `53-27-23`.
export const pathWords = (path: number[]): string => path.join('-');

/// What the search of the console finds for `query`, at most `most`, each item with its path: where the query is a
/// number, `65` or `#65`, the item of that number; where it is a path from a root, `53-27-23` or `53-27`, the item it
/// names; each with the items under it, in the tree's order; then every item whose title holds the query, in any case,
/// by number. Every item counts, open, done or cancelled.
export function findItems(data: Items, query: string, most = 8): { item: Item; path: number[] }[] {
  const q = query.trim();
  if (q === '') return [];
  const paths = itemPaths(data);
  const found: { item: Item; path: number[] }[] = [];
  const taken = new Set<number>();
  const take = (item: Item) => {
    if (taken.has(item.number)) return;
    taken.add(item.number);
    found.push({ item, path: paths.get(item.number) ?? [item.number] });
  };
  const byNumber = [...data.items].sort((a, b) => a.number - b.number);
  const asked = /^#?\d+(-\d+)*$/.test(q) ? q.replace(/^#/, '').split('-').map(Number) : null;
  if (asked !== null) {
    const named = asked.length === 1 ? byNumber.find((i) => i.number === asked[0]) : undefined;
    if (named !== undefined) take(named);
    const starts = (path: number[]) => asked.length <= path.length && asked.every((n, k) => path[k] === n);
    const order = (a: number[], b: number[]) => {
      for (let k = 0; k < Math.min(a.length, b.length); k += 1) if (a[k] !== b[k]) return a[k] - b[k];
      return a.length - b.length;
    };
    const below = byNumber
      .map((item) => ({ item, path: paths.get(item.number) ?? [item.number] }))
      .filter(({ path }) => starts(path))
      .sort((a, b) => order(a.path, b.path));
    for (const { item } of below) take(item);
  }
  const words = q.toLocaleLowerCase();
  for (const item of byNumber) if (item.title.toLocaleLowerCase().includes(words)) take(item);
  return found.slice(0, most);
}

/// The tree of the items that stand as one of `shown` stands, and what stands over them; each item the owner folded or
/// unfolded in `folds` as he last chose, the others folded where neither they nor anything under them is open.
export function shownTree(
  data: Items,
  shown: ReadonlySet<ItemStanding> = new Set(['open']),
  folds: ReadonlyMap<number, boolean> = new Map(),
): Shown[] {
  const byId = new Map(data.items.map((i) => [i.number, i]));
  const standing = (id: number): ItemStanding => standingOf(byId.get(id));
  const main = mainDependents(data);
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
  // An item is kept when it or something under it stands as one of those shown. An item met again on its own way down,
  // which only a cycle of dependencies does, is not gone down again.
  const kept = new Map<number, boolean>();
  const keep = (id: number): boolean => {
    const known = kept.get(id);
    if (known !== undefined) return known;
    const own = shown.has(standing(id));
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
    const children = [...(under.get(id) ?? [])]
      .filter((c) => keep(c) && !making.has(c))
      .sort((a, b) => a - b)
      .map(make);
    const shadows = [...(besides.get(id) ?? [])]
      .filter(keep)
      .sort((a, b) => a - b)
      .map(shadow);
    const shown: Shown = {
      id,
      title: i?.title ?? '',
      standing: standing(id),
      asks: w === undefined ? 0 : asksOf(w),
      deliver: deliverable || w?.delivers === true,
      // A cancelled item it depends on holds it back no more: only what is still open does.
      blocked: (i?.dependsOn ?? []).filter((d) => byId.has(d) && standing(d) === 'open').length,
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
              level: w.level ?? null,
              raise: w.raise ?? 0,
              lines: linesOf(w),
            }
          : null,
      endedLines: w !== undefined && !drawn(w) ? linesOf(w) : null,
      children: [...children, ...shadows],
      fold:
        children.length + shadows.length === 0 ? null : foldOf(standing(id), children, shadows.length, folds.get(id)),
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

/// The fold of an item that stands as `own` stands, with `children` standing under it and `shadows` shadows beside
/// them: how many items stand under it by how they stand, at every depth, and how many shadows; folded as the owner
/// last chose (`chosen`), or else where neither it nor any item under it is open, so that what still has open work in
/// it shows how far it has come. A shadow's item, which stands elsewhere, does not count for that.
function foldOf(own: ItemStanding, children: Shown[], shadows: number, chosen: boolean | undefined): Fold {
  const under: Under = { open: 0, done: 0, cancelled: 0, elsewhere: shadows };
  for (const c of children) {
    under[c.standing] += 1;
    for (const s of STANDINGS) under[s] += c.fold?.under[s] ?? 0;
    under.elsewhere += c.fold?.under.elsewhere ?? 0;
  }
  return { folded: chosen ?? (own !== 'open' && under.open === 0), under };
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
