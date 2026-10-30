// A cell and what the chart draws beside it, laid out together as one node of the chart's layout, the same way for
// every cell: the circles of rows into it from another part in a row above it, each with its row's name on its outer
// side and a line down into it; the rules given to it on its left, the interjection under them, each with a line into
// it; a row from it back to itself as a loop on its right, and what it does every time on its right; and the circles
// of its rows to another part in a row below it, each under the actions on the row, if any. The layout sees each cell
// and what is beside it as one node, and the arrows to and from other nodes meet the cell itself, in a lane in the
// middle of its top and bottom edges, with the circles' lines on either side of it, so that no arrow runs through a
// circle or a label. The lines between a cell and what is beside it are drawn here, straight, with bends only where a
// line reaches the cell from beyond the part of its edge it may meet.

import type { Box, Point, Route, Size } from './flowLayout';
import { type ChartEdge, type ChartNode, cellNode } from './flowModel';

/// A cell and what is beside it, as laid out together.
export type Unit = {
  /// The cell's node.
  cell: string;
  size: Size;
  /// Where each of its nodes is in it, the cell's included.
  boxes: Map<string, Box>;
  /// The lines between them, each with its label.
  links: Map<string, Route>;
  /// Where along the cell's top edge and its bottom edge, from the unit's left, the arrows from other nodes meet it.
  top: { from: number; to: number };
  bottom: { from: number; to: number };
};

/// What is drawn beside a cell: its circles, its rules, its interjection and what it does every time.
const BESIDE = new Set(['in', 'out', 'rule', 'interjection', 'does']);

