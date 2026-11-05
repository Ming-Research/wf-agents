// Space on the flow page, in a real browser, after what the owner does before pressing it: a click on the chart's
// background, a click on a cell (which opens its panel over the chart's right side), the pointer going out of the window
// and coming back, the legend and the menus over the chart opened and closed. Viewer.browser.test.tsx holds the cases
// where the pointer only moves onto the chart; each case here starts from a different state of the page and then
// presses Space with the pointer on the chart, expecting what Space does there: the whole flow in sight. When it does
// not, the failure says where the key stopped: whether it reached the window, what had the keyboard, what the page
// answered it, and what was under the pointer.

import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cdp, userEvent } from 'vitest/browser';

import {
  arrived,
  cellNearTop,
  describeElement,
  find,
  keys,
  landed,
  openFlowPage,
  pointerAt,
  seen,
  stubPage,
  unmoved,
  unstubPage,
  whole,
} from './flowPage';

beforeEach(stubPage);
afterEach(unstubPage);

/// A place on the chart that the drawing leaves empty and the window over its right side does not cover, in the viewer's
/// own pixels, for the pointer to rest on.
function emptySpot(): { x: number; y: number } {
  const viewer = find('.viewer');
  const v = viewer.getBoundingClientRect();
  for (let fy = 0.9; fy > 0.1; fy -= 0.1) {
    for (let fx = 0.15; fx < 0.5; fx += 0.05) {
      const x = v.left + v.width * fx;
      const y = v.top + v.height * fy;
      const e = document.elementFromPoint(x, y);
      if (e !== null && viewer.contains(e) && e.closest('[data-node],[data-edge],[data-store],.fc-map,.legend-button,.legend') === null) {
        return { x: x - v.left, y: y - v.top };
      }
    }
  }
  throw new Error('the chart has no empty place in its left half');
}

/// Presses Space and waits for the whole flow to be in sight; when it does not come, fails saying where the key got.
async function spaceShowsWhole(what: string): Promise<void> {
  // The drawing is at rest, and not yet whole.
  await landed(() => true, 5000);
  if (whole()) throw new Error(`${what}: the chart was whole before the key`);
  keys.length = 0;
  arrived.length = 0;
  const before = seen();
  await userEvent.keyboard(' ');
  try {
    await expect.poll(whole, { timeout: 3000 }).toBe(true);
    return;
  } catch {
    // Reported below, with where the key stopped.
  }
  const at = pointerAt();
  const under = at === null ? null : document.elementFromPoint(at.x, at.y);
  throw new Error(
    `${what}: Space did not show the whole chart. ` +
      JSON.stringify({
        reachedWindow: arrived,
        endedAs: keys,
        pointer: at,
        underPointer: describeElement(under),
        underIsInViewer: under !== null && find('.viewer').contains(under),
        keyboardIn: describeElement(document.activeElement),
        windowHasFocus: document.hasFocus(),
        drawingMoved: !unmoved(before),
      }),
  );
}

describe('Space on the flow page, after what the owner does first', () => {
  it('works after a click on the chart background', async () => {
    await openFlowPage();
    await landed(() => true, 5000);
    const spot = emptySpot();
    await userEvent.click(find('.viewer'), { position: spot });
    await spaceShowsWhole('after a click on the background');
  });

  it('works after a click on a cell, which opens its panel, with the pointer on the chart', async () => {
    await openFlowPage();
    await userEvent.click(cellNearTop());
    // The panel opens in the window over the chart's right side.
    await expect.poll(() => document.querySelector('.dock') !== null, { timeout: 3000 }).toBe(true);
    await landed(() => true, 5000);
    await userEvent.hover(find('.viewer'), { position: emptySpot() });
    await spaceShowsWhole('after a click on a cell');
  });

  it('works after the pointer went out of the window and came back', async () => {
    await openFlowPage();
    await userEvent.hover(find('.viewer'));
    const left: string[] = [];
    document.documentElement.addEventListener('pointerleave', () => left.push('pointerleave'), { once: true });
    // Out of the window, as the browser's own input moves it.
    // The session's `send` is typed by the provider's own types, which the page's tsconfig does not load.
    const session = cdp() as unknown as { send: (method: string, params: object) => Promise<unknown> };
    await session.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: -20, y: -20, button: 'none' });
    // The page's root did get the pointer leaving: without it this case would only be Space after a hover.
    await expect.poll(() => left, { timeout: 2000 }).toEqual(['pointerleave']);
    await userEvent.hover(find('.viewer'), { position: emptySpot() });
    await spaceShowsWhole('after the pointer left the window and came back');
  });

  it('works after the legend was opened and closed', async () => {
    await openFlowPage();
    await userEvent.click(find('.legend-button'));
    await expect.poll(() => document.querySelector('.legend') !== null, { timeout: 3000 }).toBe(true);
    await userEvent.click(find('.legend-button'));
    await expect.poll(() => document.querySelector('.legend') === null, { timeout: 3000 }).toBe(true);
    await userEvent.hover(find('.viewer'), { position: emptySpot() });
    await spaceShowsWhole('after the legend was opened and closed');
  });

  it('works after the flow menu was opened and closed', async () => {
    await openFlowPage();
    await userEvent.click(find('.flow-tools button'));
    await expect.poll(() => document.querySelector('[role="menu"], [role="listbox"]') !== null, { timeout: 3000 }).toBe(true);
    await userEvent.keyboard('{Escape}');
    await expect.poll(() => document.querySelector('[role="menu"], [role="listbox"]') === null, { timeout: 3000 }).toBe(true);
    await userEvent.hover(find('.viewer'), { position: emptySpot() });
    await spaceShowsWhole('after the flow menu was opened and closed');
  });

  it('works after the comparison box was opened and closed', async () => {
    await openFlowPage();
    const buttons = [...document.querySelectorAll<HTMLElement>('.flow-tools > button')];
    const compare = buttons[buttons.length - 1];
    if (compare === undefined) throw new Error('the flow tools have no Compare button');
    await userEvent.click(compare);
    await expect.poll(() => document.querySelector('.compare-box') !== null, { timeout: 3000 }).toBe(true);
    await userEvent.click(compare);
    await expect.poll(() => document.querySelector('.compare-box') === null, { timeout: 3000 }).toBe(true);
    await userEvent.hover(find('.viewer'), { position: emptySpot() });
    await spaceShowsWhole('after the comparison box was opened and closed');
  });
});
