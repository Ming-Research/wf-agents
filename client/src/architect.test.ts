import { describe, expect, it } from 'vitest';

import type {
  ArchitectWork,
  CompiledGraph,
  FlowDiff,
  GraphView,
  Handled,
  Matter,
  Observation,
  WorkSummary,
} from './api';
import {
  againstInOrder,
  architectState,
  cardWords,
  holdWords,
  observationsShown,
  outcomes,
  matterKind,
  matterSentence,
  matterSteps,
  queueParts,
  shownWork,
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
  project: 'p',
  graph: 'architect',
  flow_version: 2,
  moved_from: null,
  at: 'E2',
  at_title: 'This work is done',
  stage: 'record',
  state: 'done',
  paused: null,
  paused_by: null,
  ended: null,
  waits: null,
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

const architectWork = (id: number, extra: Partial<WorkSummary> = {}, handling: ArchitectWork['handling'] = null): ArchitectWork => ({
  work: work(id, extra),
  started_for: [],
  handled: 0,
  handling,
});

const matter = (id: number, extra: Partial<Matter> = {}): Matter => ({
  id,
  kind: 'owner_words',
  detail: 'said',
  what: `matter ${id}`,
  source: 'owner',
  item: null,
  title: null,
  work: null,
  round: null,
  at: id,
  asked: null,
  resume_cell: null,
  answered: false,
  waits: 'owner',
  waited_ms: 0,
  urgent: false,
  fix: null,
  card: null,
  ...extra,
});

const observation = (id: number, extra: Partial<Observation> = {}): Observation => ({
  id,
  what: `observation ${id}`,
  tags: [],
  source: 'the architect, at cell H3',
  times_reported: 1,
  refs: [],
  folded_into: null,
  closed: null,
  closed_why: null,
  created: id,
  updated: id,
  ...extra,
});

describe('a matter in words', () => {
  it('names its kind as what is to be done, and what more the kind says only where that helps', () => {
    expect(matterKind('set_straight', 'off_policy')).toBe('Plan fix: does not follow the policy');
    expect(matterKind('set_straight', 'dependencies')).toBe('Plan fix: dependencies');
    expect(holdWords('set_straight')).toBe('No new works start until the plan fixes are done');
    expect(matterKind('design_card', null)).toBe('Look at a design card');
    expect(matterKind('check_policy', null)).toBe('Check whether the policy still holds');
    expect(matterKind('make_plan', null)).toBe('Plan what to do next');
    expect(matterKind('group_talk', null)).toBe('Read what was said in the groups');
    expect(matterKind('report', 'followup')).toBe('Go through side reports');
    expect(matterKind('report', 'fault')).toBe('Look into a fault of wf-agents');
    expect(matterKind('owner_words', 'said')).toBe('Your words');
    expect(matterKind('failure', 'tool_failed')).toBe('Have a failure fixed: a tool failed');
    expect(matterKind('new_kind', null)).toBe('new kind');
  });

  it('says first what is to be done, a place to set straight by where it is and what is wrong there', () => {
    const fix = { where: 'item 12', wrong: 'It waits for item 15, which is done.', steps: ['Drop the link.', '', 'Note it.'] };
    expect(matterSentence(matter(1, { kind: 'set_straight', fix }))).toBe('item 12: It waits for item 15, which is done.');
    expect(matterSteps(matter(1, { kind: 'set_straight', fix }))).toEqual(['Drop the link.', 'Note it.']);
    expect(matterSentence(matter(2))).toBe('matter 2');
    expect(matterSteps(matter(2))).toEqual([]);
  });

  it('says where the design card it is about stands', () => {
    const card = (status: string, rulings: unknown[] = [], replaced_by: number | null = null) => ({
      id: 7,
      status,
      rulings: rulings as never,
      replaced_by,
    });
    expect(cardWords(card('open'))).toBe('Design card 7 waits for a ruling.');
    expect(cardWords(card('replaced', [], 9))).toBe('Design card 7 was replaced by design card 9.');
    const confirmed = { by: 'owner', decisions: [{ decision: 1, verdict: 'confirm' }] };
    expect(cardWords(card('ruled', [confirmed]))).toBe('Design card 7: you confirmed it.');
    const against = {
      by: 'architect',
      decisions: [
        { decision: 1, verdict: 'confirm' },
        { decision: 2, verdict: 'disagree', text: 'too wide' },
      ],
    };
    expect(cardWords(card('ruled', [confirmed, against]))).toBe('Design card 7: architect disagreed with decision 2: too wide.');
  });
});

describe("the architect's page", () => {
  it("shows the architect's work the address names, else the one going on, else the latest", () => {
    const works = [architectWork(9), architectWork(8, { state: 'asking', at: 'P3' }), architectWork(7)];
    expect(shownWork(works, 7)?.work.id).toBe(7);
    expect(shownWork(works, null)?.work.id).toBe(8);
    expect(shownWork(works, 99)?.work.id).toBe(8);
    expect(shownWork([architectWork(9), architectWork(7)], null)?.work.id).toBe(9);
    expect(shownWork([], null)).toBeNull();
  });

  it('splits the queue into what is in it and what an ask of the owner holds, and says how the architect stands', () => {
    const queue = [
      matter(1, { kind: 'engineer_question', detail: 'split', waits: 'agent', source: 'work', work: 4, round: 2 }),
      matter(2),
      matter(3, { kind: 'failure', waits: null, asked: 5, resume_cell: 'H7' }),
      matter(4, { kind: 'report', waits: 'none', asked: 6, resume_cell: 'H5', answered: true }),
    ];
    const parts = queueParts(queue);
    expect(parts.waiting.map((m) => m.id)).toEqual([1, 2, 4]);
    expect(parts.held.map((m) => m.id)).toEqual([3]);
    // Idle since its last work ended.
    expect(architectState([architectWork(9), architectWork(7)], queue)).toEqual({ kind: 'idle', since: 950, waiting: 3 });
    expect(architectState([], queue)).toEqual({ kind: 'never', waiting: 3 });
    // A work of it runs, handling the matter its latest pick chose.
    const going = architectWork(10, { state: 'running', at: 'H3' }, 1);
    expect(architectState([going, architectWork(9)], queue)).toEqual({
      kind: 'working',
      work: going.work,
      handling: queue[0],
      waiting: 3,
    });
    expect(architectState([architectWork(10, { state: 'running', at: 'Q0' })], [])).toMatchObject({ handling: null, waiting: 0 });
  });

  it('shows the observations of the tag chosen and the words searched for, those closed only when asked', () => {
    const all = [
      observation(3, { tags: ['ui'], refs: ['PR #12'] }),
      observation(2, { tags: ['perf', 'ui'], what: 'The board is slow' }),
      observation(1, { tags: ['perf'], closed: 'owner', closed_why: 'Not worth it' }),
    ];
    const ids = (os: Observation[]) => os.map((o) => o.id);
    expect(ids(observationsShown(all, '', '', false))).toEqual([3, 2]);
    expect(ids(observationsShown(all, '', '', true))).toEqual([3, 2, 1]);
    expect(ids(observationsShown(all, 'perf', '', true))).toEqual([2, 1]);
    expect(ids(observationsShown(all, '', ' SLOW ', false))).toEqual([2]);
    expect(ids(observationsShown(all, '', 'pr #12', false))).toEqual([3]);
    expect(ids(observationsShown(all, 'ui', 'CELL h3', false))).toEqual([3, 2]);
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

  it('has an address that names the project, the work, the tab, the round and the cell', () => {
    const architect = {
      page: 'architect' as const,
      project: null,
      work: null,
      tab: 'rounds' as const,
      round: null,
      cell: null,
      only: null,
      span: '7d' as const,
    };
    const routes: Route[] = [
      architect,
      { ...architect, tab: 'changes' },
      { ...architect, tab: 'usage', span: '30d' },
      { ...architect, work: 12, only: 'sent_back' },
      { ...architect, project: 'p', tab: 'observations' },
      { ...architect, work: 12, round: 3, tab: 'usage' },
      { ...architect, work: 12, cell: 'H3' },
    ];
    for (const r of routes) {
      const at = href(r);
      const i = at.indexOf('?');
      expect(i < 0 ? parse(at, '') : parse(at.slice(0, i), at.slice(i))).toEqual(r);
    }
    expect(href({ ...architect, work: 12, round: 3 })).toBe('/architect?work=12&round=R12-3');
    expect(href(architect)).toBe('/architect');
    expect(href({ ...architect, project: 'p', work: 12 })).toBe('/architect?project=p&work=12');
    // A round of another work, or one with no work named, opens none; a tab the page does not know, such as the asks, the
    // conversation and the queue it had before, is its rounds, which the page opens on.
    expect(parse('/architect', '?tab=asks')).toEqual(architect);
    expect(parse('/architect', '?tab=conversation')).toEqual(architect);
    // The policy and what to do next, which were its tabs, are in the panel beside the one-to-one.
    expect(parse('/architect', '?tab=policy')).toEqual(architect);
    expect(parse('/architect', '?tab=next')).toEqual(architect);
    expect(parse('/architect', '?tab=queue')).toEqual(architect);
    expect(parse('/architect', '?work=12&round=R11-3&tab=x')).toEqual({ ...architect, work: 12 });
    expect(parse('/architect', '?round=R11-3')).toEqual(architect);
  });

  it('is read again when what it shows changed', () => {
    expect(named({ kind: 'architect' })).toEqual(['architect', 'messages', 'system']);
  });

  it('says what became of a matter handled, one line for each kind of outcome, or why nothing did', () => {
    const handled = (extra: Partial<Handled> = {}): Handled => ({
      matter: 4,
      kind: 'engineer_question',
      detail: 'split',
      what: 'Split #3?',
      work: 9,
      round: 12,
      cell: 'H3',
      at: 1,
      reply: null,
      reply_to: null,
      told: 0,
      made: 0,
      dropped: 0,
      collected: 0,
      observations: [],
      actions: [],
      messages: [],
      ask: null,
      why: null,
      ...extra,
    });
    const lines = outcomes(
      handled({
        reply: 'Split it in two.',
        reply_to: 7,
        made: 2,
        dropped: 1,
        observations: ['new', 'close'],
        actions: [
          { kind: 'pause_work', item: 3 },
          { kind: 'start_work_over', item: 4 },
        ],
        messages: ['q', 'q'],
      }),
    );
    expect(lines.map((o) => [o.key, o.label, o.text, o.tab])).toEqual([
      ['reply', 'Replied to work 7:', 'Split it in two.', null],
      ['changes', 'Item changes:', '2 made, 1 dropped', 'changes'],
      ['observations', 'Observations:', '1 recorded, 1 closed', 'observations'],
      ['actions', 'Did at once:', 'paused the work on #3; started the work on #4 over', null],
      ['messages', 'Wrote to the architect of:', 'q', null],
    ]);
    expect(outcomes(handled({ why: 'Already done by #2.' })).map((o) => [o.key, o.text])).toEqual([
      ['nothing', 'Already done by #2.'],
    ]);
    expect(outcomes(handled({ told: 1 })).map((o) => o.label)).toEqual(['Told you once']);
    expect(outcomes(handled({ told: 3 })).map((o) => o.label)).toEqual(['Told you 3 times']);
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
    { g: graph('engineer', ['S0', 'R1', 'X1', 'E1']), view: view('engineer') },
    { g: graph('architect', ['S0', 'S1', 'X1', 'E2']), view: view('architect') },
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
    expect(graphOfCell(graphs, 'S1')?.g.name).toBe('architect');
    expect(graphOfCell(graphs, 'R1')?.g.name).toBe('engineer');
    expect(withGraph({ kind: 'cell', cell: 'S1' }, graphs)).toEqual({ kind: 'cell', cell: 'S1', graph: 'architect' });
    // A name both graphs have is the first graph's, unless the subject names its graph.
    expect(graphOfCell(graphs, 'X1')?.g.name).toBe('engineer');
    expect(withGraph({ kind: 'cell', cell: 'S0' }, graphs)).toEqual({ kind: 'cell', cell: 'S0', graph: 'engineer' });
    expect(withGraph({ kind: 'cell', cell: 'X1', graph: 'architect' }, graphs)).toEqual({
      kind: 'cell',
      cell: 'X1',
      graph: 'architect',
    });
    expect(withGraph({ kind: 'rule', rule: 'general' }, graphs)).toEqual({ kind: 'rule', rule: 'general', graph: 'engineer' });
    expect(withGraph({ kind: 'cell', cell: 'S0', graph: 'architect' }, graphs)).toEqual({
      kind: 'cell',
      cell: 'S0',
      graph: 'architect',
    });
    expect(inGraph({ kind: 'rule', rule: 'general' }, 'architect')).toEqual({ kind: 'rule', rule: 'general', graph: 'architect' });
    expect(inGraph({ kind: 'round', round: 3 }, 'architect')).toEqual({ kind: 'round', round: 3 });
    // A subject is the same whatever order its fields were written in.
    expect(subjectKey({ graph: 'architect', kind: 'cell', cell: 'S0' })).toBe(
      subjectKey({ kind: 'cell', cell: 'S0', graph: 'architect' }),
    );
  });

  it('marks what a comparison changed in either graph', () => {
    const diff: FlowDiff = {
      graphs: [
        {
          name: 'engineer',
          status: 'changed',
          changes: [],
          cells: [{ id: 'R1', title: 'R1', status: 'changed', changes: [] }],
        },
        { name: 'architect', status: 'added', changes: [], cells: [{ id: 'S0', title: 'S0', status: 'added', changes: [] }] },
      ],
    };
    const marks = diffMarks(graphs, diff);
    expect([...marks[0].changed]).toEqual(['R1']);
    expect([...marks[0].added]).toEqual([]);
    expect([...marks[1].added]).toEqual(['S0']);
    expect(diffMarks(graphs, null).every((m) => m.added.size === 0 && m.changed.size === 0)).toBe(true);
  });
});
