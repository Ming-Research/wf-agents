// The console in a real browser (Chromium, through Vitest's browser mode): the tree is drawn by WebGL, which happy-dom
// does not have, and its motion is the frames the browser draws. These pin what the console does: a scroll moves the
// whole tree up or down and rests where the scroll stops, as far as leaves three rows on the screen; a sideways scroll
// or a drag moves it sideways, no further than leaves half of the first or the last column on the screen; the wheel
// with Shift held, or `zoomAt`, makes it larger or smaller round a point of the canvas, which stays on the same point of
// the tree; Space fits the whole tree to the room it is given and goes back, and the grid of every project's trees shows
// each whole at once and gives back the view it had; a click selects the item under the pointer, and a click on a
// shadow the item it points to, brought to the middle; the tree drawn again stands where it stood, the item selected
// kept, and while folds turn the item in view stays where it stands on the screen; an item revealed comes into the room
// the panel leaves, or to the middle when it is above or below what is seen; the item the pointer comes onto lights at
// once; the map is one size for every tree and frames the part the screen shows; the view stands through a change of
// height or theme; and once destroyed, a scene listens to nothing and leaves the others drawing. The pointer and the
// wheel are events sent to the canvas at the points the test works out from where the cards stand (`cardAt`, `place`),
// so that each lands where the test means; the keys are the browser's own. They pin as well: a click on an item's
// funnel folds what is under it into a dot and a click on the dot shows it again, both selecting nothing, the fold
// turning over its time; the console keeps that choice over its own rule; a lit way turns into the cards it joins along
// its own curve; the way to a work the engine runs a call for is lit, as to one whose round runs; and the item selected
// flashes once when asked, as a click on its title in the panel asks, and then draws nothing that stands again while
// only the lights move.

import '../styles/tokens.css';
import '../styles/parts.css';

import { createRoot } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { page as browserPage, userEvent } from 'vitest/browser';

import { DEFAULT_ACCENT, pickAccent, PRESETS, tones } from '../accent';
import { apart, lchOf } from '../colour';
import { MARKS } from '../marks';
import { useBarItems } from '../Engine';
import { NO_LEVEL, STATE } from '../works/words';
import type { CellNow, Changed, Items, Now } from '../works-api';
import { FIXTURES, type Source as PageSource, SourceContext } from '../works/source';
import { fixtureSource } from './fixtures';
import { FIND_SAYS } from './find';
import { CONSOLE_MARKS } from './help';
import { ConsolePage, emptyWords } from './index';
import {
  foldable,
  quietFolds,
  shownTree,
  STANDINGS,
  type Shown,
  type ShownWork,
  standingOf,
} from './items';
import {
  DIM,
  DOT_AT,
  FLASH_MS,
  FOLD_MS,
  KEEP,
  NODE_W,
  PITCH,
  ROW,
  RUN,
  SnapScene,
  themed,
  UNFOLD_MS,
} from './snap';
import { TRACK_KINDS, TRACK_WORDS } from './track';

const WIDTH = 1000;
const HEIGHT = 640;

/// An item with no work, and the items under it.
const card = (id: number, children: Shown[] = []): Shown => ({
  id,
  title: `Item ${id}`,
  fresh: 'none',
  repliesUnread: 0,
  standing: 'open',
  asks: 0,
  ahead: 0,
  deliver: false,
  begun: false,
  underWay: false,
  shadow: false,
  stands: null,
  work: null,
  children,
});
const shadow = (id: number): Shown => ({ ...card(id), shadow: true });
/// An item whose work is `work`, standing as that work does, as the console makes it from the engine's items.
const worked = (id: number, work: ShownWork): Shown => ({
  ...card(id),
  begun: true,
  stands: { now: work.now, cells: work.cells, waits: work.waits },
  work,
});
/// The work numbered `number`, running its implement cell since now.
const running = (number: number): ShownWork => {
  const now = Date.now();
  return {
    number,
    now: { kind: 'round', cell: 'implement', run: 1, round: 1, since: now },
    paused: null,
    cells: [{ cell: 'implement', state: 'running', finished: 0, record: null, open: 1 }],
    waits: [],
    from: now,
    bar: '',
    sentBack: 0,
    usdMicros: 0,
    last: null,
    level: '2',
    raise: 0,
    cellLevel: '2',
    held: null,
    lines: null,
  };
};

const frame = () => new Promise<void>((done) => requestAnimationFrame(() => done()));
/// Two frames: one in which the scene has surely drawn, whichever of its frame and the test's came first.
const drew = async () => {
  await frame();
  await frame();
};

/// Waits until the tree stands still: three frames in a row in which nothing moved and no fold turned, which it reaches
/// within a second of frames of what moved it. The bound is in frames, which the scene moves by, not in time, which a
/// busy machine stretches.
function settle(scene: SnapScene) {
  let was = JSON.stringify(scene.place());
  let still = 0;
  for (let k = 0; k < 60 && still < 3; k += 1) {
    scene.step();
    const now = JSON.stringify(scene.place());
    still = now === was && scene.place().turns === 0 ? still + 1 : 0;
    was = now;
  }
  expect(still, 'the tree still moved 60 frames on').toBe(3);
}

/// Where the card of item `id` stands on the canvas, its left side and its middle, drawn as large as `size` of its full
/// size; it fails where the tree does not draw it.
function where(scene: SnapScene, id: number): { x: number; y: number; size: number } {
  const at = scene.cardAt(id);
  if (at === null) throw new Error(`item ${id} is not drawn`);
  return at;
}

