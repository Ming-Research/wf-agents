import { describe, expect, it } from 'vitest';

import type { CompiledCell, CompiledGraph, CompiledRow, GraphView, RoundSummary, Status, WorkSummary } from './api';
import { controls, course } from './components';
import { held } from './course';
import { cellTab, cellTabs, numbered } from './flow/CellPanel';
import { conditionsWords, conditionWords, targetWords, valuesWords } from './flow/words';
import { cost, duration } from './format';
import { badgeRow, labelSize } from './graph/flowDraw';
import { addHops, lengthenLast, onLine, pathOf, type Route as Routed } from './graph/flowLayout';
import { ASKING, chart, MAIN, overlays, partsOf, related, runs, sharedRows, taken } from './graph/flowModel';
import { named } from './live';
import { parse as parseMarkdown } from './markdown';
import { firstSentence, pickApproach, verdictLine, verdictReady, type Verdict } from './pages/Decide';
import { addressesByNumber, atPath, inputRows, jsonLines, modelName, sourceWords } from './pages/RoundView';
import { roundsShown } from './pages/Work';
import { catalog, t } from './messages';
import { href, parse, type Route } from './route';

describe('the message catalog', () => {
  it('has every text, none empty, all in English', () => {
    for (const [key, text] of Object.entries(catalog)) {
      expect(text.trim(), key).not.toBe('');
      // No character of a CJK script, nor full-width punctuation.
      expect(/[\u{3000}-\u{303f}\u{3040}-\u{30ff}\u{3400}-\u{9fff}\u{ff00}-\u{ffef}]/u.test(text), key).toBe(false);
    }
    expect(t('slots', { used: 1, total: 2 })).toBe('Slots 1/2');
  });
});

describe('the addresses', () => {
  it('read back what they write', () => {
    const flow = { page: 'flow' as const, version: null, cell: null, project: null, ref: null, from: null };
    const work = { page: 'work' as const, id: 3, round: null, cell: null, tab: 'rounds' as const, only: null };
    const routes: Route[] = [
      { page: 'works' },
      { ...work, round: 240 },
      { ...work, cell: '2a' },
      { ...work, tab: 'cards' },
      { ...work, tab: 'messages', round: 7 },
      { ...work, only: 'failed' },
      { page: 'items' },
      { page: 'item', id: 5 },
      { page: 'messages' },
      { ...flow, version: 1, cell: '13' },
      flow,
      { ...flow, version: 2, from: 1 },
      { ...flow, project: 'p', ref: 'wf-agents/12-x', from: 1, cell: '4' },
    ];
    for (const r of routes) {
      const at = href(r);
      const i = at.indexOf('?');
      expect(i < 0 ? parse(at, '') : parse(at.slice(0, i), at.slice(i))).toEqual(r);
    }
    // A round is named by its work and its number in the work; a name of another work's round opens none.
    expect(href({ ...work, round: 240 })).toBe('/works/3?round=R3-240');
    expect(parse('/works/3', '?round=R4-240')).toEqual(work);
    expect(parse('/works/3', '?round=240')).toEqual(work);
    expect(parse('/', '')).toEqual({ page: 'works' });
    expect(parse('/works/x', '')).toEqual({ page: 'missing' });
    // A tab or a choice of rounds the page does not know is the work's rounds, all of them.
    expect(parse('/works/3', '?tab=x&only=y')).toEqual(work);
    expect(href(work)).toBe('/works/3');
  });
});

