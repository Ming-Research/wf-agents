// Lays the flowchart out with elkjs: layered, top to bottom, each cell with what is drawn beside it as one node laid
// out on its own (`units.ts`), the main path and the ways back routed at right angles around the nodes with room for
// their labels and meeting each cell in the lane of its edge, each with a last straight piece long enough for its
// head. Each arrow's route also says where it crosses an arrow that runs the other way, so that it is drawn with a
// small arch there; and its label is moved near where it leaves its node, clear of the nodes and the other labels.
//
// Brought over from the chart of `old-main` (`client/src/graph/flowLayout.ts`) as it was, but for what this flow has
// none of: the stage boxes round a stage's cells, and the parts of the chart laid out apart, this flow's chart being
// one part with no boxes; and elkjs runs in a worker where the page runs, since it takes seconds to lay the engineer
// flow out as thoroughly as `old-main` did, which on the page's own thread held the whole page still.

import ELK, { type ELK as Elk, type ElkExtendedEdge, type ElkNode } from 'elkjs/lib/elk-api.js';
import workerUrl from 'elkjs/lib/elk-worker.min.js?url';

import { type Chart, type ChartEdge, type ChartNode, cellNode } from './model';
import { attachments, landings, layUnit, type Unit } from './units';

/// elkjs, in a worker where there are workers, and on the page's own thread where there are none (the page's tests).
let elk: Promise<Elk> | null = null;
function elkOf(): Promise<Elk> {
  elk ??=
    typeof Worker !== 'undefined'
      ? Promise.resolve(new ELK({ workerUrl }))
      : import('elkjs/lib/elk.bundled.js').then((m) => {
          // A module of CommonJS, as it is read where there are no workers: its export, or that export's default.
          const made = m.default as typeof m.default & { default?: typeof m.default };
          return new (made.default ?? made)();
        });
  return elk;
}

export type Point = { x: number; y: number };
export type Box = { x: number; y: number; width: number; height: number };
export type Size = { width: number; height: number };

/// An arrow as laid out: the points its route passes through, its label's box, and for each straight piece of the
/// route, the places along it where it arches over another arrow.
export type Route = { points: Point[]; label: Box | null; hops: number[][] };

export type Laid = {
  nodes: Map<string, Box>;
  edges: Map<string, Route>;
  bounds: Box;
};

/// The room between arrows side by side, and how hard the layout tries to order each layer for fewer crossings
/// (elkjs's own is 7).
const EDGE_EDGE = 10;
const THOROUGH = 40;

const options: Record<string, string> = {
  'elk.algorithm': 'layered',
  'elk.direction': 'DOWN',
  'elk.edgeRouting': 'ORTHOGONAL',
  'elk.json.edgeCoords': 'ROOT',
  'elk.json.shapeCoords': 'ROOT',
  'elk.edgeLabels.placement': 'CENTER',
  'elk.spacing.nodeNode': '44',
  'elk.spacing.edgeNode': '26',
  'elk.spacing.edgeEdge': String(EDGE_EDGE),
  'elk.spacing.edgeLabel': '4',
  'elk.spacing.labelNode': '8',
  'elk.spacing.componentComponent': '60',
  'elk.layered.spacing.nodeNodeBetweenLayers': '40',
  'elk.layered.spacing.edgeNodeBetweenLayers': '26',
  'elk.layered.spacing.edgeEdgeBetweenLayers': String(EDGE_EDGE),
  'elk.layered.thoroughness': String(THOROUGH),
  'elk.layered.nodePlacement.strategy': 'NETWORK_SIMPLEX',
  'elk.layered.considerModelOrder.strategy': 'NODES_AND_EDGES',
  // A loop is broken where an arrow goes back to a cell earlier in the walk, so the cells keep the walk's order from
  // top to bottom.
  'elk.layered.cycleBreaking.strategy': 'MODEL_ORDER',
  'elk.layered.mergeEdges': 'false',
  // The layers drawn as close together as their arrows let them, and a cell with many arrows out given room for what
  // they lead to beside it, rather than spreading its layer and making long level runs to its far ends.
  'elk.layered.compaction.postCompaction.strategy': 'EDGE_LENGTH',
  'elk.layered.highDegreeNodes.treatment': 'true',
  'elk.layered.highDegreeNodes.threshold': '8',
};

/// The shortest first and last straight piece of an arrow: room for its head.
export const STRAIGHT = 16;

