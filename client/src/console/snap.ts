// The console's tree in snapping columns, drawn in WebGL by PixiJS. One column for each level, from the roots at the
// left; in each, the level's items one under another, those under one item standing apart from those under the next.
// A faint line across the middle of the screen holds one item of each column: the next column is moved so that the
// items under the held one sit around the line, the middle one of them held; an item with none under it leaves the
// next column's place on the line empty. Scrolling over a column moves its held item one at a time, like a wheel with
// detents: a pull short of the threshold springs back to the line; past it, the next item comes to the line. Holding an
// item of another parent holds that parent in the column to the left. A funnel, filled, joins each item's top and
// bottom to the top of the first item under it and the bottom of the last, light blue from a held item and light grey
// from the others. Every held item glows blue, the one of the column under the pointer orange. Only what is on the
// screen is drawn. What it draws and how fast is kept in `stats`.

import { Application, BitmapText, BlurFilter, Container, Graphics, TextStyle } from 'pixi.js';

import type { Shown } from './items';
import { indexTree, type Tree } from './tree';

const NODE_W = 300;
const NODE_H = 26;
const ROW = 32;
const PITCH = 370;
/// How far, in rows, a scroll must pull a column before the next item comes to the line; and how much of the pull
/// shows while it is short of that.
const DETENT = 0.85;
const GIVE = 0.28;
/// The room between the items of one item and those of the next in a column, in rows.
const APART = 0.7;
/// How far the glow round a held item reaches out from its edge: within half the room between two items.
const GLOW = (ROW - NODE_H) / 2;
/// The map at the top left: each column a bar of this width, this far apart, the map this high, set this far in from
/// the corner and padded this much inside.
const MAP_STRIP = 12;
const MAP_APART = 5;
const MAP_HIGH = 72;
const MAP_INSET = 12;
const MAP_PAD = 8;
/// How quickly a held item's glow turns between blue and orange: the milliseconds in which it goes about two thirds of
/// the way.
const TURN = 35;
/// The part of the way to where it belongs that a column moves each frame.
const SPRING = 0.22;

export type Stats = { items: number; drawn: number; fps: number; meanMs: number; p95Ms: number };

/// The colour a part `k` of the way from `a` to `b`, both `#rrggbb`.
function mix(a: string, b: string, k: number): number {
  const at = (h: string, i: number) => Number.parseInt(h.slice(1 + 2 * i, 3 + 2 * i), 16);
  let out = 0;
  const f = Math.min(1, Math.max(0, k));
  for (let i = 0; i < 3; i += 1) out = out * 256 + Math.round(at(a, i) + (at(b, i) - at(a, i)) * f);
  return out;
}

/// A value of the page's theme, a colour or a font, from its CSS custom property.
function colour(name: string, fallback: string): string {
  const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return v === '' ? fallback : v;
}

/// One column: the item held on the line, by its rank in the column, or none when the place is empty; where the empty
/// place is, before the item of that rank; and where the column is shown, in rows from its first item to the line,
/// moving towards where it belongs, with the scroll's pull not yet past the threshold.
type Column = {
  held: number;
  gapAt: number;
  /// How much of a row the empty place takes now, moving towards 1 while the place is empty and 0 while not.
  gap: number;
  at: number;
  pull: number;
  lastScroll: number;
  /// How far its held item's glow has turned from blue to orange, moving to 1 while the pointer is over the column.
  warm: number;
};