describe('how a work goes on', () => {
  const work = (extra: Partial<WorkSummary>): WorkSummary => ({
    id: 5,
    item: 1,
    title: 't',
    project: 'p',
    graph: 'engineer',
    flow_version: 2,
    at: '16',
    at_title: 'Owner',
    stage: null,
    state: 'ready',
    paused: null,
    paused_by: null,
    ended: null,
    waits: null,
    answered: null,
    branch: null,
    pr: null,
    started_at: null,
    last_at: null,
    rounds: 0,
    sent_back: 0,
    failed: 0,
    running: null,
    cost_usd: 0,
    open_asks: 0,
    open_design_cards: 0,
    ...extra,
  });
  const status = (extra: Partial<Status>): Status => ({
    paused: false,
    running: null,
    stepping: null,
    last_step: null,
    slots: 1,
    slots_used: 0,
    flow_version: 2,
    config_commit: 'c',
    repos: {},
    started_at: 0,
    version: 'v',
    halt: null,
    ...extra,
  });

  it('keeps what holds a work back apart from what it waits for, and offers what does something', () => {
    // Paused by the owner and waiting for an answer: both; Continue, and no Step while it waits.
    const both = course(
      work({ state: 'paused', paused: 'the owner paused it', paused_by: 'owner', waits: { kind: 'answer', ask: 6 } }),
      status({}),
    );
    expect([both.held, both.waits]).toEqual(['you', { kind: 'answer', ask: 6 }]);
    expect(controls(both, status({}))).toEqual({ pause: false, resume: true, step: false, canStep: true });
    // Every work paused: held back with them all, Step offered, no Continue of its own.
    const all = course(work({}), status({ paused: true }));
    expect(all.held).toBe('all');
    expect(controls(all, status({ paused: true }))).toEqual({ pause: false, resume: false, step: true, canStep: true });
    // Only the owner's own pause of a work reads as paused by the owner; held with every work, it reads as what it is.
    expect(held(all, work({}))).toBeNull();
    expect(held(both, work({ paused: 'the owner paused it', paused_by: 'owner' }))).toBe('paused by you');
    // Going on by itself, its round running: Pause only; once paused, it pauses after the round.
    const on = course(work({}), status({ running: 5 }));
    expect([on.running, controls(on, status({ running: 5 })).pause]).toEqual(['goes_on', true]);
    expect(course(work({ paused: 'x', paused_by: 'owner' }), status({ running: 5 })).running).toBe('pausing');
    expect(course(work({ paused: 'x', paused_by: 'owner' }), status({ running: 5, stepping: 5 })).running).toBe('step');
    // Stopped by the engine on an older flow version: Continue and Step, which wait for the round under way.
    const stopped = course(work({ paused: 'cell 4: no', paused_by: 'engine', flow_version: 1 }), status({}));
    expect([stopped.held, stopped.outdated]).toEqual(['engine', true]);
    expect(controls(stopped, status({ stepping: 3 }))).toEqual({ pause: false, resume: true, step: true, canStep: false });
    // An ended work goes on no more.
    expect(controls(course(work({ state: 'ended', ended: 'x' }), status({})), status({})).pause).toBe(false);
  });
});

describe('the formats', () => {
  it('write durations in their two largest units, and costs to the cent', () => {
    expect(duration(41_000)).toBe('41s');
    expect(duration(187_000)).toBe('3m 07s');
    expect(duration(7_500_000)).toBe('2h 05m');
    expect(cost(1.234)).toBe('$1.23');
    expect(cost(null)).toBe('');
  });
});

describe('the flow in words', () => {
  it('writes conditions and targets as words, names in code', () => {
    const known = new Set(['split']);
    expect(conditionWords(['own'], known)).toBe("the return is this cell's own answer");
    expect(conditionWords(['has', 'tools'], known)).toBe('the return has `tools`');
    expect(conditionWords(['is', 'ask.to', ['advisor']], known)).toBe('`ask.to` is `advisor`');
    expect(conditionWords(['inp', 'split', ['all_split']], known)).toBe("the engine's value `split` is `all_split`");
    expect(conditionWords(['inp', 'stage', ['research']], known)).toBe('the recorded value `stage` is `research`');
    expect(conditionsWords([['none', 'failed.kind', ['a', 'b', 'c']], ['all']], known)).toBe(
      'no `failed.kind` is `a`, `b` or `c`, and always',
    );
    expect(valuesWords(['x', 'y'])).toBe('`x` or `y`');
    expect(targetWords('@caller')).toBe('back to the cell that called');
    expect(targetWords('@self')).toBe('itself');
    expect(targetWords('4')).toBeNull();
  });
});

