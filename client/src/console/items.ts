// The items as the console shows them: a tree by their main dependencies, each item under the item whose dependency on
// it is its main one, at the top the items nothing depends on. Done and cancelled items are left out, unless something
// under them is still open. Each item carries its own state, the work going on on it, if any, and how many of its
// questions wait for the owner. Pure, so that it is tested on its own.

import type { Items, Waits, WorkState, WorkSummary } from '../api';

/// What the item is to deliver: not yet, delivered, or given up.
export type ItemStanding = 'open' | 'done' | 'cancelled';

/// The work going on on an item, as its node shows it: what it does now, the stage it is at among its graph's stages,
/// where its time went, when it started and its span's end, and how many of its returns were sent back.
export type ShownWork = {
  state: WorkState;
  waits: Waits | null;
  stage: string | null;
  stages: string[];
  bar: string;
  from: number;
  until: number;
  sentBack: number;
};

export type Shown = {
  id: number;
  title: string;
  standing: ItemStanding;
  /// Its questions and decision cards waiting for the owner, which hold its work until answered.
  asks: number;
  work: ShownWork | null;
  children: Shown[];
};

/// Whether a work still goes on: not done and not ended.
const live = (w: WorkSummary) => w.state !== 'done' && w.state !== 'ended';

export function shownTree(data: Items, works: WorkSummary[] = []): Shown[] {
  const byId = new Map(data.items.map((i) => [i.id, i]));
  const workById = new Map(works.map((w) => [w.id, w]));
  const standing = (id: number): ItemStanding => {
    const s = byId.get(id)?.state;
    return s === 'cancelled' ? 'cancelled' : s === 'done' ? 'done' : 'open';
  };
  const under = new Map<number, number[]>();
  const above = new Set<number>();
  for (const l of data.links) {
    if (!l.main || !byId.has(l.item) || !byId.has(l.depends_on)) continue;
    under.set(l.item, [...(under.get(l.item) ?? []), l.depends_on]);
    above.add(l.depends_on);
  }
  // An item is kept when it or something under it is open.
  const kept = new Map<number, boolean>();
  const keep = (id: number): boolean => {
    const known = kept.get(id);
    if (known !== undefined) return known;
    const children = (under.get(id) ?? []).map(keep);
    const k = standing(id) === 'open' || children.some((c) => c);
    kept.set(id, k);
    return k;
  };
  const make = (id: number): Shown => {
    const i = byId.get(id);
    const w = i?.work !== null && i?.work !== undefined ? workById.get(i.work) : undefined;
    return {
      id,
      title: i?.title ?? '',
      standing: standing(id),
      asks: (i?.open_asks ?? 0) + (i?.open_design_cards ?? 0),
      work:
        w !== undefined && live(w)
          ? {
              state: w.state,
              waits: w.waits,
              stage: i?.stage ?? null,
              stages: i?.stages ?? [],
              bar: w.time.bar,
              from: w.time.from,
              until: w.time.until,
              sentBack: w.sent_back,
            }
          : null,
      children: [...(under.get(id) ?? [])]
        .filter(keep)
        .sort((a, b) => a - b)
        .map(make),
    };
  };
  return data.items
    .filter((i) => !above.has(i.id) && keep(i.id))
    .sort((a, b) => a.id - b.id)
    .map((i) => make(i.id));
}
