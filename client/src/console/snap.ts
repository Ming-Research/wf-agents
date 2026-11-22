// The console's tree in columns, drawn in WebGL by PixiJS. One column for each level, from the roots at the left; in
// each, the level's items one under another, those under one item standing apart from those under the next. Scrolling
// over a column moves it with the scroll, as far as leaves a few of its rows on the screen, and it rests where the
// scroll stops. Each column beside it stands where the most of what joins the two near the middle of the screen is
// level: each item of the one beside it, weighed the more the nearer the middle it is, wants the block of items under
// it, or the item above it, level with it, any part of either doing; the column stays where it is while every want is
// met, and else glides only as far as the weighed wants call for, so that no column leaps. Each column further out
// follows the one beside it the same way.
//
// Each item is a card; one a work goes on on is a faint tint of the colour of what the work does, and one above such
// an item has an edge of the light accent. Under each item that depends on an item besides its main dependent, a
// shadow of it stands: a white card with a dashed edge and grey words, pointing to where the item stands. A filled
// funnel, grey, joins each item's top and bottom to the first item under it and the last. Two items are lit, each with
// the way from it to its root, and from each of its shadows to theirs, by rings and by ribbons joining each to the one
// above: the item under the pointer, faded out once the pointer leaves, and the item selected by a click, until it is
// let go, each ring in the colour of its own item; a click on a shadow selects the item it points to and brings it to
// the middle of the screen. The mark of a cell running pulses, while its card stands still. The way from each item
// whose work runs up to its root is lit in the colour of a running card's edge, with a light running down the whole of
// it from the root to the item, over the cards between and on: each card's edge is lit as the light crosses it, and a
// light stands where a way meets a card. Every column fades out into the page near the top and the bottom of the
// screen; at those two edges the column under the pointer has a line of blue, faded in as the pointer comes. Only what
// is on the screen is drawn. What it draws and how fast is kept in `stats`. An act of the engine's own under way keeps
// its card running as a round does; a work that waits for the calls of tools the engine carries out is a work that
// waits. Either way its cell's mark, in the yellow of a call, pulses as a running cell's does. An item with items under
// it has, just past its card's right edge where the funnel to them leaves, the page's mark of a part that folds: a
// press on it folds them away or shows them again, as the console is told, and selects nothing. A folded item says at
// its row's right how many items stand under it, by how they stand; what its fold hides counts at it wherever the tree
// marks what is under an item: the edge above a work, the lit way, the title of what has begun, and the map. A click
// on the shadow of an item folded away, or the selection of one, has the console open the folds over it, and the item
// is selected once it is drawn.
//
// Brought over from the console of the engine on `old-main` (`client/src/console/snap.ts`), its layout, its motion and
// every constant as they were. What changed: a card's second row draws the level the work's cell runs at, the works
// page's strip of the work's cells where it drew the stages, and the lines the work changed; an ended item's one row,
// the lines its work changed; what the new interface has nothing for
// yet (an item new to the owner, messages unread) is read from the console's made-up data (`mock.ts`); the colours the
// page's tokens came from are read from the tokens, so that the canvas follows light and dark; the canvas follows the
// size of what holds it, besides the window's; the links dim with the items they join while an item is lit
// (`DIM_LINKS`); and the way from each item whose work runs up to its root is lit, with a light running down it
// (`RUN`), where a running card had a light going round its edge and a card that waits for the owner rings round
// it.

import { Application, BitmapText, BlurFilter, Container, FillGradient, Graphics, TextStyle } from 'pixi.js';

import { accentNow, onAccent, turning } from '../accent';
import { LINES_LOOKS, levelLook, levelWords } from '../marks';
import { CELL_LOOKS, MARK, pulsing } from '../works/state-marks';
import type { CellNow, CellState } from '../works-api';
import { NO_LEVEL, STATE, linesAdded, linesRemoved, stateOf, stoppedAt } from '../works/words';
import { STANDINGS, type Shown, type ShownWork, age } from './items';
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
/// How bright the items not in focus are drawn while an item is in focus or selected; the least of the links' own
/// strength they could be drawn at, were the items drawn at no brightness ([`linksStrength`]); and how long an item
/// takes to fade, down and back up alike, in milliseconds.
export const DIM = 0.35;
const DIM_LINKS = 0.4;
const FADE_MS = 220;
/// Everything the lit ways are drawn by, in one place, so that the look is read and set from one block and not
/// from numbers strewn through the drawing.
export const RUN = {
  /// The way from each item whose work runs up to its root, lit in the colour of a running card's edge. A line alone
  /// would be read as a link like the funnels under it; the light is what says the work on it is running now, as the
  /// dashes that moved along it said before, and it says it standing still, where a dash said it only by moving.
  ///
  /// It is drawn as a cord of light: its line, sharp, and the glow round it, which is the same line drawn wider and
  /// blurred, once close round it and once further out. Blurred, the glow falls away smoothly with the distance from
  /// the line, as light does; lines of fixed widths laid one over another for it read as bands with edges, not as a
  /// glow. Each is a width in pixels and a strength, and each glow how far it is blurred. Where the page is dark all
  /// of it is added to the ground ([`adding`]), as light is, so that it tells on it.
  line: 1.4,
  lineAlpha: 0.8,
  near: 4,
  nearAlpha: 0.9,
  nearBlur: 4,
  far: 10,
  farAlpha: 0.3,
  farBlur: 14,
  /// Where the page is light the way cannot be lit, a light added to a light ground telling nothing. There the way is
  /// drawn as a line of the running colour, darker than the page, and the light running down it as a near white glint
  /// within that line and narrower than it, which is how a light reads on a dark thing; the glow round the line, which
  /// is laid as ink there and tells far more strongly than light on a dark ground, is drawn fainter. These are how much
  /// wider the way's line is drawn there, how strongly, how much of the light's own width its glint takes, and how much
  /// fainter the glow goes.
  inkLine: 2.1,
  inkAlpha: 0.95,
  inkGlint: 1,
  inkRest: 0.3,
  /// The line is thinner at its ends than in the middle, as a cord drawn tight between two cards is: how thin, as a
  /// part of its width, and in how many lengths it is drawn, each at the width of its own middle.
  ends: 0.35,
  wide: 8,
  /// One light runs the whole way down from each root, over every card and way under it, to the item whose work runs:
  /// how long the light is, in pixels across the page, and how long one run takes, in milliseconds. A run is a time
  /// and not a speed: the ways move while the columns glide, so the length of a run changes under the light, and a
  /// speed would then put its head at a new place each frame, which reads as a flicker. Everything is measured across
  /// the page, from the left side of the roots' cards: the light's head is a place across the page, where it stands
  /// on every way and every card at once, so that it goes on from one way to the next without a break, goes down
  /// both where the ways under a root fork, and stands level on ways of every steepness: a steep way's curve is longer
  /// than a shallow one's over the same distance across, so that measured along each way the heads on the longer
  /// curves would fall behind. A way counts for how far it goes across, and a card for its width. Every root's light
  /// begins at one moment and they begin again together, once the furthest item of all has been reached, so that what
  /// runs across the page is read as one sweep and no two roots drift apart.
  light: 140,
  beat: 1500,
  /// Between one run and the next the ways are dark for a moment: how long, in milliseconds. Without it a run begins
  /// again the instant the one before has gone, and what is read is a line pouring on without end, not a light that
  /// sets off from the root each time.
  dark: 700,
  /// The light does not run at one speed: it starts slow and gathers pace, as a current filling a wire does, so that a
  /// run is seen to begin at the root and not merely to pass. [`RUN.beat`] is how long a whole run takes; this is the
  /// power its distance goes by with the part of the run gone, 1 being one speed throughout and more than 1 starting
  /// slower and ending faster.
  ease: 1.8,
  /// The light running down the way is the way's own line, brighter and wider where the light is, as a wire is where
  /// the current is in it: its line and its glow close in and further out, blurred as the way's are, each a width in
  /// pixels and a strength; and how many lengths it is drawn in, each brighter than the one behind it, so that it
  /// fades out behind its head.
  lightLine: 2.8,
  lightAlpha: 0.95,
  lightNear: 6,
  lightNearAlpha: 0.7,
  lightFar: 16,
  lightFarAlpha: 0.45,
  lightPieces: 16,
  /// The way's line is a cord of strands: hairs of light beside the line, each at its own distance out from it and its
  /// own strength, gathered close round it and faint at rest, and parted where the light is, as a cord of fibres is
  /// where the current in it is strong; parted, each runs beside the line at its own distance, level with the others
  /// and not narrowing towards the light's head, and at either end of the way they gather into the line, where it
  /// meets a card. How many strands, how wide each is (a whole pixel at least, a thinner line being drawn broken), how
  /// strong at the light's head and, as a part of that, at rest, and how far out the outermost stands at rest and
  /// where the light is, in pixels.
  strands: 14,
  strand: 1,
  strandAlpha: 0.4,
  strandRest: 0.35,
  spreadRest: 2.5,
  spread: 9,
  /// Parted, the strands come in from nothing over this part of the light's length behind its head, so that they do
  /// not stop short at the head, which the light's own line and glow hold.
  strandIn: 0.2,
  /// The light is the colour of a light (`pulse`) where it is brightest and the running colour where it fades, so that
  /// what runs is seen as a light passing and not as the way itself brightening; the way at rest is the running colour
  /// alone, which the accent picked gives. How far the glow, the strands and the cards' edges the light lights go from
  /// the running colour towards it at the light's head.
  hue: 0.8,
  /// How far the line of the way, of the light and of the spark is taken towards `glowTo`, which is what makes it read
  /// as a light and not as a line of the accent.
  core: 0.55,
  /// Two lights are drawn over the cards, where the ways run under them: the one where a way meets a card, which
  /// stands at both ends of every way lit, and a card's edge as a light crosses it, which comes and goes with the
  /// light. Both are laid as a way's line is, with its glow ([`RUN.near`] and the rest), so that each reaches past
  /// the line it lies along and falls away with the distance from it, as the light on a way does; these are the parts
  /// of a way's widths they take, a card's edge being thinner than a way and a way's end a little wider, so that the
  /// way is seen to go on into the card.
  edgeWide: 0.8,
  portWide: 1.2,
  /// The light at a way's end is a flare along the card's edge the way meets, burning white where the way meets it,
  /// as the way's own line carries the light along the way: the way's line turns into the edge both ways in a short
  /// curve, so that the light is seen to flow from one into the other and not to stop against it, and goes on along
  /// the edge, falling away from the point, so that the point reads as the hottest place. It lies along the card's
  /// own edge alone and never goes round the corner, so that the card's border elsewhere stays one colour, its own.
  /// How far the flare reaches along the edge each way, as a part of the edge from the point to the corner; in how
  /// many lengths it is drawn; how steeply it falls away along them (1 falling evenly, more falling faster near the
  /// point); and how far along the way and the edge the curve between them begins and ends, in pixels.
  portHold: 1,
  portPieces: 8,
  portFall: 0.7,
  fillet: 5,
  /// At the point a glare, drawn out along the edge as the flare is, which is most of what is seen of the point: a
  /// point read as hot, and not as a dot, is a small core in a wide glow. How wide its core is, in pixels, how many
  /// times taller than wide it is drawn, and how strong the glow round it is.
  spot: 1.1,
  spotTall: 4,
  spotGlow: 1,
  /// It burns at this much of its strength while the light is elsewhere on the way, and up to the full as the light's
  /// head goes through it; it takes the light's brightness from this far along the way to either side of it, so that
  /// it burns up as the head comes and falls away as it goes.
  portRest: 0.8,
  portNear: 24,
  /// How many straight pieces a way's curve is drawn in: enough that its bend reads as curved; and how near two places
  /// along a way count as one, in pixels, which tells the head of a light from a light the way has cut short.
  pieces: 32,
};
const NEAR = 0.5;
/// How long the pointer rests on an item before it is the one in focus, and how long it is off every item before the
/// focus is let go, in milliseconds.
const DWELL_ON = 100;
const DWELL_OFF = 180;
const GLOW_OUT = 160;
/// The ring round an item lit, the one selected or under the pointer or one above them: its edge's width; the item
/// selected's own a little wider, so that it stands out from the item under the pointer, lit the same way.
const RING = 2;
const CHOSEN_RING = 3;
/// How far in from the side of the room an item brought into view stands.
const INSET = 24;
/// The mark of an item's fold: the page's mark of a part that folds (`FoldMark`, parts.tsx), a triangle in a box this
/// wide, to the right while the items under it are folded away and turned down while they are shown, muted as the
/// form tree's is; its box this far out from the card's right edge, its middle at the card's. A press this far out from
/// the edge, within the card's height, is on it; under the pointer, its box this much wider each way is filled with the
/// accent's tint, as what can be pressed is (principle quiet-motion): the tint the pointer's ribbons are drawn in.
const FOLD_BOX = 12;
const FOLD_OUT = 6;
const FOLD_REACH = 24;
const FOLD_PAD = 3;
/// The triangle of the mark, folded and shown, as the corners of `FoldMark`'s path on its box, turned for the second.
const FOLD_RIGHT = [4, 2.5, 4, 9.5, 9, 6];
const FOLD_DOWN = [9.5, 4, 2.5, 4, 6, 9];
/// What pointing at the mark says it does.
export const FOLD_SAYS = { folded: 'Show the items under it', shown: 'Fold the items under it away' } as const;
/// The soft light round the item under the pointer, and round the item selected: a fill of the item's own colour behind
/// its card, this many pixels past its edge, blurred this much, at this strength, so that the light
/// shows only as a narrow glow round the card; the item selected's a little wider and softer, so that it stands out
/// from the one under the pointer. Over the light and under the cards lies the page's ground at every item the light
/// reaches, so that none of it shows through a card drawn faint.
const SOFT_SPREAD = 2;
const SOFT_BLUR = 6;
const SOFT_ALPHA = 0.45;
const CHOSEN_SPREAD = 4;
const CHOSEN_BLUR = 9;
/// The room a soft light's blur is drawn in past its fill, in strengths of the blur: wide enough that the blur fades
/// out before its edge. It is also as far as the light reaches past its fill.
const BLUR_ROOM = 4;