/// Lays a graph out; elkjs fails at drawing the layers closer together on some graphs, which are then laid out
/// without that.
async function lay(g: ElkNode): Promise<ElkNode> {
  const elk = await elkOf();
  try {
    return await elk.layout(g);
  } catch {
    const loose = (n: ElkNode): ElkNode => ({
      ...n,
      layoutOptions: n.layoutOptions
        ? Object.fromEntries(Object.entries(n.layoutOptions).filter(([k]) => !k.endsWith('postCompaction.strategy')))
        : undefined,
      children: n.children?.map(loose),
    });
    return elk.layout(loose(g));
  }
}

// The arrow out of the way into a cell points down, is short and straight: the way in is laid out just above the cell
// it leads into, rather than anywhere between the cells its arrows leave and it.
const WAY_IN: Record<string, string> = {
  'elk.layered.priority.direction': '20',
  'elk.layered.priority.shortness': '20',
  'elk.layered.priority.straightness': '20',
};

// A step of the main path points down and is kept straight before every other arrow, the many arrows a cell has
// together too, so that the path runs straight down the chart and the other arrows go round it.
const MAIN_STRAIGHT = '200';
const MAIN: Record<string, string> = {
  'elk.layered.priority.direction': '10',
  'elk.layered.priority.straightness': MAIN_STRAIGHT,
};
const MAIN_WAY_IN: Record<string, string> = { ...WAY_IN, 'elk.layered.priority.straightness': MAIN_STRAIGHT };

/// A node the arrows meet at fixed places in the second layout: a cell's unit, where they meet the cell's top and
/// bottom edges, or the way into a cell, where they meet its own; its size, the box they meet in it, and the lanes
/// along that box's top and bottom edges.
type Ported = { size: Size; box: Box; top: { from: number; to: number }; bottom: { from: number; to: number } };

/// How far from the corners of the way into a cell an arrow meets it.
const WAY_IN_INSET = 4;

/// Where an arrow meets a cell: on its top edge or its bottom edge, and where along it from the unit's left.
type Side = 'NORTH' | 'SOUTH';
type Port = { id: string; side: Side; x: number };

/// Where a layout put each node, and the points of each arrow's route.
function placement(out: ElkNode): { boxes: Map<string, Box>; routes: Map<string, Point[]> } {
  const boxes = new Map<string, Box>();
  const routes = new Map<string, Point[]>();
  const walk = (n: ElkNode) => {
    for (const e of n.edges ?? []) {
      const s = e.sections?.[0];
      if (s) routes.set(e.id, [s.startPoint, ...(s.bendPoints ?? []), s.endPoint]);
    }
    for (const k of n.children ?? []) {
      boxes.set(k.id, { x: k.x ?? 0, y: k.y ?? 0, width: k.width ?? 0, height: k.height ?? 0 });
      walk(k);
    }
  };
  walk(out);
  return { boxes, routes };
}

