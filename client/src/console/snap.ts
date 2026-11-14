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
// item has an edge of the light accent.
// Under each item that depends on an item besides its main dependent, a shadow of it stands: a white card with a dashed
// edge and grey words, pointing to where the item stands. A filled funnel, grey, joins each item's top and bottom to
// the first item under it and the last. Two items are lit, each with the way from it to its root, and from each of its
// shadows to theirs, by rings and by ribbons joining each to the one above: the item under the pointer in blue, faded
// out once the pointer leaves, and the item selected by a click in purple, until it is let go; a click on a shadow
// selects the item it points to and brings it to the middle of the screen. The mark of a cell running pulses. Every
// column fades out into the page near the top and the bottom of the screen; at those two edges the column under the
// pointer has a line of blue, faded in as the pointer comes. Only what is on the screen is drawn. What it draws and how
// fast is kept in `stats`.
//
// Brought over from the console of the engine on `old-main` (`client/src/console/snap.ts`), its layout, its motion and
// every constant as they were. What changed: a card's second row draws the works page's strip of the work's cells where
// it drew the stages; what the new interface has nothing for
// yet (an item new to the owner, messages unread) is read from the console's made-up data (`mock.ts`); the colours the
// page's tokens came from are read from the tokens, so that the canvas follows light and dark; and the canvas follows
// the size of what holds it, besides the window's.

import { Application, BitmapText, Container, FillGradient, Graphics, TextStyle } from 'pixi.js';

import { MARK, MARKS } from '../works/state-marks';
import type { CellNow, CellState } from '../works-api';
import { type Shown, type ShownWork, whoGives } from './items';
import { indexTree, type Tree } from './tree';

export const NODE_W = 300;
/// A row's height, and the height of a node of one row: an item with a work going on has a second row, the work's, this
/// part of a row high, so that the work sits close under the title.
const NODE_H = 26;
export const ROW = 32;
const WORK_ROW = 0.55;
export const PITCH = 370;
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
/// How bright the items not in focus are drawn while an item is in focus or selected; how much of the links' own
/// strength is left when the items are fully dimmed; and how long an item takes to fade down and to come back up, in
/// milliseconds.
const DIM = 0.35;
const DIM_LINKS = 0.4;
const FADE_DOWN = 220;
const FADE_UP = 110;
/// How long the pointer rests on an item before it is the one in focus, and how long it is off every item before the
/// focus is let go, in milliseconds.
const DWELL_ON = 100;
const DWELL_OFF = 180;
const GLOW_OUT = 160;
/// The ring round an item lit, the one selected or under the pointer or one above them: its edge's width, and the halo
/// outside it, how wide and how strong, within the room between two items.
const RING = 2;
/// How far in from the side of the room an item brought into view stands.
const INSET = 24;
const HALO = 5;
const HALO_ALPHA = 0.32;
/// The mark of a cell running pulses: lit most of each beat, it fades out and back in during the last part of it.
/// How long a beat is, in milliseconds; the part of it the fade takes; and how faint the mark goes at its faintest.
const PULSE_MS = 750;
const FADE = 0.4;
const DARK = 0.15;
/// The part of the way to where they are going that the columns move sideways each frame, when the scene brings an
/// item into view.
const SNAP = 0.32;
/// How much of the way to where the scroll has taken it the column scrolled goes each frame; and how many pixels of the
/// way, which no screen shows, may be left when it goes the rest at once, so that it comes to rest where the scroll took
/// it within some thirty frames, and does not creep on unseen until what is left is too small for a number to hold.
const GLIDE = 0.35;
const REST = 0.01;
/// How far from the middle of the screen, in rows, an item's want of the column beside it still weighs: its weight
/// falls off as a bell of this width. A want is met within `SLACK` rows times the inverse of its weight less one, none
/// for an item in the middle and the more the further out it is, so that items far out, weighing little, ask for little
/// and do not swing the column between them, and an item coming in towards the middle asks for a little more at a time.
const REACH = 3;
const SLACK = 1;
/// How many rows of the column scrolled stay on the screen at least, at its top or its bottom.
export const KEEP = 3;
/// How much of the way to where it stands a column beside the one scrolled goes each frame: one that was the one
/// scrolled rests where its own scroll left it, not where its links would put it, and glides there once another is.
/// It goes no faster than `PACE` times the column it follows moved in that frame, so that a small scroll never moves
/// another column far, however far its links would put it, and every column stops when the scroll does; what is left it
/// makes up as the scroll goes on.
const LINK_EASE = 0.2;
export const PACE = 1.5;

/// The console's colours: words, marks and lines in greys; one accent for what runs; amber for what
/// waits for the owner; red for what has gone wrong or on long; violet for what has gone on for days. These are the
/// console's own, in light; `themed` gives those the page's tokens came from as the tokens say, light or dark.
const LIGHT = {
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
  /// A card's edge above a work.
  branch: '#c7cdfd',
  /// The ribbons from the item under the pointer up to its root.
  hoverRibbon: '#e4dcfd',
  /// The ring round the item selected and those on its way to its root, and the ribbons between them.
  chosen: '#8b5cf6',
  chosenRibbon: '#e2d7fd',
  /// A column's bar on the map, light, so that the marks across it show.
  mapBar: '#aab6fd',
  /// The colours of works, which tint their cards: running a vivid sky blue, green being kept for what is done; waiting
  /// for the owner a vivid orange, the one that asks for them; waiting for something else a pale yellow, which asks
  /// nothing of them; paused grey; halted red.
  running: '#8b5cf6',
  asked: '#f5840c',
  paused: '#8b93a1',
  halted: '#e5484d',
  waiting: '#e8cc6a',
  amber: '#d97706',
  red: '#e5484d',
  violet: '#8b5cf6',
  green: '#2f9e6e',
  /// The item under the pointer, and those above it, ringed; the lines at the edges of the column under it.
  over: '#a78bfa',
  /// The words on a filled tag.
  onTag: '#ffffff',
  /// What is new to the owner: the line inside a card's left edge, and its mark on the map.
  unseen: '#8b5cf6',
  accent: '#8b5cf6',
  /// Where a work's time went, in the colours of the track in the item's window (console.css): a model's, the
  /// tools', idle, and the ground they lie on with how far it lets the card through.
  model: '#8b5cf6',
  tools: '#eb670f',
  idle: '#e8ecf2',
  track: '#818b98',
  trackAlpha: 0x1f / 255,
  /// What joins an item to those under it: Primer's neutral muted, `#818b981f` in light and `#656c7633` in dark, as
  /// the console on `old-main` read it, a little lighter.
  other: '#818b98',
  otherAlpha: 0.85 * (0x1f / 255),
};

