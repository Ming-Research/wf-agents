// The works list as the owner reads it: how far each work has come as a row of narrow stage cells read from its flow
// version, the version's number alone in a column of its own, where its time went as the usage tab's track, and a
// group for each project whose head is outside its table and whose name folds it.

import type { ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { ArchitectStanding, Compiled, RoundSummary, Status, Waits, WorkSummary } from './api';
import { barSpans, blockSpans } from './charts';
import { stagesOf } from './graph/flowModel';
import { Row, timeWords, Works } from './pages/Works';
import { stageLooks } from './pages/StageBar';
import { workHref } from './route';
import { gapsOf, usageOf } from './usage';

// What the page has read, which each test sets: the engine's status, the works, the architects and the compiled flow.
const read = vi.hoisted(() => ({
  status: null as Status | null,
  works: [] as WorkSummary[],
  architects: undefined as ArchitectStanding[] | undefined,
  compiled: undefined as Compiled | undefined,
  navigated: [] as string[],
}));

vi.mock('./live', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./live')>()),
  useStatus: () => read.status,
  useResource: (_key: string | null, path: string | null) => ({
    data: path === '/api/v1/works' ? read.works : path?.startsWith('/api/v1/flows/') ? read.compiled : undefined,
    error: null,
  }),
  useShared: (_key: string, path: string) => (path === '/api/v1/architects' ? read.architects : undefined),
}));

vi.mock('./route', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./route')>()),
  navigate: (to: string) => {
    read.navigated.push(to);
  },
}));

/// A cell of a compiled graph, with what the bar reads of it: its code and its stage.
const cell = (id: string, stage: string | null) => ({ id, stage });

/// An engineer graph shaped as version 34 of the flow: the cells of another line (G1 to G3, the librarian's) stand among
/// the research and the design cells, and the first cell of each of those two stages is one of them.
const compiled = {
  graphs: [
    {
      name: 'engineer',
      cells: [
        cell('S0', 'start'),
        cell('S1', 'start'),
        cell('G1', 'research'),
        cell('R0', 'research'),
        cell('G2', 'research'),
        cell('R1', 'research'),
        cell('R2', 'research'),
        cell('G3', 'design'),
        cell('D1', 'design'),
        cell('D2', 'design'),
        cell('D3', 'design'),
        cell('I1', 'implement'),
        cell('T1', 'test'),
        cell('T2', 'test'),
        cell('M1', 'merge'),
        cell('M2', 'merge'),
        cell('L2', 'learn'),
        cell('A2', null),
      ],
    },
  ],
} as unknown as Compiled;

const STAGES = ['start', 'research', 'design', 'implement', 'test', 'merge', 'learn'];

const status = (extra: Partial<Status> = {}): Status =>
  ({
    paused: false,
    running: [],
    started: [],
    releasing: [],
    waiting_for_slot: [],
    stopping: [],
    engine_stopping: false,
    stepping: [],
    architects_started: [],
    flow_version: 3,
    halt: null,
    repos: {},
    ...extra,
  }) as unknown as Status;

const work = (id: number, extra: Partial<WorkSummary> = {}): WorkSummary => ({
  id,
  item: id,
  title: `Item ${id}`,
  project: 'p',
  graph: 'engineer',
  flow_version: 3,
  moved_from: null,
  at: 'D1',
  at_title: 'Design',
  stage: 'design',
  state: 'ready',
  paused: null,
  paused_by: null,
  ended: null,
  waits: null,
  branch: null,
  pr: null,
  started_at: 0,
  last_at: 0,
  ran_ms: 0,
  time: { from: 0, until: 0, model_ms: 0, tool_ms: 0, engine_ms: 0, bar: '' },
  rounds: 3,
  sent_back: 0,
  failed: 0,
  running: null,
  cost_usd: 1,
  open_asks: 0,
  open_design_cards: 0,
  ...extra,
});

