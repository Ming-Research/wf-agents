// The items view and an item's page: the tree of what depends on what, the checklist of a work's rounds, and the
// markdown agents write.

import { describe, expect, it } from 'vitest';

import type { CompiledCell, CompiledGraph, CompiledRow, ItemLink, ItemSummary, RoundSummary } from './api';
import { byStage, mainPath, progress, type Step } from './item/checklist';
import { depsDone, stageShare, tree, type TreeRow } from './item/tree';
import { inline, parse } from './markdown';

function item(id: number, state: ItemSummary['state'], extra: Partial<ItemSummary> = {}): ItemSummary {
  return {
    id,
    project: 'p',
    title: `item ${id}`,
    deliver: 'change',
    level: '1',
    cancelled: null,
    state,
    queued: null,
    work: null,
    at: null,
    stage: null,
    stages: [],
    open_asks: 0,
    open_design_cards: 0,
    pr: null,
    ...extra,
  };
}

const link = (from: number, to: number): ItemLink => ({ item: from, depends_on: to, needs: '' });

/// A tree as lines: the item's number, indented by depth, a link to where it is shown in full marked `->`.
function lines(rows: TreeRow[], depth = 0): string[] {
  return rows.flatMap((r) => [
    `${'  '.repeat(depth)}${r.full ? '' : '->'}${r.item.id}`,
    ...lines(r.children, depth + 1),
  ]);
}