export class SnapScene {
  stats: Stats = { items: 0, drawn: 0, fps: 0, meanMs: 0, p95Ms: 0 };
  private app = new Application();
  private world = new Container();
  private back = new Graphics();
  /// The funnels from each item to the items under it, and the glow round the held items.
  private funnels = new Graphics();
  private glow = new Graphics();
  private layer = new Container();
  /// The map at the top left: its frame, and a bar for each column, drawn each frame, cut to the room inside the frame.
  private map = new Container();
  private mapFrame = new Graphics();
  private mapBars = new Graphics();
  private mapMask = new Graphics();
  /// The map's height of a row: the longest column fits it twice, as a column is held by its middle.
  private mapRow = 1;
  private tree: Tree | null = null;
  /// Each item's number, in the tree's order.
  private ids: number[] = [];
  private views: Container[] = [];
  private columns: Column[] = [];
  /// For each column, each rank's group: how many items above in the column to the left have items here before it.
  private group: Int32Array[] = [];
  private shown = new Set<Container>();
  /// The column under the pointer, or none; and how far the columns are moved right.
  private hovered = -1;
  private left = 40;
  private frames: number[] = [];
  private lastDraw = 0;
  private last = 0;
  private dragging: { x: number } | null = null;
  private ready = false;
  private gone = false;
  private colours = {
    centre: '#d1d9e0',
    mapLine: '#59636e',
    frame: '#ffffff',
    held: '#e4eeff',
    other: '#8d9bb3',
    otherAlpha: 0.12,
    glow: '#3b87ff',
    glowOver: '#f29a1f',
  };

  async mount(host: HTMLElement): Promise<void> {
    await this.app.init({
      resizeTo: host,
      background: colour('--bgColor-default', '#ffffff'),
      antialias: true,
      resolution: window.devicePixelRatio,
      autoDensity: true,
    });
    if (this.gone) {
      this.app.destroy(true, { children: true, texture: true });
      return;
    }
    host.appendChild(this.app.canvas);
    this.glow.filters = [new BlurFilter({ strength: 2, quality: 4 })];
    this.world.addChild(this.back, this.funnels, this.glow, this.layer);
    this.app.stage.addChild(this.world, this.map);
    this.map.position.set(MAP_INSET, MAP_INSET);
    this.map.addChild(this.mapFrame);
    const canvas = this.app.canvas;
    canvas.addEventListener('wheel', this.wheel, { passive: false });
    canvas.addEventListener('pointermove', this.hover);
    canvas.addEventListener('pointerleave', () => (this.hovered = -1));
    canvas.addEventListener('pointerdown', (e) => (this.dragging = { x: e.clientX }));
    window.addEventListener('pointermove', this.drag);
    window.addEventListener('pointerup', this.up);
    this.app.ticker.add(this.tick);
    this.ready = true;
  }

  destroy(): void {
    this.gone = true;
    window.removeEventListener('pointermove', this.drag);
    window.removeEventListener('pointerup', this.up);
    if (this.ready) this.app.destroy(true, { children: true, texture: true });
  }