/// The folded groups this browser keeps, and nothing else of what the page keeps.
const kept = (folded: string[] = []) =>
  vi.stubGlobal('window', {
    localStorage: {
      getItem: (k: string) =>
        k === 'wf-agents.works-folded' ? JSON.stringify(folded) : k === 'wf-agents.talks-hidden-states' ? '[]' : null,
      setItem: () => undefined,
    },
  });

beforeEach(() => {
  read.status = status();
  read.works = [];
  read.architects = undefined;
  read.compiled = compiled;
  read.navigated = [];
  kept();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

const view = () => renderToStaticMarkup(<Works work={null} />);

/// A work's row in the markup.
const rowOf = (html: string, id: number) => html.split('<tr ').find((r) => r.includes(`>W${id}<`)) ?? '';

/// The stage cells of a row: each its letter, its look and what it says on hovering.
const cellsOf = (row: string) =>
  [...row.matchAll(/class="stage-cell stage-(\w+)" title="([^"]*)"[^>]*>(\w)</g)].map((m) => ({ letter: m[3], look: m[1], title: m[2] }));

describe('the bar of how far a work has come', () => {
  it('has the stage gone through, the one it is at, the one it waits at, and those ahead, each its own look', () => {
    expect(stageLooks(STAGES, 'design', 'ready', false)).toEqual(['done', 'done', 'now', 'ahead', 'ahead', 'ahead', 'ahead']);
    // While it waits, the stage it is at takes the colour of waiting and none is added.
    expect(stageLooks(STAGES, 'design', 'waiting', true)).toEqual(['done', 'done', 'wait', 'ahead', 'ahead', 'ahead', 'ahead']);
    // Going back to an earlier stage, the later cells are plain again.
    expect(stageLooks(STAGES, 'research', 'ready', false)).toEqual(['done', 'now', 'ahead', 'ahead', 'ahead', 'ahead', 'ahead']);
    // Done: every stage gone through; ended before its end: the stage it stopped at is no stage it is at now.
    expect(stageLooks(STAGES, 'learn', 'done', false)).toEqual(STAGES.map(() => 'done'));
    expect(stageLooks(STAGES, 'test', 'ended', false)).toEqual(['done', 'done', 'done', 'done', 'ahead', 'ahead', 'ahead']);
    // A stage the flow does not name marks none gone through.
    expect(stageLooks(STAGES, null, 'ready', false)).toEqual(STAGES.map(() => 'ahead'));
    expect(stageLooks(STAGES, 'elsewhere', 'ready', false)).toEqual(STAGES.map(() => 'ahead'));
  });

  it("takes each stage's letter from the flow version's cells by the minimap's rule, as the research and design keep theirs", () => {
    const letters = stagesOf(compiled.graphs[0]);
    // A cell of another line first in a stage does not take its letter: research is R though G1 comes first, design is D.
    expect(letters.map((s) => `${s.stage}:${s.letter}`)).toEqual([
      'start:S',
      'research:R',
      'design:D',
      'implement:I',
      'test:T',
      'merge:M',
      'learn:L',
    ]);
  });

  it('draws one cell for each stage in a work\'s row, the stage it is at standing out, and says its stage on hovering', () => {
    read.works = [work(9, { stage: 'design' })];
    const cells = cellsOf(rowOf(view(), 9));
    expect(cells.map((c) => c.letter).join('')).toBe('SRDITML');
    expect(cells.map((c) => c.look)).toEqual(['done', 'done', 'now', 'ahead', 'ahead', 'ahead', 'ahead']);
    expect(cells.map((c) => c.title)).toEqual(['start', 'research', 'design, where it is now', 'implement', 'test', 'merge', 'learn']);
    // The same work back at research: the bar goes back with it.
    read.works = [work(9, { stage: 'research' })];
    expect(cellsOf(rowOf(view(), 9)).map((c) => c.look)).toEqual(['done', 'now', 'ahead', 'ahead', 'ahead', 'ahead', 'ahead']);
  });

  it('shows the colour of waiting on the stage a work waits at, whether for an answer, the architect or items', () => {
    const kinds: Waits[] = [
      { kind: 'answer', ask: 3 },
      { kind: 'architect', matter: 4 },
      { kind: 'items', items: [7] },
    ];
    for (const waits of kinds) {
      read.works = [work(6, { stage: 'research', state: 'waiting', waits })];
      const cells = cellsOf(rowOf(view(), 6));
      expect(cells.map((c) => c.look), waits.kind).toEqual(['done', 'wait', 'ahead', 'ahead', 'ahead', 'ahead', 'ahead']);
      expect(cells[1].title).toBe('research, where it waits');
    }
  });

  it('is drawn once the flow version is read, and is not drawn for the architect, which has its cell in words', () => {
    read.compiled = undefined;
    read.works = [work(9)];
    read.architects = [{ project: 'p', works: [], queue: [], asking: false }];
    const html = view();
    expect(html).not.toContain('stage-bar');
    read.compiled = compiled;
    const withBar = view();
    const architect = withBar.split('<tr ').find((r) => r.includes('works-architect')) ?? '';
    expect(withBar).toContain('stage-bar');
    expect(architect).not.toContain('stage-bar');
  });
});

describe('the works list', () => {
  it('has no column of the conversation, the last activity or the pull request, and has the progress, flow and time', () => {
    read.works = [work(9)];
    const head = view().split('<thead>')[1].split('</thead>')[0];
    const names = [...head.matchAll(/<th[^>]*>([^<]*)</g)].map((m) => m[1]);
    expect(names).toEqual(['Work', 'Item', 'State', 'Progress', 'Flow', 'Rounds', 'Run time', 'Time', 'Cost']);
    for (const gone of ['Conversation', 'Last activity', 'PR', '>At<', '>Stage<']) expect(head).not.toContain(gone);
  });

  it("leads from a work's number to its conversation, with the mark of one, from its title to its item, and from the rest of the row to the work", () => {
    read.works = [work(9)];
    const html = view();
    const row = rowOf(html, 9);
    expect(row).toMatch(/<td class="control-cell"><a[^>]*href="\/messages\?c=work%3A9"[^>]*>W9<svg/);
    expect(row).toContain('href="/items/9"');
    const el = Row({ w: work(9), tab: 'rounds' }) as unknown as ReactElement<{ onClick: () => void }>;
    expect(el.type).toBe('tr');
    el.props.onClick();
    expect(read.navigated).toEqual([workHref(9, { tab: 'rounds' })]);
  });

  it("puts the flow version's number alone in a column of its own, and no word about the version anywhere in the list", () => {
    // A work on an older version than the engine runs, and one that moved from another version to the one it is on.
    read.status = status({ flow_version: 5 });
    read.works = [work(9, { flow_version: 3 }), work(8, { flow_version: 4, moved_from: 2 })];
    const html = view();
    const flow = (id: number) => {
      const cells = [...rowOf(html, id).matchAll(/<td([^>]*)>([\s\S]*?)<\/td>/g)];
      return cells[4];
    };
    expect(flow(9)[2]).toBe('3');
    expect(flow(8)[2]).toBe('4');
    // No hover on it either.
    expect(flow(9)[1]).not.toContain('title=');
    // The header says what the column is; the rows say nothing of the version in words, hovering included.
    const rows = html.slice(html.indexOf('<tbody'));
    for (const words of ['old flow', 'moved to', 'older', 'brought up', 'version']) expect(rows, words).not.toContain(words);
  });

  it('keeps the head of each project outside its table, folding the group by its name, kept in the browser', () => {
    read.works = [work(9), work(4, { project: 'q' })];
    const open = view();
    expect((open.match(/aria-expanded="true"/g) ?? []).length).toBe(2);
    expect((open.match(/<table/g) ?? []).length).toBe(2);
    kept(['p']);
    const folded = view();
    const [p, q] = folded.split('<section').slice(1);
    expect(p).toContain('aria-expanded="false"');
    expect(p).not.toContain('<table');
    expect(p).not.toContain('>W9<');
    // Its buttons stay in the head, and the other group is open.
    expect(p).toContain('tree-state');
    expect(q).toContain('aria-expanded="true"');
    expect(q).toContain('>W4<');
  });
});

describe("a work's time bar", () => {
  // The rounds of the Rust test `a_work_s_time_splits_into_models_tools_and_the_engine_and_lays_out_as_a_bar`
  // (crates/api/src/read.rs): a start step, a model, the tools, a stretch no round ran, a model; and the bar the engine
  // lays them out in, which this test carries over as it is.
  const BAR = `.${'m'.repeat(49)}${'t'.repeat(10)}${'.'.repeat(10)}${'m'.repeat(26)}`;
  const cells = new Map([
    ['S0', { kind: 'engine', title: 'Waits to start' }],
    ['R1', { kind: 'agent', title: 'Research' }],
    ['X1', { kind: 'tool', title: 'Run tools' }],
  ]);
  const round = (n: number, cell: string, start: number, end: number, line: string | null): RoundSummary => ({
    work: 1,
    n,
    cell,
    line,
    outcome: 'took',
    transition: 'done',
    why: null,
    set_aside: null,
    started_at: start,
    ended_at: end,
    model: null,
    cost_usd: null,
    compactions: 0,
    tool_runs: 0,
    progress: null,
    not_asked: null,
    tokens: null,
    session: null,
    new_session: null,
  });
  const rounds = [
    round(1, 'S0', 0, 10_000, null),
    round(2, 'R1', 10_000, 500_000, 'engineer'),
    round(3, 'X1', 500_000, 600_000, null),
    round(4, 'R1', 700_000, 960_000, 'engineer'),
  ];

  it('is drawn by the usage tab\'s track: the same blocks for the same rounds, to the width of a cell', () => {
    const u = usageOf([{ work: 1, rounds }], cells, 960_000);
    // The usage tab's times for these rounds are the engine's (the Rust test's, in thousands of milliseconds): models
    // 490 + 260, tools 100, the engine's step 10, and the 100 between the tools and the last model nobody ran.
    expect([u.time.model, u.time.tool, u.time.engine, u.time.idle]).toEqual([750_000, 100_000, 10_000, 100_000]);
    const tab = blockSpans(
      u,
      gapsOf(u.blocks).map((g) => ({ start: g.start, end: g.end, key: `${g.start}` })),
    );
    const row = barSpans(BAR);
    const shape = (spans: Array<{ kind: string; left: number; width: number }>) =>
      [...spans].sort((a, b) => a.left - b.left).map((s) => ({ kind: s.kind, left: Math.round(s.left * 100), width: Math.round(s.width * 100) }));
    expect(shape(row)).toEqual(shape(tab));
  });

  it('says on hovering how long models, tools and the engine worked and how long no round ran, as the usage tab does', () => {
    const hours = 3_600_000;
    const words = timeWords({ from: 0, until: 3 * hours, model_ms: 2 * hours, tool_ms: 600_000, engine_ms: 2000, bar: BAR });
    // The time nobody ran is what is left of the span: three hours less two, ten minutes and two seconds.
    expect(words).toBe(
      "Models | 2h 00m | Tools | 10m 00s | Engine's steps | 2s | Not running | 49m 58s: paused, stepped one round at a time, or waiting for you",
    );
    // The engine's steps under a second are left out, as the tab's bar leaves them out.
    expect(timeWords({ from: 0, until: 3 * hours, model_ms: 2 * hours, tool_ms: 600_000, engine_ms: 900, bar: BAR })).not.toContain("Engine's steps");
  });

  it("is in a work's row as the track, the architect's row has none", () => {
    read.works = [work(9, { time: { from: 0, until: 960_000, model_ms: 750_000, tool_ms: 100_000, engine_ms: 10_000, bar: BAR } })];
    const row = rowOf(view(), 9);
    expect(row).toContain('class="works-time"');
    expect((row.match(/class="us-block us-model"/g) ?? []).length).toBe(2);
    expect((row.match(/class="us-block us-tool"/g) ?? []).length).toBe(1);
    expect((row.match(/class="us-block us-gap"/g) ?? []).length).toBe(1);
  });
});
