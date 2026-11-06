// The console's tree in columns, drawn in WebGL by PixiJS. One column for each level, from the roots at the left; in
// each, the level's items one under another, those under one item standing apart from those under the next. Scrolling
// over a column moves it with the scroll, as far as leaves a few of its rows on the screen, and it rests where the
// scroll stops. Each column beside it stands where the most of what joins the two near the middle of the screen is
// level: each item of the one beside it, weighed the more the nearer the middle it is, wants the block of items under
// it, or the item above it, level with it, any part of either doing; the column stays where it is while every want is
// met, and else glides only as far as the weighed wants call for, so that no column leaps. Each column further out
// follows the one beside it the same way.
//
// Each item is a card; one a work goes on on is a faint tint of the colour of what the work does, and one above such an
// item has an edge of the light accent; one new to the owner has a short line of a light accent inside its left edge.
// Under each item that depends on an item besides its main dependent, a shadow of it stands: a white card with a dashed
// edge and grey words, pointing to where the item stands. A filled funnel, grey, joins each item's top and bottom to
// the first item under it and the last. Two items are lit, each with the way from it to its root, and from each of its
// shadows to theirs, by rings and by ribbons joining each to the one above: the item under the pointer in blue, faded
// out once the pointer leaves, and the item selected by a click in purple, until it is let go; a click on a shadow
// selects the item it points to and brings it to the middle of the screen. The stage a running work is at pulses. Every
// column fades out into the page near the top and the bottom of the screen; at those two edges the column under the
// pointer has a line of blue, faded in as the pointer comes. Only what is on the screen is drawn. What it draws and how
// fast is kept in `stats`.

import { Application, BitmapText, Container, FillGradient, Graphics, TextStyle } from 'pixi.js';

import type { Shown, ShownWork } from './items';
import { indexTree, type Tree } from './tree';

const NODE_W = 300;
/// A row's height, and the height of a node of one row: an item with a work going on has a second row, the work's, this
/// part of a row high, so that the work sits close under the title.
const NODE_H = 26;
const ROW = 32;
const WORK_ROW = 0.55;
const PITCH = 370;
/// The room between the items of one item and those of the next in a column, in rows.
const APART = 0.7;
/// The map at the top left: each column a bar of this width, this far apart, the map this high, set this far in from
/// the corner and padded this much inside.
const MAP_STRIP = 12;
const MAP_APART = 5;
const MAP_HIGH = 72;
const MAP_INSET = 12;
const MAP_PAD = 8;
/// How quickly the blue lines at a column's edges come and go as the pointer comes over it and leaves: the
/// milliseconds in which it goes about two thirds of the way.
const TURN = 140;
/// How far in from the top and the bottom of the screen the columns fade out into the page, in pixels; and how thick the
/// blue line is at each edge of the column under the pointer.
const EDGE = 48;
const EDGE_LINE = 6;
/// How quickly the blue rings and ribbons of the item under the pointer come in, and fade out once it leaves: the
/// milliseconds in which each goes about two thirds of the way.
const GLOW_IN = 30;
const GLOW_OUT = 160;
/// The ring round an item lit, the one selected or under the pointer or one above them: its edge's width, and the halo
/// outside it, how wide and how strong, within the room between two items.
const RING = 2;
/// How far in from the side of the room an item brought into view stands.
const INSET = 24;
const HALO = 5;
const HALO_ALPHA = 0.32;
/// The stage a running work is at pulses: lit most of each beat, it fades out and back in during the last part of it.
/// How long a beat is, in milliseconds; the part of it the fade takes; and how faint the stage goes at its faintest.
const PULSE_MS = 750;
const FADE = 0.4;
const DARK = 0.15;
/// The part of the way to where they are going that the columns move sideways each frame, when the scene brings an
/// item into view.
const SNAP = 0.32;
/// How much of the way to where the scroll has taken it the column scrolled goes each frame.
const GLIDE = 0.35;
/// How far from the middle of the screen, in rows, an item's want of the column beside it still weighs: its weight
/// falls off as a bell of this width; and how much a column holds to where it is, against the wants, so that wants that
/// weigh little move it little at a frame, and it never leaps between two places that meet the heavy ones alike.
const REACH = 3;
const STAY = 0.2;
/// How many rows of the column scrolled stay on the screen at least, at its top or its bottom.
const KEEP = 3;
/// How much of the way to where it stands a column beside the one scrolled goes each frame: one that was the one
/// scrolled rests where its own scroll left it, not where its links would put it, and glides there once another is.
const LINK_EASE = 0.2;

/// The console's colours: words, marks and lines in greys; one accent for what runs; amber for what
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
  /// The ground the tree is drawn on: a very light grey, the cards white on it as paper.
  page: '#f6f7f9',
  accent: '#3b5bfd',
  /// A card's edge above a work.
  branch: '#c7cdfd',
  /// The line inside the left edge of a card new to the owner.
  unseen: '#6a82fd',
  /// The ribbons from the item under the pointer up to its root.
  hoverRibbon: '#d3dcfe',
  /// The ring round the item selected and those on its way to its root, and the ribbons between them.
  chosen: '#8b5cf6',
  chosenRibbon: '#e2d7fd',
  /// A column's bar on the map, light, so that the marks across it show.
  mapBar: '#aab6fd',
  /// The colours of works, which tint their cards: running a vivid sky blue, green being kept for what is done; waiting
  /// for the owner a vivid orange, the one that asks for them; waiting for something else a pale yellow, which asks
  /// nothing of them; paused grey; halted red.
  running: '#1a9fff',
  asked: '#f5840c',
  paused: '#8b93a1',
  halted: '#e5484d',
  waiting: '#e8cc6a',
  amber: '#d97706',
  red: '#e5484d',
  violet: '#8b5cf6',
  green: '#2f9e6e',
  /// The item under the pointer, and those above it, ringed; the lines at the edges of the column under it.
  over: '#7891fc',
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

