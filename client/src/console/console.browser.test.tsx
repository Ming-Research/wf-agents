// The console in a real browser (Chromium, through Vitest's browser mode): the tree is drawn by WebGL, which happy-dom
// does not have, and its motion is the frames the browser draws. These pin what the console on `main` did, which had no
// tests of its own for it: a scroll over a column moves that column and rests where the scroll stops, as far as leaves
// three rows on the screen; a sideways scroll or a drag moves the columns sideways, no further than leaves half of the
// first or the last column on the screen; a click selects the item under the pointer, and a click on a shadow the item
// it points to, brought to the middle; the tree drawn again stands where it stood, the item selected kept; an item
// revealed comes into the room the panel leaves, or to the middle when it is above or below what is seen; the columns
// beside the one scrolled follow it, each where what joins it to the next near the middle of the screen is level, and
// never faster than half again the one it follows; the view stands through a change of height or theme; and once
// destroyed, the scene listens to nothing. The pointer and the wheel are events sent to the canvas at the points
// the test works out from where the columns stand (`place`), so that each lands where the test means; the keys are the
// browser's own.

import '../styles/tokens.css';
import '../styles/parts.css';

import { createRoot } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { userEvent } from 'vitest/browser';

import { fixtureSource } from './fixtures';
import { ConsolePage } from './index';
import type { Shown } from './items';
import { KEEP, NODE_W, PACE, PITCH, ROW, SnapScene, themed } from './snap';

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

