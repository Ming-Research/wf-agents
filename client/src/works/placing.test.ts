import { describe, expect, it } from 'vitest';

import type { Item, Now, Step, Summary } from '../works-api';
import { FLOW, fixtureWork } from './fixtures';
import { cellAt, defaultBlock, orderWorks, placeSteps } from './placing';

const DAY = 86_400_000;

describe('the rows and columns of a history', () => {
  it('puts a run in its cell’s column, and marks where each cell that reads what was written stands', () => {
    const page = fixtureWork(41);
    const rows = placeSteps(page.steps, page.flow, page.blocks);
    expect(rows.map((r) => r.index)).toEqual(page.steps.map((_, i) => i));
    const at = (index: number) => rows[index];
    const step = (index: number) => page.steps[index];
    // The request is written by a person, and plan reads it.
    expect(at(0).step.kind).toBe('wrote');
    expect(at(0).column).toBeNull();
    expect([...at(0).marks]).toEqual([[0, 'read']]);
    // plan, design and the implementation's first run, which was abandoned.
    expect([1, 2, 3].map((i) => at(i).column)).toEqual([0, 1, 2]);
    const abandoned = step(3);
    expect(abandoned.kind === 'run' && abandoned.end?.kind).toBe('abandoned');
    // The owner's opinion for plan, then plan again.
    const opinion = step(4);
    expect(opinion.kind === 'wrote' && opinion.block).toBe('opinions_plan');
    expect([...at(4).marks]).toEqual([[0, 'read']]);
    expect(at(5).column).toBe(0);
    // The main the outside wrote is read by check.
    const main = step(13);
    expect(main.kind === 'wrote' && main.by.kind).toBe('outside');
    expect([...at(13).marks]).toEqual([[4, 'read']]);
  });

  it('draws a way round from the cell whose write it ran for back to the run', () => {
    const page = fixtureWork(41);
    const rows = placeSteps(page.steps, page.flow, page.blocks);
    const ways = rows.filter((r) => r.backFrom !== null).map((r) => [r.column, r.backFrom]);
    // Twice the review's opinion sent the implementation round.
    expect(ways).toEqual([
      [2, 3],
      [2, 3],
    ]);
  });

  it('marks a condition that reads what was written and the cell let go, and shows the day where it changes', () => {
    const t = Date.UTC(2026, 10, 9, 12);
    const steps: Step[] = [
      { kind: 'wrote', at: 1, time: t, block: 'needs_design', by: { kind: 'person' }, changed: true, value: 'yes', chars: 0 },
      { kind: 'letGo', at: 2, time: t + 1000, cell: 'implement' },
      { kind: 'paused', at: 3, time: t + DAY },
    ];
    const rows = placeSteps(steps, FLOW);
    expect(rows.map((r) => [...r.marks])).toEqual([[[1, 'condition']], [[2, 'letGo']], []]);
    expect(rows.map((r) => r.column)).toEqual([null, null, null]);
    expect(rows.map((r) => r.newDay)).toEqual([true, false, true]);
  });
});

/// A work in brief that does this now.
function summary(number: number, now: Now, last: number | null, paused = false): Summary {
  return {
    number,
    item: number,
    flow: 'engineer',
    added: 0,
    paused,
    now,
    cells: [],
    spent: { usdMicros: 0, inputTokens: 0, cachedTokens: 0, outputTokens: 0, workingMs: 0, rounds: 0 },
    last,
  };
}

describe('the list of works', () => {
  it('puts what waits for the owner first, then a fault, then what goes on, then what is paused, then what ended', () => {
    const works = [
      summary(1, { kind: 'merged' }, 900),
      summary(2, { kind: 'round', cell: 'plan', run: 1, round: 1, since: 0 }, 500),
      summary(3, { kind: 'waits', at: 'plan', on: [{ kind: 'call', call: 1, who: null, what: { kind: 'tool', name: 'x', input: '' } }] }, 800),
      summary(4, { kind: 'waits', at: 'merge', on: [{ kind: 'limit', runs: 3 }] }, 100),
      summary(5, { kind: 'paused' }, 950, true),
      summary(6, { kind: 'fault', words: 'x' }, 10),
      summary(7, { kind: 'waits', at: 'plan', on: [{ kind: 'precondition', test: 't', words: 'w', who: 'owner' }] }, 200),
      summary(8, { kind: 'cancelled' }, 990),
      summary(9, { kind: 'round', cell: 'plan', run: 1, round: 1, since: 0 }, 600, true),
    ];
    const items: Item[] = works.map((w) => ({ number: w.number, title: `${w.number}`, added: 0, dependsOn: [], works: [w] }));
    expect(orderWorks(items).map((w) => w.work.number)).toEqual([7, 4, 6, 3, 2, 5, 9, 8, 1]);
  });

  it('lists every work of an item, not only its last', () => {
    const item: Item = {
      number: 1,
      title: 'one',
      added: 0,
      dependsOn: [],
      works: [summary(10, { kind: 'cancelled' }, 5), summary(11, { kind: 'next', cell: 'plan' }, 6)],
    };
    expect(orderWorks([item]).map((w) => w.work.number)).toEqual([11, 10]);
  });
});

describe('the box to write for a cell', () => {
  it('starts at the opinions of the cell the work is at, or the block people write that it reads', () => {
    expect(defaultBlock(FLOW, 'implement')).toBe('opinions_implement');
    expect(defaultBlock(FLOW, 'plan')).toBe('opinions_plan');
    expect(defaultBlock(FLOW, 'merge')).toBe('approval');
    expect(cellAt(fixtureWork(41).summary)).toBe('merge');
    expect(cellAt(fixtureWork(46).summary)).toBeNull();
  });
});