/// The blur of a soft light round an item, this strong, with its room: set after the filter is made, which sets its
/// own from its strength, too narrow for the blur to fade out in.
function softBlur(strength: number): BlurFilter {
  const blur = new BlurFilter({ strength });
  blur.padding = BLUR_ROOM * strength;
  return blur;
}

/// A layer of light: its lines, drawn sharp, and the glow round them close in and further out, the same lines drawn
/// wider on layers of their own and blurred ([`RUN.near`], [`RUN.far`]).
type Lit = { line: Graphics; near: Graphics; far: Graphics; nearBlur: BlurFilter; farBlur: BlurFilter };

function litLayer(): Lit {
  const nearBlur = softBlur(RUN.nearBlur);
  const farBlur = softBlur(RUN.farBlur);
  return {
    line: new Graphics(),
    near: new Graphics({ filters: [nearBlur] }),
    far: new Graphics({ filters: [farBlur] }),
    nearBlur,
    farBlur,
  };
}

/// Blurs `blur` `strength` strong, with its room, where it is not already.
function blurTo(blur: BlurFilter, strength: number) {
  if (blur.strength === strength) return;
  blur.strength = strength;
  blur.padding = BLUR_ROOM * strength;
}

/// The mark of a cell running pulses: lit most of each beat, it fades out and back in during the last part of it, the
/// part the works page's marks fade in too (styles/works.css, `mark-pulse`). How long a beat is, in milliseconds, and
/// how faint the mark goes at its faintest are the page's tokens (`--pulse`, `--pulse-faint`), read as the scene is
/// drawn; these are taken where the page has none, as in a test with no styles.
const PULSE_MS = 750;
const FADE = 0.4;
const DARK = 0.15;

/// The beat of a running cell's mark, as the page's tokens give it.
export function beatOf(): { ms: number; faint: number } {
  const ms = millisecondsOf('--pulse');
  const faint = Number.parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--pulse-faint'));
  return { ms: ms > 0 ? ms : PULSE_MS, faint: Number.isFinite(faint) ? faint : DARK };
}

/// A time the page's tokens give, in milliseconds, in whichever unit the styles write it: the page's build writes
/// `750ms` as `.75s`. None, as 0, where the token is no time.
function millisecondsOf(name: string): number {
  const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  const m = /^([+-]?(?:\d+\.?\d*|\.\d+))(ms|s)$/.exec(v);
  return m === null ? 0 : Number.parseFloat(m[1]) * (m[2] === 's' ? 1000 : 1);
}
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

/// The console's colours: words, marks and lines in greys; one accent for what runs; red for a stop, what waits for the
/// owner or what the engine halted; and for a work's age amber, then red past an hour. These are the
/// console's own, in light, those of the accent's hue in the default accent; `themed` gives those the page's tokens
/// came from as the tokens say, light or dark, and turns the others of the accent's hue to the accent picked in the
/// Settings view.
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
  branch: '#abd3fd',
  /// The ribbons from the item under the pointer up to its root.
  hoverRibbon: '#d5e2ff',
  /// What the core of a light running along a way is taken towards: white, which is what a light reads as. Where
  /// the page is dark it reads against the ground; where it is light it reads against the way's own line, which is
  /// drawn dark there ([`RUN.inkLine`]), as a glint on a dark thing does.
  glowTo: '#ffffff',
  /// The colour of the light running down a way where it is brightest ([`RUN.hue`]): a violet beside the running
  /// colour, apart from the red of what waits for the owner. Not of the default accent's hue, it is not turned to the
  /// accent picked, as the console's other colours of no accent's hue are not.
  pulse: '#b46cff',
  /// The accent: the mark on the map of where the item selected stands.
  chosen: '#3673f5',
  chosenRibbon: '#d0ddff',
  /// A column's bar on the map, light, so that the marks across it show.
  mapBar: '#7cc0fd',
  /// The colours of works, which tint their cards: running in the accent, green being kept for what is done; waiting
  /// for the calls of tools, in the yellow of a call under way, as the page says that state everywhere; paused, and
  /// waiting for something else, grey; stopped, for the owner or by the engine, red.
  running: '#3673f5',
  calling: '#eab308',
  paused: '#8b93a1',
  halted: '#e5484d',
  amber: '#b45309',
  red: '#e5484d',
  green: '#2f9e6e',
  /// The pointer's blue: the lines at the edges of the column under it, and the own colour of an item with no work of
  /// its own above a work, ringed and lit in it.
  over: '#6d9bf9',
  /// The words on a filled tag.
  onTag: '#ffffff',
  /// What is new to the owner: the line inside a card's left edge, and its mark on the map.
  unseen: '#3673f5',
  accent: '#3673f5',
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

