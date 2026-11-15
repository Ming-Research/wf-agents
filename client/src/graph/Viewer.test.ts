import { describe, expect, it } from 'vitest';

import { inSight, revealed, topView } from './Viewer';

describe('where the flowchart opens', () => {
  const seen = { x0: 0, y0: 0, x1: 1200, y1: 700 };

  it('opens a long chart at its own size, its top in sight and across in the middle, the rest to scroll to', () => {
    // As the engineer's flow is: far longer than the window, and narrower than it.
    const v = topView(seen, { width: 900, height: 4200 }, 1);
    expect(v.s).toBe(1);
    expect(v.y).toBe(24);
    expect(v.x).toBe((1200 - 900) / 2);
  });

  it("fits a chart wider than the window to its width, not the whole chart to the window", () => {
    const v = topView(seen, { width: 2400, height: 4200 }, 1);
    expect(v.s).toBeCloseTo((1200 - 32) / 2400);
    expect(v.x).toBeCloseTo(16);
    expect(v.y).toBe(24);
  });

  it('opens at its own size where nothing can be seen yet', () => {
    expect(topView({ x0: 0, y0: 0, x1: 0, y1: 0 }, { width: 900, height: 4200 }, 1).s).toBe(1);
  });
});

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
