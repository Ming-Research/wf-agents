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

import { Application, BitmapText, BlurFilter, Container, FillGradient, Graphics, TextStyle } from 'pixi.js';

import type { Shown, ShownWork } from './items';
import { indexTree, type Tree } from './tree';

const NODE_W = 300;
/// A row's height, and the height of a node of one row: an item with a work going on takes two rows, the second the
/// work's.
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
/// How long the light takes to cross a node whose work runs, in milliseconds, and how wide it is.
const SWEEP_MS = 2600;
const SWEEP_W = 90;
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

/// A length of the page's theme in pixels: Primer gives sizes in `rem`, the root's font size each.
function pixels(length: string): number {
  const n = Number.parseFloat(length);
  if (!length.trim().endsWith('rem')) return n;
  return n * Number.parseFloat(getComputedStyle(document.documentElement).fontSize);
}

/// A node's height in pixels, of `rows` rows.
const high = (rows: number) => rows * ROW - (ROW - NODE_H);

/// How long a work has gone on, as a short label and the tone of its age: under half an hour green, under an hour
/// orange, under a day red, longer purple.
export function age(ms: number): { label: string; tone: 'success' | 'attention' | 'danger' | 'done' } {
  const minutes = ms / 60000;
  if (minutes < 60) return { label: `${Math.max(1, Math.round(minutes))}m`, tone: minutes < 30 ? 'success' : 'attention' };
  const hours = minutes / 60;
  if (hours < 24) return { label: `${Math.round(hours * 10) / 10}h`, tone: 'danger' };
  return { label: `${Math.round((hours / 24) * 10) / 10}d`, tone: 'done' };
}

/// The colour a work's state gives its whole node: running, waiting for the owner, paused, stopped by the engine for a
/// problem, or waiting for something else.
export type Mood = 'running' | 'owner' | 'paused' | 'halted' | 'waiting';

export function moodOf(w: ShownWork): Mood {
  if (w.state === 'running' || w.state === 'ready') return 'running';
  if (w.state === 'halted') return 'halted';
  if (w.state === 'paused') return 'paused';
  if (w.state === 'asking' && (w.waits?.kind === 'answer' || w.waits?.kind === 'ruling')) return 'owner';
  return 'waiting';
}