describe('events', () => {
  it('name what to read again', () => {
    expect(named({ kind: 'round', work: 2, round: 9 })).toEqual(['works', 'work:2', 'round:2-9', 'items', 'item:*']);
    expect(named({ kind: 'item', item: null })).toEqual(['items', 'item:*', 'works']);
    expect(named({ kind: 'ready' })).toEqual([]);
    expect(named({ kind: 'ask', ask: 4, work: 2 })).toEqual(['works', 'work:2', 'item:*']);
    expect(named({ kind: 'owner_message', message: 7, work: 2 })).toEqual(['messages', 'work:2']);
  });
});

function row(name: string, to: string, extra: Partial<CompiledRow> = {}): CompiledRow {
  return {
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
  };
}

function cell(id: string, rows: CompiledRow[], extra: Partial<CompiledCell> = {}): CompiledCell {
  return {
    id,
    kind: 'agent',
    gate: false,
    title: `cell ${id}`,
    step: null,
    needs: null,
    line: 'engineer',
    stage: 'build',
    inputs: [],
    prompt_by: null,
    schema: null,
    rows,
    ...extra,
  };
}

function round(n: number, cellId: string | null, outcome: RoundSummary['outcome'], transition: string | null): RoundSummary {
  return {
    n,
    cell: cellId,
    line: null,
    outcome,
    transition,
    why: null,
    started_at: n,
    ended_at: outcome === 'running' ? null : n + 1,
    model: null,
    cost_usd: null,
    compactions: 0,
    tool_runs: 0,
    progress: null,
    not_asked: null,
  };
}

// A start, two agent cells that share the rows to the tools and to asking, the tool cell, an asking part that
// answers back, and the end.
const shared = { common: true };
const graph: CompiledGraph = {
  name: 'engineer',
  start: 's',
  end: 'z',
  within: 'outcome',
  asking: ['q', 'r'],
  receive: null,
  counters: [],
  rules: [{ name: 'tree', desc: 'How the tree is written', text: ['Write it so.'] }],
  interject: { name: 'interjection', desc: 'What is said', text: [], from: [] },
  cells: [
    cell('s', [row('started', 'a', { main: true })], { kind: 'engine', line: null, stage: 'queued' }),
    cell(
      'a',
      [
        row('ask', 'q', shared),
        row('done', 'b', { main: true }),
        row('again', 'b'),
        row('tools', 't', { ...shared, main: true }),
        row('pause', '@self', shared),
      ],
      { rules: [{ name: 'tree', shared: false }, { name: 'general', shared: true }] },
    ),
    cell('b', [row('ask', 'q', shared), row('done', 'z', { main: true }), row('back', 'a'), row('tools', 't', shared)], {
      line: 'review',
    }),
    cell('t', [row('ran', '@caller'), row('failed', 'r')], { kind: 'tool', line: null, stage: null }),
    cell('q', [row('answered', '@asker')], { kind: 'engine', line: null, stage: null }),
    cell('r', [row('back', '@stopped'), row('redo', 'a')], { kind: 'engine', line: null, stage: null }),
    cell('z', [row('interjection', 'a', { interject: true })], { kind: 'engine', line: null, stage: 'done' }),
  ],
};

const view: GraphView = {
  lines: [],
  name: 'engineer',
  common_format: '',
  cells: graph.cells.map((c) => ({
    id: c.id,
    targets: c.rows.map((r) => {
      if (r.to === '@self') return [c.id];
      if (r.to === '@caller') return ['a', 'b'];
      if (r.to === '@asker') return ['a', 'b'];
      if (r.to === '@stopped') return ['a'];
      return [r.to];
    }),
    entered_from: graph.cells.flatMap((x) =>
      x.rows.flatMap((r, i) => {
        const to = r.to === '@self' ? [x.id] : r.to === '@caller' || r.to === '@asker' ? ['a', 'b'] : r.to === '@stopped' ? ['a'] : [r.to];
        return to.includes(c.id) ? [{ cell: x.id, row: i }] : [];
      }),
    ),
    inputs: [],
    returns: { within: null, forms: [], every: [], used_by: [] },
  })),
};

