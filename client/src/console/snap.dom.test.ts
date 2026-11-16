// @vitest-environment happy-dom
// What the canvas draws of a work on its card, apart from drawing it: the colour of its age, how its rings move and the
// mood its card takes.

import { describe, expect, it } from 'vitest';

import type { CellNow, Now } from '../works-api';
import { stateOf } from '../works/words';
import { age, moodOf } from './snap';

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
