import { describe, expect, it } from 'vitest';

import type { Atom, CompiledCell, CompiledGraph, CompiledRow, RoundSummary } from './api';
import { cost, duration } from './format';
import { build, overlays, taken } from './graph/flowGraph';
import { catalogs, langOf, translate } from './i18n';
import { named } from './live';
import { href, parse } from './route';
import { atomText, srcText } from './text';

describe('the message catalog', () => {
  it('has every text in both languages, none empty, with the same values to fill', () => {
    const fills = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
    for (const [key, text] of Object.entries(catalogs.en)) {
      const zh = catalogs.zh[key as keyof typeof catalogs.en];
      expect(text.trim(), key).not.toBe('');
      expect(zh.trim(), key).not.toBe('');
      expect(fills(zh), key).toEqual(fills(text));
    }
  });

  it('takes the language from the configuration', () => {
    expect(langOf('Chinese')).toBe('zh');
    expect(langOf('English')).toBe('en');
    expect(langOf(null)).toBe('en');
    expect(translate('en', 'slots', { used: 1, total: 2 })).toBe('Slots 1/2');
  });
});

describe('the addresses', () => {
  it('read back what they write', () => {
    const routes = [
      { page: 'works' as const },
      { page: 'work' as const, id: 3, round: 240, cell: null },
      { page: 'work' as const, id: 3, round: null, cell: '2a' },
      { page: 'items' as const },
      { page: 'item' as const, id: 5 },
      { page: 'flow' as const, version: 1, cell: '13' },
      { page: 'flow' as const, version: null, cell: null },
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

describe('the flow written out', () => {
  it('writes sources and conditions on one line', () => {
    expect(srcText(['7', 'result.head'])).toBe('7 → result.head');
    expect(srcText({ db: 'work_state', by: 'work' })).toBe('db work_state by work');
    const a: Atom = ['or', [['own'], ['is', 'answer.verdict', ['yes', 'no']]]];
    expect(atomText(a)).toBe('(own or answer.verdict is yes | no)');
  });
});

describe('events', () => {
  it('name what to read again', () => {
    expect(named({ kind: 'round', work: 2, round: 9 })).toEqual(['works', 'work:2', 'round:9', 'items', 'item:*']);
    expect(named({ kind: 'item', item: null })).toEqual(['items', 'item:*', 'works']);
    expect(named({ kind: 'ready' })).toEqual([]);
  });
});

function row(name: string, to: string, extra: Partial<CompiledRow> = {}): CompiledRow {
  return {
    name,
    if: [['all']],
    to,
    counts: null,
    waits: false,
    interject: false,
    main: false,
    common: false,
    note: null,
    ...extra,
  };
}

function cell(id: string, rows: CompiledRow[]): CompiledCell {
  return {
    id,
    kind: 'agent',
    gate: false,
    title: `cell ${id}`,
    step: null,
    needs: null,
    line: 'engineer',
    stage: null,
    inputs: [],
    project_rules: null,
    prompt_by: null,
    schema: null,
    rows,
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
  };
}

const graph: CompiledGraph = {
  name: 'engineer',
  start: 'a',
  end: 'c',
  within: 'outcome',
  asking: [],
  receive: null,
  counters: [],
  rules: [],
  interject: null,
  cells: [
    cell('a', [row('done', 'b', { main: true }), row('ask', 't', { common: true }), row('pause', '@self')]),
    cell('b', [row('done', 'c', { main: true }), row('back', 'a'), row('ask', 't', { common: true })]),
    cell('t', [row('ran', '@caller')]),
    cell('c', []),
  ],
};

describe('a work over its flow', () => {
  const rounds = [
    round(1, 'a', 'took', 'ask'),
    round(2, 't', 'took', 'ran'),
    round(3, 'a', 'sent_back', null),
    round(4, 'a', 'took', 'done'),
    round(5, 'b', 'failed', null),
    round(6, 'b', 'running', null),
  ];

  it('marks what the rounds did at each cell', () => {
    const o = overlays(rounds, 'b');
    expect(o.get('a')).toEqual({ rounds: 3, sentBack: 1, failed: 0, running: false, current: false });
    expect(o.get('b')).toEqual({ rounds: 2, sentBack: 0, failed: 1, running: true, current: true });
  });

  it('follows a row back to the cell that called', () => {
    const w = taken(graph, rounds);
    expect(w.get('a->t')?.times).toBe(1);
    expect(w.get('t->a')?.times).toBe(1);
    expect(w.get('a->b')?.times).toBe(1);
  });

  it('draws the rows every agent cell has only when asked, or when the work went that way', () => {
    const plain = build(graph, { common: false });
    expect(plain.edges.map((e) => e.id).sort()).toEqual(['a->b', 'b->a', 'b->c']);
    const all = build(graph, { common: true });
    expect(all.edges.map((e) => e.id).sort()).toEqual(['a->b', 'a->t', 'b->a', 'b->c', 'b->t']);
    const work = build(graph, { common: false, rounds, at: 'b' });
    expect(work.edges.map((e) => e.id).sort()).toEqual(['a->b', 'a->t', 'b->a', 'b->c', 't->a']);
    expect(work.edges.find((e) => e.id === 't->a')?.taken).toBe(1);
  });
});