/// Lays the chart out. `size` gives each node's size and `labelSize` each arrow's label's, none for no label.
export async function layoutChart(
  c: Chart,
  size: (id: string) => Size,
  labelSize: (edge: string) => Size | null,
): Promise<Laid> {
  const members = c.nodes;
  // Each cell with what is beside it is one node of the layout; the lines among them are drawn by the unit.
  const byId = new Map(members.map((n) => [n.id, n]));
  const owner = attachments(members, c.edges);
  const unitOf = (id: string) => owner.get(id) ?? id;
  const inside = (e: ChartEdge) =>
    unitOf(e.source) === unitOf(e.target) && (owner.has(e.source) || owner.has(e.target));
  // The arrows a write runs are not laid out: the layout is the main path's and the ways back's, and those arrows are
  // drawn over it, a few at a time (`data.ts`), so that what is not drawn does not bend what is.
  const external = c.edges.filter((e) => !inside(e) && !(e.kind === 'arrow' && !e.back));
  const laidOut = members.filter((n) => !owner.has(n.id));
  const units = new Map<string, Unit>(
    laidOut
      .filter((n) => n.kind === 'cell')
      .map((n) => [
        n.id,
        layUnit(
          n,
          members.filter((m) => owner.get(m.id) === n.id),
          c.edges.filter((e) => inside(e) && unitOf(e.source) === n.id),
          size,
        ),
      ]),
  );
  const ported = new Map<string, Ported>();
  for (const [id, u] of units) {
    const box = u.boxes.get(id);
    if (box !== undefined) ported.set(id, { size: u.size, box, top: u.top, bottom: u.bottom });
  }
  for (const n of laidOut) {
    if (n.kind !== 'way-in' && n.kind !== 'fork') continue;
    const s = size(n.id);
    const lane = { from: WAY_IN_INSET, to: s.width - WAY_IN_INSET };
    ported.set(n.id, { size: s, box: { x: 0, y: 0, ...s }, top: lane, bottom: lane });
  }
  // A loop elkjs does not always break by the order the flow writes the cells in, so every arrow that goes back up the
  // walk is given to it already turned round, and the chart holds no loop for elkjs to break: the nodes keep the walk's
  // order, the way into a cell stays just above it, and a way back comes up.
  const place = new Map(laidOut.map((n, i) => [n.id, i]));
  const order = new Map(members.map((n, i) => [n.id, i]));
  // Where a node comes in the chart's order: where it, or the cell it is drawn beside, comes among the nodes laid out;
  // then where the flow writes its cell; the way into a cell just before it.
  const rank = (n: ChartNode): number[] => [
    place.get(unitOf(n.id)) ?? 0,
    order.get(cellNode(n.cell)) ?? 0,
    n.kind === 'cell' ? 1 : 0,
  ];
  const turned = (e: ChartEdge) => {
    const to = byId.get(e.target);
    const from = byId.get(e.source);
    if (to === undefined || from === undefined) return false;
    const [a, b] = [rank(from), rank(to)];
    const k = a.findIndex((x, i) => x !== b[i]);
    return k >= 0 && a[k] > b[k];
  };
  // The arrows that run up the chart, found from the first layout: given to the second turned round too, so that elkjs
  // takes each as running down from its target into its source rather than as one it turned round itself, which it
  // routes out of the edge it would leave by had it not, round the cell and back, and up past the cells between in a
  // tangle of turns back and pieces over each other's.
  const upward = new Set<string>();
  const given = (e: ChartEdge) => turned(e) || upward.has(e.id);
  /// A route of the layout as the chart's arrow runs, from its source to its target.
  const along = (e: ChartEdge, ps: Point[]) => (given(e) ? [...ps].reverse() : ps);
  // Laid out twice: first with the arrows free to meet each unit and each way into a cell anywhere, for which edge of
  // it each meets, the top or the bottom as the layout turns the arrows of a loop, and in which order; then with each
  // where that order puts it in the lane of that edge.
  const graph = (ports: Map<string, Port[]> | null): ElkNode => {
    const child = (id: string): ElkNode => {
      const u = ported.get(id);
      if (u === undefined) return { id, ...size(id) };
      if (ports === null) return { id, ...u.size };
      return {
        id,
        ...u.size,
        layoutOptions: { 'elk.portConstraints': 'FIXED_POS' },
        ports: (ports.get(id) ?? []).map((p) => ({
          id: p.id,
          width: 0,
          height: 0,
          x: p.x,
          y: p.side === 'NORTH' ? 0 : u.size.height,
          layoutOptions: { 'elk.port.side': p.side },
        })),
      };
    };
    const edges: ElkExtendedEdge[] = external.map((e) => {
      const l = labelSize(e.id);
      const end = (id: string, which: 'source' | 'target') =>
        ports !== null && ported.has(id) ? `${e.id}|${which}` : id;
      return {
        id: e.id,
        sources: given(e) ? [end(e.target, 'target')] : [end(e.source, 'source')],
        targets: given(e) ? [end(e.source, 'source')] : [end(e.target, 'target')],
        labels: l ? [{ id: `l:${e.id}`, text: e.label, ...l }] : undefined,
        layoutOptions: e.kind === 'in' ? (e.main ? MAIN_WAY_IN : WAY_IN) : e.kind === 'step' ? MAIN : undefined,
      };
    });
    return { id: 'chart', layoutOptions: options, children: laidOut.map((n) => child(n.id)), edges };
  };
  const free = placement(await lay(graph(null)));
  for (const e of external) {
    const from = free.boxes.get(e.source);
    const to = free.boxes.get(e.target);
    if (!turned(e) && from !== undefined && to !== undefined && to.y + to.height <= from.y) upward.add(e.id);
  }
  // Where each arrow met each cell's unit: its top or its bottom, or for one that met a side, the way it turned from
  // there; and where along the cell from its left edge.
  const meets = new Map<string, Array<{ id: string; side: Side; at: number; main: boolean }>>();
  for (const e of external) {
    const route = free.routes.get(e.id);
    if (route === undefined) continue;
    const ps = along(e, route);
    for (const end of ['source', 'target'] as const) {
      const unit = free.boxes.get(e[end]);
      const cell = ported.get(e[end])?.box;
      if (unit === undefined || cell === undefined) continue;
      const way = end === 'source' ? ps : [...ps].reverse();
      const p = way[0];
      const turn = way.find((q) => Math.abs(q.y - p.y) > 0.5);
      const laid: Side =
        Math.abs(p.y - unit.y) < 1 || (Math.abs(p.y - unit.y - unit.height) >= 1 && turn !== undefined && turn.y < p.y)
          ? 'NORTH'
          : 'SOUTH';
      // An arrow to or from a node wholly above the cell meets its top, and one wholly below it its bottom, whichever
      // edge elkjs left it at: elkjs routes an arrow it turned round to break a loop out of the edge it would leave by
      // had it not, then round the cell and back, and up past the cells between in a tangle; where along the edge,
      // toward the other end.
      const far = free.boxes.get(e[end === 'source' ? 'target' : 'source']);
      const above = far !== undefined && far.y + far.height <= unit.y;
      const below = far !== undefined && far.y >= unit.y + unit.height;
      const side: Side = above ? 'NORTH' : below ? 'SOUTH' : laid;
      const at = side === laid || far === undefined ? p.x : far.x + far.width / 2;
      const one = { id: `${e.id}|${end}`, side, at: at - unit.x - cell.x, main: e.main };
      meets.set(e[end], [...(meets.get(e[end]) ?? []), one]);
    }
  }
  const portsOf = new Map<string, Port[]>();
  for (const [cell, ms] of meets) {
    const u = ported.get(cell);
    if (u === undefined) continue;
    const x0 = u.box.x;
    const placed: Port[] = [];
    for (const side of ['NORTH', 'SOUTH'] as const) {
      const lane = side === 'NORTH' ? u.top : u.bottom;
      const ours = ms.filter((p) => p.side === side);
      const mains = ours.filter((p) => p.main).sort((p, q) => p.at - q.at);
      const rest = ours.filter((p) => !p.main).sort((p, q) => p.at - q.at);
      // As far apart as arrows side by side, so that elkjs moves none over onto the line of the one beside it.
      const land = (ps: typeof ours, from: number, to: number) => {
        const xs = landings(ps.map((p) => x0 + p.at), from, to, EDGE_EDGE);
        ps.forEach((p, i) => placed.push({ id: p.id, side, x: xs[i] }));
      };
      if (mains.length === 0) {
        land(rest, lane.from + 4, lane.to - 4);
        continue;
      }
      // The main path meets a cell in the middle of its edge, its steps side by side round the middle in the order
      // they came, and the other arrows on either side of them, each on the side it came from. The branches out of a
      // fork leave it spread over its bottom edge, so that each branch's value stands by its own line.
      const middle = x0 + u.box.width / 2;
      const branches = side === 'SOUTH' && byId.get(cell)?.kind === 'fork' && mains.length > 1;
      const apart = branches ? Math.max(EDGE_EDGE, u.box.width / (mains.length + 1)) : EDGE_EDGE;
      const first = middle - ((mains.length - 1) / 2) * apart;
      mains.forEach((p, i) => placed.push({ id: p.id, side, x: first + i * apart }));
      const last = first + (mains.length - 1) * apart;
      land(rest.filter((p) => x0 + p.at < middle), lane.from + 4, first - EDGE_EDGE);
      land(rest.filter((p) => x0 + p.at >= middle), last + EDGE_EDGE, lane.to - 4);
    }
    portsOf.set(cell, placed);
  }
  const out: ElkNode = await lay(graph(portsOf));
  const nodes = new Map<string, Box>();
  const routes = new Map<string, Route>();
  // Where each cell is, for the arrows that meet it.
  const cells = new Map<string, Box>();
  for (const k of out.children ?? []) {
    const b = { x: k.x ?? 0, y: k.y ?? 0, width: k.width ?? 0, height: k.height ?? 0 };
    const u = units.get(k.id);
    if (u === undefined) {
      nodes.set(k.id, b);
      continue;
    }
    for (const [id, x] of u.boxes) nodes.set(id, shift(x, b.x, b.y));
    for (const [id, r] of u.links) {
      routes.set(id, { points: r.points.map((p) => ({ x: p.x + b.x, y: p.y + b.y })), label: null, hops: [] });
    }
    cells.set(k.id, nodes.get(k.id) ?? b);
  }
  const sideOf = new Map([...portsOf.values()].flat().map((p) => [p.id, p.side]));
  // An arrow that meets a unit at its edge goes on straight to the cell's own edge.
  const reach = (p: Point, port: string, cell: string): Point => {
    const b = cells.get(cell);
    const side = sideOf.get(port);
    if (b === undefined || side === undefined) return p;
    return { x: p.x, y: side === 'NORTH' ? b.y : b.y + b.height };
  };
  const chartEdges = new Map(external.map((e) => [e.id, e]));
  for (const e of out.edges ?? []) {
    const s = e.sections?.[0];
    const ce = chartEdges.get(e.id);
    // A tie is laid out for the order it keeps, and drawn by nothing.
    if (!s || !ce || ce.kind === 'tie') continue;
    const l = e.labels?.[0];
    const ps = along(
      ce,
      [s.startPoint, ...(s.bendPoints ?? []), s.endPoint].map((p) => ({ x: p.x, y: p.y })),
    );
    ps[0] = reach(ps[0], `${ce.id}|source`, ce.source);
    ps[ps.length - 1] = reach(ps[ps.length - 1], `${ce.id}|target`, ce.target);
    const route = {
      points: straightened(ps),
      label: l ? { x: l.x ?? 0, y: l.y ?? 0, width: l.width ?? 0, height: l.height ?? 0 } : null,
      hops: [],
    };
    routes.set(e.id, lengthenLast(onLine(route), STRAIGHT));
  }
  // Each arrow's label near the node it leaves, clear of every node and of the labels placed before it.
  const outward = [...chartEdges.keys()].filter((id) => routes.get(id)?.label != null);
  const edges = nearStart(routes, outward, [...nodes.values()]);
  addHops(edges);
  return { nodes, edges, bounds: { x: 0, y: 0, width: out.width ?? 0, height: out.height ?? 0 } };
}

