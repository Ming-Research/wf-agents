// Lays a flow's chart out with elkjs: each part of the chart on its own, top to bottom, its stage boxes as boxes that
// hold their cells, arrows routed at right angles around the boxes with room for their labels, each with a last
// straight piece long enough for its head; then the main part on top and the other parts in a row below it. Each
// arrow's route also says where it crosses an arrow that runs the other way, so that it is drawn with a small arch
// there.

import ELK from 'elkjs/lib/elk.bundled.js';
import type { ElkExtendedEdge, ElkNode } from 'elkjs/lib/elk-api';

import { type Chart, MAIN } from './flowModel';

const elk = new ELK();

export type Point = { x: number; y: number };
export type Box = { x: number; y: number; width: number; height: number };
export type Size = { width: number; height: number };

/// An arrow as laid out: the points its route passes through, its label's box, and for each straight piece of the
/// route, the places along it where it arches over another arrow.
export type Route = { points: Point[]; label: Box | null; hops: number[][] };

export type Laid = {
  nodes: Map<string, Box>;
  groups: Map<string, Box>;
  edges: Map<string, Route>;
  /// Each part's box, the main part's first.
  parts: Map<string, Box>;
  bounds: Box;
};

/// The space between the main part and the parts below it, and between those.
const BELOW = 110;
const BESIDE = 90;
/// Room at the top of a box for its title.
const TITLE = 34;

const options: Record<string, string> = {
  'elk.algorithm': 'layered',
  'elk.direction': 'DOWN',
  'elk.hierarchyHandling': 'INCLUDE_CHILDREN',
  'elk.edgeRouting': 'ORTHOGONAL',
  'elk.json.edgeCoords': 'ROOT',
  'elk.json.shapeCoords': 'ROOT',
  'elk.edgeLabels.placement': 'CENTER',
  'elk.spacing.nodeNode': '44',
  'elk.spacing.edgeNode': '26',
  'elk.spacing.edgeEdge': '16',
  'elk.spacing.edgeLabel': '4',
  'elk.spacing.labelNode': '8',
  'elk.spacing.componentComponent': '60',
  'elk.layered.spacing.nodeNodeBetweenLayers': '56',
  'elk.layered.spacing.edgeNodeBetweenLayers': '26',
  'elk.layered.spacing.edgeEdgeBetweenLayers': '16',
  'elk.layered.nodePlacement.strategy': 'NETWORK_SIMPLEX',
  'elk.layered.considerModelOrder.strategy': 'NODES_AND_EDGES',
  // A loop is broken where a row goes back to a cell written earlier, so the cells keep the order the flow writes
  // them in from top to bottom.
  'elk.layered.cycleBreaking.strategy': 'MODEL_ORDER',
  'elk.layered.mergeEdges': 'false',
};

/// The shortest first and last straight piece of an arrow: room for its head, and for the ring around the cell a work
/// is at, which an arrow starts and ends at.
export const STRAIGHT = 16;

// Inside a stage box elkjs takes the box's own options, not the chart's: without its spacing, arrows there left a cell
// and came into one with a piece too short to carry a head; without its node placement, the actions on a line into a
// cell there were placed toward the cells the line leaves, away from the cell they lead into.
const groupOptions: Record<string, string> = {
  'elk.layered.spacing.edgeNodeBetweenLayers': String(STRAIGHT),
  'elk.layered.nodePlacement.strategy': 'NETWORK_SIMPLEX',
  'elk.padding': `[top=${TITLE},left=20,bottom=20,right=20]`,
};

// The arrow out of the actions on a line points down, is short and straight: the actions are laid out just above the
// cell they lead into, or the circle that names it, on the way in, rather than anywhere between the cells their lines
// leave and it.
const WAY_IN: Record<string, string> = {
  'elk.layered.priority.direction': '20',
  'elk.layered.priority.shortness': '20',
  'elk.layered.priority.straightness': '20',
};

