import { describe, expect, it } from 'vitest';

import type { CompiledGraph, FlowDiff, GraphView, Matter, Wake, WorkSummary } from './api';
import {
  againstInOrder,
  architectState,
  changesAnswer,
  choicesOf,
  queueParts,
  rulingAnswer,
  shownWake,
} from './architect';
import { inGraph, subjectKey } from './flow/context';
import { graphOfCell, withGraph } from './flow/FlowScreen';
import { besides, BESIDE_GRAPHS, GRAPH_TITLE } from './graph/flowLayout';
import { named } from './live';
import { diffMarks } from './pages/Flow';
import { href, parse, type Route } from './route';

const work = (id: number, extra: Partial<WorkSummary> = {}): WorkSummary => ({
  id,
  item: null,
  title: null,
  project: null,
  graph: 'architect',
  flow_version: 2,
  at: 'A12',
  at_title: 'This wake is done',
  stage: 'asleep',
  state: 'done',
  paused: null,
  paused_by: null,
  ended: null,
  waits: null,
  answered: null,
  branch: null,
  pr: null,
  started_at: id * 100,
  last_at: id * 100 + 50,
  rounds: 10,
  sent_back: 0,
  failed: 0,
  running: null,
  cost_usd: 1,
  open_asks: 0,
  open_design_cards: 0,
  ...extra,
});

const wake = (id: number, extra: Partial<WorkSummary> = {}, handling: Wake['handling'] = null): Wake => ({
  work: work(id, extra),
  woke_for: [],
  handled: 0,
  handling,
});

const matter = (id: number, extra: Partial<Matter> = {}): Matter => ({
  id,
  kind: 'owner_words',
  what: `matter ${id}`,
  source: 'owner',
  item: null,
  title: null,
  work: null,
  round: null,
  at: id,
  due: null,
  due_item: null,
  held: null,
  held_by: null,
  waits: null,
  waited_ms: null,
  answers: null,
  ...extra,
});

describe("the architect's page", () => {
  it('shows the wake the address names, else the one going on, else the latest', () => {
    const wakes = [wake(9), wake(8, { state: 'asking', at: 'P2' }), wake(7)];
    expect(shownWake(wakes, 7)?.work.id).toBe(7);
    expect(shownWake(wakes, null)?.work.id).toBe(8);
    expect(shownWake(wakes, 99)?.work.id).toBe(8);
    expect(shownWake([wake(9), wake(7)], null)?.work.id).toBe(9);
    expect(shownWake([], null)).toBeNull();
  });

  it('splits the queue into what waits for the architect, what is held for the owner and what is not due yet', () => {
    const queue = [
      matter(1, { waits: 'agent', source: 'work', work: 4, round: 2 }),
      matter(2, { waits: 'owner' }),
      matter(3, { held: 'owner_ask', held_by: 5 }),
      matter(4, { kind: 'next_look', due: 900, source: 'architect' }),
      matter(5, { kind: 'next_look', due: 600, source: 'architect' }),
    ];
    const parts = queueParts(queue);
    expect(parts.waiting.map((m) => m.id)).toEqual([1, 2]);
    expect(parts.held.map((m) => m.id)).toEqual([3]);
    expect(parts.later.map((m) => m.id)).toEqual([4, 5]);
    // Asleep since its last wake ended, with the matter that falls due soonest.
    expect(architectState([wake(9), wake(7)], queue)).toEqual({
      kind: 'asleep',
      since: 950,
      next: queue[4],
      waiting: 2,
    });
    expect(architectState([], queue)).toEqual({ kind: 'never', waiting: 2 });
    const awake = wake(10, { state: 'running', at: 'A7' }, { matter: 'Split #4?' });
    expect(architectState([awake, wake(9)], [])).toEqual({
      kind: 'awake',
      wake: awake.work,
      handling: { matter: 'Split #4?' },
      waiting: 0,
    });
  });

  it('sends a ruling with words unless the owner agrees', () => {
    const body = { from: 'architect', kind: 'study', choices: [['satisfied', 'yes'], ['study_on', 'more'], ['no_change', 'no']] };
    const choices = choicesOf(body);
    expect(choices).toEqual([
      ['satisfied', 'yes'],
      ['study_on', 'more'],
      ['no_change', 'no'],
    ]);
    expect(rulingAnswer(choices, 'satisfied', ' ')).toEqual({ choice: 'satisfied', text: '' });
    expect(rulingAnswer(choices, 'study_on', '')).toBeNull();
    expect(rulingAnswer(choices, 'study_on', ' the costs ')).toEqual({ choice: 'study_on', text: 'the costs' });
    expect(rulingAnswer(choices, null, 'x')).toBeNull();
    expect(rulingAnswer(choices, 'other', 'x')).toBeNull();
    expect(choicesOf({ kind: 'study' })).toEqual([]);
  });

  it('approves all, some or none of the changes, saying why for what is not approved', () => {
    const all = [1, 2, 3];
    expect(changesAnswer('approve_all', [], all, '')).toEqual({ choice: 'approve_all', approved: [1, 2, 3], text: '' });
    expect(changesAnswer('approve_none', [], all, '')).toBeNull();
    expect(changesAnswer('approve_none', [2], all, 'not now')).toEqual({ choice: 'approve_none', approved: [], text: 'not now' });
    expect(changesAnswer('approve_some', [3, 1], all, 'two waits')).toEqual({
      choice: 'approve_some',
      approved: [1, 3],
      text: 'two waits',
    });
    expect(changesAnswer('approve_some', [1, 3], all, '')).toBeNull();
    expect(changesAnswer('approve_some', [], all, 'why')).toBeNull();
    expect(changesAnswer('approve_some', [1, 2, 3], all, 'why')).toBeNull();
    expect(changesAnswer(null, [1], all, 'why')).toBeNull();
  });

  it('puts the articles broken first, then those at risk, then those kept', () => {
    const rows = [
      { principle: 'a', status: 'kept' },
      { principle: 'b', status: 'at_risk' },
      { principle: 'c', status: 'broken' },
      { principle: 'd', status: 'kept' },
      { principle: 'e', status: 'broken' },
    ];
    expect(againstInOrder(rows).map((r) => r.principle)).toEqual(['c', 'e', 'b', 'a', 'd']);
    expect(againstInOrder(null)).toEqual([]);
  });

  it('has an address that names the wake, the tab, the round and the cell', () => {
    const architect = { page: 'architect' as const, wake: null, tab: 'asks' as const, round: null, cell: null };
    const routes: Route[] = [
      architect,
      { ...architect, tab: 'queue' },
      { ...architect, wake: 12, round: 3, tab: 'rounds' },
      { ...architect, wake: 12, cell: 'A6' },
    ];
    for (const r of routes) {
      const at = href(r);
      const i = at.indexOf('?');
      expect(i < 0 ? parse(at, '') : parse(at.slice(0, i), at.slice(i))).toEqual(r);
    }
    expect(href({ ...architect, wake: 12, round: 3 })).toBe('/architect?wake=12&round=R12-3');
    expect(href(architect)).toBe('/architect');
    // A round of another wake, or one with no wake named, opens none; a tab the page does not know is the asks.
    expect(parse('/architect', '?wake=12&round=R11-3&tab=x')).toEqual({ ...architect, wake: 12 });
    expect(parse('/architect', '?round=R11-3')).toEqual(architect);
  });

  it('is read again when what it shows changed', () => {
    expect(named({ kind: 'architect' })).toEqual(['architect']);
  });
});