describe('the chart of a flow', () => {
  const c = chart(graph, view);
  const node = (id: string) => c.nodes.find((n) => n.id === id);
  const edge = (id: string) => c.edges.find((e) => e.id === id);

  it('puts the asking part apart, and a cell only the shared rows lead to apart too', () => {
    const parts = partsOf(graph, view);
    expect(parts.get('a')).toBe(MAIN);
    expect(parts.get('q')).toBe(ASKING);
    expect(parts.get('t')).toBe('t');
    expect(c.parts).toEqual([MAIN, 't', ASKING]);
    expect(c.groups.map((g) => g.id)).toEqual(['stage:queued', 'stage:build', ASKING, 'stage:done']);
  });

  it('draws the rows every agent cell shares as one circle beside it, those back to itself too, with no label', () => {
    expect(sharedRows(graph.cells[1])).toEqual([0, 3, 4]);
    expect(node('k:a')?.rows).toEqual([0, 3, 4]);
    expect(edge('c:a->k:a')).toMatchObject({ label: '', numbers: [] });
    expect(edge('c:a->c:t')).toBeUndefined();
    expect(runs([0, 3, 4, 5, 6, 8])).toEqual([[0, 0], [3, 6], [8, 8]]);
  });

  it('draws the rows to one cell as one arrow with their names and numbers, the main path marked', () => {
    expect(edge('c:a->c:b')).toMatchObject({ rows: [1, 2], label: 'done, again', numbers: [[1, 1], [2, 2]], main: true });
    // A cell of one row needs no number.
    expect(edge('c:s->c:a')).toMatchObject({ label: 'started', numbers: [] });
  });

  it("draws a row's number as a badge, and leaves the numbers out of a work's chart", () => {
    expect(badgeRow([[0, 0], [3, 6]])).toEqual({
      items: [
        { x: 8, row: 0 },
        { x: 26, row: 3 },
        { x: 39, row: null },
        { x: 52, row: 6 },
      ],
      width: 60,
    });
    // The arrow to the shared circle has no label at all, numbered or not.
    const shared = c.edges.find((e) => e.id === 'c:a->k:a');
    expect(shared && labelSize(shared, true)).toBeNull();
    expect(shared && labelSize(shared, false)).toBeNull();
  });

  it("keeps an arrow's label on its own line", () => {
    const route = (label: { x: number; y: number }) => ({
      points: [
        { x: 100, y: 100 },
        { x: 100, y: 300 },
        { x: 160, y: 300 },
      ],
      label: { ...label, width: 40, height: 20 },
      hops: [],
    });
    // Where elkjs put it on the line, it stays.
    expect(onLine(route({ x: 80, y: 190 })).label).toEqual({ x: 80, y: 190, width: 40, height: 20 });
    // Left at a stage box's corner, it goes to the middle of the longest piece.
    expect(onLine(route({ x: 0, y: 0 })).label).toEqual({ x: 80, y: 190, width: 40, height: 20 });
  });

  it('lights up only the arrow pointed at, with its other half through the circles into the same cell', () => {
    // The asking part answers back into each cell that asked: pointing at the way into a lights that way alone.
    expect(related(c.edges, 'i:q:a->c:a')).toEqual(new Set(['i:q:a->c:a', 'c:q->o:a:q']));
    expect(related(c.edges, 'c:q->o:b:q')).toEqual(new Set(['c:q->o:b:q', 'i:q:b->c:b']));
    expect(related(c.edges, 'c:t->o:r:t')).toEqual(new Set(['c:t->o:r:t']));
    expect(related(c.edges, 'c:a->c:b')).toEqual(new Set(['c:a->c:b']));
    expect(related(c.edges, 'r:tree:a->c:a')).toEqual(new Set(['r:tree:a->c:a']));
    expect(related(c.edges, null).size).toBe(0);
  });

  it('draws a row to another part as a circle beside the cell, and one beside the cell it comes back into', () => {
    expect(node('o:r:t')).toMatchObject({ kind: 'out', cell: 't', part: 't' });
    expect(node('o:a:q')).toMatchObject({ kind: 'out', cell: 'q', part: ASKING });
    expect(node('i:q:a')).toMatchObject({ kind: 'in', cell: 'a', part: MAIN });
    // Back to where the work stopped: from the cell whose row decided it.
    expect(node('i:t:a')).toMatchObject({ kind: 'in', cell: 'a' });
    expect(edge('i:t:a->c:a')).toMatchObject({ label: 'failed', numbers: [[1, 1]] });
  });

  it('puts the rules beside the cells given them and the interjection beside the steps that take one', () => {
    expect(node('r:tree:a')).toMatchObject({ kind: 'rule', group: 'stage:build' });
    // A rule every cell of a line is given is listed in the cell's inputs, not drawn beside it.
    expect(node('r:general:a')).toBeUndefined();
    expect(edge('r:tree:a->c:a')?.kind).toBe('rule');
    expect(node('j:z')?.kind).toBe('interjection');
    expect(node('j:s')).toBeUndefined();
  });

  it('marks what a work did at each cell, and the arrows it took', () => {
    const rounds = [
      round(1, 's', 'took', 'started'),
      round(2, 'a', 'took', 'tools'),
      round(3, 't', 'took', 'ran'),
      round(4, 'a', 'sent_back', null),
      round(5, 'a', 'took', 'done'),
      round(6, 'b', 'failed', null),
      round(7, 'b', 'running', null),
    ];
    const o = overlays(rounds, 'b', { running: true, held: null });
    expect(o.get('a')).toEqual({ rounds: 3, sentBack: 1, failed: 0, running: false, current: false, held: null });
    expect(o.get('b')).toEqual({ rounds: 2, sentBack: 0, failed: 1, running: true, current: true, held: null });
    // A round left open while the engine runs nothing of the work does not run; a work held back says why at its cell.
    const held = overlays(rounds, 'b', { running: false, held: 'paused by you' });
    expect(held.get('b')).toEqual({
      rounds: 2,
      sentBack: 0,
      failed: 1,
      running: false,
      current: true,
      held: 'paused by you',
    });
    const w = taken(c, graph, rounds);
    expect(w.get('c:s->c:a')).toBe(1);
    expect(w.get('c:a->k:a')).toBe(1);
    expect(w.get('c:a->c:b')).toBe(1);
    expect(w.get('c:b->c:z')).toBeUndefined();
  });
});

