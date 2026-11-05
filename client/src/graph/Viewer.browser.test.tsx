// Space on the flow page, in a real browser (Chromium, through Vitest's browser mode). With the pointer on the chart,
// Space shows the whole flow and the next Space goes back to the zoom it left; with the pointer on something floating
// over the chart, or in a box the owner types in, Space does neither. Viewer.dom.test.tsx counts the same in happy-dom,
// where the layout, what is under a point and the keys are made up by the test; here the page is drawn by the browser,
// the pointer is moved and the key pressed by the browser's own input (the DevTools protocol), and what is checked is
// where the drawing is on the screen. The page is the real one (`Flow`, with the page's own styles), reading a flow of
// 24 cells from a stand-in for the interface.

import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { userEvent } from 'vitest/browser';

import {
  cellNearTop,
  find,
  keys,
  landed,
  openFlowPage,
  pointerAt,
  seen,
  settle,
  stubPage,
  unmoved,
  unstubPage,
  whole,
} from './flowPage';

beforeEach(stubPage);
afterEach(unstubPage);

describe('Space on the flow page, in a real browser', () => {
  it('shows the whole flow with the pointer on the chart and goes back to the zoom it left', async () => {
    await openFlowPage();
    const opened = seen();
    // The chart opens at a zoom one can read, on part of the flow.
    expect(whole()).toBe(false);

    // The pointer is on a cell near the top of the viewer, far from the viewer's middle, where Space goes back to.
    await userEvent.hover(cellNearTop());
    const at = pointerAt();
    if (at === null) throw new Error('the browser sent no pointer move');
    const start = find('.viewer').getBoundingClientRect();
    expect(Math.abs(at.y - (start.top + start.height / 2))).toBeGreaterThan(100);

    await userEvent.keyboard(' ');
    // The browser reported the key as Space, and the page took it.
    expect(keys[0]).toEqual({ code: 'Space', prevented: true });
    await landed(whole, 3000);
    const all = seen();
    expect(all.zoom).toBeLessThan(opened.zoom / 1.05);
    // The point of the drawing under the pointer now, in the drawing's own pixels.
    const under = { x: (at.x - all.d.left) / all.zoom, y: (at.y - all.d.top) / all.zoom };

    await userEvent.keyboard(' ');
    expect(keys[1]).toEqual({ code: 'Space', prevented: true });
    // Back at the zoom it left, with that point in the middle of the viewer's height; the move is under way until both
    // hold.
    await expect
      .poll(
        () => {
          const back = seen();
          const middle = back.d.top + under.y * back.zoom - (back.v.top + back.v.height / 2);
          return Math.abs(back.zoom - opened.zoom) < 0.001 && Math.abs(middle) < 1.5;
        },
        { timeout: 3000 },
      )
      .toBe(true);
  });

  it('works on a chart drawn again under a pointer that does not move', async () => {
    const page = await openFlowPage();
    await userEvent.hover(find('.viewer'));
    // The chart is drawn anew, as when it is laid out again: its viewer is new, and the pointer sends it no event.
    await page.unmount();
    await openFlowPage();
    expect(whole()).toBe(false);
    await userEvent.keyboard(' ');
    expect(keys[0]).toEqual({ code: 'Space', prevented: true });
    await expect.poll(whole, { timeout: 3000 }).toBe(true);
  });

  it('does nothing while the pointer is on what floats over the chart', async () => {
    await openFlowPage();
    const opened = seen();
    // The tools over the chart's top left corner are on the page above the viewer, not in it.
    await userEvent.hover(find('.flow-tools'));
    await userEvent.keyboard(' ');
    expect(keys[0]).toEqual({ code: 'Space', prevented: false });
    await settle();
    expect(unmoved(opened)).toBe(true);
  });

  it('leaves Space to a box the owner types in', async () => {
    await openFlowPage();
    const opened = seen();
    await userEvent.hover(find('.viewer'));
    const box = document.createElement('input');
    // Where it takes no room and focusing it scrolls nothing.
    box.style.position = 'fixed';
    box.style.bottom = '0';
    document.body.append(box);
    try {
      box.focus();
      await userEvent.keyboard(' ');
      expect(keys[0]).toEqual({ code: 'Space', prevented: false });
      expect(box.value).toBe(' ');
      await settle();
      expect(unmoved(opened)).toBe(true);
    } finally {
      box.remove();
    }
  });
});
