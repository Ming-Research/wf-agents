// The list of rounds, one part for a work's page and an architect's: the architect's as one list across all its works,
// opened on the newest, read further at both ends as it scrolls there, and opened around a round a link names, which
// is brought to the list's middle and chosen; every round drawn alike, the one that ended a work as plainly as the row
// any other took; who ran each in its cell's colour on the chart; the round that started a new session of its line
// marked, with why; and a rule where each of the architect's works began.

import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

import type { ArchitectRounds, CompiledGraph, RoundSummary } from './api';
import { time } from './format';
import { newSessionWords, sessionMarkWords, startedSession } from './round';
import {
  centredTop,
  endsNear,
  inSight,
  merged,
  NEAR_END,
  openRead,
  refreshRead,
  type RoundsWindow,
  windowPath,
} from './rounds';
import { RoundsList } from './pages/WorkChart';

// Drawn once, outside a browser: the engine's state is not known, and the lines' colours are as the flow gives them.
vi.mock('./live', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./live')>()),
  useStatus: () => null,
  useActivity: () => [],
  useShared: () => [
    {
      graph: 'architect',
      lines: [
        { name: 'architect', desc: 'The architect', colour: 'purple' },
        { name: 'architect_review', desc: 'The review of its batch', colour: 'indigo' },
      ],
    },
  ],
}));

const cell = (id: string, kind: string, line: string | null) => ({
  id,
  kind,
  gate: false,
  title: `cell ${id}`,
  step: null,
  needs: null,
  line,
  stage: null,
  inputs: [],
  prompt_by: null,
  schema: null,
  rows: [],
});

const g: CompiledGraph = {
  name: 'architect',
  start: 'S1',
  end: 'E2',
  within: 'outcome',
  asking: [],
  receive: null,
  counters: [],
  rules: [],
  interject: null,
  cells: [
    cell('S1', 'engine', null),
    cell('U1', 'agent', 'architect'),
    cell('H4', 'agent', 'architect'),
    cell('B2', 'agent', 'architect_review'),
    cell('X1', 'tool', null),
    cell('A3', 'owner', null),
    cell('R2', 'agent', null),
    cell('E2', 'engine', null),
  ],
};

const round = (work: number, n: number, at: string, extra: Partial<RoundSummary> = {}): RoundSummary => ({
  work,
  n,
  cell: at,
  line: null,
  outcome: 'took',
  transition: 'next',
  why: null,
  started_at: work * 1000 + n,
  ended_at: work * 1000 + n + 1,
  model: null,
  cost_usd: null,
  compactions: 0,
  tool_runs: 0,
  progress: null,
  not_asked: null,
  tokens: null,
  session: null,
  new_session: null,
  ...extra,
});

const key = (r: RoundSummary) => `R${r.work}-${r.n}`;
const keys = (w: RoundsWindow) => w.rounds.map(key);
const got = (rounds: RoundSummary[], older: boolean, newer: boolean): ArchitectRounds => ({ rounds, older, newer });