/// A colour of the theme as it shows on the page's ground: one given with its opacity (`#rrggbbaa`) laid over the ground,
/// and more transparent still by `alpha`.
function solid(name: string, alpha = 1): number {
  const v = colour(name, '#f6f8fa');
  const ground = colour('--bgColor-default', '#ffffff');
  const own = v.length === 9 ? Number.parseInt(v.slice(7), 16) / 255 : 1;
  return mix(ground, v.slice(0, 7), own * alpha);
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
  /// Each item's height in rows; for each column, where each of its items starts, its rows counted from the top.
  private rows: Int32Array = new Int32Array(0);
  private pre: Float64Array[] = [];
  /// The parts of a node that change while it is shown: the light crossing a node whose work runs, and the label of
  /// how long its work has gone on, with when the work started.
  private sweeps = new Map<number, Graphics>();
  private ages = new Map<number, { from: number; label: BitmapText; chip: Graphics; shown: string; row: number }>();
  /// For each column, the ranks of its items whose questions wait for the owner, which the map marks.
  private asking: number[][] = [];
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
    asking: '#cf222e',
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
      fontSize: pixels(colour('--text-body-size-medium', '14px')),
      fill: colour('--fgColor-default', '#1f2328'),
    });
    const border = colour('--borderColor-default', '#d1d9e0');
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
      asking: colour('--bgColor-danger-emphasis', '#cf222e'),
    };
    const { tree, items } = indexTree(roots, (i) => i.children);
    this.tree = tree;
    this.ids = items.map((i) => i.id);
    this.rows = Int32Array.from(items, (i) => (i.work !== null ? 2 : 1));
    this.pre = tree.columns.map((col) => {
      const p = new Float64Array(col.length + 1);
      for (let r = 0; r < col.length; r += 1) p[r + 1] = p[r] + this.rows[col[r]];
      return p;
    });
    this.asking = tree.columns.map((col) => col.flatMap((i, r) => (items[i].asks > 0 ? [r] : [])));
    // The items above one whose work runs, up to its root, tinted: from the roots one sees which branch has work on.
    const tinted = new Set<number>();
    items.forEach((i, k) => {
      if (i.work === null || moodOf(i.work) !== 'running') return;
      for (let p = tree.parent[k]; p >= 0; p = tree.parent[p]) tinted.add(p);
    });
    this.sweeps = new Map();
    this.ages = new Map();
    this.views = items.map((item, k) => this.node(item, k, style, border, tinted.has(k)));
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

  /// The node of an item: its whole ground in the colour of its work's state, tinted when a work runs under it, or plain;
  /// on its first row its number on a badge in the colour of the item's state, its title, and at the right its badges,
  /// red for its questions waiting for the owner and grey for its returns sent back; on the second row, for an item
  /// whose work goes on, the work's stages, where its time went, and how long it has gone on.
  private node(item: Shown, k: number, style: TextStyle, border: string, tinted: boolean): Container {
    const view = new Container();
    const h = high(this.rows[k]);
    const top = -h / 2;
    const mood = item.work !== null ? moodOf(item.work) : null;
    const ground: Record<Mood, string> = {
      running: '--bgColor-accent-muted',
      owner: '--bgColor-attention-muted',
      paused: '--bgColor-neutral-muted',
      halted: '--bgColor-danger-muted',
      waiting: '--bgColor-muted',
    };
    const fill = mood !== null ? solid(ground[mood]) : tinted ? solid('--bgColor-accent-muted', 0.45) : solid('--bgColor-muted');
    const box = new Graphics().roundRect(0, top, NODE_W, h, 6).fill(fill).stroke({ color: border, width: 1 });
    view.addChild(box);
    if (mood === 'running') {
      // A light crossing the node, cut to its box.
      const mask = new Graphics().roundRect(0, top, NODE_W, h, 6).fill(0xffffff);
      const light = new FillGradient(0, 0, SWEEP_W, 0);
      light.addColorStop(0, 'rgba(255,255,255,0)');
      light.addColorStop(0.5, 'rgba(255,255,255,0.65)');
      light.addColorStop(1, 'rgba(255,255,255,0)');
      const sweep = new Graphics().rect(0, top, SWEEP_W, h).fill(light);
      sweep.mask = mask;
      view.addChild(mask, sweep);
      this.sweeps.set(k, sweep);
    }
    const small = new TextStyle({ ...style, fontSize: Math.round(style.fontSize as number * 0.8), fill: colour('--fgColor-onEmphasis', '#ffffff') });
    const row1 = top + NODE_H / 2;
    // The number, on a badge in the item's state's colour.
    const standing = { open: '--bgColor-accent-emphasis', done: '--bgColor-success-emphasis', cancelled: '--bgColor-neutral-emphasis' };
    const number = new BitmapText({ text: String(item.id), style: small });
    const badge = new Graphics().roundRect(6, row1 - 9, number.width + 10, 18, 5).fill(colour(standing[item.standing], '#3b87ff'));
    number.position.set(11, row1 - number.height / 2);
    const title = new BitmapText({ text: item.title, style });
    title.position.set(number.width + 24, row1 - title.height / 2);
    view.addChild(badge, number, title);
    // The badges at the right of the first row: questions waiting for the owner, red; returns sent back, grey.
    let right = NODE_W - 6;
    const pill = (text: string, colourName: string, back: boolean) => {
      const t = new BitmapText({ text, style: small });
      const icon = back ? 10 : 0;
      const w = Math.max(18, t.width + 10 + icon);
      right -= w;
      const g = new Graphics().roundRect(right, row1 - 9, w, 18, 9).fill(colour(colourName, '#59636e'));
      if (back) {
        // A small arrow turning back, drawn: the font may have no such sign.
        const ax = right + 5;
        const on = colour('--fgColor-onEmphasis', '#ffffff');
        g.moveTo(ax + 7, row1 - 4)
          .lineTo(ax + 7, row1 + 1)
          .lineTo(ax + 1, row1 + 1)
          .moveTo(ax + 3, row1 - 1)
          .lineTo(ax + 1, row1 + 1)
          .lineTo(ax + 3, row1 + 3)
          .stroke({ color: on, width: 1.4 });
      }
      t.position.set(right + icon + (w - icon - t.width) / 2, row1 - t.height / 2);
      view.addChild(g, t);
      right -= 4;
    };
    if (item.asks > 0) pill(String(item.asks), '--bgColor-danger-emphasis', false);
    if (item.work !== null && item.work.sentBack > 0) pill(String(item.work.sentBack), '--bgColor-neutral-emphasis', true);
    if (item.work === null) return view;
    // The second row: the work's stages, each a letter, those gone through, the one it is at, and those ahead.
    const w = item.work;
    const row2 = top + ROW + NODE_H / 2;
    const here = w.stage !== null ? w.stages.indexOf(w.stage) : -1;
    const waiting = mood !== 'running';
    let x = 8;
    const letter = new TextStyle({ ...style, fontSize: Math.round(style.fontSize as number * 0.7) });
    w.stages.forEach((stage, i) => {
      const look = i < here ? 'done' : i > here || here < 0 ? 'ahead' : waiting ? 'wait' : 'now';
      const bg = { done: '--bgColor-success-muted', now: '--bgColor-accent-emphasis', wait: '--bgColor-attention-emphasis', ahead: '--bgColor-neutral-muted' }[look];
      const fg = { done: '--fgColor-success', now: '--fgColor-onEmphasis', wait: '--fgColor-onEmphasis', ahead: '--fgColor-muted' }[look];
      const cell = new Graphics().roundRect(x, row2 - 8, 15, 16, 3).fill(solid(bg));
      const l = new BitmapText({ text: stage.charAt(0).toUpperCase(), style: new TextStyle({ ...letter, fill: colour(fg, '#1f2328') }) });
      l.position.set(x + (15 - l.width) / 2, row2 - l.height / 2);
      view.addChild(cell, l);
      x += 17;
    });
    // Where its time went: the span cut into cells, a model's, the tools', or idle.
    x += 8;
    const barW = 96;
    const cells = [...w.bar];
    const kind = { m: colour('--chart-1', '#3b87ff'), t: colour('--chart-2', '#f29a1f'), '.': colour('--chart-idle', '#d1d9e0') };
    const track = new Graphics();
    cells.forEach((c, i) => {
      const cw = barW / Math.max(1, cells.length);
      track.rect(x + i * cw, row2 - 3, cw, 6).fill(kind[c as 'm' | 't' | '.'] ?? kind['.']);
    });
    view.addChild(track);
    // How long it has gone on, on a chip at the right, its colour by its age; the label is kept up as time passes.
    const chip = new Graphics();
    const label = new BitmapText({ text: '', style: small });
    view.addChild(chip, label);
    this.ages.set(k, { from: w.from, label, chip, shown: '', row: row2 });
    this.age(k);
    return view;
  }

  /// Puts on an item's chip how long its work has gone on, when the label has changed.
  private age(k: number) {
    const a = this.ages.get(k);
    if (a === undefined) return;
    const { label, tone } = age(Date.now() - a.from);
    if (label === a.shown) return;
    a.shown = label;
    a.label.text = label;
    const w = a.label.width + 12;
    a.chip.clear().roundRect(NODE_W - 6 - w, a.row - 9, w, 18, 9).fill(colour(`--bgColor-${tone}-emphasis`, '#59636e'));
    a.label.position.set(NODE_W - w, a.row - a.label.height / 2);
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

  /// How many rows the item of rank `rank` in column `d` takes.
  private rowsAt(d: number, rank: number): number {
    const col = (this.tree as Tree).columns[d];
    return this.rows[col[Math.max(0, Math.min(col.length - 1, rank))]];
  }

  /// Where a column belongs: its held item's middle on the line, or the middle of its empty place.
  private target(d: number): number {
    const c = this.columns[d];
    if (c.held >= 0) return this.slot(d, c.held) + this.rowsAt(d, c.held) / 2;
    const len = this.group[d].length;
    const above = c.gapAt > 0 ? this.slot(d, c.gapAt - 1) + this.rowsAt(d, c.gapAt - 1) : this.slot(d, 0) - 1 - APART;
    const below = c.gapAt < len ? this.slot(d, c.gapAt) : above + 1 + APART;
    return (above + below) / 2;
  }

  /// Adds to the funnels the one from an item ending at `px`, its box from `top` to `bottom`, to items starting at `cx`,
  /// the first's box starting at `first` and the last's ending at `last`.
  private funnel(px: number, top: number, bottom: number, cx: number, first: number, last: number) {
    const bend = (cx - px) / 2;
    this.funnels
      .moveTo(px, top)
      .bezierCurveTo(px + bend, top, cx - bend, first, cx, first)
      .lineTo(cx, last)
      .bezierCurveTo(cx - bend, last, px + bend, bottom, px, bottom)
      .closePath();
  }

  /// The rank of the item of column `d` at `place` rows from its top, the first whose place is not above it.
  private rankAt(d: number, place: number): number {
    let lo = 0;
    let hi = this.group[d].length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (this.slot(d, mid) + this.rowsAt(d, mid) < place) lo = mid + 1;
      else hi = mid;
    }
    return lo;
  }

  /// Where an item of a column starts, in rows from the column's top: the rows of the items above it, the room between
  /// the groups above it, and the empty place as far as it is open.
  private slot(d: number, rank: number): number {
    const c = this.columns[d];
    const g = this.group[d];
    const r = Math.max(0, Math.min(g.length - 1, rank));
    return this.pre[d][r] + (rank - r) + APART * g[r] + (rank >= c.gapAt ? c.gap : 0);
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
    for (let d = 0; d < t.columns.length; d += 1) {
      const n = t.columns[d].length;
      longest = Math.max(longest, this.slot(d, n - 1) + this.rowsAt(d, n - 1));
    }
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
  /// as its held item's glow turns while the pointer is over it, marked red where questions wait for the owner; and the
  /// middle line over them, dark grey.
  private drawMap(t: Tree) {
    const mid = MAP_HIGH / 2;
    this.mapBars.clear();
    this.columns.forEach((c, d) => {
      const from = mid + (0 - c.at - c.pull * GIVE) * this.mapRow;
      const n = t.columns[d].length;
      const to = mid + (this.slot(d, n - 1) + this.rowsAt(d, n - 1) - c.at - c.pull * GIVE) * this.mapRow;
      const left = MAP_PAD + d * (MAP_STRIP + MAP_APART);
      this.mapBars.rect(left, from, MAP_STRIP, Math.max(1, to - from)).fill(mix(this.colours.glow, this.colours.glowOver, c.warm));
      // Where questions wait for the owner, red across the bar.
      for (const r of this.asking[d] ?? []) {
        const at = mid + (this.slot(d, r) + this.rowsAt(d, r) / 2 - c.at - c.pull * GIVE) * this.mapRow;
        this.mapBars.rect(left - 1, at - 1, MAP_STRIP + 2, 2).fill(this.colours.asking);
      }
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
      return line + (this.slot(d, rank) + this.rowsAt(d, rank) / 2 - c.at - c.pull * GIVE) * ROW;
    };
    const half = (d: number, rank: number) => high(this.rowsAt(d, rank)) / 2;
    const phase = (now % SWEEP_MS) / SWEEP_MS;
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
        if (yy < -2 * ROW || yy > height + 2 * ROW) continue;
        const item = col[rank];
        const view = this.views[item];
        view.position.set(cx, yy);
        // The light crosses every running node at once, so that the screen does not flicker; the age is kept up.
        const sweep = this.sweeps.get(item);
        if (sweep !== undefined) sweep.x = phase * (NODE_W + SWEEP_W) - SWEEP_W;
        this.age(item);
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
        const ph = half(d - 1, r);
        const f = t.rank[kids[0]];
        const l = t.rank[kids[kids.length - 1]];
        const first = y(d, f) - half(d, f);
        const lastY = y(d, l) + half(d, l);
        if (Math.max(py + ph, lastY) < 0 || Math.min(py - ph, first) > height) continue;
        if (r === heldAbove) heldFunnels.push([px, py - ph, py + ph, cx, first, lastY]);
        else this.funnel(px, py - ph, py + ph, cx, first, lastY);
      }
    }
    this.funnels.fill({ color: this.colours.other, alpha: this.colours.otherAlpha });
    for (const [px, top, bottom, cx, first, lastY] of heldFunnels) this.funnel(px, top, bottom, cx, first, lastY);
    this.funnels.fill(this.colours.held);
    // A glow round each held item: a box a little larger than it, blurred a little, so that it fades out evenly from
    // the edge within the room between it and the items above and below it, where they would hide it.
    for (let d = 0; d < t.columns.length; d += 1) {
      const c = this.columns[d];
      if (c.held < 0) continue;
      const gx = x(d);
      const gy = y(d, c.held);
      const gh = half(d, c.held);
      if (gx > width || gx + NODE_W < 0 || gy < -2 * ROW || gy > height + 2 * ROW) continue;
      const fill = mix(this.colours.glow, this.colours.glowOver, c.warm);
      this.glow.roundRect(gx - GLOW, gy - gh - GLOW, NODE_W + 2 * GLOW, 2 * gh + 2 * GLOW, 6 + GLOW).fill(fill);
    }
    this.drawMap(t);
    for (const v of this.shown) if (!nowShown.has(v)) this.layer.removeChild(v);
    this.shown = nowShown;
    this.stats.drawn = nowShown.size;
  }
}
