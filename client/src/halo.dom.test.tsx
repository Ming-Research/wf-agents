// @vitest-environment happy-dom
// The halo of a work's chart, counted in a DOM: it is one element that stays while what the page reads changes (the
// rounds read again, the overlays of the cell, the other cells' marks), and it moves only when the cell a stage works
// at is another, with no moment of none or of two between. The page draws the chart through `FlowSvg`, whose halo is
// a child of the cell it goes round, at one place in that cell's tree.

import './dom-setup';

import { act, createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import type { CompiledCell, CompiledGraph, CompiledRow, EngineNames, GraphView } from './api';
import { FlowSvg, type Marks, sizes } from './graph/flowDraw';
import type { Laid } from './graph/flowLayout';
import { chart, overlays } from './graph/flowModel';
import { coloursOf } from './roles';

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

const cell = (id: string, rows: CompiledRow[], extra: Partial<CompiledCell> = {}): CompiledCell => ({
  id,
  kind: 'agent',
  gate: false,
  title: `cell ${id}`,
  step: null,
  line: 'engineer',
  stage: 'build',
  inputs: [],
  prompt_by: null,
  schema: null,
  rows,
  ...extra,
});

// A start, two cells of an agent that follow each other, and the end.
const graph: CompiledGraph = {
  name: 'engineer',
  start: 's',
  end: 'z',
  within: 'outcome',
  asking: [],
  receive: null,
  counters: [],
  rules: [],
  interject: { name: 'interjection', desc: 'What is said', text: [], from: [] },
  rounds: {
    desc: '',
    sentBack: { desc: 'Sent back to be written again.', pauseAfter: 3, pause: 'Paused after three.' },
    failed: { desc: '', kinds: [] },
    record: [],
    environment: [],
  },
  cells: [
    cell('s', [row('started', 'a', { main: true })], { kind: 'engine', line: null, stage: 'start' }),
    cell('a', [row('done', 'b', { main: true })]),
    cell('b', [row('done', 'z', { main: true })]),
    cell('z', [], { kind: 'engine', line: null, stage: 'done' }),
  ],
};

const view: GraphView = {
  name: 'engineer',
  common_format: '',
  cells: graph.cells.map((c) => ({
    id: c.id,
    targets: c.rows.map((r) => [r.to]),
    entered_from: graph.cells.flatMap((x) => x.rows.flatMap((r, i) => (r.to === c.id ? [{ cell: x.id, row: i }] : []))),
    inputs: [],
    returns: { within: null, forms: [], every: [], used_by: [] },
  })),
};

const engine: EngineNames = {
  steps: {},
  actions: {},
  checks: {},
  inputs: {},
  values: {},
  needs: {},
  stores: {},
  writes: {},
  reads: {},
};

const colours = coloursOf([{ graph: 'engineer', lines: [{ name: 'engineer', desc: 'The engineer', colour: 'blue' }] }]);
const model = chart(graph, view, engine);
const measured = sizes(graph, model, engine);
const laid: Laid = {
  nodes: new Map(model.nodes.map((n, i) => [n.id, { x: 0, y: i * 60, width: 120, height: 40 }])),
  groups: new Map(),
  edges: new Map(
    model.edges.map((x, i) => [x.id, { points: [{ x: 200, y: i * 10 }, { x: 200, y: i * 10 + 50 }], label: null, hops: [] }]),
  ),
  parts: new Map(),
  bounds: { x: 0, y: 0, width: 300, height: model.nodes.length * 60 },
};

let root: Root | null = null;
let host: HTMLElement | null = null;

afterEach(async () => {
  const mine = root;
  if (mine !== null) await act(async () => mine.unmount());
  host?.remove();
  root = null;
  host = null;
});

/// Draws the chart with the halo on `halo` and `running` on the overlay of `at`; `rounds` varies what else the marks hold.
async function draw(halo: string | null, at: string, running: boolean, sentBack = 0): Promise<HTMLElement> {
  if (host === null) {
    host = document.createElement('div');
    document.body.append(host);
    root = createRoot(host);
  }
  const marks: Marks = {
    selected: null,
    overlays: overlays([], at, { running, waits: null }),
    taken: null,
    added: new Set(),
    changed: new Set(),
    halo,
  };
  if (sentBack > 0) marks.overlays?.set('s', { rounds: sentBack, sentBack, failed: 0, running: false, current: false, waits: null });
  const props = { g: graph, chart: model, laid, lines: measured.lines, notes: measured.notes, engine, colours, marks };
  const mine = root;
  if (mine === null) throw new Error('no root');
  await act(async () => mine.render(createElement(FlowSvg, props)));
  return host;
}

describe('the halo in a DOM', () => {
  it('is one element that stays while what is read changes, and moves to the next cell with none or two between', async () => {
    const halos = (h: HTMLElement) => [...h.querySelectorAll('.fc-halo')];
    const first = halos(await draw('a', 'a', true));
    expect(first.length).toBe(1);
    const node = first[0];
    // The cell is drawn again for the other cells' overlays, the running mark of its ring going out and coming back,
    // as the status events and the rounds read again make the marks differ: it is the same element, not a new one.
    for (const [running, sentBack] of [
      [true, 2],
      [false, 0],
      [true, 0],
      [true, 5],
    ] as const) {
      const now = halos(await draw('a', 'a', running, sentBack));
      expect(now.length).toBe(1);
      expect(now[0]).toBe(node);
    }
    // The stage that worked ends, and the next works: the halo is on the next cell, and at no time on none or on two.
    const next = halos(await draw('b', 'b', true));
    expect(next.length).toBe(1);
    expect(next[0]).not.toBe(node);
    expect(next[0].closest('[data-node]')?.getAttribute('data-node')).toBe('c:b');
    // No stage works: none.
    expect(halos(await draw(null, 'b', false)).length).toBe(0);
  });
});