describe('the routes of arrows', () => {
  it('round their bends and arch where they cross another arrow', () => {
    const across: Routed = { points: [{ x: 0, y: 50 }, { x: 100, y: 50 }], label: null, hops: [] };
    const down: Routed = { points: [{ x: 50, y: 0 }, { x: 50, y: 100 }], label: null, hops: [] };
    const bent: Routed = { points: [{ x: 0, y: 0 }, { x: 0, y: 40 }, { x: 40, y: 40 }], label: null, hops: [] };
    const edges = new Map([
      ['across', across],
      ['down', down],
    ]);
    addHops(edges);
    expect(across.hops).toEqual([[50]]);
    expect(down.hops).toEqual([[]]);
    expect(pathOf(across)).toBe('M 0 50 L 45 50 A 5 5 0 0 1 55 50 L 100 50');
    expect(pathOf(bent)).toBe('M 0 0 L 0 31 Q 0 40 9 40 L 40 40');
  });

  it('end where the head starts, the last piece straight before it, and at the ring as they start there', () => {
    const bent: Routed = { points: [{ x: 0, y: 0 }, { x: 0, y: 40 }, { x: 40, y: 40 }], label: null, hops: [] };
    expect(pathOf(bent, { end: 7 })).toBe('M 0 0 L 0 31 Q 0 40 9 40 L 33 40');
    const close: Routed = { points: [{ x: 0, y: 0 }, { x: 0, y: 40 }, { x: 12, y: 40 }], label: null, hops: [] };
    expect(pathOf(close, { end: 7 })).toBe('M 0 0 L 0 38 Q 0 40 2 40 L 5 40');
    expect(pathOf(bent, { start: 7, end: 7 })).toBe('M 0 7 L 0 31 Q 0 40 9 40 L 33 40');
  });

  it('give a loop back to its own cell a last piece long enough for a head', () => {
    const loop = (label: Routed['label']): Routed => ({
      points: [{ x: 10, y: 0 }, { x: 0, y: 0 }, { x: 0, y: 20 }, { x: 10, y: 20 }],
      label,
      hops: [],
    });
    const far = { x: -40, y: 5, width: 20, height: 10 };
    const near = { x: -20, y: 5, width: 18, height: 10 };
    expect(lengthenLast(loop(far), 16)).toEqual({
      points: [{ x: 10, y: 0 }, { x: -6, y: 0 }, { x: -6, y: 20 }, { x: 10, y: 20 }],
      label: far,
      hops: [],
    });
    expect(lengthenLast(loop(near), 16).label).toEqual({ ...near, x: -26 });
    // A route of three points cannot move the piece that starts at its source.
    const three: Routed = { points: [{ x: 0, y: 0 }, { x: 0, y: 40 }, { x: 5, y: 40 }], label: null, hops: [] };
    expect(lengthenLast(three, 16)).toBe(three);
  });
});

