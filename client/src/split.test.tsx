// A round picked on a work's page and on the architect's shows the same way, from the one part both draw the window
// floating over their chart with: under the page's panel, whose list of rounds stays in view, split from it by a
// divider that drags. The window minimises to its one head row and expands again, a click on the chart that opens
// something expands it, and the chart centres in the part of it the window leaves.

import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { coverOf, foldIn, LEAST_SEEN } from './flow/Dock';
import { FlowScreen, pickExpands } from './flow/FlowScreen';
import { Panel } from './flow/Panel';
import { centredOn, inSight, seenOf } from './graph/Viewer';
import type {
  Architect as ArchitectRead,
  ArchitectWork,
  Compiled,
  CompiledGraph,
  FlowView,
  RoundSummary,
  WorkDetail,
  WorkSummary,
} from './api';
import { Chart as ArchitectChart } from './pages/Architect';
import { Body as WorkBody } from './pages/Work';

// Drawn once, outside a browser: nothing is read, and the engine's state is not known.
vi.mock('./live', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./live')>()),
  useStatus: () => null,
  useActivity: () => [],
  useResource: () => ({ data: undefined, error: null }),
}));
// The chart measures its text in a browser; what is checked here is the window over it.
vi.mock('./graph/FlowChart', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./graph/FlowChart')>()),
  FlowChart: () => null,
}));

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
      title: `cell ${id}`,
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

const flow = (g: CompiledGraph): { compiled: Compiled; view: FlowView } => ({
  compiled: {
    format: '',
    engine: { steps: {}, actions: {}, checks: {}, inputs: {}, values: {}, needs: {} },
    tools: [],
    graphs: [g],
    settings: [],
  },
  view: { graphs: [{ name: g.name, lines: [], cells: [], common_format: '' }] },
});