  /// Draws the tree of `roots` in place of the one before. The items held before stay held where they still are, the
  /// deepest that is first, and the columns move from where they were; with none, the middle root is held.
  show(roots: Shown[]): void {
    const before = new Map<number, number>();
    this.columns.forEach((c, d) => {
      if (c.held >= 0 && this.tree !== null) before.set(d, this.ids[this.tree.columns[d][c.held]]);
    });
    const was = this.columns.map((c) => c.at);
    this.layer.removeChildren();
    for (const v of this.views) v.destroy({ children: true });
    this.shown = new Set();
    // Primer's text, as the rest of the page has it.
    const style = new TextStyle({
      fontFamily: colour('--fontStack-sansSerif', 'sans-serif'),
      fontSize: Number.parseFloat(colour('--text-body-size-medium', '14px')),
      fill: colour('--fgColor-default', '#1f2328'),
    });
    const fill = colour('--bgColor-muted', '#f6f8fa');
    const border = colour('--borderColor-default', '#d1d9e0');
    // The mark at each item's left edge, one grey for all until how a state is shown is settled.
    const mark = colour('--bgColor-neutral-emphasis', '#59636e');
    // A colour with its opacity as `#rrggbbaa`, split for Pixi.
    const neutral = colour('--bgColor-neutral-muted', '#8d9bb31f');
    this.colours = {
      centre: colour('--borderColor-default', '#d1d9e0'),
      mapLine: colour('--fgColor-muted', '#59636e'),
      frame: colour('--bgColor-default', '#ffffff'),
      held: colour('--bgColor-accent-muted', '#e4eeff'),
      other: neutral.slice(0, 7),
      otherAlpha: neutral.length === 9 ? Number.parseInt(neutral.slice(7), 16) / 255 : 0.12,
      glow: colour('--bgColor-accent-emphasis', '#3b87ff'),
      glowOver: colour('--bgColor-attention-emphasis', '#f29a1f'),
    };
    const { tree, items } = indexTree(roots, (i) => i.children);
    this.tree = tree;
    this.ids = items.map((i) => i.id);
    this.views = items.map((item) => {
      const view = new Container();
      const box = new Graphics()
        .roundRect(0, -NODE_H / 2, NODE_W, NODE_H, 6)
        .fill(fill)
        .stroke({ color: border, width: 1 })
        .roundRect(0, -NODE_H / 2, 4, NODE_H, 2)
        .fill(mark);
      const title = new BitmapText({ text: `#${item.id} ${item.title}`, style });
      title.position.set(10, -title.height / 2);
      view.addChild(box, title);
      return view;
    });
    this.columns = tree.columns.map(() => ({ held: -1, gapAt: 0, gap: 0, at: 0, pull: 0, lastScroll: 0, warm: 0 }));
    this.group = tree.columns.map((col) => {
      const g = new Int32Array(col.length);
      for (let r = 1; r < col.length; r += 1) g[r] = g[r - 1] + (tree.parent[col[r]] === tree.parent[col[r - 1]] ? 0 : 1);
      return g;
    });
    if (tree.columns.length === 0) {
      this.buildMap(tree);
      this.stats.items = 0;
      return;
    }
    // The deepest item held before that is still in the tree, held again; else the middle root.
    const index = new Map(this.ids.map((id, i) => [id, i]));
    const kept = [...before].reverse().find(([, id]) => index.has(id));
    if (kept === undefined) this.hold(0, Math.floor((tree.columns[0].length - 1) / 2));
    else {
      const i = index.get(kept[1]) as number;
      this.hold(tree.depth[i], tree.rank[i]);
    }
    // The columns start from where they were drawn, and glide to their places; a new column starts at its place.
    this.columns.forEach((c, d) => (c.at = was[d] ?? this.target(d)));
    this.buildMap(tree);
    this.stats.items = items.length;
  }

  /// Holds the item of rank `rank` in column `d` on the line, the items above it in the columns to its left, and the
  /// middle of those under it in the columns to its right.
  hold(d: number, rank: number): void {
    const t = this.tree;
    if (t === null) return;
    const col = t.columns[d];
    const r = Math.max(0, Math.min(col.length - 1, rank));
    this.setHeld(d, r);
    // Leftwards: each column holds the item above the one held to its right.
    for (let c = d - 1; c >= 0; c -= 1) {
      const below = t.columns[c + 1][this.columns[c + 1].held];
      this.setHeld(c, t.rank[t.parent[below]]);
    }
    // Rightwards: the middle of the items under the one held; none when it has none, the place left empty where
    // its items would be.
    for (let c = d + 1; c < t.columns.length; c += 1) {
      const prev = this.columns[c - 1];
      const above = prev.held >= 0 ? t.columns[c - 1][prev.held] : -1;
      const kids = above >= 0 ? t.children[above] : [];
      if (kids.length > 0) this.setHeld(c, t.rank[kids[Math.floor((kids.length - 1) / 2)]]);
      else {
        // Where the items under the held one would start: after those of the items before it.
        const before = above >= 0 ? t.childStart[above] : this.gapBelow(c - 1);
        this.columns[c].held = -1;
        this.columns[c].gapAt = before;
      }
    }
  }

  /// In a column whose place is empty, where the next column's empty place goes: after the items under those before.
  private gapBelow(d: number): number {
    const t = this.tree as Tree;
    const col = t.columns[d];
    const g = this.columns[d].gapAt;
    return g < col.length ? t.childStart[col[g]] : t.columns[d + 1].length;
  }

  private setHeld(d: number, rank: number) {
    this.columns[d].held = rank;
  }

