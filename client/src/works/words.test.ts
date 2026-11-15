import { describe, expect, it } from 'vitest';

import type { Now, Round, Step, Summary, Wait, Waited } from '../works-api';
import {
  RUN_WORDS,
  STATE,
  callState,
  causeWords,
  clock,
  dollars,
  duration,
  isOpen,
  isWayRound,
  letGoFailed,
  nowTone,
  nowWords,
  outageWords,
  pauseWords,
  runState,
  stateOf,
  tokens,
  unmetWords,
  waitCount,
  waitedWords,
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

/// A work doing this now, waiting on the owner for these, as the engine's list says.
const doing = (now: Now, waits: Wait[] = []): Pick<Summary, 'now' | 'cells' | 'waits'> => ({ now, cells: [], waits });

const limit: Wait = {
  key: 'limit',
  why: 'merge ran 3 times in a row for what a cell after it wrote, its limit.',
  ends: { kind: 'letGo', cell: 'merge' },
};

describe('what a work does now, in words', () => {
  it('names every state by the one catalogue: the word first, then the cell, then why', () => {
    const since = Date.UTC(2026, 10, 9, 14, 2);
    expect(nowWords(doing({ kind: 'fault', words: 'Two entries at place 3' }))).toBe('Halted · Two entries at place 3');
    expect(nowWords(doing({ kind: 'merged' }))).toBe('Merged');
    expect(nowWords(doing({ kind: 'cancelled' }))).toBe('Cancelled');
    expect(nowWords(doing({ kind: 'round', cell: 'implement', run: 5, round: 2, since }))).toBe('Running · implement');
    expect(nowWords(doing({ kind: 'paused', by: 'owner', why: null }))).toBe('Paused');
    expect(nowWords(doing({ kind: 'everything', by: 'owner' }))).toBe('Paused');
    expect(nowWords(doing({ kind: 'stalled', at: 'M2', on: [], by: [] }))).toBe(
      'Halted · M2 · nothing new runs this last cell, and what runs it no cell can write',
    );
    // What the engine's list holds waits for you, said by the engine's reasons.
    const pause: Wait = { key: 'pause', why: 'The engine paused the work: the agent could not start', ends: { kind: 'resume' } };
    expect(nowWords(doing({ kind: 'paused', by: 'engine', why: 'the agent could not start' }, [pause]))).toBe(
      'Waiting for you · The engine paused the work: the agent could not start',
    );
    expect(nowWords(doing({ kind: 'waits', at: 'merge', on: [precondition, { kind: 'limit', runs: 3 }] }, [limit]))).toBe(
      'Waiting for you · merge · merge ran 3 times in a row for what a cell after it wrote, its limit.',
    );
    // What waits for another names who gives it.
    expect(nowWords(doing({ kind: 'waits', at: 'plan', on: [{ kind: 'slot' }] }))).toBe(
      'Waiting for the engine · plan · a free slot, which the engine gives in its order',
    );
    expect(nowWords(doing({ kind: 'waits', at: 'M2', on: [{ kind: 'turn', holder: 12, since }] }))).toBe(
      "Waiting for W12 · M2 · the project's turn to check and merge, which W12 holds",
    );
    expect(nowWords(doing({ kind: 'next', cell: 'plan' }))).toBe('Running · plan');
  });

  it('says the state word of the catalogue, and the cell only where the state names one', () => {
    expect(stateOf(doing({ kind: 'paused', by: 'owner', why: null }))).toEqual({ tone: 'quiet', word: STATE.paused, cell: null });
    expect(stateOf(doing({ kind: 'fault', words: 'x' }))).toEqual({ tone: 'failed', word: STATE.halted, cell: null });
    const architect: Waited = {
      kind: 'call',
      call: 3,
      who: 'architect',
      what: { kind: 'question', who: 'architect', question: 'Which table?' },
    };
    expect(stateOf(doing({ kind: 'waits', at: 'plan', on: [architect] }))).toEqual({
      tone: 'quiet',
      word: 'Waiting for the architect',
      cell: 'plan',
    });
    // A call the engine carries out for a cell is running: the cell's yellow says it is a call.
    const tool: Waited = { kind: 'call', call: 2, who: null, what: { kind: 'tool', name: 'ci', input: 'all' } };
    const calling = { now: { kind: 'waits', at: 'M1', on: [tool] } as Now, cells: [{ cell: 'M1', state: 'calling' as const, finished: 0, record: null, open: 1 }], waits: [] };
    expect(stateOf(calling)).toEqual({ tone: 'running', word: STATE.running, cell: 'M1' });
    expect(stateOf(doing({ kind: 'waits', at: 'merge', on: [precondition] }, [limit]))).toEqual({
      tone: 'owner',
      word: STATE.owner,
      cell: 'merge',
    });
  });

  it('says each kind of thing a work waits for, each work by its number as W12', () => {
    const tool: Waited = { kind: 'call', call: 2, who: null, what: { kind: 'tool', name: 'shell', input: 'ls' } };
    const returns: Waited = { kind: 'call', call: 3, who: 'owner', what: { kind: 'atLimit', limit: { of: 'returns', sentBack: 2 } } };
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
    expect(waitedWords(question)).toBe("the owner's answer to a question");
    expect(waitedWords(tool)).toBe('the result of the tool shell');
    expect(waitedWords(returns)).toBe('the owner to let the run go on: it reached its limit, 2 returns sent back');
    expect(waitedWords(precondition)).toBe('The owner approved the merge, which the owner can make hold');
    expect(waitedWords(dependencies)).toBe(
      'The items it depends on are merged, which the outside (dependencies) can make hold',
    );
    expect(waitedWords(settled)).toBe(
      'No cell that judged the work says it is not settled, which the owner or cell T1 can make hold',
    );
    expect(nowTone(doing({ kind: 'waits', at: 'plan', on: [dependencies] }))).toBe('quiet');
    expect(stateOf(doing({ kind: 'waits', at: 'S1', on: [dependencies] })).word).toBe('Waiting for dependencies');
    const main: Waited = { kind: 'outside', source: 'main', cell: 'M1', why: 'What fails also fails on main.' };
    expect(waitedWords(main)).toBe('main to move, before M1 runs again: What fails also fails on main.');
    expect(stateOf(doing({ kind: 'waits', at: 'M1', on: [main] })).word).toBe('Waiting for main');
  });

  it('gives what waits on the owner and a halted work the one red tone of a stop, and the owner\'s pause a quiet one', () => {
    expect(nowTone(doing({ kind: 'paused', by: 'engine', why: 'x' }, [{ key: 'pause', why: 'x', ends: { kind: 'resume' } }]))).toBe(
      'owner',
    );
    expect(nowTone(doing({ kind: 'fault', words: 'Two entries at place 3' }))).toBe('failed');
    expect(nowTone(doing({ kind: 'stalled', at: 'M2', on: [], by: [] }))).toBe('failed');
    expect(nowTone(doing({ kind: 'paused', by: 'owner', why: null }))).toBe('quiet');
    expect(nowTone(doing({ kind: 'everything', by: 'owner' }))).toBe('quiet');
    expect(pauseWords('engine', 'The agent could not start.')).toBe('the engine paused the work: The agent could not start.');
    expect(pauseWords('owner', null)).toBe('the owner paused the work');
  });

  it('says the one count one way', () => {
    expect(waitCount(1)).toBe('1 thing waits for you');
    expect(waitCount(3)).toBe('3 things wait for you');
  });

  it('says the work of the round that met the wait every work shares as W12, and no way to mend it of its own', () => {
    const said = outageWords({
      service: 'agents',
      failure: 'cannotRun',
      why: 'Not logged in',
      work: 12,
      run: 3,
      round: 1,
      since: 0,
      checked: null,
      until: { kind: 'checked', at: 0 },
      holds: true,
    });
    expect(said).toContain('(W12, round 1 of run 3: Not logged in)');
    expect(said).not.toContain('install');
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
    session: null,
    tries: [],
    missed: null,
    estimated: false,
    judged: { cost: null, time: null },
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
    const row = { call: 4, what: { kind: 'tool', name: 'shell', input: 'ls' } as const, made: 0, done: null, chars: null, time: null };
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
    expect(unmetWords({ kind: 'notATrigger', name: 'on_nothing' })).toBe(
      'it said it dealt with on_nothing, which does not run the cell',
    );
    expect(unmetWords({ kind: 'notATool', name: 'browse' })).toBe(
      'it asked for browse, which is not a tool the cell may ask for',
    );
    expect(unmetWords({ kind: 'unchangedNotWritten', block: 'plan' })).toBe(
      'it named plan as unchanged in meaning, and did not write it',
    );
    expect(unmetWords({ kind: 'unexplained', path: 'AGENTS.md' })).toBe(
      'it left AGENTS.md, a file the project protects, changed, and said not why',
    );
    expect(unmetWords({ kind: 'unanswered', block: 'check' })).toBe(
      'it dealt with check, and committed nothing and asked nothing',
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

describe('a failed letting go of what an ended work held', () => {
  const paused = (at: number, by: 'owner' | 'engine'): Step => ({ kind: 'paused', at, time: at, by, why: 'HTTP 502' });
  const resumed = (at: number): Step => ({ kind: 'resumed', at, time: at });
  const merged = { now: { kind: 'merged' as const }, paused: 'engine' as const };

  it("is the engine's pause of the ended work that was not let on since", () => {
    expect(letGoFailed(merged, [paused(1, 'engine')])).toBe('HTTP 502');
    expect(letGoFailed({ ...merged, now: { kind: 'cancelled' } }, [paused(1, 'engine')])).toBe('HTTP 502');
  });

  it('is none for a work going on, one let on since, one the owner paused, or one not paused', () => {
    expect(letGoFailed({ now: { kind: 'next', cell: 'plan' }, paused: 'engine' }, [paused(1, 'engine')])).toBeNull();
    expect(letGoFailed(merged, [paused(1, 'engine'), resumed(2)])).toBeNull();
    expect(letGoFailed(merged, [paused(1, 'owner')])).toBeNull();
    expect(letGoFailed({ ...merged, paused: null }, [paused(1, 'engine')])).toBeNull();
  });
});
