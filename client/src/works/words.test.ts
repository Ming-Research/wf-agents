import { describe, expect, it } from 'vitest';

import type { Now, RanOn, Round, Summary, Wait, Waited } from '../works-api';
import {
  STATE,
  callState,
  causeWords,
  clock,
  dollars,
  duration,
  isOpen,
  isWayRound,
  itemsWords,
  nowTone,
  nowWords,
  outageWords,
  plain,
  ranLast,
  runTook,
  sentence,
  stateOf,
  stateWords,
  stoppedAt,
  tokens,
  unmetWords,
  waitCount,
  waitCounted,
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
  why: 'The cell ran 3 times in a row for what a cell after it wrote, its limit.',
  ends: { kind: 'letGo', cell: 'merge' },
};

describe('what a work does now, in words', () => {
  it('names every state by the one catalogue: the word first, then the cell, then why', () => {
    const since = Date.UTC(2026, 10, 9, 14, 2);
    expect(nowWords(doing({ kind: 'fault', words: 'Two entries at place 3' }))).toBe('Halted · Two entries at place 3');
    expect(nowWords(doing({ kind: 'merged' }))).toBe('Merged');
    expect(nowWords(doing({ kind: 'cancelled' }))).toBe('Cancelled');
    expect(nowWords(doing({ kind: 'round', cell: 'implement', run: 5, round: 2, since }))).toBe('Running · implement');
    // A paused or held work names the cell it would go on at, as the engine names it.
    expect(nowWords(doing({ kind: 'paused', at: 'plan', by: 'owner', why: null }))).toBe('Paused · plan');
    expect(nowWords(doing({ kind: 'everything', at: 'plan', by: 'owner' }))).toBe('Paused · plan');
    // A stall is said as the engine says it, its names bare on the line.
    const stall = 'Nothing new runs this last cell, and no cell can write what runs it.';
    expect(nowWords(doing({ kind: 'stalled', at: 'M2', on: [], by: [], why: stall }))).toBe(
      'Halted · M2 · Nothing new runs this last cell, and no cell can write what runs it.',
    );
    // What the engine's list holds waits for you, said by the engine's reasons, each a sentence, after the cell the
    // work waits at, named once where the state names one.
    const pause: Wait = {
      key: 'pause',
      why: 'The engine paused the work. The agent could not start',
      ends: { kind: 'resume' },
    };
    const engine = doing({ kind: 'paused', at: 'plan', by: 'engine', why: 'The agent could not start' }, [pause]);
    expect(nowWords(engine)).toBe(
      'Waiting for you · plan · The engine paused the work. The agent could not start.',
    );
    // The cell is said once, in its place: the reason beside it does not name it.
    const atLimit = nowWords(doing({ kind: 'waits', at: 'merge', on: [precondition, { kind: 'limit', runs: 3 }] }, [limit]));
    expect(atLimit).toBe('Waiting for you · merge · The cell ran 3 times in a row for what a cell after it wrote, its limit.');
    expect(atLimit.split('merge').length).toBe(2);
    // Two reasons are two sentences, a name in backticks said bare on the line.
    const ruling: Wait = {
      key: 'write-verdict',
      why: '`verdict` keeps `verdict = yes` from holding.',
      ends: { kind: 'write', block: 'verdict', value: 'yes' },
    };
    const asked: Wait = { key: 'call-4', why: 'Show the cached tokens apart?', ends: { kind: 'reply', call: 4 } };
    expect(nowWords(doing({ kind: 'waits', at: 'merge', on: [precondition, question] }, [asked, ruling, pause]))).toBe(
      'Waiting for you · merge · Show the cached tokens apart? verdict keeps verdict = yes from holding. The engine ' +
        'paused the work. The agent could not start.',
    );
    // A question the merge waits for is said by what it asks, each once.
    const merge = (name: string, ask: string): Wait => ({
      key: `answer-${name}`,
      why: ask,
      ends: { kind: 'answer', question: name, version: 4 },
    });
    const both = [merge('scope', '选哪一种？'), merge('keep', 'Keep the old table?')];
    expect(nowWords(doing({ kind: 'waits', at: 'M1', on: [precondition] }, both))).toBe(
      'Waiting for you · M1 · 选哪一种？ Keep the old table?',
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

  it('names the cell a work is stopped at, for the owner or by the engine, and none while it is not so stopped', () => {
    expect(stoppedAt(doing({ kind: 'waits', at: 'merge', on: [precondition] }, [limit]))).toBe('merge');
    expect(stoppedAt(doing({ kind: 'stalled', at: 'I1', on: [], by: [], why: 'x' }))).toBe('I1');
    const open = { cell: 'I1', state: 'resting' as const, finished: 1, record: 7, open: 9 };
    expect(stoppedAt({ now: { kind: 'fault', words: 'x' }, cells: [open], waits: [] })).toBe('I1');
    expect(stoppedAt(doing({ kind: 'paused', at: 'plan', by: 'owner', why: null }))).toBeNull();
    expect(stoppedAt(doing({ kind: 'next', cell: 'plan' }))).toBeNull();
  });

  it('says the state word of the catalogue, and the cell only where the state names one', () => {
    expect(stateOf(doing({ kind: 'paused', at: 'plan', by: 'owner', why: null }))).toEqual({
      tone: 'quiet',
      word: STATE.paused,
      cell: 'plan',
    });
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
    // A work that waits for the calls of tools the engine carries out is neither running nor done: it waits for each
    // tool by its name, where it runs, in the yellow of a call (decision a-call-waited-for-is-no-run).
    const tool: Waited = { kind: 'call', call: 2, who: null, what: { kind: 'tool', name: 'ci', input: 'all' } };
    const calling = { now: { kind: 'waits', at: 'M1', on: [tool] } as Now, cells: [{ cell: 'M1', state: 'calling' as const, finished: 0, record: null, open: 1 }], waits: [] };
    expect(stateOf(calling)).toEqual({ tone: 'calling', word: 'Waiting for ci by the engine', cell: 'M1' });
    // An act of the engine's own under way is a round, and runs.
    expect(stateOf({ ...calling, now: { kind: 'round', cell: 'M1', run: 3, round: 1, since: 0 } })).toEqual({
      tone: 'running',
      word: STATE.running,
      cell: 'M1',
    });
    expect(stateOf(doing({ kind: 'waits', at: 'merge', on: [precondition] }, [limit]))).toEqual({
      tone: 'owner',
      word: STATE.owner,
      cell: 'merge',
    });
  });

  it('says each kind of thing a work waits for, each work by its number as W12', () => {
    const tool: Waited = { kind: 'call', call: 2, who: null, what: { kind: 'tool', name: 'shell', input: 'ls' } };
    const returns: Waited = { kind: 'call', call: 3, who: 'owner', what: { kind: 'atLimit', limit: { of: 'returns', sentBack: 2 } } };
    const dependencies: Waited = { kind: 'items', items: [5, 7] };
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
    expect(waitedWords(dependencies)).toBe('#5 and #7');
    expect(waitedWords(settled)).toBe(
      'No cell that judged the work says it is not settled, which the owner or cell T1 can make hold',
    );
    expect(nowTone(doing({ kind: 'waits', at: 'plan', on: [dependencies] }))).toBe('quiet');
    // A work that waits for the items it depends on has one word for it, the bar's too, and its line names them once.
    expect(stateOf(doing({ kind: 'waits', at: 'S1', on: [dependencies] })).word).toBe(STATE.items);
    expect(STATE.items).toBe('Waiting for the items it depends on');
    expect(nowWords(doing({ kind: 'waits', at: 'S1', on: [dependencies] }))).toBe(
      'Waiting for the items it depends on · S1 · #5 and #7',
    );
    // One that was cancelled is said so, in the catalogue's words: the work waits for it for good.
    const forGood: Waited = { kind: 'items', items: [5, 7], cancelled: [7] };
    expect(nowWords(doing({ kind: 'waits', at: 'S1', on: [forGood] }))).toBe(
      'Waiting for the items it depends on · S1 · #5 and #7; it waits for good on #7, cancelled',
    );
    expect(itemsWords([5])).toBe('#5');
    expect(itemsWords([5, 6, 7])).toBe('#5, #6 and #7');
    const main: Waited = { kind: 'outside', source: 'main', cell: 'M1', why: 'What fails also fails on main.' };
    expect(waitedWords(main)).toBe('main to move, before M1 runs again: What fails also fails on main.');
    expect(stateOf(doing({ kind: 'waits', at: 'M1', on: [main] })).word).toBe('Waiting for main');
  });

  it('gives what waits on the owner and a halted work the one red tone of a stop, and the owner\'s pause a quiet one', () => {
    const pause = { key: 'pause', why: 'x', ends: { kind: 'resume' as const } };
    expect(nowTone(doing({ kind: 'paused', at: 'plan', by: 'engine', why: 'x' }, [pause]))).toBe(
      'owner',
    );
    expect(nowTone(doing({ kind: 'fault', words: 'Two entries at place 3' }))).toBe('failed');
    expect(nowTone(doing({ kind: 'stalled', at: 'M2', on: [], by: [], why: 'x' }))).toBe('failed');
    expect(nowTone(doing({ kind: 'paused', at: 'plan', by: 'owner', why: null }))).toBe('quiet');
    expect(nowTone(doing({ kind: 'everything', at: 'plan', by: 'owner' }))).toBe('quiet');
  });

  it('says a work that waits for the calls of tools by each tool and where it runs, and how long it has waited, the same wherever its state is said', () => {
    const made = Date.UTC(2026, 10, 9, 14, 2);
    const ci: Waited = {
      kind: 'call',
      call: 7,
      who: null,
      what: { kind: 'tool', name: 'run_tests', input: '' },
      made,
      at: 'https://github.com/o/r/actions/runs/1',
    };
    const engine: Waited = { kind: 'call', call: 8, who: null, what: { kind: 'tool', name: 'push', input: '' } };
    const work = doing({ kind: 'waits', at: 'I1', on: [ci] });
    expect(nowTone(work)).toBe('calling');
    expect(stateWords(work, made + 252_000)).toBe('Waiting for run_tests in CI · 4m 12s · I1');
    // The calls are said by the state's words alone, never again as what it waits for.
    expect(nowWords(work, made + 252_000)).toBe('Waiting for run_tests in CI · 4m 12s · I1');
    // A call the engine does not say when it made has no time waited, and one it runs itself is said so.
    expect(stateWords(doing({ kind: 'waits', at: 'I1', on: [engine] }))).toBe('Waiting for push by the engine · I1');
    expect(stateOf(doing({ kind: 'waits', at: 'I1', on: [ci, engine] })).word).toBe(
      'Waiting for run_tests in CI and push by the engine',
    );
  });

  it('says the one count one way', () => {
    expect(waitCount(1)).toBe('1 thing waits for you');
    expect(waitCount(3)).toBe('3 things wait for you');
  });

  it('says the wait every work shares in two sentences where no check found it down, the error of the round that met it not among them', () => {
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
    expect(said).toMatch(
      /^The agents' command line cannot run since \d\d-\d\d \d\d:\d\d\. No agent's round begins until it runs again\.$/,
    );
    expect(said).not.toContain('W12');
    expect(said).not.toContain('Not logged in');
    expect(said).not.toContain('install');
  });
});

describe("a run in the last column of a work's history", () => {
  const round = (tries: RanOn[]): Round => ({
    number: 1,
    began: 0,
    ended: 10,
    given: { kind: 'task' },
    back: null,
    spent: null,
    context: null,
    compactions: 0,
    session: null,
    tries: tries.map((ran) => ({ attempt: 1, filled: [], ran })),
    missed: null,
    estimated: false,
    judged: { cost: null, time: null },
  });
  const sonnet: RanOn = { level: '2', model: 'claude-sonnet-5-5', effort: 'high' };
  const opus: RanOn = { level: '3', model: 'claude-opus-5-5', effort: null };

  it('says how long a run took to its end, and an open one until now', () => {
    const end = { kind: 'finished' as const, at: 9, time: 70_000, writes: [] };
    expect(runTook({ time: 10_000, end }, 999_999)).toBe(60_000);
    expect(runTook({ time: 10_000, end: null }, 25_000)).toBe(15_000);
    expect(isOpen({ end: null })).toBe(true);
    expect(isOpen({ end })).toBe(false);
  });

  it("says what a run ran on by its latest round's last try that kept it, and nothing of a run no round of which kept it", () => {
    expect(ranLast({ rounds: [round([sonnet]), round([sonnet, opus])] })).toEqual(opus);
    // A round whose records keep no try, as an act of the engine's own, is passed over for the round before it.
    expect(ranLast({ rounds: [round([sonnet]), round([])] })).toEqual(sonnet);
    expect(ranLast({ rounds: [round([])] })).toBeNull();
    expect(ranLast({ rounds: [] })).toBeNull();
  });
});

describe('why a run ran, in words', () => {
  it('says each kind of cause, and a run is a way round when every reason was written by a cell after it', () => {
    expect(causeWords({ kind: 'noRecord' })).toBe('it had not run before');
    expect(causeWords({ kind: 'changed', block: 'plan', was: null, behind: false })).toBe('plan new to it');
    expect(causeWords({ kind: 'results', calls: [4, 5, 6] })).toBe('the results of the 3 calls it asked for came back');
    const behind = { kind: 'changed', block: 'opinions', was: 12, behind: true } as const;
    expect(causeWords(behind, true)).toBe('opinions changed since the v12 it last saw, written by a cell after it: a way round');
    expect(isWayRound({ because: [behind] })).toBe(true);
    expect(isWayRound({ because: [] })).toBe(false);
  });

  it('calls a run with one cause from a cell after it among others from before no way round', () => {
    const behind = { kind: 'changed', block: 'opinions', was: 12, behind: true } as const;
    const because = [behind, { kind: 'changed', block: 'plan', was: 3, behind: false } as const];
    expect(isWayRound({ because })).toBe(false);
    expect(because.map((c) => causeWords(c, isWayRound({ because })))).toEqual([
      'opinions changed since the v12 it last saw, written by a cell after it',
      'plan changed since the v3 it last saw',
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
    expect(
      unmetWords({ kind: 'nothingBut', block: 'deep_needed', value: 'yes', besides: ['plan_owner', 'r1_questions'] }),
    ).toBe('it handed in deep_needed as yes, and plan_owner, r1_questions besides, which no return of that value holds');
    expect(unmetWords({ kind: 'notWatched', name: 'on_nothing' })).toBe(
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
    expect(unmetWords({ kind: 'notATest', path: 'src/lib.rs' })).toBe(
      'it changed src/lib.rs, which is none of the project\'s tests',
    );
    expect(unmetWords({ kind: 'wordOwed', name: 'review', watched: ['code', 'title'] })).toBe(
      'it dealt with review, woken by a point in it, and answered none of its points, changed none of code, title, and asked no question the work waits for',
    );
    expect(unmetWords({ kind: 'noPoint', block: 'review_ok', value: 'no', name: 'review' })).toBe(
      'it handed in review_ok as no, which does not pass, and gave no review',
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

describe('reasons said one after another', () => {
  it('each read as one sentence, a full stop given where it has none, an end in Chinese counted as one', () => {
    expect(sentence('The agent could not start')).toBe('The agent could not start.');
    expect(sentence('Which way? ')).toBe('Which way?');
    expect(sentence('It spent $4.')).toBe('It spent $4.');
    expect(sentence('选哪一种？')).toBe('选哪一种？');
    expect(sentence('先做这个。')).toBe('先做这个。');
    expect(sentence('快！')).toBe('快！');
    expect(plain('`merge` requires `verdict = yes`.')).toBe('merge requires verdict = yes.');
  });

  it('count what waits for the owner after its number, one way', () => {
    expect(waitCounted(1)).toBe('thing waits for you');
    expect(waitCounted(2)).toBe('things wait for you');
  });
});
