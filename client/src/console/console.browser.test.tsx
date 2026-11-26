// The console in a real browser (Chromium, through Vitest's browser mode): the tree is drawn by WebGL, which happy-dom
// does not have, and its motion is the frames the browser draws. These pin what the console does: a scroll over a column moves that column and rests where the scroll stops, as far as
// leaves three rows on the screen; a sideways scroll or a drag moves the columns sideways, no further than leaves half
// of the first or the last column on the screen; a click selects the item under the pointer, and a click on a shadow
// the item it points to, brought to the middle; the tree drawn again stands where it stood, the item selected kept; an
// item revealed comes into the room the panel leaves, or to the middle when it is above or below what is seen; the
// columns beside the one scrolled follow it, each where what joins it to the next near the middle of the screen is
// level, only the way the one it follows moves and never faster than half again; the view stands through a change of height or theme; and
// once destroyed, the scene listens to nothing. The pointer and the wheel are events sent to the canvas at the points
// the test works out from where the columns stand (`place`), so that each lands where the test means; the keys are the
// browser's own. They pin as well: a click on an item's funnel folds what is under it
// into a dot and a click on the dot shows it again, both selecting nothing, the fold turning over its time; the
// console keeps that choice over its own rule; a lit way turns into the cards it joins along its own curve; the way to
// a work the engine runs a call for is lit, as to one whose round runs; and the item selected flashes once when asked,
// as a click on its title in the panel asks, and then draws nothing that stands again while only the lights move.

import '../styles/tokens.css';
import '../styles/parts.css';

import { createRoot } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { userEvent } from 'vitest/browser';

