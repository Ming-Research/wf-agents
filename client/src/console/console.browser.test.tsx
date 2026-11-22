// The console in a real browser (Chromium, through Vitest's browser mode): the tree is drawn by WebGL, which happy-dom
// does not have, and its motion is the frames the browser draws. These pin what the console on `old-main` did, which
// had no tests of its own for it: a scroll over a column moves that column and rests where the scroll stops, as far as
// leaves three rows on the screen; a sideways scroll or a drag moves the columns sideways, no further than leaves half
// of the first or the last column on the screen; a click selects the item under the pointer, and a click on a shadow
// the item it points to, brought to the middle; the tree drawn again stands where it stood, the item selected kept; an
// item revealed comes into the room the panel leaves, or to the middle when it is above or below what is seen; the
// columns beside the one scrolled follow it, each where what joins it to the next near the middle of the screen is
// level, and never faster than half again the one it follows; the view stands through a change of height or theme; and
// once destroyed, the scene listens to nothing. The pointer and the wheel are events sent to the canvas at the points
// the test works out from where the columns stand (`place`), so that each lands where the test means; the keys are the
// browser's own. They pin as well what the old console did not do: a click on an item's funnel folds what is under it
// into a dot and a click on the dot shows it again, both selecting nothing, the fold turning over its time; and the
// console keeps that choice over its own rule.

import '../styles/tokens.css';
import '../styles/parts.css';

import { createRoot } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { userEvent } from 'vitest/browser';

import { DEFAULT_ACCENT, pickAccent, PRESETS, tones } from '../accent';
import { apart, lchOf } from '../colour';
import { MARKS } from '../marks';
import type { Items } from '../works-api';
import { fixtureSource } from './fixtures';
import { FIND_SAYS } from './find';
import { CONSOLE_MARKS } from './help';
import type { Heard } from './source';
import { ConsolePage } from './index';
import type { Shown } from './items';
import { DOT_AT, FOLD_MS, KEEP, NODE_W, PACE, PITCH, ROW, SnapScene, themed, UNFOLD_MS } from './snap';

const WIDTH = 1000;
const HEIGHT = 640;

/// An item with no work, and the items under it.
const card = (id: number, children: Shown[] = []): Shown => ({
  id,
  title: `Item ${id}`,
  unseen: false,
  unread: 0,
  standing: 'open',
  asks: 0,
  deliver: false,
  blocked: 0,
  begun: false,
  shadow: false,
  work: null,
  children,
});
const shadow = (id: number): Shown => ({ ...card(id), shadow: true });

const frame = () => new Promise<void>((done) => requestAnimationFrame(() => done()));
/// Two frames: one in which the scene has surely drawn, whichever of its frame and the test's came first.
const drew = async () => {
  await frame();
  await frame();
};

/// Waits until the columns stand still: three frames in a row in which nothing moved and no fold turned, which they
/// reach within a second of frames of what moved them. The bound is in frames, which the scene moves by, not in time,
/// which a busy machine stretches.
function settle(scene: SnapScene) {
  let was = JSON.stringify(scene.place());
  let still = 0;
  for (let k = 0; k < 60 && still < 3; k += 1) {
    scene.step();
    const now = JSON.stringify(scene.place());
    still = now === was && scene.place().turns === 0 ? still + 1 : 0;
    was = now;
  }
  expect(still, 'the columns still moved 60 frames on').toBe(3);
}

