import { describe, expect, it } from 'vitest';

import { MARKS } from './state-marks';

describe("the marks of a cell's state", () => {
  it("draws a cell that runs again as a hollow ring of done's colour, and no cell's mark in orange", () => {
    expect(MARKS.again).toEqual({ shape: 'square', fill: null, edge: '--mark-done', width: 'ring' });
    expect(MARKS.done).toMatchObject({ fill: '--mark-done', edge: '--mark-done' });
    // Orange says that something waits for the owner, which is said on a card, never on a cell's mark.
    for (const look of Object.values(MARKS)) expect(JSON.stringify(look)).not.toContain('orange');
    // A run under way is filled in the deep step, however it stands: running, between two rounds, or its round failed.
    const deep = Object.entries(MARKS).filter(([, l]) => l.shape === 'square' && l.fill === '--mark-running');
    expect(deep.map(([s]) => s)).toEqual(['running', 'resting', 'failed']);
    // Filled is what ran or runs, hollow what is still to run.
    const hollow = Object.entries(MARKS).filter(([, l]) => l.shape === 'square' && l.fill === null);
    expect(hollow.map(([s]) => s)).toEqual(['again', 'notYet']);
  });
});