describe("the architect's rounds, one list across its works", () => {
  it('opens on the newest, and around a round a link names when it is not listed, else chooses it where it is', () => {
    expect(openRead(null, null)).toEqual({ kind: 'newest' });
    expect(windowPath('p', { kind: 'newest' })).toBe('/api/v1/architect/rounds?project=p&limit=100');
    const listed: RoundsWindow = { rounds: [round(32, 9, 'H4'), round(32, 10, 'B2')], older: true, newer: false };
    expect(openRead(listed, null)).toBeNull();
    expect(openRead(listed, { work: 32, n: 10 })).toBeNull();
    expect(openRead(listed, { work: 30, n: 4 })).toEqual({ kind: 'around', round: { work: 30, n: 4 } });
    expect(windowPath('p', { kind: 'around', round: { work: 30, n: 4 } })).toBe(
      '/api/v1/architect/rounds?project=p&around=R30-4&limit=100',
    );
    // Opened around a round, the window is what came back.
    const around = merged(listed, { kind: 'around', round: { work: 30, n: 4 } }, got([round(30, 3, 'H4'), round(30, 4, 'H4')], true, true));
    expect(keys(around)).toEqual(['R30-3', 'R30-4']);
    expect([around.older, around.newer]).toEqual([true, true]);
  });

  it('reads older rounds before those listed, and newer ones after them, as it scrolls to either end', () => {
    const w: RoundsWindow = { rounds: [round(31, 7, 'H4'), round(32, 1, 'S1')], older: true, newer: true };
    expect(windowPath('p', { kind: 'older', before: { work: 31, n: 7 } })).toBe(
      '/api/v1/architect/rounds?project=p&before=R31-7&limit=100',
    );
    const older = merged(w, { kind: 'older', before: { work: 31, n: 7 } }, got([round(31, 5, 'U1'), round(31, 6, 'H4')], false, true));
    expect(keys(older)).toEqual(['R31-5', 'R31-6', 'R31-7', 'R32-1']);
    expect([older.older, older.newer]).toEqual([false, true]);
    const newer = merged(older, { kind: 'newer', after: { work: 32, n: 1 } }, got([round(32, 2, 'U1')], true, false));
    expect(keys(newer)).toEqual(['R31-5', 'R31-6', 'R31-7', 'R32-1', 'R32-2']);
    expect([newer.older, newer.newer]).toEqual([false, false]);
    // The list asks for more at the end it is scrolled near, where more are left, and at neither while it is not drawn.
    const list = { scrollTop: 0, scrollHeight: 2000, clientHeight: 500 };
    expect(endsNear(list, { older: true, newer: true })).toEqual(['newer']);
    expect(endsNear({ ...list, scrollTop: 1500 - NEAR_END }, { older: true, newer: true })).toEqual(['older']);
    expect(endsNear({ ...list, scrollTop: 700 }, { older: true, newer: true })).toEqual([]);
    expect(endsNear(list, { older: true, newer: false })).toEqual([]);
    expect(endsNear({ scrollTop: 0, scrollHeight: 0, clientHeight: 0 }, { older: true, newer: true })).toEqual([]);
  });

  it("reads again, as the architect's rounds change, those from its newest work's open round on, while it shows the newest", () => {
    const running = round(33, 4, 'H4', { outcome: 'running', transition: null, ended_at: null });
    const w: RoundsWindow = { rounds: [round(33, 2, 'U1'), round(33, 3, 'H4'), running], older: true, newer: false };
    expect(refreshRead(w)).toEqual({ kind: 'newer', after: { work: 33, n: 3 } });
    const ended = merged(w, { kind: 'newer', after: { work: 33, n: 3 } }, got([{ ...running, outcome: 'took', transition: 'done' }, round(33, 5, 'B2')], true, false));
    expect(keys(ended)).toEqual(['R33-2', 'R33-3', 'R33-4', 'R33-5']);
    expect(ended.rounds[2].outcome).toBe('took');
    expect(refreshRead(ended)).toEqual({ kind: 'newer', after: { work: 33, n: 5 } });
    // Newer rounds left unread: nothing listed can change.
    expect(refreshRead({ ...w, newer: true })).toBeNull();
  });

  it('brings a round a link names to the middle of the list when it is out of sight, and never past either end', () => {
    const list = { scrollTop: 0, scrollHeight: 3000, clientHeight: 400 };
    const row = { top: 1500, height: 30 };
    expect(inSight(row, list)).toBe(false);
    expect(centredTop(row, list)).toBe(1315);
    expect(inSight(row, { ...list, scrollTop: centredTop(row, list) })).toBe(true);
    expect(centredTop({ top: 20, height: 30 }, list)).toBe(0);
    expect(centredTop({ top: 2990, height: 30 }, list)).toBe(2600);
  });
});

