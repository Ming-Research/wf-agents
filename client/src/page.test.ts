import { describe, expect, it } from 'vitest';

import type { CompiledCell, CompiledGraph, CompiledRow, GraphView, RoundSummary } from './api';
import { conditionsWords, conditionWords, targetWords, valuesWords } from './flow/words';
import { cost, duration } from './format';
import { addHops, pathOf, type Route as Routed } from './graph/flowLayout';
import { ASKING, chart, circled, MAIN, numbers, overlays, partsOf, sharedRows, taken } from './graph/flowModel';
import { named } from './live';
import { blocks } from './pages/Decide';
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
    const routes: Route[] = [
      { page: 'works' },
      { page: 'work', id: 3, round: 240, cell: null },
      { page: 'work', id: 3, round: null, cell: '2a' },
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
    expect(parse('/', '')).toEqual({ page: 'works' });
    expect(parse('/works/x', '')).toEqual({ page: 'missing' });
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
    expect(circled(0)).toBe('①');
    expect(circled(25)).toBe('(26)');
  });
});

describe('events', () => {
  it('name what to read again', () => {
    expect(named({ kind: 'round', work: 2, round: 9 })).toEqual(['works', 'work:2', 'round:9', 'items', 'item:*']);
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

function round(id: number, cellId: string | null, outcome: RoundSummary['outcome'], transition: string | null): RoundSummary {
  return {
    id,
    cell: cellId,
    line: null,
    outcome,
    transition,
    why: null,
    started_at: id,
    ended_at: outcome === 'running' ? null : id + 1,
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
  name: 'engineer',
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

  it('draws the rows every agent cell shares as one circle beside it, numbered, those back to itself too', () => {
    expect(sharedRows(graph.cells[1])).toEqual([0, 3, 4]);
    expect(node('k:a')?.rows).toEqual([0, 3, 4]);
    expect(edge('c:a->k:a')?.label).toBe('①④⑤');
    expect(edge('c:a->c:t')).toBeUndefined();
    expect(numbers([0, 3, 4, 5, 6, 8])).toBe('①④–⑦⑨');
  });

  it('draws the rows to one cell as one arrow with their numbers and names, the main path marked', () => {
    expect(edge('c:a->c:b')).toMatchObject({ rows: [1, 2], label: '②③ done, again', main: true });
    expect(edge('c:s->c:a')?.label).toBe('started');
  });

  it('draws a row to another part as a circle beside the cell, and one beside the cell it comes back into', () => {
    expect(node('o:r:t')).toMatchObject({ kind: 'out', cell: 't', part: 't' });
    expect(node('o:a:q')).toMatchObject({ kind: 'out', cell: 'q', part: ASKING });
    expect(node('i:q:a')).toMatchObject({ kind: 'in', cell: 'a', part: MAIN });
    // Back to where the work stopped: from the cell whose row decided it.
    expect(node('i:t:a')).toMatchObject({ kind: 'in', cell: 'a' });
    expect(edge('i:t:a->c:a')?.label).toBe('② failed');
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
    const o = overlays(rounds, 'b');
    expect(o.get('a')).toEqual({ rounds: 3, sentBack: 1, failed: 0, running: false, current: false });
    expect(o.get('b')).toEqual({ rounds: 2, sentBack: 0, failed: 1, running: true, current: true });
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
    expect(blocks(what)).toEqual([
      { code: false, text: 'Please run a short probe on the Mac the engine runs on.' },
      { code: true, text: "python3 - <data dir> <<'EOF'\nimport fcntl, os, sys\nfd = os.open(sys.argv[1], os.O_RDONLY)\nEOF" },
      { code: false, text: 'Expected: line 1 taken\u3002' },
      { code: true, text: 'one line of code.' },
    ]);
  });
});