describe('the tree', () => {
  let host: HTMLDivElement;
  let scene: SnapScene;
  beforeEach(async () => {
    host = document.createElement('div');
    host.style.cssText = `position: fixed; left: 0; top: 0; width: ${WIDTH}px; height: ${HEIGHT}px`;
    document.body.append(host);
    scene = new SnapScene();
    await scene.mount(host);
  });
  afterEach(() => {
    scene.destroy();
    host.remove();
    document.documentElement.removeAttribute('data-theme');
  });

  const canvas = () => host.querySelector('canvas') as HTMLCanvasElement;
  const at = (x: number, y: number): PointerEventInit => {
    const r = canvas().getBoundingClientRect();
    return { clientX: r.left + x, clientY: r.top + y, bubbles: true, pointerId: 1 };
  };
  const move = (x: number, y: number) => canvas().dispatchEvent(new PointerEvent('pointermove', at(x, y)));
  const wheel = (deltaX: number, deltaY: number) =>
    canvas().dispatchEvent(new WheelEvent('wheel', { deltaX, deltaY, bubbles: true, cancelable: true }));
  /// A click where the pointer is, after a frame that finds the item under it, drawn by the scene's own `step`: the
  /// browser gives a frame when it will, and a click that waited for one would wait on the machine's load.
  function click(x: number, y: number) {
    move(x, y);
    scene.step();
    canvas().dispatchEvent(new PointerEvent('pointerdown', at(x, y)));
    canvas().dispatchEvent(new PointerEvent('pointerup', at(x, y)));
  }
  /// The middle of the screen, where each column's place is.
  const line = HEIGHT / 2;

  it('moves the whole tree up and down with the scroll, over any column, and no further than leaves three of its rows on the screen', async () => {
    // Thirty roots, the first with an item under it: two columns.
    scene.show(Array.from({ length: 30 }, (_, k) => card(k + 1, k === 0 ? [card(100)] : [])));
    // At first the middle root stands in the middle: rank 14, its middle 14.5 rows from the tree's top.
    expect(scene.place().at).toBe(14.5);
    const [root, under] = [where(scene, 1).y, where(scene, 100).y];
    // A scroll over the second column moves both columns as one.
    move(scene.place().left + PITCH + 50, line);
    wheel(0, 3 * ROW);
    settle(scene);
    expect(scene.place().at).toBeCloseTo(17.5, 6);
    expect(where(scene, 1).y).toBeCloseTo(root - 3 * ROW, 6);
    expect(where(scene, 100).y).toBeCloseTo(under - 3 * ROW, 6);
    // As far down as it goes, the last row of the tree ends three rows from the top of the screen; as far up, the first
    // begins three rows from its bottom.
    wheel(0, 100_000);
    settle(scene);
    expect(where(scene, 30).y + ROW / 2).toBeCloseTo(KEEP * ROW, 6);
    wheel(0, -100_000);
    settle(scene);
    expect(where(scene, 1).y - ROW / 2).toBeCloseTo(HEIGHT - KEEP * ROW, 6);
  });

  it('moves the tree sideways by a sideways scroll, and every way by a drag, as far as leaves half of the first or the last column on the screen', async () => {
    scene.show([card(1, [card(2, [card(3)])])]);
    expect(scene.place().left).toBe(40);
    move(50, line);
    wheel(100, 0);
    await drew();
    expect(scene.place().left).toBe(-60);
    wheel(100_000, 0);
    await drew();
    expect(scene.place().left).toBe(-2 * PITCH - NODE_W / 2);
    // A drag: the press on the canvas, the moves anywhere in the window; the tree goes with the pointer, across and
    // down.
    const down = where(scene, 3).y;
    canvas().dispatchEvent(new PointerEvent('pointerdown', at(500, line)));
    window.dispatchEvent(new PointerEvent('pointermove', at(700, line + 2 * ROW)));
    canvas().dispatchEvent(new PointerEvent('pointerup', at(700, line + 2 * ROW)));
    await drew();
    expect(scene.place().left).toBe(-2 * PITCH - NODE_W / 2 + 200);
    expect(where(scene, 3).y).toBeCloseTo(down + 2 * ROW, 6);
    wheel(-100_000, 0);
    await drew();
    expect(scene.place().left).toBe(WIDTH - NODE_W / 2);
  });

  it('selects the item clicked and tells it; lets it go on a click on it again or on no item', async () => {
    const told = vi.fn<(item: Shown | null) => void>();
    scene.onSelect = told;
    scene.show([card(1), card(2), card(3)]);
    settle(scene);
    // Item 2 stands in the middle; item 1 a row above it.
    const x = scene.place().left + 100;
    click(x, line);
    expect(told).toHaveBeenLastCalledWith(expect.objectContaining({ id: 2 }));
    click(x, line);
    expect(told).toHaveBeenLastCalledWith(null);
    click(x, line - ROW);
    expect(told).toHaveBeenLastCalledWith(expect.objectContaining({ id: 1 }));
    click(x, HEIGHT - 20);
    expect(told).toHaveBeenLastCalledWith(null);
  });

  it('says on pointing at the level on a card what the item window says of it, in every case, and turns the pointer to no hand over it', async () => {
    const change = (id: number, work: Partial<ShownWork>): Shown => worked(id, { ...running(id), ...work });
    // The card's own middle stands at the screen's, with the level in its second row, under the number.
    const said = (item: Shown) => {
      scene.show([item]);
      settle(scene);
      const left = scene.place().left;
      // Off every card first, so that nothing said for the last card stands.
      move(WIDTH - 10, HEIGHT - 10);
      scene.step();
      expect(canvas().title).toBe('');
      for (let y = line - ROW; y <= line + ROW; y += 2) {
        for (let x = left + 4; x < left + 80; x += 4) {
          move(x, y);
          scene.step();
          if (canvas().title !== '') return { title: canvas().title, cursor: canvas().style.cursor };
        }
      }
      return { title: '', cursor: canvas().style.cursor };
    };
    // Held by the highest level a round may run at: written at the level it runs at, and what held it said.
    expect(said(change(1, { level: '3', raise: 0, cellLevel: '3', held: { asks: 1, by: 'agents.highest_level' } }))).toEqual({
      title: 'level 3, levels_above 1, agents.highest_level 3',
      cursor: '',
    });
    // Raised, and held by nothing: its keys alone.
    expect(said(change(2, { level: '3', raise: 1, cellLevel: '4', held: null }))).toEqual({
      title: 'level 3, levels_above 1',
      cursor: '',
    });
    expect(said(change(3, { level: '2', raise: 0, cellLevel: '2', held: null })).title).toBe('level 2');
    // A work whose engine gives no level: a dash, and why there is none.
    expect(said(change(4, { level: null, raise: 0, cellLevel: null, held: null })).title).toBe(NO_LEVEL.level);
  });

  // A pause and a wait stand in one grey, neither needing the owner, so the card says which on pointing at it, in the
  // engine's words for the state: the cell, and where a pause holds a round to be given again, that it is given once the
  // pause is lifted. A work that has run no round is drawn as a plain card and says its state too.
  it("says on pointing at a card how its work stands in the engine's words, and draws a pause and a wait in one grey", async () => {
    const failed: CellNow = { cell: 'implement', state: 'failed', finished: 0, record: null, open: 1 };
    const resting: CellNow = { ...failed, state: 'resting' };
    const paused: Now = { kind: 'paused', ran: true, at: 'implement', by: 'owner', why: null };
    const waits: Now = { kind: 'waits', at: 'implement', on: [{ kind: 'items', items: [9] }] };
    const never: Now = { kind: 'paused', ran: false, at: 'plan', by: 'owner', why: null };
    const standing = (id: number, now: Now, cells: CellNow[]): Shown => {
      const work = { ...running(id), now, cells, paused: now.kind === 'paused' ? ('owner' as const) : null };
      return { ...card(id), begun: true, stands: { now, cells, waits: [] }, work };
    };
    const left = () => scene.place().left;
    const said = (item: Shown) => {
      scene.show([item]);
      settle(scene);
      move(WIDTH - 10, HEIGHT - 10);
      scene.step();
      move(left() + 100, line);
      scene.step();
      return canvas().title;
    };
    expect(said(standing(1, paused, [failed]))).toBe('Paused · implement · its last round is to be given once the pause is lifted');
    expect(said(standing(2, paused, [resting]))).toBe('Paused · implement');
    expect(said(standing(3, waits, [resting]))).toBe(`${STATE.items} · implement`);
    // Never run, it is a plain card of one row with no work drawn, and says Not started as the engine does.
    expect(said({ ...card(4), stands: { now: never, cells: [], waits: [] } })).toBe('Not started · plan');
    // A shadow, and an item with no work, say nothing.
    expect(said(shadow(5))).toBe('');
    expect(said(card(6))).toBe('');
    // The pause and the wait are drawn on the map in one grey.
    scene.show([standing(1, paused, [resting]), standing(3, waits, [resting])]);
    settle(scene);
    const colour = new Map(scene.drawnMap().pieces.map((p) => [p.id, p.colour]));
    expect(colour.get(1)).toBe(themed().paused);
    expect(colour.get(3)).toBe(themed().paused);
  });

  it("writes the number of each card, a shadow's and a deliverable's too, as #n, and starts every title past the widest number the whole tree holds, folded items included", () => {
    const under = { open: 1, done: 0, cancelled: 0, elsewhere: 0 };
    // 445, the only number of three digits, stands under item 5, which the tree can fold away.
    const tree = (folded: boolean) => [
      { ...card(5, [card(445)]), fold: { folded, under } },
      card(57),
      { ...card(7), deliver: true },
      card(9, [shadow(57)]),
    ];
    const texts = () => scene.drawnNumbers().map((n) => `${n.text}${n.shadow ? ' (shadow)' : ''}`);
    const titles = () => new Set(scene.drawnNumbers().map((n) => n.titleAt));
    scene.show(tree(false));
    settle(scene);
    expect(texts()).toEqual(['#5', '#445', '#57', '#7', '#9', '#57 (shadow)']);
    expect(titles().size).toBe(1);
    const [column] = titles();
    // Folded away, the only card of three digits is not drawn, and no title moves for it.
    scene.show(tree(true));
    for (let k = 0; k <= FOLD_MS / 16; k += 1) scene.step();
    expect(texts()).toEqual(['#5', '#57', '#7', '#9', '#57 (shadow)']);
    expect([...titles()]).toEqual([column]);
    // Shown again, the cards that stayed are kept, and the shadow's number is still said.
    scene.show(tree(false));
    settle(scene);
    expect(texts()).toEqual(['#5', '#445', '#57', '#7', '#9', '#57 (shadow)']);
    expect([...titles()]).toEqual([column]);
    // A tree of numbers of two digits at most has a narrower column.
    scene.show([card(5), card(57), shadow(57)]);
    settle(scene);
    expect(texts()).toEqual(['#5', '#57', '#57 (shadow)']);
    expect(titles().size).toBe(1);
    expect([...titles()][0]).toBeLessThan(column);
  });

  it('folds the items under a card at a click on its funnel and shows them at a click on the dot they fold into, selecting nothing, each fold turning over its time', async () => {
    const told = vi.fn<(item: Shown | null) => void>();
    const turned = vi.fn<(id: number, folded: boolean) => void>();
    scene.onFold = turned;
    const under = { open: 1, done: 0, cancelled: 0, elsewhere: 0 };
    const tree = (folded: boolean) => [{ ...card(1, [card(2)]), fold: { folded, under } }];
    scene.show(tree(false));
    expect(scene.stats.items).toBe(2);
    settle(scene);
    scene.onSelect = told;
    // Item 1 stands in the middle of the first column and item 2 level with it in the second: the funnel between them
    // is at the middle of the screen, all the way between the columns.
    const left = scene.place().left;
    click(left + NODE_W + (PITCH - NODE_W) / 2, line);
    expect(turned).toHaveBeenLastCalledWith(1, true);
    expect(told).not.toHaveBeenCalled();
    expect(canvas().title).toBe('Fold the items under it away (e): open 1');
    expect(canvas().style.cursor).toBe('pointer');
    // Folded, what is under it draws in to the dot over the fold's time, 180 ms, gone at the 12th frame of 16 ms.
    scene.show(tree(true));
    expect(scene.place().turns).toBe(1);
    expect(scene.stats.items).toBe(2);
    for (let k = 0; k <= FOLD_MS / 16; k += 1) scene.step();
    expect(scene.place().turns).toBe(0);
    expect(scene.stats.items).toBe(1);
    // The dot just past the card's edge is the fold now: a click on it shows what is under it again, at once, growing
    // out of the dot over the fold's time.
    told.mockClear();
    click(left + NODE_W + DOT_AT, line);
    expect(turned).toHaveBeenLastCalledWith(1, false);
    expect(told).not.toHaveBeenCalled();
    expect(canvas().title).toBe('Show the items under it (e): open 1');
    scene.show(tree(false));
    expect(scene.stats.items).toBe(2);
    expect(scene.place().turns).toBe(1);
    settle(scene);
    expect(scene.place().turns).toBe(0);
    // A click on the card selects it and folds nothing; past the edge of an item with nothing under it, nothing folds.
    click(left + 100, line);
    expect(told).toHaveBeenLastCalledWith(expect.objectContaining({ id: 1 }));
    scene.show([card(1)]);
    settle(scene);
    click(left + NODE_W + DOT_AT, line);
    expect(turned).toHaveBeenCalledTimes(2);
    expect(canvas().title).toBe('');
  });

  it('folds a card with shadows alone under it at a click on their funnel, as it folds items, counting them apart', async () => {
    const turned = vi.fn<(id: number, folded: boolean) => void>();
    scene.onFold = turned;
    const under = { open: 0, done: 0, cancelled: 0, elsewhere: 1 };
    // Under 1, the shadow of 3 alone; under 2, item 3.
    const tree = (folded: boolean) => [{ ...card(1, [shadow(3)]), fold: { folded, under } }, card(2, [card(3)])];
    scene.show(tree(false));
    expect(scene.stats.items).toBe(4);
    settle(scene);
    // Item 1 stands in the middle of the first column: its funnel leaves its right edge there.
    const left = scene.place().left;
    click(left + NODE_W + 4, line);
    expect(turned).toHaveBeenLastCalledWith(1, true);
    expect(canvas().title).toBe('Fold the items under it away (e): 1 shown elsewhere');
    scene.show(tree(true));
    settle(scene);
    expect(scene.stats.items).toBe(3);
    click(left + NODE_W + DOT_AT, line);
    expect(turned).toHaveBeenLastCalledWith(1, false);
    expect(canvas().title).toBe('Show the items under it (e): 1 shown elsewhere');
  });

  it('draws the items under a fold drawing in to its dot frame by frame at the page rate: smaller from the first frame, and still drawn at half its time', () => {
    const under = { open: 1, done: 0, cancelled: 0, elsewhere: 0 };
    const tree = (folded: boolean) => [{ ...card(1, [card(2)]), fold: { folded, under } }];
    scene.show(tree(false));
    settle(scene);
    // How large item 2 is drawn at each frame of the fold, 16 ms apart, but the last, once the fold is shown so.
    const frames = FOLD_MS / 16;
    scene.show(tree(true));
    const folding: number[] = [];
    for (let k = 1; k < frames; k += 1) {
      scene.step(16);
      folding.push(scene.cardAt(2)?.size ?? 0);
    }
    // It moves in its first frame, by well under a quarter of its size; it is smaller at every frame after; and
    // half way through it is still drawn.
    expect(folding[0]).toBeLessThan(0.95);
    expect(folding[0]).toBeGreaterThan(0.75);
    for (let k = 1; k < folding.length; k += 1) expect(folding[k]).toBeLessThan(folding[k - 1]);
    expect(folding[Math.floor(frames / 2) - 1]).toBeGreaterThan(0.2);
    settle(scene);
    expect(scene.cardAt(2)).toBeNull();
  });

  it('draws the lit way into a running item under a turning fold with its card, ending where the card is drawn at each frame', () => {
    const under = { open: 1, done: 0, cancelled: 0, elsewhere: 0 };
    // Under 1, item 2, whose work runs.
    const tree = (folded: boolean) => [
      { ...card(1, [worked(2, running(2))]), fold: { folded, under } },
    ];
    /// The way into item 2 is lit, and ends where item 2 is drawn, smaller than whole.
    const endsAtCard = () => {
      const way = scene.litWays().find((w) => w.id === 2);
      const stands = scene.cardAt(2);
      expect(stands?.size).toBeGreaterThan(0);
      expect(stands?.size).toBeLessThan(1);
      expect(way?.to[0]).toBeCloseTo(stands?.drawn[0] ?? NaN, 6);
      expect(way?.to[1]).toBeCloseTo(stands?.drawn[1] ?? NaN, 6);
    };
    scene.show(tree(true));
    settle(scene);
    // Half way through an unfold, and then half way through a fold.
    scene.show(tree(false));
    for (let k = 0; k < UNFOLD_MS / 32; k += 1) scene.step(16);
    endsAtCard();
    settle(scene);
    scene.show(tree(true));
    for (let k = 0; k < FOLD_MS / 32; k += 1) scene.step(16);
    endsAtCard();
  });

  it('makes nothing again for a tree that draws the same, as a new read of the items gives it, and of one changed only the card that changed', () => {
    const tree = (title: string) => [card(1, [card(2), { ...card(3), title }, card(4)])];
    scene.show(tree('Item 3'));
    expect(scene.stats.made).toBe(4);
    settle(scene);
    const place = JSON.stringify(scene.place());
    const redraws = scene.stats.redraws;
    // The same tree, read again: no card is made, and the next frame draws nothing of what stands again.
    scene.show(structuredClone(tree('Item 3')));
    expect(scene.stats.made).toBe(0);
    scene.step();
    expect(scene.stats.redraws).toBe(redraws);
    expect(JSON.stringify(scene.place())).toBe(place);
    // Item 3 retitled: its card alone is made again, and the others are kept as they were.
    scene.show(tree('A new title'));
    expect(scene.stats.made).toBe(1);
    expect(scene.stats.items).toBe(4);
  });

  it('draws a frame in which only the lights move without drawing the cards, the funnels or the map again', () => {
    scene.show([card(1, [worked(2, running(2))])]);
    settle(scene);
    scene.step();
    const redraws = scene.stats.redraws;
    // The light runs down the way into item 2 frame after frame; nothing else moves.
    for (let k = 0; k < 10; k += 1) {
      scene.step(50);
      expect(scene.litWays().map((w) => w.id)).toEqual([2]);
    }
    expect(scene.stats.redraws).toBe(redraws);
    // The pointer coming over the tree draws what stands again.
    move(50, line);
    scene.step();
    expect(scene.stats.redraws).toBe(redraws + 1);
  });

  it('flashes the light along the edges of the item selected once, in view, without moving the tree, and once it has come to rest draws nothing that stands again while only the lights move', () => {
    scene.show([card(1, [worked(2, running(2))])]);
    scene.select(2);
    settle(scene);
    // Frames a second long bring the dimming of the items not selected, and all else that eases, to rest.
    for (let k = 0; k < 3; k += 1) scene.step(1000);
    const place = JSON.stringify(scene.place());
    const rest = scene.chosenLight() as { id: number; out: number; alpha: number };
    expect(rest.id).toBe(2);
    let redraws = scene.stats.redraws;
    scene.step(50);
    expect(scene.stats.redraws).toBe(redraws);
    // Item 2 is in view: brought into view, it stays where it is, and its light as it was.
    scene.reveal(2, WIDTH);
    scene.step();
    expect(JSON.stringify(scene.place())).toBe(place);
    expect(scene.chosenLight()).toEqual(rest);
    // Flashed, its light is stronger from the next frame, the flash well on at its height, and back at rest by the end
    // of the flash's time, the tree unmoved.
    scene.flash(2);
    const lights: { out: number; alpha: number }[] = [];
    for (let k = 0; k < FLASH_MS / 16; k += 1) {
      scene.step(16);
      lights.push(scene.chosenLight() as { out: number; alpha: number });
    }
    expect(lights[0].out).toBeGreaterThan(rest.out);
    expect(lights[0].alpha).toBeGreaterThan(rest.alpha);
    expect(Math.max(...lights.map((l) => l.out))).toBeGreaterThan(2 * rest.out);
    expect(scene.chosenLight()).toEqual(rest);
    expect(JSON.stringify(scene.place())).toBe(place);
    // Then the light runs down the way into item 2 frame after frame, and nothing that stands is drawn again.
    redraws = scene.stats.redraws;
    for (let k = 0; k < 10; k += 1) {
      scene.step(50);
      expect(scene.litWays().map((w) => w.id)).toEqual([2]);
    }
    expect(scene.stats.redraws).toBe(redraws);
  });

  it('moves only the light down a way from frame to frame, what stands of the way built once where it stands, and again only where the screen moves past the room it was built for', () => {
    scene.show([card(1, [worked(2, running(2))])]);
    settle(scene);
    scene.step();
    const builds = scene.stats.wayBuilds;
    // The light runs down the way into item 2 frame after frame, a whole run and the dark after it, burning at each
    // card's end as it passes: the way's line, glow and strands, and the lights at rest at the cards' ends, are not
    // built again.
    for (let k = 0; k < (RUN.beat + RUN.dark) / 50 + 2; k += 1) {
      scene.step(50);
      expect(scene.litWays().map((w) => w.id)).toEqual([2]);
    }
    expect(scene.stats.wayBuilds).toBe(builds);
    // Moved sideways a little, the tree is moved whole as it was drawn, and nothing is built again.
    wheel(100, 0);
    scene.step();
    expect(scene.stats.wayBuilds).toBe(builds);
    // Moved sideways as far as it goes, past the half screen more it was drawn for, the way is built again where it
    // now stands.
    wheel(-5000, 0);
    scene.step();
    expect(scene.stats.wayBuilds).toBe(builds + 1);
    expect(scene.litWays().map((w) => w.id)).toEqual([2]);
    // A knob that changes a number the ways are drawn by has them built again at the next frame.
    const wide = RUN.far;
    RUN.far = wide + 1;
    scene.step();
    RUN.far = wide;
    expect(scene.stats.wayBuilds).toBe(builds + 2);
  });

  it('blurs a layer of glow only while it draws something', () => {
    scene.show([card(1, [worked(2, running(2))])]);
    settle(scene);
    /// How many layers each frame blurs over a whole run of the light and the dark after it.
    const run = () =>
      Array.from({ length: Math.ceil((RUN.beat + RUN.dark) / 16) }, () => {
        scene.step(16);
        return scene.stats.blurs;
      });
    // The glow of a way into a work that moves is blurred at every frame, while the light runs and in the dark.
    expect(Math.min(...run())).toBeGreaterThan(0);
    // With no work that moves and nothing pointed at or selected, nothing is blurred.
    scene.show([card(1, [card(2)])]);
    settle(scene);
    expect(Math.max(...run())).toBe(0);
    // The light along the edges of the item selected is blurred in every frame until the item is let go, and goes
    // back out to the card's ends a moment after.
    scene.select(2);
    expect(Math.min(...run())).toBeGreaterThan(0);
    scene.select(null);
    expect(run().slice(-5)).toEqual([0, 0, 0, 0, 0]);
  });

  it('draws no frame while the page is hidden, nor while its view is, and draws again once both show', async () => {
    scene.show([card(1, [worked(2, running(2))])]);
    await drew();
    const page = (state: DocumentVisibilityState) => {
      Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => state });
      document.dispatchEvent(new Event('visibilitychange'));
    };
    try {
      page('hidden');
      const frames = scene.stats.frames;
      await drew();
      await drew();
      expect(scene.stats.frames).toBe(frames);
      // The page shown again while the view is hidden draws nothing either.
      scene.setShown(false);
      page('visible');
      await drew();
      await drew();
      expect(scene.stats.frames).toBe(frames);
      scene.setShown(true);
      await expect.poll(() => scene.stats.frames).toBeGreaterThan(frames);
    } finally {
      Reflect.deleteProperty(document, 'visibilityState');
      document.dispatchEvent(new Event('visibilitychange'));
    }
  });

  it("draws each item on the map in the colour of its card's state: running, in a call, for the owner, halted, paused, not begun, done or cancelled", () => {
    const now = Date.now();
    const call: Now = {
      kind: 'waits',
      at: 'implement',
      on: [{ kind: 'call', call: 1, who: null, what: { kind: 'tool', name: 'ci', input: 'the tests' } }],
    };
    const asks = [{ key: 'answer-which', why: 'Which?', ends: { kind: 'resume' as const } }];
    const paused: Now = { kind: 'paused', ran: true, at: 'implement', by: 'owner', why: null };
    const items: Shown[] = [
      worked(1, running(1)),
      worked(2, { ...running(2), now: call }),
      worked(3, { ...running(3), waits: asks }),
      worked(4, { ...running(4), now: { kind: 'fault', words: 'No session' } }),
      worked(5, { ...running(5), now: paused, paused: 'owner', from: now }),
      card(6),
      { ...card(7), standing: 'done' },
      { ...card(8), standing: 'cancelled' },
    ];
    scene.show(items);
    settle(scene);
    const colour = new Map(scene.drawnMap().pieces.map((p) => [p.id, p.colour]));
    const p = themed();
    // A call the engine carries out is the system's work, blue as a round is.
    expect([1, 2, 3, 4, 5, 6, 7, 8].map((id) => colour.get(id))).toEqual([
      p.running,
      p.running,
      p.halted,
      p.halted,
      p.paused,
      p.pale,
      p.hair,
      p.hair,
    ]);
  });

  it("draws the replies not read as a number on the card in the colour the map marks what is not read in, and marks on the map each item with something not read, a folded item for what is under it, whatever the item's `fresh` says", () => {
    const unread = (id: number, fresh: Shown['fresh'], repliesUnread: number, children: Shown[] = []): Shown => ({
      ...card(id, children),
      fresh,
      repliesUnread,
    });
    // 1 has two replies not read and is not new; 2 is new with none; 3 is changed with one; 4 is all read.
    scene.show([unread(1, 'none', 2), unread(2, 'new', 0), unread(3, 'changed', 1), unread(4, 'none', 0)]);
    settle(scene);
    const { unseen } = themed();
    expect([1, 2, 3, 4].map((id) => scene.drawnTags(id))).toEqual([
      [{ text: '2', fill: unseen }],
      [],
      [{ text: '1', fill: unseen }],
      [],
    ]);
    expect(scene.drawnMap().unread).toEqual([1, 2, 3]);
    // A card with a reply not read among the tags of what waits on the owner draws it after them.
    const waits = { ...running(5), waits: [{ key: 'answer-which', why: 'Which?', ends: { kind: 'resume' as const } }] };
    scene.show([{ ...unread(5, 'none', 3), begun: true, asks: 1, work: waits }]);
    settle(scene);
    expect(scene.drawnTags(5)).toEqual([
      { text: '1', fill: themed().red },
      { text: '3', fill: unseen },
    ]);
    // Folded away, what is not read under an item marks the item; shown, it marks the item it is of.
    const under = { open: 1, done: 0, cancelled: 0, elsewhere: 0 };
    const tree = (folded: boolean) => [{ ...card(6, [unread(7, 'none', 1)]), fold: { folded, under } }];
    scene.show(tree(true));
    for (let k = 0; k <= FOLD_MS / 16; k += 1) scene.step();
    settle(scene);
    expect(scene.drawnMap().unread).toEqual([6]);
    scene.show(tree(false));
    settle(scene);
    expect(scene.drawnMap().unread).toEqual([7]);
    expect(scene.drawnTags(7)).toEqual([{ text: '1', fill: unseen }]);
    // Read, the card is made again without the number and the map has no mark.
    scene.show(tree(false).map((root) => ({ ...root, children: [unread(7, 'none', 0)] })));
    settle(scene);
    expect([scene.drawnTags(7), scene.drawnMap().unread]).toEqual([[], []]);
  });

  it('frames on the map the part of the tree the screen shows, the shape of the screen, after a scroll, a pan and a zoom', () => {
    // Sixty roots in the first column, the first with an item under it and one under that: three columns.
    const roots = Array.from({ length: 60 }, (_, k) => card(k + 1, k === 0 ? [card(100, [card(101)])] : []));
    scene.show(roots);
    settle(scene);
    const framed = () => {
      const { frame } = scene.drawnMap();
      if (frame === null) throw new Error('no frame on the map');
      return frame;
    };
    /// The frame drawn is as wide for its height as the screen, and holds the pieces of the first column's items whose
    /// cards are on the screen, and none of those far off it.
    const holds = () => {
      const frame = framed();
      expect(frame.width / frame.height).toBeCloseTo(WIDTH / HEIGHT, 6);
      for (const piece of scene.drawnMap().pieces.filter((q) => q.column === 0)) {
        const y = where(scene, piece.id).y;
        const middle = piece.y + piece.height / 2;
        const inside = middle > frame.y && middle < frame.y + frame.height;
        if (y > ROW && y < HEIGHT - ROW) expect(inside, `item ${piece.id}`).toBe(true);
        if (y < -ROW || y > HEIGHT + ROW) expect(inside, `item ${piece.id}`).toBe(false);
      }
    };
    holds();
    // A scroll moves the pieces under the frame.
    move(50, line);
    wheel(0, 8 * ROW);
    settle(scene);
    holds();
    // A pan moves the frame across the map as far as the screen moved across the tree, at the map's scale.
    const before = framed();
    wheel(PITCH + 100, 0);
    settle(scene);
    holds();
    expect(framed().x - before.x).toBeCloseTo(((PITCH + 100) * before.width) / WIDTH, 6);
    // Drawn half as large round the middle of the screen, the frame is twice as wide and as tall round the same middle.
    const whole = framed();
    scene.zoomAt(0.5, WIDTH / 2, HEIGHT / 2);
    settle(scene);
    holds();
    const half = framed();
    expect(half.width).toBeCloseTo(2 * whole.width, 6);
    expect(half.height).toBeCloseTo(2 * whole.height, 6);
    expect(half.x + half.width / 2).toBeCloseTo(whole.x + whole.width / 2, 6);
    expect(half.y + half.height / 2).toBeCloseTo(whole.y + whole.height / 2, 6);
  });

  it('draws the whole of every tree on a map of one size, a tall tree and a wide one alike, and tells its size', async () => {
    const other = document.createElement('div');
    other.style.cssText = host.style.cssText;
    document.body.append(other);
    const wide = new SnapScene();
    await wide.mount(other);
    try {
      const told: [number, number][] = [];
      scene.onMapSize = (w, h) => told.push([w, h]);
      wide.onMapSize = (w, h) => told.push([w, h]);
      // Sixty roots, one column; and a line of six items, each under the one before: six columns, one row.
      scene.show(Array.from({ length: 60 }, (_, k) => card(k + 1)));
      const chain = (k: number): Shown => card(k, k < 6 ? [chain(k + 1)] : []);
      wide.show([chain(1)]);
      settle(scene);
      settle(wide);
      const size = scene.mapSize();
      expect(wide.mapSize()).toEqual(size);
      expect(told).toEqual([
        [size.width, size.height],
        [size.width, size.height],
      ]);
      // Every piece stands inside the map; the tall tree takes most of its height.
      for (const s of [scene, wide])
        for (const piece of s.drawnMap().pieces) {
          expect(piece.y).toBeGreaterThanOrEqual(0);
          expect(piece.y + piece.height).toBeLessThanOrEqual(size.height);
        }
      const pieces = scene.drawnMap().pieces;
      const tall = (pieces.at(-1)?.y ?? 0) + (pieces.at(-1)?.height ?? 0) - (pieces[0]?.y ?? 0);
      expect(tall).toBeGreaterThan(size.height / 2);
      // The wide tree, where the frame of the screen says the map's scale and where the tree's left side stands on it:
      // from its first column to its last, inside the map, across most of its width.
      const frame = wide.drawnMap().frame;
      if (frame === null) throw new Error('no frame on the map');
      const scale = frame.width / WIDTH;
      const left = frame.x + wide.place().left * scale;
      const right = left + (5 * PITCH + NODE_W) * scale;
      expect(left).toBeGreaterThanOrEqual(0);
      expect(right).toBeLessThanOrEqual(size.width);
      expect(right - left).toBeGreaterThan(size.width / 2);
    } finally {
      wide.destroy();
      other.remove();
    }
  });

  it('draws a card whose work halted before its first round in red, as one that halted after it, with no second row', () => {
    const fault: Now = { kind: 'fault', words: 'No session' };
    const halted = (k: number, from: number | null): Shown => worked(k, { ...running(k), now: fault, from });
    scene.show([halted(1, null), halted(2, Date.now()), card(3)]);
    const [before, after, plain] = [1, 2, 3].map((id) => scene.cardLook(id));
    expect(before?.rows).toBe(1);
    expect(after?.rows).toBeGreaterThan(1);
    expect([before?.fill, before?.edge]).toEqual([after?.fill, after?.edge]);
    expect(before?.fill).not.toBe(plain?.fill);
    expect(before?.edge).not.toBe(plain?.edge);
    expect(scene.drawnStrip(1)).toBeNull();
    expect(scene.drawnStrip(2)).not.toBeNull();
  });

  it('draws a card whose state the engine could not work out in the red of a halt, among plain cards', () => {
    const fault: Now = { kind: 'fault', words: 'No session' };
    const broken: Now = { kind: 'broken', words: 'the driver names no turn' };
    const stopped = (k: number, now: Now): Shown => worked(k, { ...running(k), now, from: null });
    scene.show([stopped(1, fault), stopped(2, broken), card(3)]);
    const [halted, unworked, plain] = [1, 2, 3].map((id) => scene.cardLook(id));
    expect([unworked?.fill, unworked?.edge]).toEqual([halted?.fill, halted?.edge]);
    expect(unworked?.fill).not.toBe(plain?.fill);
    expect(unworked?.edge).not.toBe(plain?.edge);
    expect(unworked?.rows).toBe(1);
  });

  it("lights the way to every work that moves, the owner's as the system's, and draws a grey way to a work the owner paused after it began", () => {
    const call: Now = {
      kind: 'waits',
      at: 'implement',
      on: [{ kind: 'call', call: 1, who: null, what: { kind: 'tool', name: 'ci', input: 'the tests' } }],
    };
    const paused: Now = { kind: 'paused', ran: true, at: 'implement', by: 'owner', why: null };
    const asks = [{ key: 'answer-which', why: 'Which?', ends: { kind: 'resume' as const } }];
    const under = (k: number, work: ShownWork) => worked(k, work);
    // Under 1: 2 runs a round, 3 waits for the engine's call, 4 waits for the owner, and 5 is paused after it began.
    const kids = [
      under(2, running(2)),
      under(3, { ...running(3), now: call }),
      under(4, { ...running(4), waits: asks }),
      under(5, { ...running(5), now: paused, paused: 'owner' }),
    ];
    scene.show([card(1, kids)]);
    settle(scene);
    expect(scene.litWays().map((w) => w.id).sort()).toEqual([2, 3, 4, 5]);
    // Folded away under 1, the work in its call ends the lit way at 1's dot, as a work whose round runs does.
    const folded = { open: 4, done: 0, cancelled: 0, elsewhere: 0 };
    scene.show([card(10, [{ ...card(1, [kids[1], kids[2]]), fold: { folded: true, under: folded } }])]);
    settle(scene);
    expect(scene.litWays().map((w) => w.id)).toEqual([1]);
  });

  it('turns a lit way that leaves an open card steeply into the edges it joins along its own curve, with no stub beside it', () => {
    // Under 1, shown, item 2, whose work runs, and eleven items more: 2 stands rows above 1, so that its way rises
    // steeply out of 1's edge.
    const rest = Array.from({ length: 11 }, (_, k) => card(3 + k));
    const under = { open: 12, done: 0, cancelled: 0, elsewhere: 0 };
    const two = worked(2, running(2));
    scene.show([{ ...card(1, [two, ...rest]), fold: { folded: false, under } }]);
    settle(scene);
    const way = scene.litWays().find((w) => w.id === 2);
    const [one, item] = [scene.cardAt(1), scene.cardAt(2)];
    if (way === undefined || one === null || item === null) throw new Error('the way into 2 is not drawn');
    // The way starts at the middle of 1's right edge and ends at the middle of 2's left edge, well above it.
    expect(way.from[0]).toBeCloseTo(one.x + NODE_W, 6);
    expect(way.from[1]).toBeCloseTo(one.y, 6);
    expect(way.to[0]).toBeCloseTo(item.x, 6);
    expect(way.to[1]).toBeCloseTo(item.y, 6);
    expect(one.y - item.y).toBeGreaterThan(4 * ROW);
    // Each turn into a card begins on the way's curve as drawn, off the card's middle where the way has already
    // turned, and heads along the curve to bend on the card's edge.
    for (const [turn, edge] of [[way.turns[0], way.from], [way.turns[1], way.to]] as const) {
      let best = { off: Infinity, along: [0, 0] as [number, number] };
      for (let n = 0; n + 1 < way.curve.length; n += 1) {
        const [a, b] = [way.curve[n], way.curve[n + 1]];
        const [dx, dy] = [b[0] - a[0], b[1] - a[1]];
        const len = Math.hypot(dx, dy);
        const k = Math.min(1, Math.max(0, ((turn.from[0] - a[0]) * dx + (turn.from[1] - a[1]) * dy) / (len * len)));
        const off = Math.hypot(turn.from[0] - (a[0] + dx * k), turn.from[1] - (a[1] + dy * k));
        if (off < best.off) best = { off, along: [dx / len, dy / len] };
      }
      expect(best.off).toBeLessThan(1e-6);
      expect(Math.abs(turn.from[1] - edge[1])).toBeGreaterThan(0.5);
      expect(turn.bend[0]).toBeCloseTo(edge[0], 6);
      const [hx, hy] = [turn.bend[0] - turn.from[0], turn.bend[1] - turn.from[1]];
      expect(Math.abs(hx * best.along[1] - hy * best.along[0]) / Math.hypot(hx, hy)).toBeLessThan(1e-6);
    }
  });

  it('draws a fold as it stands at the end of its turn in the first frame after its time, however far apart the frames come', () => {
    const under = { open: 1, done: 0, cancelled: 0, elsewhere: 0 };
    const tree = (folded: boolean) => [{ ...card(1, [card(2)]), fold: { folded, under } }];
    scene.show(tree(false));
    settle(scene);
    // A frame a second, as a browser gives a page it does not show: the first frame after the fold is its end, with
    // nothing left over of what drew in to the dot, and the first after the unfold has it whole.
    scene.show(tree(true));
    scene.step(1000);
    expect(scene.place().turns).toBe(0);
    expect(scene.stats.items).toBe(1);
    scene.show(tree(false));
    scene.step(1000);
    expect(scene.place().turns).toBe(0);
    expect(scene.stats.items).toBe(2);
    // At the page's own rate, a fold is under way for as many frames as its time holds: 11 frames of 16 ms, ending on
    // the 12th.
    scene.show(tree(true));
    for (let k = 1; k < FOLD_MS / 16; k += 1) scene.step(16);
    expect(scene.place().turns).toBe(1);
    scene.step(16);
    expect(scene.place().turns).toBe(0);
  });

  it('asks for the folds over an item folded away to be opened, at a click on its shadow or its selection, and selects it once it is drawn', async () => {
    const told = vi.fn<(item: Shown | null) => void>();
    const turned = vi.fn<(id: number, folded: boolean) => void>();
    scene.onFold = turned;
    const under = { open: 1, done: 0, cancelled: 0, elsewhere: 0 };
    // Under 1, folded or not, item 2; under 3, a shadow of 2.
    const tree = (folded: boolean) => [{ ...card(1, [card(2)]), fold: { folded, under } }, card(3, [shadow(2)])];
    scene.show(tree(true));
    settle(scene);
    scene.onSelect = told;
    // The shadow, the only item under 3, stands level with it in the next column.
    const three = where(scene, 3);
    click(three.x + PITCH + 100, three.y);
    expect(turned).toHaveBeenLastCalledWith(1, false);
    expect(told).not.toHaveBeenCalled();
    scene.show(tree(false));
    expect(told).toHaveBeenLastCalledWith(expect.objectContaining({ id: 2, shadow: false }));
    // Once it has grown out of the dot, it comes to the middle of the screen.
    settle(scene);
    expect(where(scene, 2).y).toBeCloseTo(line, 6);
    // An item selected by its number while it is folded away, the same way.
    scene.show(tree(true));
    settle(scene);
    turned.mockClear();
    told.mockClear();
    scene.select(2);
    expect(turned).toHaveBeenLastCalledWith(1, false);
    expect(told).not.toHaveBeenCalled();
    scene.show(tree(false));
    expect(told).toHaveBeenLastCalledWith(expect.objectContaining({ id: 2 }));
  });

  it('selects the item a shadow points to on a click on the shadow, and brings it to the middle', async () => {
    const told = vi.fn<(item: Shown | null) => void>();
    scene.onSelect = told;
    // Under 1: item 2 and a shadow of 3; under 4: item 3.
    scene.show([card(1, [card(2), shadow(3)]), card(4, [card(3)])]);
    settle(scene);
    // The shadow stands just under item 2.
    const two = where(scene, 2);
    click(two.x + 100, two.y + ROW);
    expect(told).toHaveBeenLastCalledWith(expect.objectContaining({ id: 3, shadow: false }));
    settle(scene);
    expect(where(scene, 3).y).toBeCloseTo(line, 6);
  });

  it("tells of a click on a shadow of an item this tree does not hold, which another project's tree holds, and selects nothing", async () => {
    const told = vi.fn<(item: Shown | null) => void>();
    const away = vi.fn<(id: number) => void>();
    // Under 1, a shadow of 9, which no fold of this tree hides.
    scene.show([card(1, [shadow(9)])]);
    settle(scene);
    scene.onSelect = told;
    scene.onAway = away;
    // The shadow, the only item under 1, stands level with it in the next column.
    const one = where(scene, 1);
    click(one.x + PITCH + 100, one.y);
    expect(away).toHaveBeenLastCalledWith(9);
    expect(told).not.toHaveBeenCalled();
  });

  it('draws new items where the tree stood, by the item selected, else by the item nearest the middle, and keeps the item selected', async () => {
    const told = vi.fn<(item: Shown | null) => void>();
    scene.onSelect = told;
    scene.show([card(1), card(2), card(3)]);
    scene.select(3);
    settle(scene);
    told.mockClear();
    // Item 9 comes in between 2 and 3: item 3, selected, stays where it stood, not item 2 in the middle.
    const three = where(scene, 3).y;
    scene.show([card(1), card(2), card(9), card(3)]);
    expect(where(scene, 3).y).toBeCloseTo(three, 9);
    expect(told).toHaveBeenLastCalledWith(expect.objectContaining({ id: 3 }));
    expect(told).not.toHaveBeenCalledWith(null);
    // With nothing selected, item 9, now in the middle, stays where it stood as item 8 comes in at the top.
    scene.select(null);
    settle(scene);
    const nine = where(scene, 9).y;
    expect(nine).toBeCloseTo(line, 9);
    scene.show([card(8), card(1), card(2), card(9), card(3)]);
    expect(where(scene, 9).y).toBeCloseTo(nine, 9);
  });

  it('keeps the item selected where it stands on the screen at every frame while a fold above it turns, and else the item under the pointer', () => {
    const under = { open: 4, done: 0, cancelled: 0, elsewhere: 0 };
    // Root 1 with four items under it, folded or not, and roots 2 and 3 after it, which the fold moves up or down the
    // tree.
    const tree = (folded: boolean) => [
      { ...card(1, [10, 11, 12, 13].map((id) => card(id))), fold: { folded, under } },
      card(2),
      card(3),
    ];
    scene.show(tree(false));
    scene.select(3);
    settle(scene);
    /// Folds or unfolds the items under 1, and holds that item `id` stands where it stood at every frame of the turn
    /// and once it is over.
    const stays = (folded: boolean, id: number) => {
      const was = where(scene, id).y;
      scene.show(tree(folded));
      expect(scene.place().turns).toBe(1);
      for (let k = 0; k < 40; k += 1) {
        scene.step();
        expect(where(scene, id).y).toBeCloseTo(was, 6);
      }
      expect(scene.place().turns).toBe(0);
    };
    stays(true, 3);
    stays(false, 3);
    // With nothing selected, the item under the pointer.
    scene.select(null);
    settle(scene);
    const two = where(scene, 2);
    move(two.x + 100, two.y);
    scene.step();
    stays(true, 2);
    stays(false, 2);
  });

  it('lights the item the pointer comes onto at once, and fades the others', () => {
    scene.show([card(1), card(2), card(3)]);
    settle(scene);
    const [one, two] = [where(scene, 1), where(scene, 2)];
    move(one.x + 100, one.y);
    scene.step();
    expect(scene.brightness(1)).toBe(1);
    // The others begin to fade at once, and are dimmed once they have.
    const fading = scene.brightness(2) as number;
    expect(fading).toBeLessThan(1);
    expect(fading).toBeGreaterThan(DIM);
    for (let k = 0; k < 3; k += 1) scene.step(1000);
    expect(scene.brightness(2)).toBe(DIM);
    // From item 1 onto item 2: item 2 lights in the first frame, and item 1 begins to fade.
    move(two.x + 100, two.y);
    scene.step();
    expect(scene.brightness(2)).toBe(1);
    const leaving = scene.brightness(1) as number;
    expect(leaving).toBeLessThan(1);
    expect(leaving).toBeGreaterThan(DIM);
  });

  /// Sixty roots, each fifth with two items under it: three times as tall as the screen.
  const tall = () =>
    Array.from({ length: 60 }, (_, k) => card(k + 1, k % 5 === 0 ? [card(100 + 2 * k), card(101 + 2 * k)] : []));
  const tallIds = [1, 30, 60, 100, 150];

  it('draws the tree larger or smaller round a point of the canvas, which stays on the same point of the tree, as the wheel does with Shift held, never larger than its full size', () => {
    scene.show(tall());
    settle(scene);
    const drawnAt = () => tallIds.map((id) => where(scene, id));
    /// Each card stands `by` times as far from the point (`sx`, `sy`) of the canvas as it stood in `was`, and is drawn
    /// `by` times as large.
    const round = (was: ReturnType<typeof drawnAt>, sx: number, sy: number, by: number) =>
      drawnAt().forEach((c, k) => {
        expect(c.x - sx).toBeCloseTo((was[k].x - sx) * by, 6);
        expect(c.y - sy).toBeCloseTo((was[k].y - sy) * by, 6);
        expect(c.size).toBeCloseTo(was[k].size * by, 9);
      });
    const [sx, sy] = [600, 200];
    let was = drawnAt();
    scene.zoomAt(0.5, sx, sy);
    expect(scene.place().zoom).toBe(0.5);
    round(was, sx, sy, 0.5);
    // Nothing glides after it: the frames after leave it where it is.
    settle(scene);
    round(was, sx, sy, 0.5);
    // The wheel with Shift held, round the pointer: a turn down makes it smaller, a turn up larger.
    const shifted = (x: number, y: number, deltaY: number) => {
      const r = canvas().getBoundingClientRect();
      canvas().dispatchEvent(
        new WheelEvent('wheel', { clientX: r.left + x, clientY: r.top + y, deltaY, shiftKey: true, bubbles: true, cancelable: true }),
      );
    };
    was = drawnAt();
    shifted(300, 400, 100);
    const smaller = scene.place().zoom;
    expect(smaller).toBeLessThan(0.5);
    round(was, 300, 400, smaller / 0.5);
    was = drawnAt();
    shifted(300, 400, -300);
    const larger = scene.place().zoom;
    expect(larger).toBeGreaterThan(smaller);
    round(was, 300, 400, larger / smaller);
    // Never larger than its full size, and never so small that it is gone.
    scene.zoomAt(10, sx, sy);
    expect(scene.place().zoom).toBe(1);
    scene.zoomAt(1e-6, sx, sy);
    expect(scene.place().zoom).toBeGreaterThan(0.01);
  });

  it('fits the whole tree into the room it is given, gliding there, and goes back to where it was, or to the size it was round the pointer', () => {
    scene.show(tall());
    wheel(0, 5 * ROW);
    wheel(60, 0);
    settle(scene);
    const view = (p: ReturnType<SnapScene['place']>) => ({ zoom: p.zoom, at: p.at, aim: p.aim, left: p.left });
    const before = view(scene.place());
    // Into the room right of 200 pixels and left of 800, part of the way in the first frame.
    scene.overview(200, 800);
    expect(scene.wholeShown()).toBe(true);
    scene.step();
    const first = scene.place().zoom;
    settle(scene);
    const fit = scene.place().zoom;
    expect(fit).toBeLessThan(first);
    expect(first).toBeLessThan(before.zoom);
    for (const id of tallIds) {
      const c = where(scene, id);
      expect(c.x).toBeGreaterThanOrEqual(200);
      expect(c.x + NODE_W * fit).toBeLessThanOrEqual(800);
      expect(c.y - (ROW * fit) / 2).toBeGreaterThanOrEqual(0);
      expect(c.y + (ROW * fit) / 2).toBeLessThanOrEqual(HEIGHT);
    }
    // Asked again, with the pointer off the canvas: back where it was.
    scene.overview(200, 800);
    expect(scene.wholeShown()).toBe(false);
    settle(scene);
    expect(view(scene.place())).toEqual(before);
    // Fitted again, and asked again with the pointer on the tree: back to the size it was, the point of the tree under
    // the pointer staying under it.
    scene.overview(200, 800);
    settle(scene);
    const [px, py] = [500, 300];
    move(px, py);
    scene.step();
    const pinned = () => {
      const p = scene.place();
      return { row: p.at + (py - HEIGHT / 2) / (p.zoom * ROW), along: px / p.zoom - p.left };
    };
    const held = pinned();
    scene.overview(200, 800);
    settle(scene);
    expect(scene.place().zoom).toBeCloseTo(before.zoom, 9);
    expect(pinned().row).toBeCloseTo(held.row, 6);
    expect(pinned().along).toBeCloseTo(held.along, 6);
  });

  it("shows the whole tree at once while every project's tree is shown, and gives back the very view it had", () => {
    scene.show(tall());
    wheel(0, 5 * ROW);
    wheel(60, 0);
    scene.zoomAt(0.7, 300, 300);
    settle(scene);
    const before = scene.place();
    scene.showWhole(true);
    const whole = scene.place();
    expect(whole.zoom).toBeLessThan(before.zoom);
    for (const id of tallIds) {
      const c = where(scene, id);
      expect(c.x).toBeGreaterThanOrEqual(0);
      expect(c.x + NODE_W * whole.zoom).toBeLessThanOrEqual(WIDTH);
      expect(c.y).toBeGreaterThan(0);
      expect(c.y).toBeLessThan(HEIGHT);
    }
    // At once, with no glide: the frames after leave it where it is.
    settle(scene);
    expect(scene.place()).toEqual(whole);
    scene.showWhole(false);
    expect(scene.place()).toEqual(before);
    settle(scene);
    expect(scene.place()).toEqual(before);
  });

  it('brings an item revealed into the room the panel leaves, a little in from its side', async () => {
    scene.show([card(1, [card(2, [card(3, [card(4)])])])]);
    settle(scene);
    scene.reveal(4, 600);
    settle(scene);
    expect(Math.abs(scene.place().left - (600 - 24 - NODE_W - 3 * PITCH))).toBeLessThan(0.5);
  });

  it('takes a press let go within four pixels of where it began for a click, and one further for a drag', async () => {
    const told = vi.fn<(item: Shown | null) => void>();
    scene.onSelect = told;
    scene.show([card(1), card(2), card(3)]);
    settle(scene);
    told.mockClear();
    const x = scene.place().left + 100;
    move(x, line);
    await drew();
    canvas().dispatchEvent(new PointerEvent('pointerdown', at(x, line)));
    canvas().dispatchEvent(new PointerEvent('pointerup', at(x + 4, line)));
    expect(told).toHaveBeenLastCalledWith(expect.objectContaining({ id: 2 }));
    told.mockClear();
    canvas().dispatchEvent(new PointerEvent('pointerdown', at(x, line)));
    canvas().dispatchEvent(new PointerEvent('pointerup', at(x + 3, line + 4)));
    // Five pixels: a drag, which lets nothing go.
    expect(told).not.toHaveBeenCalled();
  });

  it('brings an item revealed above or below what is seen to the middle, and leaves one in view where it is', async () => {
    scene.show(Array.from({ length: 30 }, (_, k) => card(k + 1)));
    settle(scene);
    scene.reveal(15, WIDTH);
    settle(scene);
    expect(scene.place().at).toBe(14.5);
    scene.reveal(30, WIDTH);
    settle(scene);
    expect(scene.place().at).toBeCloseTo(29.5, 3);
  });

  it('keeps the view where it stands when the height changes, and when the theme does', async () => {
    scene.show(Array.from({ length: 10 }, (_, k) => card(k + 1, [card(100 + 2 * k), card(101 + 2 * k)])));
    move(50, line);
    wheel(0, 2 * ROW);
    wheel(80, 0);
    settle(scene);
    const before = scene.place();
    host.style.height = `${HEIGHT - 140}px`;
    await expect.poll(() => scene.place().height).toBe(HEIGHT - 140);
    settle(scene);
    const shorter = scene.place();
    expect([shorter.zoom, shorter.left]).toEqual([before.zoom, before.left]);
    expect(shorter.at).toBeCloseTo(before.at, 9);
    document.documentElement.setAttribute('data-theme', 'dark');
    await drew();
    settle(scene);
    const dark = scene.place();
    expect([dark.zoom, dark.left]).toEqual([before.zoom, before.left]);
    expect(dark.at).toBeCloseTo(before.at, 9);
  });

  it('takes the width of what holds it when that changes, as when the panel beside it folds', async () => {
    scene.show([card(1)]);
    host.style.width = '700px';
    await expect.poll(() => scene.place().width).toBe(700);
  });

  it("draws in the console's own colours in light, and in the tokens' in dark", () => {
    expect(themed()).toMatchObject({ page: '#f6f7f9', card: '#ffffff', ink: '#1f2937', running: '#3673f5', halted: '#e5484d' });
    document.documentElement.setAttribute('data-theme', 'dark');
    expect(themed()).toMatchObject({ page: '#17191e', card: '#1f2127', ink: '#c8ccd4', running: '#739ffa' });
  });

  it("turns every colour of its own in the default accent's hue to an accent picked, its hue turned as the accent is, and keeps its other colours, in light and in dark", () => {
    const violet = PRESETS[2].colour;
    const turn = lchOf(violet).h - lchOf(DEFAULT_ACCENT).h;
    const t = tones(violet);
    /// The scene's colours of the default accent's hue: what an accent picked is to turn.
    const ofTheHue = (colours: ReturnType<typeof themed>) =>
      Object.entries(colours).filter(
        (e): e is [string, string] =>
          typeof e[1] === 'string' && lchOf(e[1]).c > 0.04 && apart(lchOf(e[1]).h, lchOf(DEFAULT_ACCENT).h) < 30,
      );
    try {
      for (const theme of ['light', 'dark']) {
        document.documentElement.setAttribute('data-theme', theme);
        pickAccent(null);
        const before = themed();
        expect(ofTheHue(before).length).toBeGreaterThan(8);
        expect(pickAccent(violet)).toBeNull();
        const after = themed();
        for (const [key, colour] of ofTheHue(before)) {
          const now = after[key as keyof typeof after] as string;
          expect(now, key).not.toBe(colour);
          // In light each is turned as a tone of the accent is; in dark some are mixed with the card, a little off.
          expect(apart(lchOf(now).h, lchOf(colour).h + turn), key).toBeLessThan(theme === 'light' ? 4 : 10);
        }
        const others = Object.keys(before).filter((k) => !ofTheHue(before).some(([p]) => p === k));
        expect(others.map((k) => after[k as keyof typeof after])).toEqual(
          others.map((k) => before[k as keyof typeof before]),
        );
        const accent = theme === 'light' ? t.light.accent : t.dark.accent;
        expect(after).toMatchObject({ chosen: accent, running: accent });
      }
    } finally {
      pickAccent(null);
    }
  });
});

