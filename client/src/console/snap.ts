// The console's tree in snapping columns, drawn in WebGL by PixiJS. One column for each level, from the roots at the
// left; in each, the level's items one under another, those under one item standing apart from those under the next.
// A faint line across the middle of the screen holds one item of each column: the next column is moved so that the
// items under the held one sit around the line, the middle one of them held; an item with none under it leaves the
// next column's place on the line empty. Scrolling over a column moves its held item one at a time, like a wheel with
// detents: a pull short of the threshold springs back to the line; past it, the next item comes to the line. Holding an
// item of another parent holds that parent in the column to the left.
//
// Each item is a card; one a work goes on on is a faint tint of the colour of what the work does, a bar of it down its
// left, and one above such an item has an edge of the light accent. A filled funnel joins each item's top and bottom to
// the first item under it and the last, light accent from a held item and grey from the others; a ribbon of the same
// light accent joins each item to each of its items a work goes on on or under, so that the way from a root to every
// item in work reads as one. Each held item is ringed in the accent, or orange while the pointer is over its column, and
// the stage a running work is at pulses. Only what is on the screen is drawn. What it draws and how fast is kept in
// `stats`.

import { Application, BitmapText, Container, Graphics, TextStyle } from 'pixi.js';

import type { Shown, ShownWork } from './items';
import { indexTree, type Tree } from './tree';

const NODE_W = 300;
/// A row's height, and the height of a node of one row: an item with a work going on has a second row, the work's, this
/// part of a row high, so that the work sits close under the title.
const NODE_H = 26;
const ROW = 32;
const WORK_ROW = 0.55;
const PITCH = 370;
/// How far, in rows, a scroll must pull a column before the next item comes to the line; and how much of the pull
/// shows while it is short of that.
const DETENT = 0.85;
const GIVE = 0.28;
/// The room between the items of one item and those of the next in a column, in rows.
const APART = 0.7;
/// The map at the top left: each column a bar of this width, this far apart, the map this high, set this far in from
/// the corner and padded this much inside.
const MAP_STRIP = 12;
const MAP_APART = 5;
const MAP_HIGH = 72;
const MAP_INSET = 12;
const MAP_PAD = 8;
/// How quickly a held item's frame turns between the accent and orange: the milliseconds in which it goes about two
/// thirds of the way.
const TURN = 35;
/// The stage a running work is at pulses: lit most of each beat, it fades out and back in during the last part of it.
/// How long a beat is, in milliseconds; the part of it the fade takes; and how faint the stage goes at its faintest.
const PULSE_MS = 750;
const FADE = 0.4;
const DARK = 0.15;
/// The part of the way to where it belongs that a column moves each frame.
const SPRING = 0.22;

/// The console's colours: words, marks and lines in greys; one accent for what runs and what is held; amber for what
/// waits for the owner; red for what has gone wrong or on long; violet for what has gone on for days.
const PALETTE = {
  ink: '#1f2937',
  muted: '#6b7280',
  faint: '#9aa1ad',
  pale: '#c3c8cf',
  line: '#cdd2d9',
  hair: '#e3e6ea',
  edge: '#e5e7eb',
  card: '#ffffff',
  accent: '#3b5bfd',
  /// A card's edge on the way to a work, and the funnel from a held card and the ribbon to a work.
  branch: '#c7cdfd',
  funnel: '#e8ebff',
  /// The bars of works: running green, as all is going well; waiting for the owner a vivid orange, the one that asks
  /// for them; waiting for something else a pale yellow, which asks nothing of them; paused grey; halted red.
  running: '#2fa66f',
  asked: '#f5840c',
  paused: '#8b93a1',
  halted: '#e5484d',
  waiting: '#e8cc6a',
  amber: '#d97706',
  red: '#e5484d',
  violet: '#8b5cf6',
  green: '#2f9e6e',
  /// A held item's frame while the pointer is over its column.
  over: '#f29a1f',
  /// Where a work's time went: a model's, the tools', idle.
  model: '#4b5563',
  tools: '#9ca3af',
  idle: '#e5e7eb',
};

export type Stats = {
  items: number;
  drawn: number;
  fps: number;
  meanMs: number;
  p95Ms: number;
};

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

