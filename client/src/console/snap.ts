// The console's tree in columns, drawn in WebGL by PixiJS. One column for each level, from the roots at the left; in
// each, the level's items one under another, those under one item standing apart from those under the next. Scrolling
// over a column moves it with the scroll, as far as leaves a few of its rows on the screen, and it rests where the
// scroll stops. Each column beside it goes with the one it follows, a row for a row, and keeps on the screen what joins
// it to the items the screen shows of that one, the block of items under each or the item above it: all of it where it
// fits, and else the screen full of it, with no room left at either edge; where keeping that on the screen holds it,
// it stands, and where it pushes it on, it goes on. Where it is first placed, the item at the middle has what it joins
// level with it. It moves only while the one it follows moves, and only that way, so that none goes against it and
// every column stops when the scroll does. Each column further out follows the one beside it the same way.
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
// item selected flashes once, whether or not the tree moved to bring it into view: its soft light grows wider and
// stronger, and eases back to rest.
// What stands is drawn once and kept until something in it changes: a tree read again that draws the same makes
// nothing again, and one that changed makes again only the cards that changed; a frame in which only the lights, the
// pulses and the ages move draws only those, the cards standing in a group of their own that is drawn as it was. The
// glow of the lit ways is drawn and blurred on texels coarser than the canvas, a part of its blur's strength wide
// ([`BLUR_TEXEL`]), and each layer of glow is blurred over the room its lights take in a frame, and not at all in a
// frame where it has none. Nothing is drawn while the page or the console's view is hidden.
//
// A card's second row draws the level the work's cell runs at, the works page's strip of the work's cells, and the
// lines the work changed; an ended item's one row, the lines its work changed. What the interface has nothing for yet
// (an item new to the owner, messages unread) is read from the console's made-up data (`mock.ts`). The colours are read
// from the page's tokens, so that the canvas follows light and dark; the canvas follows the size of what holds it,
// besides the window's; the links dim with the items they join while an item is lit (`DIM_LINKS`); and the way from
// each item whose work runs up to its root is lit, with a light running down it (`RUN`).

import {
  Application,
  BitmapText,
  BlurFilter,
  Container,
  FillGradient,
  Graphics,
  Matrix,
  RenderTexture,
  Sprite,
  TextStyle,
} from 'pixi.js';

import { accentNow, onAccent, turning } from '../accent';
import { LINES_LOOKS, levelLook, levelPointed, levelWords } from '../marks';
import { CELL_LOOKS, MARK, pulsing } from '../works/state-marks';
import type { CellNow, CellState } from '../works-api';
import { NO_LEVEL, STATE, itemName, linesAdded, linesRemoved, stateOf, stoppedAt } from '../works/words';
import { type Fold, STANDINGS, type Shown, type ShownWork, age } from './items';
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
/// On the map: the gap between two items' pieces of a column, where each is tall enough to keep one; how far the frame
/// of what the screen shows stands out past the bars it holds; and how strong the ground in it is.
const MAP_GAP = 0.5;
const MAP_FRAME_OUT = 2;
const MAP_FRAME_FILL = 0.06;
/// How quickly the blue lines at a column's edges come and go as the pointer comes over it and leaves: the
/// milliseconds in which it goes about two thirds of the way.
const TURN = 140;
/// How far in from the top and the bottom of the screen the columns fade out into the page, in pixels; and how thick the
/// blue line is at each edge of the column under the pointer.
export const EDGE = 48;
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
/// The flash of the item selected's soft light, which a click on the item's title in its window asks for, so that the
/// eye finds the card whether or not the tree moved to bring it into view. At its height the light reaches this many
/// pixels past the card's edge, four times as far as at rest, at full strength, more than twice its strength at rest.
/// It rises over this part of its time and eases back to rest over the rest, this many milliseconds in all
/// ([`flashOf`]).
const FLASH_SPREAD = 16;
const FLASH_RISE = 0.15;
export const FLASH_MS = 800;
/// The room a soft light's blur is drawn in past its fill, in strengths of the blur: wide enough that the blur fades
/// out before its edge. It is also as far as the light reaches past its fill.
const BLUR_ROOM = 4;
/// The glow of a lit way is drawn and blurred on a texture coarser than the canvas: one texel to this part of its
/// blur's strength, in pixels of the page, and never finer than the canvas. A blur `s` strong spreads a line about
/// 1.3 `s` either way, and a texel a quarter of `s` wide spreads it under 2% further, which does not show; and as the
/// texel is a part of the blur and not of the canvas, a glow is drawn on the same texels at a pixel ratio of 1 and of
/// 2, and looks the same at both. The near glow, 4 strong, is blurred on texels a pixel wide; the far glow, 14
/// strong, on texels 3.5 pixels wide, so on a twelfth of the texels. The soft lights round the items keep texels a
/// pixel wide: the flash of the item selected grows its light by a few pixels a frame, which coarser texels would show
/// in steps.
const BLUR_TEXEL = 0.25;
/// The step in pixels by which a texture a kept glow is laid on is made larger than the glow ([`SnapScene.bake`]).
const TEXTURE_STEP = 128;

/// How far on a flash of the item selected's soft light is, `ms` into it: from 0 up to 1 at its height and back down
/// to 0 once its time is over, smoothly both ways ([`FLASH_MS`]).
function flashOf(ms: number): number {
  const u = ms / FLASH_MS;
  if (u <= 0 || u >= 1) return 0;
  const k = u < FLASH_RISE ? u / FLASH_RISE : (1 - u) / (1 - FLASH_RISE);
  return k * k * (3 - 2 * k);
}

/// The blur of a soft light round an item, this strong, with its room: set after the filter is made, which sets its
/// own from its strength, too narrow for the blur to fade out in.
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

/// A part of the lights that stand still, kept until what it lights moves ([`draw`]): its line, drawn sharp, and its
/// glow close in and further out, blurred once onto a texture of its own as large as the glow reaches (`source`),
/// which `glow` draws where it stands.
type Rest = Strokes & { source: Container; glow: Sprite; texture: RenderTexture | null };

/// Lights that stand still, in parts each kept by its key ([`Rest`]): the parts' lines among `lines`, and their glows
/// among `glows`, each laid where the lights it keeps are and blurred as a layer of light's glow is ([`Lit`]).
type Kept = { parts: Map<number, Rest>; lines: Container; glows: Container; nearBlur: BlurFilter; farBlur: BlurFilter };

function keptLayer(): Kept {
  return {
    parts: new Map(),
    lines: new Container(),
    glows: new Container(),
    nearBlur: softBlur(RUN.nearBlur),
    farBlur: softBlur(RUN.farBlur),
  };
}

/// A lit way's own lights ([`draw`]): what stands of it, its line, glow and strands, kept while the way stands; and
/// over that the light running down it, drawn anew at each frame, and only while the light is on it (`lit`). Where
/// the light is on none of the ways that begin at the same place across the page as this one, `place`, the glow kept
/// of all of them is drawn instead of theirs ([`Kept`]).
type WayLights = { rest: Strokes; light: Strokes; lit: boolean; place: number };

/// The lit ways, each way's own lights by the item it goes into ([`WayLights`]): their lines among `lines`, and their
/// glow further out and close in among `far` and `near`, each blurred as one, as a layer of light's is ([`Lit`]). Each
/// way's light lies just over what stands of it, as when both were drawn on one layer.
type Ways = {
  parts: Map<number, WayLights>;
  lines: Container;
  near: Container;
  far: Container;
  nearBlur: BlurFilter;
  farBlur: BlurFilter;
};