describe('a scene destroyed', () => {
  it('listens to nothing once destroyed', async () => {
    const add = vi.spyOn(window, 'addEventListener');
    const remove = vi.spyOn(window, 'removeEventListener');
    const docAdd = vi.spyOn(document, 'addEventListener');
    const docRemove = vi.spyOn(document, 'removeEventListener');
    const mediaAdd = vi.spyOn(MediaQueryList.prototype, 'addEventListener');
    const mediaRemove = vi.spyOn(MediaQueryList.prototype, 'removeEventListener');
    const sized = vi.spyOn(ResizeObserver.prototype, 'disconnect');
    const themes = vi.spyOn(MutationObserver.prototype, 'disconnect');
    const host = document.createElement('div');
    host.style.cssText = `position: fixed; left: 0; top: 0; width: ${WIDTH}px; height: ${HEIGHT}px`;
    document.body.append(host);
    const scene = new SnapScene();
    await scene.mount(host);
    scene.show([card(1)]);
    await drew();
    scene.destroy();
    const left = (calls: unknown[][], gone: unknown[][]) =>
      calls.filter(([type, fn]) => !gone.some(([t, f]) => t === type && f === fn)).map(([type]) => type);
    expect(add.mock.calls.length).toBeGreaterThan(0);
    expect(left(add.mock.calls, remove.mock.calls)).toEqual([]);
    // On the document it listens for the page being hidden and shown, and no more once destroyed.
    const visibility = (calls: unknown[][]) => calls.filter(([type]) => type === 'visibilitychange');
    expect(visibility(docAdd.mock.calls)).toHaveLength(1);
    expect(left(visibility(docAdd.mock.calls), docRemove.mock.calls)).toEqual([]);
    expect(mediaAdd.mock.calls.length).toBeGreaterThan(0);
    expect(left(mediaAdd.mock.calls, mediaRemove.mock.calls)).toEqual([]);
    expect(sized).toHaveBeenCalled();
    expect(themes).toHaveBeenCalled();
    // A change of theme after it draws nothing, and throws nothing.
    document.documentElement.setAttribute('data-theme', 'dark');
    await drew();
    document.documentElement.removeAttribute('data-theme');
    host.remove();
    vi.restoreAllMocks();
  });

  it('leaves every other scene drawing, what the scenes share kept', async () => {
    const hosts = [0, 1].map(() => {
      const h = document.createElement('div');
      h.style.cssText = `position: fixed; left: 0; top: 0; width: ${WIDTH}px; height: ${HEIGHT}px`;
      document.body.append(h);
      return h;
    });
    const [gone, kept] = [new SnapScene(), new SnapScene()];
    await gone.mount(hosts[0]);
    await kept.mount(hosts[1]);
    try {
      // The scene to be destroyed draws nothing of its own, as a project with almost no item; the kept one draws a
      // tree with a work running under it and an item selected, and draws it again with an item more. What it draws
      // with the second time it takes back from what PixiJS keeps for every renderer on the page, which a release of
      // what they share would free under it.
      const tree = (more: Shown[]) => [card(2), card(3, [worked(4, running(4))]), ...more];
      kept.show(tree([]));
      kept.select(2);
      await drew();
      kept.show(tree([card(5)]));
      await drew();
      /// The colour the kept scene draws, as it stands now, at the point (`x`, `y`) of its canvas.
      const pixel = (x: number, y: number) => {
        const picture = kept.picture() as HTMLCanvasElement;
        const r = picture.width / WIDTH;
        return [...(picture.getContext('2d')?.getImageData(Math.round(x * r), Math.round(y * r), 1, 1).data ?? [])];
      };
      // On item 2's card, right of its title, and on the page's ground right of the tree.
      const two = where(kept, 2);
      const card2 = () => pixel(two.x + 200, two.y);
      const ground = pixel(WIDTH - 10, HEIGHT / 2);
      const drawn = card2();
      expect(drawn).toHaveLength(4);
      expect(drawn).not.toEqual(ground);
      gone.destroy();
      // The kept scene goes on drawing its frames, and draws its card as before.
      const frames = kept.stats.frames;
      await expect.poll(() => kept.stats.frames).toBeGreaterThan(frames + 2);
      kept.drawNow();
      expect(card2()).toEqual(drawn);
    } finally {
      kept.destroy();
      for (const h of hosts) h.remove();
    }
  });
});

