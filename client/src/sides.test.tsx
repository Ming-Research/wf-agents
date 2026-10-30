// A cell's panel and a round's are laid out alike, In, Out, From and To, from one set of parts: a round's return reads
// as JSON, a cell's as the pattern of it, and the rows into a cell and out of it as two tables, on a round the rows it
// came in by and left by marked. A round's cells are those of the flow version its work ran on, whichever version the
// chart beside it shows.

import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

import type { Compiled, CompiledCell, CompiledGraph, CompiledRow, FlowView, GraphView, JsonValue, RoundDetail, RoundSummary } from './api';
import { flowOf, FlowContext } from './flow/context';
import { Json, ReturnPattern } from './flow/Returns';
import { FromTable, ToTable } from './flow/Rows';
import { RoundView } from './pages/RoundView';

// What the interface gives, by its path: set by each test, read synchronously as the page draws.
const served = new Map<string, unknown>();
vi.mock('./live', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./live')>()),
  useStatus: () => null,
  useResource: (_key: string | null, path: string | null) => ({ data: path !== null ? served.get(path) : undefined, error: null }),
}));

const row = (name: string, to: string, extra: Partial<CompiledRow> = {}): CompiledRow => ({
  name,
  if: [['all']],
  to,
  counts: null,
  waits: false,
  pauses: false,
  interject: false,
  main: false,
  common: false,
  note: null,
  ...extra,
});

const cell = (id: string, title: string, rows: CompiledRow[], extra: Partial<CompiledCell> = {}): CompiledCell => ({
  id,
  kind: 'engine',
  gate: false,
  title,
  step: null,
  needs: null,
  line: null,
  stage: null,
  inputs: [],
  prompt_by: null,
  schema: null,
  rows,
  ...extra,
});

/// An agent's return schema: its own answer, `take` and its way, or a search, in `outcome`, beside its progress.
const schema: JsonValue = {
  type: 'object',
  properties: {
    outcome: {
      anyOf: [
        {
          type: 'object',
          properties: { take: { type: 'object', description: 'How the matter goes on.', properties: { way: { enum: ['do_now', 'later'] } } } },
        },
        { type: 'object', properties: { search: { type: 'string' } } },
      ],
    },
    progress: { type: 'string', description: 'What this round did.' },
  },
};

function graphOf(cells: CompiledCell[]): CompiledGraph {
  return {
    name: 'architect',
    start: cells[0].id,
    end: cells[cells.length - 1].id,
    within: 'outcome',
    asking: [],
    receive: null,
    counters: [],
    rules: [],
    interject: null,
    cells,
  };
}

/// A flow of one graph, each cell's targets its rows' and each entered from every row that leads to it.
function flowOfCells(cells: CompiledCell[]): { compiled: Compiled; view: FlowView; g: CompiledGraph; v: GraphView } {
  const g = graphOf(cells);
  const v: GraphView = {
    name: g.name,
    lines: [],
    common_format: '',
    cells: cells.map((c) => ({
      id: c.id,
      targets: c.rows.map((r) => [r.to]),
      entered_from: cells.flatMap((x) => x.rows.flatMap((r, i) => (r.to === c.id ? [{ cell: x.id, row: i }] : []))),
      inputs: [],
      returns: { within: null, forms: [], every: [], used_by: [] },
    })),
  };
  const compiled: Compiled = {
    format: '',
    engine: { steps: {}, actions: { close_matter: 'Closes the matter.' }, checks: {}, inputs: {}, values: { route: 'Where the matter goes.' }, needs: {} },
    tools: [],
    graphs: [g],
    settings: [],
  };
  return { compiled, view: { graphs: [v] }, g, v };
}