function graph(name: string, cells: string[]): CompiledGraph {
  return {
    name,
    start: cells[0],
    end: cells[cells.length - 1],
    within: 'outcome',
    asking: [],
    receive: null,
    counters: [],
    rules: [],
    interject: null,
    cells: cells.map((id) => ({
      id,
      kind: 'engine',
      gate: false,
      title: id,
      step: null,
      needs: null,
      line: null,
      stage: null,
      inputs: [],
      prompt_by: null,
      schema: null,
      rows: [],
    })),
  };
}

const view = (name: string): GraphView => ({ name, lines: [], cells: [], common_format: '' });

describe('the flow page with both graphs', () => {
  const graphs = [
    { g: graph('engineer', ['0', '1', '20']), view: view('engineer') },
    { g: graph('architect', ['A0', 'A1', 'A12']), view: view('architect') },
  ];

  it('lays the graphs side by side, their tops in line under their names', () => {
    const at = besides([
      { width: 300, height: 900 },
      { width: 200, height: 500 },
    ]);
    expect(at).toEqual({ x: [0, 300 + BESIDE_GRAPHS], top: GRAPH_TITLE, width: 500 + BESIDE_GRAPHS, height: GRAPH_TITLE + 900 });
    // One graph needs no name above it.
    expect(besides([{ width: 300, height: 900 }])).toEqual({ x: [0], top: 0, width: 300, height: 900 });
  });

  it('opens a cell in the graph that has it, and what a panel opens in its own graph', () => {
    expect(graphOfCell(graphs, 'A1')?.g.name).toBe('architect');
    expect(graphOfCell(graphs, '1')?.g.name).toBe('engineer');
    expect(withGraph({ kind: 'cell', cell: 'A0' }, graphs)).toEqual({ kind: 'cell', cell: 'A0', graph: 'architect' });
    expect(withGraph({ kind: 'rule', rule: 'general' }, graphs)).toEqual({ kind: 'rule', rule: 'general', graph: 'engineer' });
    expect(withGraph({ kind: 'cell', cell: 'A0', graph: 'engineer' }, graphs)).toEqual({
      kind: 'cell',
      cell: 'A0',
      graph: 'engineer',
    });
    expect(inGraph({ kind: 'rule', rule: 'general' }, 'architect')).toEqual({ kind: 'rule', rule: 'general', graph: 'architect' });
    expect(inGraph({ kind: 'round', round: 3 }, 'architect')).toEqual({ kind: 'round', round: 3 });
    // A subject is the same whatever order its fields were written in.
    expect(subjectKey({ graph: 'architect', kind: 'cell', cell: 'A0' })).toBe(
      subjectKey({ kind: 'cell', cell: 'A0', graph: 'architect' }),
    );
  });

  it('marks what a comparison changed in either graph', () => {
    const diff: FlowDiff = {
      graphs: [
        { name: 'engineer', status: 'changed', changes: [], cells: [{ id: '1', title: '1', status: 'changed', changes: [] }] },
        { name: 'architect', status: 'added', changes: [], cells: [{ id: 'A0', title: 'A0', status: 'added', changes: [] }] },
      ],
    };
    const marks = diffMarks(graphs, diff);
    expect([...marks[0].changed]).toEqual(['1']);
    expect([...marks[0].added]).toEqual([]);
    expect([...marks[1].added]).toEqual(['A0']);
    expect(diffMarks(graphs, null).every((m) => m.added.size === 0 && m.changed.size === 0)).toBe(true);
  });
});