describe('the tree of items', () => {
  // 1 needs 2 and 3, 4 needs 3 too, 3 needs 5; 6 stands alone.
  const items = [
    item(1, 'working'),
    item(2, 'done'),
    item(3, 'queued', { queued: 2 }),
    item(4, 'new'),
    item(5, 'new'),
    item(6, 'queued', { queued: 1 }),
  ];
  const links = [link(1, 2), link(1, 3), link(4, 3), link(3, 5)];

  it('puts what nothing waits for at the top, and what an item depends on under it', () => {
    expect(lines(tree(items, links))).toEqual(['1', '  3', '    5', '  2', '6', '4', '  ->3']);
  });

  it('shows an item several depend on in full under the first only', () => {
    const rows = tree(items, links);
    const shared = rows.find((r) => r.item.id === 4)?.children[0];
    expect(shared).toMatchObject({ full: false, children: [] });
  });

  it('makes an item a top one when everything that waits for it is hidden', () => {
    const shown = items.filter((i) => i.state !== 'working');
    expect(lines(tree(shown, links))).toEqual(['6', '4', '  3', '    5', '2']);
  });

  it('counts what is done of what an item depends on, and how far its stage is', () => {
    expect(depsDone(1, items, links)).toEqual({ done: 1, of: 2 });
    expect(depsDone(6, items, links)).toEqual({ done: 0, of: 0 });
    expect(stageShare('design', ['queued', 'research', 'design', 'done'])).toBe(0.75);
    expect(stageShare(null, ['queued'])).toBeNull();
    expect(stageShare('gone', ['queued'])).toBeNull();
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

function cell(id: string, stage: string | null, rows: CompiledRow[], extra: Partial<CompiledCell> = {}): CompiledCell {
  return {
    id,
    kind: 'agent',
    gate: false,
    title: `cell ${id}`,
    step: null,
    needs: null,
    line: 'engineer',
    stage,
    inputs: [],
    prompt_by: null,
    schema: null,
    rows,
    ...extra,
  };
}

/// A cell at no stage, as a compiled flow writes it: with the stage left out.
function noStage(c: CompiledCell): CompiledCell {
  const copy: Partial<CompiledCell> = { ...c };
  delete copy.stage;
  return copy as CompiledCell;
}

const shared = { common: true };
const engine = { kind: 'engine', line: null };

// Research (a, and the gate g), design (b), a step that only routes (k), review (c) and its fix (f), a wait before
// the merge (m), the end (z); the tools (t) any agent may ask for, and the owner (q) any agent may ask.
const graph: CompiledGraph = {
  name: 'engineer',
  start: 's',
  end: 'z',
  within: 'outcome',
  asking: ['q'],
  receive: null,
  counters: [],
  rules: [],
  interject: null,
  cells: [
    cell('s', 'queued', [row('started', 'a', { main: true })], engine),
    cell('a', 'research', [row('done', 'g', { main: true }), row('tools', 't', { ...shared, main: true }), row('ask', 'q', shared)]),
    cell('g', 'research', [row('no', 'b', { main: true }), row('tools', 't', shared)], { gate: true }),
    cell('b', 'design', [row('done', 'k', { main: true }), row('tools', 't', shared), row('ask', 'q', shared)]),
    cell('k', 'test', [row('review', 'c', { main: true }), row('fix', 'f')], engine),
    cell('c', 'test', [row('done', 'm', { main: true }), row('back', 'f'), row('ask', 'q', shared)]),
    cell('f', 'test', [row('done', 'k'), row('ask', 'q', shared)]),
    cell('m', 'merge', [row('wait', 'm', { main: true, waits: true }), row('clear', 'z', { main: true })], engine),
    noStage(cell('t', null, [row('ran', '@caller', { main: true })], { kind: 'tool', line: null })),
    noStage(cell('q', null, [row('answered', '@asker')], { kind: 'owner', line: null })),
    cell('z', 'done', [], engine),
  ],
};

function round(id: number, at: string, transition: string | null, extra: Partial<RoundSummary> = {}): RoundSummary {
  return {
    id,
    cell: at,
    line: null,
    outcome: transition === null ? 'running' : 'took',
    transition,
    why: null,
    started_at: id,
    ended_at: transition === null ? null : id + 1,
    model: null,
    cost_usd: null,
    compactions: 0,
    tool_runs: 0,
    progress: null,
    not_asked: null,
    ...extra,
  };
}

const summary = (steps: Step[]) => steps.map((s) => `${s.cell} ${s.state}${s.passes > 1 ? ` x${s.passes}` : ''}`);

describe('the checklist of a work', () => {
  const rounds = [
    round(1, 's', 'started'),
    round(2, 'a', 'tools'),
    round(3, 't', 'ran'),
    round(4, 'a', 'done', { progress: 'understood it' }),
    round(5, 'g', 'no', { not_asked: 'plan' }),
    round(6, 'b', 'ask'),
    round(7, 'q', 'answered'),
    round(8, 'b', 'done', { progress: 'designed it' }),
    round(9, 'k', 'review'),
    round(10, 'c', 'back'),
    round(11, 'f', 'done'),
    round(12, 'k', 'review'),
    round(13, 'c', null),
  ];

  it('takes the main path from the start, each cell after those that lead to it', () => {
    expect(mainPath(graph)).toEqual(['s', 'a', 'g', 'b', 'k', 'c', 'm', 'z']);
  });

  it('lists each cell the work was at once, the one it is at, then what is ahead on the main path', () => {
    const steps = progress(graph, rounds, { at: 'c', resumeAt: null, live: true });
    // No line for the start, the step that only routes, the end, a gate the engine answered itself, or the tools.
    expect(summary(steps)).toEqual(['a done', 'b done', 'q done', 'c now x2', 'f done', 'm ahead']);
    const a = steps[0];
    // The tools' round counts under the cell that asked for them.
    expect(a.rounds.map((r) => r.id)).toEqual([2, 3, 4]);
    expect(a.row).toBe('done');
    // Coming back from asking the owner is not getting to the cell again; the ask is under the stage it was asked at.
    expect(steps[1].passes).toBe(1);
    expect(steps[2].stage).toBe('design');
    expect(byStage(graph, steps).map((x) => [x.stage, x.steps.map((s) => s.cell)])).toEqual([
      ['research', ['a']],
      ['design', ['b', 'q']],
      ['test', ['c', 'f']],
      ['merge', ['m']],
    ]);
  });

  it('goes on from the cell the work left to ask, and from the cell its tools serve', () => {
    const asking = progress(graph, rounds.slice(0, 7), { at: 'q', resumeAt: 'b', live: true });
    expect(summary(asking)).toEqual(['a done', 'b done', 'q now', 'c ahead', 'm ahead']);
    const tools = progress(graph, rounds.slice(0, 3), { at: 't', resumeAt: null, live: true });
    expect(summary(tools)).toEqual(['a now', 'g ahead', 'b ahead', 'c ahead', 'm ahead']);
  });

  it('marks nothing now or ahead for a work that ended', () => {
    const steps = progress(graph, rounds, { at: 'c', resumeAt: null, live: false });
    expect(steps.every((s) => s.state === 'done')).toBe(true);
  });
});

describe('the markdown agents write', () => {
  it('reads headings, lists in lists, fences and paragraphs', () => {
    const text = [
      '## The plan',
      'First do this:',
      '- one `thing`',
      '- two',
      '  - two and a half',
      '',
      '1. then',
      '2. after',
      '',
      '```rust',
      'fn main() {}',
      '```',
      'The end.',
    ].join('\n');
    expect(parse(text)).toEqual([
      { kind: 'heading', level: 2, text: 'The plan' },
      { kind: 'para', text: 'First do this:' },
      {
        kind: 'list',
        ordered: false,
        start: 1,
        items: [
          [{ kind: 'para', text: 'one `thing`' }],
          [
            { kind: 'para', text: 'two' },
            { kind: 'list', ordered: false, start: 1, items: [[{ kind: 'para', text: 'two and a half' }]] },
          ],
        ],
      },
      { kind: 'list', ordered: true, start: 1, items: [[{ kind: 'para', text: 'then' }], [{ kind: 'para', text: 'after' }]] },
      { kind: 'code', text: 'fn main() {}' },
      { kind: 'para', text: 'The end.' },
    ]);
  });

  it('reads inline code, bold and links, and leaves what is not markup as text', () => {
    expect(inline('Run `cargo test` **now**, see [the run](https://example.com/1) and 2 * 3')).toEqual([
      { kind: 'text', text: 'Run ' },
      { kind: 'code', text: 'cargo test' },
      { kind: 'text', text: ' ' },
      { kind: 'bold', children: [{ kind: 'text', text: 'now' }] },
      { kind: 'text', text: ', see ' },
      { kind: 'link', href: 'https://example.com/1', children: [{ kind: 'text', text: 'the run' }] },
      { kind: 'text', text: ' and 2 * 3' },
    ]);
    expect(inline('a `b')).toEqual([{ kind: 'text', text: 'a `b' }]);
    expect(inline('snake_case_name')).toEqual([{ kind: 'text', text: 'snake_case_name' }]);
  });
});