// The flow version the work ran on: H8 hands back `take.way`, and `done` leads to B1 as it was titled then.
const before = flowOfCells([
  cell('Q2', 'Route the matter', [row('to_owner_words', 'H8', { if: [['inp', 'route', ['H8']]] })]),
  cell(
    'H8',
    'What you ask',
    [
      row('done', 'B1', { if: [['is', 'take.way', ['do_now']]], main: true, do: ['close_matter'] }),
      row('later', 'B1', { if: [['is', 'take.way', ['later']]] }),
      row('search', 'X2', { if: [['has', 'search']], common: true }),
    ],
    { kind: 'agent', schema },
  ),
  cell('X2', 'Search observations', [row('found', 'H8')], { kind: 'tool' }),
  cell('B1', 'Where the batch stood', []),
]);

// The flow version the chart shows now: H8's return and rows changed, and so did B1's title.
const now = flowOfCells([
  cell('Q2', 'Route the matter', [row('to_owner_words', 'H8', { if: [['inp', 'route', ['H8']]] })]),
  cell('H8', 'What you ask', [row('done', 'N1', { if: [['is', 'talk.verdict', ['done']]] })], { kind: 'agent' }),
  cell('N1', 'What to do next', []),
  cell('B1', 'Where the batch stands', []),
]);

