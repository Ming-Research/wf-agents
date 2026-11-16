// @vitest-environment happy-dom
// What the canvas draws of a work on its card, apart from drawing it: the colour of its age, how its rings move and the
// mood its card takes.

import { describe, expect, it } from 'vitest';

import type { CellNow, Now } from '../works-api';
import { stateOf } from '../works/words';
import { age, dashesAlong, moodOf, waysUp } from './snap';

const MINUTE = 60_000;

describe("a work's age on its card", () => {
  it('takes three colours and no more: grey under half an hour, amber to an hour, red past an hour however long', () => {
    expect(age(10 * MINUTE).tone).toBe('success');
    expect(age(45 * MINUTE).tone).toBe('attention');
    expect(age(90 * MINUTE).tone).toBe('danger');
    expect(age(30 * 60 * MINUTE).tone).toBe('danger');
    expect(age(3 * 24 * 60 * MINUTE)).toEqual({ label: '3d', tone: 'danger' });
  });
});

describe("a card's mood", () => {
  it('is taken from the state every view names a work by', () => {
    // A tool's call under way at a cell the work does not wait at: the state says it waits, and the card says so too.
    const now: Now = { kind: 'waits', at: 'M2', on: [{ kind: 'turn', holder: 3, since: 0 }] };
    const cells: CellNow[] = [{ cell: 'M1', state: 'calling', finished: 0, record: null, open: 1 }];
    expect(stateOf({ now, cells, waits: [] }).tone).toBe('quiet');
    expect(moodOf({ now, cells, waits: [] })).toBe('waiting');
    // At the cell it waits at, the call runs, as its state says.
    const at: CellNow[] = [{ ...cells[0], cell: 'M2' }];
    expect(stateOf({ now, cells: at, waits: [] }).tone).toBe('running');
    expect(moodOf({ now, cells: at, waits: [] })).toBe('running');
  });
});

describe('the dashed ways of the items whose work runs', () => {
  // Item 0 is a root with 1 and 2 under it, and 3 under 1; item 4 is a root of its own.
  const parent = Int32Array.from([-1, 0, 0, 1, -1]);
  const ways = (running: number[]) => [...waysUp(running, parent)].sort((a, b) => a - b);

  it('take each link from a running item up to its root once, however many running items it is on', () => {
    expect(ways([3])).toEqual([1, 3]);
    expect(ways([3, 2])).toEqual([1, 2, 3]);
    expect(ways([3, 1])).toEqual([1, 3]);
  });

  it('take no link above a root', () => {
    expect(ways([0, 4])).toEqual([]);
  });

  it('are dashed from the start, the dashes moving towards the end as time goes on, cut at the ends', () => {
    expect(dashesAlong(40, 0).slice(0, 2)).toEqual([
      [0, 5],
      [9, 14],
    ]);
    // Shifted on two pixels, the first dash begins two pixels further along.
    expect(dashesAlong(40, 2)[0]).toEqual([2, 7]);
    // Shifted on seven, the dash before it comes in cut at the start.
    expect(dashesAlong(40, 7).slice(0, 2)).toEqual([
      [0, 3],
      [7, 12],
    ]);
    for (const [from, to] of dashesAlong(40, 3)) {
      expect(from).toBeGreaterThanOrEqual(0);
      expect(to).toBeLessThanOrEqual(40);
      expect(to - from).toBeLessThanOrEqual(5);
    }
    expect(dashesAlong(0, 3)).toEqual([]);
  });
});