describe('the console page', () => {
  let container: HTMLDivElement;
  let unmount: () => void;
  // Each test begins with nothing kept, whatever a test before it left: a test cut off at its time limit goes on
  // typing after the one after it has begun.
  const forget = () => {
    try {
      localStorage.clear();
    } catch {
      // Nothing kept.
    }
  };
  beforeEach(() => {
    forget();
    container = document.createElement('div');
    container.style.cssText = 'position: fixed; inset: 0; display: flex';
    document.body.append(container);
    delete (window as { consoleScene?: SnapScene }).consoleScene;
    const root = createRoot(container);
    root.render(<ConsolePage source={fixtureSource()} />);
    unmount = () => root.unmount();
  });
  afterEach(() => {
    unmount();
    container.remove();
    forget();
  });

  /// The scene, once it has drawn the made-up items.
  async function drawn(): Promise<SnapScene> {
    await expect.poll(() => (window as { consoleScene?: SnapScene }).consoleScene?.stats.items ?? 0).toBeGreaterThan(0);
    return (window as unknown as { consoleScene: SnapScene }).consoleScene;
  }
  const title = () => container.querySelector('.drawer-title')?.textContent ?? null;
  const folded = () => container.querySelector('aside.cw')?.classList.contains('cw-open') === false;
  const button = (words: string) =>
    [...container.querySelectorAll('button')].find((b) => b.textContent === words) ?? null;

  it('shows the open items at first, and the done and the cancelled as their boxes are ticked, keeping the boxes ticked in the browser', async () => {
    const scene = await drawn();
    const boxes = () => [...container.querySelectorAll<HTMLLabelElement>('.console-shows label')];
    const box = (name: string) => boxes().find((l) => l.textContent === name)?.querySelector('input') as HTMLInputElement;
    expect(container.querySelector('.console-shows')?.getAttribute('aria-label')).toBe('Which items the tree shows');
    expect(boxes().map((l) => l.textContent)).toEqual(['Open', 'Done', 'Cancelled']);
    expect(boxes().map((l) => l.title)).toEqual([
      'The items not done or cancelled: running, paused, waiting or not started',
      'The items whose work merged',
      'The items whose work was cancelled',
    ]);
    expect(['Open', 'Done', 'Cancelled'].map((n) => box(n).checked)).toEqual([true, false, false]);
    // Of the made-up items of wf-agents, the project listed first, 26 are open or stand over an open one. Done shows 4
    // more: two done; one done, the two done under it folded away; and one cancelled, the one done under it folded
    // away. Cancelled shows 2 more. And 13 are done or cancelled or stand over one that is, what is folded away not
    // drawn.
    expect(scene.stats.items).toBe(26);
    box('Done').click();
    await expect.poll(() => scene.stats.items).toBe(30);
    box('Cancelled').click();
    await expect.poll(() => scene.stats.items).toBe(32);
    box('Open').click();
    await expect.poll(() => scene.stats.items).toBe(13);
    expect(localStorage.getItem('wf-agents.console-shows')).toBe('done,cancelled');
  });

  it('folds what has no open item in it into a dot, shows it at a click on the dot, and keeps that over the rule for the next page', async () => {
    let scene = await drawn();
    const done = [...container.querySelectorAll<HTMLLabelElement>('.console-shows label')]
      .find((l) => l.textContent === 'Done')
      ?.querySelector('input') as HTMLInputElement;
    done.click();
    await expect.poll(() => scene.stats.items).toBe(30);
    /// A click `out` pixels past the right edge of item `id`, brought into view, level with its middle: on its dot
    /// where it is folded, else on the mouth of its funnel.
    const press = async (id: number, out: number) => {
      scene.reveal(id, scene.place().width);
      settle(scene);
      const stands = scene.cardAt(id) as { x: number; y: number };
      const canvas = container.querySelector('canvas') as HTMLCanvasElement;
      const r = canvas.getBoundingClientRect();
      const at: PointerEventInit = {
        clientX: r.left + stands.x + NODE_W + out,
        clientY: r.top + stands.y,
        bubbles: true,
      };
      canvas.dispatchEvent(new PointerEvent('pointermove', at));
      await drew();
      canvas.dispatchEvent(new PointerEvent('pointerdown', at));
      canvas.dispatchEvent(new PointerEvent('pointerup', at));
    };
    /// How many items are drawn once every fold has turned.
    const turnedItems = () => {
      settle(scene);
      return scene.stats.items;
    };
    // Item 27, done, stands folded into a dot over the two done items under it: a click on the dot shows them.
    await press(27, DOT_AT);
    await expect.poll(turnedItems).toBe(32);
    expect(JSON.parse(localStorage.getItem('wf-agents.console-folds') ?? '')).toEqual({ 27: 'unfolded' });
    unmount();
    delete (window as { consoleScene?: SnapScene }).consoleScene;
    const root = createRoot(container);
    root.render(<ConsolePage source={fixtureSource()} />);
    unmount = () => root.unmount();
    scene = await drawn();
    await expect.poll(() => scene.stats.items).toBe(32);
    // Shown as the owner chose, its funnel folds them away again.
    await press(27, 4);
    await expect.poll(turnedItems).toBe(30);
    expect(JSON.parse(localStorage.getItem('wf-agents.console-folds') ?? '')).toEqual({ 27: 'folded' });
  });

  it('folds and unfolds the items under the card the pointer is over at `e`, and does nothing over a card with none, while the owner types in a field, or with Ctrl held', async () => {
    const scene = await drawn();
    const done = [...container.querySelectorAll<HTMLLabelElement>('.console-shows label')]
      .find((l) => l.textContent === 'Done')
      ?.querySelector('input') as HTMLInputElement;
    done.click();
    await expect.poll(() => scene.stats.items).toBe(30);
    /// The pointer over the card of item `id`, brought into view, once a frame has found it there.
    const over = async (id: number) => {
      scene.reveal(id, scene.place().width);
      settle(scene);
      const stands = scene.cardAt(id) as { x: number; y: number };
      const canvas = container.querySelector('canvas') as HTMLCanvasElement;
      const r = canvas.getBoundingClientRect();
      const at = { clientX: r.left + stands.x + 100, clientY: r.top + stands.y, bubbles: true };
      canvas.dispatchEvent(new PointerEvent('pointermove', at));
      await drew();
    };
    const press = (on: Element, keys: KeyboardEventInit = {}) =>
      on.dispatchEvent(new KeyboardEvent('keydown', { key: 'e', bubbles: true, cancelable: true, ...keys }));
    const kept = () => localStorage.getItem('wf-agents.console-folds');
    const turnedItems = () => {
      settle(scene);
      return scene.stats.items;
    };
    // Over item 27, done and folded over the two done items under it, `e` shows them, and again folds them.
    await over(27);
    press(document.body);
    await expect.poll(turnedItems).toBe(32);
    expect(JSON.parse(kept() ?? '')).toEqual({ 27: 'unfolded' });
    await over(27);
    press(document.body);
    await expect.poll(turnedItems).toBe(30);
    expect(JSON.parse(kept() ?? '')).toEqual({ 27: 'folded' });
    // With Ctrl held, or while the owner types in a field, `e` folds nothing.
    press(document.body, { ctrlKey: true });
    const field = container.appendChild(document.createElement('input'));
    field.focus();
    press(field);
    field.remove();
    // Over item 10, done, with nothing under it, `e` folds nothing either.
    await over(10);
    press(document.body);
    await drew();
    expect(turnedItems()).toBe(30);
    expect(JSON.parse(kept() ?? '')).toEqual({ 27: 'folded' });
  });

  it('unfolds every item of the tree at `O`, folds at `o` those under which no work is under way, keeps both, and does neither while the owner types in a field or with Ctrl held', async () => {
    const scene = await drawn();
    const done = [...container.querySelectorAll<HTMLLabelElement>('.console-shows label')]
      .find((l) => l.textContent === 'Done')
      ?.querySelector('input') as HTMLInputElement;
    done.click();
    await expect.poll(() => scene.stats.items).toBe(30);
    // The tree shown, of the project first in the list.
    const tree = shownTree(await fixtureSource().items.loadItems(), new Set(['open', 'done']), new Map(), 'wf-agents');
    const all = foldable(tree);
    const quiet = new Set(quietFolds(tree));
    // The made-up items hold both kinds, so that `o` is seen to leave the folds over a work under way as they are.
    expect(quiet.size).toBeGreaterThan(0);
    expect(quiet.size).toBeLessThan(all.length);
    const press = (key: string, on: Element = document.body, keys: KeyboardEventInit = {}) =>
      on.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...keys }));
    const kept = () => JSON.parse(localStorage.getItem('wf-agents.console-folds') ?? '{}') as Record<string, string>;
    const turnedItems = () => {
      settle(scene);
      return scene.stats.items;
    };
    press('O', document.body, { shiftKey: true });
    await expect.poll(() => kept()).toEqual(Object.fromEntries(all.map((id) => [id, 'unfolded'])));
    await expect.poll(turnedItems).toBeGreaterThan(30);
    const unfolded = turnedItems();
    press('o');
    const after = Object.fromEntries(all.map((id) => [id, quiet.has(id) ? 'folded' : 'unfolded']));
    await expect.poll(() => kept()).toEqual(after);
    await expect.poll(turnedItems).toBeLessThan(unfolded);
    // With Ctrl held, or while the owner types in a field, neither key changes a fold.
    press('O', document.body, { shiftKey: true, ctrlKey: true });
    const field = container.appendChild(document.createElement('input'));
    field.focus();
    press('O', field, { shiftKey: true });
    field.remove();
    await drew();
    expect(kept()).toEqual(after);
  });

  /// The search's field, while the search is open, and the numbers of the rows it finds.
  const field = () => container.querySelector('.console-find input') as HTMLInputElement | null;
  const found = () => [...container.querySelectorAll('.console-found-row > .mono')].map((s) => s.textContent);
  /// The box of a kind of item over the tree.
  const box = (name: string) =>
    [...container.querySelectorAll<HTMLLabelElement>('.console-shows label')]
      .find((l) => l.textContent === name)
      ?.querySelector('input') as HTMLInputElement;

  it('shows the search only from `/`, over the middle of the top of the console, finds by number and by a part of the title, and closes at a press elsewhere or Escape', async () => {
    await drawn();
    // Nothing of it shows until `/`.
    expect(field()).toBeNull();
    await userEvent.keyboard('/');
    await expect.poll(() => document.activeElement !== null && document.activeElement === field()).toBe(true);
    expect(field()?.value).toBe('');
    // By its number: item 27, done.
    await userEvent.fill(field() as HTMLInputElement, '27');
    await expect.poll(found).toEqual(['#27']);
    expect(container.querySelector('.console-found-row .console-found-standing')?.textContent).toBe('Done');
    // Numbers joined by `-` name no path; nothing found is said in the list's own frame, on its ground.
    await userEvent.fill(field() as HTMLInputElement, '26-27');
    await expect.poll(() => container.querySelector('.console-found > .console-found-none')?.textContent).toBe(
      FIND_SAYS.none,
    );
    // It stands over the middle of the console, in its top third, and the list under the field is as wide as the field.
    const whole = (container.querySelector('.console') as HTMLElement).getBoundingClientRect();
    const search = (container.querySelector('.console-find') as HTMLElement).getBoundingClientRect();
    expect(Math.abs(search.left + search.width / 2 - (whole.left + whole.width / 2))).toBeLessThan(1);
    expect(search.top - whole.top).toBeLessThan(whole.height / 3);
    const list = (container.querySelector('.console-found') as HTMLElement).getBoundingClientRect();
    expect(Math.abs(list.width - (field() as HTMLInputElement).getBoundingClientRect().width)).toBeLessThan(1);
    // By a part of the title, in any case, over every item, done or not, by number.
    await userEvent.fill(field() as HTMLInputElement, 'FOLD');
    await expect.poll(found).toEqual(['#26', '#27', '#30', '#31']);
    // A press elsewhere closes it and keeps what was typed for the next time it opens.
    (document.activeElement as HTMLElement | null)?.blur();
    await expect.poll(field).toBeNull();
    await userEvent.keyboard('/');
    await expect.poll(() => field()?.value).toBe('FOLD');
    // Escape empties it, closes it and gives the keys back.
    await userEvent.keyboard('{Escape}');
    await expect.poll(field).toBeNull();
    expect(document.activeElement?.closest('.console-find') ?? null).toBeNull();
    await userEvent.keyboard('/');
    await expect.poll(() => field()?.value).toBe('');
  });

  it('dims the tree while it is open, lighting what it finds as round the item under the pointer, and lets the pointer choose a row', async () => {
    const scene = await drawn();
    const near = (id: number, to: number) => Math.abs((scene.brightness(id) ?? -1) - to);
    // Open, with nothing typed, every item dims.
    await userEvent.keyboard('/');
    await expect.poll(() => near(1, DIM)).toBeLessThan(0.01);
    expect(near(26, DIM)).toBeLessThan(0.01);
    await userEvent.fill(field() as HTMLInputElement, 'Fold the items under');
    await expect.poll(found).toEqual(['#26']);
    // Item 26, found, lights up; item 1, drawn and not found, stays dimmed.
    await expect.poll(() => near(26, 1)).toBeLessThan(0.01);
    expect(near(1, DIM)).toBeLessThan(0.01);
    // The pointer chooses the row it is over, as the arrow keys do.
    await userEvent.fill(field() as HTMLInputElement, 'fold');
    await expect.poll(found).toEqual(['#26', '#27', '#30', '#31']);
    const rows = () => [...container.querySelectorAll('.console-found-row')];
    await userEvent.hover(rows()[2] as HTMLElement);
    await expect
      .poll(() => rows().map((r) => r.getAttribute('aria-selected')))
      .toEqual(['false', 'false', 'true', 'false']);
    // Closed, it dims nothing any more.
    await userEvent.keyboard('{Escape}');
    await expect.poll(field).toBeNull();
    await expect.poll(() => near(1, 1)).toBeLessThan(0.01);
    expect(near(26, 1)).toBeLessThan(0.01);
  });

  it('opens the folds over an item it picks, ticking the box of its kind where it was not, and selects it', async () => {
    await drawn();
    expect(box('Done').checked).toBe(false);
    // Item 30 is done, and folded away under 27, done too.
    await userEvent.keyboard('/');
    await userEvent.fill(field() as HTMLInputElement, '30');
    await expect.poll(found).toEqual(['#30']);
    await userEvent.keyboard('{Enter}');
    await expect.poll(title).toBe('Draw a fold as a dot');
    expect(box('Done').checked).toBe(true);
    expect(JSON.parse(localStorage.getItem('wf-agents.console-folds') ?? '{}')).toEqual({ 27: 'unfolded' });
    // The pick closed the search and emptied it.
    await expect.poll(field).toBeNull();
    await userEvent.keyboard('/');
    await expect.poll(() => field()?.value).toBe('');
  });

  it('ticks Cancelled for a cancelled item it picks with a click, and selects it', async () => {
    await drawn();
    expect(box('Cancelled').checked).toBe(false);
    await userEvent.keyboard('/');
    await userEvent.fill(field() as HTMLInputElement, 'flat list');
    await expect.poll(found).toEqual(['#35']);
    await userEvent.click(container.querySelector('.console-found-row') as HTMLElement);
    await expect.poll(title).toBe('Try the tree as a flat list');
    expect(box('Cancelled').checked).toBe(true);
  });

  it('fades in what the marks and the keys mean while the pointer is on the question mark at the bottom left, and fades it out once it leaves, taking the pointer no more', async () => {
    await drawn();
    const mark = container.querySelector('.console-help-mark') as HTMLElement;
    const panel = () => container.querySelector('.console-help-panel') as HTMLElement;
    const look = () => getComputedStyle(panel());
    // Hidden at first: no pointer, nothing to read.
    expect(panel().classList.contains('shown')).toBe(false);
    expect(look().pointerEvents).toBe('none');
    expect(look().visibility).toBe('hidden');
    await userEvent.hover(mark);
    await expect.poll(() => panel().classList.contains('shown')).toBe(true);
    expect(look().pointerEvents).toBe('auto');
    expect(look().visibility).toBe('visible');
    // It fades in over the page's short time for what the pointer does, written in seconds or milliseconds.
    const ms = (t: string) => (t.trim().endsWith('ms') ? parseFloat(t) : parseFloat(t) * 1000);
    expect(look().transitionProperty.split(',')[0].trim()).toBe('opacity');
    const hover = getComputedStyle(document.documentElement).getPropertyValue('--hover-time');
    expect(ms(look().transitionDuration.split(',')[0])).toBe(ms(hover));
    expect(ms(hover)).toBeGreaterThan(0);
    await expect.poll(() => look().opacity).toBe('1');
    const listed = (label: string) =>
      [...container.querySelectorAll(`.console-help-panel ul[aria-label="${label}"] > li`)].map((li) => li.textContent);
    expect(listed("The console's keys")).toEqual([
      '/ search',
      'e fold or unfold the card under the pointer',
      'o fold every card with no work under way under it',
      'O unfold every card',
      'n / N next / previous question',
      'm / M next / previous reply you have not read',
      '1 – 9 the tree of the project of that place in the list',
      'Space the whole tree on the screen, and back',
      'Shift + scroll larger or smaller, as a pinch does',
      "v every project's tree at once",
      'Esc close',
    ]);
    const marks = listed("The console's marks");
    expect(marks).toHaveLength(CONSOLE_MARKS.length);
    CONSOLE_MARKS.forEach((key, k) => expect(marks[k]?.endsWith(` ${MARKS[key].name}`)).toBe(true));
    // Every colour of the time track is there as a swatch of its own, saying what it is on pointing at it, the wait for
    // the project's turn to check and merge among them, in a colour no other kind of stretch has.
    const swatch = (kind: string) =>
      container.querySelector(`.console-help-panel ul[aria-label="The console's marks"] .legend-track .us-${kind}`);
    TRACK_KINDS.forEach((kind) => expect(swatch(kind)?.getAttribute('title')).toBe(TRACK_WORDS[kind]));
    expect(swatch('turn')?.getAttribute('title')).toBe('Waiting for its turn to check and merge');
    const colour = (kind: string) => getComputedStyle(swatch(kind) as Element).backgroundColor;
    for (const other of TRACK_KINDS.filter((kind) => kind !== 'turn')) expect(colour('turn')).not.toBe(colour(other));
    // Once the pointer leaves for the tree, above where the panel reaches, it takes the pointer no more at once, and is
    // hidden once it has faded.
    await userEvent.hover(container.querySelector('canvas') as HTMLElement, { position: { x: 4, y: 4 } });
    await expect.poll(() => panel().classList.contains('shown')).toBe(false);
    expect(look().pointerEvents).toBe('none');
    await expect.poll(() => look().visibility).toBe('hidden');
    await expect.poll(() => look().opacity).toBe('0');
  });

  it('lays the legend out across, and keeps it inside the window at every size', async () => {
    await drawn();
    const mark = container.querySelector('.console-help-mark') as HTMLElement;
    const panel = container.querySelector('.console-help-panel') as HTMLElement;
    await userEvent.hover(mark);
    await expect.poll(() => panel.classList.contains('shown')).toBe(true);
    const inside = () => {
      const r = panel.getBoundingClientRect();
      return [r.left >= 0, r.top >= 0, r.right <= window.innerWidth, r.bottom <= window.innerHeight];
    };
    try {
      for (const [width, height] of [
        [1920, 1080],
        [1280, 800],
        [640, 480],
        [400, 300],
      ]) {
        await browserPage.viewport(width, height);
        await expect.poll(() => window.innerWidth).toBe(width);
        expect(inside(), `${width} by ${height}`).toEqual([true, true, true, true]);
      }
      // In a window of the usual size, the marks stand in several columns side by side, and the legend is read whole
      // with no scrolling.
      await browserPage.viewport(1280, 800);
      await expect.poll(() => window.innerWidth).toBe(1280);
      const lefts = [...panel.querySelectorAll('ul[aria-label="The console\'s marks"] > li')].map(
        (li) => li.getBoundingClientRect().left,
      );
      expect(new Set(lefts).size).toBeGreaterThan(1);
      expect(panel.scrollHeight).toBeLessThanOrEqual(panel.clientHeight);
    } finally {
      await browserPage.viewport(1280, 800);
    }
  });

  it('writes the item selected into the address, so that the tab of the console leads back to it', async () => {
    const scene = await drawn();
    const was = window.location.hash;
    try {
      scene.select(12);
      await expect.poll(() => window.location.hash).toBe('#/console/items/12');
      scene.select(2);
      await expect.poll(() => window.location.hash).toBe('#/console/items/2');
      scene.select(null);
      await expect.poll(() => window.location.hash).toBe('#/console');
    } finally {
      history.replaceState(null, '', was === '' ? window.location.pathname : was);
    }
  });

  /// A key pressed on the page, as the owner presses it.
  const key = (k: string, keys: KeyboardEventInit = {}, on: Element = document.body) =>
    on.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true, ...keys }));
  /// The item the address says is selected.
  const inAddress = () => Number(/#\/console\/items\/(\d+)/.exec(window.location.hash)?.[1] ?? Number.NaN);

  it("walks the stops with n and N in the tree's order, an item's own in turn, round from the last to the first and back, but not in a field or with Ctrl", async () => {
    await drawn();
    const was = window.location.hash;
    /// Presses `k` and waits until the item `id` is selected and the page has taken it in.
    const walkTo = async (k: string, id: number) => {
      key(k, k === 'N' ? { shiftKey: true } : {});
      await expect.poll(inAddress).toBe(id);
      await drew();
    };
    try {
      // With Ctrl held, or while the owner types in a field, n goes nowhere.
      key('n', { ctrlKey: true });
      await userEvent.keyboard('/');
      await expect.poll(field).not.toBeNull();
      key('n', {}, field() as HTMLInputElement);
      await drew();
      expect(title()).toBeNull();
      await userEvent.keyboard('{Escape}');
      await expect.poll(field).toBeNull();
      // The items with stops, in the tree's order: 1 (two), 15 under 5 under it, 7, 20 (two), 12, 4 and 25. With
      // nothing selected, n goes to item 1's first stop; then to its second; then to 15's.
      await walkTo('n', 1);
      await walkTo('n', 1);
      await walkTo('n', 15);
      // From 15's first stop, N goes back to item 1's last, then to its first, and from there round to the last stop
      // of all, item 25's; n from there goes round to item 1 again.
      await walkTo('N', 1);
      await walkTo('N', 1);
      await walkTo('N', 25);
      await walkTo('n', 1);
      expect(title()).toBe('Run works without anyone watching');
    } finally {
      history.replaceState(null, '', was === '' ? window.location.pathname : was);
    }
  });

  it('gives the stop n goes to the soft light of a stop chosen, and moves it to another stop clicked on its frame, from which n goes on', async () => {
    await drawn();
    const was = window.location.hash;
    const stops = () => [
      ...container.querySelectorAll<HTMLElement>('.cw-item .cw-choice[data-wait]:not(.cw-choice-done)'),
    ];
    const lit = () => stops().map((s) => s.classList.contains('cw-stop-chosen'));
    try {
      // n goes to item 1's first stop, which stands in the light; its second does not.
      key('n');
      await expect.poll(inAddress).toBe(1);
      await expect.poll(lit).toEqual([true, false]);
      expect(getComputedStyle(stops()[0]).boxShadow).not.toBe('none');
      expect(getComputedStyle(stops()[1]).boxShadow).toBe('none');
      // A click on the second stop's frame, outside its buttons, moves the light there, and n goes on from it, to the
      // first stop of item 15.
      (stops()[1].querySelector('.cw-choice-q') as HTMLElement).click();
      await expect.poll(lit).toEqual([false, true]);
      key('n');
      await expect.poll(inAddress).toBe(15);
    } finally {
      history.replaceState(null, '', was === '' ? window.location.pathname : was);
    }
  });

  it("puts on each of an item's stops but its last a button to the next, saying its place, left of Discuss on a question, and brings the card of a stop gone to onto the screen", async () => {
    const scene = await drawn();
    const was = window.location.hash;
    const nexts = () =>
      [...container.querySelectorAll('.cw-item button')]
        .map((b) => b.textContent ?? '')
        .filter((t) => t.startsWith('Next ('));
    /// The buttons of the last row of each question that is a stop, by their words.
    const feet = () =>
      [...container.querySelectorAll('.cw-item .cw-choice[data-wait]:not(.cw-choice-done)')].map((s) =>
        [...s.querySelectorAll('.cw-choice-foot button.button')].map((b) => b.textContent),
      );
    try {
      // Item 1 asks two questions: the first has Next just left of its Discuss, on its last row; the last, Discuss
      // alone.
      scene.select(1);
      await expect.poll(title).toBe('Run works without anyone watching');
      await expect.poll(feet).toEqual([['Next (1/2)', 'Discuss'], ['Discuss']]);
      // Item 20 waits for an answer and for a ruling: the first of its two stops has the button, the last none.
      scene.select(20);
      await expect.poll(title).toBe('Keep one stream open for each page');
      await expect.poll(nexts).toEqual(['Next (1/2)']);
      // With nothing selected, N goes to the last stop of all, item 25's, and its card is brought onto the screen,
      // within what the window beside the tree leaves of it.
      scene.select(null);
      await expect.poll(title).toBeNull();
      key('N', { shiftKey: true });
      await expect.poll(inAddress).toBe(25);
      await expect.poll(title).toBe('Say which cell a note goes to');
      settle(scene);
      const card = scene.cardAt(25) as { x: number; y: number };
      const { width, height } = scene.place();
      const room = width - (container.querySelector('aside.cw') as HTMLElement).offsetWidth;
      expect(card.x).toBeGreaterThanOrEqual(0);
      expect(card.x + NODE_W).toBeLessThanOrEqual(room);
      expect(card.y).toBeGreaterThan(0);
      expect(card.y).toBeLessThan(height);
    } finally {
      history.replaceState(null, '', was === '' ? window.location.pathname : was);
    }
  });

  it('frames a question that holds the work at nothing yet in orange, as its count on the card, and one that holds it in red', async () => {
    const scene = await drawn();
    // Item 1 waits for the answer to stale-runs; stale-limit, asked as well, holds it at nothing yet.
    const items = await fixtureSource().items.loadItems();
    const one = items.items.find((i) => i.number === 1);
    expect([one?.work.waits?.map((w) => w.key), one?.work.ahead]).toEqual([['answer-stale-runs'], 1]);
    scene.select(1);
    await expect.poll(title).toBe('Run works without anyone watching');
    const frame = (key: string) => container.querySelector(`.cw-choice[data-wait="${key}"]`) as HTMLElement | null;
    await expect.poll(() => frame('answer-stale-limit')).not.toBeNull();
    /// What the page's token `name` resolves to as a colour.
    const colour = (name: string) => {
      const probe = container.appendChild(document.createElement('span'));
      probe.style.color = `var(${name})`;
      const got = getComputedStyle(probe).color;
      probe.remove();
      return got;
    };
    expect(getComputedStyle(frame('answer-stale-limit') as HTMLElement).borderTopColor).toBe(colour('--orange'));
    expect(getComputedStyle(frame('answer-stale-runs') as HTMLElement).borderTopColor).toBe(colour('--red'));
  });

  it('brings the panel out with the item selected, flashes its card at a click on its title, and on Escape lets the item go and folds the panel', async () => {
    const scene = await drawn();
    expect(folded()).toBe(true);
    scene.select(12);
    await expect.poll(title).toBe('Show the cost of a work');
    expect(folded()).toBe(false);
    // Once the tree stands where the panel brought the item, its light is at rest; a click on the title, which leaves
    // the keys where they were, flashes it, stronger at the next frame.
    settle(scene);
    const rest = scene.chosenLight() as { id: number; out: number };
    expect(rest.id).toBe(12);
    (container.querySelector('.drawer-title button') as HTMLButtonElement).click();
    scene.step();
    expect(scene.chosenLight()?.out).toBeGreaterThan(rest.out);
    await userEvent.keyboard('{Escape}');
    await expect.poll(folded).toBe(true);
    expect(title()).toBeNull();
  });

  it('reads again what changed, without moving the tree or closing the panel', async () => {
    const scene = await drawn();
    scene.select(2);
    await expect.poll(() => button('Pause')).not.toBeNull();
    settle(scene);
    const before = scene.place();
    await userEvent.click(button('Pause') as HTMLElement);
    // The work's switch now shows it paused.
    await expect.poll(() => button('Pause')?.getAttribute('aria-pressed')).toBe('true');
    expect(title()).toBe('See every work at one glance');
    expect(folded()).toBe(false);
    const after = scene.place();
    expect(after.left).toBe(before.left);
    expect(after.zoom).toBe(before.zoom);
    expect(after.at).toBeCloseTo(before.at, 9);
  });
});