/// How long a work has gone on, as a short label and the tone of its age: under half an hour, under an hour, under a
/// day, or longer.
export function age(ms: number): {
  label: string;
  tone: 'success' | 'attention' | 'danger' | 'done';
} {
  const minutes = ms / 60000;
  if (minutes < 60)
    return {
      label: `${Math.max(1, Math.round(minutes))}m`,
      tone: minutes < 30 ? 'success' : 'attention',
    };
  const hours = minutes / 60;
  if (hours < 24) return { label: `${Math.round(hours * 10) / 10}h`, tone: 'danger' };
  return { label: `${Math.round((hours / 24) * 10) / 10}d`, tone: 'done' };
}

/// The colour of a work's age: grey under half an hour, amber under an hour, red under a day, violet longer.
const TONE = {
  success: PALETTE.muted,
  attention: PALETTE.amber,
  danger: PALETTE.red,
  done: PALETTE.violet,
};

/// What a work's state is to its node: running, waiting for the owner, paused, stopped by the engine for a problem, or
/// waiting for something else.
export type Mood = 'running' | 'owner' | 'paused' | 'halted' | 'waiting';

export function moodOf(w: ShownWork): Mood {
  if (w.state === 'running' || w.state === 'ready') return 'running';
  if (w.state === 'halted') return 'halted';
  if (w.state === 'paused') return 'paused';
  if (w.state === 'asking' && (w.waits?.kind === 'answer' || w.waits?.kind === 'ruling')) return 'owner';
  return 'waiting';
}

/// The colour of the bar down the left of an item's card by what its work does; and of the stage it is at.
const BAR: Record<Mood, string> = {
  running: PALETTE.running,
  owner: PALETTE.asked,
  paused: PALETTE.paused,
  halted: PALETTE.halted,
  waiting: PALETTE.waiting,
};
const STAGE: Record<Mood, string> = {
  running: PALETTE.running,
  owner: PALETTE.asked,
  paused: PALETTE.muted,
  halted: PALETTE.red,
  waiting: PALETTE.muted,
};

/// How much of its work's colour a card in work takes, the rest white.
const TINT = 0.09;

/// The colour of an item's number by its state: open, done or cancelled.
const NUMBER = {
  open: PALETTE.faint,
  done: PALETTE.green,
  cancelled: PALETTE.pale,
};

/// A value of the page's theme, a colour or a font, from its CSS custom property.
function colour(name: string, fallback: string): string {
  const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return v === '' ? fallback : v;
}

/// The styles of a node's words, by their colour: its title in Primer's text, and its numbers and letters in Primer's
/// monospace, smaller. Each is made from plain values once: one made by spreading another keeps the other's fill.
type Styles = {
  title: (fill: string) => TextStyle;
  mono: (fill: string) => TextStyle;
};

function makeStyles(): Styles {
  const sans = colour('--fontStack-sansSerif', 'sans-serif');
  const mono = colour('--fontStack-monospace', 'monospace');
  const size = pixels(colour('--text-body-size-medium', '14px'));
  const made = new Map<string, TextStyle>();
  const style = (fontFamily: string, fontSize: number, fill: string) => {
    const key = `${fontFamily}|${fontSize}|${fill}`;
    let s = made.get(key);
    if (s === undefined) {
      s = new TextStyle({ fontFamily, fontSize, fill });
      made.set(key, s);
    }
    return s;
  };
  return {
    title: (fill) => style(sans, size, fill),
    mono: (fill) => style(mono, Math.round(size * 0.8), fill),
  };
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
  /// How far its held item's frame has turned from the accent to orange, moving to 1 while the pointer is over the
  /// column.
  warm: number;
};

