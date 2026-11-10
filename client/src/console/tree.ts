// The console's tree, indexed for drawing: its items in the order of a walk down it, each with its parent, level, and
// place among the items of its level. Pure, so that it is tested on its own.
//
// Brought over unchanged from the console of the engine on `main` (`client/src/console/tree.ts`).

export type Tree = {
  /// In the order of a walk down the tree, each item before those under it.
  parent: Int32Array;
  depth: Int32Array;
  /// An item's place among the items of its column, in the walk's order.
  rank: Int32Array;
  children: number[][];
  /// How many items of the next column come before the first under it.
  childStart: Int32Array;
  /// The items of each column, by rank.
  columns: number[][];
};

/// The tree of `roots`, items in the order of a walk down it.
export function indexTree<T>(roots: T[], children: (t: T) => T[]): { tree: Tree; items: T[] } {
  const items: T[] = [];
  const parents: number[] = [];
  const depths: number[] = [];
  const walk = (t: T, parent: number, depth: number) => {
    const at = items.length;
    items.push(t);
    parents.push(parent);
    depths.push(depth);
    for (const c of children(t)) walk(c, at, depth + 1);
  };
  for (const r of roots) walk(r, -1, 0);
  const n = items.length;
  const tree: Tree = {
    parent: Int32Array.from(parents),
    depth: Int32Array.from(depths),
    rank: new Int32Array(n),
    children: Array.from({ length: n }, () => []),
    childStart: new Int32Array(n),
    columns: [],
  };
  for (let i = 0; i < n; i += 1) {
    const d = tree.depth[i];
    while (tree.columns.length <= d) tree.columns.push([]);
    tree.rank[i] = tree.columns[d].length;
    tree.columns[d].push(i);
    if (tree.parent[i] >= 0) tree.children[tree.parent[i]].push(i);
  }
  for (let d = 0; d < tree.columns.length; d += 1) {
    let before = 0;
    for (const i of tree.columns[d]) {
      tree.childStart[i] = before;
      before += tree.children[i].length;
    }
  }
  return { tree, items };
}
