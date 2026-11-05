// @vitest-environment happy-dom
// Space on a viewer, counted in a DOM: it shows the whole drawing and goes back to the zoom it left, with the point
// under the pointer in the middle, when the pointer is on the viewer, and does nothing when it is not. Whether the
// pointer is on the viewer is read from where it was last seen, as the page's charts are laid out again and mounted
// again under a pointer that stays where it is and sends nothing to the new viewer. The browser's layout is not here:
// the viewer is 800 by 600 at the window's top left corner, and what is under a point is answered from that.

import '../dom-setup';

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { Viewer } from './Viewer';

const BOX = { left: 0, top: 0, right: 800, bottom: 600, x: 0, y: 0, width: 800, height: 600 };
const original = document.elementFromPoint;

let host: HTMLElement | null = null;
let root: Root | null = null;
/// What floats over the page, if anything: it is what is under every point while it is there.
let floating: Element | null = null;

/// Draws a viewer of a 1536 by 1136 drawing, whose whole fits 800 by 600 at half its size with a margin of 16 pixels
/// round it, and gives its box and its stage.
async function mount(): Promise<{ box: HTMLElement; stage: HTMLElement }> {
  host = document.createElement('div');
  document.body.append(host);
  const mine = createRoot(host);
  root = mine;
  await act(async () =>
    mine.render(
      <Viewer width={1536} height={1136} onPick={() => undefined}>
        <div />
      </Viewer>,
    ),
  );
  const box = host.querySelector<HTMLElement>('.viewer');
  const stage = host.querySelector<HTMLElement>('.viewer-stage');
  if (!box || !stage) throw new Error('the viewer is not drawn');
  box.getBoundingClientRect = () => BOX as DOMRect;
  document.elementFromPoint = (x, y) => {
    if (floating !== null) return floating;
    const inside = x >= BOX.left && x < BOX.right && y >= BOX.top && y < BOX.bottom;
    return inside ? stage : document.body;
  };
  return { box, stage };
}

/// Takes the viewer away, as laying its chart out again does.
async function unmount(): Promise<void> {
  const mine = root;
  if (mine !== null) await act(async () => mine.unmount());
  host?.remove();
  root = null;
  host = null;
}

/// The pointer moves to a point, over `on`: no other event comes with it, as none comes to a viewer that is drawn under
/// a pointer that stays where it is.
function pointAt(on: Element, x: number, y: number): void {
  on.dispatchEvent(new MouseEvent('pointermove', { bubbles: true, clientX: x, clientY: y }));
}

/// Space is pressed on `on`; whether the page's own use of the key was held back.
function space(on: Element = document.body): boolean {
  const e = new KeyboardEvent('keydown', { code: 'Space', bubbles: true, cancelable: true });
  on.dispatchEvent(e);
  return e.defaultPrevented;
}

/// The view the stage is at once the move has landed, as its zoom and where the drawing's top left corner is.
async function landed(stage: HTMLElement): Promise<{ s: number; x: number; y: number } | null> {
  await act(async () => {
    vi.advanceTimersByTime(300);
  });
  const m = /translate\((-?[\d.]+)px, (-?[\d.]+)px\) scale\(([\d.]+)\)/.exec(stage.style.transform);
  return m ? { x: Number(m[1]), y: Number(m[2]), s: Number(m[3]) } : null;
}

beforeEach(() => {
  vi.useFakeTimers();
  // The pointer is not known to be anywhere.
  document.documentElement.dispatchEvent(new Event('pointerleave'));
});

afterEach(async () => {
  await unmount();
  floating?.remove();
  floating = null;
  document.elementFromPoint = original;
  vi.useRealTimers();
});

describe('Space on a viewer', () => {
  it('shows the whole drawing and goes back with the point under the pointer in the middle', async () => {
    const { box, stage } = await mount();
    pointAt(box, 600, 400);
    expect(space()).toBe(true);
    expect(await landed(stage)).toEqual({ s: 0.5, x: 16, y: 16 });
    expect(space()).toBe(true);
    // The drawing point (1168, 768) was under the pointer; it comes to the middle of the viewer, (400, 300).
    expect(await landed(stage)).toEqual({ s: 1, x: -768, y: -468 });
  });

  it('works on a viewer mounted again under a pointer that sent it nothing', async () => {
    const first = await mount();
    pointAt(first.box, 600, 400);
    await unmount();
    const { stage } = await mount();
    expect(space()).toBe(true);
    expect(await landed(stage)).toEqual({ s: 0.5, x: 16, y: 16 });
    expect(space()).toBe(true);
    // The pointer is still taken to be at (600, 400): the same point comes to the middle.
    expect(await landed(stage)).toEqual({ s: 1, x: -768, y: -468 });
  });

  it('does nothing while the pointer is elsewhere on the page', async () => {
    const { box, stage } = await mount();
    pointAt(box, 900, 700);
    expect(space()).toBe(false);
    expect(await landed(stage)).toBeNull();
  });

  it('does nothing while something floating over the page covers the pointer', async () => {
    const { box, stage } = await mount();
    pointAt(box, 600, 400);
    floating = document.createElement('div');
    document.body.append(floating);
    expect(space()).toBe(false);
    expect(await landed(stage)).toBeNull();
  });

  it('does nothing once the pointer has left the window', async () => {
    const { box, stage } = await mount();
    pointAt(box, 600, 400);
    document.documentElement.dispatchEvent(new Event('pointerleave'));
    expect(space()).toBe(false);
    expect(await landed(stage)).toBeNull();
  });

  it('does nothing before the pointer has been seen', async () => {
    const { stage } = await mount();
    expect(space()).toBe(false);
    expect(await landed(stage)).toBeNull();
  });

  it('leaves Space to a box the owner types in', async () => {
    const { box, stage } = await mount();
    pointAt(box, 600, 400);
    const field = document.createElement('input');
    document.body.append(field);
    try {
      expect(space(field)).toBe(false);
      expect(await landed(stage)).toBeNull();
    } finally {
      field.remove();
    }
  });
});