describe('the console page, with several projects', () => {
  let container: HTMLDivElement;
  let unmount: () => void;
  const forget = () => {
    try {
      localStorage.clear();
    } catch {
      // Nothing kept.
    }
  };
  /// The page over the made-up items of four projects: wf-agents and Whitefoot, Alder made of most of wf-agents'
  /// items again, and Birch, which has no item yet.
  const open = () => {
    container = document.createElement('div');
    container.style.cssText = 'position: fixed; inset: 0; display: flex';
    document.body.append(container);
    delete (window as { consoleScene?: SnapScene }).consoleScene;
    const root = createRoot(container);
    root.render(<ConsolePage source={fixtureSource(Date.now(), 4)} />);
    unmount = () => root.unmount();
  };
  beforeEach(() => {
    forget();
    open();
  });
  afterEach(() => {
    unmount();
    container.remove();
    forget();
  });

  /// The scene shown, once it has drawn its tree.
  const shown = () => (window as { consoleScene?: SnapScene }).consoleScene ?? null;
  async function drawn(): Promise<SnapScene> {
    await expect.poll(() => shown()?.stats.items ?? 0).toBeGreaterThan(0);
    return shown() as SnapScene;
  }
  const rows = () => [...container.querySelectorAll<HTMLButtonElement>('.console-project-name')];
  const names = () => rows().map((r) => r.querySelector('.console-project-title')?.textContent ?? '');
  const current = () => container.querySelector('.console-project-name[aria-current="true"] .console-project-title')
    ?.textContent;
  const key = (k: string) =>
    document.body.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true }));
  const inAddress = () => Number(/#\/console\/items\/(\d+)/.exec(window.location.hash)?.[1] ?? Number.NaN);

  it("lists every project under the map, each after the key that shows its tree and before what waits for the owner in it, and shows a project's tree at its key or a click on its name, keeping it in the browser", async () => {
    await drawn();
    await expect.poll(names).toEqual(['Alder', 'Birch', 'wf-agents', 'Whitefoot']);
    expect(rows().map((r) => r.querySelector('kbd')?.textContent)).toEqual(['1', '2', '3', '4']);
    // What waits for the owner in wf-agents, as the bar counts it: in orange the questions to answer ahead, left of
    // the things that wait, in red; nothing in Birch, which has no item.
    const counts = (name: string) =>
      [...(rows()[names().indexOf(name)]?.querySelectorAll('.bar-count') ?? [])].map((c) => c.className);
    expect(counts('wf-agents')).toEqual(['bar-count ahead', 'bar-count']);
    expect(counts('Birch')).toEqual([]);
    expect(current()).toBe('Alder');
    const first = shown();
    key('3');
    await expect.poll(current).toBe('wf-agents');
    await expect.poll(() => shown() !== first).toBe(true);
    // Of the made-up items of wf-agents, 26 are open or stand over an open one.
    await expect.poll(() => shown()?.stats.items).toBe(26);
    expect(localStorage.getItem('wf-agents.console-project')).toBe('wf-agents');
    // A tree that draws items says nothing of an empty one. With Open unticked the boxes hide every item of the project,
    // and the page says how many and which boxes to tick, each of the kinds the project has; ticked again, it says
    // nothing.
    const said = () => container.querySelector('.console-empty')?.textContent ?? null;
    const box = (name: string) =>
      [...container.querySelectorAll<HTMLLabelElement>('.console-shows label')]
        .find((l) => l.textContent === name)
        ?.querySelector('input') as HTMLInputElement;
    expect(said()).toBeNull();
    const own = (await fixtureSource(Date.now(), 4).items.loadItems()).items.filter((i) => i.project === 'wf-agents');
    const kinds = STANDINGS.filter((s) => own.some((i) => standingOf(i) === s));
    box('Open').click();
    await expect.poll(said).toBe(emptyWords({ kind: 'hidden', count: own.length, unticked: kinds }));
    expect(said()).toMatch(/^All \d+ items are hidden by the boxes: tick Open/);
    box('Open').click();
    await expect.poll(() => shown()?.stats.items).toBe(26);
    expect(said()).toBeNull();
    rows()[names().indexOf('Whitefoot')].click();
    await expect.poll(current).toBe('Whitefoot');
    await expect.poll(() => shown()?.stats.items).toBe(3);
    // The tree of a project with no item yet is empty, and says so.
    key('2');
    await expect.poll(current).toBe('Birch');
    await expect.poll(() => shown()?.stats.items).toBe(0);
    await expect.poll(said).toBe('No items in this project yet');
    // Kept, the project shown is shown again on the next page.
    unmount();
    container.remove();
    open();
    await expect.poll(current).toBe('Birch');
  });

  it('says in words, which the empty tree draws, that no item is there, or that all of them, or the one, are hidden by boxes to tick', () => {
    expect(emptyWords({ kind: 'none' })).toBe('No items in this project yet');
    expect(emptyWords({ kind: 'hidden', count: 3, unticked: ['done', 'cancelled'] })).toBe(
      'All 3 items are hidden by the boxes: tick Done or Cancelled',
    );
    expect(emptyWords({ kind: 'hidden', count: 7, unticked: ['open'] })).toBe('All 7 items are hidden by the boxes: tick Open');
    expect(emptyWords({ kind: 'hidden', count: 4, unticked: ['open', 'done', 'cancelled'] })).toBe(
      'All 4 items are hidden by the boxes: tick Open, Done or Cancelled',
    );
    expect(emptyWords({ kind: 'hidden', count: 1, unticked: ['done'] })).toBe('The item is hidden by the boxes: tick Done');
  });

  it('puts the list in the order a name is dragged into, keeps that order, and gives the keys by it', async () => {
    await drawn();
    await expect.poll(names).toEqual(['Alder', 'Birch', 'wf-agents', 'Whitefoot']);
    const item = (name: string) => rows()[names().indexOf(name)].closest('li') as HTMLLIElement;
    const carried = new DataTransfer();
    item('Whitefoot').dispatchEvent(new DragEvent('dragstart', { dataTransfer: carried, bubbles: true }));
    await expect.poll(() => item('Whitefoot').classList.contains('dragging')).toBe(true);
    item('Alder').dispatchEvent(new DragEvent('dragover', { dataTransfer: carried, bubbles: true, cancelable: true }));
    await expect.poll(names).toEqual(['Whitefoot', 'Alder', 'Birch', 'wf-agents']);
    item('Whitefoot').dispatchEvent(new DragEvent('dragend', { dataTransfer: carried, bubbles: true }));
    expect(JSON.parse(localStorage.getItem('wf-agents.project-order') ?? '[]')).toEqual([
      'Whitefoot',
      'Alder',
      'Birch',
      'wf-agents',
    ]);
    key('1');
    await expect.poll(current).toBe('Whitefoot');
  });

  it("shows every project's tree at once at `v`, each in its place of a grid with its key and its name, the one shown framed apart, its list and boxes gone; Escape or `v` goes back, and a key or a click shows that tree", async () => {
    await drawn();
    const overview = () => container.querySelector('.console-overview:not(.closing)');
    const tiles = () => [...container.querySelectorAll<HTMLButtonElement>('.console-overview-tree')];
    const away = () => container.querySelector('.console-chrome')?.classList.contains('away');
    key('v');
    await expect.poll(overview).not.toBeNull();
    expect(tiles().map((t) => t.textContent)).toEqual(['1Alder', '2Birch', '3wf-agents', '4Whitefoot']);
    expect(container.querySelector('.console-overview-tree.shown')?.textContent).toBe('1Alder');
    expect(away()).toBe(true);
    // Four trees stand two across and two down, the same size, apart.
    const at = tiles().map((t) => t.getBoundingClientRect());
    expect(at[1].top).toBeCloseTo(at[0].top, 3);
    expect(at[2].left).toBeCloseTo(at[0].left, 3);
    expect(at[2].top).toBeGreaterThan(at[0].bottom);
    expect(at[1].left).toBeGreaterThan(at[0].right);
    expect(at[3].width).toBeCloseTo(at[0].width, 3);
    key('Escape');
    await expect.poll(overview).toBeNull();
    expect(away()).toBe(false);
    key('v');
    await expect.poll(overview).not.toBeNull();
    key('v');
    await expect.poll(overview).toBeNull();
    key('v');
    await expect.poll(overview).not.toBeNull();
    key('3');
    await expect.poll(overview).toBeNull();
    await expect.poll(current).toBe('wf-agents');
    key('v');
    await expect.poll(overview).not.toBeNull();
    tiles()[3].click();
    await expect.poll(overview).toBeNull();
    await expect.poll(current).toBe('Whitefoot');
  });

  it('fits the whole tree shown between the list of projects and the window at Space, and brings it back at Space again, opening no grid', async () => {
    const scene = await drawn();
    const overview = () => container.querySelector('.console-overview:not(.closing)');
    const tree = (container.querySelector('.console-canvas') as HTMLElement).getBoundingClientRect();
    const list = (container.querySelector('.console-projects') as HTMLElement).getBoundingClientRect();
    settle(scene);
    const before = scene.place();
    key(' ');
    expect(scene.wholeShown()).toBe(true);
    settle(scene);
    expect(overview()).toBeNull();
    const { zoom } = scene.place();
    const ids = scene.drawnMap().pieces.flatMap((p) => (scene.cardAt(p.id) === null ? [] : [p.id]));
    expect(ids.length).toBeGreaterThan(0);
    for (const id of ids) {
      const c = where(scene, id);
      expect(c.x, `item ${id}`).toBeGreaterThanOrEqual(list.right - tree.left);
      expect(c.x + NODE_W * zoom, `item ${id}`).toBeLessThanOrEqual(tree.width);
      expect(c.y, `item ${id}`).toBeGreaterThan(0);
      expect(c.y, `item ${id}`).toBeLessThan(tree.height);
    }
    key(' ');
    expect(scene.wholeShown()).toBe(false);
    settle(scene);
    expect(scene.place().zoom).toBe(before.zoom);
    expect(overview()).toBeNull();
  });

  it("shows the tree of another project at a click on a shadow of an item of it, and selects the item there", async () => {
    await drawn();
    key('3');
    await expect.poll(current).toBe('wf-agents');
    // Under item 2 of wf-agents, a shadow of item 8, which stands in Whitefoot's tree.
    (shown() as SnapScene).onAway?.(8);
    await expect.poll(current).toBe('Whitefoot');
    await expect.poll(inAddress).toBe(8);
    expect(shown()?.selectedId()).toBe(8);
  });

  it("names the project of each item the search finds, and shows the tree of an item of another project it picks", async () => {
    await drawn();
    key('3');
    await expect.poll(current).toBe('wf-agents');
    await userEvent.keyboard('/');
    const field = () => container.querySelector('.console-find input') as HTMLInputElement | null;
    await expect.poll(field).not.toBeNull();
    await userEvent.fill(field() as HTMLInputElement, '17');
    await expect.poll(() => container.querySelector('.console-found-row .console-found-project')?.textContent).toBe(
      'Whitefoot',
    );
    await userEvent.keyboard('{Enter}');
    await expect.poll(current).toBe('Whitefoot');
    await expect.poll(() => shown()?.selectedId()).toBe(17);
  });

  it('lists the items of no project as a project of their own only while a box ticked shows some of them', async () => {
    await drawn();
    await expect.poll(names).toEqual(['Alder', 'Birch', 'wf-agents', 'Whitefoot']);
    // The one item of no project is done.
    const done = [...container.querySelectorAll<HTMLLabelElement>('.console-shows label')]
      .find((l) => l.textContent === 'Done')
      ?.querySelector('input') as HTMLInputElement;
    done.click();
    await expect.poll(names).toEqual(['Alder', 'Birch', 'wf-agents', 'Whitefoot', 'No project']);
    done.click();
    await expect.poll(names).toEqual(['Alder', 'Birch', 'wf-agents', 'Whitefoot']);
  });
});

