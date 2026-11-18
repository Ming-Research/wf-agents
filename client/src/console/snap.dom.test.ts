// @vitest-environment happy-dom
// What the canvas draws of a work on its card, apart from drawing it: the colour of its age, how its rings move and the
// mood its card takes.

import { describe, expect, it } from 'vitest';

import type { CellNow, Now } from '../works-api';
import { stateOf } from '../works/words';
import { age } from './items';
import {
  beatOf,
  dashesAlong,
  DIM,
  eased,
  grounded,
  type Light,
  linksStrength,
  moodOf,
  NODE_W,
  PITCH,
  ROW,
  wayAims,
  waysUp,
} from './snap';

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
    // A work that waits for the turn, whatever call is under way at another cell: the state says it waits, and the card
    // says so too.
    const now: Now = { kind: 'waits', at: 'M2', on: [{ kind: 'turn', holder: 3, since: 0 }] };
    const cells: CellNow[] = [{ cell: 'M1', state: 'calling', finished: 0, record: null, open: 1 }];
    expect(stateOf({ now, cells, waits: [] }).tone).toBe('quiet');
    expect(moodOf({ now, cells, waits: [] })).toBe('waiting');
    // One that waits at its cell for the call of a tool the engine carries out waits, in the yellow of a call, and its
    // card waits for the call, in that yellow (decision a-call-waited-for-is-no-run).
    const tool: Now = {
      kind: 'waits',
      at: 'M2',
      on: [{ kind: 'call', call: 4, who: null, what: { kind: 'tool', name: 'run_tests', input: '' } }],
    };
    const at: CellNow[] = [{ ...cells[0], cell: 'M2' }];
    expect(stateOf({ now: tool, cells: at, waits: [] }).tone).toBe('calling');
    expect(moodOf({ now: tool, cells: at, waits: [] })).toBe('calling');
    // An act of the engine's own under way is a round, and its card runs.
    const act: Now = { kind: 'round', cell: 'M2', run: 2, round: 1, since: 0 };
    expect(stateOf({ now: act, cells: at, waits: [] }).tone).toBe('running');
    expect(moodOf({ now: act, cells: at, waits: [] })).toBe('running');
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

  it('stand at full strength while nothing is lit, and while something is, only where both ends are lit', () => {
    const aims = (on: number[], lit: number[]) =>
      Object.fromEntries([...wayAims(on, parent, new Set(lit))].map(([i, a]) => [i, a === 1 ? 'full' : 'dim']));
    expect(aims([1, 2, 3], [])).toEqual({ 1: 'full', 2: 'full', 3: 'full' });
    // Item 1 lit, with its root and the item under it.
    expect(aims([1, 2, 3], [1, 0, 3])).toEqual({ 1: 'full', 2: 'dim', 3: 'full' });
    // Item 2 lit with its root: the link from 3 joins nothing lit, and the link from 1 has only one end lit.
    expect(aims([1, 2, 3], [2, 0])).toEqual({ 1: 'dim', 2: 'full', 3: 'dim' });
    // Dimmed, a way is as strong as the links it lies on once the items are dimmed.
    expect(wayAims([1], parent, new Set([4])).get(1)).toBe(linksStrength(DIM));
    expect(linksStrength(1)).toBe(1);
  });

  it('ease to their strength over frames, down and back up in the same time, and come to it, as the items do', () => {
    const frame = 16;
    const dimmed = linksStrength(DIM);
    // One frame after the light goes off a way, it has gone only a little of the way down.
    const once = eased(1, dimmed, frame);
    expect(once).toBeLessThan(1);
    expect(once).toBeGreaterThan(1 - (1 - dimmed) / 4);
    // Coming back up goes as far in a frame as going down.
    expect(eased(dimmed, 1, frame) - dimmed).toBeCloseTo(1 - once, 12);
    let a = dimmed;
    for (let n = 0; n < 120; n += 1) a = eased(a, 1, frame);
    expect(a).toBeCloseTo(1, 3);
    expect(eased(0.7, 0.7, frame)).toBe(0.7);
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

describe('the ground of the page under the soft lights', () => {
  // A light behind item 0, a card two rows high at the top of its column, reaching 40 pixels past it.
  const light: Light = { item: 0, gx: 0, gy: ROW, gh: ROW / 2, reach: 40 };
  const under = (item: number, x: number, rows: number, boxed = true) =>
    grounded(item, x, ROW + rows * ROW, ROW / 2, boxed, [light]);

  it('lies under the card lit and under the cards beside it in its column that the light reaches', () => {
    expect(under(0, 0, 0)).toBe(true);
    expect(under(1, 0, 1)).toBe(true);
    expect(under(2, 0, -1)).toBe(true);
  });

  it('lies under no card the light does not reach: one further down the column, or one in the next column', () => {
    expect(under(3, 0, 4)).toBe(false);
    expect(under(4, PITCH, 0)).toBe(false);
    expect(PITCH).toBeGreaterThan(NODE_W + light.reach);
  });

  it('lies under an item with no box only where the light is its own', () => {
    expect(under(1, 0, 1, false)).toBe(false);
    expect(under(0, 0, 0, false)).toBe(true);
  });

  it('lies nowhere while nothing is lit', () => {
    expect(grounded(0, 0, ROW, ROW / 2, true, [])).toBe(false);
  });
});

describe("a running cell's beat on its card", () => {
  // The page's build writes the token's time in seconds (`.75s`), the page's source in milliseconds: the card's mark
  // beats once in 750 ms either way, as the works page's marks do, never once in 0.75 ms.
  it.each(['750ms', '.75s', '0.75s'])('is 750 ms when the styles write %s', (written) => {
    document.documentElement.style.setProperty('--pulse', written);
    try {
      expect(beatOf().ms).toBe(750);
    } finally {
      document.documentElement.style.removeProperty('--pulse');
    }
  });
});