const round = (n: number, cell: string): RoundSummary => ({
  n,
  cell,
  line: null,
  outcome: 'took',
  transition: 'next',
  why: null,
  started_at: n,
  ended_at: n + 1,
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

const work = (id: number, g: CompiledGraph): WorkSummary => ({
  id,
  item: null,
  title: null,
  project: 'p',
  graph: g.name,
  flow_version: 1,
  moved_from: null,
  at: g.cells[1].id,
  at_title: null,
  stage: null,
  state: 'done',
  paused: null,
  paused_by: null,
  ended: null,
  waits: null,
  branch: null,
  pr: null,
  started_at: 1,
  last_at: 9,
  rounds: 8,
  sent_back: 0,
  failed: 0,
  running: null,
  cost_usd: 0,
  open_asks: 0,
  open_design_cards: 0,
});

const detail = (w: WorkSummary): WorkDetail => ({
  work: w,
  machine: null,
  worktree: null,
  pushed_head: null,
  resume_at: null,
  waiting: null,
  recorded: null,
  rounds: [1, 2, 3, 4, 5, 6, 7, 8].map((n) => round(n, n % 2 === 0 ? 'B1' : 'A1')),
  design_cards: [],
  asks: [],
  messages: [],
  interjections: [],
  sessions: [],
  pings: [],
});

/// What the window floating over the chart holds, read from the page drawn: the page's panel on top and the share of
/// it set, the list of rounds in it and the round picked there, the divider, and what is open under them.
function column(html: string) {
  const top = /<div class="(dock-top[^"]*)"(?: style="([^"]*)")?>/.exec(html);
  return {
    top: top?.[1] ?? null,
    share: top?.[2] ?? null,
    rounds: [...html.matchAll(/<tr class="clickable( selected)?">/g)].length,
    picked: /<tr class="clickable selected"><td class="muted nowrap">([^<]*)</.exec(html)?.[1] ?? null,
    divider: html.includes('role="separator" aria-orientation="horizontal"'),
    open: /<h2[^>]*class="[^"]*panel-title[^"]*"[^>]*>(.*?)<\/h2>/.exec(html)?.[1].replace(/<[^>]+>/g, '') ?? null,
    // The page's panel comes before what is open: the list above, the round below.
    panelFirst: html.indexOf('drawer-tabs') >= 0 && html.indexOf('drawer-tabs') < html.indexOf('class="panel"'),
  };
}

describe('a round picked beside a chart', () => {
  it("shows on a work's page under the work's panel, its list of rounds still there, split by a divider", () => {
    const g = graph('engineer', ['A1', 'B1']);
    const { compiled, view } = flow(g);
    const d = detail(work(7, g));
    const html = renderToStaticMarkup(
      <WorkBody r={{ page: 'work', id: 7, round: 6, cell: null, tab: 'rounds', only: null }} d={d} compiled={compiled} view={view} />,
    );
    expect(column(html)).toEqual({
      top: 'dock-top dock-top-split',
      share: 'height:50%',
      rounds: 8,
      picked: 'R7-6',
      divider: true,
      open: 'R7-6 (B1 cell B1)',
      panelFirst: true,
    });
  });

  it("shows on the architect's page the same way, under the architect's panel with its list of rounds", () => {
    const g = graph('architect', ['A1', 'B1']);
    const { compiled, view } = flow(g);
    const d = detail(work(7, g));
    const shown: ArchitectWork = { work: d.work, started_for: [], handled: 0, handling: null };
    const a: ArchitectRead = {
      project: 'p',
      projects: ['p'],
      works: [shown],
      queue: [],
      holds: [],
      asks: [],
      handled: [],
      changes: [],
      policy: null,
      draft: null,
      check: null,
      next: { plan: null, matters: [] },
      notebook: [],
      observations: [],
      tags: [],
      own: true,
      constitution: null,
    };
    const html = renderToStaticMarkup(
      <ArchitectChart
        route={{ page: 'architect', project: null, work: 7, tab: 'rounds', round: 6, cell: null, only: null, span: '7d' }}
        a={a}
        shown={shown}
        d={d}
        compiled={compiled}
        view={view}
        go={() => {}}
      />,
    );
    expect(column(html)).toEqual({
      top: 'dock-top dock-top-split',
      share: 'height:50%',
      rounds: 8,
      picked: 'R7-6',
      divider: true,
      open: 'R7-6 (B1 cell B1)',
      panelFirst: true,
    });
  });

  it('gives the panel the whole column while nothing is open', () => {
    const g = graph('engineer', ['A1', 'B1']);
    const { compiled, view } = flow(g);
    const html = renderToStaticMarkup(
      <WorkBody
        r={{ page: 'work', id: 7, round: null, cell: null, tab: 'rounds', only: null }}
        d={detail(work(7, g))}
        compiled={compiled}
        view={view}
      />,
    );
    expect(column(html)).toMatchObject({ top: 'dock-top', share: null, rounds: 8, picked: null, divider: false, open: null });
  });
});

/// The window as drawn: its classes, and the buttons at the end of each head, by what they say: the tabs row's, where
/// the button that minimises and expands the window is while the tabs are its top row, and the head of what is open.
function windowOf(html: string) {
  const labels = (part: string) => [...part.matchAll(/data-component="Tooltip"[^>]*>([^<]*)</g)].map((m) => m[1]);
  return {
    dock: /<aside class="([^"]*)"/.exec(html)?.[1] ?? null,
    tabsRow: labels(/<div class="drawer-fold">(.*?)<\/div>/.exec(html)?.[1] ?? ''),
    openRow: labels(/<div class="panel-head">(.*?)<\/div><div class="panel-body"/.exec(html)?.[1] ?? ''),
  };
}

/// The counts on the tabs of the page's panel, in their order.
const counts = (html: string) => [...html.matchAll(/class="[^"]*drawer-count[^"]*"[^>]*>(\d+)</g)].map((m) => Number(m[1]));

/// This browser keeps the window minimised, as it does once the owner minimised it on any page.
function keptMinimised() {
  const kept = new Map([['wf-agents.dock-minimised', '1']]);
  vi.stubGlobal('localStorage', {
    getItem: (key: string) => kept.get(key) ?? null,
    setItem: (key: string, value: string) => kept.set(key, value),
    removeItem: (key: string) => kept.delete(key),
  });
}