export class SnapScene {
  stats: Stats = { items: 0, drawn: 0, fps: 0, meanMs: 0, p95Ms: 0 };
  private app = new Application();
  private world = new Container();
  private back = new Graphics();
  /// The funnels from each item to the items under it, with the ribbons to the items in work over them; and the rings
  /// of the held items.
  private links = new Graphics();
  private marks = new Graphics();
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
  private rows: Float64Array = new Float64Array(0);
  private pre: Float64Array[] = [];
  /// For each item, whether a work goes on on it or under it: the ribbons and the edges of the cards on the way to it
  /// are lit.
  private lit: Uint8Array = new Uint8Array(0);
  /// The parts of a node that change while it is shown: the stage a running work is at, which pulses, and the label of
  /// how long its work has gone on, with when the work started.
  private pulses = new Map<number, Graphics>();
  private ages = new Map<number, { from: number; label: BitmapText; shown: string; row: number }>();
  private styles: Styles | null = null;
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
    other: '#8d9bb3',
    otherAlpha: 0.12,
    ground: '#ffffff',
    mapLine: '#59636e',
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
    this.world.addChild(this.back, this.links, this.layer, this.marks);
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
    // A colour with its opacity as `#rrggbbaa`, split for Pixi.
    const neutral = colour('--bgColor-neutral-muted', '#8d9bb31f');
    const ground = colour('--bgColor-default', '#ffffff');
    this.colours = {
      other: neutral.slice(0, 7),
      otherAlpha: neutral.length === 9 ? Number.parseInt(neutral.slice(7), 16) / 255 : 0.12,
      ground,
      mapLine: colour('--fgColor-muted', '#59636e'),
    };
    this.styles = makeStyles();
    const { tree, items } = indexTree(roots, (i) => i.children);
    this.tree = tree;
    this.ids = items.map((i) => i.id);
    this.rows = Float64Array.from(items, (i) => (i.work !== null ? 1 + WORK_ROW : 1));
    this.pre = tree.columns.map((col) => {
      const p = new Float64Array(col.length + 1);
      for (let r = 0; r < col.length; r += 1) p[r + 1] = p[r] + this.rows[col[r]];
      return p;
    });
    this.asking = tree.columns.map((col) => col.flatMap((i, r) => (items[i].asks > 0 ? [r] : [])));
    // The items a work goes on on, and those above them up to their roots: from the roots one sees which branches
    // have work on.
    this.lit = new Uint8Array(items.length);
    items.forEach((i, k) => {
      if (i.work === null) return;
      for (let p = k; p >= 0 && this.lit[p] === 0; p = tree.parent[p]) this.lit[p] = 1;
    });
    this.pulses = new Map();
    this.ages = new Map();
    this.views = items.map((item, k) => this.node(item, k));
    this.columns = tree.columns.map(() => ({
      held: -1,
      gapAt: 0,
      gap: 0,
      at: 0,
      pull: 0,
      lastScroll: 0,
      warm: 0,
    }));
    this.group = tree.columns.map((col) => {
      const g = new Int32Array(col.length);
      for (let r = 1; r < col.length; r += 1)
        g[r] = g[r - 1] + (tree.parent[col[r]] === tree.parent[col[r - 1]] ? 0 : 1);
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

  /// The node of an item: a card, white, or while a work goes on on it a faint tint of the colour of what the work does,
  /// with a bar of that colour down its left; its edge the accent's tint above a work. On its first row its number in its state's
  /// colour, its title, and at the right, framed, its questions waiting for the owner, red, and its returns sent back,
  /// grey; on the second row, for an item whose work goes on, the work's stages, those gone through grey, the one it is
  /// at in its state's colour and those ahead faint, the letter of the one it is at, where its time went, and how long
  /// it has gone on.
  private node(item: Shown, k: number): Container {
    const s = this.styles as Styles;
    const view = new Container();
    const h = high(this.rows[k]);
    const top = -h / 2;
    const row1 = top + NODE_H / 2;
    const mood = item.work !== null ? moodOf(item.work) : null;
    const card = new Graphics().roundRect(0, top, NODE_W, h, 6);
    view.addChild(
      card
        .fill(mood !== null ? mix(PALETTE.card, BAR[mood], TINT) : PALETTE.card)
        .stroke({ color: this.lit[k] === 1 && mood === null ? PALETTE.branch : PALETTE.edge, width: 1 }),
    );
    if (mood !== null) {
      // The bar, cut to the card's rounded corners.
      const shape = new Graphics().roundRect(0, top, NODE_W, h, 6).fill(0xffffff);
      const bar = new Graphics().rect(0, top, 5, h).fill(BAR[mood]);
      bar.mask = shape;
      view.addChild(shape, bar);
    }
    const x = 10;
    const number = new BitmapText({
      text: String(item.id),
      style: s.mono(NUMBER[item.standing]),
    });
    number.position.set(x, row1 - number.height / 2);
    const title = new BitmapText({
      text: item.title,
      style: s.title(item.standing === 'open' ? PALETTE.ink : PALETTE.faint),
    });
    title.position.set(x + Math.max(26, number.width + 8), row1 - title.height / 2);
    view.addChild(number, title);
    // At the right of the first row, framed: its questions waiting for the owner, and its returns sent back.
    let right = NODE_W - 8;
    const tag = (text: string, fill: string, back: boolean) => {
      const t = new BitmapText({ text, style: s.mono(fill) });
      const pad = 4;
      const icon = back ? 10 : 0;
      const w = t.width + icon + 2 * pad;
      right -= w;
      const g = new Graphics();
      g.roundRect(right, row1 - 8, w, 16, 3).stroke({ color: fill, width: 1 });
      if (back) {
        // A small arrow turning back, drawn: the font may have no such sign.
        const ax = right + pad;
        g.moveTo(ax + 7, row1 - 4)
          .lineTo(ax + 7, row1 + 1)
          .lineTo(ax + 1, row1 + 1)
          .moveTo(ax + 3, row1 - 1)
          .lineTo(ax + 1, row1 + 1)
          .lineTo(ax + 3, row1 + 3)
          .stroke({ color: fill, width: 1.2 });
      }
      t.position.set(right + pad + icon, row1 - t.height / 2);
      view.addChild(g, t);
      right -= 6;
    };
    if (item.asks > 0) tag(String(item.asks), PALETTE.red, false);
    if (item.work !== null && item.work.sentBack > 0) tag(String(item.work.sentBack), PALETTE.muted, true);
    if (item.work === null || mood === null) return view;
    // The second row: the work's stages, each a short bar.
    const w = item.work;
    const row2 = top + h - (WORK_ROW * ROW) / 2 - 3;
    const here = w.stage !== null ? w.stages.indexOf(w.stage) : -1;
    const [cell, thick, gap] = [12, 4, 3];
    const stages = new Graphics();
    w.stages.forEach((_, i) => {
      if (i !== here)
        stages.rect(x + i * (cell + gap), row2 - thick / 2, cell, thick).fill(i < here ? PALETTE.faint : PALETTE.hair);
    });
    view.addChild(stages);
    if (here >= 0) {
      // The one it is at, of its own, so that it can pulse while the work runs.
      const at = new Graphics().rect(x + here * (cell + gap), row2 - thick / 2, cell, thick).fill(STAGE[mood]);
      view.addChild(at);
      if (mood === 'running') this.pulses.set(k, at);
      const letter = new BitmapText({
        text: w.stages[here].charAt(0).toUpperCase(),
        style: s.mono(PALETTE.muted),
      });
      letter.position.set(x + w.stages.length * (cell + gap) + 4, row2 - letter.height / 2);
      view.addChild(letter);
    }
    // Where its time went, on a thin line: a model's, the tools', or idle.
    const barW = 70;
    const barX = NODE_W - 8 - 36 - barW;
    const cells = [...w.bar];
    const kind = { m: PALETTE.model, t: PALETTE.tools, '.': PALETTE.idle };
    const cw = barW / Math.max(1, cells.length);
    const track = new Graphics();
    cells.forEach((c, i) => track.rect(barX + i * cw, row2 - 1, cw, 2).fill(kind[c as 'm' | 't' | '.'] ?? kind['.']));
    // How long it has gone on, at the right, its colour by its age; the label is kept up as time passes.
    const label = new BitmapText({ text: '', style: s.mono(PALETTE.muted) });
    view.addChild(track, label);
    this.ages.set(k, { from: w.from, label, shown: '', row: row2 });
    this.age(k);
    return view;
  }

  /// Puts at the right of an item's second row how long its work has gone on, when the label has changed.
  private age(k: number) {
    const a = this.ages.get(k);
    if (a === undefined || this.styles === null) return;
    const { label, tone } = age(Date.now() - a.from);
    if (label === a.shown) return;
    a.shown = label;
    a.label.text = label;
    a.label.style = this.styles.mono(TONE[tone]);
    a.label.position.set(NODE_W - 8 - a.label.width, a.row - a.label.height / 2);
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
    this.links
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
  bench(frames: number): {
    meanMs: number;
    p95Ms: number;
    maxMs: number;
    drawn: number;
  } {
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
      .fill(this.colours.ground)
      .stroke({ color: PALETTE.line, width: 1 });
    this.mapMask
      .clear()
      .rect(MAP_PAD / 2, MAP_PAD / 2, wide - MAP_PAD, MAP_HIGH - MAP_PAD)
      .fill(0xffffff);
    this.mapBars.mask = this.mapMask;
    this.map.addChild(this.mapFrame, this.mapMask, this.mapBars);
  }

  /// Draws each column on the map as one bar from its first item to its last, moved as the column is: in the accent,
  /// or orange as its held item's frame turns while the pointer is over it, marked red where questions wait for the
  /// owner; and the middle line over them, dark grey.
  private drawMap(t: Tree) {
    const mid = MAP_HIGH / 2;
    this.mapBars.clear();
    this.columns.forEach((c, d) => {
      const from = mid + (0 - c.at - c.pull * GIVE) * this.mapRow;
      const n = t.columns[d].length;
      const to = mid + (this.slot(d, n - 1) + this.rowsAt(d, n - 1) - c.at - c.pull * GIVE) * this.mapRow;
      const left = MAP_PAD + d * (MAP_STRIP + MAP_APART);
      this.mapBars.rect(left, from, MAP_STRIP, Math.max(1, to - from)).fill(mix(PALETTE.accent, PALETTE.over, c.warm));
      // Where questions wait for the owner, red across the bar.
      for (const r of this.asking[d] ?? []) {
        const at = mid + (this.slot(d, r) + this.rowsAt(d, r) / 2 - c.at - c.pull * GIVE) * this.mapRow;
        this.mapBars.rect(left - 1, at - 1, MAP_STRIP + 2, 2).fill(PALETTE.red);
      }
    });
    const wide = 2 * MAP_PAD + t.columns.length * MAP_STRIP + (t.columns.length - 1) * MAP_APART;
    this.mapBars
      .moveTo(MAP_PAD / 2, mid)
      .lineTo(wide - MAP_PAD / 2, mid)
      .stroke({ color: this.colours.mapLine, width: 1 });
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
      // The ring turns to orange while the pointer is over the column, and back, in about a tenth of a second.
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
    // The stage a running work is at: lit, then in the last part of each beat faded out and back in, every running node
    // at once.
    const beat = (now % PULSE_MS) / PULSE_MS;
    const fade = beat < 1 - FADE ? 0 : Math.sin((Math.PI * (beat - (1 - FADE))) / FADE) ** 2;
    const pulse = 1 - (1 - DARK) * fade;
    const x = (d: number) => this.left + d * PITCH;
    this.back.clear();
    // The line across the middle, dashed.
    for (let dx = 0; dx < width; dx += 12) this.back.moveTo(dx, line).lineTo(dx + 6, line);
    this.back.stroke({ color: PALETTE.line, width: 1 });
    this.links.clear();
    this.marks.clear();
    const lit: [number, number, number, number, number, number][] = [];
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
        const p = this.pulses.get(item);
        if (p !== undefined) p.alpha = pulse;
        this.age(item);
        nowShown.add(view);
        if (!this.shown.has(view)) this.layer.addChild(view);
      }
      if (d === 0) continue;
      // The funnel from each item of the column to the left to the items under it: from its top to the top of the
      // first and from its bottom to the bottom of the last, filled; those from held items, and the ribbons from an
      // item to each of its items in work, drawn last, over the rest.
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
        const top = y(d, f) - half(d, f);
        const bottom = y(d, l) + half(d, l);
        if (Math.max(py + ph, bottom) < 0 || Math.min(py - ph, top) > height) continue;
        if (r === heldAbove) lit.push([px, py - ph, py + ph, cx, top, bottom]);
        else this.funnel(px, py - ph, py + ph, cx, top, bottom);
        if (this.lit[prev[r]] === 0) continue;
        for (const kid of kids) {
          if (this.lit[kid] === 0) continue;
          const ky = y(d, t.rank[kid]);
          const kh = half(d, t.rank[kid]);
          if (Math.max(py + ph, ky + kh) < 0 || Math.min(py - ph, ky - kh) > height) continue;
          lit.push([px, py - ph, py + ph, cx, ky - kh, ky + kh]);
        }
      }
    }
    this.links.fill({ color: this.colours.other, alpha: this.colours.otherAlpha });
    for (const [px, top, bottom, cx, f, l] of lit) this.funnel(px, top, bottom, cx, f, l);
    this.links.fill(PALETTE.funnel);
    // Each held item ringed, in the accent or orange.
    for (let d = 0; d < t.columns.length; d += 1) {
      const c = this.columns[d];
      if (c.held < 0) continue;
      const gx = x(d);
      const gy = y(d, c.held);
      const gh = half(d, c.held);
      if (gx > width || gx + NODE_W < 0 || gy < -2 * ROW || gy > height + 2 * ROW) continue;
      const tone = mix(PALETTE.accent, PALETTE.over, c.warm);
      this.marks
        .roundRect(gx - 1.5, gy - gh - 1.5, NODE_W + 3, 2 * gh + 3, 7.5)
        .stroke({ color: tone, width: 3, alpha: 0.18 })
        .roundRect(gx, gy - gh, NODE_W, 2 * gh, 6)
        .stroke({ color: tone, width: 1 });
    }
    this.drawMap(t);
    for (const v of this.shown) if (!nowShown.has(v)) this.layer.removeChild(v);
    this.shown = nowShown;
    this.stats.drawn = nowShown.size;
  }
}