const summary = (n: number, at: string, transition: string | null): RoundSummary => ({
  n,
  cell: at,
  line: null,
  outcome: 'took',
  transition,
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

const rounds = [summary(1, 'Q2', 'to_owner_words'), summary(2, 'H8', 'done'), summary(3, 'B1', null)];

const detail: RoundDetail = {
  round: rounds[1],
  work: 7,
  flow_version: 3,
  graph: 'architect',
  session: null,
  config_commit: '',
  message: 'the message',
  conditional: [],
  call: null,
  level: null,
  effort: null,
  acceptance_before: null,
  limits: {},
  inputs: [],
  input: null,
  output: { outcome: { take: { way: 'do_now' } }, progress: 'Closed it.' },
  rejected: null,
  usage: null,
  tool_runs: [],
  design_cards: [],
  asks: [],
  interjections: [],
};

/// The text of each line of a table of the page drawn, its cells between bars, what a row does after " · ", with
/// whether it is marked as the row taken or greyed.
function lines(html: string): string[] {
  return [...html.matchAll(/<tr( class="([^"]*)")?>(.*?)<\/tr>/g)].map((m) => {
    const text = m[3]
      .replace(/<span class="side-does">/g, ' · ')
      .replace(/<\/?t[dh][^>]*>/g, '|')
      .replace(/<[^>]+>/g, '')
      .replace(/&#x27;/g, "'")
      .replace(/\|+/g, '|');
    return `${m[2] === 'side-taken' ? '* ' : m[2] === 'side-other' ? '- ' : ''}${text}`;
  });
}

describe("a round's panel", () => {
  it('names its cells and rows as the flow version its work ran on has them, not as the chart beside it shows them', () => {
    served.clear();
    served.set('/api/v1/works/7/rounds/2', detail);
    served.set('/api/v1/flows/3', before.compiled);
    served.set('/api/v1/flows/3/view', before.view);
    const chart = flowOf(now.compiled, now.g, now.v, () => {});
    const html = renderToStaticMarkup(
      <FlowContext.Provider value={chart}>
        <RoundView work={7} n={2} rounds={rounds} />
      </FlowContext.Provider>,
    );
    // The four sections, in order.
    expect([...html.matchAll(/<h2[^>]*section-title[^>]*>([^<]*)</g)].map((m) => m[1])).toEqual(['In', 'Out', 'From', 'To']);
    const table = lines(html);
    // From: the row it came in by, marked, with the round it came from.
    expect(table).toContain("* |Q2 Route the matter|to_owner_words|the engine's value route is H8|R7-1|");
    expect(table).toContain('- |X2 Search observations|found|always|');
    // To: done, worked out from the return's `outcome.take.way`, to B1 as titled then, with the round it led to.
    expect(table).toContain('* |done main path|outcome.take.way is do_now|B1 Where the batch stood · close matter|R7-3|');
    expect(table).toContain('- |search|the return has outcome.search|X2 Search observations|');
    expect(html).not.toContain('talk.verdict');
    expect(html).not.toContain('What to do next');
    expect(html).not.toContain('Where the batch stands');
  });

  it('says where it came from and went over a table none of whose rows is the one', () => {
    served.clear();
    served.set('/api/v1/works/7/rounds/1', { ...detail, round: rounds[0], message: null, output: null });
    served.set('/api/v1/flows/3', before.compiled);
    served.set('/api/v1/flows/3/view', before.view);
    const html = renderToStaticMarkup(<RoundView work={7} n={1} rounds={rounds} />);
    expect(html).toContain('Started the work');
    expect(lines(html)).toContain("* |to_owner_words|the engine's value route is H8|H8 What you ask|R7-2|");
  });
});

describe("a cell's panel", () => {
  const f = flowOf(before.compiled, before.g, before.v, () => {});
  const h8 = before.g.cells[1];

  it('lists every row into the cell and every row of its own, each with its condition and where it leads, none marked', () => {
    const html = renderToStaticMarkup(
      <FlowContext.Provider value={f}>
        <FromTable c={h8} />
        <ToTable c={h8} />
      </FlowContext.Provider>,
    );
    expect(lines(html)).toEqual([
      '|From|Row|When|',
      "|Q2 Route the matter|to_owner_words|the engine's value route is H8|",
      '|X2 Search observations|found|always|',
      '|Row|When|Leads to|',
      '|done main path|outcome.take.way is do_now|B1 Where the batch stood · close matter|',
      '|later|outcome.take.way is later|B1 Where the batch stood|',
      '|search|the return has outcome.search|X2 Search observations|',
    ]);
  });

  it("shows a round's return as JSON, a key's tip what the flow says of its place", () => {
    const html = renderToStaticMarkup(<Json value={detail.output ?? null} schema={schema} within="outcome" titles={{ take: 'Which way' }} />);
    const text = html.replace(/<[^>]+>/g, '').replace(/&quot;/g, '"');
    expect(text).toBe('{"outcome": {"take": {"way": "do_now"}},"progress": "Closed it."}');
    expect(html).toContain('title="Which way\nHow the matter goes on."');
    expect(html).toContain('title="What this round did."');
    expect(html).toContain('<span class="json-string">&quot;do_now&quot;</span>');
  });

  it('shows what an agent cell hands back as the pattern of its return', () => {
    const field = (name: string, kind: 'text' | 'enum' | 'object', extra = {}) => ({
      name,
      path: name,
      kind,
      item: null,
      desc: null,
      optional: false,
      min: null,
      values: [],
      used_by: [],
      fields: [],
      disc: null,
      disc_used_by: [],
      common: [],
      branches: [],
      ...extra,
    });
    const way = { ...field('way', 'enum', { values: [['do_now', 'At once'], ['later', null]] }), path: 'take.way' };
    const v: GraphView = {
      ...before.v,
      cells: before.v.cells.map((c) =>
        c.id === 'H8'
          ? {
              ...c,
              returns: {
                within: 'outcome',
                forms: [
                  { field: null, fields: [field('take', 'object', { fields: [way] })], used_by: [] },
                  { field: 'search', fields: [field('search', 'text')], used_by: [] },
                ],
                every: [field('progress', 'text', { optional: true })],
                used_by: [],
              },
            }
          : c,
      ),
    };
    const html = renderToStaticMarkup(
      <FlowContext.Provider value={flowOf(before.compiled, before.g, v, () => {})}>
        <ReturnPattern c={h8} />
      </FlowContext.Provider>,
    );
    const text = html.replace(/<[^>]+>/g, '').replace(/&quot;/g, '"');
    expect(text).toBe('{"outcome": one of{"take": {"way": "do_now" | "later"}}{"search": text},"progress": text optional}');
    expect(html).toContain('title="At once"');
  });
});
