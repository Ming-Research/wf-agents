// The items as the console shows them: a tree by their dependencies, each item under the item whose dependency on it is
// its main one, at the top the items nothing depends on; under each other item that depends on it, after the items
// standing there, a shadow of it that points to its place, a done or a cancelled item as any other. The tree holds the
// items that stand as one of the kinds the owner ticks, and what stands over them. Each item with items under it folds
// them away, as the owner last chose or else where neither it nor any of them is open, and counts them by how they
// stand, and apart the shadows among them, which fold with them. Each item carries its own state, its work while it
// goes on, whether the work has begun, and how many of the things its work waits for wait for the owner; and a work's
// age and what it has taken, as a card and the item's window say them. Pure, so that it is tested on its own.
//
// It reads the items as `works-api.ts` gives them. The interface does not say which of an item's dependents is its
// main one: the first of them by number is.

import type { CellNow, Fresh, Held, Item, Items, Now, Pauser, Summary, Wait } from '../works-api';
import { mocksShown } from '../mock-data';
import { aheadOf, waitsOf } from '../works/waiting';
import { stateOf } from '../works/words';
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
  /// When its first round began, from which its age is counted; none before it has run a round, when the card has no
  /// row of it.
  from: number | null;
  /// Where its time went, in equal cells (`Summary.bar`); empty where the engine does not say.
  bar: string;
  /// How many of its returns were sent back.
  sentBack: number;
  usdMicros: number;
  last: number | null;
  /// The level its rounds run at, none where the engine gives none; how many levels above it the cell it stands at
  /// really runs, none counted as nought; the level that cell runs at; and what held it below what its line asks.
  level: string | null;
  raise: number;
  cellLevel: string | null;
  held: Held | null;
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
  /// How many questions put to the owner its work has standing that hold it at nothing yet, which the owner may answer
  /// ahead, as the engine counts them (`Summary.ahead`): none that `asks` counts.
  ahead: number;
  /// It is a deliverable: an item that says what is to be delivered and is made of the items under it, with no work
  /// of its own to show. Its flow says so (`deliver`); until the engine has that flow, the page's rule above does.
  deliver: boolean;
  /// New to the owner, or changed since they last read it, as the engine's one definition of what the owner has not
  /// read says (`Summary.fresh`; made up with the mocks, `mock.ts`).
  fresh: Fresh;
  /// How many replies of the cells the owner has not read (`Summary.repliesUnread`; made up with the mocks).
  repliesUnread: number;
  /// Whether its work has begun, which is whether it has run a round ([`started`]).
  begun: boolean;
  /// A shadow of the item under an item that depends on it by a dependency that is not its main one: it points to the
  /// item's place, and carries none of its work, its questions or the items under it.
  shadow: boolean;
  /// Its work while it goes on, once it has run a round, or before that while it is halted or waits for the owner, the
  /// card then of one row ([`drawn`]); else none, the card an open item's, of one row.
  work: ShownWork | null;
  /// Every item under it, whether or not they are folded away.
  children: Shown[];
  /// Of an item with items or shadows under it: whether they are folded away, and how many stand there as the tree
  /// holds them. None for an item with nothing under it, and for a shadow.
  fold?: Fold | null;
};

/// Whether the owner has something to read of an item: it is new or has changed, or a reply is not read. The one test of
/// what the map marks of an item, and of a folded item for what is under it; the card says only how many replies are not
/// read.
export const somethingUnread = (s: Pick<Shown, 'fresh' | 'repliesUnread'>): boolean =>
  s.fresh !== 'none' || s.repliesUnread > 0;

/// What an item's work makes of it.
export function standingOf(i: Item | undefined): ItemStanding {
  const now = i?.work.now;
  return now?.kind === 'merged' ? 'done' : now?.kind === 'cancelled' ? 'cancelled' : 'open';
}

/// How many things a work waits on the owner for.
export const asksOf = (w: Pick<Summary, 'waits'>): number => waitsOf(w).length;

/// Whether a work has begun: it has run a round, so that the engine says when its first round began. A cell can stand
/// done with no round of it, as the first cell of an item brought over from elsewhere does; that is no beginning.
export const started = (w: Summary): boolean => w.from != null;

/// Whether a work still goes on: not merged and not cancelled.
const live = (w: Summary) => w.now.kind !== 'merged' && w.now.kind !== 'cancelled';

/// Whether a card draws the work, in the colours of how it stands: while it goes on, once it has begun, and before
/// that while it is halted or waits for the owner, which a card says in red whether or not the work has run. Its second
/// row it draws only once the work has run a round (`ShownWork.from`); one paused or waiting for other items before
/// its first round draws no work, as an open item with none.
const drawn = (w: Summary) => {
  if (!live(w)) return false;
  const { tone } = stateOf(w);
  return started(w) || tone === 'owner' || tone === 'failed';
};

/// The name the console gives the project of an item added before items had projects.
export const NO_PROJECT = 'No project';