function waysLayer(): Ways {
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

/// Blurs `blur` `strength` strong, with its room, on the texels a glow that strong is drawn on, on a canvas of `canvas`
/// pixels of the screen to one of the page, where it is not already.
function blurTo(blur: BlurFilter, strength: number, canvas: number) {
  if (blur.strength !== strength) {
    blur.strength = strength;
    blur.padding = BLUR_ROOM * strength;
  }
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
/// The part of the way to where they are going that the columns move sideways each frame, when the scene brings an
/// item into view.
const SNAP = 0.32;
/// How much of the way to where the scroll has taken it the column scrolled goes each frame; and how many pixels of the
/// way, which no screen shows, may be left when it goes the rest at once, so that it comes to rest where the scroll took
/// it within some thirty frames, and does not creep on unseen until what is left is too small for a number to hold.
const GLIDE = 0.35;
const REST = 0.01;
/// How many rows of the column scrolled stay on the screen at least, at its top or its bottom.
export const KEEP = 3;
/// How much of the way to where it stands a column beside the one scrolled goes each frame where it glides: where it is
/// first placed, and where what keeps what joins it on the screen is far, as for a column that was the one scrolled and
/// rests where its own scroll left it until another is.
const LINK_EASE = 0.2;

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
  /// What is new to the owner: the line inside a card's left edge, and its mark on the map.
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

/// A card as it was made, kept for the next tree that draws one the same ([`show`]): what it draws, its view, the room
/// of its strip, and what its making set: its age, the pulse of its running cells, its strip, its look and the number
/// it wrote, the last three for the tests.
type CardMade = {
  sig: string;
  view: Container;
  strip: { start: number; end: number; count: number } | null;
  age: { from: number; label: BitmapText; shown: string; row: number } | undefined;
  pulse: Graphics | undefined;
  drawnStrip: DrawnStrip | undefined;
  look: { rows: number; fill: string | number; edge: string | number } | undefined;
  number: DrawnNumber | undefined;
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
  /// How many layers the last frame blurred: the glow of the lit ways close in and further out, the same over the
  /// cards, and the soft lights round the item selected and the item under the pointer, each only while it draws
  /// something.
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

/// An item's piece of a column's bar on the map, as it was drawn: the item, its column, its top on the map and how
/// tall, and its colour.
export type MapPiece = { id: number; column: number; y: number; height: number; colour: string };

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
  /// ([`Ways`]); and the glow of the ways that begin at each place across the page, kept and laid as it is while the
  /// light is on none of them ([`Kept`]), so that only the glow of the ways the light is on is blurred at each
  /// frame. What of them is drawn over the cards instead: the spark where a way meets a card, and a card's edge as
  /// a light crosses it, which the cards, drawn over the ways, would else hide, in lines and their glow ([`Lit`]); and
  /// the lights at rest where the ways meet the cards and at the dots of folds, kept for each place across the page the
  /// light comes to ([`Kept`]), so that where it burns, that place alone is drawn as it burns and the others stand as
  /// they were laid.
  private ways = waysLayer();
  private waysAtRest = keptLayer();
  private runEdges = litLayer();
  private endsAtRest = keptLayer();
  /// The numbers the lit ways were last built by ([`RUN`]), which the page's knobs may change while it runs; and the
  /// strength of each fold's dot as it was last built.
  private runNumbers = '';
  private dotsBuilt: number[] = [];
  /// Whether what they draw is added to the ground, as the theme has it ([`adding`]).
  private adds = false;
  /// The soft lights behind the item selected and the item under the pointer, drawn anew each frame where they stand,
  /// blurred.
  private chosenGlow = new Graphics({ filters: [softBlur(CHOSEN_BLUR)] });
  private hoverGlow = new Graphics({ filters: [softBlur(SOFT_BLUR)] });
  /// The flash of the item selected's soft light under way, by the item's number and the scene's time it began at
  /// ([`flash`]); and the item selected's soft light as it was last drawn, for the tests ([`chosenLight`]).
  private flashing: { id: number; at: number } | null = null;
  private chosenDrawn: { id: number; out: number; alpha: number } | null = null;
  /// The page's ground at each item a soft light reaches, over the lights and under the cards.
  private glowGround = new Graphics();
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
  /// What the map drew at the last frame ([`drawnMap`]).
  private mapDrawn: { frame: { x: number; y: number; width: number; height: number } | null; pieces: MapPiece[] } = {
    frame: null,
    pieces: [],
  };
  private tree: Tree | null = null;
  /// Each item's number, in the tree's order.
  private ids: number[] = [];
  private views: Container[] = [];
  /// Each item's height in rows when drawn whole, and as drawn now; how much of its whole size it is drawn at, less
  /// than whole only while a fold over it turns; and for each column, where each of its items starts, its rows counted
  /// from the top, and the room between its groups above each.
  private base: Float64Array = new Float64Array(0);
  private rows: Float64Array = new Float64Array(0);
  private scaleOf: Float64Array = new Float64Array(0);
  private pre: Float64Array[] = [];
  private gaps: Float64Array[] = [];
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
  private drawnLeft = Number.NaN;
  private drawnCols: number[] = [];
  private wayCache = new Map<number, { way: Way; len: number; dist: number }>();
  private waysDrawn: LitWay[] = [];
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
  /// For each column, the ranks of its items new to the owner, or with one under them folded away, which the map marks.
  private fresh: number[][] = [];
  private columns: Column[] = [];
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
  /// While the console's search is open, the items it finds, by number: lit as the items in focus are, every other
  /// item dimmed, all of them while it finds nothing; none while it is closed.
  private found: number[] | null = null;
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
    // What stands still of the lights lies under what moves of them, its glow under their glow and its lines under
    // their lines.
    this.world.addChild(
      this.links,
      this.foldLight,
      this.dots,
      this.waysAtRest.glows,
      this.ways.far,
      this.ways.near,
      this.ways.lines,
      this.hoverGlow,
      this.chosenGlow,
      this.glowGround,
      this.layer,
      this.endsAtRest.glows,
      this.runEdges.far,
      this.runEdges.near,
      this.endsAtRest.lines,
      this.runEdges.line,
      this.marks,
    );
    this.app.stage.addChild(this.world, this.edges, this.map);
    // Nothing is blurred until a frame draws something to blur ([`draw`]).
    for (const layer of this.blurred) layer.renderable = false;
    // The cards stand in a group of their own, which is drawn as it was while none of them changes, whatever the lights
    // over and under them do.
    this.layer.enableRenderGroup();
    this.map.position.set(MAP_INSET, MAP_INSET);
    this.map.addChild(this.mapFrame);
    const canvas = this.app.canvas;
    canvas.addEventListener('wheel', this.wheel, { passive: false });
    canvas.addEventListener('pointermove', this.hover);
    canvas.addEventListener('pointerleave', () => {
      this.mark('pointer');
      this.hovered = -1;
      this.pointer = null;
    });
    canvas.addEventListener('pointerdown', (e) => {
      this.dragging = { x: e.clientX };
      this.down = { x: e.clientX, y: e.clientY };
    });
    window.addEventListener('pointermove', this.drag);
    window.addEventListener('pointerup', this.up);
    document.addEventListener('visibilitychange', this.run);
    this.app.ticker.add(this.tick);
    this.ready = true;
    this.run();
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
    if (this.ready) {
      // The lights kept go first, each with its texture, which the application does not hold.
      for (const kept of [this.waysAtRest, this.endsAtRest]) {
        for (const key of [...kept.parts.keys()]) this.dropPart(kept, key);
      }
      this.waysAtRest.lines.destroy();
      this.app.destroy(true, { children: true, texture: true });
    }
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

  /// Where the columns stand, for the tests: how far they are moved right, the column the others follow, and each
  /// column's place, in rows from its first item to the middle of the screen, and where it is going; and how many folds
  /// are turning.
  place(): {
    left: number;
    driver: number;
    columns: { at: number; aim: number }[];
    height: number;
    width: number;
    turns: number;
  } {
    return {
      left: this.left,
      driver: this.driver,
      columns: this.columns.map((c) => ({ at: c.at, aim: c.aim })),
      height: this.app.screen.height,
      width: this.app.screen.width,
      turns: this.turns.size,
    };
  }

  /// Draws the tree of `roots` in place of the one before. Each column stands as it stood by the item that was nearest
  /// the middle of the screen, where that item is still in it, and else where it was; a column new to the tree stands
  /// where its links put it, and at first the middle root stands in the middle.
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
    const before = this.columns.map((c, d) => {
      if (old === null || old.columns[d].length === 0) return null;
      const r = Math.min(old.columns[d].length - 1, this.rankAt(d, c.at));
      return { id: this.ids[old.columns[d][r]], off: c.at - this.middle(d, r) };
    });
    const was = this.columns.map((c) => c.at);
    this.roots = roots;
    // What eases by item, kept by the item into the new tree: the lights round it, the strength of the way into it,
    // what is in focus and what the pointer was last on; and how far the blue lines of each column had come.
    const byId = (m: Map<number, number>) =>
      new Map([...m].flatMap(([k, a]) => (this.shadows[k] === 1 ? [] : [[this.ids[k], a] as const])));
    const easedBy = { glowing: byId(this.glowing), softLit: byId(this.softLit), runStrength: byId(this.runStrength) };
    const focusIds = [...this.inFocus].flatMap((k) => (this.shadows[k] === 1 ? [] : [this.ids[k]]));
    const hoverId = this.hoverWas >= 0 && this.shadows[this.hoverWas] === 0 ? this.ids[this.hoverWas] : null;
    const warmth = this.columns.map((c) => c.warm);
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
    this.hoverWas = hoverId === null ? -1 : (this.real.get(hoverId) ?? -1);
    this.under = -1;
    this.underFold = -1;
    this.drawnMoved = null;
    this.waysDrawn = [];
    this.base = Float64Array.from(items, (i) =>
      i.work === null || i.work.from === null ? 1 : 1 + WORK_ROW,
    );
    this.shape();
    const unseen = (s: Shown) => s.unseen;
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
    // The way from each item whose work is under way up to its root, under a turning fold as well: there it is drawn
    // with the cards as they draw in to the dot or grow out of it ([`ends`] in `draw`). A work is under way while a round
    // of it runs and while the engine runs a call for it, as its tests: its card says which, and the way, which other
    // works share, is lit alike for both. The dot of an item folded, or whose fold turns, with such an item under it ends
    // the way: the way runs from the root through the item's card to the dot, which comes and goes with the turn.
    const running = (s: Shown) => {
      const mood = s.shadow || s.work === null ? null : moodOf(s.work);
      return mood === 'running' || mood === 'calling';
    };
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
    this.softLit = back(easedBy.softLit);
    this.ages = new Map();
    this.strips = [];
    this.cardsDrawn = new Map();
    this.numbersDrawn = new Map();
    // Each card is made only where no card drawn before draws the same: else the one drawn is kept, with what its
    // making set. What a card draws is its item as the cards draw it, the rows it stands in, whether a work goes on on
    // it or under it, whether one has begun there, and the theme. A card made new comes in as dim as the tree is.
    const reuse = this.cardCache;
    const sigs = items.map((item, k) => {
      const drawn = JSON.stringify({ ...item, children: undefined }, undrawn);
      return `${this.themed}|${this.titleOffset}|${this.base[k]}|${this.lit[k]}|${this.begun[k]}|${drawn}`;
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
        number: this.numbersDrawn.get(k),
      };
      this.cardCache.set(sigs[k], [...(this.cardCache.get(sigs[k]) ?? []), card]);
    });
    this.stats.made = kept.filter((c) => c === undefined).length;
    this.shown = new Set([...this.shown].filter((v) => !v.destroyed));
    this.columns = tree.columns.map((_, d) => ({ at: 0, aim: 0, warm: warmth[d] ?? 0 }));
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

  /// Sizes every item and its column as the folds turning stand now: an item under a turning fold at that fold's size,
  /// times the size of any fold turning over that, its rows and the room above its group with it.
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
    this.pre = t.columns.map((col) => {
      const p = new Float64Array(col.length + 1);
      for (let r = 0; r < col.length; r += 1) p[r + 1] = p[r] + this.rows[col[r]];
      return p;
    });
    // The room above each group of items under one item, the first of a column's groups having none: as large as the
    // group is drawn, so that a group drawn whole has the whole of it.
    this.gaps = t.columns.map((col) => {
      const g = new Float64Array(col.length);
      let groups = 0;
      for (let r = 1; r < col.length; r += 1) {
        if (t.parent[col[r]] !== t.parent[col[r - 1]]) groups += scale[col[r]];
        g[r] = APART * groups;
      }
      return g;
    });
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
      const fill = filled ? mix(PALETTE.card, state, FILL) : PALETTE.card;
      const edge =
        state !== null
          ? mix(PALETTE.card, state, mood === 'running' ? EDGE_RUNNING : EDGE_TINT)
          : this.lit[k] === 1
            ? PALETTE.branch
            : PALETTE.edge;
      view.addChild(card.fill(fill).stroke({ color: edge, width: EDGE_WIDTH }));
      this.cardsDrawn.set(item.id, { rows: this.base[k], fill, edge });
    }
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
    // which hold the work at nothing yet, white on orange; its messages unread, white on the accent; and its returns
    // sent back, framed in grey. What an item waits for stands under it in the tree, and no tag counts it again.
    let right = NODE_W - 8;
    const tag = (text: string, fill: string, sign: 'back' | null, solid: boolean) => {
      const t = new BitmapText({ text, style: s.mono(solid ? PALETTE.onTag : fill) });
      const pad = 4;
      const icon = sign !== null ? 10 : 0;
      const w = t.width + icon + 2 * pad;
      right -= w;
      const g = new Graphics();
      if (solid) g.roundRect(right, row1 - 8, w, 16, 3).fill(fill);
      else g.roundRect(right, row1 - 8, w, 16, 3).stroke({ color: fill, width: 1 });
      const ax = right + pad;
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
    if (item.ahead > 0) tag(String(item.ahead), PALETTE.orange, null, true);
    if (item.unread > 0) tag(String(item.unread), PALETTE.unseen, null, true);
    if (item.work !== null && item.work.sentBack > 0) tag(String(item.work.sentBack), PALETTE.muted, 'back', false);
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
    if (w === null || mood === null || w.from === null) return view;
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
    const at = here(w);
    const start = Math.max(title.x, x + level.width + 4);
    const end = LINES_X - 8;
    this.strips.push({
      k,
      start,
      end,
      count: w.cells.length,
      draw: (scale) => {
        this.strip(view, item.id, k, w.cells, start, row2, end, scale, at);
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
  /// order of the walk, as the works page's (`state-marks.ts`), from `x` and no further than `end`, their middles on
  /// `y`. Every cell is drawn: the marks, the gaps between them, the dashes' height, the corners and the rings are all
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
    at: { cell: string; colour: string } | null = null,
  ) {
    const side = this.measures.side * scale;
    const pitch = side + this.measures.gap * scale;
    const marks: DrawnMark[] = cells.map((c, i) => {
      const own = this.looks[c.state];
      // The cell the work is at, while it waits for the owner or is halted, in the colour of its state, filled.
      const look = at !== null && at.cell === c.cell ? { ...own, fill: at.colour, edge: at.colour } : own;
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
    if (marks.some((m) => m.pulses)) this.pulses.set(k, running);
    this.drawnStrips.set(id, { marks, more: 0, end, level: null });
  }

  /// How the card of item `id` was drawn, for the tests: how many rows high, and its fill and its edge's colour. None
  /// for an item the tree does not draw, a shadow, and a deliverable, which has no card.
  cardLook(id: number): { rows: number; fill: string | number; edge: string | number } | null {
    return this.cardsDrawn.get(id) ?? null;
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
    const line = this.app.screen.height / 2;
    const [x, y] = [this.left + d * PITCH, line + (this.middle(d, t.rank[i]) - this.columns[d].at) * ROW];
    const m = this.drawnMoved;
    if (m === null) return { x, y, size: 1, drawn: [x, y] };
    return { x, y, size: m.a[i], drawn: [m.a[i] * x + m.bx[i], m.a[i] * y + m.by[i]] };
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

  /// The soft light round the item selected as it was last drawn, and so stands, for the tests: the item's number, how
  /// many pixels past its card's edge its fill reaches, and how strong the fill is. None while no item selected is
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

  /// Moves column `c` by its links with the column `from` beside it, which it follows. What joins `c` to the part of
  /// `from` within the screen's clear part, the block of the items under each item or the item above it, stays on the
  /// screen: all of it where it fits, and else the screen full of it. Given `way`, how far `from` moved in this frame,
  /// `c` goes as far as `from`, a row for a row, and no further than keeps what joins them on the screen; where keeping
  /// it there pushes it on, it goes on. Each goes only the way `from` moved, so that no column goes against the one it
  /// follows, none outruns it but where the screen asks, and none moves while it stands. Without `way`, as where it is
  /// first placed, it glides to standing level, kept as above: where the item of `from` at the middle of the screen has
  /// what it joins level with it, going over evenly between two such items, and the first's or the last's beyond them.
  /// With nothing joining them, it stays.
  private link(c: number, from: number, way?: number): void {
    const t = this.tree as Tree;
    const me = this.columns[c];
    const col = t.columns[from];
    // Each item of `from` that joins `c`: where its middle stands in `from`, and the rows of `c` its block takes.
    const ys: number[] = [];
    const tops: number[] = [];
    const bottoms: number[] = [];
    for (let r = 0; r < col.length; r += 1) {
      const item = col[r];
      if (c > from) {
        const kids = t.children[item];
        if (kids.length === 0) continue;
        const lastKid = t.rank[kids[kids.length - 1]];
        tops.push(this.slot(c, t.rank[kids[0]]));
        bottoms.push(this.slot(c, lastKid) + this.rowsAt(c, lastKid));
      } else {
        const above = t.parent[item];
        if (above < 0) continue;
        tops.push(this.slot(c, t.rank[above]));
        bottoms.push(tops[tops.length - 1] + this.rowsAt(c, t.rank[above]));
      }
      ys.push(this.middle(from, r));
    }
    if (ys.length === 0 || way === 0) return;
    // What `v` gives for the row `y` of `from`: its value at each item, going over evenly between two, and the first's or
    // the last's beyond them.
    const along = (v: number[], y: number) => {
      if (y <= ys[0]) return v[0];
      for (let i = 1; i < ys.length; i += 1)
        if (y <= ys[i]) return v[i - 1] + ((v[i] - v[i - 1]) * (y - ys[i - 1])) / (ys[i] - ys[i - 1]);
      return v[v.length - 1];
    };
    const line = this.columns[from].at;
    const clear = Math.max(0, this.app.screen.height / 2 / ROW - EDGE / ROW);
    const level = (along(tops, line) + along(bottoms, line)) / 2;
    // From `first` to `end` is all on the screen while the place is between `end - clear` and `first + clear`; where it
    // is more than the screen holds the two cross, and between them the screen is full of it.
    const a = along(bottoms, line + clear) - clear;
    const b = along(tops, line - clear) + clear;
    const [lo, hi] = [Math.min(a, b), Math.max(a, b)];
    if (way === undefined) {
      const to = Math.max(lo, Math.min(hi, level));
      me.at = Math.abs(to - me.at) * ROW < REST ? to : me.at + (to - me.at) * LINK_EASE;
      return;
    }
    let next = me.at + way;
    const walled = Math.max(lo, Math.min(hi, next));
    // Pushed on by what keeps it on the screen: at once while that is near, and gliding where it is far, as for a
    // column that was the one scrolled and stood apart.
    if ((walled - next) * way > 0) next += (walled - next) * (Math.abs(walled - next) <= 1 ? 1 : LINK_EASE);
    else if (walled !== next) next = way > 0 ? Math.max(me.at, hi) : Math.min(me.at, lo);
    me.at = next;
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
    for (const lit of [this.ways, this.waysAtRest, this.runEdges, this.endsAtRest]) {
      lit.nearBlur.blendMode = how;
      lit.farBlur.blendMode = how;
    }
    this.ways.lines.blendMode = how;
    this.runEdges.line.blendMode = how;
    this.endsAtRest.lines.blendMode = how;
    // The glow kept is laid on its texture by its blur, as a layer's is laid on the page, and the texture on the page
    // as that blur would lay it.
    this.waysAtRest.glows.blendMode = how;
    this.endsAtRest.glows.blendMode = how;
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
      this.coats(this.runEdges, lay, tint(bright), line, strength * bright, RUN.edgeWide, nose);
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
    let hi = this.gaps[d].length;
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
    const g = this.gaps[d];
    const r = Math.max(0, Math.min(g.length - 1, rank));
    return this.pre[d][r] + (rank - r) + g[r];
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

  /// Draws `frames` frames, every sixth with a scroll of most of a row over a column, waiting for the GPU each; gives
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
      if (k % 6 === 0) this.scroll(1 + (k % 4), ROW * 0.7);
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
    const still = [this.layer, this.links, this.foldLight, this.dots, this.hoverGlow, this.chosenGlow, this.glowGround];
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

  /// Draws each column on the map as the items it holds, moved as the column is: each item a piece of the column's bar
  /// as tall as its rows, a hairline apart where there is room, in the colour of how its card stands ([`mapColour`]);
  /// in the accent across the bar where items are new to the owner; and where the item selected stands, the accent over
  /// every other mark. Over the bars, in a thin light line round a faint ground that the bars show through, the part of
  /// the tree the screen shows: from the first column it shows of to the last, as many rows as it holds round the
  /// middle, where each column's place on the screen is. What it drew is kept, for the tests.
  private drawMap(t: Tree) {
    const mid = MAP_HIGH / 2;
    const { width, height } = this.app.screen;
    const barAt = (d: number) => MAP_PAD + d * (MAP_STRIP + MAP_APART);
    this.mapBars.clear();
    const pieces: MapPiece[] = [];
    this.columns.forEach((c, d) => {
      const left = barAt(d);
      t.columns[d].forEach((k, r) => {
        const from = mid + (this.slot(d, r) - c.at) * this.mapRow;
        const tall = this.rowsAt(d, r) * this.mapRow;
        const gap = tall > 3 * MAP_GAP ? MAP_GAP : 0;
        const colour = this.mapColour(k);
        this.mapBars.rect(left, from, MAP_STRIP, Math.max(0.5, tall - gap)).fill(colour);
        pieces.push({ id: this.ids[k], column: d, y: from, height: tall, colour });
      });
      // Where items are new to the owner, the accent across the bar.
      for (const r of this.fresh[d] ?? []) {
        const at = mid + (this.slot(d, r) + this.rowsAt(d, r) / 2 - c.at) * this.mapRow;
        this.mapBars.rect(left - 1, at - 1, MAP_STRIP + 2, 2).fill(PALETTE.accent);
      }
    });
    // Where the item selected stands, the accent across its column's bar, over every other mark, a little wider, so that
    // it is found again from anywhere.
    const chosen = this.selected === null ? undefined : this.real.get(this.selected);
    if (chosen !== undefined) {
      const [d, r] = [t.depth[chosen], t.rank[chosen]];
      const c = this.columns[d];
      const at = mid + (this.slot(d, r) + this.rowsAt(d, r) / 2 - c.at) * this.mapRow;
      this.mapBars.rect(barAt(d) - 2, at - 1.5, MAP_STRIP + 4, 3).fill(PALETTE.chosen);
    }
    // The part of the tree on the screen: the columns any of whose cards' room is on it, and round the middle the rows
    // it holds.
    const shown = this.columns.flatMap((_, d) => {
      const x = this.left + d * PITCH;
      return x + NODE_W > 0 && x < width ? [d] : [];
    });
    let frame: { x: number; y: number; width: number; height: number } | null = null;
    if (shown.length > 0) {
      const [first, last] = [shown[0], shown[shown.length - 1]];
      const half = (height / ROW / 2) * this.mapRow;
      frame = {
        x: barAt(first) - MAP_FRAME_OUT,
        y: mid - half,
        width: barAt(last) + MAP_STRIP - barAt(first) + 2 * MAP_FRAME_OUT,
        height: 2 * half,
      };
      this.mapBars
        .rect(frame.x, frame.y, frame.width, frame.height)
        .fill({ color: PALETTE.ink, alpha: MAP_FRAME_FILL })
        .stroke({ color: PALETTE.faint, width: 1 });
    }
    this.mapDrawn = { frame, pieces };
  }

  /// The colour an item's piece of the map takes, as its card stands: of its work's state while a work is drawn on it,
  /// running, in a call, for the owner or halted, or paused or waiting; the light grey of an item whose work has not
  /// begun, or a shadow; the faint grey of a done or cancelled item, as its card is dim.
  private mapColour(k: number): string {
    const item = this.shownItems[k];
    if (!item.shadow && item.work !== null) return WORK()[moodOf(item.work)];
    if (!item.shadow && item.standing !== 'open') return PALETTE.hair;
    return PALETTE.pale;
  }

  /// What the map drew at the last frame, for the tests: the part of the tree the screen shows, on the map, and each
  /// item's piece, by its column, from its top on the map and how tall, in its colour.
  drawnMap(): { frame: { x: number; y: number; width: number; height: number } | null; pieces: MapPiece[] } {
    return this.mapDrawn;
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
    this.mark('pointer');
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
    this.mark('pointer');
    this.dragging = null;
    const d = this.down;
    this.down = null;
    if (d === null || Math.hypot(e.clientX - d.x, e.clientY - d.y) > 4 || this.tree === null) return;
    // A click on the funnel of an item's fold folds the items under it away, on its dot shows them again, and either
    // selects nothing.
    if (this.turnFold(this.underFold)) return;
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
    this.mark('view');
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

  /// Flashes the soft light round the item numbered `id` once, from the next frame: it grows wider and stronger and
  /// eases back to rest ([`FLASH_SPREAD`]). Only the item selected has that light, so the flash ends once `id` is not
  /// the item selected; and a flash asked for again begins again.
  flash(id: number): void {
    this.mark('flash');
    this.flashing = { id, at: this.clock };
  }

  /// Folds the items under the card the pointer is over away, or shows them again, as a click on its funnel or its dot
  /// does, the console told; a shadow, or a card with nothing under it, folds nothing. Whether it folded or unfolded.
  foldPointedCard(): boolean {
    return this.turnFold(this.under);
  }

  /// Tells the console to fold the items under the item at `k` away, or show them again, where it has any; its column
  /// becomes the one the others follow, so that the items shown under it come level with it. Whether it had any.
  private turnFold(k: number): boolean {
    const fold = k < 0 ? null : this.shownItems[k].fold;
    if (fold == null || this.tree === null) return false;
    const c = this.tree.depth[k];
    this.driver = c;
    this.columns[c].aim = this.columns[c].at;
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
    const { width, height } = this.app.screen;
    const line = height / 2;
    // The columns take the sizes the folds turning give them now. The column followed goes a part of the way to where
    // it is taken; the others after it, out from it each way.
    if (this.turns.size > 0) this.shape();
    const was = this.columns.map((c) => c.at);
    // Whether anything the still parts draw is still easing at this frame, which draws them again at the next.
    let easing = false;
    this.columns.forEach((c, d) => {
      // The blue lines at the column's edges fade in while the pointer is over it, and out once it leaves, in about
      // half a second.
      const warm = d === this.hovered ? 1 : 0;
      c.warm += (warm - c.warm) * (1 - Math.exp(-step / TURN));
      if (Math.abs(warm - c.warm) < SETTLED) c.warm = warm;
      else easing = true;
      if (d !== this.driver) return;
      c.at += (c.aim - c.at) * GLIDE;
      if (Math.abs(c.aim - c.at) * ROW < REST) c.at = c.aim;
    });
    const moved = (d: number) => this.columns[d].at - was[d];
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
    // The still parts of the tree, the cards where they stand, the funnels, the rings and the ribbons, the soft lights,
    // the dots, the edges and the map, are drawn again only where something they draw changed: a new tree, the
    // pointer, a selection, a flash, the columns moving, a fold turning, the screen's size, or what was still easing at
    // the last frame. Else they stand as they were drawn, and only the lights, the pulses and the ages are drawn.
    // Moved since the last frame: by the glide of this frame, or by a pan or a scroll between the two.
    const shifted =
      this.left !== this.drawnLeft ||
      this.columns.length !== this.drawnCols.length ||
      this.columns.some((c, d) => c.at !== this.drawnCols[d]);
    const sized = width !== this.drawnSize[0] || height !== this.drawnSize[1];
    const why = [...this.dirty];
    if (this.turns.size > 0) why.push('turns');
    if (shifted) why.push('moved');
    if (sized) why.push('size');
    if (this.easing) why.push('easing');
    const redraw = why.length > 0;
    for (const w of why) this.stats.redrawn[w] = (this.stats.redrawn[w] ?? 0) + 1;
    // Where the ways stand anew: a new tree, the theme, a fold turning, the columns moved, the screen's size, or the
    // numbers the ways are drawn by changed by a knob. Their curves are worked out again, and what stands of them
    // built again.
    const anew = ['start', 'tree', 'shown'].some((w) => this.dirty.has(w));
    const numbers = Object.values(RUN).join();
    const tuned = numbers !== this.runNumbers;
    this.runNumbers = numbers;
    const placed = anew || this.turns.size > 0 || shifted || sized || tuned;
    // While a fold turns, what is under it is drawn in towards its dot, or grows back out of it: where a point of an
    // item `k` is drawn, and how large the item is.
    const m = this.turns.size === 0 ? null : this.movedNow(t, x, y);
    this.drawnMoved = m;
    const at = (k: number, px: number, py: number): [number, number] =>
      m === null ? [px, py] : [m.a[k] * px + m.bx[k], m.a[k] * py + m.by[k]];
    const size = (k: number) => (m === null ? 1 : m.a[k]);
    if (redraw) {
      this.drawnSize = [width, height];
      this.stats.redraws += 1;
      this.links.clear();
      this.marks.clear();
      const nowShown = new Set<Container>();
      const drawn: number[] = [];
      const rows = Math.ceil(height / ROW / 2) + 2;
      // The funnel under the pointer, as it is drawn: every funnel folds, one over shadows alone as well.
      const pointer = this.pointer;
      let funnelled: { k: number; shape: [number, number, number, number, number, number] } | null = null;
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
          view.position.set(...at(item, cx, yy));
          view.scale.set(size(item));
          nowShown.add(view);
          drawn.push(item);
          if (!this.shown.has(view)) this.layer.addChild(view);
        }
        if (d === 0) continue;
        // The funnel from each item of the column to the left to the items under it: from its top to the top of the
        // first and from its bottom to the bottom of the last, filled. While the item's fold turns, the whole funnel is
        // drawn in towards its dot with the items under it, its end at the card too.
        const px = x(d - 1) + NODE_W;
        const prev = t.columns[d - 1];
        for (let r = 0; r < prev.length; r += 1) {
          const i = prev[r];
          const kids = t.children[i];
          if (kids.length === 0) continue;
          const [one, end] = [kids[0], kids[kids.length - 1]];
          const py = y(d - 1, r);
          const ph = high(this.base[i]) / 2;
          const [fx, top] = at(one, px, py - ph);
          const [, bottom] = at(one, px, py + ph);
          const [gx, first] = at(one, cx, y(d, t.rank[one]) - high(this.base[one]) / 2);
          const [, last] = at(one, cx, y(d, t.rank[end]) + high(this.base[end]) / 2);
          if (Math.max(bottom, last) < 0 || Math.min(top, first) > height) continue;
          this.funnel(fx, top, bottom, gx, first, last);
          const on = pointer !== null && inFunnel(fx, top, bottom, gx, first, last, pointer.x, pointer.y);
          if (on && this.shownItems[i].fold != null) funnelled = { k: i, shape: [fx, top, bottom, gx, first, last] };
        }
      }
      this.links.fill({ color: PALETTE.other, alpha: PALETTE.otherAlpha * linksStrength(this.dim) });
      // What is under the pointer: a card first; else the dot of an item folded, past its card; else a funnel.
      this.under = this.pointed(t, x, y);
      const dot = this.under >= 0 ? -1 : this.dotPointed(t, x, y);
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
      const said = foldWordsSaid !== '' ? foldWordsSaid : this.levelSaid(this.under);
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
    // How far on the flash of the item selected's soft light is. It ends once its time is over or its item is no longer
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
    // the pointer than for the selection alone, so that the lit items and what joins them stand out; it comes and goes
    // over a moment, so that a pointer passing over the tree does not make it flicker.
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
    // While the search is open, every item is dimmed but those it finds, lit as the items in focus are.
    const searching = this.found !== null;
    const finding = new Set(
      (this.found ?? []).flatMap((id) => (this.real.has(id) ? [this.real.get(id) as number] : [])),
    );
    const aim = focus.size > 0 || chosen.size > 0 || searching ? DIM : 1;
    this.dim = eased(this.dim, aim, step);
    if (this.dim !== aim) easing = true;
    for (const i of drawn) {
      const view = this.views[i];
      const to = aim === 1 || focus.has(i) || chosen.has(i) || finding.has(i) ? 1 : DIM;
      view.alpha = eased(view.alpha, to, step);
      if (view.alpha !== to) easing = true;
    }
    // The way from each item whose work runs up to its root, lit in the colour of a running card's edge, with one
    // length of light running along it from the root's side to the item's: each link from an item to the one above it
    // drawn once, however many running items it is on. While something is lit, the links on its way stand at full
    // strength and the others dim with the items they join; each eases to its strength as the items do, so that none
    // jumps as the light moves or goes out.
    const shining = new Set([...focus, ...chosen, ...finding]);
    // Where a way's strength eases, or a dot's changes, what stands of the ways is built again at it. A root's dot is
    // as strong as the ways from it are lit; another's as the way into its item.
    let strengthened = false;
    for (const [i, to] of wayAims(this.runWays, t.parent, shining, aim !== 1)) {
      const had = this.runStrength.get(i);
      const now = eased(had ?? to, to, step);
      if (now !== had) strengthened = true;
      this.runStrength.set(i, now);
    }
    const dotStrength = (k: number) =>
      t.parent[k] >= 0
        ? (this.runStrength.get(k) ?? 1)
        : aim === 1 || shining.has(k)
          ? 1
          : linksStrength(this.dim);
    const dotsNow = this.litDots.map(dotStrength);
    if (dotsNow.length !== this.dotsBuilt.length || dotsNow.some((v, n) => v !== this.dotsBuilt[n])) {
      strengthened = true;
    }
    const restAgain = placed || strengthened;
    const stroked = (to: Strokes) => [to.line, to.near, to.far];
    if (restAgain) {
      this.dotsBuilt = dotsNow;
      for (const part of this.ways.parts.values()) for (const g of stroked(part.rest)) g.clear();
      for (const kept of [this.waysAtRest, this.endsAtRest]) {
        for (const part of kept.parts.values()) for (const g of stroked(part)) g.clear();
      }
    }
    for (const part of this.ways.parts.values()) {
      if (part.lit) for (const g of stroked(part.light)) g.clear();
      part.lit = false;
    }
    for (const g of stroked(this.runEdges)) g.clear();
    const canvas = this.app.renderer.resolution;
    for (const lit of [this.ways, this.waysAtRest, this.runEdges, this.endsAtRest]) {
      blurTo(lit.nearBlur, RUN.nearBlur, canvas);
      blurTo(lit.farBlur, RUN.farBlur, canvas);
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
    // folds, each with the place across the page the light comes to it at, how it burns now, and how it is
    // laid on what it is drawn on as it burns ([`sparkAt`]). They are drawn once the ways are.
    const wayOrder: number[] = [];
    const meets: { place: number; burn: number; lay: (to: Strokes, burn: number) => void }[] = [];
    for (const [i, strength] of this.runStrength) {
      const p = t.parent[i];
      const d = t.depth[i];
      const { px, py, cx, iy } = ends(i);
      if (cx < 0 || px > width) continue;
      if (Math.max(py, iy) < 0 || Math.min(py, iy) > height) continue;
      const { way, len, dist } = wayDown(i);
      const turns = [turnOf(way, true, RUN.fillet), turnOf(way, false, RUN.fillet)];
      this.waysDrawn.push({ id: this.ids[i], from: [px, py], to: [cx, iy], curve: way.points, turns });
      // How long the way's curve is, and how far along it the place stands that is `across` the page from its start.
      const curve = way.along[RUN.pieces];
      const on = (across: number) => this.arcAt(way, across);
      const lit = (g: Graphics, from: number, to: number, w: number, alpha: number, paint: number, end = false) => {
        this.alongWay(g, way, from, to);
        g.stroke({ color: paint, width: w, alpha: strength * alpha, cap: end ? 'round' : 'butt' });
      };
      const lights = this.wayLights(i);
      wayOrder.push(i);
      lights.place = Math.round(dist);
      const [still, shine] = [lights.rest, lights.light];
      // The way itself, as a wire of light: its glow further out and close in, then the line, drawn in lengths each
      // at the width of its own middle so that it is thinner where it meets the cards. The glow is fainter where the
      // page is light ([`RUN.inkRest`]). Built only where the ways stand anew, and kept; the glow both as the way's
      // own and among the glow kept of the ways that begin where it does.
      if (restAgain) {
        const kept = this.partOf(this.waysAtRest, lights.place);
        for (const g of [still.far, kept.far]) lit(g, 0, curve, RUN.far, RUN.farAlpha * rest, tint(0), true);
        for (const g of [still.near, kept.near]) lit(g, 0, curve, RUN.near, RUN.nearAlpha * rest, tint(0), true);
        for (let n = 0; n < RUN.wide; n += 1) {
          const thick = lineWide * thickAt((n + 0.5) / RUN.wide);
          lit(still.line, (curve * n) / RUN.wide, (curve * (n + 1)) / RUN.wide, thick, lineAlpha, lineColour);
        }
      }
      // The light running down the way: the same line, brighter and wider where the light is, as a wire is where the
      // current is in it. It is a drop drawn out along the way, its head round and widest, thinning away behind it;
      // the head is round only where it stands on this way, and not where the way cuts the light short.
      const drop = lightOn(dist, len, head);
      if (drop.length > 0) lights.lit = true;
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
        const strand = (g: Graphics, from: number, to: number, by: number, bright: number, colour: number) => {
          this.alongWay(g, way, from, to, (at) => side * by * gatherAt(at / curve));
          const alpha = strength * RUN.strandAlpha * own * bright * rest;
          g.stroke({ color: colour, width: RUN.strand, alpha, cap: 'butt' });
        };
        if (restAgain) strand(still.line, 0, curve, RUN.spreadRest, RUN.strandRest, tint(0));
        for (const { from, to, bright } of drop) {
          const comes = Math.min(1, (1 - bright + 1 / RUN.lightPieces) / Math.max(RUN.strandIn, 1e-6));
          strand(shine.line, on(from), on(to), RUN.spread, bright * comes, tint(bright));
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
      const [out, into] = [sparkAt(dist, head), sparkAt(dist + len, head)];
      meets.push(
        {
          place: dist,
          burn: out,
          lay: (to, burn) => this.turnIn(to, px, py, turns[0], tint, lineColour, strength, burn),
        },
        {
          place: dist + len,
          burn: into,
          lay: (to, burn) => this.turnIn(to, cx, iy, turns[1], tint, lineColour, strength, burn),
        },
      );
      if (!sparked.has(p * 2)) {
        sparked.add(p * 2);
        const ph = (high(this.base[p]) / 2) * size(p);
        meets.push({
          place: dist,
          burn: out,
          lay: (to, burn) => this.spark(to, px, py, ph, tint, lineColour, glintColour, strength, burn),
        });
      }
      if (!sparked.has(i * 2 + 1)) {
        sparked.add(i * 2 + 1);
        const ih = (high(this.base[i]) / 2) * size(i);
        meets.push({
          place: dist + len,
          burn: into,
          lay: (to, burn) => this.spark(to, cx, iy, ih, tint, lineColour, glintColour, strength, burn),
        });
      }
      if (t.parent[p] < 0 && !crossed.has(p)) {
        crossed.add(p);
        const over = lightOn(0, NODE_W, head);
        this.across(x(d - 1), py, half(d - 1, t.rank[p]), over, tint, lineColour, glintColour, strength);
      }
      if (!crossed.has(i) && size(i) === 1) {
        crossed.add(i);
        const over = lightOn(dist + len, NODE_W, head);
        this.across(x(d), iy, half(d, t.rank[i]), over, tint, lineColour, glintColour, strength);
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
      if (ness <= 0 || x(d) > width || x(d) + NODE_W + DOT_AT + DOT_R < 0 || ky < -ROW || ky > height + ROW) continue;
      dotted.add(k);
      const root = t.parent[k] < 0;
      const strength = dotStrength(k);
      if (root && !crossed.has(k)) {
        crossed.add(k);
        this.across(x(d), ky, half(d, t.rank[k]), lightOn(0, NODE_W, head), tint, lineColour, glintColour, strength);
      }
      const [ex, ey] = at(k, x(d) + NODE_W, ky);
      const [dx, dy] = at(k, x(d) + NODE_W + DOT_AT, ky);
      const r = DOT_R * size(k) * ness;
      const lay = (to: Strokes, burn: number) => {
        const glow = strength * rest * burn;
        to.line
          .moveTo(ex, ey)
          .lineTo(dx, dy)
          .stroke({ color: lineColour, width: lineWide, alpha: strength * ness * lineAlpha });
        to.far.circle(dx, dy, r * 3).fill({ color: tint(burn), alpha: glow * RUN.farAlpha });
        to.near.circle(dx, dy, r * 1.8).fill({ color: tint(burn), alpha: glow * RUN.nearAlpha });
        to.line.circle(dx, dy, r).fill({ color: lineColour, alpha: strength * lineAlpha });
      };
      meets.push({ place: dotFrom(k), burn: sparkAt(dotFrom(k), head), lay });
    }
    // Each place the light comes to stands at rest until the light is near it, and burns while it is: at rest its
    // lights are those kept, laid once as they rest; burning, all of them there are drawn as they burn at this frame,
    // and what is kept of them there is hidden until they rest again. The places are told apart to the pixel.
    const burning = new Set(meets.filter((m) => m.burn > RUN.portRest).map((m) => Math.round(m.place)));
    const built = new Set<number>();
    for (const { place, burn, lay } of meets) {
      const key = Math.round(place);
      if (restAgain) {
        lay(this.partOf(this.endsAtRest, key), RUN.portRest);
        built.add(key);
      }
      if (burning.has(key)) lay(this.runEdges, burn);
    }
    for (const [key, part] of this.endsAtRest.parts) {
      if (restAgain && !built.has(key)) this.dropPart(this.endsAtRest, key);
      else part.line.visible = part.glow.visible = !burning.has(key);
    }
    // The ways that begin at the same place across the page are lit together, as the light comes to that place: while
    // it is on any of them, the glow of each is drawn and blurred at each frame with the light's on it, and the glow
    // kept of them is hidden; while it is on none, only that kept is drawn. A layer drawn empty is hidden too, as the
    // blur would else reach out to take it in; and while the light is on no way, the glow of the ways is hidden whole,
    // so that nothing is blurred for it.
    const drawnLights = wayOrder.map((i) => this.wayLights(i));
    const litPlaces = new Set(drawnLights.flatMap((w) => (w.lit ? [w.place] : [])));
    for (const w of drawnLights) {
      w.rest.far.visible = w.rest.near.visible = litPlaces.has(w.place);
      for (const g of stroked(w.light)) g.visible = w.lit;
    }
    this.ways.far.renderable = this.ways.near.renderable = litPlaces.size > 0;
    // The glow over the cards is blurred only in a frame that lights an edge or burns where a way meets a card.
    this.runEdges.far.renderable = drawsAny(this.runEdges.far);
    this.runEdges.near.renderable = drawsAny(this.runEdges.near);
    const placesDrawn = new Set(drawnLights.map((w) => w.place));
    for (const [key, part] of this.waysAtRest.parts) {
      if (restAgain && !placesDrawn.has(key)) this.dropPart(this.waysAtRest, key);
      else part.glow.visible = !litPlaces.has(key);
    }
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
      for (const kept of [this.waysAtRest, this.endsAtRest]) {
        for (const part of kept.parts.values()) this.bake(part, width, height);
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
    ease(this.softLit, new Set(this.under < 0 ? [] : [this.under]));
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
      // its ribbon, ring or soft light, which wait until it is drawn whole.
      const isWhole = (i: number) => this.scaleOf[i] === 1;
      for (const i of chosen) {
        const p = t.parent[i];
        if (p < 0 || !isWhole(i)) continue;
        const d = t.depth[i];
        const [py, ph, iy, ih] = [y(d - 1, t.rank[p]), half(d - 1, t.rank[p]), y(d, t.rank[i]), half(d, t.rank[i])];
        this.funnel(x(d - 1) + NODE_W, py - ph, py + ph, x(d), iy - ih, iy + ih);
      }
      if (chosen.size > 1) this.links.fill(PALETTE.chosenRibbon);
      for (const [i, a] of this.glowing) {
        const p = t.parent[i];
        if (p < 0 || !isWhole(i)) continue;
        const d = t.depth[i];
        const [py, ph, iy, ih] = [y(d - 1, t.rank[p]), half(d - 1, t.rank[p]), y(d, t.rank[i]), half(d, t.rank[i])];
        this.funnel(x(d - 1) + NODE_W, py - ph, py + ph, x(d), iy - ih, iy + ih);
        this.links.fill({ color: PALETTE.hoverRibbon, alpha: a });
      }
      // The soft lights, each in its item's own colour: behind the item selected, wider and stronger as far as its
      // flash is on, and behind the item under the pointer, as far as it has come in; over them, the page's ground
      // where [`grounded`] says, so that the light shows only round the cards and never through one drawn faint.
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
      this.chosenDrawn = null;
      if (picked >= 0 && isWhole(picked)) {
        const out = CHOSEN_SPREAD + (FLASH_SPREAD - CHOSEN_SPREAD) * flare;
        const on = 1 + (1 / SOFT_ALPHA - 1) * flare;
        softLight(this.chosenGlow, picked, out, CHOSEN_BLUR, this.own[picked], on);
        this.chosenDrawn = { id: this.ids[picked], out, alpha: SOFT_ALPHA * on };
      }
      for (const [i, a] of this.softLit)
        if (isWhole(i)) softLight(this.hoverGlow, i, SOFT_SPREAD, SOFT_BLUR, this.own[i], a);
      // A soft light is blurred only while it lights an item.
      this.chosenGlow.renderable = drawsAny(this.chosenGlow);
      this.hoverGlow.renderable = drawsAny(this.hoverGlow);
      this.glowGround.clear();
      if (lights.length > 0) {
        for (const j of drawn) {
          if (!isWhole(j)) continue;
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
        if (!isWhole(i)) continue;
        const d = t.depth[i];
        const [gx, gy, gh] = [x(d), y(d, t.rank[i]), half(d, t.rank[i])];
        this.marks
          .roundRect(gx, gy - gh, NODE_W, 2 * gh, CARD_CORNER)
          .stroke({ color: this.own[i], width: RING, alpha: a });
      }
      for (const i of chosen) {
        if (!isWhole(i)) continue;
        const d = t.depth[i];
        const [gx, gy, gh] = [x(d), y(d, t.rank[i]), half(d, t.rank[i])];
        const ring = i === picked ? CHOSEN_RING : RING;
        this.marks.roundRect(gx, gy - gh, NODE_W, 2 * gh, CARD_CORNER).stroke({ color: this.own[i], width: ring });
      }
      // The columns fade into the page at the top and the bottom; at those edges the column under the pointer has a
      // line of blue.
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
      for (const v of this.shown) if (!this.nowShown.has(v)) this.layer.removeChild(v);
      this.shown = this.nowShown;
      this.stats.drawn = this.nowShown.size;
    }
    // The marks of cells running pulse, and each work's age is kept up, at every frame.
    for (const i of drawn) {
      const p = this.pulses.get(i);
      if (p !== undefined) p.alpha = pulse;
      this.age(i);
    }
    this.easing = easing;
    this.dirty.clear();
    this.drawnLeft = this.left;
    this.drawnCols = this.columns.map((c) => c.at);
    this.stats.blurs = this.blurred.filter((layer) => layer.visible && layer.renderable).length;
  }

  /// The layers blurred at each frame they draw something in: the glow of the lit ways further out and close in, the
  /// same over the cards, and the soft lights round the item under the pointer and the item selected.
  private get blurred(): Container[] {
    return [this.ways.far, this.ways.near, this.runEdges.far, this.runEdges.near, this.hoverGlow, this.chosenGlow];
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

  /// The part of the lights `kept` kept by `key`, made empty where there is none yet ([`Kept`]).
  private partOf(kept: Kept, key: number): Rest {
    const had = kept.parts.get(key);
    if (had !== undefined) return had;
    const near = new Graphics({ filters: [kept.nearBlur] });
    const far = new Graphics({ filters: [kept.farBlur] });
    const source = new Container();
    source.addChild(far, near);
    const part: Rest = { line: new Graphics(), near, far, source, glow: new Sprite(), texture: null };
    kept.lines.addChild(part.line);
    kept.glows.addChild(part.glow);
    kept.parts.set(key, part);
    return part;
  }

  /// Lets go of the part of the lights `kept` kept by `key`, with its texture.
  private dropPart(kept: Kept, key: number) {
    const part = kept.parts.get(key);
    if (part === undefined) return;
    kept.parts.delete(key);
    part.line.destroy();
    part.glow.destroy();
    part.source.destroy({ children: true });
    part.texture?.destroy(true);
  }

  /// Lays `part`'s glow, blurred as a layer of light's is, on a texture of its own, which its sprite draws where it
  /// stands until the part is built again ([`draw`]): the glow's blur laid once, and not at every frame. The texture
  /// takes what the glow covers and the room its blur reaches past that, within the screen, as far as a glow drawn on
  /// the page is blurred; a part with no glow draws none.
  private bake(part: Rest, width: number, height: number) {
    const room = BLUR_ROOM * Math.max(RUN.nearBlur, RUN.farBlur);
    // What the glow covers, of each of its layers that has anything on it.
    const b = { minX: Infinity, minY: Infinity, maxX: -Infinity, maxY: -Infinity };
    for (const { bounds } of [part.far, part.near]) {
      if (bounds.width <= 0 || bounds.height <= 0) continue;
      b.minX = Math.min(b.minX, bounds.minX);
      b.minY = Math.min(b.minY, bounds.minY);
      b.maxX = Math.max(b.maxX, bounds.maxX);
      b.maxY = Math.max(b.maxY, bounds.maxY);
    }
    const [x0, y0] = [Math.max(0, Math.floor(b.minX - room)), Math.max(0, Math.floor(b.minY - room))];
    const [x1, y1] = [Math.min(width, Math.ceil(b.maxX + room)), Math.min(height, Math.ceil(b.maxY + room))];
    const shows = x1 > x0 && y1 > y0;
    part.glow.renderable = shows;
    if (!shows) return;
    // A texture is kept while the glow fits in it with less than two steps to spare each way, so that a glow moving
    // across the edge of the screen, its size changing at every frame, does not make one at every frame. It is as
    // fine as the finer of the glow's two blurs, as nothing finer is on it ([`BLUR_TEXEL`]).
    const [w, h] = [x1 - x0, y1 - y0];
    const resolution = glowResolution(Math.min(RUN.nearBlur, RUN.farBlur), this.app.renderer.resolution);
    const fits = (has: number, needs: number) => has >= needs && has - needs < 2 * TEXTURE_STEP;
    let texture = part.texture;
    if (
      texture === null ||
      !fits(texture.width, w) ||
      !fits(texture.height, h) ||
      texture.source.resolution !== resolution
    ) {
      texture?.destroy(true);
      const step = (n: number) => Math.ceil(n / TEXTURE_STEP) * TEXTURE_STEP;
      texture = RenderTexture.create({ width: step(w), height: step(h), resolution });
      part.texture = texture;
      part.glow.texture = texture;
    }
    part.glow.position.set(x0, y0);
    this.app.renderer.render({
      container: part.source,
      target: texture,
      clear: true,
      clearColor: [0, 0, 0, 0],
      transform: new Matrix().translate(-x0, -y0),
    });
  }
}
