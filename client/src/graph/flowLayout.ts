// Lays a flow's chart out with elkjs: each part of the chart on its own, top to bottom, its stage boxes as boxes that
// hold their cells, arrows routed at right angles around the boxes with room for their labels; then the main part on
// top and the other parts in a row below it. Each arrow's route also says where it crosses an arrow that runs the
// other way, so that it is drawn with a small arch there.

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

const groupOptions: Record<string, string> = {
  'elk.padding': `[top=${TITLE},left=20,bottom=20,right=20]`,
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
  const edges: ElkExtendedEdge[] = c.edges
    .filter((e) => ids.has(e.source) && ids.has(e.target))
    .map((e) => {
      const l = labelSize(e.id);
      return {
        id: e.id,
        sources: [e.source],
        targets: [e.target],
        labels: l ? [{ id: `l:${e.id}`, text: e.label, ...l }] : undefined,
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
      routes.set(e.id, {
        points: [s.startPoint, ...(s.bendPoints ?? []), s.endPoint].map((p) => ({ x: p.x, y: p.y })),
        label: l ? { x: l.x ?? 0, y: l.y ?? 0, width: l.width ?? 0, height: l.height ?? 0 } : null,
        hops: [],
      });
    }
    for (const k of n.children ?? []) collect(k);
  };
  collect(out);
  return { nodes, groups: boxes, edges: routes, box: { x: 0, y: 0, width: out.width ?? 0, height: out.height ?? 0 } };
}

const shift = (b: Box, dx: number, dy: number): Box => ({ ...b, x: b.x + dx, y: b.y + dy });

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

/// The SVG path of a route: right angles rounded, and an arch at each place it crosses another arrow.
export function pathOf(r: Pick<Route, 'points' | 'hops'>, radius = 9, hop = 5): string {
  const p = r.points;
  if (p.length < 2) return '';
  const len = (a: Point, b: Point) => Math.hypot(b.x - a.x, b.y - a.y);
  const unit = (a: Point, b: Point) => {
    const l = len(a, b) || 1;
    return { x: (b.x - a.x) / l, y: (b.y - a.y) / l };
  };
  // The radius each bend is rounded with: no more than half of either piece beside it.
  const bend = (k: number) => Math.min(radius, len(p[k - 1], p[k]) / 2, len(p[k], p[k + 1]) / 2);
  const f = (n: number) => Math.round(n * 10) / 10;
  let d = `M ${f(p[0].x)} ${f(p[0].y)}`;
  for (let k = 0; k + 1 < p.length; k++) {
    const a = p[k];
    const b = p[k + 1];
    const u = unit(a, b);
    const last = k + 1 === p.length - 1;
    const rEnd = last ? 0 : bend(k + 1);
    const end = { x: b.x - u.x * rEnd, y: b.y - u.y * rEnd };
    for (const x of r.hops[k] ?? []) {
      const s = Math.sign(b.x - a.x);
      d += ` L ${f(x - s * hop)} ${f(a.y)} A ${hop} ${hop} 0 0 ${s > 0 ? 1 : 0} ${f(x + s * hop)} ${f(a.y)}`;
    }
    d += ` L ${f(end.x)} ${f(end.y)}`;
    if (!last) {
      const v = unit(b, p[k + 2]);
      d += ` Q ${f(b.x)} ${f(b.y)} ${f(b.x + v.x * rEnd)} ${f(b.y + v.y * rEnd)}`;
    }
  }
  return d;
}