/// The least a piece between two bends is long; a shorter one is a jog of a few pixels, which reads as a kink.
const JOG = 6;

/// A route with its jogs taken out: where a piece between two bends is shorter than `JOG`, the route after it moves
/// over onto the line before it, up to its next bend; then repeated points and points in the middle of a straight run
/// go.
export function straightened(ps: Point[]): Point[] {
  const near = (a: number, b: number) => Math.abs(a - b) < 0.5;
  const p = ps.map((q) => ({ ...q }));
  for (let i = 1; i + 2 < p.length; i++) {
    const a = p[i];
    const b = p[i + 1];
    const dx = Math.abs(b.x - a.x);
    const dy = Math.abs(b.y - a.y);
    if (dx + dy === 0 || dx + dy >= JOG) continue;
    if (near(dy, 0)) {
      const x = b.x;
      for (let k = i + 1; k < p.length && near(p[k].x, x); k++) p[k].x = a.x;
    } else if (near(dx, 0)) {
      const y = b.y;
      for (let k = i + 1; k < p.length && near(p[k].y, y); k++) p[k].y = a.y;
    }
  }
  const out: Point[] = [];
  for (const q of p) {
    const last = out[out.length - 1];
    if (last && near(last.x, q.x) && near(last.y, q.y)) continue;
    out.push(q);
    while (out.length >= 3) {
      const [u, v, w] = out.slice(-3);
      if ((near(u.x, v.x) && near(v.x, w.x)) || (near(u.y, v.y) && near(v.y, w.y))) out.splice(out.length - 2, 1);
      else break;
    }
  }
  return out;
}

