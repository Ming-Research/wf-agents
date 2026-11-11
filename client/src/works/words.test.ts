import { describe, expect, it } from 'vitest';

import type { Round, Waited } from '../works-api';
import {
  RUN_WORDS,
  callState,
  causeWords,
  clock,
  dollars,
  duration,
  isOpen,
  isWayRound,
  nowTone,
  nowWords,
  pauseWords,
  runState,
  tokens,
  unmetWords,
  waitedWords,
  waitsForOwner,
} from './words';

const precondition: Waited = {
  kind: 'precondition',
  test: 'approved',
  words: 'The owner approved the merge',
  by: [{ kind: 'owner' }],
};
const question: Waited = {
  kind: 'call',
  call: 4,
  who: 'owner',
  what: { kind: 'question', who: 'owner', question: 'Show the cached tokens apart?' },
};

describe('what a work does now, in words', () => {
  it('says each kind of now', () => {
    const since = Date.UTC(2026, 10, 9, 14, 2);
    expect(nowWords({ kind: 'fault', words: 'Two entries at place 3' })).toBe('Fault: Two entries at place 3');
    expect(nowWords({ kind: 'merged' })).toBe('Merged');
    expect(nowWords({ kind: 'cancelled' })).toBe('Cancelled');
    expect(nowWords({ kind: 'round', cell: 'implement', run: 5, round: 2, since })).toBe(
      `Running implement, round 2 of run 5, since ${clock(since)}`,
    );
    expect(nowWords({ kind: 'paused', by: 'owner', why: null })).toBe(
      'Paused: only a note to a cell is answered until it is let on',
    );
    expect(nowWords({ kind: 'paused', by: 'engine', why: 'The agent could not start.' })).toBe(
      'Paused by the engine: The agent could not start.',
    );
    expect(nowWords({ kind: 'waits', at: 'merge', on: [precondition, { kind: 'limit', runs: 3 }] })).toBe(
      'Waits at merge for The owner approved the merge, which the owner can make hold; ' +
        'the owner to let the cell go on: its last 3 runs in a row were each a way round',
    );
    expect(nowWords({ kind: 'next', cell: 'plan' })).toBe('About to go on at plan');
  });

  it('says each kind of thing a work waits for, and whether the owner is the one it waits for', () => {
    const tool: Waited = { kind: 'call', call: 2, who: null, what: { kind: 'tool', name: 'shell', input: 'ls' } };
    const returns: Waited = { kind: 'call', call: 3, who: 'owner', what: { kind: 'atLimit', limit: { of: 'returns', sentBack: 2 } } };
    const calls: Waited = { kind: 'call', call: 3, who: null, what: { kind: 'atLimit', limit: { of: 'calls', made: 40 } } };
    const dependencies: Waited = {
      kind: 'precondition',
      test: 'may_start',
      words: 'The items it depends on are merged',
      by: [{ kind: 'outside', source: 'dependencies' }],
    };
    const settled: Waited = {
      kind: 'precondition',
      test: 'settled',
      words: 'No cell that judged the work says it is not settled',
      by: [{ kind: 'owner' }, { kind: 'cell', cell: 'T1' }],
    };
    const checked: Waited = {
      kind: 'precondition',
      test: 'checked',
      words: 'The check before the merge passed',
      by: [{ kind: 'cell', cell: 'M1' }],
    };
    expect(waitedWords(question)).toBe("the owner's answer to a question");
    expect(waitedWords(tool)).toBe('the result of the tool shell');
    expect(waitedWords(returns)).toBe('the owner to let the run go on: it reached its limit, 2 returns sent back');
    expect(waitedWords(calls)).toBe('the owner to let the run go on: it reached its limit, 40 calls made');
    expect(waitedWords(precondition)).toBe('The owner approved the merge, which the owner can make hold');
    expect(waitedWords(dependencies)).toBe(
      'The items it depends on are merged, which the outside (dependencies) can make hold',
    );
    expect(waitedWords(settled)).toBe(
      'No cell that judged the work says it is not settled, which the owner or cell T1 can make hold',
    );
    expect(waitedWords(checked)).toBe('The check before the merge passed, which cell M1 can make hold');
    expect(waitedWords({ kind: 'limit', runs: 1 })).toBe(
      'the owner to let the cell go on: its last run was a way round',
    );
    const all = [question, tool, returns, calls, precondition, dependencies, settled, checked, { kind: 'limit', runs: 3 } as Waited];
    expect(all.map(waitsForOwner)).toEqual([true, false, true, true, true, false, true, false, true]);
    expect(nowTone({ kind: 'waits', at: 'plan', on: [tool] })).toBe('quiet');
    expect(nowTone({ kind: 'waits', at: 'plan', on: [tool, question] })).toBe('owner');
  });

  it("gives the engine's pause the tone of what failed, and the owner's a quiet one", () => {
    expect(nowTone({ kind: 'paused', by: 'engine', why: 'The agent could not start.' })).toBe('failed');
    expect(nowTone({ kind: 'paused', by: 'owner', why: null })).toBe('quiet');
    expect(pauseWords('engine', 'The agent could not start.')).toBe('the engine paused the work: The agent could not start.');
    expect(pauseWords('owner', null)).toBe('the owner paused the work');
  });
});