/// The colours the scene draws in now: the console's own in light, or as `themed` gives them.
const PALETTE = { ...LIGHT };

/// The colour of a token of the page (styles/tokens.css), `#rrggbb`, or `fallback` where it has none such.
function token(name: string, fallback: string): string {
  const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return /^#[0-9a-f]{6}$/i.test(v) ? v : fallback;
}

/// The colour a part `k` of the way from `a` to `b`, as `#rrggbb`.
const between = (a: string, b: string, k: number) => `#${mix(a, b, k).toString(16).padStart(6, '0')}`;

/// The scene's colours as the page's theme has them. Those the page's tokens came from (look.md) are read from the
/// tokens, which in light are the console's own; in dark, the colours the tokens have no step for are worked out from
/// the tokens, keeping their relations to the card and the ground, as the console had no dark ground.
export function themed(): typeof LIGHT {
  const t = {
    ink: token('--text', LIGHT.ink),
    muted: token('--muted', LIGHT.muted),
    hair: token('--line', LIGHT.hair),
    card: token('--card', LIGHT.card),
    page: token('--ground', LIGHT.page),
    chosen: token('--accent', LIGHT.chosen),
    running: token('--running', LIGHT.running),
    asked: token('--orange', LIGHT.asked),
    halted: token('--red', LIGHT.halted),
    red: token('--red', LIGHT.red),
    green: token('--green', LIGHT.green),
    onTag: token('--on-colour', LIGHT.onTag),
  };
  if (getComputedStyle(document.documentElement).colorScheme !== 'dark') return { ...LIGHT, ...t };
  return {
    ...LIGHT,
    ...t,
    faint: between(t.muted, t.card, 0.3),
    pale: between(t.muted, t.card, 0.55),
    line: between(t.hair, t.muted, 0.35),
    edge: t.hair,
    branch: between(t.card, LIGHT.over, 0.45),
    hoverRibbon: between(t.card, LIGHT.over, 0.3),
    chosenRibbon: between(t.card, t.chosen, 0.3),
    mapBar: between(t.card, LIGHT.over, 0.6),
    paused: t.muted,
    amber: token('--orange-text', LIGHT.amber),
    violet: t.chosen,
    other: '#656c76',
    otherAlpha: 0.85 * (0x33 / 255),
    // Where a work's time went, as the track in the item's window has it in dark.
    model: t.chosen,
    tools: '#984b10',
    idle: '#2a2d33',
    track: '#656c76',
    trackAlpha: 0x33 / 255,
  };
}

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
const TONE = () => ({
  success: PALETTE.muted,
  attention: PALETTE.amber,
  danger: PALETTE.red,
  done: PALETTE.violet,
});

/// What a work's state is to its node: running, waiting for the owner, paused, stopped by the engine for a problem, or
/// waiting for something else.
export type Mood = 'running' | 'owner' | 'paused' | 'halted' | 'waiting';

/// A round under way or about to begin is running; a fault halts it, and so does the engine pausing it, as for a round
/// that failed; a work that waits for anything the owner gives waits for the owner.
export function moodOf(w: Pick<ShownWork, 'now'>): Mood {
  const now = w.now;
  if (now.kind === 'round' || now.kind === 'next') return 'running';
  if (now.kind === 'fault') return 'halted';
  if (now.kind === 'paused') return now.by === 'engine' ? 'halted' : 'paused';
  if (now.kind === 'everything') return 'paused';
  if (now.kind === 'waits' && now.on.some((o) => whoGives(o) === 'owner')) return 'owner';
  return 'waiting';
}

/// The cell a work is at, and the colour its mark takes: red while the work is stopped for the owner or by the
/// engine, as its card is; none in any other state, where the mark's own state says enough.
function here(w: ShownWork, mood: Mood): { cell: string; colour: string } | null {
  const colour = mood === 'owner' || mood === 'halted' ? PALETTE.halted : null;
  if (colour === null) return null;
  const now = w.now;
  const cell =
    now.kind === 'round' || now.kind === 'next'
      ? now.cell
      : now.kind === 'waits'
        ? now.at
        : (w.cells.find((c) => c.open !== null)?.cell ?? null);
  return cell === null ? null : { cell, colour };
}

/// The colour of what an item's work does, which tints its card.
const WORK = (): Record<Mood, string> => ({
  running: PALETTE.running,
  // A work is running or stopped. One stopped for the owner is red, as one the engine stopped: both need the owner.
  owner: PALETTE.halted,
  paused: PALETTE.paused,
  halted: PALETTE.halted,
  // Stopped and not for the owner, as paused: one grey for both, since neither needs the owner.
  waiting: PALETTE.paused,
});

/// A token's length in pixels, or none where the page has no such token.
function length(name: string): number {
  const n = Number.parseFloat(getComputedStyle(document.documentElement).getPropertyValue(name));
  return Number.isFinite(n) ? n : 0;
}

