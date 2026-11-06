// The items as the console shows them: a tree by their main dependencies, each item under the item whose dependency on
// it is its main one, at the top the items nothing depends on. Cancelled items are left out, and so is every tree whose
// items are all done: it is history, not part of anything going on; a done item in a tree with something still to do
// stays, as part of how far that tree has come. Pure, so that it is tested on its own.

import type { Items } from '../api';

/// An item as the console shows it: its number and title, and the items under it. How its state is shown is still to
/// be settled, and the console shows none yet.
export type Shown = {
  id: number;
  title: string;
  children: Shown[];
};

export function shownTree(data: Items): Shown[] {
  const byId = new Map(data.items.map((i) => [i.id, i]));
  const kept = (id: number) => {
    const i = byId.get(id);
    return i !== undefined && i.state !== 'cancelled';
  };
  const under = new Map<number, number[]>();
  const above = new Set<number>();
  for (const l of data.links) {
    if (!l.main || !kept(l.item) || !kept(l.depends_on)) continue;
    under.set(l.item, [...(under.get(l.item) ?? []), l.depends_on]);
    above.add(l.depends_on);
  }
  const make = (id: number): Shown => {
    const i = byId.get(id);
    return {
      id,
      title: i?.title ?? '',
      children: [...(under.get(id) ?? [])].sort((a, b) => a - b).map(make),
    };
  };
  const open = (t: Shown): boolean => byId.get(t.id)?.state !== 'done' || t.children.some(open);
  return data.items
    .filter((i) => kept(i.id) && !above.has(i.id))
    .sort((a, b) => a.id - b.id)
    .map((i) => make(i.id))
    .filter(open);
}