/// Cuts the words of `t` short, ending them with an ellipsis, so that they take no more than `room` pixels.
function fit(t: BitmapText, room: number) {
  if (t.width <= room) return;
  const full = [...t.text];
  let lo = 0;
  let hi = full.length;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    t.text = `${full.slice(0, mid).join('')}…`;
    if (t.width <= room) lo = mid;
    else hi = mid - 1;
  }
  t.text = `${full.slice(0, lo).join('').trimEnd()}…`;
}

/// A gradient from top to bottom in the measure of the shape it fills, from `colour` at opacity `from` to `to`.
function vertical(colour: string, from: number, to: number): FillGradient {
  const rgba = (a: number) => {
    const n = Number.parseInt(colour.slice(1), 16);
    return `rgba(${n >> 16},${(n >> 8) & 255},${n & 255},${a})`;
  };
  return new FillGradient({
    start: { x: 0, y: 0 },
    end: { x: 0, y: 1 },
    textureSpace: 'local',
    colorStops: [
      { offset: 0, color: rgba(from) },
      { offset: 1, color: rgba(to) },
    ],
  });
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

/// The colour of what an item's work does, which tints its card; and of the stage it is at.
const WORK: Record<Mood, string> = {
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

/// How much of its work's colour a card in work takes, the rest white, and its edge, a deeper shade of it.
const TINT = 0.09;
const EDGE_TINT = 0.45;

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

/// One column: where it is shown, in rows from its first item to the middle of the screen.
type Column = {
  at: number;
  /// Where the scroll, or an item brought to the middle, takes the column, which it moves towards while it is the one
  /// the others follow.
  aim: number;
  /// How far the blue lines at its edges have come, moving to 1 while the pointer is over the column.
  warm: number;
};

export class SnapScene {
  stats: Stats = { items: 0, drawn: 0, fps: 0, meanMs: 0, p95Ms: 0 };
  private app = new Application();
  private world = new Container();
  /// The funnels from each item to the items under it; and the rings of the items lit.
  private links = new Graphics();
  private marks = new Graphics();
  private layer = new Container();
  /// The fade of every column into the page at the top and the bottom of the screen, each a gradient in the shape's own
  /// measure, from its top (0) to its bottom (1); and the blue lines there of the column under the pointer.
  private edges = new Graphics();
  private fades = { top: vertical(PALETTE.page, 1, 0), bottom: vertical(PALETTE.page, 0, 1) };
  /// The map at the top left: its frame, and a bar for each column, drawn each frame, cut to the room inside the frame.
  private map = new Container();
  private mapFrame = new Graphics();
  private mapBars = new Graphics();
  private mapMask = new Graphics();
  /// The map's height of a row: the longest column fits it twice, as a column may stand with either end in the middle.
  private mapRow = 1;
  private tree: Tree | null = null;
  /// Each item's number, in the tree's order.
  private ids: number[] = [];
  private views: Container[] = [];
  /// Each item's height in rows; for each column, where each of its items starts, its rows counted from the top.
  private rows: Float64Array = new Float64Array(0);
  private pre: Float64Array[] = [];
  /// For each item, whether a work goes on on it or under it: the edges of the cards above a work are lit.
  private lit: Uint8Array = new Uint8Array(0);
  /// The parts of a node that change while it is shown: the stage a running work is at, which pulses, and the label of
  /// how long its work has gone on, with when the work started.
  private pulses = new Map<number, Graphics>();
  private ages = new Map<number, { from: number; label: BitmapText; shown: string; row: number }>();
  private styles: Styles | null = null;
  /// For each column, the ranks of its items whose questions wait for the owner, and of those new to the owner, which
  /// the map marks.
  private asking: number[][] = [];
  private fresh: number[][] = [];
  private columns: Column[] = [];
  /// For each column, each rank's group: how many items above in the column to the left have items here before it.
  private group: Int32Array[] = [];
  private shown = new Set<Container>();
  /// The column under the pointer, or none; and how far the columns are moved right.
  private hovered = -1;
  /// Where the pointer is over the canvas, or none; the item under it is found each frame, as the columns move.
  private pointer: { x: number; y: number } | null = null;
  /// How far each item ringed in blue, the one under the pointer and those above it, has come in, from 0 to 1; one
  /// the pointer has left stays until it has faded out.
  private glowing = new Map<number, number>();
  /// The same for the glow around the item under the pointer itself, which those lit with it do not have.
  private haloed = new Map<number, number>();
  /// The column last scrolled, or whose item was last brought to the middle, which the others follow.
  private driver = 0;
  private left = 40;
  /// Where the columns are moving sideways to, when the scene brings an item into view, until they are there or the
  /// owner moves them.
  private leftGoal: number | null = null;
  private frames: number[] = [];
  private lastDraw = 0;
  private last = 0;
  private dragging: { x: number } | null = null;
  /// Where the pointer went down, so that a press let go where it began is a click, not a drag.
  private down: { x: number; y: number } | null = null;
  /// The item under the pointer at the last frame, by its index in the tree, or none.
  private under = -1;
  /// Each item's place in the tree, a shadow's left out; and for each item, its shadows: what a shadow points to and
  /// what points to an item.
  private real = new Map<number, number>();
  /// The item selected, by its number, or none: it and the way from it to its root, and from each of its shadows to
  /// theirs, stay lit in purple until it is let go.
  private selected: number | null = null;
  /// Told the item selected each time it changes, or is drawn afresh: the window beside the tree shows it.
  onSelect: ((item: Shown | null) => void) | null = null;
  /// The items as last shown, by their place in the tree.
  private shownItems: Shown[] = [];
  private shadowsOf = new Map<number, number[]>();
  private shadows: Uint8Array = new Uint8Array(0);
  private ready = false;
  private gone = false;
  private colours = {
    other: '#8d9bb3',
    otherAlpha: 0.1,
    ground: '#ffffff',
  };

  async mount(host: HTMLElement): Promise<void> {
    await this.app.init({
      resizeTo: host,
      background: PALETTE.page,
      antialias: true,
      resolution: window.devicePixelRatio,
      autoDensity: true,
    });
    if (this.gone) {
      this.app.destroy(true, { children: true, texture: true });
      return;
    }
    host.appendChild(this.app.canvas);
    this.world.addChild(this.links, this.layer, this.marks);
    this.app.stage.addChild(this.world, this.edges, this.map);
    this.map.position.set(MAP_INSET, MAP_INSET);
    this.map.addChild(this.mapFrame);
    const canvas = this.app.canvas;
    canvas.addEventListener('wheel', this.wheel, { passive: false });
    canvas.addEventListener('pointermove', this.hover);
    canvas.addEventListener('pointerleave', () => {
      this.hovered = -1;
      this.pointer = null;
    });
    canvas.addEventListener('pointerdown', (e) => {
      this.dragging = { x: e.clientX };
      this.down = { x: e.clientX, y: e.clientY };
    });
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

  /// Draws the tree of `roots` in place of the one before. Each column stands as it stood by the item that was nearest
  /// the middle of the screen, where that item is still in it, and else where it was; a column new to the tree stands
  /// where its links put it, and at first the middle root stands in the middle.
  show(roots: Shown[]): void {
    const old = this.tree;
    const before = this.columns.map((c, d) => {
      if (old === null || old.columns[d].length === 0) return null;
      const r = Math.min(old.columns[d].length - 1, this.rankAt(d, c.at));
      return { id: this.ids[old.columns[d][r]], off: c.at - this.middle(d, r) };
    });
    const was = this.columns.map((c) => c.at);
    this.layer.removeChildren();
    for (const v of this.views) v.destroy({ children: true });
    this.shown = new Set();
    // A colour with its opacity as `#rrggbbaa`, split for Pixi; the funnels take it a little lighter.
    const neutral = colour('--bgColor-neutral-muted', '#8d9bb31f');
    const ground = PALETTE.page;
    this.colours = {
      other: neutral.slice(0, 7),
      otherAlpha: 0.85 * (neutral.length === 9 ? Number.parseInt(neutral.slice(7), 16) / 255 : 0.12),
      ground,
    };
    this.styles = makeStyles();
    const { tree, items } = indexTree(roots, (i) => i.children);
    this.tree = tree;
    this.ids = items.map((i) => i.id);
    this.shownItems = items;
    this.shadows = Uint8Array.from(items, (i) => (i.shadow ? 1 : 0));
    this.real = new Map();
    this.shadowsOf = new Map();
    items.forEach((it, k) => {
      if (!it.shadow) this.real.set(it.id, k);
      else this.shadowsOf.set(it.id, [...(this.shadowsOf.get(it.id) ?? []), k]);
    });
    this.under = -1;
    this.rows = Float64Array.from(items, (i) => (i.work !== null ? 1 + WORK_ROW : 1));
    this.pre = tree.columns.map((col) => {
      const p = new Float64Array(col.length + 1);
      for (let r = 0; r < col.length; r += 1) p[r + 1] = p[r] + this.rows[col[r]];
      return p;
    });
    this.asking = tree.columns.map((col) => col.flatMap((i, r) => (items[i].asks > 0 ? [r] : [])));
    this.fresh = tree.columns.map((col) => col.flatMap((i, r) => (items[i].unseen ? [r] : [])));
    // The items a work goes on on, and those above them up to their roots: from the roots one sees which branches
    // have work on.
    this.lit = new Uint8Array(items.length);
    items.forEach((i, k) => {
      if (i.work === null) return;
      for (let p = k; p >= 0 && this.lit[p] === 0; p = tree.parent[p]) this.lit[p] = 1;
    });
    this.pulses = new Map();
    this.glowing = new Map();
    this.haloed = new Map();
    this.ages = new Map();
    this.views = items.map((item, k) => this.node(item, k));
    this.columns = tree.columns.map(() => ({ at: 0, aim: 0, warm: 0 }));
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
    const index = new Map(items.flatMap((it, i) => (it.shadow ? [] : [[it.id, i] as const])));
    const fresh = new Set<number>();
    this.columns.forEach((c, d) => {
      const b = before[d];
      const i = b === null || b === undefined ? undefined : index.get(b.id);
      if (b !== null && b !== undefined && i !== undefined && tree.depth[i] === d)
        c.at = this.middle(d, tree.rank[i]) + b.off;
      else if (was[d] !== undefined) c.at = was[d];
      else fresh.add(d);
    });
    if (old === null) {
      this.driver = 0;
      this.columns[0].at = this.middle(0, Math.floor((tree.columns[0].length - 1) / 2));
      fresh.delete(0);
    }
    this.driver = Math.min(this.driver, tree.columns.length - 1);
    this.columns[this.driver].aim = this.columns[this.driver].at;
    // The new columns where their links put them, at once.
    for (let k = 0; k < 100 && fresh.size > 0; k += 1) {
      for (let d = this.driver + 1; d < this.columns.length; d += 1) if (fresh.has(d)) this.link(d, d - 1);
      for (let d = this.driver - 1; d >= 0; d -= 1) if (fresh.has(d)) this.link(d, d + 1);
    }
    this.buildMap(tree);
    this.stats.items = items.length;
    if (this.selected !== null && !this.real.has(this.selected)) this.selected = null;
    this.announce();
  }

  /// The node of an item: a card, white, or while a work goes on on it a faint tint of the colour of what the work does
  /// with an edge of a deeper shade of it; its edge the accent's tint above a work; and while the item is new to the owner, a short line of a light accent
  /// inside its left edge. On its first row its number in its state's colour, its title, and at the right its questions
  /// waiting for the owner, white on red, its messages unread that hold nothing up, white on orange, and its returns
  /// sent back, grey; on the second row, for an item whose work
  /// goes on, the work's stages, those gone through grey, the one it is at in its state's colour and those ahead faint,
  /// the letter of the one it is at, where its time went, and how long it has gone on.
  private node(item: Shown, k: number): Container {
    const s = this.styles as Styles;
    const view = new Container();
    const h = high(this.rows[k]);
    const top = -h / 2;
    const row1 = top + NODE_H / 2;
    if (item.shadow) return this.shadow(item, view, top, h, row1);
    const mood = item.work !== null ? moodOf(item.work) : null;
    const card = new Graphics().roundRect(0, top, NODE_W, h, 6);
    view.addChild(
      card.fill(mood !== null ? mix(PALETTE.card, WORK[mood], TINT) : PALETTE.card).stroke({
        color:
          mood !== null ? mix(PALETTE.card, WORK[mood], EDGE_TINT) : this.lit[k] === 1 ? PALETTE.branch : PALETTE.edge,
        width: 1,
      }),
    );
    // While the item is new to the owner, a line inside the card's left edge, a little in from it and short of its top
    // and bottom; the room for it kept on every card, so that the numbers and titles stand in line.
    if (item.unseen) view.addChild(new Graphics().roundRect(4, top + 5, 3, h - 10, 1.5).fill(PALETTE.unseen));
    const x = 14;
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
    // At the right of the first row: its questions waiting for the owner, white on red, the strongest mark a card has,
    // as what the owner looks for first; its messages unread that hold nothing up, white on orange; its returns sent
    // back, framed in grey; and, while it has not started, a lock framed in grey with how many items it waits for to
    // merge, which stand under it.
    let right = NODE_W - 8;
    const tag = (text: string, fill: string, sign: 'back' | 'lock' | null, solid: boolean) => {
      const t = new BitmapText({ text, style: s.mono(solid ? '#ffffff' : fill) });
      const pad = 4;
      const icon = sign !== null ? 10 : 0;
      const w = t.width + icon + 2 * pad;
      right -= w;
      const g = new Graphics();
      if (solid) g.roundRect(right, row1 - 8, w, 16, 3).fill(fill);
      else g.roundRect(right, row1 - 8, w, 16, 3).stroke({ color: fill, width: 1 });
      const ax = right + pad;
      if (sign === 'lock') {
        // A small padlock, drawn: its body, and the shackle over it.
        g.roundRect(ax + 1, row1 - 1, 7, 5.5, 1)
          .stroke({ color: fill, width: 1.2 })
          .moveTo(ax + 2.5, row1 - 1)
          .lineTo(ax + 2.5, row1 - 3)
          .arc(ax + 4.5, row1 - 3, 2, Math.PI, 0)
          .lineTo(ax + 6.5, row1 - 1)
          .stroke({ color: fill, width: 1.2 });
      }
      if (sign === 'back') {
        // A small arrow turning back, drawn: the font may have no such sign.
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
    if (item.asks > 0) tag(String(item.asks), PALETTE.red, null, true);
    if (item.unread > 0) tag(String(item.unread), PALETTE.asked, null, true);
    if (item.work !== null && item.work.sentBack > 0) tag(String(item.work.sentBack), PALETTE.muted, 'back', false);
    if (item.work === null && item.blocked > 0) tag(String(item.blocked), PALETTE.muted, 'lock', false);
    // The title ends where the badges begin.
    fit(title, right - title.x);
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

  /// The shadow of an item under another item that depends on it besides its main one: a white card, its edge dashed,
  /// its number and title grey, and at the right an arrow pointing away, to where the item stands.
  private shadow(item: Shown, view: Container, top: number, h: number, row1: number): Container {
    const s = this.styles as Styles;
    const ground = new Graphics().roundRect(0, top, NODE_W, h, 6).fill(PALETTE.card);
    const edge = new Graphics();
    const [dash, gap, r] = [4, 3, 6];
    // Dashes along each straight side, the rounded corners left open.
    const along = (x0: number, y0: number, x1: number, y1: number) => {
      const len = Math.hypot(x1 - x0, y1 - y0);
      for (let at = 0; at < len; at += dash + gap) {
        const b = Math.min(len, at + dash);
        edge.moveTo(x0 + ((x1 - x0) * at) / len, y0 + ((y1 - y0) * at) / len);
        edge.lineTo(x0 + ((x1 - x0) * b) / len, y0 + ((y1 - y0) * b) / len);
      }
    };
    along(r, top, NODE_W - r, top);
    along(r, top + h, NODE_W - r, top + h);
    along(0, top + r, 0, top + h - r);
    along(NODE_W, top + r, NODE_W, top + h - r);
    edge.stroke({ color: PALETTE.pale, width: 1 });
    const number = new BitmapText({ text: String(item.id), style: s.mono(PALETTE.pale) });
    number.position.set(14, row1 - number.height / 2);
    const title = new BitmapText({ text: item.title, style: s.title(PALETTE.faint) });
    title.position.set(14 + Math.max(26, number.width + 8), row1 - title.height / 2);
    fit(title, NODE_W - 24 - title.x);
    // The arrow, drawn: the font may have no such sign.
    const ax = NODE_W - 16;
    const arrow = new Graphics()
      .moveTo(ax, row1 + 4)
      .lineTo(ax + 8, row1 - 4)
      .moveTo(ax + 3, row1 - 4)
      .lineTo(ax + 8, row1 - 4)
      .lineTo(ax + 8, row1 + 1)
      .stroke({ color: PALETTE.faint, width: 1.3 });
    view.addChild(ground, edge, number, title, arrow);
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

  /// Brings the item of rank `rank` in column `d` to the middle of the screen, its column the one the others follow.
  private focus(d: number, rank: number): void {
    const t = this.tree;
    if (t === null) return;
    this.driver = d;
    this.columns[d].aim = this.middle(d, Math.max(0, Math.min(t.columns[d].length - 1, rank)));
  }

  /// How many rows the item of rank `rank` in column `d` takes.
  private rowsAt(d: number, rank: number): number {
    const col = (this.tree as Tree).columns[d];
    return this.rows[col[Math.max(0, Math.min(col.length - 1, rank))]];
  }

  /// The middle of the item of rank `rank` in column `d`, in rows from the column's top.
  private middle(d: number, rank: number): number {
    return this.slot(d, rank) + this.rowsAt(d, rank) / 2;
  }

  /// Moves column `c` towards where it stands by its links with the column `from` beside it, which it follows. Each
  /// item of `from` near the middle of the screen wants what it is joined to in `c` beside it: the block of the items
  /// under it, or the item above it, any part of which level with it will do; it weighs the more the nearer the line it
  /// is. The column stays where it is while every want is met, and else goes where the wants not met, weighed, pull it
  /// no further either way against its hold to where it is, which it takes again each frame. With nothing joining them
  /// near the middle, it stays.
  private link(c: number, from: number): void {
    const t = this.tree as Tree;
    const src = this.columns[from];
    const me = this.columns[c];
    const col = t.columns[from];
    const line = src.at;
    // Where each want is met, as the places of the column `c` at the line, from `lows` to `highs`, and its weight.
    const lows: number[] = [];
    const highs: number[] = [];
    const weights: number[] = [];
    const last = Math.min(col.length - 1, this.rankAt(from, line + 3 * REACH));
    for (let r = this.rankAt(from, line - 3 * REACH); r <= last; r += 1) {
      const s = this.middle(from, r) - line;
      const item = col[r];
      let top: number;
      let bottom: number;
      if (c > from) {
        const kids = t.children[item];
        if (kids.length === 0) continue;
        const first = t.rank[kids[0]];
        const end = t.rank[kids[kids.length - 1]];
        top = this.slot(c, first);
        bottom = this.slot(c, end) + this.rowsAt(c, end);
      } else {
        const above = t.parent[item];
        if (above < 0) continue;
        top = this.slot(c, t.rank[above]);
        bottom = top + this.rowsAt(c, t.rank[above]);
      }
      lows.push(top - s);
      highs.push(bottom - s);
      weights.push(Math.exp(-(s * s) / (2 * REACH * REACH)));
    }
    if (weights.length === 0) return;
    const now = me.at;
    // How hard the wants not met, and its hold to where it is, pull the column below `x`, those above it counted
    // against: it grows with `x`.
    const pull = (x: number) => {
      let g = STAY * (x - now);
      for (let i = 0; i < weights.length; i += 1) {
        if (x < lows[i]) g += weights[i] * (x - lows[i]);
        else if (x > highs[i]) g += weights[i] * (x - highs[i]);
      }
      return g;
    };
    const g = pull(now);
    let to = now;
    if (Math.abs(g) > 1e-9) {
      let lo = g > 0 ? Math.min(now, ...lows) : now;
      let hi = g > 0 ? now : Math.max(now, ...highs);
      for (let i = 0; i < 40; i += 1) {
        const mid = (lo + hi) / 2;
        if (pull(mid) < 0) lo = mid;
        else hi = mid;
      }
      to = (lo + hi) / 2;
    }
    me.at += (to - me.at) * LINK_EASE;
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

  /// Where an item of a column starts, in rows from the column's top: the rows of the items above it, and the room
  /// between the groups above it.
  private slot(d: number, rank: number): number {
    const g = this.group[d];
    const r = Math.max(0, Math.min(g.length - 1, rank));
    return this.pre[d][r] + (rank - r) + APART * g[r];
  }

  /// Draws `frames` frames, every sixth with a scroll of most of a row over a column, waiting for the GPU each; gives
  /// the time of each frame in milliseconds.
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

  /// Makes the map's frame, and its height of a row: the longest column fits it twice, as a column may stand with
  /// either end in the middle.
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

  /// Draws each column on the map as one bar from its first item to its last, moved as the column is: in a light accent,
  /// or a deeper blue while the pointer is over it, marked in the accent where items are new to the owner, red where questions
  /// wait for them, and purple where the item selected stands.
  private drawMap(t: Tree) {
    const mid = MAP_HIGH / 2;
    this.mapBars.clear();
    this.columns.forEach((c, d) => {
      const from = mid + (0 - c.at) * this.mapRow;
      const n = t.columns[d].length;
      const to = mid + (this.slot(d, n - 1) + this.rowsAt(d, n - 1) - c.at) * this.mapRow;
      const left = MAP_PAD + d * (MAP_STRIP + MAP_APART);
      this.mapBars.rect(left, from, MAP_STRIP, Math.max(1, to - from)).fill(mix(PALETTE.mapBar, PALETTE.over, c.warm));
      // Where items are new to the owner, the accent across the bar; where questions wait for the owner, red, over it.
      for (const r of this.fresh[d] ?? []) {
        const at = mid + (this.slot(d, r) + this.rowsAt(d, r) / 2 - c.at) * this.mapRow;
        this.mapBars.rect(left - 1, at - 1, MAP_STRIP + 2, 2).fill(PALETTE.accent);
      }
      for (const r of this.asking[d] ?? []) {
        const at = mid + (this.slot(d, r) + this.rowsAt(d, r) / 2 - c.at) * this.mapRow;
        this.mapBars.rect(left - 1, at - 1, MAP_STRIP + 2, 2).fill(PALETTE.red);
      }
    });
    // Where the item selected stands, purple across its column's bar, over every other mark, a little wider, so that
    // it is found again from anywhere.
    const chosen = this.selected === null ? undefined : this.real.get(this.selected);
    if (chosen !== undefined) {
      const [d, r] = [t.depth[chosen], t.rank[chosen]];
      const c = this.columns[d];
      const at = mid + (this.slot(d, r) + this.rowsAt(d, r) / 2 - c.at) * this.mapRow;
      this.mapBars.rect(MAP_PAD + d * (MAP_STRIP + MAP_APART) - 2, at - 1.5, MAP_STRIP + 4, 3).fill(PALETTE.chosen);
    }
  }

  /// A scroll of `dy` pixels over column `d`: it moves the column, which the others then follow, as far as leaves
  /// `KEEP` rows of it on the screen at its top or its bottom.
  scroll(d: number, dy: number): void {
    const t = this.tree;
    if (t === null || d < 0 || d >= this.columns.length) return;
    const c = this.columns[d];
    if (d !== this.driver) {
      this.driver = d;
      c.aim = c.at;
    }
    const room = Math.max(0, this.app.screen.height / 2 / ROW - KEEP);
    const last = t.columns[d].length - 1;
    c.aim = Math.max(this.middle(d, 0) - room, Math.min(this.middle(d, last) + room, c.aim + dy / ROW));
  }

  private wheel = (e: WheelEvent) => {
    e.preventDefault();
    if (Math.abs(e.deltaX) > Math.abs(e.deltaY)) {
      this.leftGoal = null;
      this.left -= e.deltaX;
    } else this.scroll(this.hovered, e.deltaY);
  };

  private hover = (e: PointerEvent) => {
    const r = this.app.canvas.getBoundingClientRect();
    const d = Math.floor((e.clientX - r.left - this.left + (PITCH - NODE_W) / 2) / PITCH);
    this.hovered = d >= 0 && d < this.columns.length ? d : -1;
    this.pointer = { x: e.clientX - r.left, y: e.clientY - r.top };
  };

  /// The item under the pointer, by its index in the tree, or none: the card of column `d` whose box holds the
  /// pointer. `y` gives an item's middle on the screen.
  private pointed(t: Tree, x: (d: number) => number, y: (d: number, rank: number) => number): number {
    const p = this.pointer;
    const d = this.hovered;
    if (p === null || d < 0 || p.x < x(d) || p.x > x(d) + NODE_W) return -1;
    const c = this.columns[d];
    const line = this.app.screen.height / 2;
    const rank = this.rankAt(d, (p.y - line) / ROW + c.at);
    if (rank >= t.columns[d].length) return -1;
    const h = high(this.rowsAt(d, rank)) / 2;
    return Math.abs(p.y - y(d, rank)) <= h ? t.columns[d][rank] : -1;
  }

  private drag = (e: PointerEvent) => {
    if (this.dragging === null) return;
    this.leftGoal = null;
    this.left += e.clientX - this.dragging.x;
    this.dragging = { x: e.clientX };
  };

  private up = (e: PointerEvent) => {
    this.dragging = null;
    const d = this.down;
    this.down = null;
    if (d === null || Math.hypot(e.clientX - d.x, e.clientY - d.y) > 4 || this.tree === null) return;
    // A click on an item selects it, and on it again, or on no item, lets it go. A click on a shadow selects the item
    // it points to and brings it to the middle of the screen, so that the columns come to where it stands.
    const i = this.under;
    if (i < 0) this.selected = null;
    else if (this.shadows[i] === 1) {
      const to = this.real.get(this.ids[i]);
      if (to === undefined) return;
      this.selected = this.ids[to];
      this.focus(this.tree.depth[to], this.tree.rank[to]);
    } else this.selected = this.selected === this.ids[i] ? null : this.ids[i];
    this.announce();
  };

  /// Brings the item numbered `id` into view, within the `room` pixels of the screen's width from its left that the
  /// window beside the tree leaves: to the middle of the screen if it is above or below what is seen, and, if its
  /// column is not wholly within the room, the columns moved sideways only until it is, a little in from the side it
  /// was out at. An item already in view stays where it is.
  reveal(id: number, room: number): void {
    const t = this.tree;
    const i = this.real.get(id);
    if (t === null || i === undefined) return;
    const d = t.depth[i];
    const rank = t.rank[i];
    const c = this.columns[d];
    const height = this.app.screen.height;
    const y = height / 2 + (this.middle(d, rank) - c.at) * ROW;
    const h = high(this.rowsAt(d, rank)) / 2;
    const x = this.left + d * PITCH;
    if (y - h < EDGE || y + h > height - EDGE) this.focus(d, rank);
    if (x < 0) this.leftGoal = INSET - d * PITCH;
    else if (x + NODE_W > room) this.leftGoal = room - INSET - NODE_W - d * PITCH;
  }

  /// Selects the item numbered `id`, or none.
  select(id: number | null): void {
    this.selected = id !== null && this.real.has(id) ? id : null;
    this.announce();
  }

  /// Tells `onSelect` the item selected, as last shown, or none.
  private announce() {
    const at = this.selected === null ? undefined : this.real.get(this.selected);
    this.onSelect?.(at === undefined ? null : this.shownItems[at]);
  }

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
    // The column followed goes a part of the way to where it is taken; the others after it, out from it each way.
    const step = this.lastDraw > 0 ? Math.max(0, now - this.lastDraw) : 16;
    this.lastDraw = now;
    this.columns.forEach((c, d) => {
      // The blue lines at the column's edges fade in while the pointer is over it, and out once it leaves, in about
      // half a second.
      c.warm += ((d === this.hovered ? 1 : 0) - c.warm) * (1 - Math.exp(-step / TURN));
      if (d === this.driver) c.at += (c.aim - c.at) * GLIDE;
    });
    for (let d = this.driver + 1; d < this.columns.length; d += 1) this.link(d, d - 1);
    for (let d = this.driver - 1; d >= 0; d -= 1) this.link(d, d + 1);
    const y = (d: number, rank: number) => {
      const c = this.columns[d];
      return line + (this.middle(d, rank) - c.at) * ROW;
    };
    const half = (d: number, rank: number) => high(this.rowsAt(d, rank)) / 2;
    // The stage a running work is at: lit, then in the last part of each beat faded out and back in, every running node
    // at once.
    const beat = (now % PULSE_MS) / PULSE_MS;
    const fade = beat < 1 - FADE ? 0 : Math.sin((Math.PI * (beat - (1 - FADE))) / FADE) ** 2;
    const pulse = 1 - (1 - DARK) * fade;
    // Moved sideways no further than leaves half of the first column on the screen at its right, or half of the last at
    // its left, so that one always sees where the tree ends.
    const last = Math.max(0, t.columns.length - 1) * PITCH;
    const bound = (v: number) => Math.min(width - NODE_W / 2, Math.max(-last - NODE_W / 2, v));
    if (this.leftGoal !== null) {
      const goal = bound(this.leftGoal);
      this.left += (goal - this.left) * SNAP;
      if (Math.abs(goal - this.left) < 0.5) this.leftGoal = null;
    }
    this.left = bound(this.left);
    const x = (d: number) => this.left + d * PITCH;
    this.links.clear();
    this.marks.clear();
    const nowShown = new Set<Container>();
    const rows = Math.ceil(height / ROW / 2) + 2;
    for (let d = 0; d < t.columns.length; d += 1) {
      const cx = x(d);
      if (cx > width || cx + NODE_W < 0) continue;
      const col = t.columns[d];
      const c = this.columns[d];
      const centre = this.rankAt(d, c.at);
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
      // first and from its bottom to the bottom of the last, filled.
      const px = x(d - 1) + NODE_W;
      const prev = t.columns[d - 1];
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
        this.funnel(px, py - ph, py + ph, cx, top, bottom);
      }
    }
    this.links.fill({ color: this.colours.other, alpha: this.colours.otherAlpha });
    // The item under the pointer and those above it up to its root, each joined to the one above by a ribbon of light
    // blue and ringed in blue, over everything else, the item itself glowing too; each comes in quickly, and fades out
    // once the pointer has left.
    // Over a shadow, the item it points to and those above it light too; over an item, each of its shadows and those
    // above each, up to its root, which depend on it too.
    const lit = (from: number) => {
      const set = new Set<number>();
      const light = (start: number) => {
        for (let i = start; i >= 0 && !set.has(i); i = t.parent[i]) set.add(i);
      };
      if (from < 0) return set;
      light(from);
      const id = this.ids[from];
      const to = this.real.get(id);
      if (this.shadows[from] === 1 && to !== undefined) light(to);
      if (this.shadows[from] === 0) for (const sh of this.shadowsOf.get(id) ?? []) light(sh);
      return set;
    };
    this.under = this.pointed(t, x, y);
    const chain = lit(this.under);
    // The item selected, the same way, in purple, under the blue; only the item itself glows, those lit with it are
    // only ringed, so that it stands out among them.
    const picked = this.selected === null ? -1 : (this.real.get(this.selected) ?? -1);
    const chosen = lit(picked);
    for (const i of chosen) {
      const p = t.parent[i];
      if (p < 0) continue;
      const d = t.depth[i];
      const [py, ph, iy, ih] = [y(d - 1, t.rank[p]), half(d - 1, t.rank[p]), y(d, t.rank[i]), half(d, t.rank[i])];
      this.funnel(x(d - 1) + NODE_W, py - ph, py + ph, x(d), iy - ih, iy + ih);
    }
    if (chosen.size > 1) this.links.fill(PALETTE.chosenRibbon);
    const ease = (lights: Map<number, number>, on: Set<number>) => {
      for (const i of on) if (!lights.has(i)) lights.set(i, 0);
      for (const [i, a] of lights) {
        const to = on.has(i);
        const next = a + ((to ? 1 : 0) - a) * (1 - Math.exp(-step / (to ? GLOW_IN : GLOW_OUT)));
        if (!to && next < 0.01) lights.delete(i);
        else lights.set(i, next);
      }
    };
    ease(this.glowing, chain);
    ease(this.haloed, new Set(this.under < 0 ? [] : [this.under]));
    for (const [i, a] of this.glowing) {
      const p = t.parent[i];
      if (p < 0) continue;
      const d = t.depth[i];
      const [py, ph, iy, ih] = [y(d - 1, t.rank[p]), half(d - 1, t.rank[p]), y(d, t.rank[i]), half(d, t.rank[i])];
      this.funnel(x(d - 1) + NODE_W, py - ph, py + ph, x(d), iy - ih, iy + ih);
      this.links.fill({ color: PALETTE.hoverRibbon, alpha: a });
    }
    for (const i of chosen) {
      const d = t.depth[i];
      const [gx, gy, gh] = [x(d), y(d, t.rank[i]), half(d, t.rank[i])];
      if (i === picked)
        this.marks
          .roundRect(gx - HALO / 2, gy - gh - HALO / 2, NODE_W + HALO, 2 * gh + HALO, 6 + HALO / 2)
          .stroke({ color: PALETTE.chosen, width: HALO, alpha: HALO_ALPHA });
      this.marks.roundRect(gx, gy - gh, NODE_W, 2 * gh, 6).stroke({ color: PALETTE.chosen, width: RING });
    }
    for (const [i, a] of this.haloed) {
      const d = t.depth[i];
      const [gx, gy, gh] = [x(d), y(d, t.rank[i]), half(d, t.rank[i])];
      this.marks
        .roundRect(gx - HALO / 2, gy - gh - HALO / 2, NODE_W + HALO, 2 * gh + HALO, 6 + HALO / 2)
        .stroke({ color: PALETTE.over, width: HALO, alpha: HALO_ALPHA * a });
    }
    for (const [i, a] of this.glowing) {
      const d = t.depth[i];
      const [gx, gy, gh] = [x(d), y(d, t.rank[i]), half(d, t.rank[i])];
      this.marks.roundRect(gx, gy - gh, NODE_W, 2 * gh, 6).stroke({ color: PALETTE.over, width: RING, alpha: a });
    }
    // The columns fade into the page at the top and the bottom; at those edges the column under the pointer has a line
    // of blue.
    this.edges.clear();
    this.edges.rect(0, 0, width, EDGE).fill(this.fades.top);
    this.edges.rect(0, height - EDGE, width, EDGE).fill(this.fades.bottom);
    this.columns.forEach((c, d) => {
      if (c.warm < 0.01) return;
      const gx = x(d) - (PITCH - NODE_W) / 4;
      const gw = NODE_W + (PITCH - NODE_W) / 2;
      this.edges.rect(gx, 0, gw, EDGE_LINE).fill({ color: PALETTE.over, alpha: c.warm });
      this.edges.rect(gx, height - EDGE_LINE, gw, EDGE_LINE).fill({ color: PALETTE.over, alpha: c.warm });
    });
    this.drawMap(t);
    for (const v of this.shown) if (!nowShown.has(v)) this.layer.removeChild(v);
    this.shown = nowShown;
    this.stats.drawn = nowShown.size;
  }
}