async function layPart(
  c: Chart,
  part: string,
  size: (id: string) => Size,
  labelSize: (edge: string) => Size | null,
): Promise<{ nodes: Map<string, Box>; groups: Map<string, Box>; edges: Map<string, Route>; box: Box }> {
  const members = c.nodes.filter((n) => n.part === part);
  const ids = new Set(members.map((n) => n.id));
  const child = (id: string): ElkNode => ({ id, ...size(id) });
  const children: ElkNode[] = [];
  const placed = new Set<string>();
  // Each node and box in the order the flow writes the cells, a box where its first cell comes.
  for (const n of members) {
    if (placed.has(n.id)) continue;
    if (n.group === null) {
      children.push(child(n.id));
      placed.add(n.id);
      continue;
    }
    if (placed.has(`g:${n.group}`)) continue;
    placed.add(`g:${n.group}`);
    const inside = members.filter((m) => m.group === n.group);
    inside.forEach((m) => placed.add(m.id));
    children.push({ id: `g:${n.group}`, layoutOptions: groupOptions, children: inside.map((m) => child(m.id)) });
  }
  const byId = new Map(members.map((n) => [n.id, n]));
  const edges: ElkExtendedEdge[] = c.edges
    .filter((e) => ids.has(e.source) && ids.has(e.target))
    .map((e) => {
      const l = labelSize(e.id);
      return {
        id: e.id,
        sources: [e.source],
        targets: [e.target],
        labels: l ? [{ id: `l:${e.id}`, text: e.label, ...l }] : undefined,
        layoutOptions: byId.get(e.source)?.kind === 'action' ? WAY_IN : undefined,
      };
    });
  const out: ElkNode = await elk.layout({ id: `part:${part}`, layoutOptions: options, children, edges });
  const nodes = new Map<string, Box>();
  const boxes = new Map<string, Box>();
  const walk = (n: ElkNode) => {
    for (const k of n.children ?? []) {
      const b = { x: k.x ?? 0, y: k.y ?? 0, width: k.width ?? 0, height: k.height ?? 0 };
      if (k.id.startsWith('g:')) boxes.set(k.id.slice(2), b);
      else nodes.set(k.id, b);
      walk(k);
    }
  };
  walk(out);
  const routes = new Map<string, Route>();
  const collect = (n: ElkNode) => {
    for (const e of n.edges ?? []) {
      const s = e.sections?.[0];
      if (!s) continue;
      const l = e.labels?.[0];
      const route = {
        points: [s.startPoint, ...(s.bendPoints ?? []), s.endPoint].map((p) => ({ x: p.x, y: p.y })),
        label: l ? { x: l.x ?? 0, y: l.y ?? 0, width: l.width ?? 0, height: l.height ?? 0 } : null,
        hops: [],
      };
      // A loop back to its own cell is given room for both its ends to show beside the ring.
      const loop = e.sources[0] === e.targets[0];
      routes.set(e.id, lengthenLast(onLine(route), loop ? 2 * STRAIGHT : STRAIGHT));
    }
    for (const k of n.children ?? []) collect(k);
  };
  collect(out);
  return { nodes, groups: boxes, edges: routes, box: { x: 0, y: 0, width: out.width ?? 0, height: out.height ?? 0 } };
}

const shift = (b: Box, dx: number, dy: number): Box => ({ ...b, x: b.x + dx, y: b.y + dy });

/// How far a point is from the straight piece from `a` to `b`.
function distance(x: number, y: number, a: Point, b: Point): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const len = dx * dx + dy * dy;
  const k = len === 0 ? 0 : Math.max(0, Math.min(1, ((x - a.x) * dx + (y - a.y) * dy) / len));
  return Math.hypot(x - (a.x + k * dx), y - (a.y + k * dy));
}

/// A route whose label sits on its own line. elkjs leaves the label of some arrows inside a stage box where it gave
/// it no place, at the box's corner over the stage's name; such a label, away from every piece of its arrow, is put
/// on the middle of the arrow's longest straight piece.
export function onLine(r: Route): Route {
  const l = r.label;
  if (l === null || r.points.length < 2) return r;
  const cx = l.x + l.width / 2;
  const cy = l.y + l.height / 2;
  let longest = 0;
  let at = 0;
  let near = Infinity;
  for (let i = 0; i + 1 < r.points.length; i++) {
    const a = r.points[i];
    const b = r.points[i + 1];
    const len = Math.hypot(b.x - a.x, b.y - a.y);
    if (len > longest) {
      longest = len;
      at = i;
    }
    near = Math.min(near, distance(cx, cy, a, b));
  }
  if (near <= Math.max(l.width, l.height) / 2 + 12) return r;
  const a = r.points[at];
  const b = r.points[at + 1];
  return { ...r, label: { ...l, x: (a.x + b.x) / 2 - l.width / 2, y: (a.y + b.y) / 2 - l.height / 2 } };
}

/// Whether a box, grown by `pad` on each side, touches the straight piece from `a` to `b`.
function touches(box: Box, a: Point, b: Point, pad: number): boolean {
  return (
    Math.max(a.x, b.x) >= box.x - pad &&
    Math.min(a.x, b.x) <= box.x + box.width + pad &&
    Math.max(a.y, b.y) >= box.y - pad &&
    Math.min(a.y, b.y) <= box.y + box.height + pad
  );
}