describe('the tree in columns', () => {
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
  /// A click where the pointer is, after a frame that finds the item under it.
  async function click(x: number, y: number) {
    move(x, y);
    await drew();
    canvas().dispatchEvent(new PointerEvent('pointerdown', at(x, y)));
    canvas().dispatchEvent(new PointerEvent('pointerup', at(x, y)));
  }
  /// The middle of the screen, where each column's place is.
  const line = HEIGHT / 2;

  it('moves the column scrolled over with the scroll, and no further than leaves three of its rows on the screen', async () => {
    scene.show(Array.from({ length: 30 }, (_, k) => card(k + 1)));
    // At first the middle item of the roots stands in the middle: rank 14, its middle 14.5 rows from the column's top.
    expect(scene.place().columns[0].at).toBe(14.5);
    move(50, line);
    wheel(0, 3 * ROW);
    settle(scene);
    expect(scene.place().columns[0].at).toBeCloseTo(17.5, 3);
    const room = HEIGHT / 2 / ROW - KEEP;
    wheel(0, 100_000);
    settle(scene);
    expect(scene.place().columns[0].at).toBeCloseTo(29.5 + room, 3);
    wheel(0, -100_000);
    settle(scene);
    expect(scene.place().columns[0].at).toBeCloseTo(0.5 - room, 3);
  });

  it('moves the columns sideways by a sideways scroll or a drag, as far as leaves half of the first or the last on the screen', async () => {
    scene.show([card(1, [card(2, [card(3)])])]);
    expect(scene.place().left).toBe(40);
    move(50, line);
    wheel(100, 0);
    await drew();
    expect(scene.place().left).toBe(-60);
    wheel(100_000, 0);
    await drew();
    expect(scene.place().left).toBe(-2 * PITCH - NODE_W / 2);
    // A drag: the press on the canvas, the moves anywhere in the window.
    canvas().dispatchEvent(new PointerEvent('pointerdown', at(500, line)));
    window.dispatchEvent(new PointerEvent('pointermove', at(700, line)));
    canvas().dispatchEvent(new PointerEvent('pointerup', at(700, line)));
    await drew();
    expect(scene.place().left).toBe(-2 * PITCH - NODE_W / 2 + 200);
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
    await click(x, line);
    expect(told).toHaveBeenLastCalledWith(expect.objectContaining({ id: 2 }));
    await click(x, line);
    expect(told).toHaveBeenLastCalledWith(null);
    await click(x, line - ROW);
    expect(told).toHaveBeenLastCalledWith(expect.objectContaining({ id: 1 }));
    await click(x, HEIGHT - 20);
    expect(told).toHaveBeenLastCalledWith(null);
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
    await click(left + NODE_W + (PITCH - NODE_W) / 2, line);
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
    await click(left + NODE_W + DOT_AT, line);
    expect(turned).toHaveBeenLastCalledWith(1, false);
    expect(told).not.toHaveBeenCalled();
    expect(canvas().title).toBe('Show the items under it (e): open 1');
    scene.show(tree(false));
    expect(scene.stats.items).toBe(2);
    expect(scene.place().turns).toBe(1);
    settle(scene);
    expect(scene.place().turns).toBe(0);
    // A click on the card selects it and folds nothing; past the edge of an item with nothing under it, nothing folds.
    await click(left + 100, line);
    expect(told).toHaveBeenLastCalledWith(expect.objectContaining({ id: 1 }));
    scene.show([card(1)]);
    settle(scene);
    await click(left + NODE_W + DOT_AT, line);
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
    await click(left + NODE_W + 4, line);
    expect(turned).toHaveBeenLastCalledWith(1, true);
    expect(canvas().title).toBe('Fold the items under it away (e): 1 shown elsewhere');
    scene.show(tree(true));
    settle(scene);
    expect(scene.stats.items).toBe(3);
    await click(left + NODE_W + DOT_AT, line);
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
    const now = Date.now();
    const work: Shown['work'] = {
      number: 2,
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
      lines: null,
    };
    const under = { open: 1, done: 0, cancelled: 0, elsewhere: 0 };
    // Under 1, item 2, whose work runs.
    const tree = (folded: boolean) => [{ ...card(1, [{ ...card(2), begun: true, work }]), fold: { folded, under } }];
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
    // The shadow is the only item of the second column, its middle half a row from the column's top.
    const p = scene.place();
    await click(p.left + PITCH + 100, line + (0.5 - p.columns[1].at) * ROW);
    expect(turned).toHaveBeenLastCalledWith(1, false);
    expect(told).not.toHaveBeenCalled();
    scene.show(tree(false));
    expect(told).toHaveBeenLastCalledWith(expect.objectContaining({ id: 2, shadow: false }));
    // Once it has grown out of the dot, it comes to the middle.
    settle(scene);
    expect(scene.place().driver).toBe(1);
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
    const p = scene.place();
    // The shadow is the second of its column, its middle 1.5 rows from the column's top.
    await click(p.left + PITCH + 100, line + (1.5 - p.columns[1].at) * ROW);
    expect(told).toHaveBeenLastCalledWith(expect.objectContaining({ id: 3, shadow: false }));
    expect(scene.place().driver).toBe(1);
    settle(scene);
    // Item 3 is the third of its column, after the room between the items of 1 and those of 4: 2 + 0.7 + 0.5 rows.
    expect(scene.place().columns[1].at).toBeCloseTo(3.2, 3);
  });

  it('draws new items where the tree stood, by the item nearest the middle, and keeps the item selected', async () => {
    const told = vi.fn<(item: Shown | null) => void>();
    scene.onSelect = told;
    scene.show([card(1), card(2), card(3)]);
    scene.select(2);
    settle(scene);
    told.mockClear();
    expect(scene.place().columns[0].at).toBe(1.5);
    scene.show([card(0), card(1), card(2), card(3)]);
    // Item 2 is a row further down its column, and the column a row further, so that it stands where it stood.
    expect(scene.place().columns[0].at).toBe(2.5);
    expect(told).toHaveBeenLastCalledWith(expect.objectContaining({ id: 2 }));
    expect(told).not.toHaveBeenCalledWith(null);
  });

  it('brings an item revealed into the room the panel leaves, a little in from its side', async () => {
    scene.show([card(1, [card(2, [card(3, [card(4)])])])]);
    settle(scene);
    scene.reveal(4, 600);
    settle(scene);
    expect(Math.abs(scene.place().left - (600 - 24 - NODE_W - 3 * PITCH))).toBeLessThan(0.5);
  });

  /// Ten items, each with two under it: the second column's items stand in pairs, 0.7 rows apart.
  const pairs = () => Array.from({ length: 10 }, (_, k) => card(k + 1, [card(100 + 2 * k), card(101 + 2 * k)]));
  /// Where an item of the second column starts, in rows from the column's top.
  const slot1 = (rank: number) => rank + 0.7 * Math.floor(rank / 2);

  it('places a column where its links put it, level with the item in the middle, when it is first drawn', async () => {
    scene.show(pairs());
    const p = scene.place();
    // Item 5 stands in the middle; the two under it are ranks 8 and 9 of the second column.
    expect(p.columns[0].at).toBe(4.5);
    expect(p.columns[1].at).toBeGreaterThanOrEqual(slot1(8) - 1e-9);
    expect(p.columns[1].at).toBeLessThanOrEqual(slot1(9) + 1 + 1e-9);
  });

  it('leaves the column beside the one scrolled where it is while every want is met, and an item far out wants little', async () => {
    // One item with ten under it: it wants the column beside it anywhere along the ten, and the further it is from the
    // middle, the more room it gives.
    scene.show([card(1, Array.from({ length: 10 }, (_, k) => card(100 + k)))]);
    settle(scene);
    const before = scene.place();
    expect(before.columns[0].at).toBe(0.5);
    expect(before.columns[1].at).toBe(5);
    move(50, line);
    wheel(0, 0.2 * ROW);
    settle(scene);
    expect(scene.place().columns[0].at).toBeCloseTo(0.7, 6);
    expect(scene.place().columns[1].at).toBe(5);
    // Six rows off the middle, the item's want is loosened by six rows and more: still met.
    wheel(0, 5.8 * ROW);
    settle(scene);
    expect(scene.place().columns[0].at).toBeCloseTo(6.5, 6);
    expect(scene.place().columns[1].at).toBe(5);
  });

  it('brings the column beside the one scrolled after it, the same way, never faster than half again its pace, and stops it when the scroll stops', async () => {
    scene.show(pairs());
    settle(scene);
    const start = scene.place().columns.map((c) => c.at);
    move(50, line);
    wheel(0, 3 * ROW);
    let was = start;
    let still = 0;
    for (let k = 0; k < 400 && still < 3; k += 1) {
      await frame();
      const now = scene.place().columns.map((c) => c.at);
      const moved = [now[0] - was[0], now[1] - was[1]];
      // Each frame, the second column moves the way the first does, no further than half again as far.
      expect(moved[1]).toBeGreaterThanOrEqual(-1e-9);
      expect(Math.abs(moved[1])).toBeLessThanOrEqual(PACE * Math.abs(moved[0]) + 1e-9);
      still = moved[0] === 0 && moved[1] === 0 ? still + 1 : 0;
      was = now;
    }
    expect(was[0]).toBeCloseTo(7.5, 3);
    expect(was[1] - start[1]).toBeGreaterThan(0.5);
    expect(was[1] - start[1]).toBeLessThanOrEqual(PACE * 3 + 1e-6);
  });

  it('makes the column scrolled the one the others follow, and brings the first column after it', async () => {
    scene.show(pairs());
    settle(scene);
    const start = scene.place().columns.map((c) => c.at);
    expect(scene.place().driver).toBe(0);
    move(scene.place().left + PITCH + 50, line);
    wheel(0, 6 * ROW);
    await drew();
    expect(scene.place().driver).toBe(1);
    settle(scene);
    const p = scene.place();
    // The column scrolled goes where the scroll took it; the first follows it down, no further than half again.
    expect(p.columns[1].at).toBeCloseTo(start[1] + 6, 3);
    expect(p.columns[0].at - start[0]).toBeGreaterThan(0.5);
    expect(p.columns[0].at - start[0]).toBeLessThanOrEqual(PACE * 6 + 1e-6);
    // A scroll over the first column makes it the one followed again.
    move(50, line);
    wheel(0, ROW);
    await drew();
    expect(scene.place().driver).toBe(0);
  });

  it('moves nothing on a wheel over no column, or once the pointer has left', async () => {
    scene.show([card(1), card(2)]);
    settle(scene);
    const before = scene.place();
    // Right of the only column.
    move(before.left + PITCH + 100, line);
    wheel(0, 500);
    await drew();
    expect(scene.place()).toEqual(before);
    move(50, line);
    canvas().dispatchEvent(new PointerEvent('pointerleave', at(50, line)));
    wheel(0, 500);
    await drew();
    expect(scene.place()).toEqual(before);
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
    expect(scene.place().columns[0].at).toBe(14.5);
    scene.reveal(30, WIDTH);
    settle(scene);
    expect(scene.place().columns[0].at).toBeCloseTo(29.5, 3);
  });

  it('keeps the view where it stands when the height changes, and when the theme does', async () => {
    const roots = pairs();
    scene.show(roots);
    move(50, line);
    wheel(0, 2 * ROW);
    wheel(80, 0);
    settle(scene);
    const before = scene.place();
    host.style.height = `${HEIGHT - 140}px`;
    await expect.poll(() => scene.place().height).toBe(HEIGHT - 140);
    settle(scene);
    const shorter = scene.place();
    expect(shorter.left).toBe(before.left);
    shorter.columns.forEach((c, d) => expect(Math.abs(c.at - before.columns[d].at)).toBeLessThan(1e-9));
    document.documentElement.setAttribute('data-theme', 'dark');
    await drew();
    settle(scene);
    const dark = scene.place();
    expect(dark.left).toBe(before.left);
    dark.columns.forEach((c, d) => expect(Math.abs(c.at - before.columns[d].at)).toBeLessThan(1e-9));
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
});

describe('the console page', () => {
  let container: HTMLDivElement;
  let unmount: () => void;
  beforeEach(() => {
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
    try {
      localStorage.clear();
    } catch {
      // Nothing kept.
    }
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
      'The items not done or cancelled: running, paused or waiting',
      'The items whose work merged',
      'The items whose work was cancelled',
    ]);
    expect(['Open', 'Done', 'Cancelled'].map((n) => box(n).checked)).toEqual([true, false, false]);
    // Of the made-up items, 28 are open or stand over an open one. Done shows 4 more: two done; one done, the two done
    // under it folded away; and one cancelled, the one done under it folded away. Cancelled shows 2 more. And 12 are
    // done or cancelled or stand over one that is, what is folded away not drawn.
    expect(scene.stats.items).toBe(28);
    box('Done').click();
    await expect.poll(() => scene.stats.items).toBe(32);
    box('Cancelled').click();
    await expect.poll(() => scene.stats.items).toBe(34);
    box('Open').click();
    await expect.poll(() => scene.stats.items).toBe(12);
    expect(localStorage.getItem('wf-agents.console-shows')).toBe('done,cancelled');
  });

  it('folds what has no open item in it into a dot, shows it at a click on the dot, and keeps that over the rule for the next page', async () => {
    let scene = await drawn();
    const done = [...container.querySelectorAll<HTMLLabelElement>('.console-shows label')]
      .find((l) => l.textContent === 'Done')
      ?.querySelector('input') as HTMLInputElement;
    done.click();
    await expect.poll(() => scene.stats.items).toBe(32);
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
    await expect.poll(turnedItems).toBe(34);
    expect(JSON.parse(localStorage.getItem('wf-agents.console-folds') ?? '')).toEqual({ 27: 'unfolded' });
    unmount();
    delete (window as { consoleScene?: SnapScene }).consoleScene;
    const root = createRoot(container);
    root.render(<ConsolePage source={fixtureSource()} />);
    unmount = () => root.unmount();
    scene = await drawn();
    await expect.poll(() => scene.stats.items).toBe(34);
    // Shown as the owner chose, its funnel folds them away again.
    await press(27, 4);
    await expect.poll(turnedItems).toBe(32);
    expect(JSON.parse(localStorage.getItem('wf-agents.console-folds') ?? '')).toEqual({ 27: 'folded' });
  });

  it('folds and unfolds the items under the card the pointer is over at `e`, and does nothing over a card with none, while the owner types in a field, or with Ctrl held', async () => {
    const scene = await drawn();
    const done = [...container.querySelectorAll<HTMLLabelElement>('.console-shows label')]
      .find((l) => l.textContent === 'Done')
      ?.querySelector('input') as HTMLInputElement;
    done.click();
    await expect.poll(() => scene.stats.items).toBe(32);
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
    await expect.poll(turnedItems).toBe(34);
    expect(JSON.parse(kept() ?? '')).toEqual({ 27: 'unfolded' });
    await over(27);
    press(document.body);
    await expect.poll(turnedItems).toBe(32);
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
    expect(turnedItems()).toBe(32);
    expect(JSON.parse(kept() ?? '')).toEqual({ 27: 'folded' });
  });

  /// The search's field, and the paths of the rows it finds.
  const field = () => container.querySelector('.console-find input') as HTMLInputElement;
  const found = () => [...container.querySelectorAll('.console-found-row > .mono')].map((s) => s.textContent);
  /// The box of a kind of item over the tree.
  const box = (name: string) =>
    [...container.querySelectorAll<HTMLLabelElement>('.console-shows label')]
      .find((l) => l.textContent === name)
      ?.querySelector('input') as HTMLInputElement;

  it('gives the keys to the search at `/`, finds by number, by path and by a part of the title, and empties at Escape', async () => {
    const scene = await drawn();
    await userEvent.keyboard('/');
    expect(document.activeElement).toBe(field());
    expect(field().value).toBe('');
    // By its number: item 27, done, shown by its path under 26, its root.
    await userEvent.keyboard('27');
    await expect.poll(found).toEqual(['26-27']);
    expect(container.querySelector('.console-found-row .console-found-standing')?.textContent).toBe('Done');
    // By a path from its root: the item it names, and the items under it.
    await userEvent.clear(field());
    await userEvent.keyboard('26-27');
    await expect.poll(found).toEqual(['26-27', '26-27-30', '26-27-31']);
    // As a path is typed a digit at a time: `26-2` names 26-27, 26-28 and 26-29, each with the items under it.
    await userEvent.clear(field());
    await userEvent.keyboard('26-2');
    await expect.poll(found).toEqual([
      '26-27',
      '26-27-30',
      '26-27-31',
      '26-28',
      '26-28-32',
      '26-29',
      '26-29-33',
      '26-29-34',
    ]);
    // The list's frame reaches past the right edge of the tree's first column, so that no card shows beside it.
    const frame = (container.querySelector('.console-found') as HTMLElement).getBoundingClientRect();
    const canvas = (container.querySelector('canvas') as HTMLCanvasElement).getBoundingClientRect();
    expect(frame.right).toBeGreaterThanOrEqual(canvas.left + scene.place().left + NODE_W);
    // Nothing found is said in the list's own frame, on its ground.
    await userEvent.clear(field());
    await userEvent.keyboard('nowhere at all');
    await expect.poll(() => container.querySelector('.console-found > .console-found-none')?.textContent).toBe(
      FIND_SAYS.none,
    );
    // By a part of the title, in any case, over every item, done or not.
    await userEvent.clear(field());
    await userEvent.keyboard('FOLD');
    await expect.poll(found).toEqual(['26', '26-27', '26-27-30', '26-27-31']);
    // Escape empties the field and gives the keys back.
    await userEvent.keyboard('{Escape}');
    expect(field().value).toBe('');
    expect(document.activeElement).not.toBe(field());
    expect(container.querySelector('.console-found')).toBeNull();
  });

  it('opens the folds over an item it picks, ticking the box of its kind where it was not, and selects it', async () => {
    await drawn();
    expect(box('Done').checked).toBe(false);
    // Item 30 is done, and folded away under 27, done too.
    await userEvent.keyboard('/');
    await userEvent.keyboard('26-27-30');
    await expect.poll(found).toEqual(['26-27-30']);
    await userEvent.keyboard('{Enter}');
    await expect.poll(title).toBe('Draw a fold as a dot');
    expect(box('Done').checked).toBe(true);
    expect(JSON.parse(localStorage.getItem('wf-agents.console-folds') ?? '{}')).toEqual({ 27: 'unfolded' });
    expect(field().value).toBe('');
  });

  it('ticks Cancelled for a cancelled item it picks with a click, and selects it', async () => {
    await drawn();
    expect(box('Cancelled').checked).toBe(false);
    await userEvent.keyboard('/');
    await userEvent.keyboard('flat list');
    await expect.poll(found).toEqual(['35']);
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
      'Esc close',
    ]);
    const marks = listed("The console's marks");
    expect(marks).toHaveLength(CONSOLE_MARKS.length);
    CONSOLE_MARKS.forEach((key, k) => expect(marks[k]?.endsWith(` ${MARKS[key].name}`)).toBe(true));
    // Once the pointer leaves, it takes the pointer no more at once, and is hidden once it has faded.
    await userEvent.unhover(mark);
    await expect.poll(() => panel().classList.contains('shown')).toBe(false);
    expect(look().pointerEvents).toBe('none');
    await expect.poll(() => look().visibility).toBe('hidden');
    await expect.poll(() => look().opacity).toBe('0');
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

  it('brings the panel out with the item selected, and on Escape lets the item go and folds the panel', async () => {
    const scene = await drawn();
    expect(folded()).toBe(true);
    scene.select(12);
    await expect.poll(title).toBe('Show the cost of a work');
    expect(folded()).toBe(false);
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
    after.columns.forEach((c, d) => expect(Math.abs(c.at - before.columns[d].at)).toBeLessThan(1e-9));
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
});

describe('the console page, as things happen', () => {
  it('reads the items at most once at a time, and once more after however many things happened meanwhile', async () => {
    const source = fixtureSource();
    const answers: Array<() => void> = [];
    let asked = 0;
    let answered = 0;
    const stream: { on: ((heard: Heard) => void) | null } = { on: null };
    const container = document.createElement('div');
    container.style.cssText = 'position: fixed; inset: 0; display: flex';
    document.body.append(container);
    delete (window as { consoleScene?: SnapScene }).consoleScene;
    const root = createRoot(container);
    root.render(
      <ConsolePage
        source={{
          ...source,
          loadItems: () => {
            asked += 1;
            return new Promise<Items>((answer) => answers.push(() => answer(source.loadItems()))).finally(() => {
              answered += 1;
            });
          },
          changes: (on) => {
            stream.on = on;
            return () => undefined;
          },
        }}
      />,
    );
    await expect.poll(() => asked).toBe(1);
    for (let i = 0; i < 3; i += 1) stream.on?.('everything');
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
});

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
