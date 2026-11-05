import { describe, expect, it } from 'vitest';

import type {
  Architect,
  ArchitectStanding,
  ArchitectWork,
  CompiledGraph,
  FlowDiff,
  GraphView,
  Handled,
  MadeThing,
  Matter,
  MatterProgress,
  OwnerAsk,
  Status,
  WorkSummary,
} from './api';
import {
  architectMark,
  architectState,
  architectTotals,
  cardWords,
  holdWords,
  matterKind,
  matterResult,
  matterSentence,
  matterSteps,
  queueParts,
  releaseWords,
  standing,
} from './architect';
import { inGraph, subjectKey } from './flow/context';
import { graphOfCell, withGraph } from './flow/FlowScreen';
import { besides, BESIDE_GRAPHS, GRAPH_TITLE } from './graph/flowLayout';
import { named } from './live';
import { diffMarks } from './pages/Flow';
import { href, moved, parse, type Route } from './route';
import { VIEWS, viewOf } from './TopBar';

const work = (id: number, extra: Partial<WorkSummary> = {}): WorkSummary => ({
  id,
  item: null,
  title: null,
  project: 'p',
  graph: 'architect',
  flow_version: 2,
  moved_from: null,
  at: 'R1',
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
  ran_ms: 0,
  time: { from: 0, until: 0, model_ms: 0, tool_ms: 0, engine_ms: 0, bar: '' },
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
  handling,
  listens: false,
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

describe('a matter in words', () => {
  it('names its kind as what is to be done, and what more the kind says only where that helps', () => {
    expect(matterKind('set_straight', 'off_policy')).toBe('Plan fix: does not follow the policy');
    expect(matterKind('set_straight', 'dependencies')).toBe('Plan fix: dependencies');
    expect(holdWords('set_straight')).toBe('No new works start until the plan fixes are done');
    expect(holdWords('owner')).toBe('You stopped new works from starting');
    expect(holdWords('architect')).toBe('No new works start: the architect holds them');
    expect(matterKind('design_card', null)).toBe('Look at a decision card');
    expect(matterKind('open_question', null)).toBe("Answer a design's open question");
    expect(matterKind('check_policy', null)).toBe('Check whether the policy still holds');
    expect(matterKind('make_plan', null)).toBe('Plan what to do next');
    expect(matterKind('group_talk', null)).toBe('Read what was said in the groups');
    expect(matterKind('report', 'followup')).toBe('Go through side reports');
    expect(matterKind('report', 'fault')).toBe('Look into a fault of wf-agents');
    expect(matterKind('owner_words', 'said')).toBe('Your words');
    expect(matterKind('failure', 'tool_failed')).toBe('Have a failure fixed: a tool failed');
    // A question carries no kind, and one queued with a kind before is named without it.
    expect(matterKind('engineer_question', null)).toBe(matterKind('engineer_question', 'split'));
    expect(matterKind('new_kind', null)).toBe('new kind');
  });

  it('says first what is to be done, a place to set straight by where it is and what is wrong there', () => {
    const fix = { where: 'item 12', wrong: 'It waits for item 15, which is done.', steps: ['Drop the link.', '', 'Note it.'] };
    expect(matterSentence(matter(1, { kind: 'set_straight', fix }))).toBe('item 12: It waits for item 15, which is done.');
    expect(matterSteps(matter(1, { kind: 'set_straight', fix }))).toEqual(['Drop the link.', 'Note it.']);
    expect(matterSentence(matter(2))).toBe('matter 2');
    expect(matterSteps(matter(2))).toEqual([]);
  });

  it('says where the decision card it is about stands', () => {
    const card = (status: string, rulings: unknown[] = [], replaced_by: number | null = null) => ({
      id: 7,
      status,
      rulings: rulings as never,
      replaced_by,
    });
    expect(cardWords(card('open'))).toBe('Decision card 7 waits for a ruling.');
    expect(cardWords(card('replaced', [], 9))).toBe('Decision card 7 was replaced by decision card 9.');
    const confirmed = { by: 'owner', decisions: [{ decision: 1, verdict: 'confirm' }] };
    expect(cardWords(card('ruled', [confirmed]))).toBe('Decision card 7: you confirmed it.');
    const against = {
      by: 'architect',
      decisions: [
        { decision: 1, verdict: 'confirm' },
        { decision: 2, verdict: 'disagree', text: 'too wide' },
      ],
    };
    expect(cardWords(card('ruled', [confirmed, against]))).toBe('Decision card 7: architect disagreed with decision 2: too wide.');
  });
});

describe("the architect's page", () => {
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
      listens: false,
      waiting: 3,
    });
    expect(architectState([architectWork(10, { state: 'running', at: 'N0' })], [])).toMatchObject({ handling: null, waiting: 0 });
  });

  it('adds up the rounds, run time and cost of all its works, and takes the latest activity and the cell of the one that goes on', () => {
    const stand = (works: ArchitectWork[], queue: Matter[] = []): ArchitectStanding => ({
      project: 'p',
      works,
      queue,
      asking: false,
    });
    const newest = architectWork(9, { rounds: 4, ran_ms: 1_000, cost_usd: 1.5, last_at: 950, at: 'R1' });
    const older = architectWork(7, { rounds: 6, ran_ms: 2_000, cost_usd: 0.25, last_at: 750, at: 'E1' });
    // Sums over all its works, and the latest activity of them; none going on, the newest work's cell.
    expect(architectTotals(stand([newest, older]))).toMatchObject({ rounds: 10, ran_ms: 3_000, cost_usd: 1.75, last_at: 950 });
    expect(architectTotals(stand([newest, older])).work?.at).toBe('R1');
    // The latest activity is the greatest, in whatever order the works come, and a work with none lowers nothing.
    const late = architectWork(5, { last_at: 5_000 });
    const quiet = architectWork(4, { last_at: null });
    expect(architectTotals(stand([newest, late, quiet])).last_at).toBe(5_000);
    expect(architectTotals(stand([quiet])).last_at).toBeNull();
    // A work that goes on is where the architect is, the newest or not.
    const going = architectWork(8, { state: 'running', at: 'H3', rounds: 1, ran_ms: 10, cost_usd: 0.5, last_at: 800 });
    const t = architectTotals(stand([newest, going, older]));
    expect(t).toMatchObject({ rounds: 11, ran_ms: 3_010, cost_usd: 2.25, last_at: 950 });
    expect(t.work?.id).toBe(8);
    expect(t.work?.at).toBe('H3');
    // It never worked: nothing to add and nowhere it is.
    expect(architectTotals(stand([]))).toEqual({ rounds: 0, ran_ms: 0, cost_usd: 0, last_at: null, work: null });
  });

  it('is worked out from the lean read of every architect or from its page alike, a question of its own waiting included', () => {
    const page = (extra: Partial<Architect> = {}): Architect =>
      ({ project: 'p', projects: ['p'], works: [], queue: [], asks: [], ...extra }) as unknown as Architect;
    const ask = (answer: unknown): OwnerAsk => ({ id: 1, answer }) as unknown as OwnerAsk;
    // An ask of any of its works that is not answered is a question waiting; the lean read says it as a flag.
    expect(standing(page({ asks: [ask(null), ask({ text: 'yes' })] })).asking).toBe(true);
    expect(standing(page({ asks: [ask({ text: 'yes' })] })).asking).toBe(false);
    expect(standing(page())).toEqual({ project: 'p', works: [], queue: [], asking: false });
    // Waiting for the owner's next words, held back with a queue and every work paused, asking: the three states the
    // mark in the list of conversations and the architect's row in the works both say, one function.
    const status = {
      paused: false,
      running: [],
      started: [],
      releasing: [],
      waiting_for_slot: [],
      stopping: [],
      stepping: [],
      architects_started: [],
      engine_stopping: false,
      flow_version: 2,
    } as unknown as Status;
    const listening = { ...architectWork(10, { state: 'waiting', at: 'N0' }), listens: true };
    expect(architectMark(standing(page({ works: [listening] })), status)).toEqual({
      state: 'waiting',
      words: 'waiting for your next words',
    });
    expect(architectMark(standing(page({ queue: [matter(1)] })), { ...status, paused: true } as Status)).toEqual({
      state: 'paused',
      words: 'held back: every work is paused',
    });
    expect(architectMark(standing(page({ works: [listening], asks: [ask(null)] })), status)).toEqual({
      state: 'asking',
      words: 'asking',
    });
  });

  it('has an address that names the project, the tab, the round of any of its works and the cell', () => {
    const architect = {
      page: 'architect' as const,
      project: null,
      tab: 'rounds' as const,
      round: null,
      cell: null,
      span: '7d' as const,
    };
    const routes: Route[] = [
      architect,
      { ...architect, tab: 'changes' },
      { ...architect, tab: 'usage', span: '30d' },
      { ...architect, round: { work: 11, n: 3 } },
      { ...architect, project: 'p', tab: 'changes' },
      { ...architect, round: { work: 12, n: 3 }, tab: 'usage' },
      { ...architect, cell: 'H3' },
    ];
    for (const r of routes) {
      const at = href(r);
      const i = at.indexOf('?');
      expect(i < 0 ? parse(at, '') : parse(at.slice(0, i), at.slice(i))).toEqual(r);
    }
    // The page is under the works.
    expect(href({ ...architect, round: { work: 12, n: 3 } })).toBe('/works/architect?round=R12-3');
    expect(href(architect)).toBe('/works/architect');
    expect(href({ ...architect, project: 'p', round: { work: 12, n: 3 } })).toBe('/works/architect?project=p&round=R12-3');
    // A round of another work, or one with no work named, opens none; a tab the page does not know, such as the asks, the
    // conversation and the queue it had before, is its rounds, which the page opens on.
    expect(parse('/works/architect', '?tab=asks')).toEqual(architect);
    expect(parse('/works/architect', '?tab=conversation')).toEqual(architect);
    // The policy and what to do next, which were its tabs, are in the panel beside the one-to-one.
    expect(parse('/works/architect', '?tab=policy')).toEqual(architect);
    expect(parse('/works/architect', '?tab=next')).toEqual(architect);
    expect(parse('/works/architect', '?tab=queue')).toEqual(architect);
    // The observations, which were its tab, are a view of their own.
    expect(parse('/works/architect', '?tab=observations')).toEqual(architect);
    // A stretch of its work, which an address named before its rounds were one list, is no longer chosen, and an
    // address that names no round opens none.
    expect(parse('/works/architect', '?work=12&round=R11-3&tab=x')).toEqual({ ...architect, round: { work: 11, n: 3 } });
    expect(parse('/works/architect', '?work=12&only=failed')).toEqual(architect);
    expect(parse('/works/architect', '?round=11-3')).toEqual(architect);
  });

  it("reads its old address as the new one with every parameter, and a work's page and the works as they were", () => {
    const every = '?project=p&tab=usage&span=30d&round=R12-3&cell=H3';
    expect(parse('/architect', every)).toEqual(parse('/works/architect', every));
    expect(parse('/architect', every)).toEqual({
      page: 'architect',
      project: 'p',
      tab: 'usage',
      span: '30d',
      round: { work: 12, n: 3 },
      cell: 'H3',
    });
    expect(parse('/works/12', '')).toMatchObject({ page: 'work', id: 12 });
    expect(parse('/works', '')).toEqual({ page: 'works' });
    expect(parse('/works/architect/x', '')).toEqual({ page: 'missing' });
  });

  it("moves an old address to where its view is now, with every parameter, and no other", () => {
    const at = (path: string, search = '') => moved(path, parse(path, search));
    // The architect's old address keeps the project, the tab, the span, the round and the cell.
    const every = '?project=p&tab=usage&span=30d&round=R12-3&cell=H3';
    expect(at('/architect', every)).toBe('/works/architect?project=p&tab=usage&span=30d&round=R12-3&cell=H3');
    expect(at('/architect')).toBe('/works/architect');
    expect(at('/architect/')).toBe('/works/architect');
    // The Spend view's goes to the System view.
    expect(at('/spend')).toBe('/system');
    expect(at('/spend', '?period=4w')).toBe('/system?period=4w');
    // Any other address is left where it is: the root, the works with a trailing slash, the new address and the system.
    for (const path of ['/', '/works/', '/works', '/works/architect', '/works/12', '/system', '/messages']) {
      expect(at(path), path).toBeNull();
    }
  });

  it("has no tab of its own in the bar: the works, a work's page and the architect's page are the one view", () => {
    expect(VIEWS.map((v) => v.to)).not.toContain('/architect');
    for (const page of ['works', 'work', 'architect'] as const) {
      expect(viewOf(page), page).toBe('/works');
    }
    expect(viewOf('messages')).toBe('/messages');
    expect(viewOf('missing')).toBeNull();
  });

  it('is read again when what it shows changed', () => {
    expect(named({ kind: 'architect' })).toEqual(['architect', 'messages', 'system']);
  });

  it('says what came of a matter done in one shape: a summary in the order the owner reads it, then each thing', () => {
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
      nothing: false,
      ...extra,
    });
    const thing = (kind: string, change: string, id: number, extra: Partial<MadeThing> = {}): MadeThing => ({
      kind,
      change,
      id,
      title: `Thing ${id}`,
      place: null,
      next_to: null,
      ...extra,
    });
    const matter = (h: Handled | null, made: MadeThing[] = []): MatterProgress => ({
      matter: 4,
      kind: 'engineer_question',
      detail: 'split',
      what: 'Split #3?',
      state: 'done',
      began: 1,
      cell: null,
      cell_title: null,
      architect_work: null,
      round: null,
      work: null,
      item: null,
      handled: h,
      withdrawn: null,
      asked: null,
      fix: null,
      made,
    });
    // Every kind of result, in the order the owner reads it; the things it changed by name listed in the same order
    // under it, whatever order they came in.
    const all = matterResult(
      matter(
        handled({
          reply: 'Split it in two.',
          reply_to: 7,
          made: 3,
          dropped: 1,
          observations: [
            { kind: 'new', observation: null },
            { kind: 'merge', observation: 22 },
            { kind: 'close', observation: 9 },
          ],
          actions: [
            { kind: 'pause_work', item: 3 },
            { kind: 'start_work_over', item: 4 },
          ],
          messages: ['q', 'q'],
        }),
        [
          thing('observation', 'close', 9),
          thing('observation', 'new', 37),
          thing('item', 'reordered', 5, { place: 'after', next_to: 6 }),
          thing('observation', 'merge', 22),
          thing('item', 'new', 11),
        ],
      ),
    );
    expect(all.parts.map((p) => [p.key, p.words, p.to])).toEqual([
      ['reply', 'replied to W7', href({ page: 'messages', open: 'work:7' })],
      ['item new', '1 item made', null],
      ['item reordered', '1 item moved', null],
      ['item other', '1 other item change made', null],
      ['item dropped', '1 item change dropped', null],
      ['observation new', '1 observation recorded', null],
      ['observation merge', '1 merged into an observation already there', null],
      ['observation close', '1 observation closed', null],
      ['actions', 'did at once: paused the work on #3; started the work on #4 over', null],
      ['messages', 'wrote to the architect of q', null],
    ]);
    expect(all.things.map((x) => `${x.kind} ${x.change} ${x.id}`)).toEqual([
      'item new 11',
      'item reordered 5',
      'observation new 37',
      'observation merge 22',
      'observation close 9',
    ]);
    expect(all.one).toBeNull();
    // A merge and a record are the same shape: counted on the line, each a line of its own under it.
    const recorded = matterResult(
      matter(handled({ observations: [{ kind: 'new', observation: null }, { kind: 'new', observation: null }] }), [
        thing('observation', 'new', 37),
        thing('observation', 'new', 38),
      ]),
    );
    expect(recorded.parts.map((p) => p.words)).toEqual(['2 observations recorded']);
    expect(recorded.things.map((x) => x.id)).toEqual([37, 38]);
    // One thing changed and nothing else: the line is that thing, and no line under it says it again; so for a merge
    // as for a record or an item made.
    for (const x of [thing('observation', 'merge', 22), thing('observation', 'new', 37), thing('item', 'new', 11)]) {
      const one = matterResult(matter(handled({ made: x.kind === 'item' ? 1 : 0 }), [x]));
      expect(one.one, `${x.kind} ${x.change}`).toEqual(x);
    }
    // One thing beside another result is counted and listed.
    const beside = matterResult(matter(handled({ reply: 'Yes.', reply_to: 7 }), [thing('observation', 'merge', 22)]));
    expect(beside.one).toBeNull();
    expect(beside.parts.map((p) => p.words)).toEqual(['replied to W7', '1 merged into an observation already there']);
    // Item changes beside none it names, what waits for the review, and a count of one or more.
    expect(matterResult(matter(handled({ made: 2, collected: 1 }))).parts.map((p) => p.words)).toEqual([
      '2 item changes made',
      '1 item change waiting for the review',
    ]);
    expect(matterResult(matter(handled({ told: 1 }))).parts.map((p) => p.words)).toEqual(['told you once']);
    expect(matterResult(matter(handled({ told: 3, ask: 12 }))).parts.map((p) => p.words)).toEqual([
      'told you 3 times',
      'put ask 12 to you',
    ]);
    // Nothing came of it: that, and why; what came of it no longer known: that it is done.
    expect(matterResult(matter(handled({ why: 'Already done by #2.' }))).parts.map((p) => p.words)).toEqual([
      'nothing changed: Already done by #2.',
    ]);
    expect(matterResult(matter(handled())).parts.map((p) => p.words)).toEqual(['nothing changed']);
    expect(matterResult(matter(null)).parts.map((p) => p.words)).toEqual(['done']);
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
      line: null,
      stage: null,
      inputs: [],
      prompt_by: null,
      schema: null,
      rows: [],
    })),
  };
}

