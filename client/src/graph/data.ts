// The arrows a write runs, drawn over the flowchart once it is laid out, a few at a time: those of the cell chosen, or
// every one where the owner shows them all (`shownArrows`). The layout is the main path's and the ways back's alone, so
// that what is not drawn does not bend what is, and the arrows drawn are routed here, the same way for every cell.
//
// The arrows from one cell that carry the same name are one trunk (`trunksOf`): it leaves the cell once, its name said
// once beside it, and runs down a corridor beside the cells, from which a branch turns off into each cell it runs, with
// what else runs that cell said near its end. A trunk runs on the side of the main path that crosses it least, as near
// the cells as the nodes and the other lines let it, and straight down where nothing stands between; trunks side by
// side keep lanes apart, the inner one leaving its cell nearer the middle and turning further from it, so that they do
// not cross each other where they leave a cell or come into one.

import { type Box, HEAD_ROOM, type Point, type Route, type Size, addHops, overlap, pointAlong, straightened } from './layout';
import { type Carried, type ChartEdge, cellNode } from './model';

/// The arrows from one cell drawn as one trunk.
export type Trunk = {
  /// The cell that writes.
  from: string;
  /// Each arrow of it, to a cell it runs.
  arrows: ChartEdge[];
  /// The names every arrow of it carries, said once beside the trunk.
  names: Carried[];
};

/// The trunks of a set of arrows: for each cell that writes, in the order of the arrows, the name the most of its
/// arrows carry (of a tie, the one carried first) makes a trunk of every arrow that carries it, and the rest likewise,
/// each arrow in one trunk.
export function trunksOf(arrows: ChartEdge[]): Trunk[] {
  const out: Trunk[] = [];
  for (const from of [...new Set(arrows.map((a) => a.from ?? ''))]) {
    let left = arrows.filter((a) => (a.from ?? '') === from);
    while (left.length > 0) {
      const named = left.flatMap((a) => a.carries.map((c) => c.name));
      const count = (n: string) => named.filter((x) => x === n).length;
      const best = named.reduce((b, n) => (count(n) > count(b) ? n : b), named[0]);
      const these = left.filter((a) => a.carries.some((c) => c.name === best));
      left = left.filter((a) => !these.includes(a));
      const names = these[0].carries.filter((c) => these.every((a) => a.carries.some((d) => d.name === c.name)));
      out.push({ from, arrows: these, names });
    }
  }
  return out;
}

/// What an arrow of a trunk carries besides the trunk's names, said near its end.
export function extrasOf(trunk: Trunk, arrow: ChartEdge): Carried[] {
  return arrow.carries.filter((c) => !trunk.names.some((n) => n.name === c.name));
}

/// A label of the arrows drawn here: the names it says, where, and the arrows it is of.
export type DataLabel = { id: string; box: Box; carries: Carried[]; arrows: ChartEdge[] };

/// The room between lanes side by side, and between turns in one gap; how far a corridor keeps from a node; the first
/// straight piece out of a cell, and the last into a cell, which holds the head.
const LANE = 10;
const TURN = 6;
const CLEAR = 12;
const OUT = 10;
const IN = 16;
/// What drawing a trunk as two costs, against the other things a route is weighed by.
const SPLIT = 3000;
/// How far from an edge's corners an arrow meets it.
const INSET = 8;

type Side = 1 | -1;
type Upright = { x: number; y0: number; y1: number };
type Level = { y: number; x0: number; x1: number };

/// The pieces of routes, upright and level.
function piecesOf(routes: Iterable<Route>): { upright: Upright[]; level: Level[] } {
  const upright: Upright[] = [];
  const level: Level[] = [];
  for (const r of routes) {
    for (let i = 0; i + 1 < r.points.length; i++) {
      const a = r.points[i];
      const b = r.points[i + 1];
      if (Math.abs(a.x - b.x) < 0.5) upright.push({ x: a.x, y0: Math.min(a.y, b.y), y1: Math.max(a.y, b.y) });
      else if (Math.abs(a.y - b.y) < 0.5) level.push({ y: a.y, x0: Math.min(a.x, b.x), x1: Math.max(a.x, b.x) });
    }
  }
  return { upright, level };
}

/// Where along an edge of a box, from its middle out to one side, the next arrow can meet it: the first place a lane
/// from the middle that no arrow meets already; the outermost where none is left.
function portOf(box: Box, side: Side, taken: number[]): number {
  const mid = box.x + box.width / 2;
  const far = side > 0 ? box.x + box.width - INSET : box.x + INSET;
  for (let k = 1; ; k++) {
    const x = mid + side * k * LANE;
    if ((x - far) * side > 0) return far;
    if (!taken.some((t) => Math.abs(t - x) < LANE / 2)) return x;
  }
}

