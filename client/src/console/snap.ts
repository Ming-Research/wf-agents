// The console's tree in columns, drawn in WebGL by PixiJS. One column for each level, from the roots at the left; in
// each, the level's items one under another, those under one item standing apart from those under the next. Every item
// has a fixed place, which the tree alone decides ([`layOut`]): an item stands in the middle of the items under it,
// where its funnel opens onto them, and each item as high as its column leaves room for. The tree is one canvas: the
// wheel moves it up and down and sideways, a drag on it moves it too, and the wheel with Shift held, or a pinch, makes
// it larger or smaller round the pointer. Asked to, the whole tree is made small enough to fit the screen, and asked
// again, it goes back to the size it was, round the pointer; a click on an item while the whole tree is shown brings
// that item to the middle at its full size. While the tree is drawn smaller than its full size, the cards near the
// pointer grow where they stand, the one under it to its full size, the nearer the larger and over the others; the
// funnels, the ways and the dots stay as they are. When the folds change, the item selected, or else the item under
// the pointer, or else the item nearest the middle of the screen, stays where it is on the screen, and the tree moves
// round it.
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
// waits. Either way its cell's mark, in the yellow of a call, pulses as a running cell's does. A press on the funnel
// from an item to the items under it folds them away, as the console is told, and selects nothing: the funnel and the
// cards under it draw in to a small grey dot just past the item's card, where the funnel began, and a press on the dot
// grows them back out of it. Where a running item is folded in it, the dot ends the lit way, in its colour and glow;
// while the fold turns, the lit way draws in and grows out with the cards. Pointing at the funnel or the dot fills it
// with the pointer's tint and says what a press does and how many items stand under the item by kind. What a fold hides
// counts at its item wherever the tree marks what is under an item: the edge above a work, the title of what has begun,
// and the map. A click on the shadow of an item folded away, or the selection of one, has the console open the folds
// over it, and the item is selected once it is drawn. Asked to, as a click on the item's title in its window asks, the
// item selected flashes once, whether or not the tree moved to bring it into view: the light along its edges grows
// stronger, and eases back to rest.
// What stands is drawn once and kept until something in it changes: a tree read again that draws the same makes
// nothing again, and one that changed makes again only the cards that changed; a frame in which only the lights, the
// pulses and the ages move draws only those, the cards standing in a group of their own that is drawn as it was. The
// glow of the lit ways is drawn and blurred on texels coarser than the canvas, a part of its blur's strength wide
// ([`BLUR_TEXEL`]), and each layer of glow is blurred over the room its lights take in a frame, and not at all in a
// frame where it has none. Nothing is drawn while the page or the console's view is hidden.
//
// A card's second row draws the level the work's cell runs at, the works page's strip of the work's cells, and the
// lines the work changed; an ended item's one row, the lines its work changed. What the owner has not read of an item
// (new or changed, and its replies) is the engine's word (`Summary.fresh`, `Summary.repliesUnread`); what the interface
// has nothing for yet is read from the console's made-up data (`mock.ts`). The colours are read
// from the page's tokens, so that the canvas follows light and dark; the canvas follows the size of what holds it,
// besides the window's; the links dim with the items they join while an item is lit (`DIM_LINKS`); and the way from
// each item whose work runs up to its root is lit, with a light running down it (`RUN`).

import {
  Application,
  BitmapFontManager,
  BitmapText,
  BlurFilter,
  Container,
  FillGradient,
  Graphics,
  TextStyle,
} from 'pixi.js';

import { accentNow, onAccent, turning } from '../accent';
import { LINES_LOOKS, levelLook, levelPointed, levelWords } from '../marks';
import { MARK, RUN_STATES, type RunState, cellRunState, pulses } from '../works/state-marks';
import type { CellNow, CellState } from '../works-api';
import {
  NO_LEVEL,
  itemName,
  linesAdded,
  linesRemoved,
  stateOf,
  stateSaid,
  stoppedAt,
  waitsItsTurn,
} from '../works/words';
import { type Fold, STANDINGS, type Shown, age, somethingUnread } from './items';
import { indexTree, type Tree } from './tree';

// A font the canvas builds as it meets each new character measures, unless told not to, the kerning of that character
// against every character it holds, two measures a pair: with the thousands of Chinese characters the titles bring that
// grows to millions of measures, and on 2026-11-26 it held the owner's page unresponsive for seconds at a time. Chinese
// text has no kerning, and the page's Latin text is names and numbers, so no font of the canvas measures it.
BitmapFontManager.defaultOptions.skipKerning = true;

/// How a scene lets its renderer go: its canvas taken off the page, and nothing more. `true` would also free what
/// PixiJS keeps once for every renderer on the page, and every other project's scene, which shares it, would then
/// throw at each frame and draw nothing.
const KEEP_SHARED = { removeView: true, releaseGlobalResources: false };

export const NODE_W = 300;
/// A row's height, and the height of a node of one row: an item with a work going on has a second row, the work's, this
/// part of a row high, so that the work sits close under the title.
const NODE_H = 26;
export const ROW = 32;
const WORK_ROW = 0.55;
export const PITCH = 370;
/// The room between the items of one item and those of the next in a column, in rows.
const APART = 0.7;
/// The map at the top left: as wide and as tall as this for every project, set this far in from the corner and padded
/// this much inside; the whole tree is drawn in it as small as it must be to fit, as tall for its width as it is, in
/// the middle. The console lays the list of projects under it by the page's token `--map-high`, which it sets to its
/// height.
const MAP_WIDE = 140;
const MAP_HIGH = 100;
const MAP_INSET = 12;
const MAP_PAD = 8;
/// The radius of the map's corners.
const MAP_CORNER = 6;
/// On the map: the gap between two items' pieces of a column, where each is tall enough to keep one; and how strong the
/// ground is in the frame of what the screen shows.
const MAP_GAP = 0.5;
const MAP_FRAME_FILL = 0.06;
/// How far in from the top and the bottom of the screen the tree fades out into the page, in pixels.
const EDGE = 48;
/// How large the tree is drawn at most, its full size; and at least, as a part of the size at which the whole tree fits
/// the screen, and never smaller than `ZOOM_LEAST`. How much one pixel of the wheel's turn, Shift held, makes it larger
/// or smaller, and one of a pinch's, which comes as the wheel with Ctrl held, in smaller steps.
const ZOOM_MOST = 1;
const ZOOM_UNDER_FIT = 0.8;
const ZOOM_LEAST = 0.05;
const ZOOM_WHEEL = 0.002;
const ZOOM_PINCH = 0.01;
/// While the tree is drawn smaller than its full size, the items near the pointer are drawn larger where they stand,
/// the nearer the larger, the nearest over the others: how many rows up or down, and how many columns across, an item
/// is drawn larger; how much smaller than its full size the tree must be drawn for it; and how quickly it comes in as
/// the pointer comes onto the tree and goes as it leaves, in milliseconds for about two thirds of the way.
const LENS_ROWS = 5;
const LENS_COLUMNS = 1.2;
const LENS_UNDER = 0.95;
const LENS_EASE = 90;

/// How large an item is drawn, as a part of its full size, with the tree drawn `zoom` of its full size and the item
/// `rows` rows up or down from the pointer, its middle from the pointer's, and `columns` columns across, its nearest
/// side from the pointer: its full size at the pointer, falling as the square of how far it is towards the tree's size,
/// which it is from [`LENS_ROWS`] rows or [`LENS_COLUMNS`] columns away on.
export function lensSize(rows: number, columns: number, zoom: number): number {
  const u = Math.min(1, Math.hypot(rows / LENS_ROWS, columns / LENS_COLUMNS));
  return zoom + (1 - zoom) * (1 - u) * (1 - u);
}

/// The room left at the sides of the whole tree when it is made to fit the screen, in pixels; at its top and bottom,
/// the room the fade into the page takes ([`EDGE`]).
const FIT_ROOM = 24;
/// How quickly the blue rings and ribbons of the item under the pointer come in, and fade out once it leaves: the
/// milliseconds in which each goes about two thirds of the way.
const GLOW_IN = 30;
/// How bright the items not in focus are drawn while an item is in focus or selected; the least of the links' own
/// strength they could be drawn at, were the items drawn at no brightness ([`linksStrength`]); and how long an item
/// takes to fade, down and back up alike, in milliseconds; only what the pointer comes onto lights at once
/// ([`dimmed`]).
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
  /// A way at rest, the light elsewhere, glows less than the edges of the cards lit the same way: its glow this strong,
  /// as a part of theirs, and the glow further out this wide.
  restGlow: 0.6,
  restFar: 6,
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
const GLOW_OUT = 160;
/// How far in from the side of the room an item brought into view stands.
const INSET = 24;
/// The dot the funnel from an item to the items under it folds into: a disc of this radius, 9 pixels across, its
/// middle this far out from the card's right edge at the card's middle, where the funnel folds into it and grows out
/// of it; in the funnel's grey, or, where a running item is folded in it, as the end of the lit way. A press this far
/// out from the edge, within the card's height, is on the dot of a folded item; under the pointer, the dot is set in a
/// disc this much wider, and the funnel filled, in the accent's tint, as what can be pressed is (principle
/// quiet-motion): the tint the pointer's ribbons are drawn in.
export const DOT_R = 4.5;
export const DOT_AT = 12;
export const FOLD_REACH = 24;
export const FOLD_PAD = 3;
/// How a fold turns. The funnel and the items under it grow out of the dot in the page's motion for opening and
/// closing a part, the window's as it slides (styles/parts.css, `.cw`) and a question card's as it folds
/// (`--turn-time`, `--turn-ease`): this many milliseconds, on this cubic Bézier. They draw in to the dot on a plain
/// ease-out of their own, in less time. The curves differ because the page's goes most of its way in its first frames:
/// run forward on a shrink, it empties the cards in those frames, and the fold looks instant; run backwards, it stalls
/// at the start, and the fold looks stuck. The plain ease-out moves at once, but gently, so the cards are seen drawing
/// in all the way to the dot; over the whole time of an unfold it is slow, and a fold put away a little faster reads
/// better.
export const UNFOLD_MS = 240;
export const UNFOLD_EASE = [0.2, 0.8, 0.2, 1] as const;
export const FOLD_MS = 180;
export const FOLD_EASE = [0, 0, 0.58, 1] as const;
/// What pointing at the funnel or the dot says a click does, with the key that does it over the card, before how many
/// items stand under the item, by kind.
export const FOLD_SAYS = { folded: 'Show the items under it (e)', shown: 'Fold the items under it away (e)' } as const;
/// The flash of the light along the edges of the item selected, which a click on the item's title in its window asks
/// for, so that the eye finds the card whether or not the tree moved to bring it into view: at its height the light is
/// as strong again. It rises over this part of its time and eases back to rest over the rest, this many milliseconds in
/// all ([`flashOf`]).
const FLASH_RISE = 0.15;
/// The light along the edges of the card under the pointer and of the card selected: how many milliseconds it takes to
/// reach in from the card's ends to its middle as the card is lit, and to go back out as it is let go; in how many
/// pieces each half is drawn, brightest at its head and this bright at its tail; and how strong it is on the card
/// under the pointer, the selected one's being whole.
const EDGE_IN = 150;
const EDGE_OUT = 110;
const EDGE_PIECES = 6;
const EDGE_TAIL = 0.45;
const EDGE_HOVER = 0.75;
/// How strong the light round the edges of a card whose own work moves is, in its tone, as a part of a lit way's.
const TONE_EDGE = 0.8;
export const FLASH_MS = 800;
/// The room a blur is drawn in past what it blurs, in strengths of the blur: wide enough that the blur fades out before
/// its edge.
const BLUR_ROOM = 4;
/// The glow of a lit way is drawn and blurred on a texture coarser than the canvas: one texel to this part of its
/// blur's strength, in pixels of the page, and never finer than the canvas. A blur `s` strong spreads a line about
/// 1.3 `s` either way, and a texel a quarter of `s` wide spreads it under 2% further, which does not show; and as the
/// texel is a part of the blur and not of the canvas, a glow is drawn on the same texels at a pixel ratio of 1 and of
/// 2, and looks the same at both. The near glow, 4 strong, is blurred on texels a pixel wide; the far glow, 14
/// strong, on texels 3.5 pixels wide, so on a twelfth of the texels.
const BLUR_TEXEL = 0.25;

/// How far on a flash of the item selected's light is, `ms` into it: from 0 up to 1 at its height and back down
/// to 0 once its time is over, smoothly both ways ([`FLASH_MS`]).
function flashOf(ms: number): number {
  const u = ms / FLASH_MS;
  if (u <= 0 || u >= 1) return 0;
  const k = u < FLASH_RISE ? u / FLASH_RISE : (1 - u) / (1 - FLASH_RISE);
  return k * k * (3 - 2 * k);
}

/// A blur this strong, with its room: set after the filter is made, which sets its own from its strength, too narrow
/// for the blur to fade out in.
function softBlur(strength: number): BlurFilter {
  const blur = new BlurFilter({ strength });
  blur.padding = BLUR_ROOM * strength;
  return blur;
}