/// How far a label moves along its arrow at a time to keep clear of what is already there, and the room it keeps from
/// it.
const LABEL_STEP = 4;
const LABEL_CLEAR = 3;

/// The point `d` along a route from its start; none past its end.
export function pointAlong(ps: Point[], d: number): Point | null {
  let left = d;
  for (let i = 0; i + 1 < ps.length; i++) {
    const a = ps[i];
    const b = ps[i + 1];
    const len = Math.hypot(b.x - a.x, b.y - a.y);
    if (left <= len) {
      const k = len === 0 ? 0 : left / len;
      return { x: a.x + (b.x - a.x) * k, y: a.y + (b.y - a.y) * k };
    }
    left -= len;
  }
  return null;
}

/// Whether two boxes, one grown by `pad` on each side, overlap.
export function overlap(a: Box, b: Box, pad = 0): boolean {
  return a.x < b.x + b.width + pad && b.x < a.x + a.width + pad && a.y < b.y + b.height + pad && b.y < a.y + a.height + pad;
}

/// The routes with the label of each of `ids` moved to near its start, on its line, where the owner looks for a cell's
/// ways out and where it stays in view however far the arrow runs: the first place along the arrow, a step at a time
/// from where it leaves its cell, where the label touches none of `fixed` and no label placed before it, short of the
/// arrow's head, the labels with the least room placed first. A label with no such place clear of `fixed` (an arrow too
/// short) keeps the layout's, which the others keep clear of too; and one whose places the others took keeps the
/// layout's.
export function nearStart(routes: Map<string, Route>, ids: string[], fixed: Box[]): Map<string, Route> {
  const out = new Map(routes);
  // The places a label may take along its route, nearest its start first.
  const places = (r: Route, l: Box): Box[] => {
    let total = 0;
    for (let i = 0; i + 1 < r.points.length; i++) {
      total += Math.hypot(r.points[i + 1].x - r.points[i].x, r.points[i + 1].y - r.points[i].y);
    }
    const last = total - HEAD_ROOM - Math.max(l.width, l.height) / 2;
    const all: Box[] = [];
    for (let d = 0; d <= last; d += LABEL_STEP) {
      const p = pointAlong(r.points, d);
      if (p === null) break;
      all.push({ x: p.x - l.width / 2, y: p.y - l.height / 2, width: l.width, height: l.height });
    }
    return all;
  };
  const clear = (at: Box, of: Box[]) => !of.some((b) => overlap(at, b, LABEL_CLEAR));
  const start = (id: string) => routes.get(id)?.points[0] ?? { x: 0, y: 0 };
  const order = [...ids].sort((a, b) => start(a).y - start(b).y || start(a).x - start(b).x);
  const kept: Box[] = [];
  const room = new Map<string, number>();
  for (const id of order) {
    const r = routes.get(id);
    const l = r?.label;
    if (r === undefined || l == null) continue;
    const free = places(r, l).filter((at) => clear(at, fixed)).length;
    if (free > 0) room.set(id, free);
    else kept.push(l);
  }
  // The labels with the least room first, so that one on a short arrow is not crowded out by one on a long arrow
  // beside it.
  const moving = [...room.keys()].sort((a, b) => (room.get(a) ?? 0) - (room.get(b) ?? 0));
  const placed: Box[] = [];
  for (const id of moving) {
    const r = routes.get(id);
    const l = r?.label;
    if (r === undefined || l == null) continue;
    const label = places(r, l).find((at) => clear(at, [...fixed, ...kept, ...placed])) ?? l;
    placed.push(label);
    out.set(id, { ...r, label });
  }
  return out;
}