/// The arrows drawn over a laid-out chart: the route of each arrow of `trunks`, and their labels. `nodes` are the boxes
/// of the chart's nodes, `routes` the lines already drawn, `main` which of them are the main path's, `labels` the labels
/// already placed, and `size` the size of a label's words.
export function routeTrunks(
  trunks: Trunk[],
  nodes: Map<string, Box>,
  routes: Map<string, Route>,
  main: Set<string>,
  labels: Box[],
  size: (words: string) => Size,
): { routes: Map<string, Route>; labels: DataLabel[] } {
  const boxes = [...nodes.values()];
  const lines = piecesOf(routes.values());
  const mainLines = piecesOf([...routes].filter(([id]) => main.has(id)).map(([, r]) => r));
  // Where arrows already meet each node's top and bottom.
  const meets = new Map<string, number[]>();
  const meet = (id: string, edge: 'top' | 'bottom', x: number) =>
    meets.set(`${id}|${edge}`, [...(meets.get(`${id}|${edge}`) ?? []), x]);
  for (const r of routes.values()) {
    for (const p of [r.points[0], r.points[r.points.length - 1]]) {
      if (p === undefined) continue;
      for (const [id, b] of nodes) {
        if (p.x < b.x || p.x > b.x + b.width) continue;
        if (Math.abs(p.y - b.y) < 1) meet(id, 'top', p.x);
        if (Math.abs(p.y - b.y - b.height) < 1) meet(id, 'bottom', p.x);
      }
    }
  }
  const takenAt = (id: string, edge: 'top' | 'bottom') => meets.get(`${id}|${edge}`) ?? [];

  const corridors: Upright[] = [];
  /// The side of a box a corridor at `x` stands on, which the arrows meet the box on.
  const facing = (box: Box, x: number): Side => (x >= box.x + box.width / 2 ? 1 : -1);

  // The corridor of a trunk going one way from `x`: moved out past every node and every line that stands in its way
  // between the turn under the cell it leaves and the turns over the cells it runs.
  const corridor = (x: number, y0: number, y1: number, way: Side): number => {
    let at = x;
    for (let round = 0; round < 200; round++) {
      const was = at;
      for (const b of boxes) {
        const across = at > b.x - CLEAR && at < b.x + b.width + CLEAR;
        if (across && y1 > b.y && y0 < b.y + b.height) at = way > 0 ? b.x + b.width + CLEAR : b.x - CLEAR;
      }
      for (const u of [...lines.upright, ...corridors]) {
        if (Math.abs(u.x - at) < LANE && u.y1 > y0 && u.y0 < y1) at = u.x + way * LANE;
      }
      if (at === was) break;
    }
    return at;
  };
  // How many lines of the main path a route's level pieces and its corridor cross; how many nodes its level pieces
  // run under, besides the cells it leaves and comes into.
  const crossings = (level: Level[], up: Upright) =>
    level.reduce(
      (n, l) => n + mainLines.upright.filter((u) => u.x > l.x0 + 1 && u.x < l.x1 - 1 && l.y > u.y0 && l.y < u.y1).length,
      0,
    ) + mainLines.level.filter((l) => up.x > l.x0 + 1 && up.x < l.x1 - 1 && l.y > up.y0 && l.y < up.y1).length;
  const under = (level: Level[], own: Box[]) =>
    level.reduce(
      (n, l) =>
        n +
        boxes.filter(
          (b) => !own.includes(b) && l.y > b.y && l.y < b.y + b.height && l.x0 < b.x + b.width && l.x1 > b.x,
        ).length,
      0,
    );

  // The trunks with the shortest runs first, so that they keep nearest the cells.
  const span = (t: Trunk) => {
    const s = nodes.get(cellNode(t.from));
    const ys = t.arrows.map((a) => nodes.get(a.target)?.y ?? 0);
    return s ? Math.max(...ys.map((y) => Math.abs(y - s.y))) : 0;
  };
  type End = { arrow: ChartEdge; box: Box };
  type Choice = { x: number; score: number; yOut: number; yIns: number[] };
  // The height a turn is made at: from `y`, moved away from the cell's edge at `edge` (down from a cell's bottom, up
  // from a cell's top) past every node the level piece from `xa` to `xb` would run under, as long as the upright piece
  // from the edge to it at `at` runs under none.
  const clearLevel = (y: number, xa: number, xb: number, edge: number, at: number, own: Box[]): number => {
    const away = y > edge ? 1 : -1;
    let level = y;
    for (let round = 0; round < 20; round++) {
      const [lo, hi] = [Math.min(xa, xb), Math.max(xa, xb)];
      const hit = boxes.find(
        (b) => !own.includes(b) && level > b.y - TURN && level < b.y + b.height + TURN && lo < b.x + b.width && hi > b.x,
      );
      if (hit === undefined) break;
      const next = away > 0 ? hit.y + hit.height + TURN : hit.y - TURN;
      const [y0, y1] = [Math.min(edge, next), Math.max(edge, next)];
      if (boxes.some((b) => !own.includes(b) && at > b.x - 1 && at < b.x + b.width + 1 && y1 > b.y && y0 < b.y + b.height)) {
        break;
      }
      level = next;
    }
    return level;
  };
  // From where it would leave its cell or come into a cell it runs on either side, going out either way: the corridor
  // that runs under the fewest nodes, then crosses the main path least, then has the shortest level pieces; each end
  // meets its box on the side the corridor stands on, and each turn is made clear of the nodes where it can be.
  const choose = (from: string, source: Box, ends: End[]): Choice | null => {
    const own = [source, ...ends.map((e) => e.box)];
    const portAt = (box: Box, id: string, edge: 'top' | 'bottom', x: number) =>
      portOf(box, facing(box, x), takenAt(id, edge));
    const starts = own.flatMap((b) => [b.x + b.width / 2 + LANE, b.x + b.width / 2 - LANE]);
    let best: Choice | null = null;
    for (const way of [1, -1] as Side[]) {
      for (const start of starts) {
        let yOut = source.y + source.height + OUT;
        let yIns = ends.map((e) => e.box.y - IN);
        const band = () => [Math.min(yOut, ...yIns), Math.max(yOut, ...yIns)] as const;
        let x = corridor(start, ...band(), way);
        let sx = 0;
        let txs: number[] = [];
        for (let round = 0; round < 2; round++) {
          sx = portAt(source, cellNode(from), 'bottom', x);
          txs = ends.map((e) => portAt(e.box, e.arrow.target, 'top', x));
          yOut = clearLevel(source.y + source.height + OUT, sx, x, source.y + source.height, sx, own);
          yIns = ends.map((e, k) => clearLevel(e.box.y - IN, txs[k], x, e.box.y, txs[k], own));
          x = corridor(start, ...band(), way);
        }
        const [y0, y1] = band();
        const level = [
          { y: yOut, x0: Math.min(sx, x), x1: Math.max(sx, x) },
          ...txs.map((tx, k) => ({ y: yIns[k], x0: Math.min(tx, x), x1: Math.max(tx, x) })),
        ];
        const length = level.reduce((n, l) => n + l.x1 - l.x0, 0) + (y1 - y0) / 10;
        const score = under(level, own) * 20000 + crossings(level, { x, y0, y1 }) * 10000 + length;
        if (best === null || score < best.score) best = { x, score, yOut, yIns };
      }
    }
    return best;
  };
  type Placed = { trunk: Trunk; x: number; source: Box; ends: End[]; yOut: number; yIns: Map<string, number> };
  const placed: Placed[] = [];
  const put = (trunk: Trunk, source: Box, ends: End[], c: Choice) => {
    corridors.push({ x: c.x, y0: Math.min(c.yOut, ...c.yIns), y1: Math.max(c.yOut, ...c.yIns) });
    placed.push({ trunk, x: c.x, source, ends, yOut: c.yOut, yIns: new Map(ends.map((e, k) => [e.arrow.id, c.yIns[k]])) });
  };
  for (const trunk of [...trunks].sort((a, b) => span(a) - span(b))) {
    const source = nodes.get(cellNode(trunk.from));
    const ends = trunk.arrows.flatMap((arrow) => {
      const box = nodes.get(arrow.target);
      return box ? [{ arrow, box }] : [];
    });
    if (source === undefined || ends.length === 0) continue;
    const whole = choose(trunk.from, source, ends);
    // A trunk to cells it would each reach best on another side of the cell it leaves is drawn as two, one each side,
    // where that crosses the main path less, or runs under fewer nodes.
    const mid = source.x + source.width / 2;
    const left = ends.filter((e) => (choose(trunk.from, source, [e])?.x ?? mid) < mid);
    const right = ends.filter((e) => !left.includes(e));
    const apart = left.length > 0 && right.length > 0 ? [choose(trunk.from, source, left), choose(trunk.from, source, right)] : null;
    if (apart !== null && apart[0] !== null && apart[1] !== null && (whole === null || apart[0].score + apart[1].score + SPLIT < whole.score)) {
      put(trunk, source, left, apart[0]);
      put(trunk, source, right, apart[1]);
    } else if (whole !== null) {
      put(trunk, source, ends, whole);
    }
  }

  // Where each trunk leaves its cell and each branch comes into its cell, and the height of each turn: on one side of
  // a cell, the trunk whose corridor is nearer leaves nearer the middle and turns further from the cell, and the branch
  // whose corridor is nearer comes in nearer the middle and turns further from the cell.
  const outOf = new Map<Placed, { x: number; y: number }>();
  const into = new Map<string, { x: number; y: number }>();
  const groups = <T>(items: T[], key: (t: T) => string) => {
    const by = new Map<string, T[]>();
    for (const t of items) by.set(key(t), [...(by.get(key(t)) ?? []), t]);
    return [...by.values()];
  };
  for (const group of groups(placed, (p) => `${p.trunk.from}|${facing(p.source, p.x)}`)) {
    const { source } = group[0];
    const side = facing(source, group[0].x);
    const mid = source.x + source.width / 2;
    const inner = [...group].sort((a, b) => Math.abs(a.x - mid) - Math.abs(b.x - mid));
    const taken = [...takenAt(cellNode(group[0].trunk.from), 'bottom')];
    inner.forEach((p, i) => {
      const x = portOf(source, side, taken);
      taken.push(x);
      outOf.set(p, { x, y: p.yOut + (inner.length - 1 - i) * TURN });
    });
  }
  const branches = placed.flatMap((p) => p.ends.map((e) => ({ p, ...e })));
  for (const group of groups(branches, (b) => `${b.arrow.target}|${facing(b.box, b.p.x)}`)) {
    const { box } = group[0];
    const side = facing(box, group[0].p.x);
    const mid = box.x + box.width / 2;
    const inner = [...group].sort((a, b) => Math.abs(a.p.x - mid) - Math.abs(b.p.x - mid));
    const taken = [...takenAt(group[0].arrow.target, 'top')];
    inner.forEach((b, i) => {
      const x = portOf(box, side, taken);
      taken.push(x);
      into.set(b.arrow.id, { x, y: (b.p.yIns.get(b.arrow.id) ?? box.y - IN) - (inner.length - 1 - i) * TURN });
    });
  }

  // Each arrow's route: out of its cell, to its trunk's corridor, along it, and into the cell it runs; straight down
  // where its cell, its corridor and the cell it runs are in line.
  const out = new Map<string, Route>();
  for (const p of placed) {
    const start = outOf.get(p);
    if (start === undefined) continue;
    for (const { arrow, box } of p.ends) {
      const end = into.get(arrow.id);
      if (end === undefined) continue;
      const points: Point[] = [
        { x: start.x, y: p.source.y + p.source.height },
        { x: start.x, y: start.y },
        { x: p.x, y: start.y },
        { x: p.x, y: end.y },
        { x: end.x, y: end.y },
        { x: end.x, y: box.y },
      ];
      out.set(arrow.id, { points: straightened(points), label: null, hops: [] });
    }
  }

  // The labels: the trunk's names once beside it, along what its arrows share, as near the cell it leaves as is clear;
  // what else runs a cell near the end of its branch.
  const kept: Box[] = [...labels];
  const avoid = () => [
    ...boxes,
    ...kept,
    ...mainLines.upright.map((u) => ({ x: u.x - 1, y: u.y0, width: 2, height: u.y1 - u.y0 })),
  ];
  // The first place along a route, a step at a time from `from` towards `to`, where a label is clear of the nodes,
  // the labels and the main path; else the first clear place on the rest of the route; else the middle of the stretch.
  const place = (ps: Point[], from: number, to: number, s: Size, step: number): Box | null => {
    const at = (d: number): Box | null => {
      const p = pointAlong(ps, d);
      return p === null ? null : { x: p.x - s.width / 2, y: p.y - s.height / 2, width: s.width, height: s.height };
    };
    const clear = (b: Box | null) => b !== null && !avoid().some((x) => overlap(b, x, 3));
    const total = lengthOf(ps);
    const ends = step > 0 ? [from, to, total - HEAD_ROOM] : [from, to, 0];
    for (const [a, b] of [
      [ends[0], ends[1]],
      [ends[1], ends[2]],
    ]) {
      for (let d = a; step > 0 ? d <= b : d >= b; d += step) {
        const box = at(d);
        if (clear(box)) return box;
      }
    }
    return at((from + to) / 2);
  };
  const lengthOf = (ps: Point[]) => ps.slice(1).reduce((n, q, i) => n + Math.hypot(q.x - ps[i].x, q.y - ps[i].y), 0);
  const said: DataLabel[] = [];
  for (const p of placed) {
    const routed = p.ends.flatMap((e) => {
      const r = out.get(e.arrow.id);
      return r ? [{ ...e, r }] : [];
    });
    const start = outOf.get(p);
    if (routed.length === 0 || start === undefined) continue;
    // What the arrows share: out of the cell, to the corridor, and along it to where the nearest branch turns off.
    const nearest = routed.reduce((a, b) => (Math.abs(b.box.y - p.source.y) < Math.abs(a.box.y - p.source.y) ? b : a));
    const ps = nearest.r.points;
    const turn = into.get(nearest.arrow.id)?.y ?? start.y;
    const shared =
      routed.length > 1
        ? start.y - (p.source.y + p.source.height) + Math.abs(p.x - start.x) + Math.abs(turn - start.y)
        : lengthOf(ps) - HEAD_ROOM;
    const words = p.trunk.names.map((c) => c.name).join(', ');
    const s = size(words);
    const half = Math.max(s.width, s.height) / 2;
    const box = place(ps, half, shared - half, s, 4);
    if (box !== null) {
      kept.push(box);
      said.push({ id: `d:${p.trunk.from}:${words}:${p.x}`, box, carries: p.trunk.names, arrows: p.ends.map((e) => e.arrow) });
    }
    for (const { arrow, r } of routed) {
      const extra = extrasOf(p.trunk, arrow);
      if (extra.length === 0) continue;
      const w = extra.map((c) => c.name).join(', ');
      const es = size(w);
      const total = lengthOf(r.points);
      const half = Math.max(es.width, es.height) / 2;
      const at = place(r.points, total - HEAD_ROOM - half, half, es, -4);
      if (at !== null) {
        kept.push(at);
        said.push({ id: `d:${arrow.id}:${w}`, box: at, carries: extra, arrows: [arrow] });
      }
    }
  }
  return { routes: out, labels: said };
}