/// A route whose last piece is at least `min` long: where it is shorter (a loop from a cell back to itself, which
/// elkjs draws close around the cell), the piece before it, which runs across it, moves away from the target, and a
/// label beside that piece moves with it. A route of three points, whose piece before the last starts at the
/// source, or one whose piece before that would turn back on itself, is left as it is.
export function lengthenLast(r: Route, min: number): Route {
  const p = r.points;
  const n = p.length;
  if (n < 4) return r;
  const a = p[n - 2];
  const b = p[n - 1];
  const len = Math.hypot(b.x - a.x, b.y - a.y);
  if (len >= min || len < 0.5) return r;
  const u = { x: (b.x - a.x) / len, y: (b.y - a.y) / len };
  const c = p[n - 3];
  if (Math.abs((a.x - c.x) * u.x + (a.y - c.y) * u.y) > 0.5) return r;
  const d = min - len;
  const q = p[n - 4];
  // The piece before that runs along the last one: heading for the target, it gets shorter, and must stay a piece.
  const along = (c.x - q.x) * u.x + (c.y - q.y) * u.y;
  if (along > 0 && along - d < 1) return r;
  const away = (x: Point): Point => ({ x: x.x - u.x * d, y: x.y - u.y * d });
  // A label beside that piece, on the side it moves to and near enough to be run into, moves with it.
  const behind = (l: Box) => (l.x + l.width / 2 - c.x) * u.x + (l.y + l.height / 2 - c.y) * u.y < 0;
  const moves = r.label !== null && behind(r.label) && touches(r.label, c, a, d + 4);
  const label = moves && r.label !== null ? shift(r.label, -u.x * d, -u.y * d) : r.label;
  return { points: [...p.slice(0, n - 3), away(c), away(a), b], label, hops: r.hops };
}

/// The room between two graphs drawn side by side, and the room above each for its name when there are several.
export const BESIDE_GRAPHS = 160;
export const GRAPH_TITLE = 44;

/// Where graphs drawn side by side go, left to right with their tops in line: each one's left edge, how far down
/// they start, and the size they take together. With several, each has room above it for its name.
export function besides(sizes: Size[]): { x: number[]; top: number; width: number; height: number } {
  const top = sizes.length > 1 ? GRAPH_TITLE : 0;
  const x: number[] = [];
  let at = 0;
  for (const s of sizes) {
    x.push(at);
    at += s.width + BESIDE_GRAPHS;
  }
  const width = sizes.length > 0 ? at - BESIDE_GRAPHS : 0;
  return { x, top, width, height: top + Math.max(0, ...sizes.map((s) => s.height)) };
}

/// Lays the chart out. `size` gives each node's size and `labelSize` each arrow's label's, none for no label.
export async function layoutChart(
  c: Chart,
  size: (id: string) => Size,
  labelSize: (edge: string) => Size | null,
): Promise<Laid> {
  const laid = await Promise.all(c.parts.map((p) => layPart(c, p, size, labelSize)));
  const out: Laid = {
    nodes: new Map(),
    groups: new Map(),
    edges: new Map(),
    parts: new Map(),
    bounds: { x: 0, y: 0, width: 0, height: 0 },
  };
  const main = laid[0];
  const rest = laid.slice(1);
  const restWidth = rest.reduce((w, p) => w + p.box.width, 0) + BESIDE * Math.max(0, rest.length - 1);
  const width = Math.max(main.box.width, restWidth);
  const offsets: Point[] = [{ x: (width - main.box.width) / 2, y: 0 }];
  let x = (width - restWidth) / 2;
  for (const p of rest) {
    offsets.push({ x, y: main.box.height + BELOW });
    x += p.box.width + BESIDE;
  }
  const height = main.box.height + (rest.length > 0 ? BELOW + Math.max(...rest.map((p) => p.box.height)) : 0);
  laid.forEach((p, i) => {
    const { x: dx, y: dy } = offsets[i];
    for (const [id, b] of p.nodes) out.nodes.set(id, shift(b, dx, dy));
    for (const [id, b] of p.groups) out.groups.set(id, shift(b, dx, dy));
    for (const [id, r] of p.edges) {
      out.edges.set(id, {
        points: r.points.map((q) => ({ x: q.x + dx, y: q.y + dy })),
        label: r.label ? shift(r.label, dx, dy) : null,
        hops: [],
      });
    }
    out.parts.set(c.parts[i] ?? MAIN, shift(p.box, dx, dy));
  });
  out.bounds = { x: 0, y: 0, width, height };
  addHops(out.edges);
  return out;
}