describe("a cell's panel", () => {
  const a = graph.cells[1];
  it('has its tabs in order, the rounds on a work\'s chart and the checks only when there are any', () => {
    expect(cellTabs(a, { rounds: false, diff: false })).toEqual(['inputs', 'returns', 'route']);
    const checked = { ...a, checks: [{ check: 'x', at: null }] };
    expect(cellTabs(checked, { rounds: true, diff: true })).toEqual(['changes', 'rounds', 'inputs', 'returns', 'route', 'checks']);
    expect(cellTab(['inputs', 'returns', 'route'], 'checks')).toBe('inputs');
    expect(cellTab(['rounds', 'inputs', 'returns', 'route'], 'route')).toBe('route');
    expect(cellTab(['rounds', 'inputs', 'returns', 'route'], null)).toBe('rounds');
  });

  it('numbers the inputs as the message does, leaving out those the agent reads itself', () => {
    const c = cell('x', [], {
      inputs: [
        { id: null, what: 'the item', from: [{ db: 'item_body', by: 'item' }], maybe: false },
        { id: null, what: 'the code', from: ['self'], maybe: false },
        { id: null, what: 'the design', from: [['2a', 'design']], maybe: false },
      ],
    });
    expect(numbered(c).map((x) => x.no)).toEqual([1, null, 2]);
  });
});

describe("a work's rounds", () => {
  it('are shown all, or only those sent back, or only those that failed or were cut off', () => {
    const rounds = [
      round(1, 'a', 'took', 'done'),
      round(2, 'a', 'sent_back', null),
      round(3, 'a', 'failed', null),
      round(4, 'a', 'interrupted', null),
    ];
    expect(roundsShown(rounds, null)).toHaveLength(4);
    expect(roundsShown(rounds, 'sent_back').map((r) => r.n)).toEqual([2]);
    expect(roundsShown(rounds, 'failed').map((r) => r.n)).toEqual([3, 4]);
  });
});

describe('an ask', () => {
  it('keeps a command in its text as code, and the rest as prose', () => {
    const what = [
      'Please run a short probe on the Mac the engine runs on.',
      '',
      "python3 - <data dir> <<'EOF'",
      'import fcntl, os, sys',
      'fd = os.open(sys.argv[1], os.O_RDONLY)',
      'EOF',
      '',
      'Expected: line 1 taken\u3002',
      '',
      '```',
      'one line of code.',
      '```',
    ].join('\n');
    expect(parseMarkdown(what)).toEqual([
      { kind: 'para', text: 'Please run a short probe on the Mac the engine runs on.' },
      { kind: 'code', text: "python3 - <data dir> <<'EOF'\nimport fcntl, os, sys\nfd = os.open(sys.argv[1], os.O_RDONLY)\nEOF" },
      { kind: 'para', text: 'Expected: line 1 taken\u3002' },
      { kind: 'code', text: 'one line of code.' },
    ]);
  });
});

