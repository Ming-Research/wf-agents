import { describe, expect, it } from 'vitest';

import type { CellState } from '../works-api';
import { STRIP_MARKS, cellNowWords } from './marks';
import {
  CELL_LOOKS,
  RUN_STATES,
  type RunState,
  STATE_OF_CELL,
  cellRunState,
  colourOf,
  pulses,
  pulsing,
} from './state-marks';

describe('the marks of how a run ended or stands', () => {
  it('draws a run that runs again as a hollow ring of handed in’s green, a failed one as a hollow red ring, the stop filled in red, what waits its turn in yellow, and none in orange', () => {
    expect(RUN_STATES.again).toEqual({ shape: 'square', fill: null, edge: '--mark-handed-in', width: 'ring' });
    expect(RUN_STATES.handedIn).toMatchObject({ fill: '--mark-handed-in', edge: '--mark-handed-in' });
    expect(RUN_STATES.queued).toEqual({ shape: 'square', fill: '--yellow', edge: '--yellow', width: 'ring' });
    expect(RUN_STATES.failed).toEqual({ shape: 'square', fill: null, edge: '--red', width: 'ring' });
    expect(RUN_STATES.stopped).toEqual({ shape: 'square', fill: '--red', edge: '--red', width: 'ring' });
    // Orange says that something waits for the owner, which is said on a card, never on a run's mark.
    for (const look of Object.values(RUN_STATES)) expect(JSON.stringify(look)).not.toContain('orange');
    // A run under way is filled in the deep step: a round of it running, a call carried out for it, or at rest between
    // two rounds.
    const deep = Object.entries(RUN_STATES).filter(([, l]) => l.shape === 'square' && l.fill === '--mark-running');
    expect(deep.map(([s]) => s)).toEqual(['running', 'waiting', 'atRest']);
    // Filled is what ran or runs, hollow what is still to run or to be given again.
    const hollow = Object.entries(RUN_STATES).filter(([, l]) => l.shape === 'square' && l.fill === null);
    expect(hollow.map(([s]) => s)).toEqual(['failed', 'again', 'notYet']);
    expect(colourOf('failed')).toBe('var(--red)');
    expect(colourOf('handedIn')).toBe('var(--mark-handed-in)');
    expect(colourOf('queued')).toBe('var(--yellow)');
    expect(colourOf('skipped')).toBe('var(--muted)');
  });

  it('draws each cell of the strip as its run stands, from the same looks, the bright marks pulsing', () => {
    for (const [cell, run] of Object.entries(STATE_OF_CELL)) expect(CELL_LOOKS[cell as CellState]).toBe(RUN_STATES[run]);
    expect(STATE_OF_CELL.done).toBe('handedIn');
    expect(STATE_OF_CELL.calling).toBe('waiting');
    expect(STATE_OF_CELL.resting).toBe('atRest');
    expect(Object.keys(RUN_STATES).filter((s) => pulses(s as RunState))).toEqual(['running', 'waiting']);
    expect(Object.keys(STATE_OF_CELL).filter((s) => pulsing(s as CellState))).toEqual(['running', 'calling']);
  });

  it('draws the cell a work is stopped at as stopped, and the cell of a work that waits its turn as queued', () => {
    expect(cellRunState('resting', true, false)).toBe('stopped');
    expect(cellRunState('calling', true, true)).toBe('stopped');
    // Waiting its turn, a cell at rest or waiting for a call is queued, and its mark does not pulse.
    expect(cellRunState('calling', false, true)).toBe('queued');
    expect(cellRunState('resting', false, true)).toBe('queued');
    expect(pulses('queued')).toBe(false);
    // Else a cell stands as it does.
    expect(cellRunState('calling', false, false)).toBe('waiting');
    expect(cellRunState('running', false, true)).toBe('running');
    expect(cellRunState('done', false, true)).toBe('handedIn');
  });

  it("draws a cell of a work that ended with no run finished as the strip's dash, saying it did not run, never that it was skipped", () => {
    expect(CELL_LOOKS.notRun).toEqual({ shape: 'dash', fill: '--muted' });
    const cell = { cell: 'D4', state: 'notRun', finished: 0, record: null, open: null } as const;
    expect(cellNowWords(cell, 'notRun')).toBe('D4: did not run, 0 runs finished');
    expect(STRIP_MARKS).toContain('notRun');
  });
});