/// The room a label leaves before the end of its arrow, for the arrow's head.
export const HEAD_ROOM = 10;

const shift = (b: Box, dx: number, dy: number): Box => ({ ...b, x: b.x + dx, y: b.y + dy });

/// How far a point is from the straight piece from `a` to `b`.
function distance(x: number, y: number, a: Point, b: Point): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const len = dx * dx + dy * dy;
  const k = len === 0 ? 0 : Math.max(0, Math.min(1, ((x - a.x) * dx + (y - a.y) * dy) / len));
  return Math.hypot(x - (a.x + k * dx), y - (a.y + k * dy));
}

/// A route whose label sits on its own line: a label elkjs left away from every piece of its arrow is put on the
/// middle of the arrow's longest straight piece.
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

/// A route whose last piece is at least `min` long: where it is shorter, the piece before it, which runs across it,
/// moves away from the target, and a label beside that piece moves with it. A route of three points, whose piece
/// before the last starts at the source, or one whose piece before that would turn back on itself, is left as it is.
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

/// What `pathOf` cuts off a route's ends: `end` for the arrowhead, which the path's end then carries.
export type Cut = { start?: number; end?: number };

/// How much of the last piece stays straight before the arrowhead, once its end is cut off for one.
const BEFORE_HEAD = 3;

/// The SVG path of a route: right angles rounded, and an arch at each place it crosses another arrow. With its end cut
/// off for an arrowhead, the last bend is rounded no more than leaves the last piece straight for a little before the
/// head, so that the head points along it.
export function pathOf(r: Pick<Route, 'points' | 'hops'>, cut: Cut = {}, radius = 4, hop = 5): string {
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