describe('the window floating over the chart', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });
  const g = graph('engineer', ['A1', 'B1']);
  const { compiled, view } = flow(g);
  const d = detail(work(7, g));
  const workPage = (round: number | null) =>
    renderToStaticMarkup(
      <WorkBody r={{ page: 'work', id: 7, round, cell: null, tab: 'rounds', only: null }} d={d} compiled={compiled} view={view} />,
    );

  it("expanded on a work's page, has the tabs as its top row, ending in the button that minimises it", () => {
    expect(windowOf(workPage(6))).toEqual({
      dock: 'dock dock-half',
      tabsRow: ['Minimise the panel to its head'],
      openRow: ['Close'],
    });
  });

  it("minimised with a round open, is that round's head alone: its title, its close and the button that expands it", () => {
    keptMinimised();
    const html = workPage(6);
    expect(windowOf(html)).toEqual({ dock: 'dock dock-half dock-min', tabsRow: [], openRow: ['Close', 'Expand the panel'] });
    // The page's panel stays drawn out of sight, its round still picked, so that the window expands to where it was.
    expect(column(html)).toMatchObject({ top: 'dock-top dock-top-split', picked: 'R7-6', open: 'R7-6 (B1 cell B1)' });
  });

  it('minimised with nothing open, is the tabs row, every tab with its count, and the button that expands it', () => {
    keptMinimised();
    const html = workPage(null);
    expect(windowOf(html)).toEqual({ dock: 'dock dock-half dock-min', tabsRow: ['Expand the panel'], openRow: [] });
    expect(counts(html)).toEqual([8, 0, 0]);
  });

  it('on the flow page, holds what is open alone, its head the top row: its close, then the button', () => {
    const html = renderToStaticMarkup(
      <FlowScreen
        compiled={compiled}
        graphs={[{ g, view: view.graphs[0] }]}
        legend="flow"
        marks={[]}
        cell={null}
        onCell={() => {}}
        start={null}
        work={7}
        rounds={d.rounds}
        round={6}
        renderRound={() => null}
      />,
    );
    expect(windowOf(html)).toEqual({ dock: 'dock', tabsRow: [], openRow: ['Close', 'Minimise the panel to its head'] });
  });

  it('minimised, leaves out of the head of what is open the way back and who does it', () => {
    const head = (minimised: boolean) =>
      renderToStaticMarkup(
        <Panel title="B1 cell B1" who="The engine" back="A1 cell A1" onBack={() => {}} onClose={() => {}} shown="c" minimised={minimised}>
          body
        </Panel>,
      );
    const parts = (html: string) => ({
      back: html.includes('panel-back'),
      who: html.includes('panel-who'),
      title: html.includes('B1 cell B1'),
    });
    expect(parts(head(false))).toEqual({ back: true, who: true, title: true });
    expect(parts(head(true))).toEqual({ back: false, who: false, title: true });
  });
});

describe('what expands the window, and where the chart centres beside it', () => {
  it('a click on the chart that opens what it is on expands it; a click on its background does not', () => {
    expect(pickExpands({ kind: 'cell', graph: 'engineer', cell: 'A1' })).toBe(true);
    expect(pickExpands({ kind: 'store', store: 'works' })).toBe(true);
    expect(pickExpands({ kind: 'out', graph: 'engineer', cell: 'A1', to: 'B1' })).toBe(true);
    expect(pickExpands({ kind: 'nothing' })).toBe(false);
  });

  it('folds to the tabs, unless something is open while it is minimised; with no tabs, to what is open', () => {
    expect(foldIn(true, false, false)).toBe('tabs');
    expect(foldIn(true, true, false)).toBe('tabs');
    expect(foldIn(true, false, true)).toBe('tabs');
    expect(foldIn(true, true, true)).toBe('open');
    expect(foldIn(false, true, false)).toBe('open');
    expect(foldIn(false, true, true)).toBe('open');
  });

  it('centres a cell in the part of the chart the expanded window leaves, never under the window', () => {
    const cover = coverOf({ left: 0, right: 1000 }, { left: 600 });
    expect(cover).toBe(400);
    const seen = seenOf({ width: 1000, height: 600 }, cover);
    const cell = { x: 900, y: 50, width: 200, height: 60 };
    const v = centredOn(cell, 1, seen);
    expect(v.x + (cell.x + cell.width / 2) * v.s).toBe(300);
    expect(v.y + (cell.y + cell.height / 2) * v.s).toBe(300);
    expect(inSight(cell, v, seen)).toBe(true);
    // A cell under the window is out of sight, so that the chart brings it out; one beside the window stays.
    expect(inSight({ x: 650, y: 100, width: 100, height: 40 }, { s: 1, x: 0, y: 0 }, seen)).toBe(false);
    expect(inSight({ x: 100, y: 100, width: 100, height: 40 }, { s: 1, x: 0, y: 0 }, seen)).toBe(true);
  });

  it('counts nothing as covered while the window is minimised or closed, or where it leaves no room, as on a phone', () => {
    expect(coverOf({ left: 0, right: 1000 }, null)).toBe(0);
    expect(coverOf({ left: 0, right: 375 }, { left: 8 })).toBe(0);
    expect(coverOf({ left: 0, right: 1000 }, { left: LEAST_SEEN })).toBe(1000 - LEAST_SEEN);
    expect(seenOf({ width: 1000, height: 600 }, 0)).toEqual({ x0: 0, y0: 0, x1: 1000, y1: 600 });
  });
});
