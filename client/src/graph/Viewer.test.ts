import { describe, expect, it } from 'vitest';

import { inSight, revealed } from './Viewer';

describe('bringing a part of the flowchart out from under the window', () => {
  // A viewer 1000 by 600, of which a window over its right takes 400.
  const left = { x0: 0, y0: 0, x1: 600, y1: 600 };

  it('moves the chart only as far as brings the part wholly into what the window leaves, with room round it', () => {
    const v = { s: 1, x: 0, y: 0 };
    const cell = { x: 700, y: 100, width: 150, height: 60 };
    const to = revealed(cell, v, left);
    expect(to.s).toBe(1);
    expect(to.y).toBe(0);
    // Its right edge 24 short of the window's edge.
    expect(cell.x + cell.width + to.x).toBe(600 - 24);
    expect(inSight(cell, to, left)).toBe(true);
  });

  it('keeps the zoom, and moves up or down only for a part out of sight that way', () => {
    const v = { s: 0.5, x: 0, y: 0 };
    const cell = { x: 200, y: 1400, width: 150, height: 60 };
    const to = revealed(cell, v, left);
    expect(to.s).toBe(0.5);
    expect(to.x).toBe(0);
    expect((cell.y + cell.height) * 0.5 + to.y).toBe(600 - 24);
  });

  it('shows the left of a part wider than what the window leaves', () => {
    const v = { s: 1, x: 0, y: 0 };
    const wide = { x: 900, y: 10, width: 800, height: 60 };
    const to = revealed(wide, v, left);
    expect(wide.x + to.x).toBe(0);
  });
});