/// The nodes drawn beside a cell, each with the cell's node: those of a kind drawn beside a cell, and the actions on a
/// row from a cell to a circle of its own; a node with a line to anything else is laid out on its own.
export function attachments(nodes: ChartNode[], edges: ChartEdge[]): Map<string, string> {
  const ids = new Map(nodes.map((n) => [n.id, n]));
  const owner = new Map<string, string>();
  for (const n of nodes) {
    const cell = cellNode(n.cell);
    if (BESIDE.has(n.kind) && ids.get(cell)?.kind === 'cell') owner.set(n.id, cell);
  }
  for (const n of nodes) {
    if (n.kind !== 'action') continue;
    const ins = edges.filter((e) => e.target === n.id);
    const outs = edges.filter((e) => e.source === n.id);
    const from = ins[0]?.source;
    if (from === undefined || ids.get(from)?.kind !== 'cell' || !ins.every((e) => e.source === from)) continue;
    if (outs.length > 0 && outs.every((e) => ids.get(e.target)?.kind === 'out' && owner.get(e.target) === from)) {
      owner.set(n.id, from);
    }
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

/// The least room between the cell and what stands on its left, the room on its right, and the least reach of a row
/// from the cell back to itself.
const LEFT = 24;
const RIGHT = 16;
const LOOP = 26;
/// Between things stacked on the cell's left, and between things side by side in a row above or below it.
const STACK = 8;
const APART = 14;
/// Between a label and the circle or the cell its line leaves.
const LABEL_GAP = 4;
/// The last straight piece of a line into the cell, past its badges and the ring round it; into a circle or the
/// actions on a row; and the first piece of a line out of the cell, past the ring.
const INTO_CELL = 20;
const INTO = 18;
const OUT_OF = 12;
/// How far apart the bends of lines that turn in one band are, and where lines meet an edge side by side.
const STEP = 6;
const SPREAD = 10;
/// How far from the cell's left edge the lines of its circles may meet it: past the effort badge over its top left
/// corner; and how far short of its right edge: above it, short of a work's badges over its top right corner.
const LANE_START = 40;
const CORNER_TOP = 28;
const CORNER_BOTTOM = 10;
/// The least width of the lane, and the room each arrow in it takes.
const LANE = 20;
const PORT = 14;
/// Where the things on the cell's left start when they still fit beside it: under the tag beside its top left corner
/// that says why a work waits there.
const UNDER_TAG = 12;

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

/// A line out of the cell to one of its circles below, with its label, straight or through the actions on the row.
type Slot = { edge: ChartEdge; actions: ChartNode | null; onward: ChartEdge | null; width: number };

/// Lays a cell and what is beside it out together. `members` are the nodes beside it, `links` the lines among them and
/// the cell, and `ports` how many arrows from other nodes meet the cell's top and bottom edges.
export function layUnit(
  cell: ChartNode,
  members: ChartNode[],
  links: ChartEdge[],
  size: (id: string) => Size,
  labelSize: (edge: string) => Size | null,
  ports: { top: number; bottom: number },
): Unit {
  const boxes = new Map<string, Box>();
  const routes = new Map<string, Route>();
  const { width: W, height: H } = size(cell.id);
  boxes.set(cell.id, { x: 0, y: 0, width: W, height: H });
  const of = (kind: ChartNode['kind']) => members.filter((n) => n.kind === kind);
  const into = (id: string) => links.filter((e) => e.target === id);
  const labelAt = (e: ChartEdge, cx: number, top: number): Box | null => {
    const s = labelSize(e.id);
    return s ? { x: cx - s.width / 2, y: top, width: s.width, height: s.height } : null;
  };
  const route = (e: ChartEdge, ps: Point[], label: Box | null) => routes.set(e.id, { points: ps, label, hops: [] });

  // The rules, then the interjection, on the left, their right edges in line and each line level into the cell where
  // its middle is beside the cell, below the tag beside the cell's top left corner where they all fit there too.
  const left = [...of('rule'), ...of('interjection')];
  const heights = left.map((n) => size(n.id).height);
  const tall = heights.reduce((a, h) => a + h, 0) + STACK * Math.max(0, left.length - 1);
  const centres = (top: number) => {
    let y = top;
    return heights.map((h) => {
      const c = y + h / 2;
      y += h + STACK;
      return c;
    });
  };
  const beside = (cs: number[]) => cs.every((c) => c >= 6 && c <= H - 6);
  const leftAt = centres(beside(centres(UNDER_TAG)) ? Math.max(UNDER_TAG, (H - tall) / 2) : (H - tall) / 2);
  const leftLand = landings(leftAt, 6, H - 6, STEP);
  const leftGap = Math.max(LEFT, band(leftAt, leftLand, 6, INTO_CELL - 6));
  const leftLines = bundle(leftAt, leftLand, -leftGap, 0, INTO_CELL - 6);
  left.forEach((n, i) => {
    const s = size(n.id);
    boxes.set(n.id, { x: -leftGap - s.width, y: leftAt[i] - s.height / 2, width: s.width, height: s.height });
    for (const e of links.filter((x) => x.source === n.id)) route(e, points(leftLines[i], false), null);
  });

  // On its right, a row from the cell back to itself, out of its right edge and back in above, its label beside the
  // upright piece; and what the cell does every time, level with its middle, or above the row back to itself.
  const does = of('does');
  const loop = links.find((e) => e.source === cell.id && e.target === cell.id);
  const doesAt = loop !== undefined && does.length > 0 ? Math.min(H / 2, Math.max(8, H / 4)) : H / 2;
  let right = W + RIGHT;
  const sides: Box[] = [];
  if (loop !== undefined) {
    const l = labelSize(loop.id);
    const back = does.length > 0 ? doesAt + 10 : Math.max(6, H / 2 - 8);
    const out = Math.max(back + 8, does.length > 0 ? H - 6 : H / 2 + 8);
    // The label clear of what the cell does every time, as long as it stays beside the upright piece.
    const tallest = Math.max(0, ...does.map((n) => size(n.id).height));
    const clear = does.length > 0 ? doesAt + tallest / 2 + LABEL_GAP : -Infinity;
    const labelY = l ? Math.max((back + out) / 2 - l.height / 2, Math.min(clear, out - l.height / 2)) : 0;
    const label = l ? { x: W + LOOP + LABEL_GAP, y: labelY, ...l } : null;
    route(
      loop,
      [
        { x: W, y: out },
        { x: W + LOOP, y: out },
        { x: W + LOOP, y: back },
        { x: W, y: back },
      ],
      label,
    );
    // What the cell does every time goes past the label only where the two would still be level.
    const level = label !== null && clear > label.y + 0.5;
    right = W + LOOP + (level && label ? LABEL_GAP + label.width : 0) + RIGHT;
    sides.push({ x: W, y: back, width: LOOP, height: out - back }, ...(label ? [label] : []));
  }
  for (const n of does) {
    const s = size(n.id);
    boxes.set(n.id, { x: right, y: doesAt - s.height / 2, width: s.width, height: s.height });
    for (const e of into(n.id)) {
      route(
        e,
        [
          { x: W, y: doesAt },
          { x: right, y: doesAt },
        ],
        null,
      );
    }
  }
  sides.push(...[...boxes].filter(([id]) => id !== cell.id).map(([, b]) => b));

  // The rows: each circle above with its line and the line's label; below, a slot for each line out of the cell to
  // one of its circles, through the actions on the row where it has some, the slots of a circle side by side over it.
  const ins = of('in').map((n) => {
    const edge = links.find((e) => e.source === n.id) ?? null;
    const label = edge ? labelSize(edge.id) : null;
    const circle = size(n.id);
    return { node: n, edge, label, circle, width: circle.width + (label ? LABEL_GAP + label.width : 0) };
  });
  const columns = of('out').map((o) => {
    const slots: Slot[] = [];
    for (const e of into(o.id)) {
      const from = members.find((n) => n.id === e.source);
      if (e.source === cell.id) {
        slots.push({ edge: e, actions: null, onward: null, width: labelSize(e.id)?.width ?? 0 });
      } else if (from?.kind === 'action') {
        for (const first of into(from.id)) {
          const width = Math.max(size(from.id).width, labelSize(first.id)?.width ?? 0);
          slots.push({ edge: first, actions: from, onward: e, width });
        }
      }
    }
    const inner = slots.reduce((a, s) => a + s.width, 0) + APART * Math.max(0, slots.length - 1);
    return { node: o, slots, inner, width: Math.max(size(o.id).width, inner) };
  });
  // Lines and arrows meet the cell's top and bottom away from its corners, where a rounded cell's edge is straight.
  const inset = Math.min(H / 2, W / 4);
  const above = arrange(
    ins.map((x) => x.width),
    W,
    inset,
    ports.top,
    W - Math.max(CORNER_TOP, inset),
    { side: 'left', lead: Math.max(0, ...ins.map((x) => x.circle.width / 2)) },
  );
  const below = arrange(
    columns.map((x) => x.width),
    W,
    inset,
    ports.bottom,
    W - Math.max(CORNER_BOTTOM, inset),
  );
  // Where the lines of a row's things meet the cell: those left of the lane between the badge over its corner, or its
  // corner, and the lane; those right of it between the lane and `end`.
  const meet = (row: Row, at: number[], group: number[], start: number, end: number) => {
    const left = at.filter((_, i) => group[i] < row.split);
    const right = at.filter((_, i) => group[i] >= row.split);
    return [
      ...landings(left, start, row.lane.from - 8, SPREAD),
      ...landings(right, row.lane.to + 8, end, SPREAD),
    ];
  };
  const span = (row: Row, widths: number[]) =>
    row.at.length > 0 ? { from: row.at[0], to: row.at[row.at.length - 1] + widths[widths.length - 1] } : null;

  // Above: the circles in a row on either side of the lane, clear of what is beside the cell, each with its line's
  // label on its outer side and its line straight down where it is over the part of the cell's top it may meet, else
  // bending to it.
  if (ins.length > 0) {
    const outer = (i: number) => i >= above.split;
    const at = ins.map((it, i) =>
      outer(i) ? above.at[i] + it.circle.width / 2 : above.at[i] + it.width - it.circle.width / 2,
    );
    const land = meet(
      above,
      at,
      ins.map((_, i) => i),
      Math.max(LANE_START, inset),
      W - Math.max(CORNER_TOP, inset),
    );
    const circleH = Math.max(...ins.map((it) => it.circle.height));
    const reach = span(
      above,
      ins.map((x) => x.width),
    );
    const under = sides
      .filter((b) => reach !== null && b.x < reach.to && b.x + b.width > reach.from)
      .map((b) => b.y - STACK);
    const bottom = Math.min(-band(at, land, 8, INTO_CELL), ...under);
    const lines = bundle(at, land, bottom, 0, INTO_CELL);
    ins.forEach((it, i) => {
      const s = it.circle;
      const y = bottom - s.height;
      boxes.set(it.node.id, { x: at[i] - s.width / 2, y, width: s.width, height: s.height });
      if (it.edge === null) return;
      const l = it.label;
      const label = l
        ? {
            x: outer(i) ? at[i] + s.width / 2 + LABEL_GAP : at[i] - s.width / 2 - LABEL_GAP - l.width,
            y: bottom - circleH / 2 - l.height / 2,
            ...l,
          }
        : null;
      route(it.edge, points([[at[i], bottom], ...lines[i]], true), label);
    });
  }

  // Below: each slot's line leaves the cell's bottom straight down where it can, else bends to its slot; its label
  // under that; the actions on the row under the label; and each circle under its slots, the lines bending to it.
  if (columns.length > 0) {
    const slotAt: number[] = [];
    const slots: Slot[] = [];
    const group: number[] = [];
    const circleAt: number[] = [];
    columns.forEach((c, ci) => {
      const x = below.at[ci];
      let sx = x + (c.width - c.inner) / 2;
      for (const s of c.slots) {
        slotAt.push(sx + s.width / 2);
        slots.push(s);
        group.push(ci);
        sx += s.width + APART;
      }
      circleAt.push(x + c.width / 2);
    });
    const leave = meet(below, slotAt, group, Math.max(CORNER_BOTTOM, inset), W - Math.max(CORNER_BOTTOM, inset));
    const turned = leave.some((a, i) => Math.abs(a - slotAt[i]) > 0.5);
    const first = turned ? band(leave, slotAt, OUT_OF, 4) : 0;
    const reach = span(
      below,
      columns.map((x) => x.width),
    );
    const over = sides
      .filter((b) => reach !== null && b.x < reach.to && b.x + b.width > reach.from)
      .map((b) => b.y + b.height + STACK);
    const labelTop = Math.max(H + first + LABEL_GAP, ...over);
    const labelH = Math.max(0, ...slots.map((s) => labelSize(s.edge.id)?.height ?? 0));
    const labelBottom = labelTop + labelH;
    const acting = slots.flatMap((s) => (s.actions !== null ? [size(s.actions.id).height] : []));
    const actsTop = labelBottom + INTO;
    const actsBottom = acting.length > 0 ? actsTop + Math.max(...acting) : labelBottom;
    // The lines into each circle meet its top side by side.
    const ends: number[] = [];
    columns.forEach((c, ci) => {
      const n = c.slots.length;
      for (let j = 0; j < n; j++) ends.push(circleAt[ci] + (j - (n - 1) / 2) * SPREAD);
    });
    const circleTop = actsBottom + band(slotAt, ends, 8, INTO);
    const head: Pair[][] = turned ? bundle(leave, slotAt, H, H + first, 4) : slotAt.map((a): Pair[] => [[a, H]]);
    const tail = bundle(slotAt, ends, actsBottom, circleTop, INTO);
    slots.forEach((s, i) => {
      const label = labelAt(s.edge, slotAt[i], labelTop);
      if (s.actions === null) {
        route(s.edge, points([...head[i], [slotAt[i], actsBottom], ...tail[i]], true), label);
        return;
      }
      const a = size(s.actions.id);
      boxes.set(s.actions.id, { x: slotAt[i] - a.width / 2, y: actsTop, width: a.width, height: a.height });
      route(s.edge, points([...head[i], [slotAt[i], actsTop]], true), label);
      if (s.onward !== null) {
        route(s.onward, points([[slotAt[i], actsTop + a.height], [slotAt[i], actsBottom], ...tail[i]], true), null);
      }
    });
    columns.forEach((c, ci) => {
      const s = size(c.node.id);
      boxes.set(c.node.id, { x: circleAt[ci] - s.width / 2, y: circleTop, width: s.width, height: s.height });
    });
  }

  // Everything moved so that the unit starts at its top left corner.
  const all = [...boxes.values(), ...[...routes.values()].flatMap((r) => (r.label ? [r.label] : []))];
  const x0 = Math.min(...all.map((b) => b.x));
  const y0 = Math.min(...all.map((b) => b.y));
  const x1 = Math.max(...all.map((b) => b.x + b.width));
  const y1 = Math.max(...all.map((b) => b.y + b.height));
  const moved = (b: Box): Box => ({ ...b, x: b.x - x0, y: b.y - y0 });
  for (const [id, b] of boxes) boxes.set(id, moved(b));
  for (const [id, r] of routes) {
    routes.set(id, {
      points: r.points.map((p) => ({ x: p.x - x0, y: p.y - y0 })),
      label: r.label ? moved(r.label) : null,
      hops: [],
    });
  }
  return {
    cell: cell.id,
    size: { width: x1 - x0, height: y1 - y0 },
    boxes,
    links: routes,
    top: { from: above.lane.from - x0, to: above.lane.to - x0 },
    bottom: { from: below.lane.from - x0, to: below.lane.to - x0 },
  };
}

/// A part of a line across the cell.
type Span = { from: number; to: number };

/// A row above or below a cell: where each of its things starts, left to right; how many are left of the lane; and
/// the lane, where the arrows from other nodes meet that edge of the cell.
type Row = { at: number[]; split: number; lane: Span };

/// Lays a row of things of `widths` out above or below a cell `W` wide that `ports` arrows meet on that edge, whose
/// lines may meet the cell up to `end`. The lane is in the cell's middle, as wide as its arrows need, the whole width
/// of a cell with no row on that edge. One thing goes on the side `lone` says, on the left only where its line, `lead`
/// in from its inner edge, still meets the cell past the badge over its corner; several go on both sides, as evenly as
/// their widths allow, where the cell leaves room for their lines on both sides, else all on the right.
function arrange(
  widths: number[],
  W: number,
  inset: number,
  ports: number,
  end: number,
  lone: { side: 'left' | 'right'; lead: number } = { side: 'right', lead: 0 },
): Row {
  const k = widths.length;
  if (k === 0) return { at: [], split: 0, lane: { from: inset, to: W - inset } };
  const need = Math.min(Math.max(LANE, PORT * (ports + 1)), W / 2);
  const total = (ws: number[]) => ws.reduce((a, w) => a + w, 0) + APART * Math.max(0, ws.length - 1);
  const lane: Span = { from: W / 2 - need / 2, to: W / 2 + need / 2 };
  const start = Math.max(LANE_START, inset);
  let split = lone.side === 'left' && k === 1 && lane.from - APART - lone.lead >= start ? 1 : 0;
  if (k > 1 && lane.from - 8 - start >= 16 && end - (lane.to + 8) >= 16) {
    let best = Infinity;
    for (let m = 1; m < k; m++) {
      const d = Math.abs(total(widths.slice(0, m)) - total(widths.slice(m)));
      if (d < best) {
        best = d;
        split = m;
      }
    }
  }
  const at: number[] = [];
  let x = lane.from - APART - total(widths.slice(0, split));
  for (let i = 0; i < k; i++) {
    if (i === split) x = lane.to + APART;
    at.push(x);
    x += widths[i] + APART;
  }
  return { at, split, lane };
}