/// The rows of a list as drawn: each round's name, whether it is chosen, and its cells' text; a rule where a work began.
function rowsOf(html: string): string[] {
  return [...html.matchAll(/<tr( data-round="[^"]*")? class="([^"]*)"[^>]*>(.*?)<\/tr>/g)].map((m) =>
    m[2] === 'rounds-began' ? `-- ${m[3].replace(/<[^>]+>/g, '')}` : `${m[2].includes('selected') ? '* ' : ''}${m[3].replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ').trim()}`,
  );
}

describe('the list of rounds', () => {
  const list = (rounds: RoundSummary[], older = false) =>
    renderToStaticMarkup(<RoundsList rounds={rounds} g={g} selected={{ work: 33, n: 2 }} onRound={() => {}} older={older} />);

  it('lists the newest first, draws the round that ended a work as plainly as the row any other took, and parts the works', () => {
    const rounds = [
      round(32, 1, 'S1', { transition: 'started' }),
      round(32, 2, 'E2', { outcome: 'ended', transition: null }),
      round(33, 1, 'S1', { transition: 'started' }),
      round(33, 2, 'H4', { line: 'architect', transition: 'done' }),
      round(33, 3, 'H4', { line: 'architect', outcome: 'sent_back', transition: null, why: 'no outcome' }),
    ];
    const html = list(rounds, true);
    expect(rowsOf(html)).toEqual([
      'R33-3 H4 architect sent back no outcome 0s',
      '* R33-2 H4 architect done 0s',
      'R33-1 S1 engine started 0s',
      `-- ${time(33 * 1000 + 1)}`,
      'R32-2 E2 engine ended 0s',
      'R32-1 S1 engine started 0s',
      `-- ${time(32 * 1000 + 1)}`,
    ]);
    // "ended" is the row's plain words, no state's chip; a return sent back keeps its chip.
    expect(html).not.toMatch(/data-component="Label">ended</);
    expect(html).toMatch(/data-variant="attention" data-component="Label">sent back</);
  });

  it('draws who ran each round in its cell colour on the chart, filled, and the cell code before it', () => {
    const html = list([
      round(33, 1, 'S1'),
      round(33, 2, 'H4', { line: 'architect' }),
      round(33, 3, 'B2', { line: 'architect_review' }),
      round(33, 4, 'X1'),
      round(33, 5, 'A3'),
    ]);
    const who = [...html.matchAll(/<span class="cell-id">([^<]*)<\/span> <span class="who-chip ([^ "]*)[^"]*"[^>]*>([^<]*)</g)].map(
      (m) => `${m[1]} ${m[3]} ${m[2]}`,
    );
    expect(who).toEqual([
      'A3 owner kind-owner',
      'X1 tools kind-tool',
      'B2 architect_review role-indigo',
      'H4 architect role-purple',
      'S1 engine kind-engine',
    ]);
    expect(html).toMatch(/class="who-chip [^"]*" data-size="small" data-variant="secondary"/);
  });

  it('marks the round that started a new session of its line, saying why, and no round of a cell on no line', () => {
    const first = round(33, 2, 'U1', { line: 'architect', session: 4, new_session: 'line' });
    const limit = round(33, 3, 'U1', { line: 'architect', session: 5, new_session: 'limit' });
    const owner = round(33, 4, 'H4', { line: 'architect', session: 6, new_session: 'owner' });
    const cellOpens = round(33, 5, 'U1', { line: 'architect', session: 7, new_session: 'cell' });
    const goesOn = round(33, 6, 'H4', { line: 'architect', session: 7, new_session: null });
    const gate = round(33, 7, 'R2', { line: 'R2', session: 8, new_session: 'line' });
    const unknown = round(33, 8, 'H4', { line: 'architect', session: 9, new_session: 'wake' });
    expect([first, limit, owner, cellOpens, goesOn, gate, unknown].map(startedSession)).toEqual([true, true, true, true, false, false, true]);
    expect(sessionMarkWords(first)).toBe('A new session: its line had none going on');
    expect(sessionMarkWords(limit)).toBe("A new session: its line's session was past the context limit");
    expect(sessionMarkWords(owner)).toBe('A new session: you asked for one');
    expect(sessionMarkWords(cellOpens)).toBe('A new session: cell U1 starts one');
    expect(sessionMarkWords(round(1, 1, 'H4', { new_session: 'model' }))).toBe("A new session: its line's model changed");
    expect(sessionMarkWords(round(1, 1, 'H4', { new_session: 'account' }))).toBe("A new session: the agents' login account changed");
    // A reason the page does not know, as a round recorded before it said why, keeps the mark without why.
    expect(sessionMarkWords(unknown)).toBe('A new session');
    // The round's panel says the same among who ran it.
    expect(newSessionWords(limit)).toBe("started a new session, as its line's session was past the context limit");
    expect(newSessionWords(gate)).toBe('started a session of its own');
    const html = list([first, goesOn, gate]);
    const marks = [...html.matchAll(/<tr data-round="([^"]*)"[\s\S]*?<\/tr>/g)]
      .filter((m) => m[0].includes('session-mark'))
      .map((m) => [m[1], /class="session-mark" title="([^"]*)"/.exec(m[0])?.[1]]);
    expect(marks).toEqual([['R33-2', 'A new session: its line had none going on']]);
  });

  it("draws a work's rounds alone with no rule, its first round being where the list ends", () => {
    const html = list([round(7, 1, 'S1'), round(7, 2, 'H4')]);
    expect(html).not.toContain('rounds-began');
  });
});