describe('how a run stands', () => {
  const round = (ended: number | null, back: Round['back']): Round => ({
    number: 1,
    began: 0,
    ended,
    given: { kind: 'task' },
    back,
    spent: null,
    context: null,
    compactions: 0,
  });

  it('says a run whose last round failed failed, neither under way nor at rest, and open', () => {
    const failed = round(10, { kind: 'failed', why: 'The agent could not start.', failure: 'other', resets: null });
    expect(runState({ end: null, rounds: [failed] })).toBe('failed');
    expect(RUN_WORDS.failed).toBe('round failed');
    expect(isOpen('failed')).toBe(true);
    // Given again once the work is let on, the round is under way.
    expect(runState({ end: null, rounds: [failed, round(null, null)] })).toBe('running');
    expect(runState({ end: null, rounds: [round(10, { kind: 'asked', asks: [] })] })).toBe('resting');
  });
});

describe('why a run ran, in words', () => {
  it('says each kind of cause, and a run is a way round when every reason was written by a cell after it', () => {
    expect(causeWords({ kind: 'noRecord' })).toBe('the cell had no record');
    expect(causeWords({ kind: 'changed', block: 'plan', was: null, behind: false })).toBe(
      'plan changed in meaning, which its record knew no version of',
    );
    const behind = { kind: 'changed', block: 'opinions', was: 12, behind: true } as const;
    expect(causeWords(behind, true)).toBe('opinions changed in meaning since @12, by a cell after it: a way round');
    expect(isWayRound({ because: [behind] })).toBe(true);
    expect(isWayRound({ because: [] })).toBe(false);
  });

  it('calls a run with one cause from a cell after it among others from before no way round', () => {
    const behind = { kind: 'changed', block: 'opinions', was: 12, behind: true } as const;
    const because = [behind, { kind: 'changed', block: 'plan', was: 3, behind: false } as const];
    expect(isWayRound({ because })).toBe(false);
    expect(because.map((c) => causeWords(c, isWayRound({ because })))).toEqual([
      'opinions changed in meaning since @12, by a cell after it',
      'plan changed in meaning since @3',
    ]);
  });

  it('says a call of an abandoned run with no result was withdrawn with it', () => {
    const row = { call: 4, what: { kind: 'tool', name: 'shell', input: 'ls' } as const, made: 0, done: null, chars: null };
    expect(callState(row)).toBe(`made ${clock(0)}, no result yet`);
    expect(callState(row, true)).toBe(`made ${clock(0)}, withdrawn with its run`);
    expect(callState({ ...row, done: 12_000, chars: 3 }, true)).toBe(`made ${clock(0)}, done in 12s`);
  });

  it('says why a return was sent back', () => {
    expect(unmetWords({ kind: 'asksNothing' })).toBe('it asked for calls and named none');
    expect(unmetWords({ kind: 'notAnOutput', block: 'code' })).toBe('it handed in code, which the cell may not write');
    expect(unmetWords({ kind: 'writtenTwice', block: 'code' })).toBe('it handed in code twice');
    expect(unmetWords({ kind: 'noValue', block: 'ci' })).toBe('it handed in ci, a choice, with no value');
    expect(unmetWords({ kind: 'notAValue', block: 'ci', value: 'amber' })).toBe(
      'it handed in ci as amber, which is not one of its values',
    );
    expect(unmetWords({ kind: 'notAChoice', block: 'plan' })).toBe('it handed in plan with a value, but it is not a choice');
    expect(unmetWords({ kind: 'notHandedIn', block: 'verdict' })).toBe(
      'it did not hand in verdict, which the cell must hand in every time',
    );
  });
});

describe('what was taken, in words', () => {
  it('says times, dollars and tokens short', () => {
    expect(duration(45_000)).toBe('45s');
    expect(duration(252_000)).toBe('4m 12s');
    expect(duration(7_500_000)).toBe('2h 5m');
    expect(duration(4 * 86_400_000 + 3_600_000)).toBe('4d 1h');
    expect(dollars(420_000)).toBe('$0.42');
    expect(dollars(4_000)).toBe('<$0.01');
    expect(dollars(0)).toBe('$0.00');
    expect(tokens(812)).toBe('812');
    expect(tokens(1_234)).toBe('1.2k');
    expect(tokens(14_400)).toBe('14k');
    expect(tokens(1_200_000)).toBe('1.2M');
  });
});