/// Where each arrow arches over another: a level piece of a route arches where a piece of another route crosses it
/// upright, so each crossing is drawn once, as an arch.
export function addHops(edges: Map<string, Route>): void {
  type Piece = { id: string; at: number; from: number; to: number };
  const upright: Piece[] = [];
  for (const [id, r] of edges) {
    for (let k = 0; k + 1 < r.points.length; k++) {
      const a = r.points[k];
      const b = r.points[k + 1];
      if (Math.abs(a.x - b.x) < 0.5 && Math.abs(a.y - b.y) > 0.5) {
        upright.push({ id, at: a.x, from: Math.min(a.y, b.y), to: Math.max(a.y, b.y) });
      }
    }
  }
  // No arch near either end of a piece, where the bend is rounded.
  const margin = 16;
  for (const [id, r] of edges) {
    r.hops = r.points.slice(1).map(() => []);
    for (let k = 0; k + 1 < r.points.length; k++) {
      const a = r.points[k];
      const b = r.points[k + 1];
      if (Math.abs(a.y - b.y) >= 0.5 || Math.abs(a.x - b.x) < 0.5) continue;
      const lo = Math.min(a.x, b.x);
      const hi = Math.max(a.x, b.x);
      const xs = upright
        .filter((u) => u.id !== id && u.at > lo + margin && u.at < hi - margin && a.y > u.from + 1 && a.y < u.to - 1)
        .map((u) => u.at);
      const sorted = [...new Set(xs)].sort((p, q) => (b.x > a.x ? p - q : q - p));
      r.hops[k] = sorted;
    }
  }
}

/// What `pathOf` cuts off a route's ends: `start` and `end`, for the ring around the cell a work is at and for the
/// arrowhead, which the path's end then carries.
export type Cut = { start?: number; end?: number };

/// How much of the last piece stays straight before the arrowhead, once its end is cut off for one.
const BEFORE_HEAD = 3;

/// The SVG path of a route: right angles rounded, and an arch at each place it crosses another arrow. With its end cut
/// off for an arrowhead, the last bend is rounded no more than leaves the last piece straight for a little before the
/// head, so that the head points along it.
export function pathOf(r: Pick<Route, 'points' | 'hops'>, cut: Cut = {}, radius = 9, hop = 5): string {
  if (r.points.length < 2) return '';
  const len = (a: Point, b: Point) => Math.hypot(b.x - a.x, b.y - a.y);
  const unit = (a: Point, b: Point) => {
    const l = len(a, b) || 1;
    return { x: (b.x - a.x) / l, y: (b.y - a.y) / l };
  };
  // A point moved from `from` towards `to` by `d`, stopping short of `to`.
  const toward = (from: Point, to: Point, d: number): Point => {
    const l = len(from, to);
    const k = Math.min(d, Math.max(0, l - 1)) / (l || 1);
    return { x: from.x + (to.x - from.x) * k, y: from.y + (to.y - from.y) * k };
  };
  const p = [...r.points];
  const last = p.length - 1;
  const end = cut.end ?? 0;
  if (cut.start) p[0] = toward(p[0], p[1], cut.start);
  if (end > 0) p[last] = toward(p[last], p[last - 1], end);
  // The radius each bend is rounded with: no more than half of either piece beside it, and before the head, no more
  // than leaves the last piece straight before it.
  const bend = (k: number) => {
    const after = k + 1 === last && end > 0 ? Math.max(0, len(p[k], p[k + 1]) - BEFORE_HEAD) : len(p[k], p[k + 1]) / 2;
    return Math.min(radius, len(p[k - 1], p[k]) / 2, after);
  };
  const f = (n: number) => Math.round(n * 10) / 10;
  let d = `M ${f(p[0].x)} ${f(p[0].y)}`;
  for (let k = 0; k < last; k++) {
    const a = p[k];
    const b = p[k + 1];
    const u = unit(a, b);
    const final = k + 1 === last;
    const rEnd = final ? 0 : bend(k + 1);
    const stop = { x: b.x - u.x * rEnd, y: b.y - u.y * rEnd };
    for (const x of r.hops[k] ?? []) {
      const s = Math.sign(b.x - a.x);
      d += ` L ${f(x - s * hop)} ${f(a.y)} A ${hop} ${hop} 0 0 ${s > 0 ? 1 : 0} ${f(x + s * hop)} ${f(a.y)}`;
    }
    d += ` L ${f(stop.x)} ${f(stop.y)}`;
    if (!final) {
      const v = unit(b, p[k + 2]);
      d += ` Q ${f(b.x)} ${f(b.y)} ${f(b.x + v.x * rEnd)} ${f(b.y + v.y * rEnd)}`;
    }
  }
  return d;
}