/// How a state's mark is drawn: its fill and its ring's colour, each or neither, the ring's width, its height and its
/// corners, in pixels.
type CellLook = { fill: string | null; edge: string | null; ring: number; height: number; corner: number };

/// The marks of the cells' states as the page's tokens have them now, light or dark: the works page's marks, from the
/// one definition both draw from (`state-marks.ts`).
function cellLooks(): { measures: { side: number; gap: number }; looks: Record<CellState, CellLook> } {
  const side = length(MARK.side);
  const corner = length(MARK.corner);
  const colour = (name: string) => token(name, PALETTE.muted);
  const looks = Object.fromEntries(
    Object.entries(MARKS).map(([state, look]): [string, CellLook] => [
      state,
      look.shape === 'dash'
        ? { fill: colour(look.fill), edge: null, ring: 0, height: length(MARK.dash), corner: 0 }
        : {
            fill: look.fill === null ? null : colour(look.fill),
            edge: colour(look.edge),
            ring: length(look.width === 'ring' ? MARK.ring : MARK.hairline),
            height: side,
            corner,
          },
    ]),
  ) as Record<CellState, CellLook>;
  return { measures: { side, gap: length(MARK.gap) }, looks };
}

/// One mark of a card's strip as it was drawn: its cell and state; its top left, from the card's left and its middle;
/// its width, height and corners; its fill and its ring's colour and width; and whether it pulses.
export type DrawnMark = {
  cell: string;
  state: CellState;
  x: number;
  y: number;
  width: number;
  height: number;
  corner: number;
  fill: string | null;
  edge: string | null;
  ring: number;
  pulses: boolean;
};

/// A card's strip as it was drawn: its marks, how many cells did not fit, and where its room ends.
export type DrawnStrip = { marks: DrawnMark[]; more: number; end: number };

/// How much of its work's colour a card in work takes, the rest white, and its edge, a deeper shade of it.
/// How much of its state's colour fills a card whose work is running or that needs the owner.
const FILL = 0.18;
const EDGE_TINT = 0.45;
/// How much of the running colour the edge of a card whose work runs takes: nearly all of it, so that it is bright.
const EDGE_RUNNING = 0.9;
/// How far the title of an item not begun goes from the ink towards the muted grey.
const NOT_BEGUN = 0.7;
/// How far a card's number and strip stand in from its left edge.
const CARD_INSET = 10;
/// The glow of a card that needs the owner or runs: how far out each ring stands and how strong it is.
const CARD_GLOW: [number, number][] = [
  [1, 0.34],
  [2.5, 0.2],
  [4, 0.11],
  [5.5, 0.05],
];
/// How a glow moves: how long a breath of its rings takes and how faint they are at its low point; how long the
/// light on a running card's edge takes to go round, and the light's length as so many parts of so many pixels.
const BREATH_MS = 2600;
const BREATH_LOW = 0.35;
const TRAIL_MS = 3200;
const TRAIL_PARTS = 16;
const TRAIL_PART = 7;
/// TRIAL: whether a running card has the glowing rings as well as the light going round its edge.
const RUNNING_RINGS = false;
/// A card's corners: the page's corner (`--corner`), as its buttons and tags have.
const CARD_CORNER = 3;
/// The height of the line under a deliverable, which has no card.
const DELIVER_LINE = 1.5;
/// The width of every card's edge.
const EDGE_WIDTH = 1.5;

/// The colour of an item's number by its state: open, done or cancelled.
const NUMBER = () => ({
  open: PALETTE.faint,
  done: PALETTE.green,
  cancelled: PALETTE.pale,
});

/// What a work has taken, in dollars.
export const dollars = (usdMicros: number) => `$${(usdMicros / 1e6).toFixed(2)}`;

/// The size of a node's title: the page's body text (`--size-body`).
const TITLE_SIZE = 13;

/// The styles of a node's words, by their colour: its title in the sans-serif face, and its numbers in the page's
/// monospace, smaller. Each is made from plain values once: one made by spreading another keeps the other's fill.
type Styles = {
  title: (fill: string) => TextStyle;
  mono: (fill: string) => TextStyle;
};

/// The faces of a node's words: Primer's stacks, which the console on `old-main` drew in, without the face Primer names
/// first and the page never loaded. A canvas resolves a stack by its own rules (PixiJS quotes every name but the
/// generic ones), so the page's own stacks (`--font`, `--mono`) could draw another face in some browsers.
const SANS = '-apple-system, BlinkMacSystemFont, "Segoe UI", "Noto Sans", Helvetica, Arial, sans-serif';
const MONO = 'ui-monospace, SFMono-Regular, SF Mono, Menlo, Consolas, Liberation Mono, monospace';