/// Waits until the columns stand still: three frames in a row in which nothing moved.
async function settle(scene: SnapScene) {
  let was = JSON.stringify(scene.place());
  let still = 0;
  for (let k = 0; k < 400 && still < 3; k += 1) {
    await frame();
    const now = JSON.stringify(scene.place());
    still = now === was ? still + 1 : 0;
    was = now;
  }
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
    await settle(scene);
    expect(scene.place().columns[0].at).toBeCloseTo(17.5, 3);
    const room = HEIGHT / 2 / ROW - KEEP;
    wheel(0, 100_000);
    await settle(scene);
    expect(scene.place().columns[0].at).toBeCloseTo(29.5 + room, 3);
    wheel(0, -100_000);
    await settle(scene);
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
    await settle(scene);
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

  it('selects the item a shadow points to on a click on the shadow, and brings it to the middle', async () => {
    const told = vi.fn<(item: Shown | null) => void>();
    scene.onSelect = told;
    // Under 1: item 2 and a shadow of 3; under 4: item 3.
    scene.show([card(1, [card(2), shadow(3)]), card(4, [card(3)])]);
    await settle(scene);
    const p = scene.place();
    // The shadow is the second of its column, its middle 1.5 rows from the column's top.
    await click(p.left + PITCH + 100, line + (1.5 - p.columns[1].at) * ROW);
    expect(told).toHaveBeenLastCalledWith(expect.objectContaining({ id: 3, shadow: false }));
    expect(scene.place().driver).toBe(1);
    await settle(scene);
    // Item 3 is the third of its column, after the room between the items of 1 and those of 4: 2 + 0.7 + 0.5 rows.
    expect(scene.place().columns[1].at).toBeCloseTo(3.2, 3);
  });

  it('draws new items where the tree stood, by the item nearest the middle, and keeps the item selected', async () => {
    const told = vi.fn<(item: Shown | null) => void>();
    scene.onSelect = told;
    scene.show([card(1), card(2), card(3)]);
    scene.select(2);
    await settle(scene);
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
    await settle(scene);
    scene.reveal(4, 600);
    await settle(scene);
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
    await settle(scene);
    const before = scene.place();
    expect(before.columns[0].at).toBe(0.5);
    expect(before.columns[1].at).toBe(5);
    move(50, line);
    wheel(0, 0.2 * ROW);
    await settle(scene);
    expect(scene.place().columns[0].at).toBeCloseTo(0.7, 6);
    expect(scene.place().columns[1].at).toBe(5);
    // Six rows off the middle, the item's want is loosened by six rows and more: still met.
    wheel(0, 5.8 * ROW);
    await settle(scene);
    expect(scene.place().columns[0].at).toBeCloseTo(6.5, 6);
    expect(scene.place().columns[1].at).toBe(5);
  });

  it('brings the column beside the one scrolled after it, the same way, never faster than half again its pace, and stops it when the scroll stops', async () => {
    scene.show(pairs());
    await settle(scene);
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
    await settle(scene);
    const start = scene.place().columns.map((c) => c.at);
    expect(scene.place().driver).toBe(0);
    move(scene.place().left + PITCH + 50, line);
    wheel(0, 6 * ROW);
    await drew();
    expect(scene.place().driver).toBe(1);
    await settle(scene);
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
    await settle(scene);
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
    await settle(scene);
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
    await settle(scene);
    scene.reveal(15, WIDTH);
    await settle(scene);
    expect(scene.place().columns[0].at).toBe(14.5);
    scene.reveal(30, WIDTH);
    await settle(scene);
    expect(scene.place().columns[0].at).toBeCloseTo(29.5, 3);
  });

  it('keeps the view where it stands when the height changes, and when the theme does', async () => {
    const roots = pairs();
    scene.show(roots);
    move(50, line);
    wheel(0, 2 * ROW);
    wheel(80, 0);
    await settle(scene);
    const before = scene.place();
    host.style.height = `${HEIGHT - 140}px`;
    await expect.poll(() => scene.place().height).toBe(HEIGHT - 140);
    await settle(scene);
    const shorter = scene.place();
    expect(shorter.left).toBe(before.left);
    shorter.columns.forEach((c, d) => expect(Math.abs(c.at - before.columns[d].at)).toBeLessThan(1e-9));
    document.documentElement.setAttribute('data-theme', 'dark');
    await drew();
    await settle(scene);
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
    expect(themed()).toMatchObject({ page: '#f6f7f9', card: '#ffffff', ink: '#1f2937', running: '#1a9fff', asked: '#f5840c' });
    document.documentElement.setAttribute('data-theme', 'dark');
    expect(themed()).toMatchObject({ page: '#0f1012', card: '#17181b', ink: '#e6e7ea', running: '#4fb4ff' });
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
  const folded = () => container.querySelector('.console')?.classList.contains('console-folded');
  const button = (words: string) =>
    [...container.querySelectorAll('button')].find((b) => b.textContent === words) ?? null;

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

  it('folds the panel on Escape while it has the keys, and keeps the item selected', async () => {
    const scene = await drawn();
    scene.select(12);
    await expect.poll(() => container.querySelector('textarea')).not.toBeNull();
    (container.querySelector('textarea') as HTMLTextAreaElement).focus();
    await userEvent.keyboard('{Escape}');
    await expect.poll(folded).toBe(true);
    expect(title()).toBe('Show the cost of a work');
    // The handle brings it back, the item still there.
    await userEvent.click(container.querySelector('.drawer-tab') as HTMLElement);
    await expect.poll(folded).toBe(false);
  });

  it('reads again what changed, without moving the tree or closing the panel', async () => {
    const scene = await drawn();
    scene.select(2);
    await expect.poll(() => button('Pause')).not.toBeNull();
    await settle(scene);
    const before = scene.place();
    await userEvent.click(button('Pause') as HTMLElement);
    await expect.poll(() => button('Let it on')).not.toBeNull();
    expect(title()).toBe('See every work at one glance');
    expect(folded()).toBe(false);
    const after = scene.place();
    expect(after.left).toBe(before.left);
    after.columns.forEach((c, d) => expect(Math.abs(c.at - before.columns[d].at)).toBeLessThan(1e-9));
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
    await expect
      .poll(() => container.querySelector('.cw-notes .cw-refused')?.textContent ?? null)
      .toBe('The page could not read the work: /api/v1/works/12 answered 500');
    fails = false;
    (reopened as (() => void) | null)?.();
    await expect.poll(() => container.querySelector('.cw-notes select')).not.toBeNull();
    root.unmount();
    container.remove();
  });
});
