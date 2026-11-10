import { describe, expect, it } from 'vitest';

import type { Block, Cause, Item, Now, Run, Step, Summary, Version, Writer } from '../works-api';
import { FLOW, fixtureWork } from './fixtures';
import { stepWords } from './History';
import { cellAt, defaultBlock, orderWorks, placeSteps } from './placing';

const DAY = 86_400_000;

/// A finished run of one round, for these reasons, from these versions.
function run(n: number, cell: string, because: Cause[], basis: Run['basis']): Step {
  return {
    kind: 'run',
    run: n,
    cell,
    at: n * 10,
    time: n * 1000,
    because,
    basis,
    rounds: [],
    calls: [],
    end: { kind: 'finished', at: n * 10 + 1, time: n * 1000 + 1, writes: [] },
    spent: { usdMicros: 0, inputTokens: 0, cachedTokens: 0, outputTokens: 0, workingMs: 0, rounds: 0 },
  };
}

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
    const ways = rows.filter((r) => r.backFrom.length > 0).map((r) => [r.column, r.backFrom]);
    // Twice the review's opinion sent the implementation round.
    expect(ways).toEqual([
      [2, [3]],
      [2, [3]],
    ]);
  });

  it('marks each cell whose write a way round ran for, draws none where no writer is found, and keeps a run of a cell gone', () => {
    const version = (at: number, by: Writer): Version => ({ at, time: 0, by, meaning: at, value: null, chars: 0 });
    const blocks: Block[] = [
      { name: 'x', versions: [version(5, { kind: 'run', run: 1, cell: 'review' })] },
      { name: 'y', versions: [version(6, { kind: 'run', run: 2, cell: 'check' })] },
      { name: 'z', versions: [version(7, { kind: 'person' })] },
    ];
    const behind = (block: string): Cause => ({ kind: 'changed', block, was: null, behind: true });
    const steps: Step[] = [
      run(3, 'implement', [behind('x'), behind('y')], [{ block: 'x', version: 5 }, { block: 'y', version: 6 }]),
      run(4, 'implement', [behind('z')], [{ block: 'z', version: 7 }]),
      run(5, 'gone', [{ kind: 'noRecord' }], []),
    ];
    const rows = placeSteps(steps, FLOW, blocks);
    expect(rows.map((r) => [r.column, r.backFrom])).toEqual([
      [2, [3, 4]],
      [2, []],
      [null, []],
    ]);
    expect(stepWords(steps[2], FLOW)).toBe('gone, which the flow no longer has: gone had no record');
    expect(stepWords(steps[0], FLOW)).toBe('way round for x, y');
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
    const items: Item[] = works.map((w) => ({ number: w.number, title: `${w.number}`, added: 0, dependsOn: [], work: w }));
    expect(orderWorks(items).map((w) => w.work.number)).toEqual([7, 4, 6, 3, 2, 5, 9, 8, 1]);
  });
});

describe('the box to write for a cell', () => {
  it('starts at the opinions of the cell the work is at, or the block people write that it reads, and else at none', () => {
    expect(defaultBlock(FLOW, 'implement')).toBe('opinions_implement');
    expect(defaultBlock(FLOW, 'plan')).toBe('opinions_plan');
    expect(defaultBlock(FLOW, 'merge')).toBe('approval');
    expect(defaultBlock(FLOW, 'review')).toBeNull();
    expect(defaultBlock(FLOW, null)).toBeNull();
    expect(cellAt(fixtureWork(41).summary)).toBe('merge');
    expect(cellAt(fixtureWork(46).summary)).toBeNull();
  });
});