const view = (name: string): GraphView => ({ name, cells: [], common_format: '' });

describe('the flow page with both graphs', () => {
  const graphs = [
    { g: graph('engineer', ['S0', 'R1', 'X1', 'E1']), view: view('engineer') },
    { g: graph('architect', ['S0', 'S1', 'X1', 'R1']), view: view('architect') },
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
    expect(inGraph({ kind: 'round', work: 7, round: 3 }, 'architect')).toEqual({ kind: 'round', work: 7, round: 3 });
    // A round is known by its work and its number in it: two works' rounds of one number are two.
    expect(subjectKey({ kind: 'round', work: 7, round: 3 })).not.toBe(subjectKey({ kind: 'round', work: 8, round: 3 }));
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

describe('what releasing a hold on new works lets happen', () => {
  it('is said for each of the three holds the owner may release, and for any other as a plain release', () => {
    expect(releaseWords('owner', 'p')).toBe('New works of p start again, unless the architect holds them too.');
    expect(releaseWords('architect', 'p')).toContain("This is the architect's own hold.");
    expect(releaseWords('set_straight', 'p')).toContain('may hold them again in its next plan');
    expect(releaseWords('policy', 'p')).toBe(
      'Releasing this hold lets new works of p start again, unless another hold remains.',
    );
  });
});
