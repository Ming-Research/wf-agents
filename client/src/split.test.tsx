// A round picked on a work's page and on the architect's shows the same way, from the one part both draw their column
// with: under the page's panel, whose list of rounds stays in view, split from it by a divider that drags.

import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

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
// The chart measures its text in a browser; what is checked here is the column beside it.
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

/// What the column docked beside the chart holds, read from the page drawn: the page's panel on top and the share of
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
      setting: null,
      check: null,
      next: { plan: null, matters: [] },
      observations: [],
      tags: [],
      own: true,
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
