import { describe, expect, it } from 'vitest';

import { cell, flowOf } from '../testing';
import type { CallRow, Round, Spent, Step, WorkPage } from '../works-api';
import { usageOf } from './used';

const MINUTE = 60_000;

const FLOW = flowOf(
  [
    cell('plan', { inputs: ['request'], outputs: ['plan'] }),
    cell('check', { kind: 'engine', line: null, task: null, act: 'check', inputs: ['plan'], outputs: ['ci'] }),
  ],
  { request: { people: true }, plan: {}, ci: { values: ['green', 'red'] } },
);

const spent = (usd: number, minutes: number): Spent => ({
  usdMicros: usd * 1_000_000,
  inputTokens: 100,
  cachedTokens: 300,
  outputTokens: 10,
  workingMs: minutes * MINUTE,
  rounds: 1,
});

/// A round of the engineer's line from this minute to that, which cost so many dollars.
function round(number: number, from: number, to: number, usd: number, over: Partial<Round> = {}): Round {
  return {
    number,
    began: from * MINUTE,
    ended: to * MINUTE,
    given: { kind: 'task' },
    back: null,
    spent: spent(usd, to - from),
    context: 400,
    compactions: 0,
    session: { line: 'engineer', number: 1, begins: number === 1 },
    tries: [{ attempt: 1, ran: { level: '2', model: 'claude-sonnet-5-5', effort: 'high' }, filled: [] }],
    ...over,
  };
}

function run(number: number, cellId: string, rounds: Round[], calls: CallRow[] = []): Step {
  return {
    kind: 'run',
    run: number,
    cell: cellId,
    at: number,
    time: rounds[0].began,
    because: [],
    basis: [],
    rounds,
    calls,
    end: null,
    spent: spent(0, 0),
  };
}

/// A work whose plan ran a round, was sent back and ran another in its line's first session; whose line then began a
/// session for its context; whose plan ran again in it on another model and called a tool; and whose check ran.
function work(): WorkPage {
  const steps: Step[] = [
    run(1, 'plan', [
      round(1, 0, 10, 1),
      round(2, 20, 25, 1, { given: { kind: 'sentBack', unmet: [] }, context: 800, compactions: 1 }),
    ]),
    { kind: 'session', at: 9, time: 26 * MINUTE, line: 'engineer', why: { kind: 'limit', context: 900, limit: 500 } },
    run(
      2,
      'plan',
      [
        round(1, 30, 40, 3, {
          session: { line: 'engineer', number: 2, begins: true },
          tries: [{ attempt: 1, ran: { level: '3', model: 'claude-opus-5-5', effort: null }, filled: [] }],
        }),
      ],
      [{ call: 4, what: { kind: 'tool', name: 'shell', input: 'npm test' }, made: 40 * MINUTE, done: 45 * MINUTE, chars: 3 }],
    ),
    run(3, 'check', [round(1, 50, 52, 0, { session: null, tries: [], context: null })]),
  ];
  return { steps, flow: FLOW } as WorkPage;
}

describe('what a work used', () => {
  it('sums what its rounds took, and how long it ran from its first round to its last', () => {
    const u = usageOf(work(), 0);
    expect(u.rounds.map((r) => `${r.run}/${r.number}`)).toEqual(['1/1', '1/2', '2/1', '3/1']);
    expect(u.agentRounds).toBe(3);
    expect(u.sentBack).toBe(1);
    expect(u.spent.usdMicros).toBe(5_000_000);
    expect(u.spent.cachedTokens).toBe(1200);
    expect(u.compactions).toBe(1);
    expect([u.start, u.end]).toEqual([0, 52 * MINUTE]);
  });

  it('cuts its time into the models, the tools, the engine and the stretches nothing ran in', () => {
    const u = usageOf(work(), 0);
    expect(u.time).toEqual({ model: 25 * MINUTE, tool: 5 * MINUTE, engine: 2 * MINUTE, idle: 20 * MINUTE });
    expect(u.spans.map((s) => `${s.kind} ${s.start / MINUTE}-${s.end / MINUTE}`)).toEqual([
      'model 0-10',
      'gap 10-20',
      'model 20-25',
      'gap 25-30',
      'model 30-40',
      'tool 40-45',
      'gap 45-50',
      'tool 50-52',
    ]);
  });

  it('counts a round under way to now', () => {
    const page = work();
    const last = page.steps[3];
    if (last.kind !== 'run') throw new Error('no run');
    last.rounds[0] = { ...last.rounds[0], ended: null, spent: null };
    const u = usageOf(page, 60 * MINUTE);
    expect(u.end).toBe(60 * MINUTE);
    expect(u.time.engine).toBe(10 * MINUTE);
    expect(u.rounds[3].underWay).toBe(true);
  });

  it('groups the rounds by the model, level and effort they recorded, and the cost by cell, the dearest first', () => {
    const u = usageOf(work(), 0);
    expect(u.models.map((m) => [m.ran?.model, m.ran?.level, m.ran?.effort, m.rounds, m.usdMicros])).toEqual([
      ['claude-opus-5-5', '3', null, 1, 3_000_000],
      ['claude-sonnet-5-5', '2', 'high', 2, 2_000_000],
    ]);
    expect(u.cells.map((c) => [c.cell, c.rounds, c.usdMicros])).toEqual([
      ['plan', 3, 5_000_000],
      ['check', 1, 0],
    ]);
  });

  it('gives each session of a line why it began and its context after each round', () => {
    const u = usageOf(work(), 0);
    expect(u.sessions.map((s) => [s.line, s.number, s.why, s.rounds, s.context, s.compactions])).toEqual([
      ['engineer', 1, null, 2, 800, 1],
      ['engineer', 2, { kind: 'limit', context: 900, limit: 500 }, 1, 400, 0],
    ]);
    expect(u.sessions[0].points.map((p) => [p.at, p.context, p.compacted])).toEqual([
      [0, 400, false],
      [1, 800, true],
    ]);
  });

  it('says how long each tool ran and how often it was called', () => {
    expect(usageOf(work(), 0).tools).toEqual([{ name: 'shell', calls: 1, ms: 5 * MINUTE }]);
  });
});