  /// Where a column belongs: its held item's middle on the line, or the middle of its empty place.
  private target(d: number): number {
    const c = this.columns[d];
    if (c.held >= 0) return this.slot(d, c.held) + 0.5;
    const len = this.group[d].length;
    const above = c.gapAt > 0 ? this.slot(d, c.gapAt - 1) + 1 : this.slot(d, 0) - 1 - APART;
    const below = c.gapAt < len ? this.slot(d, c.gapAt) : above + 1 + APART;
    return (above + below) / 2;
  }

  /// Adds to the funnels the one from an item ending at `px`, its middle at `py`, to items starting at `cx`, the middle
  /// of the first at `first` and of the last at `last`.
  private funnel(px: number, py: number, cx: number, first: number, last: number) {
    const h = NODE_H / 2;
    const bend = (cx - px) / 2;
    this.funnels
      .moveTo(px, py - h)
      .bezierCurveTo(px + bend, py - h, cx - bend, first - h, cx, first - h)
      .lineTo(cx, last + h)
      .bezierCurveTo(cx - bend, last + h, px + bend, py + h, px, py + h)
      .closePath();
  }

  /// The rank of the item of column `d` at `place` rows from its top, the first whose place is not above it.
  private rankAt(d: number, place: number): number {
    let lo = 0;
    let hi = this.group[d].length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (this.slot(d, mid) + 1 < place) lo = mid + 1;
      else hi = mid;
    }
    return lo;
  }

  /// Where an item of a column starts, in rows from the column's top: its rank, the room between the groups above it,
  /// and the empty place as far as it is open.
  private slot(d: number, rank: number): number {
    const c = this.columns[d];
    const g = this.group[d];
    const r = Math.max(0, Math.min(g.length - 1, rank));
    return rank + APART * g[r] + (rank >= c.gapAt ? c.gap : 0);
  }

  /// Draws `frames` frames, each with a scroll of one detent over a column, waiting for the GPU each; gives the time of
  /// each frame in milliseconds.
  bench(frames: number): { meanMs: number; p95Ms: number; maxMs: number; drawn: number } {
    const gl = (this.app.renderer as unknown as { gl?: WebGLRenderingContext }).gl;
    const times: number[] = [];
    for (let k = 0; k < frames; k += 1) {
      const t0 = performance.now();
      if (k % 6 === 0) this.scroll(1 + (k % 4), ROW * 0.7);
      this.tick();
      this.app.renderer.render(this.app.stage);
      gl?.finish();
      times.push(performance.now() - t0);
    }
    times.sort((a, b) => a - b);
    const r = (x: number) => Math.round(x * 100) / 100;
    return {
      meanMs: r(times.reduce((a, b) => a + b, 0) / times.length),
      p95Ms: r(times[Math.floor(times.length * 0.95)]),
      maxMs: r(times[times.length - 1]),
      drawn: this.stats.drawn,
    };
  }

  /// Makes the map's frame, and its height of a row: the longest column fits it twice, as a column is held by its
  /// middle.
  private buildMap(t: Tree) {
    this.map.removeChildren();
    let longest = 1;
    for (let d = 0; d < t.columns.length; d += 1) longest = Math.max(longest, this.slot(d, t.columns[d].length - 1) + 1);
    this.mapRow = (MAP_HIGH - 2 * MAP_PAD) / (2 * longest);
    const wide = 2 * MAP_PAD + t.columns.length * MAP_STRIP + (t.columns.length - 1) * MAP_APART;
    this.mapFrame
      .clear()
      .roundRect(0, 0, wide, MAP_HIGH, 6)
      .fill(this.colours.frame)
      .stroke({ color: this.colours.centre, width: 1 });
    this.mapMask.clear().rect(MAP_PAD / 2, MAP_PAD / 2, wide - MAP_PAD, MAP_HIGH - MAP_PAD).fill(0xffffff);
    this.mapBars.mask = this.mapMask;
    this.map.addChild(this.mapFrame, this.mapMask, this.mapBars);
  }

  /// Draws each column on the map as one bar from its first item to its last, moved as the column is: blue, or orange
  /// as its held item's glow turns while the pointer is over it; and the middle line over them, dark grey.
  private drawMap(t: Tree) {
    const mid = MAP_HIGH / 2;
    this.mapBars.clear();
    this.columns.forEach((c, d) => {
      const from = mid + (0 - c.at - c.pull * GIVE) * this.mapRow;
      const to = mid + (this.slot(d, t.columns[d].length - 1) + 1 - c.at - c.pull * GIVE) * this.mapRow;
      this.mapBars
        .rect(MAP_PAD + d * (MAP_STRIP + MAP_APART), from, MAP_STRIP, Math.max(1, to - from))
        .fill(mix(this.colours.glow, this.colours.glowOver, c.warm));
    });
    const wide = 2 * MAP_PAD + t.columns.length * MAP_STRIP + (t.columns.length - 1) * MAP_APART;
    this.mapBars.moveTo(MAP_PAD / 2, mid).lineTo(wide - MAP_PAD / 2, mid).stroke({ color: this.colours.mapLine, width: 1 });
  }

  /// A scroll of `dy` pixels over column `d`: it pulls the column, and each time the pull passes the threshold the
  /// next item up or down comes to the line.
  scroll(d: number, dy: number): void {
    const t = this.tree;
    if (t === null || d < 0 || d >= this.columns.length) return;
    const c = this.columns[d];
    c.pull += dy / ROW;
    c.lastScroll = performance.now();
    while (Math.abs(c.pull) >= DETENT) {
      const step = Math.sign(c.pull);
      const from = c.held >= 0 ? c.held : step > 0 ? c.gapAt - 1 : c.gapAt;
      const to = from + step;
      if (to < 0 || to >= t.columns[d].length) {
        c.pull = step * (DETENT - 0.01);
        break;
      }
      this.hold(d, to);
      // The pull given back as the item comes to the line is taken up by where the column is shown, so that what is
      // drawn does not move at the step: the column then glides on to its new place.
      c.pull -= step;
      c.at += step * GIVE;
    }
  }

  private wheel = (e: WheelEvent) => {
    e.preventDefault();
    if (Math.abs(e.deltaX) > Math.abs(e.deltaY)) this.left -= e.deltaX;
    else this.scroll(this.hovered, e.deltaY);
  };

  private hover = (e: PointerEvent) => {
    const r = this.app.canvas.getBoundingClientRect();
    const d = Math.floor((e.clientX - r.left - this.left + (PITCH - NODE_W) / 2) / PITCH);
    this.hovered = d >= 0 && d < this.columns.length ? d : -1;
  };

  private drag = (e: PointerEvent) => {
    if (this.dragging === null) return;
    this.left += e.clientX - this.dragging.x;
    this.dragging = { x: e.clientX };
  };

  private up = () => {
    this.dragging = null;
  };

  private tick = () => {
    const now = performance.now();
    if (this.last > 0) this.frames.push(now - this.last);
    this.last = now;
    if (this.frames.length > 120) this.frames.shift();
    if (this.tree !== null) this.draw(this.tree, now);
    const sorted = [...this.frames].sort((a, b) => a - b);
    const mean = sorted.reduce((a, b) => a + b, 0) / Math.max(1, sorted.length);
    this.stats = {
      ...this.stats,
      fps: Math.round(this.app.ticker.FPS),
      meanMs: Math.round(mean * 10) / 10,
      p95Ms: Math.round((sorted[Math.floor(sorted.length * 0.95)] ?? 0) * 10) / 10,
    };
  };

  private draw(t: Tree, now: number) {
    const { width, height } = this.app.screen;
    const line = height / 2;
    // Each column a part of the way to where it belongs; a pull no scroll has added to for a moment springs back.
    const step = this.lastDraw > 0 ? Math.max(0, now - this.lastDraw) : 16;
    this.lastDraw = now;
    this.columns.forEach((c, d) => {
      // The glow turns to orange while the pointer is over the column, and back, in about a tenth of a second.
      c.warm += ((d === this.hovered ? 1 : 0) - c.warm) * (1 - Math.exp(-step / TURN));
      c.gap += ((c.held >= 0 ? 0 : 1) - c.gap) * SPRING;
      // A pull short of the threshold goes back quickly once the scrolling stops: the column is held to the line.
      if (now - c.lastScroll > 70) c.pull *= 0.7;
      if (Math.abs(c.pull) < 0.001) c.pull = 0;
      c.at += (this.target(d) - c.at) * SPRING;
    });
    const y = (d: number, rank: number) => {
      const c = this.columns[d];
      return line + (this.slot(d, rank) + 0.5 - c.at - c.pull * GIVE) * ROW;
    };
    const x = (d: number) => this.left + d * PITCH;
    this.back.clear();
    // The line across the middle, dashed.
    for (let dx = 0; dx < width; dx += 12) this.back.moveTo(dx, line).lineTo(dx + 6, line);
    this.back.stroke({ color: this.colours.centre, width: 1 });
    this.funnels.clear();
    this.glow.clear();
    const heldFunnels: [number, number, number, number, number, number][] = [];
    const nowShown = new Set<Container>();
    const rows = Math.ceil(height / ROW / 2) + 2;
    for (let d = 0; d < t.columns.length; d += 1) {
      const cx = x(d);
      if (cx > width || cx + NODE_W < 0) continue;
      const col = t.columns[d];
      const c = this.columns[d];
      const centre = this.rankAt(d, c.at + c.pull * GIVE);
      for (let rank = Math.max(0, centre - rows); rank < Math.min(col.length, centre + rows); rank += 1) {
        const yy = y(d, rank);
        if (yy < -NODE_H || yy > height + NODE_H) continue;
        const item = col[rank];
        const view = this.views[item];
        view.position.set(cx, yy);
        nowShown.add(view);
        if (!this.shown.has(view)) this.layer.addChild(view);
      }
      // The funnel from each item of the column to the left to the items under it: from its top to the top of the
      // first and from its bottom to the bottom of the last, filled; those from held items drawn last, over the rest.
      if (d === 0) continue;
      const px = x(d - 1) + NODE_W;
      const prev = t.columns[d - 1];
      const heldAbove = this.columns[d - 1].held;
      for (let r = 0; r < prev.length; r += 1) {
        const kids = t.children[prev[r]];
        if (kids.length === 0) continue;
        const py = y(d - 1, r);
        const first = y(d, t.rank[kids[0]]);
        const lastY = y(d, t.rank[kids[kids.length - 1]]);
        if (Math.max(py, lastY) < -NODE_H || Math.min(py, first) > height + NODE_H) continue;
        if (r === heldAbove) heldFunnels.push([px, py, cx, first, lastY, 0]);
        else this.funnel(px, py, cx, first, lastY);
      }
    }
    this.funnels.fill({ color: this.colours.other, alpha: this.colours.otherAlpha });
    for (const [px, py, cx, first, lastY] of heldFunnels) this.funnel(px, py, cx, first, lastY);
    this.funnels.fill(this.colours.held);
    // A glow round each held item: a box a little larger than it, blurred a little, so that it fades out evenly from
    // the edge within the room between it and the items above and below it, where they would hide it.
    for (let d = 0; d < t.columns.length; d += 1) {
      const c = this.columns[d];
      if (c.held < 0) continue;
      const gx = x(d);
      const gy = y(d, c.held);
      if (gx > width || gx + NODE_W < 0 || gy < -NODE_H || gy > height + NODE_H) continue;
      const fill = mix(this.colours.glow, this.colours.glowOver, c.warm);
      this.glow.roundRect(gx - GLOW, gy - NODE_H / 2 - GLOW, NODE_W + 2 * GLOW, NODE_H + 2 * GLOW, 6 + GLOW).fill(fill);
    }
    this.drawMap(t);
    for (const v of this.shown) if (!nowShown.has(v)) this.layer.removeChild(v);
    this.shown = nowShown;
    this.stats.drawn = nowShown.size;
  }
}