describe('the console page, opened at an item the address names', () => {
  it('selects the item once the tree shows it, and brings the panel out with it', async () => {
    const container = document.createElement('div');
    container.style.cssText = 'position: fixed; inset: 0; display: flex';
    document.body.append(container);
    delete (window as { consoleScene?: SnapScene }).consoleScene;
    const root = createRoot(container);
    root.render(<ConsolePage source={fixtureSource()} named={12} />);
    try {
      await expect.poll(() => container.querySelector('.drawer-title')?.textContent ?? null).toBe('Show the cost of a work');
      expect(container.querySelector('aside.cw')?.classList.contains('cw-open')).toBe(true);
    } finally {
      root.unmount();
      container.remove();
    }
  });

  it('ticks the box of the item\'s kind where it is not ticked, and selects the item, as it does for one picked from the search', async () => {
    localStorage.clear();
    const container = document.createElement('div');
    container.style.cssText = 'position: fixed; inset: 0; display: flex';
    document.body.append(container);
    delete (window as { consoleScene?: SnapScene }).consoleScene;
    const root = createRoot(container);
    // Item 35 is cancelled, and Cancelled is not ticked at first.
    root.render(<ConsolePage source={fixtureSource()} named={35} />);
    try {
      await expect.poll(() => container.querySelector('.drawer-title')?.textContent ?? null).toBe('Try the tree as a flat list');
      const cancelled = [...container.querySelectorAll<HTMLLabelElement>('.console-shows label')]
        .find((l) => l.textContent === 'Cancelled')
        ?.querySelector('input');
      expect(cancelled?.checked).toBe(true);
      expect(container.querySelector('aside.cw')?.classList.contains('cw-open')).toBe(true);
    } finally {
      root.unmount();
      container.remove();
      localStorage.clear();
    }
  });
});