/// The routes of a chart with the arrows drawn over it, each route with the arches where it crosses another.
export function withArrows(routes: Map<string, Route>, drawn: Map<string, Route>): Map<string, Route> {
  const all = new Map<string, Route>();
  for (const [id, r] of routes) all.set(id, { ...r, hops: [] });
  for (const [id, r] of drawn) all.set(id, { ...r, hops: [] });
  addHops(all);
  return all;
}

/// Where what a chosen cell is given stands beside it (a box of `size`): at its right, or else its left, level with
/// its top or its bottom, as near it as covers the fewest nodes, then the fewest lines of `heavy` (the main path and
/// the arrows drawn for the cell), then the fewest other lines, so that it hides nothing drawn where it can help it.
export function besideCell(at: Box, size: Size, boxes: Box[], routes: Map<string, Route>, heavy: Set<string>): Point {
  const pieces = (keep: boolean) => piecesOf([...routes].filter(([id]) => heavy.has(id) === keep).map(([, r]) => r));
  const strong = pieces(true);
  const weak = pieces(false);
  const across = (b: Box, p: { upright: Upright[]; level: Level[] }) =>
    p.upright.filter((u) => u.x > b.x && u.x < b.x + b.width && u.y1 > b.y && u.y0 < b.y + b.height).length +
    p.level.filter((l) => l.y > b.y && l.y < b.y + b.height && l.x1 > b.x && l.x0 < b.x + b.width).length;
  let best: { p: Point; score: number } | null = null;
  for (let d = BESIDE_FIRST; d <= BESIDE_FIRST + 600; d += LANE) {
    for (const right of [true, false]) {
      for (const top of [at.y, at.y + at.height - size.height]) {
        const x = right ? at.x + at.width + d : at.x - d - size.width;
        const b = { x, y: top, width: size.width, height: size.height };
        const nodes = boxes.filter((n) => n !== at && overlap(b, n, 4)).length;
        const score = nodes * 10000 + across(b, strong) * 1000 + across(b, weak) * 40 + d + (right ? 0 : 30);
        if (best === null || score < best.score) best = { p: { x, y: top }, score };
      }
    }
  }
  return best?.p ?? { x: at.x + at.width + BESIDE_FIRST, y: at.y };
}

/// How far from its cell what a chosen cell is given stands at the nearest.
const BESIDE_FIRST = 12;