/// The items, by their place in the tree, whose link to the one above them is on the way from one of `running` up to
/// its root: each link once, however many of them it is on; a root has none.
export function waysUp(running: Iterable<number>, parent: ArrayLike<number>): Set<number> {
  const on = new Set<number>();
  for (const k of running) {
    for (let i = k; i >= 0 && parent[i] >= 0; i = parent[i]) {
      if (on.has(i)) break;
      on.add(i);
    }
  }
  return on;
}

/// Whether an item under `item`, at any depth, folded away or not, is as `is` says; a shadow, whose item stands
/// elsewhere, is not under it.
export function beneath(item: Shown, is: (s: Shown) => boolean): boolean {
  return item.children.some((c) => !c.shadow && (is(c) || beneath(c, is)));
}

/// How strong the links between the items are drawn, as bright as the items not lit are (`dim`): fully while nothing is
/// lit, and no fainter than [`DIM_LINKS`] of their own strength.
export function linksStrength(dim: number): number {
  return DIM_LINKS + (1 - DIM_LINKS) * dim;
}

/// How strong each of the dashed `ways` is to be drawn, by the item whose link up it is: every one at full strength
/// while nothing is `lit`; while something is, one whose two ends are both lit at full strength, and the others as dim
/// as the links once the items are dimmed.
export function wayAims(ways: Iterable<number>, parent: ArrayLike<number>, lit: Set<number>): Map<number, number> {
  const aims = new Map<number, number>();
  for (const i of ways) aims.set(i, lit.size === 0 || (lit.has(i) && lit.has(parent[i])) ? 1 : linksStrength(DIM));
  return aims;
}

/// One frame's step, `step` milliseconds long, of a brightness from `was` towards `to`: about two thirds of the way in
/// [`FADE_MS`], going down or up alike, so that nothing lit jumps as the light moves or goes out.
export function eased(was: number, to: number, step: number): number {
  return was + (to - was) * (1 - Math.exp(-step / FADE_MS));
}

/// A soft light: the item it is behind, where its card stands (`gx`, and `gy` with `gh` half its height), and how far
/// past the card it reaches.
export type Light = { item: number; gx: number; gy: number; gh: number; reach: number };

/// Whether the page's ground goes under `item`, standing at `x`, and `y` with `h` half its height, over the `lights`:
/// where one reaches it and it has a box, or the light is its own; so that the lights show only round the cards, and an
/// item drawn with no box neither cuts another's light along a box no one sees nor shows its own light filling one.
export function grounded(item: number, x: number, y: number, h: number, boxed: boolean, lights: Light[]): boolean {
  return lights.some(
    (l) =>
      (boxed || l.item === item) && Math.abs(x - l.gx) < NODE_W + l.reach && Math.abs(y - l.gy) < h + l.gh + l.reach,
  );
}

/// A way's curve: the points it is drawn in, how far along it each of them stands, and the way square to it at each,
/// one pixel long, to which a strand beside it is moved out.
type Way = { points: [number, number][]; along: number[]; across: [number, number][] };

/// The light on a stretch `length` long that begins `dist` down from the left side of its root's card, the light's
/// head `head` down from it now: the pieces the light is drawn in, each from and to how far along the stretch it
/// stands, with how brightly it is drawn, from its tail (0) to its head (1). The stretch is a way between two cards,
/// or a card's own crossing; what falls outside it is cut, so that nothing of the light is drawn off it, and a
/// stretch the light has not reached or has gone past carries none.
export function lightOn(dist: number, length: number, head: number): { from: number; to: number; bright: number }[] {
  const pieces: { from: number; to: number; bright: number }[] = [];
  if (length <= 0) return pieces;
  for (let k = 0; k < RUN.lightPieces; k += 1) {
    const from = Math.max(dist, head - RUN.light + (RUN.light * k) / RUN.lightPieces);
    const to = Math.min(dist + length, head - RUN.light + (RUN.light * (k + 1)) / RUN.lightPieces);
    const bright = (k + 1) / RUN.lightPieces;
    if (to > from) pieces.push({ from: from - dist, to: to - dist, bright });
  }
  return pieces;
}

/// How far the light's head has run of a way `whole` long, `part` of the run gone from 0 to 1: slow at the start of
/// a run and faster towards its end ([`RUN.ease`]).
export function headAt(part: number, whole: number): number {
  return whole * part ** RUN.ease;
}

/// How brightly the spark where a way meets a card burns, from [`RUN.portRest`] to 1: at rest while the light is
/// elsewhere on the way, and at the full as the light's head goes through it. The spark stands `dist` down from the
/// left side of its root's card, and the light's head is `head` down from it now.
export function sparkAt(dist: number, head: number): number {
  const near = lightOn(dist - RUN.portNear, RUN.portNear * 2, head).reduce((a, { bright }) => Math.max(a, bright), 0);
  return RUN.portRest + (1 - RUN.portRest) * near;
}

/// How thick the way's line is drawn `u` along it, from 0 to 1, as a part of its width: thinnest at its ends.
export function thickAt(u: number): number {
  return RUN.ends + (1 - RUN.ends) * Math.sin(Math.PI * u);
}

/// How far out from the line the strands stand `u` along the way, from 0 to 1, as a part of how far they stand at the
/// most: none at either end, where the way meets a card and they gather into its line, rising steeply from there and
/// nearly level along the middle, so that between the ends they run beside the line and do not swell and narrow.
export function gatherAt(u: number): number {
  return Math.sqrt(Math.sin(Math.PI * Math.min(1, Math.max(0, u))));
}

/// Where strand `s` stands beside the line, from -1 to 1 across it, and how strongly it is drawn, as a part of the
/// light. The strands are scattered across the line and in strength by two sequences that never repeat, the golden
/// ratio's and the plastic number's, so that no two stand together and none follows its neighbour in order; the
/// outer ones are fainter, as the light thins out from the line.
export function strandOf(s: number): { side: number; own: number } {
  const side = 2 * (((s + 0.5) * 0.6180339887) % 1) - 1;
  const own = (0.35 + 0.65 * (((s + 0.5) * 0.7548776662) % 1)) * (1 - 0.5 * Math.abs(side));
  return { side, own };
}

/// Whether what is drawn as light is added to the ground: on the dark ground it is, as light is, so that its glow tells
/// on it and the line, its glow and its strands build up; on the light ground adding it would wipe it out, and it is
/// drawn as ink is.
export function adding(): boolean {
  return getComputedStyle(document.documentElement).colorScheme === 'dark';
}