describe('the console page, as things happen', () => {
  it('reads the items at most once at a time, and once more after however many things happened meanwhile', async () => {
    const source = fixtureSource();
    const answers: Array<() => void> = [];
    let asked = 0;
    let answered = 0;
    const stream: { opened: (() => void) | null } = { opened: null };
    const container = document.createElement('div');
    container.style.cssText = 'position: fixed; inset: 0; display: flex';
    document.body.append(container);
    delete (window as { consoleScene?: SnapScene }).consoleScene;
    const root = createRoot(container);
    root.render(
      <ConsolePage
        source={{
          ...source,
          items: {
            loadItems: () => {
              asked += 1;
              return new Promise<Items>((answer) => answers.push(() => answer(source.items.loadItems()))).finally(
                () => {
                  answered += 1;
                },
              );
            },
            changes: (_, opened) => {
              stream.opened = opened;
              return () => undefined;
            },
          },
        }}
      />,
    );
    await expect.poll(() => asked).toBe(1);
    for (let i = 0; i < 3; i += 1) stream.opened?.();
    expect(asked).toBe(1);
    answers.shift()?.();
    await expect.poll(() => asked).toBe(2);
    answers.shift()?.();
    // Once the second read has ended, no third is asked for: nothing happened while it was under way.
    await expect.poll(() => answered).toBe(2);
    expect(asked).toBe(2);
    root.unmount();
    container.remove();
  });

  it("reads the items once for the whole page at each change, the console sharing the bar's read", async () => {
    const made = fixtureSource();
    let reads = 0;
    const told = new Set<(c: Changed) => void>();
    // The page's source, whose items the bar reads, and which the console names as what it reads its items from.
    const page: PageSource = {
      ...FIXTURES,
      loadItems: () => {
        reads += 1;
        return made.items.loadItems();
      },
      changes: (onChanged) => {
        told.add(onChanged);
        return () => told.delete(onChanged);
      },
    };
    const container = document.createElement('div');
    container.style.cssText = 'position: fixed; inset: 0; display: flex';
    document.body.append(container);
    delete (window as { consoleScene?: SnapScene }).consoleScene;
    const root = createRoot(container);
    root.render(
      <SourceContext.Provider value={page}>
        <BarCount />
        <ConsolePage source={{ ...made, items: page }} />
      </SourceContext.Provider>,
    );
    try {
      await expect.poll(() => (window as { consoleScene?: SnapScene }).consoleScene?.stats.items ?? 0).toBeGreaterThan(0);
      await expect.poll(() => container.querySelector('.bar-count')?.textContent ?? '0').not.toBe('0');
      expect(reads).toBe(1);
      // A change of a work: the store reads at once, for both.
      for (const on of told) on({ works: [1], items: false, copies: false, accounts: false, observations: false });
      expect(reads).toBe(2);
    } finally {
      root.unmount();
      container.remove();
    }
  });
});