/// Whether `g` draws anything. A layer that is blurred is hidden while it draws nothing: shown, it would be blurred
/// all the same, over the room of its blur at the corner of the canvas.
function drawsAny(g: Graphics): boolean {
  return g.context.instructions.length > 0;
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

/// What a light is drawn on: its line, and its glow close in and further out.
type Strokes = Pick<Lit, 'line' | 'near' | 'far'>;

/// The light at rest where a way meets a card, at one end of the way, or at the dot of a fold ([`draw`]): its line and
/// its glow, kept while the ways stand, drawn as strong as the way or the dot is by the alpha of its strokes, and
/// hidden while the light burns at its `place` across the page.
type EndLights = { rest: Strokes; place: number };

/// A lit way's own lights ([`draw`]): what stands of it, its line, glow and strands, kept while the way stands and
/// drawn as strong as the way is by the alpha of its strokes; and over that the light running down it, drawn anew at
/// each frame, and only while the light is on it (`lit`).
type WayLights = { rest: Strokes; light: Strokes; lit: boolean; place: number };

/// Lights in parts, each kept by its key: the lit ways, each way's own lights by the item it goes into
/// ([`WayLights`]), or the lights at rest where they meet the cards ([`EndLights`]). Their lines are among `lines`, and
/// their glow further out and close in among `far` and `near`, each blurred as one, as a layer of light's is
/// ([`Lit`]). Each way's light lies just over what stands of it, as when both were drawn on one layer.
type Ways<P = WayLights> = {
  parts: Map<number, P>;
  lines: Container;
  near: Container;
  far: Container;
  nearBlur: BlurFilter;
  farBlur: BlurFilter;
};

function waysLayer<P>(): Ways<P> {
  const nearBlur = softBlur(RUN.nearBlur);
  const farBlur = softBlur(RUN.farBlur);
  return {
    parts: new Map(),
    lines: new Container(),
    near: new Container({ filters: [nearBlur] }),
    far: new Container({ filters: [farBlur] }),
    nearBlur,
    farBlur,
  };
}

/// How many texels to a pixel of the page the glow of a lit way blurred `strength` strong is drawn on, on a canvas of
/// `canvas` pixels of the screen to one of the page ([`BLUR_TEXEL`]).
function glowResolution(strength: number, canvas: number): number {
  return Math.min(canvas, 1 / (BLUR_TEXEL * strength));
}

/// Blurs `blur` `strength` strong, in pixels of the page, with its room, where it is not already.
function strengthTo(blur: BlurFilter, strength: number) {
  if (blur.strength === strength) return;
  blur.strength = strength;
  blur.padding = BLUR_ROOM * strength;
}

/// Blurs `blur` `strength` strong, with its room, on the texels a glow that strong is drawn on, on a canvas of `canvas`
/// pixels of the screen to one of the page, where it is not already.
function blurTo(blur: BlurFilter, strength: number, canvas: number) {
  strengthTo(blur, strength);
  const resolution = glowResolution(strength, canvas);
  if (blur.resolution !== resolution) blur.resolution = resolution;
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
/// The part of the way to where it is going that the tree moves each frame, when the scene brings an item into view or
/// makes the whole tree fit the screen.
const SNAP = 0.32;
/// How much of the way to where the wheel has taken it the tree goes each frame; and how many pixels of the way, which
/// no screen shows, may be left when it goes the rest at once, so that it comes to rest where the wheel took it within
/// some thirty frames, and does not creep on unseen until what is left is too small for a number to hold.
const GLIDE = 0.35;
const REST = 0.01;
/// How many rows of the tree stay on the screen at least, at its top or its bottom.
export const KEEP = 3;

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
  /// What the owner may answer ahead, which holds the work at nothing yet: the page's orange.
  orange: '#f5840c',
  green: '#2f9e6e',
  /// The pointer's blue: the lines at the edges of the column under it, and the own colour of an item with no work of
  /// its own above a work, ringed and lit in it.
  over: '#6d9bf9',
  /// The words on a filled tag.
  onTag: '#ffffff',
  /// What the owner has not read: the number of replies on a card, and the mark across an item's bar on the map.
  unseen: '#3673f5',
  accent: '#3673f5',
  /// What joins an item to those under it: Primer's neutral muted, `#818b981f` in light and `#656c7633` in dark, a
  /// little lighter.
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
/// while the tree is not `dimming`, as while nothing is `lit`; while it is, as while something is lit or the search is
/// open, one whose two ends are both lit at full strength, and the others as dim as the links once the items are dimmed.
export function wayAims(
  ways: Iterable<number>,
  parent: ArrayLike<number>,
  lit: Set<number>,
  dimming = lit.size > 0,
): Map<number, number> {
  const aims = new Map<number, number>();
  for (const i of ways) aims.set(i, !dimming || (lit.has(i) && lit.has(parent[i])) ? 1 : linksStrength(DIM));
  return aims;
}

/// One frame's step, `step` milliseconds long, of a brightness from `was` towards `to`: about two thirds of the way in
/// [`FADE_MS`], going down or up alike, so that nothing lit jumps as the light moves or goes out.
export function eased(was: number, to: number, step: number): number {
  const next = was + (to - was) * (1 - Math.exp(-step / FADE_MS));
  return Math.abs(to - next) < SETTLED ? to : next;
}

/// One frame's step of a brightness from `was` towards `to`, going up at once and down as [`eased`] goes: what the
/// pointer comes onto lights without waiting.
export function dimmed(was: number, to: number, step: number): number {
  return to >= was ? to : eased(was, to, step);
}

/// How near its aim an eased value is taken to be there: nearer than this, it is set to its aim, so that what eases
/// comes to rest, and the still parts of the tree are not drawn again for it ([`draw`]).
const SETTLED = 1e-3;

/// What a card never draws of its item, left out of what tells whether it draws the same as before ([`show`]): what its
/// work has spent, when something last happened to it, and where its time went, which change with every round.
const undrawn = (key: string, value: unknown) =>
  key === 'usdMicros' || key === 'last' || key === 'bar' ? undefined : value;

/// Where the level stands on a card, in the card's own coordinates, and what pointing at it says: kept by the card's
/// view, which the cache of cards keeps with it.
const LEVEL_SAID = new WeakMap<Container, { x: number; y: number; width: number; height: number; said: string }>();

/// A tag at the right of a card's first row as it was drawn: its text and its colour.
export type DrawnTag = { text: string; fill: string | number };

/// A card as it was made, kept for the next tree that draws one the same ([`show`]): what it draws, its view, the room
/// of its strip, and what its making set: its age, the pulse of its running cells, its strip, its look, the number it
/// wrote and its tags, the last four for the tests.
type CardMade = {
  sig: string;
  view: Container;
  strip: { start: number; end: number; count: number } | null;
  age: { from: number; label: BitmapText; shown: string; row: number } | undefined;
  pulse: Graphics | undefined;
  drawnStrip: DrawnStrip | undefined;
  look: { rows: number; fill: string | number; edge: string | number } | undefined;
  number: DrawnNumber | undefined;
  tags: DrawnTag[] | undefined;
};

/// The number a card wrote and where its title begins from the card's left, for the tests ([`drawnNumbers`]).
type DrawnNumber = { text: string; titleAt: number };

/// The mean, the 95th part and the longest of `times`, in milliseconds to a hundredth; nought for none.
function timesOf(times: number[]): { meanMs: number; p95Ms: number; maxMs: number } {
  if (times.length === 0) return { meanMs: 0, p95Ms: 0, maxMs: 0 };
  const sorted = [...times].sort((a, b) => a - b);
  const r = (x: number) => Math.round(x * 100) / 100;
  return {
    meanMs: r(sorted.reduce((a, b) => a + b, 0) / sorted.length),
    p95Ms: r(sorted[Math.floor(sorted.length * 0.95)]),
    maxMs: r(sorted[sorted.length - 1]),
  };
}

/// A way's curve: the points it is drawn in, how far along it each of them stands, and the way square to it at each,
/// one pixel long, to which a strand beside it is moved out.
type Way = { points: [number, number][]; along: number[]; across: [number, number][] };

/// Where a way's line turns into a card's edge, at the way's start (`start`) or at its end. The turn leaves the way
/// `reach` along it from the card, at the way's own place there (`from`). It bends at `bend`, where the way's direction
/// at that place, followed on to the card, meets the card's edge. A curve from `from` through `bend` to the edge so
/// leaves the way along it and meets the edge along it, however steeply the way turns near the card. A turn begun level
/// with the card's middle would stand off a way that has already turned, as a stub beside it. The bend is kept on the
/// edge between the places the turn runs to, `reach` up and down it, so that neither half of the turn runs back.
function turnOf(way: Way, start: boolean, reach: number): { from: [number, number]; bend: [number, number] } {
  const { points, along } = way;
  const last = points.length - 1;
  const at = Math.min(along[last], Math.max(0, start ? reach : along[last] - reach));
  let n = 0;
  while (n < last - 1 && along[n + 1] < at) n += 1;
  const [a, b] = [points[n], points[n + 1]];
  const len = along[n + 1] - along[n];
  const k = len > 0 ? (at - along[n]) / len : 0;
  const from: [number, number] = [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k];
  const [ex, ey] = points[start ? 0 : last];
  const [dx, dy] = [b[0] - a[0], b[1] - a[1]];
  const by = dx === 0 ? ey : from[1] + (dy * (ex - from[0])) / dx;
  return { from, bend: [ex, Math.min(ey + reach, Math.max(ey - reach, by))] };
}

/// A lit way as it was drawn, for the tests: the item it runs into, where it began and ended, the points its curve was
/// drawn through, and its turns into the cards it joins ([`turnOf`]), out of the one above and into the item.
export type LitWay = {
  id: number;
  from: [number, number];
  to: [number, number];
  curve: [number, number][];
  turns: { from: [number, number]; bend: [number, number] }[];
};

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

/// How far the glint at a spark has come, from 0 to 1, as it burns at `burn` ([`sparkAt`]): the glint comes only
/// with the light itself, so that at rest the way's end is lit, not glinting.
function glintOf(burn: number): number {
  return Math.max(0, (burn - RUN.portRest) / (1 - RUN.portRest));
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
    // What is not read is marked in the page's accent, as the bar's count of it is, in either theme.
    unseen: token('--accent', turn(LIGHT.unseen, 'read')),
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
    orange: token('--orange', LIGHT.orange),
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
  /// How many cards the last tree shown made, the others kept from the tree before.
  made: number;
  /// How many frames have drawn the still parts of the tree again ([`draw`]), and for what, each frame counted for each
  /// thing that asked: `tree`, `pointer`, `select`, `view`, `flash`, `shown`, `start`, `turns`, `moved`, `size` or
  /// `easing`.
  redraws: number;
  redrawn: Record<string, number>;
  /// How many frames have built again what stands of the lit ways: their lines, glow and strands, and the lights at
  /// rest where they meet the cards and at the dots of folds ([`draw`]).
  wayBuilds: number;
  /// How many frames the scene's ticker has drawn, as the browser gives them or a bench draws them: none come while
  /// the page or the view is hidden ([`setShown`]).
  frames: number;
  /// How many layers the last frame blurred: the glow of the lit ways close in and further out, the same where they meet
  /// the cards, over the cards, round the edges of the cards whose own work moves, and along the edges of the item
  /// selected and the item under the pointer, each only while it draws something.
  blurs: number;
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

/// How far a fold's turn has gone, from 0 to 1, when a part `t` of its time has gone: the cubic Bézier `curve`, an
/// unfold's ([`UNFOLD_EASE`]) or a fold's ([`FOLD_EASE`]), read as CSS reads one.
export function turnEase(t: number, curve: readonly [number, number, number, number] = UNFOLD_EASE): number {
  if (t <= 0) return 0;
  if (t >= 1) return 1;
  const [x1, y1, x2, y2] = curve;
  const at = (a: number, b: number, u: number) => 3 * (1 - u) * (1 - u) * u * a + 3 * (1 - u) * u * u * b + u * u * u;
  let [lo, hi] = [0, 1];
  for (let n = 0; n < 30; n += 1) {
    const u = (lo + hi) / 2;
    if (at(x1, x2, u) < t) lo = u;
    else hi = u;
  }
  return at(y1, y2, (lo + hi) / 2);
}

/// What pointing at the funnel or the dot of an item's fold says: what a click does, then how many items stand under
/// the item, each kind by its key with its count, and apart how many shadows there point to items shown elsewhere,
/// any with none left out: `Fold the items under it away: open 2, done 4, 1 shown elsewhere`.
export function foldWords(fold: Fold): string {
  const counts = STANDINGS.filter((s) => fold.under[s] > 0).map((s) => `${s} ${fold.under[s]}`);
  if (fold.under.elsewhere > 0) counts.push(`${fold.under.elsewhere} shown elsewhere`);
  return `${FOLD_SAYS[fold.folded ? 'folded' : 'shown']}: ${counts.join(', ')}`;
}

/// Whether the point (`x`, `y`) is in the funnel from an item ending at `px`, its box from `top` to `bottom`, to items
/// starting at `cx`, the first's box starting at `first` and the last's ending at `last`: between its two curves, each
/// bent as the funnel is drawn. A part `u` along either curve stands `1.5u - 1.5u² + u³` of the way across and
/// `3u² - 2u³` of the way down.
export function inFunnel(
  px: number,
  top: number,
  bottom: number,
  cx: number,
  first: number,
  last: number,
  x: number,
  y: number,
): boolean {
  if (!(cx > px) || x < px || x > cx) return false;
  const across = (x - px) / (cx - px);
  let [lo, hi] = [0, 1];
  for (let n = 0; n < 30; n += 1) {
    const u = (lo + hi) / 2;
    if (1.5 * u - 1.5 * u * u + u * u * u < across) lo = u;
    else hi = u;
  }
  const u = (lo + hi) / 2;
  const down = 3 * u * u - 2 * u * u * u;
  return y >= top + (first - top) * down && y <= bottom + (last - bottom) * down;
}

/// A fold turning: towards which size the items under its item go, 0 folded away or 1 shown, from which, since when
/// on the scene's clock, and over how many milliseconds.
export type Turn = { to: number; from: number; at: number; ms: number };

/// How large the items under a turning fold are drawn at the scene's time `clock`, from 0 folded away to 1 shown, from
/// the size the turn began at: growing on the unfold's curve, shrinking on the fold's ([`UNFOLD_MS`]).
export function sizeOf(turn: Turn, clock: number): number {
  const t = turn.ms <= 0 ? 1 : (clock - turn.at) / turn.ms;
  return turn.from + (turn.to - turn.from) * turnEase(t, turn.to >= turn.from ? UNFOLD_EASE : FOLD_EASE);
}

/// Where each item of tree `t` starts, in rows from the top of the tree, each item `rows[k]` rows high: the fixed place
/// it is drawn at. Going down the tree, the items under an item are placed first, each as high as its column leaves
/// room for; the item then stands in the middle of them, where its funnel opens onto them. Where its own column leaves
/// no room there, it stands as high as its column lets it, and the items under it, with everything under them, move
/// down until it is in their middle again. Between the items under one item and those under the next stands `APART`
/// rows where either group holds more than one item: a lone item stands level with the item it hangs from, so nothing
/// need set it apart. A fold turning draws a group at `scale` of its size, and everything turns with it, so that the
/// tree stands as the fold leaves it the moment the turn ends: the item stands that part of the way from where it
/// stands with nothing under it to the middle of the items under it, and the room round the group shrinks with it, the
/// room it leaves going to the groups beside it.
export function layOut(t: Tree, rows: ArrayLike<number>, scale: ArrayLike<number>): Float64Array {
  const top = new Float64Array(t.parent.length);
  const free: number[] = [];
  const lastParent: number[] = [];
  // Whether a group holds more than one item: those under the same item, or the roots.
  const many = (k: number) => (t.parent[k] < 0 ? (t.columns[0]?.length ?? 0) : t.children[t.parent[k]].length) > 1;
  // Of the groups placed in each column, as far as each is drawn: how far there is one, and how far the one nearest
  // the next group holds more than one item; a group drawn smaller hands on that part of what stood before it.
  const there: number[] = [];
  const crowded: number[] = [];
  // Moves everything under item `k` down by `by` rows, with the room it takes in each column: what is under `k` was
  // placed last in every column it stands in.
  const lower = (k: number, by: number) => {
    const columns = new Set<number>();
    const walk = (j: number) => {
      for (const c of t.children[j]) {
        top[c] += by;
        columns.add(t.depth[c]);
        walk(c);
      }
    };
    walk(k);
    for (const d of columns) free[d] += by;
  };
  const place = (k: number) => {
    const d = t.depth[k];
    const starts = lastParent[d] !== t.parent[k] || free[d] === undefined;
    const [was, wasMany, big, s] = [there[d] ?? 0, crowded[d] ?? 0, many(k) ? 1 : 0, scale[k]];
    const apart = starts ? APART * s * Math.max(wasMany, big * was) : 0;
    const earliest = (free[d] ?? 0) + apart;
    let at = earliest;
    const kids = t.children[k];
    if (kids.length > 0) {
      for (const c of kids) place(c);
      const end = kids[kids.length - 1];
      const middle = (top[kids[0]] + top[end] + rows[end]) / 2 - rows[k] / 2;
      // How large the items under it are drawn, as a part of its own size: whole but while its fold turns.
      const drawn = s > 0 ? scale[kids[0]] / s : 1;
      if (middle >= earliest) at = earliest + (middle - earliest) * drawn;
      else lower(k, earliest - middle);
    }
    top[k] = at;
    free[d] = at + rows[k];
    lastParent[d] = t.parent[k];
    if (starts) {
      there[d] = s + (1 - s) * was;
      crowded[d] = s * big + (1 - s) * wasMany;
    }
  };
  for (const k of t.columns[0] ?? []) place(k);
  return top;
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
function cellLooks(): { measures: { side: number; gap: number }; looks: Record<RunState, CellLook> } {
  const side = length(MARK.side);
  const corner = length(MARK.corner);
  const colour = (name: string) => token(name, PALETTE.muted);
  const looks = Object.fromEntries(
    Object.entries(RUN_STATES).map(([state, look]): [string, CellLook] => [
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
  ) as Record<RunState, CellLook>;
  return { measures: { side, gap: length(MARK.gap) }, looks };
}

/// An item's piece of a column's bar on the map, as it was drawn: the item, its column, its top on the map and how
/// tall, and its colour.
export type MapPiece = { id: number; column: number; y: number; height: number; colour: string };

/// What the map drew at a frame: the part of the tree the screen shows, each item's piece, and the items whose piece
/// the accent was drawn across, as something under them or they themselves are not read.
export type MapDrawn = {
  frame: { x: number; y: number; width: number; height: number } | null;
  pieces: MapPiece[];
  unread: number[];
};

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

/// A card's strip as it was drawn: its marks, how many cells were left out, which is none since every strip fits, and
/// where its room ends; and the level its work's cell runs at, under the card's number, with its colour and whether it
/// is strong.
export type DrawnStrip = {
  marks: DrawnMark[];
  more: number;
  end: number;
  level: { text: string; colour: string; strong: boolean } | null;
};

/// Who moves an item next, in one set of colours for its card, its light, the way to it and the marks of its cells:
/// red where the owner does (its work waits for the owner, or the engine stopped it); blue where the system does (a
/// round runs, or a call the engine carries out runs); yellow where it waits its turn and will go on by itself (for a
/// slot, the project's turn to merge, a runner or a machine for its call, a retry, a usage limit's reset, the outside,
/// the engine starting again); grey where it does not move by itself (not let on, or paused); green where it is done.
/// An item whose work waits for the items under it moves as they do.
export type Tone = 'red' | 'blue' | 'yellow' | 'grey' | 'green';

/// The tones that move, in the order an item takes them from the items under it: blue first, since while anything
/// under it goes on the item goes on; then red, where nothing goes on and something waits for the owner, who alone can
/// move it then; then yellow, where everything under it only waits its turn.
const PRESSING: Tone[] = ['blue', 'red', 'yellow'];

/// Each item's tone, as its card draws it, of the tree `roots`; and the items whose own work moves, blue, yellow or
/// red, which stand in a light of their tone. An item whose work waits for the items under it takes the most pressing
/// tone of theirs, and grey where none of them moves; a shadow stands for an item drawn elsewhere and counts for
/// nothing.
export function tonesOf(roots: Shown[]): { tone: Map<Shown, Tone>; moves: Set<Shown> } {
  const tone = new Map<Shown, Tone>();
  const moves = new Set<Shown>();
  const walk = (s: Shown): Tone => {
    const below = s.children.filter((c) => !c.shadow).map(walk);
    const own = toneOf(s);
    const t = own === 'under' ? (PRESSING.find((p) => below.includes(p)) ?? 'grey') : own;
    if (own !== 'under' && PRESSING.includes(own)) moves.add(s);
    tone.set(s, t);
    return t;
  };
  roots.forEach(walk);
  return { tone, moves };
}

/// The tone of item `s` itself, from where its own flow stands, whatever flow it runs on; `under` where its flow waits
/// for the items it depends on, or it has no work, so that it takes theirs.
function toneOf(s: Shown): Tone | 'under' {
  if (s.shadow || s.standing === 'cancelled') return 'grey';
  if (s.standing === 'done') return 'green';
  if (s.stands === null) return 'under';
  switch (stateOf(s.stands).which) {
    case 'owner':
    case 'halted':
    case 'broken':
      return 'red';
    case 'running':
      return 'blue';
    case 'calling':
    case 'waiting':
      return waitsItsTurn(s.stands) ? 'yellow' : 'blue';
    case 'items':
      return 'under';
    case 'merged':
      return 'green';
    default:
      return 'grey';
  }
}

/// The colour of each tone.
const TONE_COLOUR = (): Record<Tone, string> => ({
  red: PALETTE.halted,
  blue: PALETTE.running,
  yellow: PALETTE.calling,
  grey: PALETTE.paused,
  green: PALETTE.green,
});

/// The colours a way is drawn in, of the tone of the item it leads to: its line; the line's glint; its glow and strands
/// at `bright` of the light, from the way at rest towards the light's own colour as it brightens; whether the light runs
/// down it, which it does only where the system works; and whether it glows, which a grey way, of a work the owner
/// paused after it began, does not.
type Paint = { line: number; glint: number; tint: (bright: number) => number; runs: boolean; glows: boolean };

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
/// The least room a card's number has before its title, and the gap between them.
const TITLE_AT = 26;
const TITLE_GAP = 8;
/// A card's corners: the page's corner (`--corner`), as its buttons and tags have.
const CARD_CORNER = 3;
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

/// The faces of a node's words: Primer's stacks, without the face Primer names
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

/// Where the tree is shown: how large (`zoom`, 1 at its full size), the row of the tree at the middle of the screen
/// (`at`), and how far right of the screen's left the tree's left side stands, in pixels at the tree's full size
/// (`left`).
export type View = { zoom: number; at: number; left: number };

/// A part of the tree as it is drawn, at its full size: from `x0` to `x1` across and `y0` to `y1` down.
export type Room = { x0: number; y0: number; x1: number; y1: number };

/// Where the tree is drawn: the screen's place in it when it was last drawn anew, its left side at `left` and its top
/// row at `top`, at the tree's full size, so that the screen's left top corner was at 0; and the `room` it was drawn
/// for, the screen then and [`FRAME_ROOM`] of it more on every side. While the screen stays in that room and as large
/// or smaller, the tree is not drawn again but moved whole as far as the screen moved, as a pan or a zoom is: what
/// the screen comes to is drawn already, and nothing is built again for it.
export type Frame = { left: number; top: number; room: Room };

/// How much more than the screen, on every side, the tree is drawn for, as a part of the screen's width and height: a
/// screen moved by up to as much, or made smaller by half, shows what is drawn.
const FRAME_ROOM = 0.5;

/// The frame taken from where the screen is now, its left side at `left` and its top row at `top`, `width` by `height`
/// at the tree's full size.
export function frameAt(left: number, top: number, width: number, height: number): Frame {
  const [wide, high] = [FRAME_ROOM * width, FRAME_ROOM * height];
  return { left, top, room: { x0: -wide, y0: -high, x1: width + wide, y1: height + high } };
}

/// Whether `frame` still holds the screen at `left` and `top`, `width` by `height`: the screen is wholly in its room,
/// and the room is not more than twice as wide as the room the screen now would take, which would draw far more than
/// is seen.
export function holds(frame: Frame, left: number, top: number, width: number, height: number): boolean {
  const [dx, dy] = [left - frame.left, top - frame.top];
  const r = frame.room;
  const inside = -dx >= r.x0 && width - dx <= r.x1 && -dy >= r.y0 && height - dy <= r.y1;
  return inside && r.x1 - r.x0 <= 2 * (1 + 2 * FRAME_ROOM) * width;
}

/// A point of the canvas, `(sx, sy)` in its pixels, that stays on the same point of the tree while the tree grows or
/// shrinks: the tree's `row` there, and how far right of the tree's left side it is (`along`), at the tree's full size.
type Pivot = { sx: number; sy: number; row: number; along: number };

export class SnapScene {
  stats: Stats = {
    items: 0,
    drawn: 0,
    fps: 0,
    meanMs: 0,
    p95Ms: 0,
    made: 0,
    redraws: 0,
    redrawn: {},
    wayBuilds: 0,
    frames: 0,
    blurs: 0,
  };
  private app = new Application();
  private world = new Container();
  /// The funnels from each item to the items under it; and the ribbons of the items lit.
  private links = new Graphics();
  /// The lit ways from the items whose work runs up to their roots, over the funnels, each way's line, glow and
  /// strands built once where it stands and kept while it stands, and the light running down it drawn anew each frame
  /// ([`Ways`]). What of them is drawn over the cards instead: the spark where a way meets a card, and a card's edge as
  /// a light crosses it, which the cards, drawn over the ways, would else hide, in lines and their glow ([`Lit`]); and
  /// the lights at rest where the ways meet the cards and at the dots of folds, kept for each end of a way and each
  /// dot ([`EndLights`]), so that where the light burns, the ends there alone are drawn as they burn and the others
  /// stand as they were laid. What stands is drawn as strong as its way is by the alpha of its strokes.
  private ways = waysLayer<WayLights>();
  private runEdges = litLayer();
  private ends = waysLayer<EndLights>();
  /// The numbers the lit ways were last built by ([`RUN`]), which the page's knobs may change while it runs.
  private runNumbers = '';
  /// Whether what they draw is added to the ground, as the theme has it ([`adding`]).
  private adds = false;
  /// The flash of the item selected's light under way, by the item's number and the scene's time it began at
  /// ([`flash`]); and the item selected's light as it was last drawn, for the tests ([`chosenLight`]).
  private flashing: { id: number; at: number } | null = null;
  private chosenDrawn: { id: number; out: number; alpha: number } | null = null;
  /// The funnel or the dot of the fold under the pointer, filled with the pointer's tint, under the lit ways; and the
  /// dots of the items folded, in the funnel's grey, over it.
  private foldLight = new Graphics();
  private dots = new Graphics();
  /// The items, by their place in the tree, whose link to the one above them is on the way from an item whose work
  /// runs up to its root ([`waysUp`]); and how strong each of those links is drawn, eased from frame to frame towards
  /// what [`wayAims`] gives it.
  private runWays = new Set<number>();
  private runStrength = new Map<number, number>();
  private marks = new Graphics();
  private layer = new Container();
  /// The fade of the tree into the page at the top and the bottom of the screen, each a gradient in the shape's own
  /// measure, from its top (0) to its bottom (1).
  private edges = new Graphics();
  private fades = { top: vertical(PALETTE.page, 1, 0), bottom: vertical(PALETTE.page, 0, 1) };
  /// The map at the top left: its frame, and a bar for each column, drawn each frame, cut to the room inside the frame.
  private map = new Container();
  private mapFrame = new Graphics();
  private mapBars = new Graphics();
  private mapMask = new Graphics();
  /// How large the tree is drawn, as a part of its full size in hundredths, at the map's bottom right, on a patch of the
  /// page's ground so that the pieces under it do not cross it.
  private mapZoom: BitmapText | null = null;
  private mapZoomGround = new Graphics();
  /// How far the items near the pointer are drawn larger, from 0 to 1, eased as it comes and goes; and the point of
  /// the tree they are drawn larger round, the pointer's, kept while it goes after the pointer has left.
  private lens = 0;
  private lensAt = { x: 0, y: 0 };
  /// Under each card drawn larger, the page's ground, just under it among the cards, so that a card it stands over
  /// does not show through it while the cards are dimmed; by the item's index in the tree, kept while they are drawn.
  private lensGrounds = new Map<number, Graphics>();
  /// The cards drawn larger, lifted over the lights that lie over the other cards, the sparks where the ways meet them
  /// and the dots of folds, so that none of those of the cards under them shows on them; each with its ground, in
  /// order of how large it is drawn.
  private lifted = new Container();
  private lensDrawn = { lens: 0, x: 0, y: 0 };
  /// The light along the edges of the card under the pointer and of the card selected, over every card, the lifted
  /// ones too: each card's by its item's number, with how far it has reached in from the card's ends, from 0 to 1,
  /// and how strong it is. It reaches in as the card is lit and goes back out as it is let go, so that the light goes
  /// over from one card to the next rather than going out and coming on elsewhere.
  private edgeLight = litLayer();
  private edgesLit = new Map<number, { reach: number; strength: number }>();
  /// The light round the edges of each card whose own work moves, in its tone, as the ways are lit: over the cards,
  /// and a lifted card's over the lifted cards, so that a card drawn over another hides that one's light.
  private toneEdges = litLayer();
  private toneEdgesLifted = litLayer();
  /// How large the map draws the tree, a part of its full size, and where the tree's top left corner stands on the map.
  private mapScale = 1;
  private mapOrigin = { x: 0, y: 0 };
  /// What the map drew at the last frame ([`drawnMap`]).
  private mapDrawn: MapDrawn = { frame: null, pieces: [], unread: [] };
  private tree: Tree | null = null;
  /// Each item's number, in the tree's order.
  private ids: number[] = [];
  private views: Container[] = [];
  /// Each item's height in rows when drawn whole, and as drawn now; how much of its whole size it is drawn at, less
  /// than whole only while a fold over it turns; where it starts, in rows from the top of the tree ([`layOut`]); and
  /// how many rows the tree takes, its tallest column's.
  private base: Float64Array = new Float64Array(0);
  private rows: Float64Array = new Float64Array(0);
  private scaleOf: Float64Array = new Float64Array(0);
  private tops: Float64Array = new Float64Array(0);
  private tall = 0;
  /// The folds turning, by the number of their item; the scene's own time, in milliseconds drawn, which they run on;
  /// and whether each item with items under it was folded as last shown, by its number, from which a turn begins.
  private turns = new Map<number, Turn>();
  private clock = 0;
  private wasFolded = new Map<number, boolean>();
  /// The items, by their place in the tree, whose dot ends the lit way of a running item folded in it.
  private litDots: number[] = [];
  /// How each item, by its place in the tree, was moved and sized at the last frame while folds turned ([`movedNow`]);
  /// none else, when each is drawn where it stands, whole. And the lit ways drawn at the last frame, each by the item
  /// it runs into, from where it began to where it ended, with its curve and its turns into the cards; for the tests.
  private drawnMoved: { a: Float64Array; bx: Float64Array; by: Float64Array } | null = null;
  /// The strips of the cards being made, each with where its room begins and ends and how many cells it has, drawn
  /// once every card is made, at one size of the marks for the whole tree ([`show`]).
  private strips: { k: number; start: number; end: number; count: number; draw: (scale: number) => void }[] = [];
  /// How each card was drawn, by its item's number: its rows, its fill and its edge's colour ([`cardLook`]).
  private cardsDrawn = new Map<number, { rows: number; fill: string | number; edge: string | number }>();
  /// The tags each card drew at the right of its first row, in the order they stand from the right, by its item's number
  /// ([`drawnTags`]).
  private tagsDrawn = new Map<number, DrawnTag[]>();
  /// The number each card wrote, by its place in the tree, and where its title begins ([`drawnNumbers`]); and how far
  /// from the card's left every title begins: past the widest number the whole tree holds, folded items included, so
  /// that no fold moves a title ([`show`]).
  private numbersDrawn = new Map<number, DrawnNumber>();
  private titleOffset = TITLE_AT;
  private numberWidths = new Map<number, number>();
  /// What the tree drawn was made from ([`show`]): the roots as the cards draw them, the folds turning and the theme,
  /// which changes each time the theme does; the cards made for it, by what each draws; and the size of the strips'
  /// marks.
  private drawnKey = '';
  private cardCache = new Map<string, CardMade[]>();
  private stripScale = 1;
  private themed = 0;
  /// What asks for the still parts of the tree to be drawn again at the next frame, by what it is ([`mark`], [`draw`]),
  /// and whether something they draw was still easing at the last; the items drawn and their views, the size of the
  /// screen they were drawn at, and the ways' curves, kept while they stand.
  private dirty = new Set<string>(['start']);
  private easing = true;
  private drawnList: number[] = [];
  private nowShown = new Set<Container>();
  private drawnSize: [number, number] = [0, 0];
  private drawnView: View = { zoom: Number.NaN, at: Number.NaN, left: Number.NaN };
  private frame: Frame | null = null;
  private wayCache = new Map<number, { way: Way; len: number; dist: number }>();
  private waysDrawn: LitWay[] = [];
  /// Each item's tone, as its card draws it, and whether its own work moves, which lights it ([`tonesOf`]); by its
  /// place in the tree.
  private toneAt: Tone[] = [];
  private movesAt: Uint8Array = new Uint8Array(0);
  /// For each item, whether a work has begun on it or under it: the title of one with none is greyer.
  private begun: Uint8Array = new Uint8Array(0);
  /// Each item's tone, as its card draws it, and the items whose own work moves ([`tonesOf`]).
  private tones: ReturnType<typeof tonesOf> = { tone: new Map(), moves: new Set() };
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
  private looks = {} as Record<RunState, CellLook>;
  private drawnStrips = new Map<number, DrawnStrip>();
  /// For each column, the ranks of its items new to the owner, or with one under them folded away, which the map marks.
  private fresh: number[][] = [];
  private shown = new Set<Container>();
  /// The column under the pointer, or none, found each frame.
  private hovered = -1;
  /// Where the pointer is over the canvas, in the canvas's pixels, or none; and where that is on the tree, in pixels at
  /// its full size, worked out each frame as the tree moves; the item under it is found each frame too.
  private screenPointer: { x: number; y: number } | null = null;
  private pointer: { x: number; y: number } | null = null;
  /// How far each item ringed in blue, the one under the pointer and those above it, has come in, from 0 to 1; one
  /// the pointer has left stays until it has faded out.
  private glowing = new Map<number, number>();
  /// How bright the items that the pointer and the selection do not light are drawn: all of them fully while nothing
  /// is lit, dimmed while something is, so that what is lit and what joins it stand alone; eased from frame to frame.
  private dim = 1;
  /// The items in focus: the one under the pointer with what joins it.
  private inFocus = new Set<number>();
  /// While the console's search is open, the items it finds, by number: lit as the items in focus are, every other
  /// item dimmed, all of them while it finds nothing; none while it is closed.
  private found: number[] | null = null;
  /// Where the tree is shown ([`View`]); and the row at the middle of the screen the wheel has taken it to, which `at`
  /// glides towards.
  private zoom = 1;
  private at = 0;
  private aim = 0;
  private left = 40;
  /// Where the tree is moving sideways to, when the scene brings an item into view, until it is there or the owner moves
  /// it.
  private leftGoal: number | null = null;
  /// Where the tree is going as a whole, while it comes to fit the screen, goes back from that, or brings an item to
  /// its full size; and where it was before it was made to fit the screen, while it is so, to go back to.
  private goal: (View & { pivot?: Pivot }) | null = null;
  private back: View | null = null;
  /// Where the tree was shown before the console's overview showed every project's tree whole, while it does
  /// ([`showWhole`]); none else.
  private gridBack: View | null = null;
  /// The item that stays where it is on the screen while the folds change the tree round it, by its number, and how
  /// many rows from the middle of the screen its middle stands; none once the owner moves the tree.
  private anchor: { id: number; off: number } | null = null;
  private frames: number[] = [];
  private lastDraw = 0;
  private last = 0;
  /// Where the pointer last was while a press on the tree moves it, or a press on the map moves the screen over the
  /// tree; none else.
  private dragging: { x: number; y: number } | null = null;
  private mapDragging = false;
  /// Where the pointer went down, so that a press let go where it began is a click, not a drag.
  private down: { x: number; y: number } | null = null;
  /// The item under the pointer at the last frame, by its index in the tree, or none.
  private under = -1;
  /// The item whose fold's funnel or dot is under the pointer at the last frame, by its index in the tree, or none;
  /// and what the canvas says on pointing at it, as last written.
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
  /// Told, at a press on the funnel or the dot of an item's fold, the item's number and whether the items under it are
  /// now to be folded away: the console keeps the choice and draws the tree again.
  onFold: ((id: number, folded: boolean) => void) | null = null;
  /// Told, at a click on a shadow whose item this tree does not hold, as one of another project's, the item's number:
  /// the console shows that project's tree and selects it there.
  onAway: ((id: number) => void) | null = null;
  /// Told how wide and how tall the map is each time that changes, in pixels: what stands beside the map stands past it,
  /// and what stands under it below it.
  onMapSize: ((width: number, height: number) => void) | null = null;
  private mapWide = MAP_WIDE;
  private mapHigh = MAP_HIGH;
  private mapTold = false;
  /// The items as last shown, by their place in the tree.
  private shownItems: Shown[] = [];
  private shadowsOf = new Map<number, number[]>();
  private shadows: Uint8Array = new Uint8Array(0);
  private ready = false;
  private gone = false;
  /// Whether the view the scene is in is shown ([`setShown`]): it draws while that and the page are.
  private viewShown = true;
  /// What a pixel read back from the canvas is put in, which a bench reads to wait for the GPU ([`waitForGpu`]).
  private pixel = new Uint8Array(4);
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
      this.app.destroy(KEEP_SHARED, { children: true, texture: true });
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
    // What stands still of the lights lies under what moves of them, its glow under their glow and its lines under
    // their lines.
    this.world.addChild(
      this.links,
      this.foldLight,
      this.dots,
      this.ways.far,
      this.ways.near,
      this.ways.lines,
      this.layer,
      this.ends.far,
      this.ends.near,
      this.runEdges.far,
      this.runEdges.near,
      this.ends.lines,
      this.runEdges.line,
      this.toneEdges.far,
      this.toneEdges.near,
      this.toneEdges.line,
      this.lifted,
      this.toneEdgesLifted.far,
      this.toneEdgesLifted.near,
      this.toneEdgesLifted.line,
      this.edgeLight.far,
      this.edgeLight.near,
      this.edgeLight.line,
      this.marks,
    );
    this.app.stage.addChild(this.world, this.edges, this.map);
    // Nothing is blurred until a frame draws something to blur ([`draw`]).
    for (const layer of this.blurred) layer.renderable = false;
    // The cards stand in a group of their own, which is drawn as it was while none of them changes, whatever the lights
    // over and under them do.
    this.layer.enableRenderGroup();
    this.layer.sortableChildren = true;
    this.lifted.sortableChildren = true;
    this.map.position.set(MAP_INSET, MAP_INSET);
    this.map.addChild(this.mapFrame);
    const canvas = this.app.canvas;
    canvas.addEventListener('wheel', this.wheel, { passive: false });
    canvas.addEventListener('pointermove', this.hover);
    canvas.addEventListener('pointerleave', () => {
      this.mark('pointer');
      this.screenPointer = null;
    });
    canvas.addEventListener('pointerdown', (e) => {
      const r = canvas.getBoundingClientRect();
      const [px, py] = [e.clientX - r.left, e.clientY - r.top];
      // A press on the map moves the screen to where it was pressed, and on as the pointer moves while it is held.
      if (this.map.visible && this.onMap(px, py)) {
        this.mapDragging = true;
        this.mapTo(px, py);
        return;
      }
      this.dragging = { x: e.clientX, y: e.clientY };
      this.down = { x: e.clientX, y: e.clientY };
    });
    window.addEventListener('pointermove', this.drag);
    window.addEventListener('pointerup', this.up);
    document.addEventListener('visibilitychange', this.run);
    this.app.ticker.add(this.tick);
    this.ready = true;
    this.run();
  }

  /// Draws the tree once as it stands now, whether or not it draws its frames: the console shows every project's tree
  /// in the overview, but only the one shown keeps drawing.
  paintOnce(): void {
    if (this.tree === null) return;
    this.step(0);
    this.app.renderer.render(this.app.stage);
  }

  /// How wide and how tall the map at the top left is, in pixels, as it was last drawn.
  mapSize(): { width: number; height: number } {
    return { width: this.mapWide, height: this.mapHigh };
  }

  /// Shows the map at the top left, or hides it, at once: while the trees move, the console holds a picture of the map
  /// still in its place ([`mapPicture`]), and none while the overview shows every tree small.
  showMap(shown: boolean): void {
    this.map.visible = shown;
    this.mark('map');
  }

  /// The map as it stands now, copied from the tree drawn with it onto a canvas of its own, as large as it stands on the
  /// screen and cut to its round corners, and where its top left corner stands on the screen; none before the tree is
  /// drawn. The tree is drawn again as it was before the copy as it draws its next frame.
  mapPicture(): { canvas: HTMLCanvasElement; x: number; y: number } | null {
    if (this.tree === null) return null;
    const was = this.map.visible;
    this.map.visible = true;
    this.paintOnce();
    const at = this.map.getLocalBounds();
    const x = this.map.x + at.x;
    const y = this.map.y + at.y;
    // The frame's line stands half its width outside the frame.
    const line = (at.width - this.mapWide) / 2;
    const canvas = this.copy(x, y, Math.ceil(at.width), Math.ceil(at.height), MAP_CORNER + line);
    this.map.visible = was;
    return { canvas, x, y };
  }

  /// The whole tree as it stands now, drawn and copied onto a canvas of its own as large as the scene's; none before the
  /// tree is drawn.
  picture(): HTMLCanvasElement | null {
    if (this.tree === null) return null;
    this.paintOnce();
    return this.copy(0, 0, this.app.screen.width, this.app.screen.height, 0);
  }

  /// The part of what the scene drew last that stands `width` by `height` from `x`, `y` on the screen, copied onto a
  /// canvas of its own as large as the part on the screen, and cut to round corners of radius `corner`.
  private copy(x: number, y: number, width: number, height: number, corner: number): HTMLCanvasElement {
    const r = this.app.renderer.resolution;
    const canvas = document.createElement('canvas');
    canvas.width = Math.round(width * r);
    canvas.height = Math.round(height * r);
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
    const pen = canvas.getContext('2d');
    if (pen !== null) {
      if (corner > 0) {
        pen.beginPath();
        pen.roundRect(0, 0, canvas.width, canvas.height, corner * r);
        pen.clip();
      }
      pen.drawImage(this.app.canvas, x * r, y * r, canvas.width, canvas.height, 0, 0, canvas.width, canvas.height);
    }
    return canvas;
  }

  /// How finely the scene draws, as a part of the screen's own pixels: all of them while its tree fills the room, and
  /// as few as the tree stands small on the screen while the console shows it so, as in its overview, so that a tree
  /// costs no more to draw than what of it is seen. Says whether that changed: the canvas then stands empty until the
  /// scene draws its next frame, or is drawn ([`paintOnce`], [`drawNow`]).
  setDetail(part: number): boolean {
    const resolution = window.devicePixelRatio * part;
    const host = this.app.resizeTo;
    if (!this.ready || this.app.renderer.resolution === resolution || !(host instanceof HTMLElement)) return false;
    this.app.renderer.resize(host.clientWidth, host.clientHeight, resolution);
    return true;
  }

  /// Draws the whole tree on the screen at once, with no glide, while the console's overview shows every project's tree,
  /// so that each shows how its whole project stands; and once the overview closes, the view it had before.
  showWhole(on: boolean): void {
    if (on) {
      this.gridBack ??= { zoom: this.zoom, at: this.aim, left: this.left };
      this.fitNow();
    } else if (this.gridBack !== null) {
      [this.zoom, this.at, this.left] = [this.gridBack.zoom, this.gridBack.at, this.gridBack.left];
      this.aim = this.at;
      this.gridBack = null;
    }
    this.goal = null;
    this.anchor = null;
    this.leftGoal = null;
    this.mark('view');
  }

  /// Makes the whole tree fit the screen at once, in its middle.
  private fitNow(): void {
    if (this.tree === null) return;
    const zoom = this.fitZoom();
    this.zoom = zoom;
    this.at = this.tall / 2;
    this.aim = this.at;
    this.left = (this.app.screen.width / zoom - this.extent().width) / 2;
  }

  /// Draws the frame of this moment, however long it is since the one before, without waiting for the browser to give
  /// one: the console draws its trees so, in turn, while its overview shows them all.
  drawNow(): void {
    if (this.tree === null) return;
    const now = performance.now();
    const step = this.advance(now);
    this.draw(this.tree, now, step);
    this.app.renderer.render(this.app.stage);
  }

  /// Draws while the view is shown, and stops while another view hides it ([`run`]).
  setShown(shown: boolean): void {
    this.viewShown = shown;
    this.run();
  }

  /// Draws while the view and the page are both shown, and stops while either is hidden, so that a page the browser
  /// does not show draws nothing. Shown again, it draws at the size of what holds it, which its observer follows, and
  /// the time it stood stopped is not counted as a frame in `stats`.
  private run = () => {
    if (!this.ready || this.gone) return;
    const draws = this.viewShown && document.visibilityState !== 'hidden';
    if (draws === this.app.ticker.started) return;
    if (draws) {
      this.mark('shown');
      this.app.queueResize();
      this.last = 0;
      this.app.ticker.start();
    } else this.app.ticker.stop();
  };

  destroy(): void {
    this.gone = true;
    document.removeEventListener('visibilitychange', this.run);
    window.removeEventListener('pointermove', this.drag);
    window.removeEventListener('pointerup', this.up);
    this.sized?.disconnect();
    this.themes?.root.disconnect();
    this.themes?.media.removeEventListener('change', this.retheme);
    this.themes?.accent();
    if (this.ready) this.app.destroy(KEEP_SHARED, { children: true, texture: true });
  }

  /// Takes the theme's colours, and draws the tree again in them where it stands.
  private retheme = () => {
    if (!this.ready || this.gone) return;
    Object.assign(PALETTE, themed());
    this.lighting();
    this.fades = { top: vertical(PALETTE.page, 1, 0), bottom: vertical(PALETTE.page, 0, 1) };
    this.app.renderer.background.color = PALETTE.page;
    this.themed += 1;
    if (this.roots !== null) this.show(this.roots);
  };

  /// Where the tree stands, for the tests: how large it is drawn, the row at the middle of the screen and where the wheel
  /// has taken it, how far right it is moved, the size of the screen, and how many folds are turning.
  place(): {
    zoom: number;
    at: number;
    aim: number;
    left: number;
    height: number;
    width: number;
    turns: number;
  } {
    return {
      zoom: this.zoom,
      at: this.at,
      aim: this.aim,
      left: this.left,
      height: this.app.screen.height,
      width: this.app.screen.width,
      turns: this.turns.size,
    };
  }

  /// How much of the tree the screen shows, in pixels at the tree's full size.
  private seen(): { width: number; height: number } {
    const { width, height } = this.app.screen;
    return { width: width / this.zoom, height: height / this.zoom };
  }

  /// The item that is to stay where it is on the screen while the tree changes round it, by its index in the tree: the
  /// item selected, where it is on the screen; else the item under the pointer; else the item whose middle is nearest
  /// the middle of the screen, of the columns the screen shows. None while the tree has no item on the screen.
  private anchorItem(t: Tree): number {
    const { width, height } = this.seen();
    const on = (k: number) => {
      const d = t.depth[k];
      const x = this.left + d * PITCH;
      const y = height / 2 + (this.middle(d, t.rank[k]) - this.at) * ROW;
      return x + NODE_W > 0 && x < width && y > 0 && y < height;
    };
    const chosen = this.selected === null ? undefined : this.real.get(this.selected);
    if (chosen !== undefined && on(chosen)) return chosen;
    if (this.under >= 0 && this.under < t.parent.length && this.shadows[this.under] === 0) return this.under;
    let best = -1;
    let gap = Number.POSITIVE_INFINITY;
    for (let d = 0; d < t.columns.length; d += 1) {
      const x = this.left + d * PITCH;
      if (x + NODE_W <= 0 || x >= width) continue;
      const r = Math.min(t.columns[d].length - 1, this.rankAt(d, this.at));
      for (const q of [r - 1, r]) {
        if (q < 0) continue;
        const k = t.columns[d][q];
        const g = Math.abs(this.middle(d, q) - this.at);
        if (this.shadows[k] === 0 && g < gap) [best, gap] = [k, g];
      }
    }
    return best;
  }

  /// Draws the tree of `roots` in place of the one before. The item [`anchorItem`] picks of the tree before stays where
  /// it was on the screen, where the new tree still holds it, and else the tree stays where it was; at first the middle
  /// root stands in the middle.
  show(roots: Shown[]): void {
    // What it would draw is what is drawn: nothing is made again. A read of the items that changes nothing the cards
    // draw, as the engine tells of every round, costs only this.
    this.turn(roots);
    const key = `${this.themed}|${[...this.turns.keys()].join(',')}|${JSON.stringify(roots, undrawn)}`;
    if (key === this.drawnKey) {
      this.stats.made = 0;
      return;
    }
    this.drawnKey = key;
    this.mark('tree');
    const old = this.tree;
    // The item that stays where it was on the screen: the one already held while the folds turn, else the one picked
    // now, with how far from the middle of the screen it stands.
    if (this.anchor === null && old !== null && old.parent.length > 0) {
      const k = this.anchorItem(old);
      if (k >= 0) this.anchor = { id: this.ids[k], off: this.middle(old.depth[k], old.rank[k]) - this.at };
    }
    this.roots = roots;
    this.tones = tonesOf(roots);
    // What eases by item, kept by the item into the new tree: the lights round it, the strength of the way into it,
    // and what is in focus.
    const byId = (m: Map<number, number>) =>
      new Map([...m].flatMap(([k, a]) => (this.shadows[k] === 1 ? [] : [[this.ids[k], a] as const])));
    const easedBy = { glowing: byId(this.glowing), runStrength: byId(this.runStrength) };
    const focusIds = [...this.inFocus].flatMap((k) => (this.shadows[k] === 1 ? [] : [this.ids[k]]));
    // One set of text styles for every tree, each made as it is first asked for.
    this.styles ??= makeStyles();
    this.titleOffset = this.titleOffsetFor(roots);
    // The items under an item folded are not drawn, but while its fold turns; what is under it counts at it, where the
    // tree marks what is under an item.
    const { tree, items } = indexTree(roots, (i) =>
      i.fold?.folded === true && !this.turns.has(i.id) ? [] : i.children,
    );
    const hides = (k: number, is: (s: Shown) => boolean) => items[k].fold?.folded === true && beneath(items[k], is);
    this.tree = tree;
    this.ids = items.map((i) => i.id);
    // The grounds under the cards drawn larger are kept by the items' places in the tree before.
    for (const ground of this.lensGrounds.values()) ground.destroy();
    this.lensGrounds.clear();
    this.shownItems = items;
    this.shadows = Uint8Array.from(items, (i) => (i.shadow ? 1 : 0));
    this.real = new Map();
    this.shadowsOf = new Map();
    items.forEach((it, k) => {
      if (!it.shadow) this.real.set(it.id, k);
      else this.shadowsOf.set(it.id, [...(this.shadowsOf.get(it.id) ?? []), k]);
    });
    const back = (m: Map<number, number>) =>
      new Map(
        [...m].flatMap(([id, a]) => {
          const k = this.real.get(id);
          return k === undefined ? [] : [[k, a] as const];
        }),
      );
    this.inFocus = new Set(focusIds.flatMap((id) => (this.real.has(id) ? [this.real.get(id) as number] : [])));
    this.under = -1;
    this.underFold = -1;
    this.drawnMoved = null;
    this.waysDrawn = [];
    this.base = Float64Array.from(items, (i) =>
      i.work === null || i.work.from === null ? 1 : 1 + WORK_ROW,
    );
    this.shape();
    this.fresh = tree.columns.map((col) =>
      col.flatMap((i, r) => (somethingUnread(items[i]) || hides(i, somethingUnread) ? [r] : [])),
    );
    // Each item's tone, and whether its own work moves.
    this.toneAt = items.map((i) => this.tones.tone.get(i) ?? 'grey');
    this.movesAt = Uint8Array.from(items, (i) => (this.tones.moves.has(i) ? 1 : 0));
    // The items whose work has begun, and those above them: an item with nothing begun on it or under it is quieter.
    this.begun = new Uint8Array(items.length);
    const begun = (s: Shown) => s.begun;
    items.forEach((i, k) => {
      if (!begun(i) && !hides(k, begun)) return;
      for (let p = k; p >= 0 && this.begun[p] === 0; p = tree.parent[p]) this.begun[p] = 1;
    });
    this.pulses = new Map();
    this.beat = beatOf();
    // The way from each item whose own work moves up to its root, and from each the owner paused after it began, under
    // a turning fold as well: there it is drawn with the cards as they draw in to the dot or grow out of it ([`ends`] in
    // `draw`). Each length of it is in the tone of the item it leads to, the paused one's a plain grey line. The dot of
    // an item folded, or whose fold turns, with such an item under it ends the way: the way runs from the root through
    // the item's card to the dot, which comes and goes with the turn.
    const running = (s: Shown) =>
      this.tones.moves.has(s) ||
      (!s.shadow && s.stands !== null && s.stands.now.kind === 'paused' && s.stands.now.ran && s.stands.now.by === 'owner');
    const turning = (k: number) => this.turns.has(items[k].id);
    const inside = new Uint8Array(items.length);
    items.forEach((_, k) => {
      const p = tree.parent[k];
      inside[k] = p >= 0 && (inside[p] === 1 || turning(p)) ? 1 : 0;
    });
    this.litDots = items.flatMap((i, k) =>
      inside[k] === 0 && (i.fold?.folded === true || turning(k)) && beneath(i, running) ? [k] : [],
    );
    this.runWays = waysUp(
      [...items.flatMap((i, k) => (running(i) ? [k] : [])), ...this.litDots],
      tree.parent,
    );
    this.runStrength = new Map([...back(easedBy.runStrength)].filter(([k]) => this.runWays.has(k)));
    ({ measures: this.measures, looks: this.looks } = cellLooks());
    this.drawnStrips = new Map();
    this.glowing = back(easedBy.glowing);
    this.ages = new Map();
    this.strips = [];
    this.cardsDrawn = new Map();
    this.tagsDrawn = new Map();
    this.numbersDrawn = new Map();
    // Each card is made only where no card drawn before draws the same: else the one drawn is kept, with what its
    // making set. What a card draws is its item as the cards draw it, the rows it stands in, whether a work goes on on
    // it or under it, whether one has begun there, and the theme. A card made new comes in as dim as the tree is.
    const reuse = this.cardCache;
    const sigs = items.map((item, k) => {
      const drawn = JSON.stringify({ ...item, children: undefined }, undrawn);
      return `${this.themed}|${this.titleOffset}|${this.base[k]}|${this.toneAt[k]}|${this.begun[k]}|${drawn}`;
    });
    const kept: (CardMade | undefined)[] = sigs.map((sig) => reuse.get(sig)?.pop());
    const make = (k: number) => {
      const view = this.node(items[k], k);
      view.alpha = this.dim;
      return view;
    };
    this.views = items.map((_, k) => kept[k]?.view ?? make(k));
    // One size for the marks of every strip on the tree: the largest at which each strip fits its room, the strip of
    // the flow with the most cells the narrowest, and never larger than the marks' own; so that a mark is as large on
    // one card as on any other ([`strip`]). A card kept whose strip was drawn at another size is made again.
    const { side, gap } = this.measures;
    const fits = (q: { start: number; end: number; count: number }) => {
      const whole = q.count * (side + gap) - gap;
      return whole > 0 ? (q.end - q.start) / whole : 1;
    };
    const fit = (cards: (CardMade | undefined)[]) =>
      Math.max(
        0,
        Math.min(1, ...this.strips.map(fits), ...cards.flatMap((c) => (c?.strip == null ? [] : [fits(c.strip)]))),
      );
    let scale = fit(kept);
    if (scale !== this.stripScale) {
      kept.forEach((c, k) => {
        if (c?.strip == null) return;
        c.view.removeFromParent();
        c.view.destroy({ children: true });
        kept[k] = undefined;
        this.views[k] = make(k);
      });
      scale = fit(kept);
    }
    this.stripScale = scale;
    const stripAt = new Map(this.strips.map((q) => [q.k, { start: q.start, end: q.end, count: q.count }] as const));
    for (const q of this.strips) q.draw(scale);
    this.strips = [];
    // What a kept card's making set, set again where it now stands.
    kept.forEach((c, k) => {
      if (c === undefined) return;
      if (c.age !== undefined) this.ages.set(k, c.age);
      if (c.pulse !== undefined) this.pulses.set(k, c.pulse);
      if (c.drawnStrip !== undefined) this.drawnStrips.set(items[k].id, c.drawnStrip);
      if (c.look !== undefined) this.cardsDrawn.set(items[k].id, c.look);
      if (c.tags !== undefined) this.tagsDrawn.set(items[k].id, c.tags);
      if (c.number !== undefined) this.numbersDrawn.set(k, c.number);
    });
    // The cards no longer drawn are let go; those drawn are kept for the next tree.
    for (const left of reuse.values())
      for (const c of left) {
        c.view.removeFromParent();
        c.view.destroy({ children: true });
      }
    this.cardCache = new Map();
    items.forEach((item, k) => {
      const card: CardMade = {
        sig: sigs[k],
        view: this.views[k],
        strip: kept[k]?.strip ?? stripAt.get(k) ?? null,
        age: this.ages.get(k),
        pulse: this.pulses.get(k),
        drawnStrip: item.shadow ? undefined : this.drawnStrips.get(item.id),
        look: item.shadow ? undefined : this.cardsDrawn.get(item.id),
        tags: item.shadow ? undefined : this.tagsDrawn.get(item.id),
        number: this.numbersDrawn.get(k),
      };
      this.cardCache.set(sigs[k], [...(this.cardCache.get(sigs[k]) ?? []), card]);
    });
    this.stats.made = kept.filter((c) => c === undefined).length;
    this.shown = new Set([...this.shown].filter((v) => !v.destroyed));
    // The pointer over a column this tree has no more is over none, until it moves.
    if (this.hovered >= tree.columns.length) this.hovered = -1;
    if (tree.columns.length === 0) {
      this.buildMap();
      this.stats.items = 0;
      return;
    }
    if (old === null || old.parent.length === 0) {
      this.at = this.middle(0, Math.floor((tree.columns[0].length - 1) / 2));
      this.aim = this.at;
    } else this.hold();
    // Once no fold turns, the item held stays no longer: the next change picks again.
    if (this.turns.size === 0) this.anchor = null;
    // While every project's tree is shown at once, this one stays whole as it changes.
    if (this.gridBack !== null) this.fitNow();
    this.buildMap();
    this.stats.items = items.length;
    if (this.selected !== null && !this.real.has(this.selected)) this.selected = null;
    // An item a click on its shadow asked for, once the folds over it are open and it is drawn whole, comes to the
    // middle of the screen.
    if (this.toFocus !== this.selected) this.toFocus = null;
    const goal = this.toFocus === null ? undefined : this.real.get(this.toFocus);
    if (goal !== undefined && this.scaleOf[goal] === 1) {
      this.focus(tree.depth[goal], tree.rank[goal]);
      this.toFocus = null;
    }
    this.announce();
  }

  /// Keeps the item held ([`anchor`]) where it stands on the screen: the tree moves so that it does. Nothing while no
  /// item is held, or where the tree no longer holds it.
  private hold(): void {
    const t = this.tree;
    const a = this.anchor;
    const k = a === null ? undefined : this.real.get(a.id);
    if (t === null || a === null || k === undefined) return;
    this.at = this.middle(t.depth[k], t.rank[k]) - a.off;
    this.aim = this.at;
  }

  /// Begins the turn of each fold that `roots` shows folded the other way than the tree before did: from where it
  /// stands, if it was turning already, else from the other end, over as much of the turn's time as it has to go. A
  /// fold no longer shown turns no more.
  private turn(roots: Shown[]) {
    const folded = new Map<number, boolean>();
    const walk = (s: Shown) => {
      if (s.shadow) return;
      if (s.fold != null) folded.set(s.id, s.fold.folded);
      s.children.forEach(walk);
    };
    roots.forEach(walk);
    for (const [id, now] of folded) {
      const was = this.wasFolded.get(id);
      if (was === undefined || was === now) continue;
      const turning = this.turns.get(id);
      const from = turning === undefined ? (now ? 1 : 0) : sizeOf(turning, this.clock);
      const to = now ? 0 : 1;
      this.turns.set(id, { to, from, at: this.clock, ms: (now ? FOLD_MS : UNFOLD_MS) * Math.abs(to - from) });
    }
    for (const id of [...this.turns.keys()]) if (!folded.has(id)) this.turns.delete(id);
    this.wasFolded = folded;
  }

  /// Ends each fold whose turn is over, and draws the tree again: without what a fold that ended folded away, and with
  /// the ways of what one that ended shows lit.
  private endTurns() {
    let ended = false;
    for (const [id, turn] of [...this.turns]) {
      if (this.clock - turn.at < turn.ms) continue;
      this.turns.delete(id);
      ended = true;
    }
    if (ended && this.roots !== null) this.show(this.roots);
  }

  /// How large the items under the item at `k` are drawn by its fold's turn, from 0 to 1: whole where it does not turn.
  private sizeUnder(k: number): number {
    const turning = this.turns.get(this.ids[k]);
    return turning === undefined ? 1 : sizeOf(turning, this.clock);
  }

  /// How much of the dot of the item at `k` is drawn: all of it while it is folded, none while it is shown, and while
  /// its fold turns, as much as the items under it have drawn in.
  private dotness(k: number): number {
    const turning = this.turns.get(this.ids[k]);
    if (turning !== undefined) return 1 - sizeOf(turning, this.clock);
    return this.shownItems[k].fold?.folded === true ? 1 : 0;
  }

  /// Sizes and places every item as the folds turning stand now: an item under a turning fold at that fold's size,
  /// times the size of any fold turning over that, its rows and the room above its group with it; each item where
  /// [`layOut`] puts it at those sizes.
  private shape() {
    const t = this.tree;
    if (t === null) return;
    const scale = new Float64Array(this.ids.length);
    for (let k = 0; k < scale.length; k += 1) {
      const p = t.parent[k];
      scale[k] = p < 0 ? 1 : scale[p] * this.sizeUnder(p);
    }
    this.scaleOf = scale;
    this.rows = Float64Array.from(this.base, (r, k) => r * scale[k]);
    this.tops = layOut(t, this.rows, scale);
    let tall = 0;
    for (let k = 0; k < this.tops.length; k += 1) tall = Math.max(tall, this.tops[k] + this.rows[k]);
    this.tall = tall;
  }

  /// How each item is moved while folds turn: under a turning fold, drawn in towards its dot by as much as that fold
  /// has turned, and moved with whatever moves that fold's item. Each item `k` is drawn at its place times `a[k]`, then
  /// moved by (`bx[k]`, `by[k]`), and its size times `a[k]`.
  private movedNow(t: Tree, x: (d: number) => number, y: (d: number, rank: number) => number) {
    const n = this.ids.length;
    const [a, bx, by] = [new Float64Array(n), new Float64Array(n), new Float64Array(n)];
    for (let k = 0; k < n; k += 1) {
      const p = t.parent[k];
      if (p < 0) {
        a[k] = 1;
        continue;
      }
      const f = this.sizeUnder(p);
      const [qx, qy] = f === 1 ? [0, 0] : [x(t.depth[p]) + NODE_W + DOT_AT, y(t.depth[p], t.rank[p])];
      a[k] = a[p] * f;
      bx[k] = a[p] * (1 - f) * qx + bx[p];
      by[k] = a[p] * (1 - f) * qy + by[p];
    }
    return { a, bx, by };
  }

  /// How each item is moved and sized with the items near the pointer drawn larger where they stand ([`lensSize`]),
  /// `lens` of the way there, after what `fold` moves it by while folds turn: each card grows round its own middle, as
  /// `fold` draws it. Each item `k` is drawn at its place times `a[k]`, then moved by (`bx[k]`, `by[k]`), and its size
  /// times `a[k]`, as [`movedNow`] gives it.
  private magnified(
    t: Tree,
    x: (d: number) => number,
    y: (d: number, rank: number) => number,
    fold: { a: Float64Array; bx: Float64Array; by: Float64Array } | null,
    lens: number,
  ) {
    const n = this.ids.length;
    const [a, bx, by] = [new Float64Array(n), new Float64Array(n), new Float64Array(n)];
    const p = this.lensAt;
    const z = this.zoom;
    for (let k = 0; k < n; k += 1) {
      const d = t.depth[k];
      const [fa, fx, fy] = fold === null ? [1, 0, 0] : [fold.a[k], fold.bx[k], fold.by[k]];
      const wide = NODE_W * fa;
      const left = fa * x(d) + fx;
      const mid = fa * y(d, t.rank[k]) + fy;
      const centre = left + wide / 2;
      const rows = Math.abs(mid - p.y) / ROW;
      const columns = Math.max(0, Math.abs(centre - p.x) - wide / 2) / PITCH;
      const s = 1 + (lensSize(rows, columns, z) / z - 1) * lens;
      a[k] = s * fa;
      bx[k] = centre + s * (fx - centre);
      by[k] = mid + s * (fy - mid);
    }
    return { a, bx, by };
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

  /// How far from a card's number's left its title begins: past the widest number the whole tree holds, folded items
  /// and shadows included, so that a fold, which leaves out the items under it, moves no title. The numbers are in
  /// monospace, so the width follows from the digits alone; it is measured once for each count of them.
  private titleOffsetFor(roots: Shown[]): number {
    let widest = 0;
    const walk = (items: Shown[]) => {
      for (const i of items) {
        widest = Math.max(widest, i.id);
        walk(i.children);
      }
    };
    walk(roots);
    const digits = String(widest).length;
    let wide = this.numberWidths.get(digits);
    if (wide === undefined) {
      const style = (this.styles as Styles).mono(PALETTE.pale);
      const probe = new BitmapText({ text: itemName(10 ** digits - 1), style });
      wide = probe.width;
      probe.destroy();
      this.numberWidths.set(digits, wide);
    }
    return Math.max(TITLE_AT, wide + TITLE_GAP);
  }

  /// The node of an item: a card, white, or while a work goes on on it a faint tint of the colour of what the work does
  /// with an edge of a deeper shade of it; its edge the accent's tint above a work. On its first row its number in its
  /// state's colour, its title, and at the right how many things its work waits for from the owner, white on red; on
  /// the second row, for an item whose work goes on, under the number the level its cell runs at, the strip of the
  /// work's cells as the works page draws it, the lines the work added and removed where it changed any, and how long
  /// it has gone on. An item whose work has ended has its one row, with the lines its work changed at its right; an
  /// item whose work has not run a round has its one row too, in the colours of how its work stands. A node is drawn at
  /// its whole size, and made smaller as a whole while a fold over it turns.
  private node(item: Shown, k: number): Container {
    const s = this.styles as Styles;
    const view = new Container();
    const h = high(this.base[k]);
    const top = -h / 2;
    const row1 = top + NODE_H / 2;
    if (item.shadow) return this.shadow(item, k, view, top, h, row1);
    const card = new Graphics().roundRect(0, top, NODE_W, h, CARD_CORNER);
    // Every card, a deliverable's as a work's, says who moves it next in the same colours (`Tone`): red and blue fill
    // it, edged in a deeper shade; yellow, a work waiting its turn, edges it in its yellow with no fill.
    const tone = this.tones.tone.get(item) ?? 'grey';
    const toned = TONE_COLOUR()[tone];
    const fill = tone === 'red' || tone === 'blue' ? mix(PALETTE.card, toned, FILL) : PALETTE.card;
    const edge =
      tone === 'red' || tone === 'blue'
        ? mix(PALETTE.card, toned, tone === 'blue' ? EDGE_RUNNING : EDGE_TINT)
        : tone === 'yellow'
          ? mix(PALETTE.card, toned, EDGE_TINT)
          : PALETTE.edge;
    view.addChild(card.fill(fill).stroke({ color: edge, width: EDGE_WIDTH }));
    this.cardsDrawn.set(item.id, { rows: this.base[k], fill, edge });
    // What is new to the owner is said once, by the number at the card's right; no line at its left.
    const x = CARD_INSET;
    const number = new BitmapText({
      text: itemName(item.id),
      style: s.mono(NUMBER()[item.standing]),
    });
    number.position.set(x, row1 - number.height / 2);
    // An open item nothing has begun on or under is a little greyer than one in work, so that it stands back.
    const ink =
      item.standing !== 'open'
        ? PALETTE.faint
        : this.begun[k] === 1
          ? PALETTE.ink
          : between(PALETTE.ink, PALETTE.muted, NOT_BEGUN);
    const title = new BitmapText({ text: item.title, style: s.title(ink) });
    title.position.set(x + this.titleOffset, row1 - title.height / 2);
    this.numbersDrawn.set(k, { text: number.text, titleAt: title.x });
    view.addChild(number, title);
    // At the right of the first row: how many things its work waits for from the owner, white on red, the strongest
    // mark a card has, as what the owner looks for first; beside it, how many questions the owner may answer ahead,
    // which hold the work at nothing yet, white on orange; and its replies unread, white on the accent. What an item
    // waits for stands under it in the tree, and no tag counts it again. Its returns sent back are counted on the
    // System view and marked on the work's history, not on the card.
    let right = NODE_W - 8;
    const tags: DrawnTag[] = [];
    const tag = (text: string, fill: string) => {
      tags.push({ text, fill });
      const t = new BitmapText({ text, style: s.mono(PALETTE.onTag) });
      const pad = 4;
      const w = t.width + 2 * pad;
      right -= w;
      const g = new Graphics();
      g.roundRect(right, row1 - 8, w, 16, 3).fill(fill);
      t.position.set(right + pad, row1 - t.height / 2);
      view.addChild(g, t);
      right -= 6;
    };
    // What waits for the owner in red, the strongest, as the engine's one list counts it; then the replies the owner has
    // not read, in the colour the map marks what is not read in.
    if (item.asks > 0) tag(String(item.asks), PALETTE.red);
    if (item.ahead > 0) tag(String(item.ahead), PALETTE.orange);
    if (item.repliesUnread > 0) tag(String(item.repliesUnread), PALETTE.unseen);
    this.tagsDrawn.set(item.id, tags);
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
    // The title ends where the badges begin. A cancelled item's is struck through in its own colour, so that it is told
    // from a done one's.
    fit(title, right - title.x);
    if (item.standing === 'cancelled') {
      const through = Math.round(row1) + 0.5;
      const strike = new Graphics().moveTo(title.x, through).lineTo(title.x + title.width, through);
      view.addChild(strike.stroke({ color: ink, width: 1 }));
    }
    // A work that has not run a round has no second row: its card says how it stands by its colours alone.
    const w = item.work;
    if (w === null || w.from === null) return view;
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
    // What pointing at it says, as the item's window says it (`levelPointed`), by where it stands on the card.
    LEVEL_SAID.set(view, {
      x: level.x,
      y: level.y,
      width: level.width,
      height: level.height,
      said: levelPointed(w.level, w.raise, w.held, w.cellLevel),
    });
    // The second row: the work's strip, as the works page draws it, from under the title, so that the number stands
    // alone in a column at the card's left, or from past the level where it is wider than that column; ending short of
    // the lines the work changed. It is drawn once every card is made, at the one size of the tree's marks ([`show`]).
    const [stopped, queued] = [stoppedAt(w), waitsItsTurn(w)];
    const start = Math.max(title.x, x + level.width + 4);
    const end = LINES_X - 8;
    this.strips.push({
      k,
      start,
      end,
      count: w.cells.length,
      draw: (scale) => {
        this.strip(view, item.id, k, w.cells, start, row2, end, scale, stopped, queued);
        const drawn = this.drawnStrips.get(item.id);
        if (drawn !== undefined) drawn.level = { text: level.text, colour, strong };
      },
    });
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
  /// order of the walk, as the works page's (`state-marks.ts`), the cell the work is `stopped` at as stopped and, the
  /// work waiting its turn (`queued`), the cell it waits at as queued, from `x` and no further than `end`, their middles
  /// on `y`. Every cell is drawn: the marks, the gaps between them, the dashes' height, the corners and the rings are all
  /// `scale` of the works page's, which the tree makes small enough for its longest strip to fit ([`show`]); a ring is
  /// at least a pixel wide. The marks of cells running fade with the beat while their rounds are under way. What it
  /// drew is kept, for the tests.
  private strip(
    view: Container,
    id: number,
    k: number,
    cells: CellNow[],
    x: number,
    y: number,
    end: number,
    scale: number,
    stopped: string | null = null,
    queued = false,
  ) {
    const side = this.measures.side * scale;
    const pitch = side + this.measures.gap * scale;
    const marks: DrawnMark[] = cells.map((c, i) => {
      const state = cellRunState(c.state, c.cell === stopped, queued);
      const look = this.looks[state];
      const height = look.height * scale;
      return {
        cell: c.cell,
        state: c.state,
        x: x + i * pitch,
        y: y - height / 2,
        width: side,
        height,
        corner: look.corner * scale,
        fill: look.fill,
        edge: look.edge,
        ring: look.ring > 0 ? Math.max(1, look.ring * scale) : 0,
        ringed: look.edge !== null && look.edge !== look.fill,
        pulses: pulses(state),
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
    if (marks.some((m) => m.pulses)) this.pulses.set(k, running);
    this.drawnStrips.set(id, { marks, more: 0, end, level: null });
  }

  /// How the card of item `id` was drawn, for the tests: how many rows high, and its fill and its edge's colour. None
  /// for an item the tree does not draw, and a shadow.
  cardLook(id: number): { rows: number; fill: string | number; edge: string | number } | null {
    return this.cardsDrawn.get(id) ?? null;
  }

  /// The tags the card of item `id` drew at the right of its first row, from the right: the number of things its work
  /// waits for from the owner, of questions to answer ahead, of replies not read, and of returns sent back, each with its
  /// colour, any with none left out; for the tests. None for an item the tree does not draw, and a shadow.
  drawnTags(id: number): DrawnTag[] | null {
    return this.tagsDrawn.get(id) ?? null;
  }

  /// The number each card of the tree wrote, shadows and deliverables too, in the tree's order, and where its title
  /// begins from the card's left, for the tests.
  drawnNumbers(): { id: number; shadow: boolean; text: string; titleAt: number }[] {
    return this.shownItems.flatMap((item, k) => {
      const drawn = this.numbersDrawn.get(k);
      return drawn === undefined ? [] : [{ id: item.id, shadow: item.shadow, ...drawn }];
    });
  }

  /// What the card of item `id` drew for its work's cells, for the tests: each mark, where it stands from the card's
  /// left and its middle, its measures and colours, and whether it pulses; how many cells were left out, none; where
  /// the room for the strip ends; and the level under the number. None for an item whose card has no work.
  drawnStrip(id: number): DrawnStrip | null {
    return this.drawnStrips.get(id) ?? null;
  }

  /// Where the card of item `id` stands on the canvas, its left side and its middle; and, as at the last frame, where
  /// that point was drawn and how large the card was, which differ while a fold over it turns; for the tests. None
  /// for an item the tree does not draw.
  cardAt(id: number): { x: number; y: number; size: number; drawn: [number, number] } | null {
    const t = this.tree;
    const i = this.real.get(id);
    if (t === null || i === undefined) return null;
    const d = t.depth[i];
    const z = this.zoom;
    // Where the card stands in the frame the tree is drawn in ([`Frame`]), and how far the screen has moved from it.
    const top = this.seen().height / 2 - this.at * ROW;
    const f = this.frame ?? { left: this.left, top };
    const [dx, dy] = [this.left - f.left, top - f.top];
    const [x, y] = [f.left + d * PITCH, f.top + this.middle(d, t.rank[i]) * ROW];
    const shown = (px: number, py: number): [number, number] => [(px + dx) * z, (py + dy) * z];
    const [sx, sy] = shown(x, y);
    const m = this.drawnMoved;
    if (m === null) return { x: sx, y: sy, size: z, drawn: [sx, sy] };
    return { x: sx, y: sy, size: m.a[i] * z, drawn: shown(m.a[i] * x + m.bx[i], m.a[i] * y + m.by[i]) };
  }

  /// How bright the card of item `id` is drawn, for the tests: fully, or dimmed while something else is lit, eased
  /// between. None for an item the tree does not draw.
  brightness(id: number): number | null {
    const i = this.real.get(id);
    return i === undefined ? null : this.views[i].alpha;
  }

  /// The lit ways drawn at the last frame, each by the number of the item it runs into, from where it began to where
  /// it ended, with its curve and its turns into the cards, for the tests.
  litWays(): LitWay[] {
    return [...this.waysDrawn];
  }

  /// The light along the edges of the item selected as it was last drawn, and so stands, for the tests: the item's
  /// number, how far on its flash is (`out`, from 0 to 1), and how strong the light is. None while no item selected is
  /// drawn whole.
  chosenLight(): { id: number; out: number; alpha: number } | null {
    return this.chosenDrawn;
  }

  /// The shadow of an item under another item that depends on it besides its main one: a white card, its edge dashed,
  /// its number and title grey, and at the right an arrow pointing away, to where the item stands.
  private shadow(item: Shown, k: number, view: Container, top: number, h: number, row1: number): Container {
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
    const number = new BitmapText({ text: itemName(item.id), style: s.mono(PALETTE.pale) });
    number.position.set(CARD_INSET, row1 - number.height / 2);
    const title = new BitmapText({ text: item.title, style: s.title(PALETTE.faint) });
    title.position.set(CARD_INSET + this.titleOffset, row1 - title.height / 2);
    this.numbersDrawn.set(k, { text: number.text, titleAt: title.x });
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

  /// Brings the item of rank `rank` in column `d` to the middle of the screen, up and down.
  private focus(d: number, rank: number): void {
    const t = this.tree;
    if (t === null) return;
    this.anchor = null;
    this.aim = this.middle(d, Math.max(0, Math.min(t.columns[d].length - 1, rank)));
  }

  /// How many rows the item of rank `rank` in column `d` takes.
  private rowsAt(d: number, rank: number): number {
    const col = (this.tree as Tree).columns[d];
    return this.rows[col[Math.max(0, Math.min(col.length - 1, rank))]];
  }

  /// The middle of the item of rank `rank` in column `d`, in rows from the tree's top.
  private middle(d: number, rank: number): number {
    return this.slot(d, rank) + this.rowsAt(d, rank) / 2;
  }

  /// Adds to the funnels, or to `g`, the one from an item ending at `px`, its box from `top` to `bottom`, to items
  /// starting at `cx`, the first's box starting at `first` and the last's ending at `last`.
  private funnel(px: number, top: number, bottom: number, cx: number, first: number, last: number, g = this.links) {
    const bend = (cx - px) / 2;
    g.moveTo(px, top)
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
    // A layer blurred is laid on the page by its blur, which takes the blend from its filter and not its own; each
    // line in a group of lines is laid as the group is.
    for (const lit of [this.ways, this.runEdges, this.ends, this.edgeLight, this.toneEdges, this.toneEdgesLifted]) {
      lit.nearBlur.blendMode = how;
      lit.farBlur.blendMode = how;
    }
    this.ways.lines.blendMode = how;
    this.runEdges.line.blendMode = how;
    this.edgeLight.line.blendMode = how;
    this.toneEdges.line.blendMode = how;
    this.toneEdgesLifted.line.blendMode = how;
    this.ends.lines.blendMode = how;
  }

  /// Lays a light over the cards as a way's line is laid, with its glow: the line itself, and the glow close round it
  /// and further out, so that the light reaches past the line it lies along and falls away with the distance from it,
  /// each on its own of `to`. `lay` draws that line on the layer it is given, once for the line and once for each
  /// glow; `wide` is the part of a way's widths they take, and `alpha` the strength of all of them. The glow is fainter
  /// where the page is light, as a way's is ([`RUN.inkRest`]).
  private coats(
    to: Strokes,
    lay: (g: Graphics) => void,
    colour: number,
    core: number,
    alpha: number,
    wide: number,
    nose = false,
  ) {
    const coat = (g: Graphics, width: number, strength: number, paint: number) => {
      lay(g);
      const cap = nose ? 'round' : 'butt';
      g.stroke({ color: paint, width: width * wide, alpha: alpha * strength, cap });
    };
    const rest = this.adds ? 1 : RUN.inkRest;
    coat(to.far, RUN.far, RUN.farAlpha * rest, colour);
    coat(to.near, RUN.near, RUN.nearAlpha * rest, colour);
    coat(to.line, this.lineWidth, RUN.lineAlpha, core);
  }

  /// The line a spark is drawn in: white where the page is dark, the hottest of the light; where it is light, the
  /// way's ink line `line`, which the glint reads within.
  private flareLine(line: number): number {
    return this.adds ? Number.parseInt(PALETTE.glowTo.slice(1), 16) : line;
  }

  /// A way's line turning into a card's edge at `(x, y)`, up and down it, from where it leaves the way through where it
  /// bends ([`turnOf`]): lit as the spark there is, at `strength`, `burn` from 0 to 1 as the light goes through it.
  /// Each way turns in along its own curve, however many ways meet the card there. It is drawn on `to`.
  private turnIn(
    to: Strokes,
    x: number,
    y: number,
    turn: { from: [number, number]; bend: [number, number] },
    tint: (bright: number) => number,
    line: number,
    strength: number,
    burn: number,
  ) {
    const [[fx, fy], [bx, by]] = [turn.from, turn.bend];
    const lay = (g: Graphics) => {
      for (const side of [-1, 1]) g.moveTo(fx, fy).quadraticCurveTo(bx, by, x, y + side * RUN.fillet);
    };
    this.coats(to, lay, tint(glintOf(burn)), this.flareLine(line), strength * burn, RUN.portWide);
  }

  /// The light where a way meets a card, at `(x, y)` on the card's edge, `half` its height, at `strength`, `burn` from
  /// 0 to 1 as the light goes through it: it lies along the edge the way meets, burning brighter as the light comes and
  /// fading out as it goes each way along it, in the light's colour as it goes through (`tint`). Along the way itself
  /// the way's own line carries it, and the way's turn into the edge ([`turnIn`]). It is drawn on `to`.
  private spark(
    to: Strokes,
    x: number,
    y: number,
    half: number,
    tint: (bright: number) => number,
    line: number,
    glint: number,
    strength: number,
    burn: number,
  ) {
    const going = glintOf(burn);
    const reach = RUN.portHold * half;
    const white = Number.parseInt(PALETTE.glowTo.slice(1), 16);
    const core = this.flareLine(line);
    for (let n = 0; n < RUN.portPieces; n += 1) {
      const [near, far] = [(reach * n) / RUN.portPieces, (reach * (n + 1)) / RUN.portPieces];
      const fade = (1 - (n + 0.5) / RUN.portPieces) ** RUN.portFall;
      const lay = (g: Graphics) => {
        for (const side of [-1, 1]) g.moveTo(x, y + side * near).lineTo(x, y + side * far);
      };
      this.coats(to, lay, tint(going), core, strength * burn * fade, RUN.portWide);
      // The glint is laid only where the page is light: where it is dark the line under it is the light's own near
      // white already, and a glint over it would only burn the card's end out.
      if (going > 0 && !this.adds) {
        lay(to.line);
        to.line.stroke({
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
      to.far.ellipse(x, y, wide * 3, wide * 3 * tall).fill(out);
      to.near.ellipse(x, y, wide * 1.5, wide * 1.5 * tall).fill({ color: white, alpha: Math.min(1, glow) });
      to.line.ellipse(x, y, wide, wide * Math.min(2, tall)).fill({ color: white, alpha: strength * burn });
    }
  }

  /// A card's edge as the light crosses it, from the card's left side to its right: the card at `x` with its middle
  /// at `y`, `half` its height and drawn `scale` times its size, and the pieces of the light `over` its crossing, each
  /// where it stands along the crossing. Each piece lights the card's top and bottom edges between the two places it
  /// stands at, and the card's own left or right edge where it is at that side, in the light's colour at that piece
  /// (`tint`). It is drawn on `to`.
  private across(
    x: number,
    y: number,
    half: number,
    over: { from: number; to: number; bright: number }[],
    tint: (bright: number) => number,
    line: number,
    glint: number,
    strength: number,
    scale = 1,
    on: Strokes = this.runEdges,
  ) {
    over.forEach(({ from, to, bright }, n) => {
      const [x0, x1] = [x + from * scale, x + to * scale];
      const nose = n === over.length - 1 && to < NODE_W - NEAR;
      const lay = (g: Graphics) => {
        g.moveTo(x0, y - half).lineTo(x1, y - half);
        g.moveTo(x0, y + half).lineTo(x1, y + half);
        if (from <= 0) g.moveTo(x, y - half).lineTo(x, y + half);
        if (to >= NODE_W) g.moveTo(x + NODE_W * scale, y - half).lineTo(x + NODE_W * scale, y + half);
      };
      this.coats(on, lay, tint(bright), line, strength * bright, RUN.edgeWide, nose);
      // The glint within that line, as on a way, and only where the page is light: where it is dark the line under it
      // is the light's own near white already.
      if (!this.adds) {
        lay(on.line);
        on.line.stroke({
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

  /// The rank of the item of column `d` at `place` rows from the tree's top, the first whose place is not above it.
  private rankAt(d: number, place: number): number {
    let lo = 0;
    let hi = (this.tree as Tree).columns[d].length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (this.slot(d, mid) + this.rowsAt(d, mid) < place) lo = mid + 1;
      else hi = mid;
    }
    return lo;
  }

  /// Where the item of rank `rank` in column `d` starts, in rows from the tree's top ([`layOut`]); a rank past either
  /// end of the column a row further on for each rank past it.
  private slot(d: number, rank: number): number {
    const col = (this.tree as Tree).columns[d];
    const r = Math.max(0, Math.min(col.length - 1, rank));
    return this.tops[col[r]] + (rank - r);
  }

  /// Draws one frame `ms` on from the last drawn, without waiting for the browser to give one: what the scene does
  /// over a span of time, drawn in one step. The browser gives a frame when it will and gives none at all to a page it
  /// does not show, so a test that waits for them waits on the machine's load; stepping, it waits on nothing.
  step(ms = 16): void {
    if (this.tree === null) return;
    const now = (this.lastDraw > 0 ? this.lastDraw : performance.now()) + ms;
    const step = this.advance(now);
    if (this.tree !== null) this.draw(this.tree, now, step);
  }

  /// Goes on to the frame drawn at `now`: the time since the frame before, by which the scene's own clock goes on; and
  /// each fold whose turn that time ends, drawn as it then stands in this same frame, however long the browser took to
  /// give it. Gives that time.
  private advance(now: number): number {
    const step = this.lastDraw > 0 ? Math.max(0, now - this.lastDraw) : 16;
    this.lastDraw = now;
    this.clock += step;
    this.endTurns();
    return step;
  }

  /// Waits until the GPU has drawn all it was asked to: a pixel read back from the canvas comes only once it has. The
  /// benches read one rather than call `finish`, which Chromium takes as a flush and does not wait on.
  private waitForGpu() {
    const gl = (this.app.renderer as unknown as { gl?: WebGLRenderingContext }).gl;
    gl?.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, this.pixel);
  }

  /// Draws `frames` frames, every sixth with a scroll of most of a row, waiting for the GPU each; gives
  /// the time of each frame in milliseconds.
  bench(frames: number): {
    meanMs: number;
    p95Ms: number;
    maxMs: number;
    drawn: number;
  } {
    const times: number[] = [];
    for (let k = 0; k < frames; k += 1) {
      const t0 = performance.now();
      if (k % 6 === 0) this.scroll(ROW * 0.7);
      this.tick();
      this.app.renderer.render(this.app.stage);
      this.waitForGpu();
      times.push(performance.now() - t0);
    }
    return { ...timesOf(times), drawn: this.stats.drawn };
  }

  /// Draws `frames` frames of the tree as it stands on a canvas `width` by `height` pixels of the page, the size of a
  /// window on a large screen, at the screen's own pixel ratio: after one that draws it at that size, frames in which
  /// nothing but the lights moves, each 16 milliseconds after the one before, as at 60 frames a second, and waited for
  /// on the GPU. As many frames as the light takes for a run and the dark after it, unless told. Gives the time of
  /// each frame in milliseconds, how many layers a frame blurred on the mean, and the canvas's size in pixels of the
  /// screen; then gives the canvas back the size of what holds it.
  benchLarge(
    frames = Math.ceil((RUN.beat + RUN.dark) / 16),
    width = 2560,
    height = 1440,
  ): { meanMs: number; p95Ms: number; maxMs: number; blurs: number; canvas: [number, number] } {
    this.app.renderer.resize(width, height);
    this.step(16);
    this.app.renderer.render(this.app.stage);
    this.waitForGpu();
    const times: number[] = [];
    let blurs = 0;
    for (let k = 0; k < frames; k += 1) {
      const t0 = performance.now();
      this.step(16);
      this.app.renderer.render(this.app.stage);
      this.waitForGpu();
      times.push(performance.now() - t0);
      blurs += this.stats.blurs;
    }
    const canvas: [number, number] = [this.app.canvas.width, this.app.canvas.height];
    this.app.resize();
    // The time the bench took is no frame of the page's.
    this.last = 0;
    return { ...timesOf(times), blurs: Math.round((blurs / Math.max(1, frames)) * 100) / 100, canvas };
  }

  /// Draws `frames` frames in which nothing but the lights moves, waiting for the GPU each, after one that draws the
  /// still parts; gives the time of each frame in milliseconds, and how many of the frames drew the still parts again,
  /// which is none while nothing else moves.
  benchStill(frames: number): {
    meanMs: number;
    p95Ms: number;
    maxMs: number;
    redrawn: number;
    why: Record<string, number>;
  } {
    this.tick();
    const before = this.stats.redraws;
    const whyBefore = { ...this.stats.redrawn };
    const times: number[] = [];
    for (let k = 0; k < frames; k += 1) {
      const t0 = performance.now();
      this.tick();
      this.app.renderer.render(this.app.stage);
      this.waitForGpu();
      times.push(performance.now() - t0);
    }
    const why = Object.fromEntries(
      Object.entries(this.stats.redrawn).flatMap(([k, n]) => {
        const more = n - (whyBefore[k] ?? 0);
        return more > 0 ? [[k, more]] : [];
      }),
    );
    return { ...timesOf(times), redrawn: this.stats.redraws - before, why };
  }

  /// Draws `frames` frames of the lights alone, the cards and every other still part hidden, waiting for the GPU each,
  /// after one that draws what stands; gives the time of each frame in milliseconds, and how many of them built what
  /// stands of the lit ways again, which is none while nothing else moves.
  benchLights(frames: number): { meanMs: number; p95Ms: number; maxMs: number; rebuilt: number } {
    this.tick();
    const still = [
      this.layer,
      this.lifted,
      this.links,
      this.foldLight,
      this.dots,
      this.toneEdges.line,
      this.toneEdges.near,
      this.toneEdges.far,
      this.toneEdgesLifted.line,
      this.toneEdgesLifted.near,
      this.toneEdgesLifted.far,
    ];
    const hidden = [...still, this.marks, this.edges, this.map];
    const shown = hidden.map((c) => c.visible);
    for (const c of hidden) c.visible = false;
    const before = this.stats.wayBuilds;
    const times: number[] = [];
    for (let k = 0; k < frames; k += 1) {
      const t0 = performance.now();
      this.tick();
      this.app.renderer.render(this.app.stage);
      this.waitForGpu();
      times.push(performance.now() - t0);
    }
    hidden.forEach((c, k) => {
      c.visible = shown[k];
    });
    return { ...timesOf(times), rebuilt: this.stats.wayBuilds - before };
  }

  /// Shows the tree shown `times` times more, each time as a new read of the items gives it, a copy that draws the
  /// same; gives the time each took in milliseconds, and how many cards the last made, which is none.
  benchShow(times: number): { meanMs: number; p95Ms: number; maxMs: number; made: number } {
    const roots = this.roots;
    const took: number[] = [];
    for (let k = 0; k < times && roots !== null; k += 1) {
      const copy = structuredClone(roots);
      const t0 = performance.now();
      this.show(copy);
      took.push(performance.now() - t0);
    }
    return { ...timesOf(took), made: this.stats.made };
  }

  /// Makes the map's frame, and how large the tree is drawn in it: the whole tree fits inside its padding, in its
  /// middle.
  /// The console is told the map's size once it is first made.
  private buildMap() {
    this.map.removeChildren();
    const [wide, high] = [MAP_WIDE, MAP_HIGH];
    const whole = this.extent();
    const scale = Math.min(
      (wide - 2 * MAP_PAD) / Math.max(1, whole.width),
      (high - 2 * MAP_PAD) / Math.max(1, whole.height),
    );
    this.mapScale = scale;
    this.mapOrigin = { x: (wide - whole.width * scale) / 2, y: (high - whole.height * scale) / 2 };
    if (!this.mapTold) {
      this.mapTold = true;
      this.onMapSize?.(wide, high);
    }
    this.mapFrame
      .clear()
      .roundRect(0, 0, wide, high, MAP_CORNER)
      .fill(PALETTE.page)
      .stroke({ color: PALETTE.line, width: 1 });
    this.mapMask
      .clear()
      .rect(MAP_PAD / 2, MAP_PAD / 2, wide - MAP_PAD, high - MAP_PAD)
      .fill(0xffffff);
    this.mapBars.mask = this.mapMask;
    this.mapZoom?.destroy();
    this.mapZoom = this.styles === null ? null : new BitmapText({ text: '', style: this.styles.mono(PALETTE.muted) });
    this.map.addChild(this.mapFrame, this.mapMask, this.mapBars, this.mapZoomGround);
    if (this.mapZoom !== null) this.map.addChild(this.mapZoom);
  }

  /// Where a point of the tree, `x` pixels right of its left side and `row` rows down from its top at its full size,
  /// stands on the map.
  private onMapAt(x: number, row: number): [number, number] {
    return [this.mapOrigin.x + x * this.mapScale, this.mapOrigin.y + row * ROW * this.mapScale];
  }

  /// Draws the whole tree on the map, each item where it stands on the tree: a piece as wide as its card and as tall as
  /// its rows, a hairline apart where there is room, in the colour of how its card stands ([`mapColour`]); in the accent
  /// across it where an item is new to the owner; and where the item selected stands, the accent over every other mark.
  /// Over them, in a thin light line round a faint ground that they show through, the part of the tree the screen
  /// shows. What it drew is kept, for the tests.
  private drawMap(t: Tree) {
    const { width, height } = this.seen();
    const s = this.mapScale;
    const card = NODE_W * s;
    this.mapBars.clear();
    const pieces: MapPiece[] = [];
    const unread: number[] = [];
    t.columns.forEach((col, d) => {
      col.forEach((k, r) => {
        const [x, from] = this.onMapAt(d * PITCH, this.slot(d, r));
        const tall = this.rowsAt(d, r) * ROW * s;
        const gap = tall > 3 * MAP_GAP ? MAP_GAP : 0;
        const colour = this.mapColour(k);
        this.mapBars.rect(x, from, card, Math.max(0.5, tall - gap)).fill(colour);
        pieces.push({ id: this.ids[k], column: d, y: from, height: tall, colour });
      });
      // Where items are new to the owner, the accent across the piece.
      for (const r of this.fresh[d] ?? []) {
        unread.push(this.ids[col[r]]);
        const [x, at] = this.onMapAt(d * PITCH, this.middle(d, r));
        this.mapBars.rect(x - 1, at - 1, card + 2, 2).fill(PALETTE.accent);
      }
    });
    // Where the item selected stands, the accent across its piece, over every other mark, a little wider, so that it is
    // found again from anywhere.
    const chosen = this.selected === null ? undefined : this.real.get(this.selected);
    if (chosen !== undefined) {
      const d = t.depth[chosen];
      const [x, at] = this.onMapAt(d * PITCH, this.middle(d, t.rank[chosen]));
      this.mapBars.rect(x - 2, at - 1.5, card + 4, 3).fill(PALETTE.chosen);
    }
    // The part of the tree on the screen.
    const [fx, fy] = this.onMapAt(-this.left, this.at - height / ROW / 2);
    const frame = { x: fx, y: fy, width: width * s, height: height * s };
    this.mapBars
      .rect(frame.x, frame.y, frame.width, frame.height)
      .fill({ color: PALETTE.ink, alpha: MAP_FRAME_FILL })
      .stroke({ color: PALETTE.faint, width: 1 });
    // How large the tree is drawn, at the bottom right.
    const label = this.mapZoom;
    if (label !== null) {
      const said = `${Math.round(this.zoom * 100)}%`;
      if (label.text !== said) label.text = said;
      const inset = MAP_PAD / 2;
      label.position.set(this.mapWide - inset - label.width, this.mapHigh - inset - label.height);
      this.mapZoomGround
        .clear()
        .rect(label.x - 2, label.y - 1, label.width + 4, label.height + 2)
        .fill(PALETTE.page);
    }
    this.mapDrawn = { frame, pieces, unread };
  }

  /// Whether the point `(px, py)` of the canvas is on the map.
  private onMap(px: number, py: number): boolean {
    const [x, y] = [px - this.map.x, py - this.map.y];
    return x >= 0 && x <= this.mapWide && y >= 0 && y <= this.mapHigh;
  }

  /// Moves the screen to the point `(px, py)` of the canvas, on the map: the point of the tree there comes to the middle
  /// of the screen.
  private mapTo(px: number, py: number): void {
    const t = this.tree;
    if (t === null || t.columns.length === 0) return;
    this.mark('view');
    this.anchor = null;
    this.goal = null;
    const s = this.mapScale;
    const x = (px - this.map.x - this.mapOrigin.x) / s;
    const row = (py - this.map.y - this.mapOrigin.y) / (ROW * s);
    this.at = this.bounded(row);
    this.aim = this.at;
    this.leftGoal = null;
    this.left = this.seen().width / 2 - x;
  }

  /// The colour an item's piece of the map takes, as its card stands: the colour of its tone where the system moves
  /// it, it waits its turn or it needs the owner, its own or the most pressing under it ([`tonesOf`]); the grey of a
  /// work the owner paused; the faint grey of a done or cancelled item, as its card is dim; else the light grey of an
  /// item that does not move by itself, or a shadow.
  private mapColour(k: number): string {
    const item = this.shownItems[k];
    const tone = this.toneAt[k];
    if (tone === 'red' || tone === 'blue' || tone === 'yellow') return TONE_COLOUR()[tone];
    if (!item.shadow && item.standing !== 'open') return PALETTE.hair;
    if (!item.shadow && item.work !== null) return TONE_COLOUR().grey;
    return PALETTE.pale;
  }

  /// What the map drew at the last frame, for the tests: the part of the tree the screen shows, on the map, and each
  /// item's piece, by its column, from its top on the map and how tall, in its colour.
  drawnMap(): MapDrawn {
    return this.mapDrawn;
  }

  /// The row `row` taken as near as it may be to the row at the middle of the screen: as far as leaves `KEEP` rows of
  /// the tree on the screen at its top or its bottom.
  private bounded(row: number): number {
    const room = Math.max(0, this.seen().height / 2 / ROW - KEEP);
    return Math.max(-room, Math.min(this.tall + room, row));
  }

  /// A scroll of `dy` pixels of the screen: it moves the tree up or down, as far as leaves `KEEP` rows of it on the
  /// screen at its top or its bottom.
  scroll(dy: number): void {
    if (this.tree === null) return;
    this.anchor = null;
    this.goal = null;
    this.aim = this.bounded(this.aim + dy / (ROW * this.zoom));
  }

  /// How wide and how tall the whole tree is, in pixels at its full size.
  private extent(): { width: number; height: number } {
    const columns = this.tree?.columns.length ?? 0;
    return { width: Math.max(0, columns - 1) * PITCH + NODE_W, height: this.tall * ROW };
  }

  /// How large the whole tree is drawn to fit the screen, or `wide` pixels of its width, its room round it included,
  /// and never larger than its full size.
  private fitZoom(wide = this.app.screen.width): number {
    const { height } = this.app.screen;
    const whole = this.extent();
    return Math.min(ZOOM_MOST, (wide - 2 * FIT_ROOM) / whole.width, (height - 2 * EDGE) / Math.max(ROW, whole.height));
  }

  /// Draws the tree `zoom` times its full size, the point `(sx, sy)` of the canvas staying on the same point of the
  /// tree: no larger than its full size, and no smaller than a part of the size at which it fits the screen.
  zoomAt(zoom: number, sx: number, sy: number): void {
    if (this.tree === null) return;
    this.mark('view');
    const least = Math.max(ZOOM_LEAST, Math.min(ZOOM_MOST, this.fitZoom() * ZOOM_UNDER_FIT));
    const to = Math.max(least, Math.min(ZOOM_MOST, zoom));
    const z = this.zoom;
    const mid = this.app.screen.height / 2;
    const row = this.at + (sy - mid) / (z * ROW);
    const along = sx / z - this.left;
    this.zoom = to;
    this.anchor = null;
    this.goal = null;
    this.leftGoal = null;
    this.at = row - (sy - mid) / (to * ROW);
    this.aim = this.at;
    this.left = sx / to - along;
  }

  /// Whether the whole tree is made to fit the screen, until it goes back ([`overview`]).
  wholeShown(): boolean {
    return this.back !== null;
  }

  /// Makes the whole tree fit the screen between `from` and `to` pixels from its left, which what stands over the tree
  /// leaves, in the middle of that room, gliding there; and once it does, glides back to where it was before.
  overview(from = 0, to = this.app.screen.width): void {
    if (this.tree === null) return;
    this.mark('view');
    this.anchor = null;
    this.leftGoal = null;
    if (this.back !== null) {
      // Back to the size it was, round the pointer where it is over the tree; else back to where it was.
      const back = this.back;
      const zoom = back.zoom;
      const sp = this.screenPointer;
      this.back = null;
      if (sp === null) {
        this.goal = back;
        return;
      }
      const mid = this.app.screen.height / 2;
      const pivot = {
        sx: sp.x,
        sy: sp.y,
        row: this.at + (sp.y - mid) / (this.zoom * ROW),
        along: sp.x / this.zoom - this.left,
      };
      this.goal = { zoom, at: pivot.row - (sp.y - mid) / (zoom * ROW), left: sp.x / zoom - pivot.along, pivot };
      return;
    }
    this.back = { zoom: this.zoom, at: this.aim, left: this.left };
    const wide = Math.max(2 * FIT_ROOM + 1, to - from);
    const zoom = this.fitZoom(wide);
    this.goal = { zoom, at: this.tall / 2, left: (from + wide / 2) / zoom - this.extent().width / 2 };
  }

  /// Brings the item at `k` to the middle of the screen at the tree's full size, gliding there.
  private closeUpOn(k: number): void {
    const t = this.tree;
    if (t === null) return;
    this.mark('view');
    this.anchor = null;
    this.back = null;
    this.leftGoal = null;
    const d = t.depth[k];
    const width = this.app.screen.width / ZOOM_MOST;
    this.goal = { zoom: ZOOM_MOST, at: this.middle(d, t.rank[k]), left: width / 2 - d * PITCH - NODE_W / 2 };
  }

  private wheel = (e: WheelEvent) => {
    e.preventDefault();
    const unit = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? this.app.screen.height : 1;
    const [dx, dy] = [e.deltaX * unit, e.deltaY * unit];
    // With Shift held, or in a pinch, which comes as the wheel with Ctrl held, the tree grows or shrinks round the
    // pointer. Some systems turn the wheel with Shift held into a sideways turn, so the larger of the two counts.
    if (e.shiftKey || e.ctrlKey) {
      const turn = Math.abs(dy) >= Math.abs(dx) ? dy : dx;
      const r = this.app.canvas.getBoundingClientRect();
      const by = Math.exp(-turn * (e.ctrlKey ? ZOOM_PINCH : ZOOM_WHEEL));
      this.zoomAt(this.zoom * by, e.clientX - r.left, e.clientY - r.top);
      return;
    }
    if (dx !== 0) {
      this.mark('view');
      this.anchor = null;
      this.goal = null;
      this.leftGoal = null;
      this.left -= dx / this.zoom;
    }
    if (dy !== 0) this.scroll(dy);
  };

  private hover = (e: PointerEvent) => {
    this.mark('pointer');
    const r = this.app.canvas.getBoundingClientRect();
    this.screenPointer = { x: e.clientX - r.left, y: e.clientY - r.top };
  };

  /// The item under the pointer, by its index in the tree, or none: the card of column `d` whose box holds the
  /// pointer. `y` gives an item's middle on the screen.
  private pointed(t: Tree, x: (d: number) => number, y: (d: number, rank: number) => number): number {
    return this.beside(t, x, y, 0, NODE_W);
  }

  /// What pointing at the card of item `k` says of how its work stands, in the engine's words (`stateSaid`), where the
  /// pointer is over the card but not its level; nothing over a shadow or an item with no work.
  private stateSaid(k: number): string {
    const stands = k < 0 ? undefined : this.shownItems[k]?.stands;
    return stands == null ? '' : stateSaid(stands);
  }

  /// What pointing at the level on the card of item `k` says, where the pointer is over it; else nothing.
  private levelSaid(k: number): string {
    const p = this.pointer;
    const view = k < 0 ? undefined : this.views[k];
    const box = view === undefined ? undefined : LEVEL_SAID.get(view);
    if (p === null || view === undefined || box === undefined) return '';
    const x = (p.x - view.x) / view.scale.x;
    const y = (p.y - view.y) / view.scale.y;
    return x >= box.x && x <= box.x + box.width && y >= box.y && y <= box.y + box.height ? box.said : '';
  }

  /// The item folded whose dot is under the pointer, by its index in the tree, or none: the room past its card's right
  /// edge as far as [`FOLD_REACH`], as high as the card, where its funnel would be.
  private dotPointed(t: Tree, x: (d: number) => number, y: (d: number, rank: number) => number): number {
    const i = this.beside(t, x, y, NODE_W, NODE_W + FOLD_REACH);
    return i >= 0 && this.shownItems[i].fold?.folded === true ? i : -1;
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
    if (p === null || d < 0 || d >= t.columns.length || p.x < x(d) + from || p.x > x(d) + to) return -1;
    const rank = this.rankAt(d, (p.y - (this.frame?.top ?? 0)) / ROW);
    if (rank >= t.columns[d].length) return -1;
    const h = high(this.rowsAt(d, rank)) / 2;
    return Math.abs(p.y - y(d, rank)) <= h ? t.columns[d][rank] : -1;
  }

  /// A move of the pointer while it is held down: on the map, the screen goes where the pointer is; on the tree, the
  /// tree goes with the pointer, every way.
  private drag = (e: PointerEvent) => {
    if (this.mapDragging) {
      const r = this.app.canvas.getBoundingClientRect();
      this.mapTo(e.clientX - r.left, e.clientY - r.top);
      return;
    }
    if (this.dragging === null) return;
    this.mark('view');
    this.anchor = null;
    this.goal = null;
    this.leftGoal = null;
    this.left += (e.clientX - this.dragging.x) / this.zoom;
    this.at = this.bounded(this.at - (e.clientY - this.dragging.y) / (ROW * this.zoom));
    this.aim = this.at;
    this.dragging = { x: e.clientX, y: e.clientY };
  };

  private up = (e: PointerEvent) => {
    this.mark('pointer');
    this.dragging = null;
    this.mapDragging = false;
    const d = this.down;
    this.down = null;
    if (d === null || Math.hypot(e.clientX - d.x, e.clientY - d.y) > 4 || this.tree === null) return;
    // A click on the funnel of an item's fold folds the items under it away, on its dot shows them again, and either
    // selects nothing.
    if (this.turnFold(this.underFold)) return;
    // A click on an item selects it, and on it again, or on no item, lets it go. A click on a shadow selects the item
    // it points to and brings it to the middle of the screen, so that the columns come to where it stands.
    const i = this.under;
    // While the whole tree is made to fit the screen, a click on an item selects it and brings it to the middle of the
    // screen at its full size.
    if (i >= 0 && this.shadows[i] === 0 && this.back !== null) {
      this.selected = this.ids[i];
      this.closeUpOn(i);
      this.announce();
      return;
    }
    if (i < 0) this.selected = null;
    else if (this.shadows[i] === 1) {
      const to = this.real.get(this.ids[i]);
      // Where folds hide the item it points to, they are opened, and the item comes to the middle once it is drawn.
      if (to === undefined) {
        if (this.unfoldTo(this.ids[i])) this.toFocus = this.ids[i];
        else this.onAway?.(this.ids[i]);
        return;
      }
      this.selected = this.ids[to];
      this.focus(this.tree.depth[to], this.tree.rank[to]);
    } else this.selected = this.selected === this.ids[i] ? null : this.ids[i];
    this.announce();
  };

  /// Brings the item numbered `id` into view, within the `room` pixels of the screen's width from its left that the
  /// window beside the tree leaves: to the middle of the screen if it is above or below what is seen, and, if its
  /// column is not wholly within the room, the tree moved sideways only until it is, a little in from the side it was
  /// out at. An item already in view stays where it is. While the tree is going somewhere as a whole, the item is
  /// brought into view where the tree is going.
  reveal(id: number, room: number): void {
    this.mark('view');
    const t = this.tree;
    const i = this.real.get(id);
    if (t === null || i === undefined) return;
    const d = t.depth[i];
    const rank = t.rank[i];
    const g = this.goal;
    const view = g ?? { zoom: this.zoom, at: this.at, left: this.left };
    const height = this.app.screen.height / view.zoom;
    const wide = room / view.zoom;
    const y = height / 2 + (this.middle(d, rank) - view.at) * ROW;
    const h = high(this.rowsAt(d, rank)) / 2;
    const x = view.left + d * PITCH;
    const toAt = y - h < EDGE || y + h > height - EDGE ? this.middle(d, rank) : null;
    const toLeft = x < 0 ? INSET - d * PITCH : x + NODE_W > wide ? wide - INSET - NODE_W - d * PITCH : null;
    if (g !== null) {
      if (toAt !== null) g.at = toAt;
      if (toLeft !== null) g.left = toLeft;
      return;
    }
    if (toAt !== null) this.focus(d, rank);
    if (toLeft !== null) this.leftGoal = toLeft;
  }

  /// Flashes the light along the edges of the item numbered `id` once, from the next frame: it grows stronger and eases
  /// back to rest ([`FLASH_MS`]). The flash is the item selected's, so it ends once `id` is not the item selected; and
  /// a flash asked for again begins again.
  flash(id: number): void {
    this.mark('flash');
    this.flashing = { id, at: this.clock };
  }

  /// Folds the items under the card the pointer is over away, or shows them again, as a click on its funnel or its dot
  /// does, the console told; a shadow, or a card with nothing under it, folds nothing. Whether it folded or unfolded.
  foldPointedCard(): boolean {
    return this.turnFold(this.under);
  }

  /// Tells the console to fold the items under the item at `k` away, or show them again, where it has any; the item
  /// stays where it is on the screen while the tree changes round it. Whether it had any.
  private turnFold(k: number): boolean {
    const fold = k < 0 ? null : this.shownItems[k].fold;
    const t = this.tree;
    if (fold == null || t === null) return false;
    this.anchor = { id: this.ids[k], off: this.middle(t.depth[k], t.rank[k]) - this.at };
    this.onFold?.(this.ids[k], !fold.folded);
    return true;
  }

  /// The item selected, by its number, or none: from the moment it is chosen, while the folds over it still open.
  selectedId(): number | null {
    return this.selected;
  }

  /// Asks for the still parts of the tree to be drawn again at the next frame, for `why`.
  private mark(why: string) {
    this.dirty.add(why);
  }

  /// Lights the items numbered `ids`, which the console's search finds while it is open, and dims every other, as
  /// the items in focus do; none, while the search is closed, lights and dims nothing.
  find(ids: number[] | null): void {
    this.mark('find');
    this.found = ids;
  }

  /// Selects the item numbered `id`, or none.
  select(id: number | null): void {
    this.mark('select');
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
    const step = this.advance(now);
    if (this.tree !== null) this.draw(this.tree, now, step);
    const sorted = [...this.frames].sort((a, b) => a - b);
    const mean = sorted.reduce((a, b) => a + b, 0) / Math.max(1, sorted.length);
    this.stats = {
      ...this.stats,
      frames: this.stats.frames + 1,
      fps: Math.round(this.app.ticker.FPS),
      meanMs: Math.round(mean * 10) / 10,
      p95Ms: Math.round((sorted[Math.floor(sorted.length * 0.95)] ?? 0) * 10) / 10,
    };
  };

  /// Draws the frame at `now`, `step` milliseconds after the one before ([`advance`]).
  private draw(t: Tree, now: number, step: number) {
    // The tree is drawn at its full size, the whole of it made larger or smaller as it is zoomed: what is worked out
    // here is in pixels at its full size, of which the screen shows `width` by `height`.
    // The items take the sizes and places the folds turning give them now, the item held staying where it is on the
    // screen. Where the tree is going as a whole, it goes a part of the way; else the row at the middle of the screen
    // goes a part of the way to where the wheel took it.
    if (this.turns.size > 0) {
      this.shape();
      this.hold();
    }
    const g = this.goal;
    if (g !== null) {
      this.zoom += (g.zoom - this.zoom) * SNAP;
      // Round a pivot, the point of the tree under it stays under it at every size on the way.
      if (g.pivot === undefined) {
        this.at += (g.at - this.at) * SNAP;
        this.left += (g.left - this.left) * SNAP;
      } else {
        const p = g.pivot;
        this.at = p.row - (p.sy - this.app.screen.height / 2) / (this.zoom * ROW);
        this.left = p.sx / this.zoom - p.along;
      }
      const there =
        Math.abs(g.zoom - this.zoom) < 1e-3 && Math.abs(g.at - this.at) * ROW < 0.5 && Math.abs(g.left - this.left) < 0.5;
      if (there) {
        [this.zoom, this.at, this.left] = [g.zoom, g.at, g.left];
        this.goal = null;
      }
      this.aim = this.at;
    } else {
      this.at += (this.aim - this.at) * GLIDE;
      if (Math.abs(this.aim - this.at) * ROW < REST) this.at = this.aim;
    }
    this.world.scale.set(this.zoom);
    const { width, height } = this.seen();
    const line = height / 2;
    // Moved sideways no further than leaves half of the first column on the screen at its right, or half of the last at
    // its left, so that one always sees where the tree ends.
    const last = Math.max(0, t.columns.length - 1) * PITCH;
    const bound = (v: number) => Math.min(width - NODE_W / 2, Math.max(-last - NODE_W / 2, v));
    if (this.leftGoal !== null) {
      const goal = bound(this.leftGoal);
      this.left += (goal - this.left) * SNAP;
      if (Math.abs(goal - this.left) < 0.5) this.leftGoal = null;
    }
    if (g === null) this.left = bound(this.left);
    // Moved since the last frame: by the glide of this frame, or by a pan, a scroll or a zoom between the two; and the
    // screen of another size.
    const view = this.drawnView;
    const shifted = this.left !== view.left || this.at !== view.at || this.zoom !== view.zoom;
    const screen = this.app.screen;
    const sized = screen.width !== this.drawnSize[0] || screen.height !== this.drawnSize[1];
    // Where the ways stand anew: a new tree, the theme, a fold turning, the screen's size, the numbers the ways are drawn
    // by changed by a knob, or the screen moved past the room the tree was last drawn for ([`Frame`]). Their curves are
    // worked out again, and what stands of them built again, in a frame taken from where the screen is now. Else the
    // tree stands as it was drawn, and the whole of it is moved as far as the screen moved.
    const anew = ['start', 'tree', 'shown'].some((w) => this.dirty.has(w));
    const numbers = Object.values(RUN).join();
    const tuned = numbers !== this.runNumbers;
    this.runNumbers = numbers;
    const top = line - this.at * ROW;
    const kept = this.frame;
    const placed =
      kept === null || anew || this.turns.size > 0 || sized || tuned || !holds(kept, this.left, top, width, height);
    const frame = kept === null || placed ? frameAt(this.left, top, width, height) : kept;
    this.frame = frame;
    // The point the items near the pointer are drawn larger round, kept in the frame, stays on the same point of the
    // tree in the new one.
    if (kept !== null && frame !== kept) {
      this.lensAt = { x: this.lensAt.x + frame.left - kept.left, y: this.lensAt.y + frame.top - kept.top };
    }
    const [dx, dy] = [this.left - frame.left, top - frame.top];
    this.world.position.set(dx * this.zoom, dy * this.zoom);
    const room = frame.room;
    const x = (d: number) => frame.left + d * PITCH;
    const y = (d: number, rank: number) => frame.top + this.middle(d, rank) * ROW;
    // Where the pointer is on the tree, in the frame it is drawn in, and the column it is over.
    const sp = this.screenPointer;
    this.pointer = sp === null ? null : { x: sp.x / this.zoom - dx, y: sp.y / this.zoom - dy };
    const over = this.pointer === null ? -1 : Math.floor((this.pointer.x - frame.left + (PITCH - NODE_W) / 2) / PITCH);
    this.hovered = over >= 0 && over < t.columns.length ? over : -1;
    // Whether anything the still parts draw is still easing at this frame, which draws them again at the next.
    let easing = false;
    // The items near the pointer are drawn larger while the tree is drawn smaller than its full size and the pointer is
    // on the tree, not on the map, and the tree is not being moved or going anywhere; round the pointer as it moves,
    // coming in and going out smoothly.
    const lensWanted =
      this.gridBack === null &&
      this.pointer !== null &&
      this.zoom < LENS_UNDER &&
      this.dragging === null &&
      !this.mapDragging &&
      g === null &&
      !(sp !== null && this.map.visible && this.onMap(sp.x, sp.y));
    if (lensWanted && this.pointer !== null) this.lensAt = { ...this.pointer };
    const lensTo = lensWanted ? 1 : 0;
    this.lens += (lensTo - this.lens) * (1 - Math.exp(-step / LENS_EASE));
    if (Math.abs(lensTo - this.lens) < SETTLED) this.lens = lensTo;
    else easing = true;
    const lensing = this.lens > 0 && this.zoom < 1;
    const drawnLens = this.lensDrawn;
    const lensMoved =
      lensing && (this.lens !== drawnLens.lens || this.lensAt.x !== drawnLens.x || this.lensAt.y !== drawnLens.y);
    this.lensDrawn = { lens: this.lens, x: this.lensAt.x, y: this.lensAt.y };
    // The marks of cells running: lit, then in the last part of each beat faded out and back in, every running node
    // at once.
    const beat = (now % this.beat.ms) / this.beat.ms;
    const fade = beat < 1 - FADE ? 0 : Math.sin((Math.PI * (beat - (1 - FADE))) / FADE) ** 2;
    const pulse = 1 - (1 - this.beat.faint) * fade;
    // The still parts of the tree, the cards where they stand, the funnels, the rings and the ribbons, the light round
    // the cards whose own work moves, the dots, the edges and the map, are drawn again only where something they draw changed: a new tree, the
    // pointer, a selection, a flash, the tree moving or zoomed, a fold turning, the screen's size, or what was still
    // easing at the last frame. Else they stand as they were drawn, and only the lights, the pulses and the ages are
    // drawn.
    const why = [...this.dirty];
    if (this.turns.size > 0) why.push('turns');
    if (shifted) why.push('moved');
    if (sized) why.push('size');
    if (this.easing) why.push('easing');
    if (lensMoved) why.push('lens');
    const redraw = why.length > 0;
    for (const w of why) this.stats.redrawn[w] = (this.stats.redrawn[w] ?? 0) + 1;
    // While a fold turns, what is under it is drawn in towards its dot, or grows back out of it: where a point of an
    // item `k` is drawn, and how large the item is.
    const m = this.turns.size === 0 ? null : this.movedNow(t, x, y);
    this.drawnMoved = m;
    const at = (k: number, px: number, py: number): [number, number] =>
      m === null ? [px, py] : [m.a[k] * px + m.bx[k], m.a[k] * py + m.by[k]];
    const size = (k: number) => (m === null ? 1 : m.a[k]);
    // The cards near the pointer drawn larger, and they alone: the funnels, the ways and the dots stand where they do.
    // Where a point of the card of item `k` is drawn, and how large the card is.
    const lens = lensing ? this.magnified(t, x, y, m, this.lens) : null;
    const cardAt = (k: number, px: number, py: number): [number, number] =>
      lens === null ? at(k, px, py) : [lens.a[k] * px + lens.bx[k], lens.a[k] * py + lens.by[k]];
    const cardSize = (k: number) => (lens === null ? size(k) : lens.a[k]);
    const liftedAt = (k: number) => lensing && cardSize(k) > 1 + SETTLED;
    // Where item `k` stands and how large, as the funnels and the ways meet it; and where its card is drawn and how
    // large: each its left side, its middle, its width and half its height.
    const box = (k: number) => {
      const d = t.depth[k];
      const [bx, by] = at(k, x(d), y(d, t.rank[k]));
      return { x: bx, y: by, w: NODE_W * size(k), h: (high(this.base[k]) / 2) * size(k) };
    };
    const cardBox = (k: number) => {
      const d = t.depth[k];
      const [bx, by] = cardAt(k, x(d), y(d, t.rank[k]));
      return { x: bx, y: by, w: NODE_W * cardSize(k), h: (high(this.base[k]) / 2) * cardSize(k) };
    };
    if (redraw) {
      this.drawnSize = [screen.width, screen.height];
      this.stats.redraws += 1;
      this.links.clear();
      this.marks.clear();
      const nowShown = new Set<Container>();
      const drawn: number[] = [];
      const lensGrounded = new Set<number>();
      const rows = Math.ceil((room.y1 - room.y0) / ROW / 2) + 2;
      const middle = ((room.y0 + room.y1) / 2 - frame.top) / ROW;
      // The funnel under the pointer, as it is drawn: every funnel folds, one over shadows alone as well.
      const pointer = this.pointer;
      let funnelled: { k: number; shape: [number, number, number, number, number, number] } | null = null;
      for (let d = 0; d < t.columns.length; d += 1) {
        const cx = x(d);
        if (cx > room.x1 || cx + NODE_W < room.x0) continue;
        const col = t.columns[d];
        const centre = this.rankAt(d, middle);
        for (let rank = Math.max(0, centre - rows); rank < Math.min(col.length, centre + rows); rank += 1) {
          const yy = y(d, rank);
          if (yy < room.y0 - 2 * ROW || yy > room.y1 + 2 * ROW) continue;
          const item = col[rank];
          const view = this.views[item];
          view.position.set(...cardAt(item, cx, yy));
          view.scale.set(cardSize(item));
          // The larger a card is drawn, the more it stands over the others, on the page's ground; drawn larger, it is
          // lifted over the lights that lie over the other cards.
          view.zIndex = lensing ? cardSize(item) : 0;
          const home = liftedAt(item) ? this.lifted : this.layer;
          if (liftedAt(item)) {
            let ground = this.lensGrounds.get(item);
            if (ground === undefined) {
              ground = new Graphics();
              this.lensGrounds.set(item, ground);
              this.lifted.addChild(ground);
            }
            const b = cardBox(item);
            ground.clear().roundRect(b.x, b.y - b.h, b.w, 2 * b.h, CARD_CORNER).fill(PALETTE.page);
            ground.zIndex = view.zIndex - SETTLED;
            lensGrounded.add(item);
          }
          nowShown.add(view);
          drawn.push(item);
          if (view.parent !== home) home.addChild(view);
        }
        if (d === 0) continue;
        // The funnel from each item of the column to the left to the items under it: from its top to the top of the
        // first and from its bottom to the bottom of the last, filled, each end where its card is drawn. While the
        // item's fold turns, the whole funnel is drawn in towards its dot with the items under it, its end at the card
        // too.
        const px = x(d - 1) + NODE_W;
        const prev = t.columns[d - 1];
        for (let r = 0; r < prev.length; r += 1) {
          const i = prev[r];
          const kids = t.children[i];
          if (kids.length === 0) continue;
          const [one, end] = [kids[0], kids[kids.length - 1]];
          const py = y(d - 1, r);
          const ph = high(this.base[i]) / 2;
          const from = this.turns.has(this.ids[i]) ? one : i;
          const [fx, top] = at(from, px, py - ph);
          const [, bottom] = at(from, px, py + ph);
          const [gx, first] = at(one, cx, y(d, t.rank[one]) - high(this.base[one]) / 2);
          const [, last] = at(end, cx, y(d, t.rank[end]) + high(this.base[end]) / 2);
          if (Math.max(bottom, last) < room.y0 || Math.min(top, first) > room.y1) continue;
          this.funnel(fx, top, bottom, gx, first, last);
          const on = pointer !== null && inFunnel(fx, top, bottom, gx, first, last, pointer.x, pointer.y);
          if (on && this.shownItems[i].fold != null) funnelled = { k: i, shape: [fx, top, bottom, gx, first, last] };
        }
      }
      this.links.fill({ color: PALETTE.other, alpha: PALETTE.otherAlpha * linksStrength(this.dim) });
      for (const [k, ground] of this.lensGrounds) {
        if (lensGrounded.has(k)) continue;
        ground.destroy();
        this.lensGrounds.delete(k);
      }
      // What is under the pointer: a card first; else the dot of an item folded, past its card; else a funnel.
      // While the items near the pointer are drawn larger, by where each card and its dot are drawn.
      let dot: number;
      if (lensing && pointer !== null) {
        const inside = (k: number) => {
          const b = cardBox(k);
          return pointer.x >= b.x && pointer.x <= b.x + b.w && Math.abs(pointer.y - b.y) <= b.h;
        };
        const onDot = (k: number) => {
          if (this.shownItems[k].fold?.folded !== true) return false;
          const b = box(k);
          const edge = b.x + b.w;
          return pointer.x > edge && pointer.x <= edge + FOLD_REACH * size(k) && Math.abs(pointer.y - b.y) <= b.h;
        };
        // Of the cards that hold the pointer, the one drawn over the others: the largest.
        const top = (holds: (k: number) => boolean) =>
          drawn.reduce((best, k) => (holds(k) && (best < 0 || cardSize(k) > cardSize(best)) ? k : best), -1);
        this.under = top(inside);
        dot = this.under >= 0 ? -1 : top(onDot);
      } else {
        this.under = this.pointed(t, x, y);
        dot = this.under >= 0 ? -1 : this.dotPointed(t, x, y);
      }
      this.underFold = this.under >= 0 ? -1 : dot >= 0 ? dot : (funnelled?.k ?? -1);
      // The fold under the pointer: its dot set in a disc, or its funnel filled, in the pointer's tint under the lit
      // ways; the pointer a hand; and what a click does and what stands under the item said on pointing.
      this.foldLight.clear();
      const fold = this.underFold < 0 ? null : this.shownItems[this.underFold].fold;
      if (fold != null && dot >= 0) {
        const d = t.depth[dot];
        const [dx, dy] = at(dot, x(d) + NODE_W + DOT_AT, y(d, t.rank[dot]));
        this.foldLight.circle(dx, dy, (DOT_R + FOLD_PAD) * size(dot)).fill(PALETTE.hoverRibbon);
      } else if (fold != null && funnelled !== null) {
        this.funnel(...funnelled.shape, this.foldLight);
        this.foldLight.fill(PALETTE.hoverRibbon);
      }
      // What a click on the fold does, else what the level under the pointer is: a level cannot be pressed, so only a
      // fold's words turn the pointer to a hand.
      const foldWordsSaid = fold == null ? '' : foldWords(fold);
      const said = foldWordsSaid !== '' ? foldWordsSaid : this.levelSaid(this.under) || this.stateSaid(this.under);
      if (said !== this.foldSaid) {
        this.foldSaid = said;
        this.app.canvas.title = said;
        this.app.canvas.style.cursor = foldWordsSaid === '' ? '' : 'pointer';
      }
      this.drawnList = drawn;
      this.nowShown = nowShown;
    }
    const drawn = this.drawnList;
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
    const chain = lit(this.under);
    // The item selected, the same way, in the accent; only the item itself glows, those lit with it are only ringed,
    // so that it stands out among them.
    const picked = this.selected === null ? -1 : (this.real.get(this.selected) ?? -1);
    const chosen = lit(picked);
    // How far on the flash of the item selected's light is. It ends once its time is over or its item is no longer
    // the one selected; while it lasts, the still parts are drawn again at the next frame.
    let flare = 0;
    if (this.flashing !== null) {
      const ms = this.clock - this.flashing.at;
      if (ms >= FLASH_MS || this.flashing.id !== this.selected) this.flashing = null;
      else {
        flare = flashOf(ms);
        easing = true;
      }
    }
    // While the pointer, the selection or the search lights something, every other item drawn is dimmed, more under
    // the pointer than for the selection alone, so that the lit items and what joins them stand out. The item under
    // the pointer and what joins it are lit at once; everything else goes to its brightness over a moment, down as
    // the light leaves it and back up once nothing is lit, so that the tree fades rather than jumps.
    this.inFocus = this.under >= 0 ? chain : new Set();
    const focus = this.inFocus;
    // While the search is open, every item is dimmed but those it finds, lit as the items in focus are.
    const searching = this.found !== null;
    const finding = new Set(
      (this.found ?? []).flatMap((id) => (this.real.has(id) ? [this.real.get(id) as number] : [])),
    );
    const aim = focus.size > 0 || chosen.size > 0 || searching ? DIM : 1;
    this.dim = eased(this.dim, aim, step);
    if (this.dim !== aim) easing = true;
    const inLight = (i: number) => focus.has(i) || chosen.has(i) || finding.has(i);
    for (const i of drawn) {
      const view = this.views[i];
      const to = aim === 1 || inLight(i) ? 1 : DIM;
      view.alpha = inLight(i) ? dimmed(view.alpha, to, step) : eased(view.alpha, to, step);
      if (view.alpha !== to) easing = true;
    }
    // The way from each item whose work runs up to its root, lit in the colour of a running card's edge, with one
    // length of light running along it from the root's side to the item's: each link from an item to the one above it
    // drawn once, however many running items it is on. While something is lit, the links on its way stand at full
    // strength and the others dim with the items they join; each eases to its strength as the items do, so that none
    // jumps as the light moves or goes out.
    const shining = new Set([...focus, ...chosen, ...finding]);
    // A way's strength eases as its items' do, and what stands of it is drawn as strong by the alpha of its strokes,
    // and not built again. A root's dot is as strong as the ways from it are lit; another's as the way into its item.
    for (const [i, to] of wayAims(this.runWays, t.parent, shining, aim !== 1)) {
      const had = this.runStrength.get(i);
      const lights = shining.has(i) && shining.has(t.parent[i]);
      this.runStrength.set(i, lights ? dimmed(had ?? to, to, step) : eased(had ?? to, to, step));
    }
    const dotStrength = (k: number) =>
      t.parent[k] >= 0
        ? (this.runStrength.get(k) ?? 1)
        : aim === 1 || shining.has(k)
          ? 1
          : linksStrength(this.dim);
    const restAgain = placed;
    const stroked = (to: Strokes) => [to.line, to.near, to.far];
    if (restAgain) {
      for (const part of this.ways.parts.values()) for (const g of stroked(part.rest)) g.clear();
      for (const part of this.ends.parts.values()) for (const g of stroked(part.rest)) g.clear();
    }
    for (const part of this.ways.parts.values()) {
      if (part.lit) for (const g of stroked(part.light)) g.clear();
      part.lit = false;
    }
    for (const g of stroked(this.runEdges)) g.clear();
    // Each glow is blurred as far as it reaches at the tree's full size, and made smaller with the tree: a blur is laid
    // in the screen's pixels, and not in the tree's, so that it is made as much smaller as the tree is drawn.
    const canvas = this.app.renderer.resolution;
    const z = this.zoom;
    for (const lit of [this.ways, this.runEdges, this.ends, this.edgeLight, this.toneEdges, this.toneEdgesLifted]) {
      blurTo(lit.nearBlur, RUN.nearBlur * z, canvas);
      blurTo(lit.farBlur, RUN.farBlur * z, canvas);
    }
    const lineWide = this.lineWidth;
    const lineAlpha = this.adds ? RUN.lineAlpha : RUN.inkAlpha;
    const glintWide = this.glint;
    // The colours of a way of each tone ([`Paint`]): its glow and strands where the light is `bright`, from the way at
    // rest to the light's head, the tone's colour, a blue way's going towards the light's own as it brightens
    // ([`RUN.hue`]), the colour whole, with none of the card in it, so that what is added to a dark ground is the colour
    // itself and not a wash of the card, which reads grey; its line, the light's near white where the page is dark,
    // where the line is itself a light, and the tone's colour where it is light, where the line is what the glint reads
    // against; a grey way's line alone, a faint grey. Taken once for each tone at each frame.
    const paints = new Map<Tone, Paint>();
    const paintOf = (tone: Tone): Paint => {
      const had = paints.get(tone);
      if (had !== undefined) return had;
      const base = TONE_COLOUR()[tone];
      const run = mix(PALETTE.card, base, EDGE_RUNNING);
      const glint = mix(between(PALETTE.card, base, EDGE_RUNNING), PALETTE.glowTo, RUN.core);
      const shades = Array.from({ length: RUN.lightPieces + 1 }, (_, k) =>
        mix(base, PALETTE.pulse, tone === 'blue' ? (RUN.hue * k) / RUN.lightPieces : 0),
      );
      const made: Paint =
        tone === 'grey'
          ? { line: mix(PALETTE.card, base, EDGE_TINT), glint, tint: () => shades[0], runs: false, glows: false }
          : {
              line: this.adds ? glint : run,
              glint,
              tint: (bright) => shades[Math.round(Math.min(1, Math.max(0, bright)) * RUN.lightPieces)],
              runs: tone === 'blue',
              glows: true,
            };
      paints.set(tone, made);
      return made;
    };
    // Each way's curve, how far it goes across the page, and how far across from the left side of the roots' cards
    // it begins ([`RUN.light`]).
    const ways = placed ? new Map<number, { way: Way; len: number; dist: number }>() : this.wayCache;
    this.wayCache = ways;
    // Where the way into item `i` begins, at the middle of its parent's right edge, and ends, at the middle of its own
    // left edge, each where its card is drawn: so that under a turning fold the way grows out of the dot or draws in
    // to it with the cards, from the folded card to the running one as it is drawn at that frame.
    const ends = (i: number) => {
      const p = t.parent[i];
      const d = t.depth[i];
      const [px, py] = at(p, x(d - 1) + NODE_W, y(d - 1, t.rank[p]));
      const [cx, iy] = at(i, x(d), y(d, t.rank[i]));
      return { px, py, cx, iy };
    };
    const wayDown = (i: number): { way: Way; len: number; dist: number } => {
      const had = ways.get(i);
      if (had !== undefined) return had;
      const { px, py, cx, iy } = ends(i);
      const found = { way: this.wayOf(px, py, cx, iy), len: cx - px, dist: px - x(0) };
      ways.set(i, found);
      return found;
    };
    // How far the furthest item whose work runs stands from its root, its own card counted, and where the light has
    // run to now: every root's light runs from its own card down every way under it, all of them in step and
    // beginning again together once the furthest of all has been reached.
    let far = 0;
    for (const i of this.runStrength.keys()) {
      const { len, dist } = wayDown(i);
      far = Math.max(far, dist + len + NODE_W);
    }
    // A way that ends at the dot of a fold, past its item's card, runs as far as the dot.
    const dotFrom = (k: number) => x(t.depth[k]) - x(0) + NODE_W + DOT_AT;
    for (const k of this.litDots) far = Math.max(far, dotFrom(k) + DOT_R);
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
    this.waysDrawn = [];
    // The ways drawn, in the order they are drawn in; and the lights where the ways meet the cards and at the dots of
    // folds, each with the place across the page the light comes to it at, how it burns now, the part it stands in
    // at rest and how strong that is, and how it is laid on what it is drawn on as it burns, `k` as strong as it is
    // ([`sparkAt`]). They are drawn once the ways are. The part of a way's end at its parent is the way's number
    // twice, at its item one more; a fold's dot's is under nought.
    const wayOrder: number[] = [];
    const meets: {
      place: number;
      burn: number;
      key: number;
      strength: number;
      lay: (to: Strokes, burn: number, k: number) => void;
    }[] = [];
    for (const [i, strength] of this.runStrength) {
      const p = t.parent[i];
      const { px, py, cx, iy } = ends(i);
      if (cx < room.x0 || px > room.x1) continue;
      if (Math.max(py, iy) < room.y0 || Math.min(py, iy) > room.y1) continue;
      const { way, len, dist } = wayDown(i);
      const paint = paintOf(this.toneAt[i]);
      const { tint, line: lineColour, glint: glintColour } = paint;
      const turns = [turnOf(way, true, RUN.fillet), turnOf(way, false, RUN.fillet)];
      this.waysDrawn.push({ id: this.ids[i], from: [px, py], to: [cx, iy], curve: way.points, turns });
      // How long the way's curve is, and how far along it the place stands that is `across` the page from its start.
      const curve = way.along[RUN.pieces];
      const on = (across: number) => this.arcAt(way, across);
      // A length of the way drawn `k` as strong as the way is: at rest wholly, the strokes' alpha making it as strong
      // as the way is; the light at its strength.
      const lit = (
        g: Graphics,
        from: number,
        to: number,
        w: number,
        alpha: number,
        paint: number,
        end = false,
        k = 1,
      ) => {
        this.alongWay(g, way, from, to);
        g.stroke({ color: paint, width: w, alpha: k * alpha, cap: end ? 'round' : 'butt' });
      };
      const lights = this.wayLights(i);
      wayOrder.push(i);
      lights.place = Math.round(dist);
      const [still, shine] = [lights.rest, lights.light];
      for (const g of stroked(still)) g.alpha = strength;
      // The way itself, as a wire of light: its glow further out and close in, then the line, drawn in lengths each
      // at the width of its own middle so that it is thinner where it meets the cards. The glow is fainter where the
      // page is light ([`RUN.inkRest`]). Built only where the ways stand anew, and kept.
      if (restAgain) {
        if (paint.glows) {
          lit(still.far, 0, curve, RUN.restFar, RUN.farAlpha * RUN.restGlow * rest, tint(0), true);
          lit(still.near, 0, curve, RUN.near, RUN.nearAlpha * RUN.restGlow * rest, tint(0), true);
        }
        for (let n = 0; n < RUN.wide; n += 1) {
          const thick = lineWide * thickAt((n + 0.5) / RUN.wide);
          lit(still.line, (curve * n) / RUN.wide, (curve * (n + 1)) / RUN.wide, thick, lineAlpha, lineColour);
        }
      }
      // The light running down the way: the same line, brighter and wider where the light is, as a wire is where the
      // current is in it. It is a drop drawn out along the way, its head round and widest, thinning away behind it;
      // the head is round only where it stands on this way, and not where the way cuts the light short.
      const drop = paint.runs ? lightOn(dist, len, head) : [];
      if (drop.length > 0) lights.lit = true;
      drop.forEach(({ from, to, bright }, n) => {
        const nose = n === drop.length - 1 && to < len - NEAR;
        const thick = RUN.lightLine * (RUN.ends + (1 - RUN.ends) * bright) * glintWide;
        const [a, b] = [on(from), on(to)];
        lit(shine.far, a, b, RUN.lightFar * bright, bright * RUN.lightFarAlpha * rest, tint(bright), nose, strength);
        lit(shine.near, a, b, RUN.lightNear * bright, bright * RUN.lightNearAlpha * rest, tint(bright), nose, strength);
        lit(shine.line, a, b, thick, bright * RUN.lightAlpha, glintColour, nose, strength);
      });
      // The strands the line is made of: each gathered close round the line along the whole way, faint, and parted
      // from it where the light is, as strong as the light is there and in its colour there; parted, each runs beside
      // the line at its own distance, and both gather into it at the way's ends ([`gatherAt`]).
      for (let s = 0; s < RUN.strands; s += 1) {
        const { side, own } = strandOf(s);
        const strand = (g: Graphics, from: number, to: number, by: number, bright: number, colour: number, k = 1) => {
          this.alongWay(g, way, from, to, (at) => side * by * gatherAt(at / curve));
          const alpha = k * RUN.strandAlpha * own * bright * rest;
          g.stroke({ color: colour, width: RUN.strand, alpha, cap: 'butt' });
        };
        if (restAgain && paint.glows) strand(still.line, 0, curve, RUN.spreadRest, RUN.strandRest, tint(0));
        for (const { from, to, bright } of drop) {
          const comes = Math.min(1, (1 - bright + 1 / RUN.lightPieces) / Math.max(RUN.strandIn, 1e-6));
          strand(shine.line, on(from), on(to), RUN.spread, bright * comes, tint(bright), strength);
        }
      }
      // The spark at both ends of the way, which burns up as the light's head goes through it; and the edge of the
      // card the way ends at, lit from its left side to its right as the light crosses it. Where the way comes from
      // a root, the root's own card is crossed before any way.
      // The light where the way meets each card: on the parent's right side, going into it, and on the item's left.
      // The way turns into each along its own curve; each card's own end is lit once, however many running ways meet
      // it there.
      // A card drawn smaller under a turning fold has its ends lit at its size, and its edge crossed by no light
      // until it is drawn whole.
      // Only a way the light runs down burns where it meets a card; one of a work the owner paused meets none.
      if (!paint.glows) continue;
      const [out, into] = paint.runs ? [sparkAt(dist, head), sparkAt(dist + len, head)] : [0, 0];
      const [outKey, inKey] = [2 * i, 2 * i + 1];
      meets.push(
        {
          place: dist,
          burn: out,
          key: outKey,
          strength,
          lay: (to, burn, k) => this.turnIn(to, px, py, turns[0], tint, lineColour, k, burn),
        },
        {
          place: dist + len,
          burn: into,
          key: inKey,
          strength,
          lay: (to, burn, k) => this.turnIn(to, cx, iy, turns[1], tint, lineColour, k, burn),
        },
      );
      if (!sparked.has(p * 2)) {
        sparked.add(p * 2);
        const ph = (high(this.base[p]) / 2) * size(p);
        meets.push({
          place: dist,
          burn: out,
          key: outKey,
          strength,
          lay: (to, burn, k) => this.spark(to, px, py, ph, tint, lineColour, glintColour, k, burn),
        });
      }
      if (!sparked.has(i * 2 + 1)) {
        sparked.add(i * 2 + 1);
        const ih = (high(this.base[i]) / 2) * size(i);
        meets.push({
          place: dist + len,
          burn: into,
          key: inKey,
          strength,
          lay: (to, burn, k) => this.spark(to, cx, iy, ih, tint, lineColour, glintColour, k, burn),
        });
      }
      if (paint.runs && t.parent[p] < 0 && !crossed.has(p)) {
        crossed.add(p);
        const over = lightOn(0, NODE_W, head);
        const b = cardBox(p);
        this.across(b.x, b.y, b.h, over, tint, lineColour, glintColour, strength, cardSize(p));
      }
      if (paint.runs && !crossed.has(i) && this.scaleOf[i] === 1) {
        crossed.add(i);
        const over = lightOn(dist + len, NODE_W, head);
        const b = cardBox(i);
        this.across(b.x, b.y, b.h, over, tint, lineColour, glintColour, strength, cardSize(i));
      }
    }
    // The dot of each fold with a running item folded in it is the end of the way: the way's line from the card's edge
    // to it, and the dot in the way's colour with its glow close in and further out, burning brighter as the light
    // comes to it; a root's card crossed by the light first, as no way down crosses it. While the fold turns, the dot
    // comes and goes with it. The ways above it are dimmed and lit as the way into its card is.
    const dotted = new Set<number>();
    for (const k of this.litDots) {
      const ness = this.dotness(k);
      const d = t.depth[k];
      const ky = y(d, t.rank[k]);
      const beyond = x(d) > room.x1 || x(d) + NODE_W + DOT_AT + DOT_R < room.x0;
      if (ness <= 0 || beyond || ky < room.y0 - ROW || ky > room.y1 + ROW) continue;
      dotted.add(k);
      const root = t.parent[k] < 0;
      const strength = dotStrength(k);
      // In the tone of the folded item, which takes the most pressing tone of what is folded in it.
      const paint = paintOf(this.toneAt[k]);
      const { tint, line: lineColour, glint: glintColour } = paint;
      if (paint.runs && root && !crossed.has(k)) {
        crossed.add(k);
        const b = cardBox(k);
        this.across(b.x, b.y, b.h, lightOn(0, NODE_W, head), tint, lineColour, glintColour, strength, cardSize(k));
      }
      const [ex, ey] = at(k, x(d) + NODE_W, ky);
      const [ox, oy] = at(k, x(d) + NODE_W + DOT_AT, ky);
      const r = DOT_R * size(k) * ness;
      const lay = (to: Strokes, burn: number, n: number) => {
        const glow = n * rest * burn;
        to.line
          .moveTo(ex, ey)
          .lineTo(ox, oy)
          .stroke({ color: lineColour, width: lineWide, alpha: n * ness * lineAlpha });
        to.far.circle(ox, oy, r * 3).fill({ color: tint(burn), alpha: glow * RUN.farAlpha });
        to.near.circle(ox, oy, r * 1.8).fill({ color: tint(burn), alpha: glow * RUN.nearAlpha });
        to.line.circle(ox, oy, r).fill({ color: lineColour, alpha: n * lineAlpha });
      };
      const place = dotFrom(k);
      meets.push({ place, burn: paint.runs ? sparkAt(place, head) : 0, key: -1 - k, strength, lay });
    }
    // Each place the light comes to stands at rest until the light is near it, and burns while it is: at rest its
    // lights are those kept, laid once as they rest; burning, all of them there are drawn as they burn at this frame,
    // and what is kept of them there is hidden until they rest again. The places are told apart to the pixel.
    const burning = new Set(meets.filter((m) => m.burn > RUN.portRest).map((m) => Math.round(m.place)));
    const built = new Set<number>();
    for (const { place, burn, key, strength, lay } of meets) {
      const where = Math.round(place);
      const part = this.endLights(key);
      if (restAgain) {
        lay(part.rest, RUN.portRest, 1);
        part.place = where;
        built.add(key);
      }
      for (const g of stroked(part.rest)) g.alpha = strength;
      if (burning.has(where)) lay(this.runEdges, burn, strength);
    }
    let endsShown = false;
    for (const [key, part] of this.ends.parts) {
      if (restAgain && !built.has(key)) {
        for (const g of stroked(part.rest)) g.destroy();
        this.ends.parts.delete(key);
        continue;
      }
      const shows = !burning.has(part.place);
      for (const g of stroked(part.rest)) g.visible = shows;
      endsShown ||= shows;
    }
    this.ends.far.renderable = this.ends.near.renderable = endsShown;
    // The glow of the ways is blurred as one at each frame, what stands of it and the light's: the blur's texels are
    // coarse ([`BLUR_TEXEL`]), so that it costs about a millisecond for a whole tree on the screen, and a way's
    // strength changing builds nothing again. A way's light is shown only while the light is on it, and the glow of
    // the ways hidden whole while no way is drawn, so that nothing is blurred for it.
    for (const w of wayOrder.map((i) => this.wayLights(i))) for (const g of stroked(w.light)) g.visible = w.lit;
    this.ways.far.renderable = this.ways.near.renderable = wayOrder.length > 0;
    // The glow over the cards is blurred only in a frame that lights an edge or burns where a way meets a card.
    this.runEdges.far.renderable = drawsAny(this.runEdges.far);
    this.runEdges.near.renderable = drawsAny(this.runEdges.near);
    if (restAgain) {
      // The ways not drawn are let go, and those drawn laid in the order they were, each way's light over what stands
      // of it.
      const drawnWays = new Set(wayOrder);
      for (const [i, part] of this.ways.parts) {
        if (drawnWays.has(i)) continue;
        for (const g of [...stroked(part.rest), ...stroked(part.light)]) g.destroy();
        this.ways.parts.delete(i);
      }
      for (const i of wayOrder) {
        const { rest, light } = this.wayLights(i);
        this.ways.far.addChild(rest.far, light.far);
        this.ways.near.addChild(rest.near, light.near);
        this.ways.lines.addChild(rest.line, light.line);
      }
      this.stats.wayBuilds += 1;
    }
    // The lights round the item under the pointer and those above it, and behind it, each easing in and out.
    const ease = (lights: Map<number, number>, on: Set<number>) => {
      for (const i of on) if (!lights.has(i)) lights.set(i, 0);
      for (const [i, a] of lights) {
        const to = on.has(i);
        // Come fully in, a light stands still.
        if (to && a === 1) continue;
        easing = true;
        const next = a + ((to ? 1 : 0) - a) * (1 - Math.exp(-step / (to ? GLOW_IN : GLOW_OUT)));
        if (!to && next < 0.01) lights.delete(i);
        else lights.set(i, to && 1 - next < SETTLED ? 1 : next);
      }
    };
    ease(this.glowing, chain);
    if (redraw) {
      // The dot of each other fold folded, or turning, in the funnel's grey, as strong as the funnels are drawn.
      this.dots.clear();
      for (const k of drawn) {
        const ness = this.dotness(k);
        if (ness <= 0 || dotted.has(k)) continue;
        const d = t.depth[k];
        const [dx, dy] = at(k, x(d) + NODE_W + DOT_AT, y(d, t.rank[k]));
        this.dots.circle(dx, dy, DOT_R * size(k) * ness).fill({ color: PALETTE.other, alpha: linksStrength(this.dim) });
      }
      // An item drawn in towards the dot of a turning fold, or growing out of it, has its lit way go with it, but not
      // its ribbon, ring or light, which wait until it is drawn whole.
      const isWhole = (i: number) => this.scaleOf[i] === 1;
      for (const i of chosen) {
        const p = t.parent[i];
        if (p < 0 || !isWhole(i)) continue;
        const [pb, ib] = [box(p), box(i)];
        this.funnel(pb.x + pb.w, pb.y - pb.h, pb.y + pb.h, ib.x, ib.y - ib.h, ib.y + ib.h);
      }
      if (chosen.size > 1) this.links.fill(PALETTE.chosenRibbon);
      for (const [i, a] of this.glowing) {
        const p = t.parent[i];
        if (p < 0 || !isWhole(i)) continue;
        const [pb, ib] = [box(p), box(i)];
        this.funnel(pb.x + pb.w, pb.y - pb.h, pb.y + pb.h, ib.x, ib.y - ib.h, ib.y + ib.h);
        this.links.fill({ color: PALETTE.hoverRibbon, alpha: a });
      }
      // Each card whose own work moves is lit round its edges in its tone, as the ways are lit, its line near white
      // where the page is dark and its glow in the tone, as bright as its card is drawn; a lifted card's over the
      // lifted cards.
      for (const lit of [this.toneEdges, this.toneEdgesLifted]) for (const g of [lit.line, lit.near, lit.far]) g.clear();
      for (const i of drawn) {
        if (this.movesAt[i] === 0 || !isWhole(i)) continue;
        const b = cardBox(i);
        const paint = paintOf(this.toneAt[i]);
        const on = liftedAt(i) ? this.toneEdgesLifted : this.toneEdges;
        const allRound = [{ from: 0, to: NODE_W, bright: 1 }];
        const strength = TONE_EDGE * this.views[i].alpha;
        this.across(b.x, b.y, b.h, allRound, paint.tint, paint.line, paint.glint, strength, cardSize(i), on);
      }
      for (const lit of [this.toneEdges, this.toneEdgesLifted]) {
        lit.far.renderable = drawsAny(lit.far);
        lit.near.renderable = drawsAny(lit.near);
      }
      // The item under the pointer and the item selected are lit by the light along their edges, drawn at every frame
      // after the still parts; the selected one's as strong again at the height of its flash.
      this.chosenDrawn = picked >= 0 && isWhole(picked) ? { id: this.ids[picked], out: flare, alpha: 1 + flare } : null;
      // The tree fades into the page at the top and the bottom of the screen, drawn over it at the screen's own size.
      this.edges.clear();
      this.edges.rect(0, 0, screen.width, EDGE).fill(this.fades.top);
      this.edges.rect(0, screen.height - EDGE, screen.width, EDGE).fill(this.fades.bottom);
      this.drawMap(t);
      for (const v of this.shown) if (!this.nowShown.has(v)) v.parent?.removeChild(v);
      this.shown = this.nowShown;
      this.stats.drawn = this.nowShown.size;
    }
    // The light along the edges of the card under the pointer and of the card selected, the selected one's as strong
    // again at the height of its flash: it reaches in from both the card's ends along its top and bottom edges, the
    // card's own sides lit, and meets in the middle; let go, it goes back out to the ends. Each half is brightest at
    // its head, as the light down a way is. It is white light, as the light's head is, and in the accent where the
    // page is light.
    const drawnWhole = (i: number) => i >= 0 && this.scaleOf[i] === 1;
    const wanted = new Map<number, number>();
    if (drawnWhole(picked)) wanted.set(this.ids[picked], 1 + flare);
    if (drawnWhole(this.under) && this.under !== picked) wanted.set(this.ids[this.under], EDGE_HOVER);
    for (const id of wanted.keys()) if (!this.edgesLit.has(id)) this.edgesLit.set(id, { reach: 0, strength: 0 });
    for (const g of stroked(this.edgeLight)) g.clear();
    const white = Number.parseInt(PALETTE.glowTo.slice(1), 16);
    const accent = Number.parseInt(PALETTE.accent.slice(1), 16);
    const [glow, core] = this.adds ? [white, white] : [accent, accent];
    for (const [id, e] of this.edgesLit) {
      const strength = wanted.get(id);
      const i = this.real.get(id);
      if (strength !== undefined) {
        e.strength = strength;
        e.reach = Math.min(1, e.reach + step / EDGE_IN);
      } else e.reach = Math.max(0, e.reach - step / EDGE_OUT);
      if (e.reach <= 0 || i === undefined || !drawnWhole(i)) {
        this.edgesLit.delete(id);
        continue;
      }
      const half = (NODE_W / 2) * e.reach * e.reach * (3 - 2 * e.reach);
      const pieces: { from: number; to: number; bright: number }[] = [];
      for (let k = 0; k < EDGE_PIECES; k += 1) {
        const bright = EDGE_TAIL + ((1 - EDGE_TAIL) * (k + 1)) / EDGE_PIECES;
        const [near, far] = [(half * k) / EDGE_PIECES, (half * (k + 1)) / EDGE_PIECES];
        pieces.push({ from: near, to: far, bright }, { from: NODE_W - far, to: NODE_W - near, bright });
      }
      const b = cardBox(i);
      this.across(b.x, b.y, b.h, pieces, () => glow, core, white, e.strength, cardSize(i), this.edgeLight);
    }
    this.edgeLight.far.renderable = drawsAny(this.edgeLight.far);
    this.edgeLight.near.renderable = drawsAny(this.edgeLight.near);
    // The marks of cells running pulse, and each work's age is kept up, at every frame.
    for (const i of drawn) {
      const p = this.pulses.get(i);
      if (p !== undefined) p.alpha = pulse;
      this.age(i);
    }
    this.easing = easing;
    this.dirty.clear();
    this.drawnView = { zoom: this.zoom, at: this.at, left: this.left };
    this.stats.blurs = this.blurred.filter((layer) => layer.visible && layer.renderable).length;
  }

  /// The layers blurred at each frame they draw something in: the glow of the lit ways further out and close in, the
  /// same where the ways meet the cards and at the dots of folds, the same over the cards, round the edges of the
  /// cards whose own work moves, and along the edges of the item under the pointer and the item selected.
  private get blurred(): Container[] {
    return [
      this.ways.far,
      this.ways.near,
      this.ends.far,
      this.ends.near,
      this.runEdges.far,
      this.runEdges.near,
      this.edgeLight.far,
      this.edgeLight.near,
      this.toneEdges.far,
      this.toneEdges.near,
      this.toneEdgesLifted.far,
      this.toneEdgesLifted.near,
    ];
  }

  /// The lights of the way into item `i`, made empty where there are none yet ([`WayLights`]).
  private wayLights(i: number): WayLights {
    const had = this.ways.parts.get(i);
    if (had !== undefined) return had;
    const strokes = (): Strokes => ({ line: new Graphics(), near: new Graphics(), far: new Graphics() });
    const part: WayLights = { rest: strokes(), light: strokes(), lit: false, place: 0 };
    for (const to of [part.rest, part.light]) {
      this.ways.far.addChild(to.far);
      this.ways.near.addChild(to.near);
      this.ways.lines.addChild(to.line);
    }
    this.ways.parts.set(i, part);
    return part;
  }

  /// The light at rest of the end or the dot kept by `key`, made empty where there is none yet ([`EndLights`]).
  private endLights(key: number): EndLights {
    const had = this.ends.parts.get(key);
    if (had !== undefined) return had;
    const part: EndLights = { rest: { line: new Graphics(), near: new Graphics(), far: new Graphics() }, place: 0 };
    this.ends.far.addChild(part.rest.far);
    this.ends.near.addChild(part.rest.near);
    this.ends.lines.addChild(part.rest.line);
    this.ends.parts.set(key, part);
    return part;
  }

}