/// The project an item is of, as the console names it.
export const projectOf = (i: Item): string => i.project ?? NO_PROJECT;

/// The projects the installation names (`named`) and those the items are of, in the order the owner last put them in
/// (`kept`), and those not in it after them, by name, the items of no project last.
export function projectsIn(data: Items, kept: readonly string[], named: readonly string[] = []): string[] {
  const all = new Set([...named, ...data.items.map(projectOf)]);
  const known = kept.filter((p, k) => all.has(p) && kept.indexOf(p) === k);
  const rest = [...all]
    .filter((p) => !known.includes(p))
    .sort((a, b) => (a === NO_PROJECT ? 1 : 0) - (b === NO_PROJECT ? 1 : 0) || a.localeCompare(b));
  return [...known, ...rest];
}

/// Each item's main dependent, which it stands under in the tree: the first by number of the items of its own project
/// that depend on it and are still open, or, where every one of them has merged or been cancelled, the first by number
/// of those: an open item stands under the open item that waits for it, never under one that ended long ago. An item
/// that only items of other projects depend on stands at the top of its own project's tree, and a shadow of it stands
/// under each of them.
function mainDependents(data: Items): Map<number, number> {
  const byId = new Map(data.items.map((i) => [i.number, i]));
  const main = new Map<number, number>();
  const byNumber = [...data.items].sort((a, b) => a.number - b.number);
  for (const open of [true, false])
    for (const i of byNumber) {
      if ((standingOf(i) === 'open') !== open) continue;
      for (const d of i.dependsOn) {
        const of = byId.get(d);
        if (of !== undefined && !main.has(d) && projectOf(of) === projectOf(i)) main.set(d, i.number);
      }
    }
  return main;
}

/// What the search of the console finds for `query`, at most `most`, as the query stands while it is typed a digit at a
/// time: where it is a number, `65` or `#65`, the items whose number begins with it, the item of that very number first;
/// then every item whose title holds the query, in any case, by number. Every item counts, open, done or cancelled.
export function findItems(data: Items, query: string, most = 8): Item[] {
  const q = query.trim();
  if (q === '') return [];
  const found: Item[] = [];
  const taken = new Set<number>();
  const take = (item: Item) => {
    if (taken.has(item.number)) return;
    taken.add(item.number);
    found.push(item);
  };
  const byNumber = [...data.items].sort((a, b) => a.number - b.number);
  const typed = /^#?\d+$/.test(q) ? q.replace(/^#/, '') : null;
  if (typed !== null) {
    const begins = byNumber.filter((i) => String(i.number).startsWith(typed));
    for (const item of [...begins.filter((i) => String(i.number) === typed), ...begins]) take(item);
  }
  const words = q.toLocaleLowerCase();
  for (const item of byNumber) if (item.title.toLocaleLowerCase().includes(words)) take(item);
  return found.slice(0, most);
}

/// The tree of the items that stand as one of `shown` stands, and what stands over them; each item the owner folded or
/// unfolded in `folds` as he last chose, the others folded where neither they nor anything under them is open. Given
/// `project`, the tree of that project's items alone.
export function shownTree(
  data: Items,
  shown: ReadonlySet<ItemStanding> = new Set(['open']),
  folds: ReadonlyMap<number, boolean> = new Map(),
  project?: string,
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
      // Only a work that goes on has questions to answer.
      ahead: w === undefined ? 0 : aheadOf(w),
      deliver: deliverable || w?.delivers === true,
      fresh: mock?.fresh ?? 'none',
      repliesUnread: mock?.repliesUnread ?? 0,
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
              from: w.from ?? null,
              bar: w.bar ?? '',
              sentBack: w.sentBack ?? 0,
              usdMicros: w.spent.usdMicros,
              last: w.last,
              level: w.level ?? null,
              raise: w.raise ?? 0,
              cellLevel: w.cellLevel ?? null,
              held: w.held ?? null,
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
    ahead: 0,
    deliver: false,
    fresh: 'none',
    repliesUnread: 0,
    begun: false,
    shadow: true,
    work: null,
    children: [],
  });
  return data.items
    .filter((i) => !above.has(i.number) && keep(i.number) && (project === undefined || projectOf(i) === project))
    .sort((a, b) => a.number - b.number)
    .map((i) => make(i.number));
}

/// The items of the tree `roots` in its order, each before the items under it, with how many stops each has: by default
/// each thing its red and its orange counts count, which n and N walk on the console; with `count` another count, as the
/// replies not read, which m and M walk. A shadow stands for an item placed elsewhere, and is left out.
export function walkOrder(
  roots: Shown[],
  count: (s: Shown) => number = (s) => s.asks + s.ahead,
): { id: number; stops: number }[] {
  const order: { id: number; stops: number }[] = [];
  const go = (s: Shown) => {
    if (s.shadow) return;
    order.push({ id: s.id, stops: count(s) });
    s.children.forEach(go);
  };
  roots.forEach(go);
  return order;
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