/// The scene's colours as the page's theme has them. Those the page's tokens came from are read from the
/// tokens, which in light are the console's own; the console's others of the accent's hue are turned to the accent in
/// force, as the accent's own tones are (accent.ts). In dark, the colours the tokens have no step for are worked out
/// from the tokens and those turned colours, keeping their relations to the card and the ground, as the console had no
/// dark ground.
export function themed(): typeof LIGHT {
  const turn = turning(accentNow());
  const own = {
    unseen: turn(LIGHT.unseen, 'read'),
    accent: turn(LIGHT.accent, 'read'),
    over: turn(LIGHT.over, 'read'),
    branch: turn(LIGHT.branch, 'read'),
    mapBar: turn(LIGHT.mapBar, 'read'),
    hoverRibbon: turn(LIGHT.hoverRibbon, 'tint'),
    chosenRibbon: turn(LIGHT.chosenRibbon, 'tint'),
  };
  const t = {
    ink: token('--text', LIGHT.ink),
    muted: token('--muted', LIGHT.muted),
    hair: token('--line', LIGHT.hair),
    card: token('--card', LIGHT.card),
    page: token('--ground', LIGHT.page),
    chosen: token('--accent', LIGHT.chosen),
    running: token('--running', LIGHT.running),
    calling: token('--yellow', LIGHT.calling),
    halted: token('--red', LIGHT.halted),
    red: token('--red', LIGHT.red),
    green: token('--green', LIGHT.green),
    amber: token('--orange-text', LIGHT.amber),
    onTag: token('--on-colour', LIGHT.onTag),
  };
  if (getComputedStyle(document.documentElement).colorScheme !== 'dark') return { ...LIGHT, ...own, ...t };
  return {
    ...LIGHT,
    ...own,
    ...t,
    faint: between(t.muted, t.card, 0.3),
    pale: between(t.muted, t.card, 0.55),
    line: between(t.hair, t.muted, 0.35),
    edge: t.hair,
    branch: between(t.card, own.over, 0.45),
    hoverRibbon: between(t.card, own.over, 0.3),
    chosenRibbon: between(t.card, t.chosen, 0.3),
    mapBar: between(t.card, own.over, 0.6),
    paused: t.muted,
    other: '#656c76',
    otherAlpha: 0.85 * (0x33 / 255),
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

/// The mark of an item's fold on its node: the triangle of `FoldMark` in its box [`FOLD_OUT`] past the card's right
/// edge, its middle at the card's, to the right while what is under the item is folded away and turned down while it is
/// shown.
function foldMark(folded: boolean): Graphics {
  const at = (v: number, n: number) => v + (n % 2 === 0 ? NODE_W + FOLD_OUT : -FOLD_BOX / 2);
  return new Graphics().poly((folded ? FOLD_RIGHT : FOLD_DOWN).map(at)).fill(PALETTE.muted);
}

/// The colour of a work's age, three and no more: grey under half an hour, amber from half an hour to an hour, and red
/// past an hour, however long.
const TONE = () => ({
  success: PALETTE.muted,
  attention: PALETTE.amber,
  danger: PALETTE.red,
});

/// Where a card's second row writes the lines its work added and removed, from the card's left: past the strip, before
/// how long the work has gone on.
const LINES_X = NODE_W - 8 - 36 - 70;

/// The middle of a card's second row, the work's, from the card's middle, for a card `h` high.
const workRow = (h: number) => -h / 2 + high(1 + WORK_ROW) - (WORK_ROW * ROW) / 2 - 3;

/// What a work's state is to its node: running, waiting for the owner, waiting for the calls of tools, paused, stopped
/// by the engine for a problem, or waiting for something else.
export type Mood = 'running' | 'owner' | 'calling' | 'paused' | 'halted' | 'waiting';

/// A work's mood, from its state as every view names it (`stateOf`): one that waits for the owner, as the engine's one
/// list of it says, waits for the owner; one running, an act of the engine's own among it, runs; one that waits at its
/// cell for the calls of tools waits for them; a halted one, at a fault or stalled, is halted; one paused, by the
/// owner's pause or the owner's pause of everything, is paused; any other waits.
export function moodOf(w: Pick<ShownWork, 'now' | 'cells' | 'waits'>): Mood {
  const { tone, word } = stateOf(w);
  if (tone === 'owner') return 'owner';
  if (tone === 'running') return 'running';
  if (tone === 'calling') return 'calling';
  if (tone === 'failed') return 'halted';
  return word === STATE.paused ? 'paused' : 'waiting';
}

/// The cell a work is stopped at, for the owner or by the engine, and the colour its mark takes, red, as its card is
/// and as the work's strip anywhere on the page draws it ([`stoppedAt`]); none in any other state, where the mark's own
/// state says enough.
function here(w: ShownWork): { cell: string; colour: string } | null {
  const cell = stoppedAt(w);
  return cell === null ? null : { cell, colour: PALETTE.halted };
}

/// The colour of what an item's work does, which tints its card.
const WORK = (): Record<Mood, string> => ({
  running: PALETTE.running,
  // A work is running or stopped. One stopped for the owner is red, as one the engine stopped: both need the owner.
  owner: PALETTE.halted,
  // Waiting for its calls: not for the owner, so its card is not filled, but in the yellow every view says it in.
  calling: PALETTE.calling,
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
    Object.entries(CELL_LOOKS).map(([state, look]): [string, CellLook] => [
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
  /// Whether its ring is drawn: not where it is of the fill's own colour, which covers it already.
  ringed: boolean;
  pulses: boolean;
};

/// A card's strip as it was drawn: its marks, how many cells did not fit, and where its room ends; and the level its
/// work's cell runs at, under the card's number, with its colour and whether it is strong.
export type DrawnStrip = {
  marks: DrawnMark[];
  more: number;
  end: number;
  level: { text: string; colour: string; strong: boolean } | null;
};

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

/// The size of a node's title: the page's body text (`--size-body`).
const TITLE_SIZE = 13;

/// The styles of a node's words, by their colour: its title in the sans-serif face, and its numbers in the page's
/// monospace, smaller, strong where asked. Each is made from plain values once: one made by spreading another keeps
/// the other's fill.
type Styles = {
  title: (fill: string) => TextStyle;
  mono: (fill: string, strong?: boolean) => TextStyle;
};

/// The weight of strong words: the page's (`--weight-strong`).
const STRONG = '600' as const;

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
  const style = (fontFamily: string, fontSize: number, fill: string, strong = false) => {
    const key = `${fontFamily}|${fontSize}|${fill}|${strong}`;
    let s = made.get(key);
    if (s === undefined) {
      s = new TextStyle({ fontFamily, fontSize, fill, ...(strong ? { fontWeight: STRONG } : {}) });
      made.set(key, s);
    }
    return s;
  };
  return {
    title: (fill) => style(sans, size, fill),
    mono: (fill, strong) => style(mono, Math.round(size * 0.8), fill, strong),
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
  /// The funnels from each item to the items under it; and the ribbons of the items lit.
  private links = new Graphics();
  /// The lit ways from the items whose work runs up to their roots, drawn anew each frame over the funnels; and what
  /// of them is drawn over the cards instead: the spark where a way meets a card, and a card's edge as a light crosses
  /// it, which the cards, drawn over the ways, would else hide. Each is lines and their glow ([`Lit`]).
  private runLines = litLayer();
  private runEdges = litLayer();
  /// Whether what they draw is added to the ground, as the theme has it ([`adding`]).
  private adds = false;
  /// The soft lights behind the item selected and the item under the pointer, drawn anew each frame where they stand,
  /// blurred.
  private chosenGlow = new Graphics({ filters: [softBlur(CHOSEN_BLUR)] });
  private hoverGlow = new Graphics({ filters: [softBlur(SOFT_BLUR)] });
  /// The page's ground at each item a soft light reaches, over the lights and under the cards.
  private glowGround = new Graphics();
  /// The box of the mark of the fold under the pointer, filled, under the mark.
  private foldLight = new Graphics();
  /// The items, by their place in the tree, whose link to the one above them is on the way from an item whose work
  /// runs up to its root ([`waysUp`]); and how strong each of those links is drawn, eased from frame to frame towards
  /// what [`wayAims`] gives it.
  private runWays = new Set<number>();
  private runStrength = new Map<number, number>();
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
  /// Each item's own colour, which its soft light and its ring take when it is pointed at or selected, as its card
  /// shows it: its work's state's (red, the running colour, or grey); for an item with no work of its own above a work,
  /// whose edge is a light blue, the pointer's blue; else grey.
  private own: string[] = [];
  /// The parts of a node that change while it is shown: the marks of the cells running, which pulse, and the label of
  /// how long its work has gone on, with when the work started.
  private pulses = new Map<number, Graphics>();
  /// The beat the marks of cells running pulse at ([`beatOf`]).
  private beat = { ms: PULSE_MS, faint: DARK };
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
  /// How far the soft light round the item under the pointer itself has come in, as `glowing` says of its rings; those
  /// lit with it have none.
  private softLit = new Map<number, number>();
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
  /// The item whose fold's mark is under the pointer at the last frame, by its index in the tree, or none; and what the
  /// canvas says on pointing at it, as last written.
  private underFold = -1;
  private foldSaid = '';
  /// Each item's place in the tree, a shadow's left out; and for each item, its shadows: what a shadow points to and
  /// what points to an item.
  private real = new Map<number, number>();
  /// The item selected, by its number, or none: it and the way from it to its root, and from each of its shadows to
  /// theirs, stay lit in the accent until it is let go.
  private selected: number | null = null;
  /// The item selected by a click on its shadow while folds hid it, to bring to the middle once it is drawn; or none.
  private toFocus: number | null = null;
  /// Told the item selected each time it changes, or is drawn afresh: the window beside the tree shows it.
  onSelect: ((item: Shown | null) => void) | null = null;
  /// Told, at a press on the mark of an item's fold, the item's number and whether the items under it are now to be
  /// folded away: the console keeps the choice and draws the tree again.
  onFold: ((id: number, folded: boolean) => void) | null = null;
  /// The items as last shown, by their place in the tree.
  private shownItems: Shown[] = [];
  private shadowsOf = new Map<number, number[]>();
  private shadows: Uint8Array = new Uint8Array(0);
  private ready = false;
  private gone = false;
  /// The items as last given to `show`, drawn again in the new colours when the theme changes.
  private roots: Shown[] | null = null;
  /// What follows the size of what holds the canvas, the theme and the accent; let go when the scene is.
  private sized: ResizeObserver | null = null;
  private themes: { media: MediaQueryList; root: MutationObserver; accent: () => void } | null = null;

  async mount(host: HTMLElement): Promise<void> {
    Object.assign(PALETTE, themed());
    this.lighting();
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
    // Light or dark, the system's or the page's switch's, and the accent picked: drawn again in the theme's colours.
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    const root = new MutationObserver(this.retheme);
    root.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
    media.addEventListener('change', this.retheme);
    this.themes = { media, root, accent: onAccent(this.retheme) };
    this.world.addChild(
      this.links,
      this.runLines.far,
      this.runLines.near,
      this.runLines.line,
      this.hoverGlow,
      this.chosenGlow,
      this.glowGround,
      this.foldLight,
      this.layer,
      this.runEdges.far,
      this.runEdges.near,
      this.runEdges.line,
      this.marks,
    );
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

  /// Draws while the view is shown, and stops while another view hides it; shown again, it draws at the size of what
  /// holds it, which its observer follows.
  setShown(shown: boolean): void {
    if (!this.ready) return;
    if (shown) {
      this.app.queueResize();
      this.app.ticker.start();
    } else this.app.ticker.stop();
  }

  destroy(): void {
    this.gone = true;
    window.removeEventListener('pointermove', this.drag);
    window.removeEventListener('pointerup', this.up);
    this.sized?.disconnect();
    this.themes?.root.disconnect();
    this.themes?.media.removeEventListener('change', this.retheme);
    this.themes?.accent();
    if (this.ready) this.app.destroy(true, { children: true, texture: true });
  }

  /// Takes the theme's colours, and draws the tree again in them where it stands.
  private retheme = () => {
    if (!this.ready || this.gone) return;
    Object.assign(PALETTE, themed());
    this.lighting();
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
    // The items under an item folded are not drawn; what is under it counts at it, where the tree marks what is under
    // an item.
    const { tree, items } = indexTree(roots, (i) => (i.fold?.folded === true ? [] : i.children));
    const hides = (k: number, is: (s: Shown) => boolean) => items[k].fold?.folded === true && beneath(items[k], is);
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
    this.underFold = -1;
    this.rows = Float64Array.from(items, (i) =>
      i.work === null ? 1 : 1 + WORK_ROW,
    );
    this.pre = tree.columns.map((col) => {
      const p = new Float64Array(col.length + 1);
      for (let r = 0; r < col.length; r += 1) p[r + 1] = p[r] + this.rows[col[r]];
      return p;
    });
    const asks = (s: Shown) => s.asks > 0;
    const unseen = (s: Shown) => s.unseen;
    this.asking = tree.columns.map((col) => col.flatMap((i, r) => (asks(items[i]) || hides(i, asks) ? [r] : [])));
    this.fresh = tree.columns.map((col) => col.flatMap((i, r) => (unseen(items[i]) || hides(i, unseen) ? [r] : [])));
    // The items a work goes on on, and those above them up to their roots: from the roots one sees which branches
    // have work on.
    this.lit = new Uint8Array(items.length);
    const working = (s: Shown) => s.work !== null;
    items.forEach((i, k) => {
      if (!working(i) && !hides(k, working)) return;
      for (let p = k; p >= 0 && this.lit[p] === 0; p = tree.parent[p]) this.lit[p] = 1;
    });
    this.own = items.map((i, k) => {
      if (i.shadow) return PALETTE.paused;
      if (i.work !== null) return WORK()[moodOf(i.work)];
      return this.lit[k] === 1 ? PALETTE.over : PALETTE.paused;
    });
    // The items whose work has begun, and those above them: an item with nothing begun on it or under it is quieter.
    this.begun = new Uint8Array(items.length);
    const begun = (s: Shown) => s.begun;
    items.forEach((i, k) => {
      if (!begun(i) && !hides(k, begun)) return;
      for (let p = k; p >= 0 && this.begun[p] === 0; p = tree.parent[p]) this.begun[p] = 1;
    });
    this.pulses = new Map();
    this.beat = beatOf();
    const running = (s: Shown) => !s.shadow && s.work !== null && moodOf(s.work) === 'running';
    this.runWays = waysUp(
      items.flatMap((i, k) => (running(i) || hides(k, running) ? [k] : [])),
      tree.parent,
    );
    this.runStrength = new Map();
    ({ measures: this.measures, looks: this.looks } = cellLooks());
    this.drawnStrips = new Map();
    this.glowing = new Map();
    this.softLit = new Map();
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
    // An item a click on its shadow asked for, once the folds over it are open, comes to the middle of the screen.
    if (this.toFocus !== this.selected) this.toFocus = null;
    const goal = this.toFocus === null ? undefined : this.real.get(this.toFocus);
    if (goal !== undefined) {
      this.focus(tree.depth[goal], tree.rank[goal]);
      this.toFocus = null;
    }
    this.announce();
  }

  /// Of the item numbered `id`, where folds hide it, the folded items over it, from its root down; none where it is
  /// drawn, or stands nowhere in the tree.
  private foldsOver(id: number): number[] {
    const path: Shown[] = [];
    const find = (items: Shown[]): boolean =>
      items.some((s) => {
        if (s.shadow) return false;
        if (s.id === id) return true;
        path.push(s);
        if (find(s.children)) return true;
        path.pop();
        return false;
      });
    if (this.roots === null || this.real.has(id) || !find(this.roots)) return [];
    return path.filter((s) => s.fold?.folded === true).map((s) => s.id);
  }

  /// Asks for the item numbered `id` where folds hide it: the console is told to unfold each folded item over it, and
  /// the item is selected once it is drawn. Whether folds hid it.
  private unfoldTo(id: number): boolean {
    const over = this.foldsOver(id);
    if (over.length === 0 || this.onFold === null) return false;
    for (const f of over) this.onFold(f, false);
    this.selected = id;
    return true;
  }

  /// The node of an item: a card, white, or while a work goes on on it a faint tint of the colour of what the work does
  /// with an edge of a deeper shade of it; its edge the accent's tint above a work. On its first row its number in its
  /// state's colour, its title, and at the right how many things its work waits for from the owner, white on red; on
  /// the second row, for an item whose work goes on, under the number the level its cell runs at, the strip of the
  /// work's cells as the works page draws it, the lines the work added and removed where it changed any, and how long
  /// it has gone on. An item whose work has ended has its one row, with the lines its work changed at its right. An
  /// item with items under it has the mark of its fold just past its right edge, and, folded, how many items stand
  /// under it by how they stand, before its other marks.
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
    if (item.fold != null) view.addChild(foldMark(item.fold.folded));
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
    // mark a card has, as what the owner looks for first; its messages unread, white on the accent; its returns sent back,
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
    // What waits for the owner in red, the strongest, as the engine's one list counts it; how much is new to read in the
    // colour of the line at the card's left that says something is new.
    if (item.asks > 0) tag(String(item.asks), PALETTE.red, null, true);
    if (item.unread > 0) tag(String(item.unread), PALETTE.unseen, null, true);
    if (item.work !== null && item.work.sentBack > 0) tag(String(item.work.sentBack), PALETTE.muted, 'back', false);
    if (item.work === null && item.blocked > 0) tag(String(item.blocked), PALETTE.muted, 'lock', false);
    // Of an item whose work has ended, the lines that work added and removed, at the right of its one row, each in its
    // colour (`LINES_LOOKS`), as the item's window says them.
    const ended = item.work === null ? item.endedLines : null;
    if (ended != null) {
      const removed = new BitmapText({
        text: linesRemoved(ended.removed),
        style: s.mono(token(LINES_LOOKS.removed, PALETTE.red)),
      });
      const added = new BitmapText({
        text: linesAdded(ended.added),
        style: s.mono(token(LINES_LOOKS.added, PALETTE.green)),
      });
      const gap = added.width / Math.max(1, added.text.length);
      removed.position.set(right - removed.width, row1 - removed.height / 2);
      added.position.set(right - removed.width - gap - added.width, row1 - added.height / 2);
      view.addChild(added, removed);
      right -= removed.width + gap + added.width + 8;
    }
    // Folded, how many items stand under it by how they stand, before the other marks: each kind by its key, with its
    // count, framed in grey as the lock is, `open 1 done 5 cancelled 2`, a kind with none left out.
    const fold = item.fold;
    if (fold?.folded === true)
      for (const kind of [...STANDINGS].reverse())
        if (fold.under[kind] > 0) tag(`${kind} ${fold.under[kind]}`, PALETTE.muted, null, false);
    // The title ends where the badges begin.
    fit(title, right - title.x);
    if (item.work === null || mood === null) return view;
    const w = item.work;
    const row2 = workRow(h);
    // Under the number, the level the work's cell runs at, in its look (`levelLook`), as the item's window writes it
    // before its strip; a dash, muted, where the engine gives none.
    const look = w.level === null ? null : levelLook(w.level, w.raise);
    const colour = look === null ? PALETTE.muted : token(look.colour, PALETTE.ink);
    const strong = look?.strong ?? false;
    const level = new BitmapText({
      text: w.level === null ? NO_LEVEL.shown : levelWords(w.level, w.raise),
      style: s.mono(colour, strong),
    });
    level.position.set(x, row2 - level.height / 2);
    view.addChild(level);
    // The second row: the work's strip, as the works page draws it, from under the title, so that the number stands
    // alone in a column at the card's left, or from past the level where it is wider than that column; ending short of
    // the lines the work changed.
    const at = here(w);
    this.strip(view, item.id, k, w.cells, Math.max(title.x, x + level.width + 4), row2, LINES_X - 8, at);
    const drawn = this.drawnStrips.get(item.id);
    if (drawn !== undefined) drawn.level = { text: level.text, colour, strong };
    // Past the strip, where the work changed lines, those it added and removed, `+210 −40`, each in its colour
    // (`LINES_LOOKS`), a blank of the monospace apart, as the item's window says them.
    if (w.lines !== null) {
      const middle = row2;
      const added = new BitmapText({
        text: linesAdded(w.lines.added),
        style: s.mono(token(LINES_LOOKS.added, PALETTE.green)),
      });
      const removed = new BitmapText({
        text: linesRemoved(w.lines.removed),
        style: s.mono(token(LINES_LOOKS.removed, PALETTE.red)),
      });
      added.position.set(LINES_X, middle - added.height / 2);
      removed.position.set(LINES_X + added.width + added.width / added.text.length, middle - removed.height / 2);
      view.addChild(added, removed);
    }
    // How long it has gone on, at the right, its colour by its age; the label is kept up as time passes.
    const label = new BitmapText({ text: '', style: s.mono(PALETTE.muted) });
    view.addChild(label);
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
        ringed: look.edge !== null && look.edge !== look.fill,
        pulses: pulsing(c.state),
      };
    });
    const still = new Graphics();
    const running = new Graphics();
    for (const m of marks) {
      const g = m.pulses ? running : still;
      if (m.fill !== null) g.roundRect(m.x, m.y, m.width, m.height, m.corner).fill(m.fill);
      // The ring inside the mark's side, as the page's boxes draw a border: its line's middle half its width in. A ring of
      // the fill's own colour is not drawn over it: the fill covers it already, and drawn twice, the ring would show as
      // a border once the card is dimmed and both are drawn faint.
      const r = m.ring;
      if (m.edge !== null && m.ringed)
        g.roundRect(m.x + r / 2, m.y + r / 2, m.width - r, m.height - r, Math.max(0, m.corner - r / 2)).stroke({
          color: m.edge,
          width: r,
        });
    }
    view.addChild(still, running);
    if (more !== null) view.addChild(more);
    if (marks.some((m) => m.pulses)) this.pulses.set(k, running);
    this.drawnStrips.set(id, { marks, more: cells.length - shown, end, level: null });
  }

  /// What the card of item `id` drew for its work's cells, for the tests: each mark, where it stands from the card's
  /// left and its middle, its measures and colours, and whether it pulses; how many cells did not fit; where the room
  /// for the strip ends; and the level under the number. None for an item whose card has no work.
  drawnStrip(id: number): DrawnStrip | null {
    return this.drawnStrips.get(id) ?? null;
  }

  /// Where the card of item `id` stands on the canvas, its left side and its middle, for the tests; none for an item
  /// the tree does not draw.
  cardAt(id: number): { x: number; y: number } | null {
    const t = this.tree;
    const i = this.real.get(id);
    if (t === null || i === undefined) return null;
    const d = t.depth[i];
    const line = this.app.screen.height / 2;
    return { x: this.left + d * PITCH, y: line + (this.middle(d, t.rank[i]) - this.columns[d].at) * ROW };
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

  /// The way from `(px, py)` to `(cx, cy)`, bent as a funnel is: the points its curve is drawn in, and how far along
  /// it each of them stands, so that a length of it can be taken by how far along the way it lies.
  private wayOf(px: number, py: number, cx: number, cy: number): Way {
    const bend = (cx - px) / 2;
    const points = Array.from({ length: RUN.pieces + 1 }, (_, n): [number, number] => {
      const u = n / RUN.pieces;
      const v = 1 - u;
      return [
        v * v * v * px + 3 * v * v * u * (px + bend) + 3 * v * u * u * (cx - bend) + u * u * u * cx,
        v * v * v * py + 3 * v * v * u * py + 3 * v * u * u * cy + u * u * u * cy,
      ];
    });
    const along = [0];
    for (let n = 1; n <= RUN.pieces; n += 1) {
      along.push(along[n - 1] + Math.hypot(points[n][0] - points[n - 1][0], points[n][1] - points[n - 1][1]));
    }
    const across = points.map((_, n): [number, number] => {
      const [a, b] = [points[Math.max(0, n - 1)], points[Math.min(RUN.pieces, n + 1)]];
      const run = Math.hypot(b[0] - a[0], b[1] - a[1]) || 1;
      return [-(b[1] - a[1]) / run, (b[0] - a[0]) / run];
    });
    return { points, along, across };
  }

  /// How wide a lit line is drawn, wherever it runs: as it is where the page is dark, and wider where the page is
  /// light, where it is a line of ink that the glint must read within ([`RUN.inkLine`]).
  private get lineWidth(): number {
    return this.adds ? RUN.line : RUN.line * RUN.inkLine;
  }

  /// How wide the glint of a light is drawn, as a part of the line it runs on: the whole of it where the page is
  /// dark, where the line is itself a light; narrower where the page is light, where it has to read within the
  /// line, which is drawn dark there.
  private get glint(): number {
    return this.adds ? 1 : RUN.inkGlint;
  }

  /// How the lit ways are laid on the page: added to the ground where it is dark, as light is, and as ink where it
  /// is light ([`adding`]).
  private lighting() {
    this.adds = adding();
    const how = this.adds ? 'add' : 'normal';
    for (const lit of [this.runLines, this.runEdges]) {
      lit.line.blendMode = how;
      // A layer blurred is laid on the page by its blur, which takes the blend from its filter and not its own.
      lit.nearBlur.blendMode = how;
      lit.farBlur.blendMode = how;
    }
  }

  /// Lays a light over the cards as a way's line is laid, with its glow: the line itself, and the glow close round it
  /// and further out, so that the light reaches past the line it lies along and falls away with the distance from it.
  /// `lay` draws that line on the layer it is given, once for the line and once for each glow; `wide` is the part of a
  /// way's widths they take, and `alpha` the strength of all of them. The glow is fainter where the page is light, as
  /// a way's is ([`RUN.inkRest`]).
  private coats(lay: (g: Graphics) => void, colour: number, core: number, alpha: number, wide: number, nose = false) {
    const coat = (g: Graphics, width: number, strength: number, paint: number) => {
      lay(g);
      const cap = nose ? 'round' : 'butt';
      g.stroke({ color: paint, width: width * wide, alpha: alpha * strength, cap });
    };
    const rest = this.adds ? 1 : RUN.inkRest;
    coat(this.runEdges.far, RUN.far, RUN.farAlpha * rest, colour);
    coat(this.runEdges.near, RUN.near, RUN.nearAlpha * rest, colour);
    coat(this.runEdges.line, this.lineWidth, RUN.lineAlpha, core);
  }

  /// The light where a way meets a card, at `(x, y)` on the card's edge, `half` its height, the way coming in against
  /// `inward`, at `strength`, `burn` from 0 to 1 as the light goes through it: it lies along the edge the way meets,
  /// burning brighter as the light comes and fading out as it goes each way along it, in the light's colour as it goes
  /// through (`tint`). Along the way itself the way's own line carries it.
  private spark(
    x: number,
    y: number,
    half: number,
    inward: number,
    tint: (bright: number) => number,
    line: number,
    glint: number,
    strength: number,
    burn: number,
  ) {
    // The glint comes only with the light itself: at rest the way's end is lit, not glinting.
    const going = Math.max(0, (burn - RUN.portRest) / (1 - RUN.portRest));
    const reach = RUN.portHold * half;
    // The flare's own line is white where the page is dark, the hottest of the light; where it is light, the way's
    // ink line, which the glint reads within.
    const white = Number.parseInt(PALETTE.glowTo.slice(1), 16);
    const core = this.adds ? white : line;
    // The way's line turning into the edge, up and down it, from where the way comes in from outside the card.
    const turn = (g: Graphics) => {
      for (const side of [-1, 1]) {
        g.moveTo(x - inward * RUN.fillet, y).quadraticCurveTo(x, y, x, y + side * RUN.fillet);
      }
    };
    this.coats(turn, tint(going), core, strength * burn, RUN.portWide);
    for (let n = 0; n < RUN.portPieces; n += 1) {
      const [near, far] = [(reach * n) / RUN.portPieces, (reach * (n + 1)) / RUN.portPieces];
      const fade = (1 - (n + 0.5) / RUN.portPieces) ** RUN.portFall;
      const lay = (g: Graphics) => {
        for (const side of [-1, 1]) g.moveTo(x, y + side * near).lineTo(x, y + side * far);
      };
      this.coats(lay, tint(going), core, strength * burn * fade, RUN.portWide);
      // The glint is laid only where the page is light: where it is dark the line under it is the light's own near
      // white already, and a glint over it would only burn the card's end out.
      if (going > 0 && !this.adds) {
        lay(this.runEdges.line);
        this.runEdges.line.stroke({
          color: glint,
          width: RUN.lightLine * RUN.portWide * this.glint,
          alpha: strength * going * fade * RUN.lightAlpha,
          cap: 'butt',
        });
      }
    }
    // The point itself, white, with its glow close in and further out, wider as the light goes through it. Only where
    // the page is dark: on a light page a white point is no light, and the way's ink line holds the point there.
    if (this.adds) {
      const wide = RUN.spot * (1 + going);
      const glow = strength * burn * RUN.spotGlow;
      const tall = RUN.spotTall;
      const out = { color: tint(going), alpha: Math.min(1, glow / 2) };
      this.runEdges.far.ellipse(x, y, wide * 3, wide * 3 * tall).fill(out);
      this.runEdges.near.ellipse(x, y, wide * 1.5, wide * 1.5 * tall).fill({ color: white, alpha: Math.min(1, glow) });
      this.runEdges.line.ellipse(x, y, wide, wide * Math.min(2, tall)).fill({ color: white, alpha: strength * burn });
    }
  }

  /// A card's edge as the light crosses it, from the card's left side to its right: the card at `x` with its middle
  /// at `y` and `half` its height, and the pieces of the light `over` its crossing, each where it stands along the
  /// crossing. Each piece lights the card's top and bottom edges between the two places it stands at, and the card's
  /// own left or right edge where it is at that side, in the light's colour at that piece (`tint`).
  private across(
    x: number,
    y: number,
    half: number,
    over: { from: number; to: number; bright: number }[],
    tint: (bright: number) => number,
    line: number,
    glint: number,
    strength: number,
  ) {
    over.forEach(({ from, to, bright }, n) => {
      const [x0, x1] = [x + from, x + to];
      const nose = n === over.length - 1 && to < NODE_W - NEAR;
      const lay = (g: Graphics) => {
        g.moveTo(x0, y - half).lineTo(x1, y - half);
        g.moveTo(x0, y + half).lineTo(x1, y + half);
        if (from <= 0) g.moveTo(x, y - half).lineTo(x, y + half);
        if (to >= NODE_W) g.moveTo(x + NODE_W, y - half).lineTo(x + NODE_W, y + half);
      };
      this.coats(lay, tint(bright), line, strength * bright, RUN.edgeWide, nose);
      // The glint within that line, as on a way, and only where the page is light: where it is dark the line under it
      // is the light's own near white already.
      if (!this.adds) {
        lay(this.runEdges.line);
        this.runEdges.line.stroke({
          color: glint,
          width: RUN.lightLine * RUN.edgeWide * this.glint,
          alpha: strength * bright * RUN.lightAlpha,
          cap: nose ? 'round' : 'butt',
        });
      }
    });
  }

  /// How far along `way` the place stands that is `across` pixels across the page from where it begins: its curve
  /// goes rightwards all the way, so that each place across the page is at one place along it.
  private arcAt(way: Way, across: number): number {
    const { points, along } = way;
    const at = points[0][0] + across;
    let n = 0;
    while (n < RUN.pieces - 1 && points[n + 1][0] < at) n += 1;
    const span = points[n + 1][0] - points[n][0];
    const k = span > 0 ? Math.min(1, Math.max(0, (at - points[n][0]) / span)) : 0;
    return along[n] + (along[n + 1] - along[n]) * k;
  }

  /// Adds to `g` the length of `way` from `from` to `to` along it, to be stroked after: on the way itself, or, where
  /// `out` is given, beside it, each place moved square to the way as many pixels as `out` gives for how far along
  /// the way it stands.
  private alongWay(g: Graphics, way: Way, from: number, to: number, out?: (at: number) => number) {
    const { points, along, across } = way;
    // The place that stands `at` along the curve, in the piece from point `n`.
    const point = (n: number, at: number): [number, number] => {
      const len = along[n + 1] - along[n];
      const k = len > 0 ? (at - along[n]) / len : 0;
      const on = (i: number, of: [number, number][]) => of[n][i] + (of[n + 1][i] - of[n][i]) * k;
      const by = out === undefined ? 0 : out(at);
      return [on(0, points) + on(0, across) * by, on(1, points) + on(1, across) * by];
    };
    let n = 0;
    while (n < RUN.pieces - 1 && along[n + 1] <= from) n += 1;
    g.moveTo(...point(n, from));
    while (n < RUN.pieces - 1 && along[n + 1] < to) {
      n += 1;
      g.lineTo(...point(n, along[n]));
    }
    g.lineTo(...point(n, to));
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

  /// Draws one frame `ms` on from the last drawn, without waiting for the browser to give one: what the scene does
  /// over a span of time, drawn in one step. The browser gives a frame when it will and gives none at all to a page it
  /// does not show, so a test that waits for them waits on the machine's load; stepping, it waits on nothing.
  step(ms = 16): void {
    if (this.tree === null) return;
    this.draw(this.tree, (this.lastDraw > 0 ? this.lastDraw : performance.now()) + ms);
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
  /// or a deeper blue while the pointer is over it, marked red where works wait for the owner, and in the accent where
  /// the item selected stands.
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
    // Where the item selected stands, the accent across its column's bar, over every other mark, a little wider, so that
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
    return this.beside(t, x, y, 0, NODE_W);
  }

  /// The item whose fold's mark is under the pointer, by its index in the tree, or none: of an item with items under
  /// it, the room past its card's right edge as far as [`FOLD_REACH`], as high as the card.
  private foldPointed(t: Tree, x: (d: number) => number, y: (d: number, rank: number) => number): number {
    const i = this.beside(t, x, y, NODE_W, NODE_W + FOLD_REACH);
    return i >= 0 && this.shownItems[i].fold != null ? i : -1;
  }

  /// The item of column `d` level with the pointer, by its index in the tree, where the pointer is from `from` to `to`
  /// pixels right of the column's left side and within the item's height; or none.
  private beside(
    t: Tree,
    x: (d: number) => number,
    y: (d: number, rank: number) => number,
    from: number,
    to: number,
  ): number {
    const p = this.pointer;
    const d = this.hovered;
    if (p === null || d < 0 || p.x < x(d) + from || p.x > x(d) + to) return -1;
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
    // A click on the mark of an item's fold folds the items under it away, or shows them again, and selects nothing;
    // the item's column becomes the one the others follow, so that the items shown under it come level with it.
    const f = this.underFold;
    const fold = f < 0 ? null : this.shownItems[f].fold;
    if (fold != null) {
      const c = this.tree.depth[f];
      this.driver = c;
      this.columns[c].aim = this.columns[c].at;
      this.onFold?.(this.ids[f], !fold.folded);
      return;
    }
    // A click on an item selects it, and on it again, or on no item, lets it go. A click on a shadow selects the item
    // it points to and brings it to the middle of the screen, so that the columns come to where it stands.
    const i = this.under;
    if (i < 0) this.selected = null;
    else if (this.shadows[i] === 1) {
      const to = this.real.get(this.ids[i]);
      // Where folds hide the item it points to, they are opened, and the item comes to the middle once it is drawn.
      if (to === undefined) {
        if (this.unfoldTo(this.ids[i])) this.toFocus = this.ids[i];
        return;
      }
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
    // An item folds hide is selected once the folds over it are opened and it is drawn.
    if (id !== null && !this.real.has(id) && this.unfoldTo(id)) return;
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
    const beat = (now % this.beat.ms) / this.beat.ms;
    const fade = beat < 1 - FADE ? 0 : Math.sin((Math.PI * (beat - (1 - FADE))) / FADE) ** 2;
    const pulse = 1 - (1 - this.beat.faint) * fade;
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
    this.links.fill({ color: PALETTE.other, alpha: PALETTE.otherAlpha * linksStrength(this.dim) });
    // The item under the pointer and those above it up to its root, each joined to the one above by a ribbon of light
    // blue and ringed in its own colour, under the selection's rings alone, the item itself glowing too; each comes in
    // quickly, and fades out once the pointer has left.
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
    this.underFold = this.under >= 0 ? -1 : this.foldPointed(t, x, y);
    // The mark of the fold under the pointer: its box filled, the pointer a hand, and what it does said on pointing.
    this.foldLight.clear();
    const fold = this.underFold < 0 ? null : this.shownItems[this.underFold].fold;
    if (fold != null) {
      const [d, r] = [t.depth[this.underFold], t.rank[this.underFold]];
      const side = FOLD_BOX + 2 * FOLD_PAD;
      this.foldLight
        .roundRect(x(d) + NODE_W + FOLD_OUT - FOLD_PAD, y(d, r) - side / 2, side, side, CARD_CORNER)
        .fill(PALETTE.hoverRibbon);
    }
    const said = fold == null ? '' : FOLD_SAYS[fold.folded ? 'folded' : 'shown'];
    if (said !== this.foldSaid) {
      this.foldSaid = said;
      this.app.canvas.title = said;
      this.app.canvas.style.cursor = said === '' ? '' : 'pointer';
    }
    const chain = lit(this.under);
    // The item selected, the same way, in the accent; only the item itself glows, those lit with it are only ringed,
    // so that it stands out among them.
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
    this.dim = eased(this.dim, aim, step);
    for (const i of drawn) {
      const view = this.views[i];
      const to = aim === 1 || focus.has(i) || chosen.has(i) ? 1 : DIM;
      view.alpha = eased(view.alpha, to, step);
    }
    // The way from each item whose work runs up to its root, lit in the colour of a running card's edge, with one
    // length of light running along it from the root's side to the item's: each link from an item to the one above it
    // drawn once, however many running items it is on. While something is lit, the links on its way stand at full
    // strength and the others dim with the items they join; each eases to its strength as the items do, so that none
    // jumps as the light moves or goes out.
    const shining = new Set([...focus, ...chosen]);
    for (const [i, to] of wayAims(this.runWays, t.parent, shining)) {
      this.runStrength.set(i, eased(this.runStrength.get(i) ?? to, to, step));
    }
    for (const lit of [this.runLines, this.runEdges]) {
      for (const g of [lit.line, lit.near, lit.far]) g.clear();
      blurTo(lit.nearBlur, RUN.nearBlur);
      blurTo(lit.farBlur, RUN.farBlur);
    }
    const runColour = mix(PALETTE.card, WORK().running, EDGE_RUNNING);
    const glintColour = mix(between(PALETTE.card, WORK().running, EDGE_RUNNING), PALETTE.glowTo, RUN.core);
    // The colour the way's glow and strands are drawn in where the light is `bright`, from the way at rest to the
    // light's head: the running colour, going towards the light's own as it brightens ([`RUN.hue`]); the running
    // colour whole, with none of the card in it, so that what is added to a dark ground is the colour itself and not a
    // wash of the card, which reads grey. Taken once for each of the lengths the light is drawn in.
    const tints = Array.from({ length: RUN.lightPieces + 1 }, (_, k) =>
      mix(WORK().running, PALETTE.pulse, (RUN.hue * k) / RUN.lightPieces),
    );
    const tint = (bright: number) => tints[Math.round(Math.min(1, Math.max(0, bright)) * RUN.lightPieces)];
    // The way's own line: the light's near white where the page is dark, where the line is itself a light; the
    // running colour where it is light, where the line is what the glint reads against.
    const lineColour = this.adds ? glintColour : runColour;
    const lineWide = this.lineWidth;
    const lineAlpha = this.adds ? RUN.lineAlpha : RUN.inkAlpha;
    const glintWide = this.glint;
    // Each way's curve, how far it goes across the page, and how far across from the left side of the roots' cards
    // it begins ([`RUN.light`]).
    const ways = new Map<number, { way: Way; len: number; dist: number }>();
    const wayDown = (i: number): { way: Way; len: number; dist: number } => {
      const had = ways.get(i);
      if (had !== undefined) return had;
      const p = t.parent[i];
      const d = t.depth[i];
      const [px, cx] = [x(d - 1) + NODE_W, x(d)];
      const at = { way: this.wayOf(px, y(d - 1, t.rank[p]), cx, y(d, t.rank[i])), len: cx - px, dist: px - x(0) };
      ways.set(i, at);
      return at;
    };
    // How far the furthest item whose work runs stands from its root, its own card counted, and where the light has
    // run to now: every root's light runs from its own card down every way under it, all of them in step and
    // beginning again together once the furthest of all has been reached.
    let far = 0;
    for (const i of this.runStrength.keys()) {
      const { len, dist } = wayDown(i);
      far = Math.max(far, dist + len + NODE_W);
    }
    const rest = this.adds ? 1 : RUN.inkRest;
    const whole = far + RUN.light;
    // Where the light has run to now, and nothing where the run is over and the next has not begun: a head a whole
    // light past the end of the ways leaves none of it on any of them.
    const part = (now % (RUN.beat + RUN.dark)) / RUN.beat;
    const head = part <= 1 ? headAt(part, whole) : whole + RUN.light;
    // Each card on a way is lit once as the light crosses it, and each end of a card lit once where the ways meet
    // it, however many ways it stands on.
    const crossed = new Set<number>();
    const sparked = new Set<number>();
    for (const [i, strength] of this.runStrength) {
      const p = t.parent[i];
      const d = t.depth[i];
      const [px, cx] = [x(d - 1) + NODE_W, x(d)];
      if (cx < 0 || px > width) continue;
      const [py, iy] = [y(d - 1, t.rank[p]), y(d, t.rank[i])];
      if (Math.max(py, iy) < 0 || Math.min(py, iy) > height) continue;
      const { way, len, dist } = wayDown(i);
      // How long the way's curve is, and how far along it the place stands that is `across` the page from its start.
      const curve = way.along[RUN.pieces];
      const on = (across: number) => this.arcAt(way, across);
      const lit = (g: Graphics, from: number, to: number, w: number, alpha: number, paint: number, end = false) => {
        this.alongWay(g, way, from, to);
        g.stroke({ color: paint, width: w, alpha: strength * alpha, cap: end ? 'round' : 'butt' });
      };
      const shine = this.runLines;
      // The way itself, as a wire of light: its glow further out and close in, then the line, drawn in lengths each
      // at the width of its own middle so that it is thinner where it meets the cards. The glow is fainter where the
      // page is light ([`RUN.inkRest`]).
      lit(shine.far, 0, curve, RUN.far, RUN.farAlpha * rest, tint(0), true);
      lit(shine.near, 0, curve, RUN.near, RUN.nearAlpha * rest, tint(0), true);
      for (let n = 0; n < RUN.wide; n += 1) {
        const thick = lineWide * thickAt((n + 0.5) / RUN.wide);
        lit(shine.line, (curve * n) / RUN.wide, (curve * (n + 1)) / RUN.wide, thick, lineAlpha, lineColour);
      }
      // The light running down the way: the same line, brighter and wider where the light is, as a wire is where the
      // current is in it. It is a drop drawn out along the way, its head round and widest, thinning away behind it;
      // the head is round only where it stands on this way, and not where the way cuts the light short.
      const drop = lightOn(dist, len, head);
      drop.forEach(({ from, to, bright }, n) => {
        const nose = n === drop.length - 1 && to < len - NEAR;
        const thick = RUN.lightLine * (RUN.ends + (1 - RUN.ends) * bright) * glintWide;
        const [a, b] = [on(from), on(to)];
        lit(shine.far, a, b, RUN.lightFar * bright, bright * RUN.lightFarAlpha * rest, tint(bright), nose);
        lit(shine.near, a, b, RUN.lightNear * bright, bright * RUN.lightNearAlpha * rest, tint(bright), nose);
        lit(shine.line, a, b, thick, bright * RUN.lightAlpha, glintColour, nose);
      });
      // The strands the line is made of: each gathered close round the line along the whole way, faint, and parted
      // from it where the light is, as strong as the light is there and in its colour there; parted, each runs beside
      // the line at its own distance, and both gather into it at the way's ends ([`gatherAt`]).
      for (let s = 0; s < RUN.strands; s += 1) {
        const { side, own } = strandOf(s);
        const strand = (from: number, to: number, by: number, bright: number, colour: number) => {
          this.alongWay(shine.line, way, from, to, (at) => side * by * gatherAt(at / curve));
          const alpha = strength * RUN.strandAlpha * own * bright * rest;
          shine.line.stroke({ color: colour, width: RUN.strand, alpha, cap: 'butt' });
        };
        strand(0, curve, RUN.spreadRest, RUN.strandRest, tint(0));
        for (const { from, to, bright } of drop) {
          const comes = Math.min(1, (1 - bright + 1 / RUN.lightPieces) / Math.max(RUN.strandIn, 1e-6));
          strand(on(from), on(to), RUN.spread, bright * comes, tint(bright));
        }
      }
      // The spark at both ends of the way, which burns up as the light's head goes through it; and the edge of the
      // card the way ends at, lit from its left side to its right as the light crosses it. Where the way comes from
      // a root, the root's own card is crossed before any way.
      // The light where the way meets each card: on the parent's right side, going into it, and on the item's left.
      // Each card's own end is lit once, however many running ways meet it there.
      if (!sparked.has(p * 2)) {
        sparked.add(p * 2);
        const ph = half(d - 1, t.rank[p]);
        this.spark(px, py, ph, -1, tint, lineColour, glintColour, strength, sparkAt(dist, head));
      }
      if (!sparked.has(i * 2 + 1)) {
        sparked.add(i * 2 + 1);
        const ih = half(d, t.rank[i]);
        this.spark(cx, iy, ih, 1, tint, lineColour, glintColour, strength, sparkAt(dist + len, head));
      }
      if (t.parent[p] < 0 && !crossed.has(p)) {
        crossed.add(p);
        const over = lightOn(0, NODE_W, head);
        this.across(x(d - 1), py, half(d - 1, t.rank[p]), over, tint, lineColour, glintColour, strength);
      }
      if (!crossed.has(i)) {
        crossed.add(i);
        const over = lightOn(dist + len, NODE_W, head);
        this.across(x(d), iy, half(d, t.rank[i]), over, tint, lineColour, glintColour, strength);
      }
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
    ease(this.softLit, new Set(this.under < 0 ? [] : [this.under]));
    for (const [i, a] of this.glowing) {
      const p = t.parent[i];
      if (p < 0) continue;
      const d = t.depth[i];
      const [py, ph, iy, ih] = [y(d - 1, t.rank[p]), half(d - 1, t.rank[p]), y(d, t.rank[i]), half(d, t.rank[i])];
      this.funnel(x(d - 1) + NODE_W, py - ph, py + ph, x(d), iy - ih, iy + ih);
      this.links.fill({ color: PALETTE.hoverRibbon, alpha: a });
    }
    // The soft lights, each in its item's own colour: behind the item selected, and behind the item under the
    // pointer, as far as it has come in; over them, the page's ground where [`grounded`] says, so that the light shows
    // only round the cards and never through one drawn faint.
    this.chosenGlow.clear();
    this.hoverGlow.clear();
    const lights: Light[] = [];
    const softLight = (g: Graphics, i: number, out: number, blur: number, colour: string, on: number) => {
      const d = t.depth[i];
      const [gx, gy, gh] = [x(d), y(d, t.rank[i]), half(d, t.rank[i])];
      g.roundRect(gx - out, gy - gh - out, NODE_W + 2 * out, 2 * gh + 2 * out, CARD_CORNER + out).fill({
        color: colour,
        alpha: SOFT_ALPHA * on,
      });
      lights.push({ item: i, gx, gy, gh, reach: out + BLUR_ROOM * blur });
    };
    if (picked >= 0) softLight(this.chosenGlow, picked, CHOSEN_SPREAD, CHOSEN_BLUR, this.own[picked], 1);
    for (const [i, a] of this.softLit) softLight(this.hoverGlow, i, SOFT_SPREAD, SOFT_BLUR, this.own[i], a);
    this.glowGround.clear();
    if (lights.length > 0) {
      for (const j of drawn) {
        const d = t.depth[j];
        const [jx, jy, jh] = [x(d), y(d, t.rank[j]), half(d, t.rank[j])];
        const boxed = this.shadows[j] === 1 || !this.shownItems[j].deliver;
        if (!grounded(j, jx, jy, jh, boxed, lights)) continue;
        this.glowGround.roundRect(jx, jy - jh, NODE_W, 2 * jh, CARD_CORNER).fill(PALETTE.page);
      }
    }
    // The rings, each in its item's own colour: of the item under the pointer and those above it, as far as each has
    // come in; over them, of the item selected and those above it, the item selected's wider, so that the selection
    // stands out under the pointer too.
    for (const [i, a] of this.glowing) {
      const d = t.depth[i];
      const [gx, gy, gh] = [x(d), y(d, t.rank[i]), half(d, t.rank[i])];
      this.marks
        .roundRect(gx, gy - gh, NODE_W, 2 * gh, CARD_CORNER)
        .stroke({ color: this.own[i], width: RING, alpha: a });
    }
    for (const i of chosen) {
      const d = t.depth[i];
      const [gx, gy, gh] = [x(d), y(d, t.rank[i]), half(d, t.rank[i])];
      const ring = i === picked ? CHOSEN_RING : RING;
      this.marks.roundRect(gx, gy - gh, NODE_W, 2 * gh, CARD_CORNER).stroke({ color: this.own[i], width: ring });
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