describe('a round', () => {
  it("lists the cell's inputs by their numbers as the round had them, those with nothing in their place", () => {
    const c = cell('x', [], {
      inputs: [
        { id: null, what: 'what is asked', from: [['@caller', 'ask']], maybe: false },
        { id: null, what: 'the code', from: ['self'], maybe: false },
        { id: null, what: 'the estimate', from: [['2a', 'estimate']], maybe: true },
        { id: null, what: 'the counters', from: [{ db: 'work_counters', by: 'work' }], maybe: false },
      ],
    });
    const workFrom = ['1. R6-12·2a·ask', 'Read yourself: the code', '3. R6-15·engine·work_counters', 'Rules: rule:general'];
    expect(addressesByNumber(workFrom)).toEqual(
      new Map([
        [1, ['R6-12·2a·ask']],
        [3, ['R6-15·engine·work_counters']],
      ]),
    );
    const sent = new Map([
      ['R6-12·2a·ask', false],
      ['R6-15·engine·work_counters', true],
    ]);
    expect(inputRows(c, workFrom, sent).map((r) => [r.no, r.addresses])).toEqual([
      [1, [{ address: 'R6-12·2a·ask', sent: false }]],
      [2, []],
      [3, [{ address: 'R6-15·engine·work_counters', sent: true }]],
    ]);
  });

  it('says where an input comes from, names a model as people do, and reads rows a query gave', () => {
    const title = (id: string) => `${id} Choose`;
    expect(sourceWords('R6-12·2a·ask', title)).toBe('from R6-12 at 2a Choose');
    expect(sourceWords('R6-15·engine·work_counters', title)).toBe('computed by the engine for R6-15');
    expect(sourceWords('item:3', title)).toBe('item 3');
    expect(modelName('claude-opus-5-5')).toBe('Opus 5.5');
    expect(modelName('gpt-x')).toBe('gpt-x');
    expect(jsonLines('{"a":1}\n{"a":2,"b":"x"}')).toEqual([{ a: 1 }, { a: 2, b: 'x' }]);
    expect(jsonLines('not rows')).toBeNull();
  });

  it('reads a value of a return by the path its address names, a branch keeping only what is of it', () => {
    const out = { design: 'd', end: { test_changes: [{ kind: 'added', name: 'a' }, { kind: 'removed', name: 'b' }] } };
    expect(atPath(out, 'design')).toBe('d');
    expect(atPath(out, 'end.test_changes[added]')).toEqual([{ kind: 'added', name: 'a' }]);
    expect(atPath(out, 'end.test_changes.name')).toEqual(['a', 'b']);
    expect(atPath(out, 'missing.path')).toBeUndefined();
  });
});

describe('a ruling on a design card', () => {
  const unset: Verdict = { verdict: null, text: '', option: '' };

  it('is set decision by decision: the approach recommended confirms, another is chosen, and a second click clears', () => {
    const chose = pickApproach(unset, 'B', 'A');
    expect(chose).toEqual({ verdict: 'disagree', text: '', option: 'B' });
    expect(verdictReady(chose)).toBe(true);
    expect(pickApproach(chose, 'B', 'A')).toEqual(unset);
    expect(pickApproach({ ...chose, text: 'Too slow.' }, 'B', 'A')).toEqual({ verdict: 'disagree', text: 'Too slow.', option: '' });
    expect(pickApproach(chose, 'A', 'A')).toEqual({ verdict: 'confirm', text: '', option: '' });
    expect(verdictReady({ verdict: 'disagree', text: ' ', option: '' })).toBe(false);
    expect(verdictReady(unset)).toBe(false);
  });

  it('is summed up before sending, one line for each decision', () => {
    expect(verdictLine(1, 'confirm', '', '')).toBe('Decision 1: confirm');
    expect(verdictLine(2, 'disagree', 'B', 'One table is enough.')).toBe('Decision 2: disagree, choosing B: One table is enough.');
    expect(verdictLine(3, 'disagree', '', 'No.')).toBe('Decision 3: disagree: No.');
    expect(verdictLine(4, null, '', '')).toBe('Decision 4: not ruled yet');
  });

  it('titles each decision with its first sentence', () => {
    expect(firstSentence('Keep one table. It is simpler.')).toBe('Keep one table.');
    expect(firstSentence('v1.2 stays\nmore')).toBe('v1.2 stays');
    expect(firstSentence('一二。三')).toBe('一二。');
    expect(firstSentence('x'.repeat(200)).length).toBe(120);
  });
});