import { DEFAULT_ACCENT, pickAccent, PRESETS, tones } from '../accent';
import { apart, lchOf } from '../colour';
import { MARKS } from '../marks';
import { useBarItems } from '../Engine';
import { NO_LEVEL } from '../works/words';
import type { Changed, Items, Now } from '../works-api';
import { FIXTURES, type Source as PageSource, SourceContext } from '../works/source';
import { fixtureSource } from './fixtures';
import { FIND_SAYS } from './find';
import { CONSOLE_MARKS } from './help';
import { ConsolePage } from './index';
import type { Shown, ShownWork } from './items';
import {
  DIM,
  DOT_AT,
  FLASH_MS,
  FOLD_MS,
  KEEP,
  NODE_W,
  PACE,
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
  unseen: false,
  unread: 0,
  standing: 'open',
  asks: 0,
  ahead: 0,
  deliver: false,
  begun: false,
  shadow: false,
  work: null,
  children,
});
const shadow = (id: number): Shown => ({ ...card(id), shadow: true });
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
    const worked = (id: number, work: Partial<ShownWork>): Shown => ({ ...card(id), work: { ...running(id), ...work } });
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
    expect(said(worked(1, { level: '3', raise: 0, cellLevel: '3', held: { asks: 1, by: 'agents.highest_level' } }))).toEqual({
      title: 'level 3, levels_above 1, agents.highest_level 3',
      cursor: '',
    });
    // Raised, and held by nothing: its keys alone.
    expect(said(worked(2, { level: '3', raise: 1, cellLevel: '4', held: null }))).toEqual({
      title: 'level 3, levels_above 1',
      cursor: '',
    });
    expect(said(worked(3, { level: '2', raise: 0, cellLevel: '2', held: null })).title).toBe('level 2');
    // A work whose engine gives no level: a dash, and why there is none.
    expect(said(worked(4, { level: null, raise: 0, cellLevel: null, held: null })).title).toBe(NO_LEVEL.level);
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
      { ...card(1, [{ ...card(2), begun: true, work: running(2) }]), fold: { folded, under } },
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
    scene.show([card(1, [{ ...card(2), begun: true, work: running(2) }])]);
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

  it('flashes the soft light of the item selected once, in view, without moving the tree, and once it has come to rest draws nothing that stands again while only the lights move', () => {
    scene.show([card(1, [{ ...card(2), begun: true, work: running(2) }])]);
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
    // Flashed, its light is wider and stronger from the next frame, more than twice as wide at its height, and back at
    // rest by the end of the flash's time, the tree unmoved.
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

  it('moves only the light down a way from frame to frame, what stands of the way built once where it stands, and again where it moves', () => {
    scene.show([card(1, [{ ...card(2), begun: true, work: running(2) }])]);
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
    // Moved sideways, the way is built again where it now stands.
    wheel(100, 0);
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

  it('blurs a layer of glow only in a frame in which it draws something', () => {
    scene.show([card(1, [{ ...card(2), begun: true, work: running(2) }])]);
    settle(scene);
    /// How many layers each frame blurs over a whole run of the light and the dark after it.
    const run = () =>
      Array.from({ length: Math.ceil((RUN.beat + RUN.dark) / 16) }, () => {
        scene.step(16);
        return scene.stats.blurs;
      });
    // While the light runs, the glow it lights is blurred; in the dark, with nothing pointed at or selected, nothing is.
    const free = run();
    expect(Math.max(...free)).toBeGreaterThan(0);
    expect(free.filter((b) => b === 0).length).toBeGreaterThanOrEqual(Math.floor(RUN.dark / 16) - 1);
    // The soft light of the item selected is blurred in every frame, the dark's too, until the item is let go.
    scene.select(2);
    expect(Math.min(...run())).toBeGreaterThan(0);
    scene.select(null);
    expect(run().filter((b) => b === 0).length).toBeGreaterThanOrEqual(Math.floor(RUN.dark / 16) - 1);
  });

  it('draws no frame while the page is hidden, nor while its view is, and draws again once both show', async () => {
    scene.show([card(1, [{ ...card(2), begun: true, work: running(2) }])]);
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
    const paused: Now = { kind: 'paused', at: 'implement', by: 'owner', why: null };
    const items: Shown[] = [
      { ...card(1), begun: true, work: running(1) },
      { ...card(2), begun: true, work: { ...running(2), now: call } },
      { ...card(3), begun: true, work: { ...running(3), waits: asks } },
      { ...card(4), begun: true, work: { ...running(4), now: { kind: 'fault', words: 'No session' } } },
      { ...card(5), begun: true, work: { ...running(5), now: paused, paused: 'owner', from: now } },
      card(6),
      { ...card(7), standing: 'done' },
      { ...card(8), standing: 'cancelled' },
    ];
    scene.show(items);
    settle(scene);
    const colour = new Map(scene.drawnMap().pieces.map((p) => [p.id, p.colour]));
    const p = themed();
    expect([1, 2, 3, 4, 5, 6, 7, 8].map((id) => colour.get(id))).toEqual([
      p.running,
      p.calling,
      p.halted,
      p.halted,
      p.paused,
      p.pale,
      p.hair,
      p.hair,
    ]);
  });

  it("frames on the map the part of the tree the screen shows, round the middle where each column's place is, after a scroll and a pan", () => {
    // Thirty roots in the first column, the first with an item under it and one under that: three columns.
    const roots = Array.from({ length: 30 }, (_, k) => card(k + 1, k === 0 ? [card(100, [card(101)])] : []));
    scene.show(roots);
    settle(scene);
    const barAt = (d: number) => 8 + d * (12 + 5);
    /// The frame drawn holds, round the map's middle, the pieces of the first column's items whose cards are on the
    /// screen, and stands over the columns from `first` to `last`.
    const holds = (first: number, last: number) => {
      const { frame, pieces } = scene.drawnMap();
      if (frame === null) throw new Error('no frame on the map');
      expect(frame.x).toBeCloseTo(barAt(first) - 2, 6);
      expect(frame.x + frame.width).toBeCloseTo(barAt(last) + 12 + 2, 6);
      expect(frame.y + frame.height / 2).toBeCloseTo(72 / 2, 6);
      for (const piece of pieces.filter((q) => q.column === 0)) {
        const stands = scene.cardAt(piece.id) as { y: number };
        const middle = piece.y + piece.height / 2;
        const inside = middle > frame.y && middle < frame.y + frame.height;
        if (stands.y > ROW && stands.y < HEIGHT - ROW) expect(inside).toBe(true);
        if (stands.y < -ROW || stands.y > HEIGHT + ROW) expect(inside).toBe(false);
      }
    };
    holds(0, 2);
    // A scroll over the first column moves its pieces under the frame, which stays round the middle.
    move(50, line);
    wheel(0, 8 * ROW);
    settle(scene);
    holds(0, 2);
    // A pan that takes the first column off the screen takes the frame off its bar.
    wheel(PITCH + 100, 0);
    settle(scene);
    holds(1, 2);
  });

  it('draws a card whose work halted before its first round in red, as one that halted after it, with no second row', () => {
    const fault: Now = { kind: 'fault', words: 'No session' };
    const halted = (k: number, from: number | null): Shown => ({ ...card(k), work: { ...running(k), now: fault, from } });
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
    const stopped = (k: number, now: Now): Shown => ({ ...card(k), work: { ...running(k), now, from: null } });
    scene.show([stopped(1, fault), stopped(2, broken), card(3)]);
    const [halted, unworked, plain] = [1, 2, 3].map((id) => scene.cardLook(id));
    expect([unworked?.fill, unworked?.edge]).toEqual([halted?.fill, halted?.edge]);
    expect(unworked?.fill).not.toBe(plain?.fill);
    expect(unworked?.edge).not.toBe(plain?.edge);
    expect(unworked?.rows).toBe(1);
  });

  it('lights the way to a work the engine runs a call for, as to one whose round runs, and to no work that waits for the owner or is paused', () => {
    const call: Now = {
      kind: 'waits',
      at: 'implement',
      on: [{ kind: 'call', call: 1, who: null, what: { kind: 'tool', name: 'ci', input: 'the tests' } }],
    };
    const paused: Now = { kind: 'paused', at: 'implement', by: 'owner', why: null };
    const asks = [{ key: 'answer-which', why: 'Which?', ends: { kind: 'resume' as const } }];
    const under = (k: number, work: ShownWork) => ({ ...card(k), begun: true, work });
    // Under 1: 2 runs a round, 3 waits for the engine's call, 4 waits for the owner, and 5 is paused.
    const kids = [
      under(2, running(2)),
      under(3, { ...running(3), now: call }),
      under(4, { ...running(4), waits: asks }),
      under(5, { ...running(5), now: paused, paused: 'owner' }),
    ];
    scene.show([card(1, kids)]);
    settle(scene);
    expect(scene.litWays().map((w) => w.id).sort()).toEqual([2, 3]);
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
    const two = { ...card(2), begun: true, work: running(2) };
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
    // The shadow is the only item of the second column, its middle half a row from the column's top.
    const p = scene.place();
    click(p.left + PITCH + 100, line + (0.5 - p.columns[1].at) * ROW);
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
    click(p.left + PITCH + 100, line + (1.5 - p.columns[1].at) * ROW);
    expect(told).toHaveBeenLastCalledWith(expect.objectContaining({ id: 3, shadow: false }));
    expect(scene.place().driver).toBe(1);
    settle(scene);
    // Item 3 is the third of its column, after the room between the items of 1 and those of 4: 2 + 0.7 + 0.5 rows.
    expect(scene.place().columns[1].at).toBeCloseTo(3.2, 3);
  });

  it("tells of a click on a shadow of an item this tree does not hold, which another project's tree holds, and selects nothing", async () => {
    const told = vi.fn<(item: Shown | null) => void>();
    const away = vi.fn<(id: number) => void>();
    // Under 1, a shadow of 9, which no fold of this tree hides.
    scene.show([card(1, [shadow(9)])]);
    settle(scene);
    scene.onSelect = told;
    scene.onAway = away;
    const p = scene.place();
    // The shadow is the only item of the second column, its middle half a row from the column's top.
    click(p.left + PITCH + 100, line + (0.5 - p.columns[1].at) * ROW);
    expect(away).toHaveBeenLastCalledWith(9);
    expect(told).not.toHaveBeenCalled();
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

  it('never moves the column beside the one scrolled against it where the items have none to three under them', () => {
    // As the items near the middle change, the wants on the second column pull it now one way, now the other.
    const under = [3, 2, 2, 3, 1, 0, 0, 2, 3, 1, 0, 2];
    scene.show(under.map((n, k) => card(k + 1, Array.from({ length: n }, (_, j) => card(100 + 4 * k + j)))));
    settle(scene);
    move(50, line);
    for (const way of [1, -1, 1, -1]) {
      for (let k = 0; k < 200; k += 1) {
        const was = scene.place().columns.map((c) => c.at);
        wheel(0, way * 0.1 * ROW);
        scene.step();
        const now = scene.place().columns.map((c) => c.at);
        const moved = [now[0] - was[0], now[1] - was[1]];
        expect(moved[1] * Math.sign(moved[0])).toBeGreaterThanOrEqual(-1e-9);
        if (moved[0] === 0) expect(moved[1]).toBe(0);
      }
    }
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
      'The items not done or cancelled: running, paused or waiting',
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
      'n / N next / previous question',
      '1 – 9 the tree of the project of that place in the list',
      "Space every project's tree at once",
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
    // Once the tree stands where the panel brought the item, its soft light is at rest; a click on the title, which
    // leaves the keys where they were, flashes it, wider at the next frame.
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
    after.columns.forEach((c, d) => expect(Math.abs(c.at - before.columns[d].at)).toBeLessThan(1e-9));
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
    rows()[names().indexOf('Whitefoot')].click();
    await expect.poll(current).toBe('Whitefoot');
    await expect.poll(() => shown()?.stats.items).toBe(3);
    // The tree of a project with no item yet is empty.
    key('2');
    await expect.poll(current).toBe('Birch');
    await expect.poll(() => shown()?.stats.items).toBe(0);
    // Kept, the project shown is shown again on the next page.
    unmount();
    container.remove();
    open();
    await expect.poll(current).toBe('Birch');
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

  it("shows every project's tree at once on Space, each in its place of a grid with its key and its name, the one shown framed apart, its list and boxes gone; Escape or Space goes back, and a key or a click shows that tree", async () => {
    await drawn();
    const overview = () => container.querySelector('.console-overview:not(.closing)');
    const tiles = () => [...container.querySelectorAll<HTMLButtonElement>('.console-overview-tree')];
    const away = () => container.querySelector('.console-chrome')?.classList.contains('away');
    key(' ');
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
    key(' ');
    await expect.poll(overview).not.toBeNull();
    key(' ');
    await expect.poll(overview).toBeNull();
    key(' ');
    await expect.poll(overview).not.toBeNull();
    key('3');
    await expect.poll(overview).toBeNull();
    await expect.poll(current).toBe('wf-agents');
    key(' ');
    await expect.poll(overview).not.toBeNull();
    tiles()[3].click();
    await expect.poll(overview).toBeNull();
    await expect.poll(current).toBe('Whitefoot');
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
