// A cell and what the chart draws beside it, laid out together as one node of the chart's layout, the same way for
// every cell: the tags of what runs it from outside the cells stacked on its left, each with a line into it. The layout
// sees each cell and what is beside it as one node, and the arrows to and from other nodes meet the cell itself, in a
// lane along its top and bottom edges, so that no arrow runs through a tag. The lines between a cell and what is beside
// it are drawn here, straight, with bends only where a line reaches the cell beyond the part of its edge it may meet.
//
// Brought over from the chart of `old-main` (`client/src/graph/units.ts`), where a cell had more beside it (the circles
// of rows to and from another part, its rules, what it did every time, the stores it used): what this flow has none of
// is left out, the rest is as it was.

import type { Box, Point, Route, Size } from './layout';
import { type ChartEdge, type ChartNode, cellNode } from './model';

/// A cell and what is beside it, as laid out together.
export type Unit = {
  /// The cell's node.
  cell: string;
  size: Size;
  /// Where each of its nodes is in it, the cell's included.
  boxes: Map<string, Box>;
  /// The lines between them.
  links: Map<string, Route>;
  /// Where along the cell's top edge and its bottom edge, from the unit's left, the arrows from other nodes meet it.
  top: { from: number; to: number };
  bottom: { from: number; to: number };
};

/// What is drawn beside a cell: the tags of what runs it from outside.
const BESIDE = new Set<ChartNode['kind']>(['outside']);

/// The nodes drawn beside a cell, each with the cell's node: those of a kind drawn beside a cell whose every line runs
/// to that cell; a node with a line to anything else (a tag of a condition's block, into the way into the cell) is laid
/// out on its own.
export function attachments(nodes: ChartNode[], edges: ChartEdge[]): Map<string, string> {
  const ids = new Map(nodes.map((n) => [n.id, n]));
  const owner = new Map<string, string>();
  for (const n of nodes) {
    const cell = cellNode(n.cell);
    if (BESIDE.has(n.kind) && ids.get(cell)?.kind === 'cell') owner.set(n.id, cell);
  }
  let changed = true;
  while (changed) {
    changed = false;
    for (const [id, cell] of owner) {
      const stays = edges
        .filter((e) => e.source === id || e.target === id)
        .every((e) => {
          const other = e.source === id ? e.target : e.source;
          return other === cell || owner.get(other) === cell;
        });
      if (!stays) {
        owner.delete(id);
        changed = true;
      }
    }
  }
  return owner;
}

/// The least room between the cell and what stands on its left.
const LEFT = 24;
/// Between things stacked on the cell's left.
const STACK = 8;
/// The last straight piece of a line into the cell, past the ring round it.
const INTO_CELL = 20;
/// How far apart the bends of lines that turn in one band are, and where lines meet an edge side by side.
const STEP = 6;

type Pair = [number, number];

/// Lines from `from[i]` at `u0` to `to[i]` at `u1` along the axis they run on, in the same order across it: straight
/// where the two are level, else with two bends in the band between, the outer lines bending nearer the end so that
/// no two cross; `last` is the straight piece at the end. Each line as points [across, along].
function bundle(from: number[], to: number[], u0: number, u1: number, last: number): Pair[][] {
  const out: Pair[][] = from.map((a, i) => [
    [a, u0],
    [to[i], u1],
  ]);
  const sign = u1 >= u0 ? 1 : -1;
  const turn = (group: number[]) =>
    group.forEach((i, j) => {
      const u = u1 - sign * (last + j * STEP);
      out[i] = [
        [from[i], u0],
        [from[i], u],
        [to[i], u],
        [to[i], u1],
      ];
    });
  const all = from.map((_, i) => i);
  turn(all.filter((i) => to[i] - from[i] > 0.5).sort((p, q) => from[p] - from[q]));
  turn(all.filter((i) => from[i] - to[i] > 0.5).sort((p, q) => from[q] - from[p]));
  return out;
}

/// The room `bundle` needs between its ends: the least first piece, the bends of the most lines turning one way, and
/// the last piece; only the last piece when every line is straight.
function band(from: number[], to: number[], first: number, last: number): number {
  const right = from.filter((a, i) => to[i] - a > 0.5).length;
  const left = from.filter((a, i) => a - to[i] > 0.5).length;
  const turning = Math.max(right, left);
  return turning === 0 ? last : first + (turning - 1) * STEP + last;
}

