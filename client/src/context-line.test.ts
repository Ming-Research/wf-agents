// The context chart of a work's usage: a line to each session, each round drawn from where its first call stood to
// where its last call ended, so that how much the context grew within a round shows and a session that opened small is
// not drawn as the line of the session before it carrying on.

import { describe, expect, it } from 'vitest';

import type { RoundSummary, RoundTokens } from './api';
import { contextLine, OPENING_SHIFT, usageOf } from './usage';

const tokens = (end: number, first: number | null): RoundTokens => ({
  steps: 1,
  input: 0,
  read: 0,
  written_5m: 0,
  written_1h: 0,
  output: 0,
  thinking: 0,
  context_end: end,
  context_first: first,
  window: 1_000_000,
  context_before: null,
  level: null,
  effort: null,
  others: [],
  costs: null,
});

const round = (n: number, session: number, end: number, first: number | null): RoundSummary => ({
  work: 1,
  n,
  cell: 'a',
  line: 'engineer',
  outcome: 'took',
  transition: 'done',
  why: null,
  set_aside: null,
  started_at: n * 10,
  ended_at: n * 10 + 5,
  model: 'm',
  cost_usd: 1,
  compactions: 0,
  tool_runs: 0,
  progress: null,
  not_asked: null,
  tokens: tokens(end, first),
  session,
  new_session: null,
});

const cells = new Map([['a', { kind: 'agent', title: 'Implement' }]]);

describe('the context chart', () => {
  // The first session grows to 341K over two rounds; the second opens at 22K and reaches 341K within its first round,
  // as the second engineer session of work 252 did, whose first point alone was at 341K.
  const rounds = [round(1, 1, 120_000, 25_000), round(2, 1, 341_000, 119_000), round(3, 2, 341_000, 22_000)];
  const sessions = [
    { id: 1, line: 'engineer', started: 'line', ended: 25 },
    { id: 2, line: 'engineer', started: 'line', ended: null },
  ];
  const u = usageOf([{ work: 1, rounds, sessions }], cells, 100);
  const [first, second] = u.sessions;

  it('draws each round from its first call to its end, the rise within it showing', () => {
    expect(contextLine(first)).toEqual([
      { at: 0, context: 25_000 },
      { at: 0, context: 120_000 },
      { at: 1 - OPENING_SHIFT, context: 119_000 },
      { at: 1, context: 341_000 },
    ]);
  });

  it('starts a new session where its first call stood, not where the session before it ended', () => {
    expect(second.id).toBe(2);
    const line = contextLine(second);
    expect(line[0]).toEqual({ at: 2 - OPENING_SHIFT, context: 22_000 });
    expect(line[line.length - 1]).toEqual({ at: 2, context: 341_000 });
    // Nothing of it is on the line of the session before, whose end it would otherwise seem to carry on from.
    const before = contextLine(first);
    expect(line.some((v) => before.some((b) => b.at === v.at && b.context === v.context))).toBe(false);
    expect(line[0].context).toBeLessThan(before[before.length - 1].context);
  });

  it('gives only where a round ended when the record has no first call', () => {
    const old = usageOf([{ work: 1, rounds: [round(1, 1, 90_000, null), round(2, 1, 120_000, 90_000)], sessions }], cells, 100);
    expect(contextLine(old.sessions[0])).toEqual([
      { at: 0, context: 90_000 },
      { at: 1 - OPENING_SHIFT, context: 90_000 },
      { at: 1, context: 120_000 },
    ]);
  });
});