/// How many items the bar read, as its count of what waits reads them.
function BarCount() {
  const { items } = useBarItems();
  return <span className="bar-count">{items?.items.length ?? 0}</span>;
}

describe('the console page, when a work cannot be read', () => {
  it('says so in the panel, and reads it again when the stream opens again', async () => {
    const source = fixtureSource();
    let fails = true;
    let reopened: (() => void) | null = null;
    const container = document.createElement('div');
    container.style.cssText = 'position: fixed; inset: 0; display: flex';
    document.body.append(container);
    delete (window as { consoleScene?: SnapScene }).consoleScene;
    const root = createRoot(container);
    root.render(
      <ConsolePage
        source={{
          ...source,
          loadWork: (n) => (fails ? Promise.reject(new Error(`/api/v1/works/${n} answered 500`)) : source.loadWork(n)),
          changes: (on) => {
            reopened = () => on('everything');
            return () => undefined;
          },
        }}
      />,
    );
    await expect.poll(() => (window as { consoleScene?: SnapScene }).consoleScene?.stats.items ?? 0).toBeGreaterThan(0);
    (window as unknown as { consoleScene: SnapScene }).consoleScene.select(12);
    // Said in the discussion, where the box to write in would stand.
    const said = () => container.querySelector('.drawer-body > .cw-fold > .cw-fold-body > p')?.textContent ?? null;
    await expect.poll(said).toBe('The page could not read the work: /api/v1/works/12 answered 500');
    fails = false;
    (reopened as (() => void) | null)?.();
    // The made-up flow has none of the engineer's cells the box offers, so no box is offered.
    await expect.poll(said).toBe("No cell of this flow takes the owner's words");
    root.unmount();
    container.remove();
  });
});