/// Where lines from `at`, in order, meet an edge between `z0` and `z1`: level with where they come from where that
/// is on the edge, else as near as their order and `gap` between two allow, spread evenly when they do not fit.
export function landings(at: number[], z0: number, z1: number, gap: number): number[] {
  const k = at.length;
  if (k === 0) return [];
  if (z1 - z0 < gap * (k - 1)) {
    const step = k > 1 ? Math.max(0, z1 - z0) / (k - 1) : 0;
    const mid = (z0 + Math.max(z0, z1)) / 2;
    return at.map((_, i) => mid + (i - (k - 1) / 2) * step);
  }
  const l = at.map((x) => Math.min(z1, Math.max(z0, x)));
  for (let i = 1; i < k; i++) l[i] = Math.max(l[i], l[i - 1] + gap);
  for (let i = k - 1; i >= 0; i--) l[i] = Math.min(l[i], i === k - 1 ? z1 : l[i + 1] - gap);
  return l;
}

/// A line's points from [across, along] pairs, upright (across is x) or level (across is y), leaving out those that
/// do not bend it.
function points(ps: Pair[], upright: boolean): Point[] {
  const raw = ps.map(([a, u]) => (upright ? { x: a, y: u } : { x: u, y: a }));
  const apart = raw.filter((p, i) => i === 0 || Math.hypot(p.x - raw[i - 1].x, p.y - raw[i - 1].y) > 0.01);
  return apart.filter((p, i) => {
    if (i === 0 || i === apart.length - 1) return true;
    const a = apart[i - 1];
    const b = apart[i + 1];
    return Math.abs((p.x - a.x) * (b.y - p.y) - (p.y - a.y) * (b.x - p.x)) > 0.01;
  });
}

/// Lays a cell and what is beside it out together. `members` are the nodes beside it and `links` the lines among them
/// and the cell.
export function layUnit(cell: ChartNode, members: ChartNode[], links: ChartEdge[], size: (id: string) => Size): Unit {
  const boxes = new Map<string, Box>();
  const routes = new Map<string, Route>();
  const { width: W, height: H } = size(cell.id);
  boxes.set(cell.id, { x: 0, y: 0, width: W, height: H });

  // The tags on the left, their right edges in line and each line level into the cell where its middle is beside the
  // cell, else round the cell's middle, bending into its edge.
  const tags = members.filter((n) => BESIDE.has(n.kind));
  if (tags.length > 0) {
    const heights = tags.map((n) => size(n.id).height);
    const tall = heights.reduce((a, h) => a + h, 0) + STACK * (tags.length - 1);
    let y = (H - tall) / 2;
    const at = heights.map((h) => {
      const c = y + h / 2;
      y += h + STACK;
      return c;
    });
    const land = landings(at, 6, H - 6, STEP);
    const gap = Math.max(LEFT, band(at, land, 6, INTO_CELL - 6));
    const lines = bundle(at, land, -gap, 0, INTO_CELL - 6);
    tags.forEach((n, i) => {
      const s = size(n.id);
      boxes.set(n.id, { x: -gap - s.width, y: at[i] - s.height / 2, width: s.width, height: s.height });
      for (const e of links.filter((l) => l.source === n.id || l.target === n.id)) {
        const ps = points(lines[i], false);
        routes.set(e.id, { points: e.source === n.id ? ps : [...ps].reverse(), label: null, hops: [] });
      }
    });
  }

  // Everything moved so that the unit starts at its top left corner.
  const all = [...boxes.values()];
  const x0 = Math.min(...all.map((b) => b.x));
  const y0 = Math.min(...all.map((b) => b.y));
  const x1 = Math.max(...all.map((b) => b.x + b.width));
  const y1 = Math.max(...all.map((b) => b.y + b.height));
  const moved = (b: Box): Box => ({ ...b, x: b.x - x0, y: b.y - y0 });
  for (const [id, b] of boxes) boxes.set(id, moved(b));
  for (const [id, r] of routes) {
    routes.set(id, { points: r.points.map((p) => ({ x: p.x - x0, y: p.y - y0 })), label: null, hops: [] });
  }
  // Arrows meet the cell's top and bottom away from its corners, where a rounded cell's edge is straight.
  const inset = Math.min(H / 2, W / 4, 12);
  const lane = { from: inset - x0, to: W - inset - x0 };
  return { cell: cell.id, size: { width: x1 - x0, height: y1 - y0 }, boxes, links: routes, top: lane, bottom: lane };
}
