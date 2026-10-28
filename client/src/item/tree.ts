// The items of a project as a tree of what depends on what: at the top the items nothing waits for, and under each,
// indented, the items it depends on, each with the kind of the dependency. An item several items depend on is shown in
// full under the first and as a line that leads to it under the others. Pure, so that it is tested on its own.

import type { ItemLink, ItemState, ItemSummary } from '../api';

export type TreeRow = {
  item: ItemSummary;
  /// Shown in full here, with what it depends on under it; otherwise a line that leads to where it is shown in full.
  full: boolean;
  /// The kind of the dependency by which the item above depends on it, blocking or parallel; none at the top.
  kind: string | null;
  children: TreeRow[];
};

/// The order states are listed in: what moves first, what is settled last.
export const STATES: ItemState[] = ['working', 'queued', 'new', 'stopped', 'done', 'cancelled'];

/// What moves first comes first: by state, then by place in the queue, then by number.
export function byOrder(a: ItemSummary, b: ItemSummary): number {
  return STATES.indexOf(a.state) - STATES.indexOf(b.state) || (a.queued ?? 0) - (b.queued ?? 0) || a.id - b.id;
}

/// The tree of `items`, by the dependencies among them; a link to an item not among them is left out.
export function tree(items: ItemSummary[], links: ItemLink[]): TreeRow[] {
  const byId = new Map(items.map((i) => [i.id, i]));
  const needs = new Map<number, { dep: ItemSummary; kind: string }[]>();
  const awaited = new Set<number>();
  for (const l of links) {
    const item = byId.get(l.item);
    const dep = byId.get(l.depends_on);
    if (item === undefined || dep === undefined) continue;
    needs.set(item.id, [...(needs.get(item.id) ?? []), { dep, kind: l.kind }]);
    awaited.add(dep.id);
  }
  const shown = new Set<number>();
  const row = (item: ItemSummary, path: Set<number>, kind: string | null): TreeRow => {
    if (shown.has(item.id)) return { item, full: false, kind, children: [] };
    shown.add(item.id);
    const under = new Set(path).add(item.id);
    const children = [...(needs.get(item.id) ?? [])]
      .sort((a, b) => byOrder(a.dep, b.dep))
      // The dependencies form no cycle; one that did would end here rather than go round.
      .filter((d) => !under.has(d.dep.id))
      .map((d) => row(d.dep, under, d.kind));
    return { item, full: true, kind, children };
  };
  const roots = items.filter((i) => !awaited.has(i.id)).sort(byOrder);
  const out = roots.map((r) => row(r, new Set(), null));
  // Items left over can only be on a cycle, which the database refuses; they are still listed.
  for (const i of [...items].sort(byOrder)) if (!shown.has(i.id)) out.push(row(i, new Set(), null));
  return out;
}

/// How many of the items an item depends on are done, of how many.
export function depsDone(id: number, items: ItemSummary[], links: ItemLink[]): { done: number; of: number } {
  const state = new Map(items.map((i) => [i.id, i.state]));
  const deps = links.filter((l) => l.item === id);
  return { done: deps.filter((l) => state.get(l.depends_on) === 'done').length, of: deps.length };
}

/// How far a stage is along the stages: from a little past nothing at the first to all at the last.
export function stageShare(stage: string | null, stages: string[]): number | null {
  if (stage === null) return null;
  const at = stages.indexOf(stage);
  if (at < 0 || stages.length === 0) return null;
  return (at + 1) / stages.length;
}