function makeStyles(): Styles {
  const sans = SANS;
  const mono = MONO;
  const size = TITLE_SIZE;
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
  /// For each item, whether a work has begun on it or under it: the title of one with none is greyer.
  private begun: Uint8Array = new Uint8Array(0);
  /// The parts of a node that change while it is shown: the marks of the cells running, which pulse, and the label of
  /// how long its work has gone on, with when the work started.
  private pulses = new Map<number, Graphics>();
  private ages = new Map<number, { from: number; label: BitmapText; shown: string; row: number }>();
  private styles: Styles | null = null;
  /// The cells' marks in the theme's tokens, read each time the tree is drawn; and each card's strip as drawn, by its
  /// item's number.
  private measures = { side: 0, gap: 0 };
  private looks = {} as Record<CellState, CellLook>;
  private drawnStrips = new Map<number, DrawnStrip>();
  /// For each column, the ranks of its items whose works wait for the owner, which the map marks.
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
  /// How bright the items that the pointer and the selection do not light are drawn: all of them fully while nothing
  /// is lit, dimmed while something is, so that what is lit and what joins it stand alone; eased from frame to frame.
  private dim = 1;
  /// The items in focus: the one the pointer last rested on with what joins it, kept while the pointer moves on.
  private inFocus = new Set<number>();
  /// The item the pointer was over at the last frame, and since when it has been over it (or over none).
  private hoverWas = -1;
  private hoverAt = 0;
  /// The glow of each card that runs or needs the owner, to move it each frame: the rings that breathe, and for a
  /// card whose work runs the light that goes round its edge.
  private glows = new Map<number, { rings: Graphics; trail: Graphics | null; top: number; high: number; colour: string }>();
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
  /// The items as last given to `show`, drawn again in the new colours when the theme changes.
  private roots: Shown[] | null = null;
  /// What follows the size of what holds the canvas, and the theme; let go when the scene is.
  private sized: ResizeObserver | null = null;
  private themes: { media: MediaQueryList; root: MutationObserver } | null = null;

  async mount(host: HTMLElement): Promise<void> {
    Object.assign(PALETTE, themed());
    this.fades = { top: vertical(PALETTE.page, 1, 0), bottom: vertical(PALETTE.page, 0, 1) };
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
    // The canvas takes the size of what holds it whenever that changes, as when the panel beside it is folded away or
    // dragged, not only when the window does.
    this.sized = new ResizeObserver(() => this.app.queueResize());
    this.sized.observe(host);
    // Light or dark, the system's or the page's switch's: drawn again in the theme's colours.
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    const root = new MutationObserver(this.retheme);
    root.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
    media.addEventListener('change', this.retheme);
    this.themes = { media, root };
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
    this.sized?.disconnect();
    this.themes?.root.disconnect();
    this.themes?.media.removeEventListener('change', this.retheme);
    if (this.ready) this.app.destroy(true, { children: true, texture: true });
  }

  /// Takes the theme's colours, and draws the tree again in them where it stands.
  private retheme = () => {
    if (!this.ready || this.gone) return;
    Object.assign(PALETTE, themed());
    this.fades = { top: vertical(PALETTE.page, 1, 0), bottom: vertical(PALETTE.page, 0, 1) };
    this.app.renderer.background.color = PALETTE.page;
    if (this.roots !== null) this.show(this.roots);
  };

  /// Where the columns stand, for the tests: how far they are moved right, the column the others follow, and each
  /// column's place, in rows from its first item to the middle of the screen, and where it is going.
  place(): { left: number; driver: number; columns: { at: number; aim: number }[]; height: number; width: number } {
    return {
      left: this.left,
      driver: this.driver,
      columns: this.columns.map((c) => ({ at: c.at, aim: c.aim })),
      height: this.app.screen.height,
      width: this.app.screen.width,
    };
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
    this.roots = roots;
    this.layer.removeChildren();
    for (const v of this.views) v.destroy({ children: true });
    this.shown = new Set();
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
    // The items whose work has begun, and those above them: an item with nothing begun on it or under it is quieter.
    this.begun = new Uint8Array(items.length);
    items.forEach((i, k) => {
      if (!i.begun) return;
      for (let p = k; p >= 0 && this.begun[p] === 0; p = tree.parent[p]) this.begun[p] = 1;
    });
    this.pulses = new Map();
    this.glows = new Map();
    ({ measures: this.measures, looks: this.looks } = cellLooks());
    this.drawnStrips = new Map();
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
    const placeless = new Set<number>();
    this.columns.forEach((c, d) => {
      const b = before[d];
      const i = b === null || b === undefined ? undefined : index.get(b.id);
      if (b !== null && b !== undefined && i !== undefined && tree.depth[i] === d)
        c.at = this.middle(d, tree.rank[i]) + b.off;
      else if (was[d] !== undefined) c.at = was[d];
      else placeless.add(d);
    });
    if (old === null) {
      this.driver = 0;
      this.columns[0].at = this.middle(0, Math.floor((tree.columns[0].length - 1) / 2));
      placeless.delete(0);
    }
    this.driver = Math.min(this.driver, tree.columns.length - 1);
    this.columns[this.driver].aim = this.columns[this.driver].at;
    // The new columns, out from the one followed: each with what joins it to the item nearest the middle of the column
    // beside it level with that item, then where its links put it, at once.
    for (let d = this.driver + 1; d < this.columns.length; d += 1) if (placeless.has(d)) this.level(d, d - 1);
    for (let d = this.driver - 1; d >= 0; d -= 1) if (placeless.has(d)) this.level(d, d + 1);
    for (let k = 0; k < 100 && placeless.size > 0; k += 1) {
      for (let d = this.driver + 1; d < this.columns.length; d += 1) if (placeless.has(d)) this.link(d, d - 1);
      for (let d = this.driver - 1; d >= 0; d -= 1) if (placeless.has(d)) this.link(d, d + 1);
    }
    this.buildMap(tree);
    this.stats.items = items.length;
    if (this.selected !== null && !this.real.has(this.selected)) this.selected = null;
    this.announce();
  }

  /// The node of an item: a card, white, or while a work goes on on it a faint tint of the colour of what the work does
  /// with an edge of a deeper shade of it; its edge the accent's tint above a work. On its first row its number in its
  /// state's colour, its title, and at the right how many things its work waits for from the owner, white on red; on
  /// the second row, for an item whose work goes on, the strip of the work's cells as the works page draws it, where
  /// its time went, and how long it has gone on.
  private node(item: Shown, k: number): Container {
    const s = this.styles as Styles;
    const view = new Container();
    const h = high(this.rows[k]);
    const top = -h / 2;
    const row1 = top + NODE_H / 2;
    if (item.shadow) return this.shadow(item, view, top, h, row1);
    const mood = item.work !== null ? moodOf(item.work) : null;
    const card = new Graphics().roundRect(0, top, NODE_W, h, CARD_CORNER);
    // A card whose work is running, or that needs the owner (it waits for the owner, or the engine stopped it), is
    // filled with its state's colour, and its edge is a deeper shade of it. A card stopped for anything else has an
    // edge of its grey and no fill, so that the filled ones stand out among them.
    const state = mood !== null ? WORK()[mood] : null;
    const filled = state !== null && (mood === 'owner' || mood === 'halted' || mood === 'running');
    // A deliverable is no card of work: it has no box and no fill, only its number and title over a line, as a heading
    // of the items under it, so that the cards of the items that are worked on stand out from it.
    if (item.deliver) view.addChild(new Graphics().rect(0, top + h - DELIVER_LINE, NODE_W, DELIVER_LINE).fill(PALETTE.faint));
    else {
      // A card that needs the owner or whose work is running glows in its colour: a few rings fading outward, which
      // on the dark ground make it the first thing seen.
      if (filled && state !== null) {
        const glow = new Graphics();
        for (const [out, alpha] of CARD_GLOW)
          glow
            .roundRect(-out, top - out, NODE_W + 2 * out, h + 2 * out, CARD_CORNER + out)
            .stroke({ color: state, width: 2, alpha });
        view.addChild(glow);
        this.glows.set(k, { rings: glow, trail: null, top, high: h, colour: state });
      }
      view.addChild(
        card.fill(filled ? mix(PALETTE.card, state, FILL) : PALETTE.card).stroke({
          color:
            state !== null
              ? mix(PALETTE.card, state, mood === 'running' ? EDGE_RUNNING : EDGE_TINT)
              : this.lit[k] === 1
                ? PALETTE.branch
                : PALETTE.edge,
          width: EDGE_WIDTH,
        }),
      );
    }
    // A card whose work runs has a light going round its edge, drawn anew each frame over the card.
    const moving = this.glows.get(k);
    if (moving !== undefined && mood === 'running') {
      moving.trail = new Graphics();
      view.addChild(moving.trail);
    }
    // What is new to the owner is said once, by the number at the card's right; no line at its left.
    const x = CARD_INSET;
    const number = new BitmapText({
      text: String(item.id),
      style: s.mono(NUMBER()[item.standing]),
    });
    number.position.set(x, row1 - number.height / 2);
    const title = new BitmapText({
      text: item.title,
      // An open item nothing has begun on or under is a little greyer than one in work, so that it stands back.
      style: s.title(
        item.standing !== 'open'
          ? PALETTE.faint
          : this.begun[k] === 1
            ? PALETTE.ink
            : between(PALETTE.ink, PALETTE.muted, NOT_BEGUN),
      ),
    });
    title.position.set(x + Math.max(26, number.width + 8), row1 - title.height / 2);
    view.addChild(number, title);
    // At the right of the first row: how many things its work waits for from the owner, white on red, the strongest
    // mark a card has, as what the owner looks for first; its messages unread, white on orange; its returns sent back,
    // framed in grey; and, while its work is not drawn, a lock framed in grey with
    // how many items it waits for to merge, which stand under it.
    let right = NODE_W - 8;
    const tag = (text: string, fill: string, sign: 'back' | 'lock' | null, solid: boolean) => {
      const t = new BitmapText({ text, style: s.mono(solid ? PALETTE.onTag : fill) });
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
    // What stops its work for the owner in red, the strongest; a question that waits for the owner's answer and does
    // not stop the work yet in orange, a step weaker; how much is new to read in the colour of the line at the card's
    // left that says something is new.
    if (item.asks > 0) tag(String(item.asks), PALETTE.red, null, true);
    if (item.questions > 0) tag(String(item.questions), PALETTE.asked, null, true);
    if (item.unread > 0) tag(String(item.unread), PALETTE.unseen, null, true);
    if (item.work !== null && item.work.sentBack > 0) tag(String(item.work.sentBack), PALETTE.muted, 'back', false);
    if (item.work === null && item.blocked > 0) tag(String(item.blocked), PALETTE.muted, 'lock', false);
    // The title ends where the badges begin.
    fit(title, right - title.x);
    if (item.work === null || mood === null) return view;
    const w = item.work;
    const row2 = top + h - (WORK_ROW * ROW) / 2 - 3;
    // Where its time went, on a thin line: a model's, the tools', or idle; nothing where the engine does not say.
    const barW = 70;
    const barX = NODE_W - 8 - 36 - barW;
    const cells = [...w.bar];
    const kind = { m: PALETTE.model, t: PALETTE.tools, '.': PALETTE.idle };
    const cw = barW / Math.max(1, cells.length);
    const track = new Graphics();
    // Drawn as the track in the item's window is, a little lower: a rounded ground, and on it a rounded block for
    // each stretch of one kind.
    const trackH = 4;
    if (cells.length > 0) {
      track
        .roundRect(barX, row2 - trackH / 2, barW, trackH, trackH / 2)
        .fill({ color: PALETTE.track, alpha: PALETTE.trackAlpha });
      for (let i = 0; i < cells.length; ) {
        let j = i;
        while (j < cells.length && cells[j] === cells[i]) j += 1;
        track
          .roundRect(barX + i * cw, row2 - trackH / 2, Math.max(2, (j - i) * cw), trackH, 2)
          .fill(kind[cells[i] as 'm' | 't' | '.'] ?? kind['.']);
        i = j;
      }
    }
    // The second row: the work's strip, as the works page draws it, from under the title, so that the number stands
    // alone in a column at the card's left, ending short of where its time went.
    this.strip(view, item.id, k, w.cells, title.x, row2, barX - 8, here(w, mood));
    // How long it has gone on, at the right, its colour by its age; the label is kept up as time passes.
    const label = new BitmapText({ text: '', style: s.mono(PALETTE.muted) });
    view.addChild(track, label);
    this.ages.set(k, { from: w.from, label, shown: '', row: row2 });
    this.age(k);
    return view;
  }

  /// The strip of a work's cells on the second row of its card, item `id` at `k` in the tree: one mark per cell in the
  /// order of the walk, as the works page's (`state-marks.ts`), from `x` and no further than `end`, their middles on
  /// `y`. Where the flow has more cells than fit at the marks' size, those that fit are drawn, the first in the walk,
  /// and after them how many more there are, muted. The marks of cells running fade with the beat while their rounds
  /// are under way. What it drew is kept, for the tests.
  private strip(
    view: Container,
    id: number,
    k: number,
    cells: CellNow[],
    x: number,
    y: number,
    end: number,
    at: { cell: string; colour: string } | null = null,
  ) {
    const s = this.styles as Styles;
    const { side, gap } = this.measures;
    const pitch = side + gap;
    let shown = cells.length;
    let more: BitmapText | null = null;
    if (pitch <= 0) shown = 0;
    else if (shown * pitch - gap > end - x) {
      // Room for the count of those left out, at its widest, after the marks that fit.
      more = new BitmapText({ text: `+${cells.length}`, style: s.mono(PALETTE.muted) });
      shown = Math.max(0, Math.floor((end - x - 4 - more.width + gap) / pitch));
      more.text = `+${cells.length - shown}`;
      more.position.set(x + shown * pitch - gap + 4, y - more.height / 2);
    }
    const marks: DrawnMark[] = cells.slice(0, shown).map((c, i) => {
      const own = this.looks[c.state];
      // The cell the work is at, while it waits for the owner or is halted, in the colour of its state, filled.
      const look = at !== null && at.cell === c.cell ? { ...own, fill: at.colour, edge: at.colour } : own;
      return {
        cell: c.cell,
        state: c.state,
        x: x + i * pitch,
        y: y - look.height / 2,
        width: side,
        height: look.height,
        corner: look.corner,
        fill: look.fill,
        edge: look.edge,
        ring: look.ring,
        pulses: c.state === 'running',
      };
    });
    const still = new Graphics();
    const running = new Graphics();
    for (const m of marks) {
      const g = m.pulses ? running : still;
      if (m.fill !== null) g.roundRect(m.x, m.y, m.width, m.height, m.corner).fill(m.fill);
      // The ring inside the mark's side, as the page's boxes draw a border: its line's middle half its width in.
      const r = m.ring;
      if (m.edge !== null)
        g.roundRect(m.x + r / 2, m.y + r / 2, m.width - r, m.height - r, Math.max(0, m.corner - r / 2)).stroke({
          color: m.edge,
          width: r,
        });
    }
    view.addChild(still, running);
    if (more !== null) view.addChild(more);
    if (marks.some((m) => m.pulses)) this.pulses.set(k, running);
    this.drawnStrips.set(id, { marks, more: cells.length - shown, end });
  }

  /// What the card of item `id` drew for its work's cells, for the tests: each mark, where it stands from the card's
  /// left and its middle, its measures and colours, and whether it pulses; how many cells did not fit; and where the
  /// room for the strip ends. None for an item whose card has no work.
  drawnStrip(id: number): DrawnStrip | null {
    return this.drawnStrips.get(id) ?? null;
  }

  /// The shadow of an item under another item that depends on it besides its main one: a white card, its edge dashed,
  /// its number and title grey, and at the right an arrow pointing away, to where the item stands.
  private shadow(item: Shown, view: Container, top: number, h: number, row1: number): Container {
    const s = this.styles as Styles;
    const ground = new Graphics().roundRect(0, top, NODE_W, h, CARD_CORNER).fill(PALETTE.card);
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
    number.position.set(CARD_INSET, row1 - number.height / 2);
    const title = new BitmapText({ text: item.title, style: s.title(PALETTE.faint) });
    title.position.set(CARD_INSET + Math.max(26, number.width + 8), row1 - title.height / 2);
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

  /// Moves a card's glow on by a frame. A card that needs the owner has rings that breathe, slowly. A card whose work
  /// runs has a short light going round its edge, bright at its head and fading behind it, in step on every such
  /// card, so that what is being worked on is seen moving.
  private moveGlow(
    glow: { rings: Graphics; trail: Graphics | null; top: number; high: number; colour: string },
    now: number,
    k: number,
  ) {
    const trail = glow.trail;
    if (trail === null) {
      // A card that needs the owner: its rings breathe, each card a little out of step with the others.
      const phase = (now / BREATH_MS + k * 0.37) % 1;
      glow.rings.alpha = BREATH_LOW + (1 - BREATH_LOW) * Math.sin(Math.PI * phase) ** 2;
      return;
    }
    // A card whose work runs: its rings are off for now (TRIAL: the owner is trying it without them), and the light
    // on its edge goes round in step with every other running card's.
    glow.rings.visible = RUNNING_RINGS;
    if (RUNNING_RINGS) glow.rings.alpha = BREATH_LOW + (1 - BREATH_LOW) * Math.sin(Math.PI * ((now / BREATH_MS) % 1)) ** 2;
    const [w, h, top] = [NODE_W, glow.high, glow.top];
    const round = 2 * (w + h);
    // A point of the card's edge, by how far round it from its top left corner, clockwise.
    const at = (far: number): [number, number] => {
      const d = ((far % round) + round) % round;
      if (d < w) return [d, top];
      if (d < w + h) return [w, top + d - w];
      if (d < 2 * w + h) return [w - (d - w - h), top + h];
      return [0, top + h - (d - 2 * w - h)];
    };
    const head = ((now / TRAIL_MS) % 1) * round;
    trail.clear();
    for (let j = 0; j < TRAIL_PARTS; j += 1) {
      const [x1, y1] = at(head - j * TRAIL_PART);
      const [x2, y2] = at(head - (j + 1) * TRAIL_PART);
      // A part that turns a corner is drawn as its two straight ends meet, through the corner.
      trail.moveTo(x1, y1);
      if (x1 !== x2 && y1 !== y2) {
        const corner: [number, number] = y1 === top || y1 === top + h ? [x2, y1] : [x1, y2];
        trail.lineTo(corner[0], corner[1]);
      }
      trail.lineTo(x2, y2).stroke({ color: PALETTE.onTag, width: 2, alpha: 0.9 * (1 - j / TRAIL_PARTS) ** 2 });
    }
  }

  /// Puts at the right of an item's second row how long its work has gone on, when the label has changed.
  private age(k: number) {
    const a = this.ages.get(k);
    if (a === undefined || this.styles === null) return;
    const { label, tone } = age(Date.now() - a.from);
    if (label === a.shown) return;
    a.shown = label;
    a.label.text = label;
    a.label.style = this.styles.mono(TONE()[tone]);
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

  /// Places column `c` with what joins it to the item of `from` nearest the middle of the screen, of those joined to
  /// it, level with that item: the middle of the block of the items under it, or the item above it.
  private level(c: number, from: number): void {
    const t = this.tree as Tree;
    const col = t.columns[from];
    const line = this.columns[from].at;
    let best = -1;
    for (let r = 0; r < col.length; r += 1) {
      const joined = c > from ? t.children[col[r]].length > 0 : t.parent[col[r]] >= 0;
      if (joined && (best < 0 || Math.abs(this.middle(from, r) - line) < Math.abs(this.middle(from, best) - line)))
        best = r;
    }
    if (best < 0) return;
    const item = col[best];
    let to: number;
    if (c > from) {
      const kids = t.children[item];
      to = (this.middle(c, t.rank[kids[0]]) + this.middle(c, t.rank[kids[kids.length - 1]])) / 2;
    } else to = this.middle(c, t.rank[t.parent[item]]);
    this.columns[c].at = to - (this.middle(from, best) - line);
  }

  /// Moves column `c` towards where it stands by its links with the column `from` beside it, which it follows. Each
  /// item of `from` near the middle of the screen wants what it is joined to in `c` level with it, the item above it or
  /// any part of the block of the items under it, the more nearly the nearer the middle it is, and weighs the more. The
  /// column stays where it is while every want is met, and else goes to the nearest place where the wants not met, each
  /// pulling by its weight times how far it is from being met, pull it no further either way, but no more than `most`
  /// rows in one frame. With nothing joining them near the middle, it stays.
  private link(c: number, from: number, most = Infinity): void {
    const t = this.tree as Tree;
    const src = this.columns[from];
    const me = this.columns[c];
    const col = t.columns[from];
    const line = src.at;
    // Where each want is met, as the places of the column `c` at the middle of the screen, from `lows` to `highs`, and
    // its weight: what stands from `top` to `bottom` of the column is level with an item `s` rows below the middle
    // while its place is between `top - s` and `bottom - s`.
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
      const weight = Math.exp(-(s * s) / (2 * REACH * REACH));
      const slack = SLACK * (1 / weight - 1);
      lows.push(top - s - slack);
      highs.push(bottom - s + slack);
      weights.push(weight);
    }
    if (weights.length === 0) return;
    const now = me.at;
    // How hard the wants not met pull the column below `x`, those above it counted against: it grows with `x`.
    const pull = (x: number) => {
      let g = 0;
      for (let i = 0; i < weights.length; i += 1) {
        if (x < lows[i]) g += weights[i] * (x - lows[i]);
        else if (x > highs[i]) g += weights[i] * (x - highs[i]);
      }
      return g;
    };
    const g = pull(now);
    if (g === 0) return;
    // The nearest place where the pull is spent: the last below where it still pulls down, or the first above where it
    // still pulls up.
    let lo = g > 0 ? Math.min(...lows) : now;
    let hi = g > 0 ? now : Math.max(...highs);
    for (let i = 0; i < 40; i += 1) {
      const mid = (lo + hi) / 2;
      if (g > 0 ? pull(mid) <= 0 : pull(mid) < 0) lo = mid;
      else hi = mid;
    }
    me.at += Math.max(-most, Math.min(most, ((g > 0 ? lo : hi) - me.at) * LINK_EASE));
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
      .fill(PALETTE.page)
      .stroke({ color: PALETTE.line, width: 1 });
    this.mapMask
      .clear()
      .rect(MAP_PAD / 2, MAP_PAD / 2, wide - MAP_PAD, MAP_HIGH - MAP_PAD)
      .fill(0xffffff);
    this.mapBars.mask = this.mapMask;
    this.map.addChild(this.mapFrame, this.mapMask, this.mapBars);
  }

  /// Draws each column on the map as one bar from its first item to its last, moved as the column is: in a light accent,
  /// or a deeper blue while the pointer is over it, marked red where works wait for the owner, and purple where the item
  /// selected stands.
  private drawMap(t: Tree) {
    const mid = MAP_HIGH / 2;
    this.mapBars.clear();
    this.columns.forEach((c, d) => {
      const from = mid + (0 - c.at) * this.mapRow;
      const n = t.columns[d].length;
      const to = mid + (this.slot(d, n - 1) + this.rowsAt(d, n - 1) - c.at) * this.mapRow;
      const left = MAP_PAD + d * (MAP_STRIP + MAP_APART);
      this.mapBars.rect(left, from, MAP_STRIP, Math.max(1, to - from)).fill(mix(PALETTE.mapBar, PALETTE.over, c.warm));
      // Where items are new to the owner, the accent across the bar; where works wait for the owner, red, over it.
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
    const was = this.columns.map((c) => c.at);
    this.columns.forEach((c, d) => {
      // The blue lines at the column's edges fade in while the pointer is over it, and out once it leaves, in about
      // half a second.
      c.warm += ((d === this.hovered ? 1 : 0) - c.warm) * (1 - Math.exp(-step / TURN));
      if (d !== this.driver) return;
      c.at += (c.aim - c.at) * GLIDE;
      if (Math.abs(c.aim - c.at) * ROW < REST) c.at = c.aim;
    });
    const moved = (d: number) => PACE * Math.abs(this.columns[d].at - was[d]);
    for (let d = this.driver + 1; d < this.columns.length; d += 1) this.link(d, d - 1, moved(d - 1));
    for (let d = this.driver - 1; d >= 0; d -= 1) this.link(d, d + 1, moved(d + 1));
    const y = (d: number, rank: number) => {
      const c = this.columns[d];
      return line + (this.middle(d, rank) - c.at) * ROW;
    };
    const half = (d: number, rank: number) => high(this.rowsAt(d, rank)) / 2;
    // The marks of cells running: lit, then in the last part of each beat faded out and back in, every running node
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
    const drawn: number[] = [];
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
        const glow = this.glows.get(item);
        if (glow !== undefined) this.moveGlow(glow, now, item);
        this.age(item);
        nowShown.add(view);
        drawn.push(item);
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
    this.links.fill({ color: PALETTE.other, alpha: PALETTE.otherAlpha * (DIM_LINKS + (1 - DIM_LINKS) * this.dim) });
    // The item under the pointer and those above it up to its root, each joined to the one above by a ribbon of light
    // blue and ringed in blue, over everything else, the item itself glowing too; each comes in quickly, and fades out
    // once the pointer has left.
    // Over a shadow, the item it points to and those above it light too; over an item, each of its shadows and those
    // above each, up to its root, which depend on it too. The items straight under it, which it depends on, light
    // as well, one level and no further, so that both what waits for it and what it waits for show at once.
    const lit = (from: number) => {
      const set = new Set<number>();
      const light = (start: number) => {
        for (let i = start; i >= 0 && !set.has(i); i = t.parent[i]) set.add(i);
      };
      const under = (of: number) => {
        for (const kid of t.children[of]) set.add(kid);
      };
      if (from < 0) return set;
      light(from);
      under(from);
      const id = this.ids[from];
      const to = this.real.get(id);
      if (this.shadows[from] === 1 && to !== undefined) {
        light(to);
        under(to);
      }
      if (this.shadows[from] === 0) for (const sh of this.shadowsOf.get(id) ?? []) light(sh);
      return set;
    };
    this.under = this.pointed(t, x, y);
    const chain = lit(this.under);
    // The item selected, the same way, in purple, under the blue; only the item itself glows, those lit with it are
    // only ringed, so that it stands out among them.
    const picked = this.selected === null ? -1 : (this.real.get(this.selected) ?? -1);
    const chosen = lit(picked);
    // While the pointer or the selection lights something, every other item drawn is dimmed, more under the pointer
    // than for the selection alone, so that the lit items and what joins them stand out; it comes and goes over a
    // moment, so that a pointer passing over the tree does not make it flicker.
    // What the dimming goes by is not the pointer's every move: an item becomes the one in focus only once the
    // pointer has rested on it a moment, and the focus is let go only once the pointer has been off every item a
    // moment. So a pointer crossing many items changes nothing, and going from one item to the next fades one way
    // into the other. Each item then fades to its own brightness, slowly down and a little faster up.
    if (this.under !== this.hoverWas) {
      this.hoverWas = this.under;
      this.hoverAt = now;
    }
    const rested = now - this.hoverAt;
    if (this.under >= 0 && rested > DWELL_ON) this.inFocus = chain;
    else if (this.under < 0 && rested > DWELL_OFF) this.inFocus = new Set();
    const focus = this.inFocus;
    const aim = focus.size > 0 || chosen.size > 0 ? DIM : 1;
    this.dim += (aim - this.dim) * (1 - Math.exp(-step / (aim < this.dim ? FADE_DOWN : FADE_UP)));
    for (const i of drawn) {
      const view = this.views[i];
      const to = aim === 1 || focus.has(i) || chosen.has(i) ? 1 : DIM;
      view.alpha += (to - view.alpha) * (1 - Math.exp(-step / (to < view.alpha ? FADE_DOWN : FADE_UP)));
    }
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
          .roundRect(gx - HALO / 2, gy - gh - HALO / 2, NODE_W + HALO, 2 * gh + HALO, CARD_CORNER + HALO / 2)
          .stroke({ color: PALETTE.chosen, width: HALO, alpha: HALO_ALPHA });
      this.marks.roundRect(gx, gy - gh, NODE_W, 2 * gh, CARD_CORNER).stroke({ color: PALETTE.chosen, width: RING });
    }
    for (const [i, a] of this.haloed) {
      const d = t.depth[i];
      const [gx, gy, gh] = [x(d), y(d, t.rank[i]), half(d, t.rank[i])];
      this.marks
        .roundRect(gx - HALO / 2, gy - gh - HALO / 2, NODE_W + HALO, 2 * gh + HALO, CARD_CORNER + HALO / 2)
        .stroke({ color: PALETTE.over, width: HALO, alpha: HALO_ALPHA * a });
    }
    for (const [i, a] of this.glowing) {
      const d = t.depth[i];
      const [gx, gy, gh] = [x(d), y(d, t.rank[i]), half(d, t.rank[i])];
      this.marks.roundRect(gx, gy - gh, NODE_W, 2 * gh, CARD_CORNER).stroke({ color: PALETTE.over, width: RING, alpha: a });
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
