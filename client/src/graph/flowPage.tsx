// The flow page as the browser tests of Space draw it (Viewer.browser.test.tsx, ViewerKeys.browser.test.tsx): the real
// page (`Flow`, with the page's own styles and Primer's theme) reading a flow of 24 cells from a stand-in for the
// interface, and what they look at: where the drawing is on the screen, what the keyboard and the pointer did. Only the
// tests in `*.browser.test.tsx` import it, since it draws in a real browser.

import '@primer/primitives/dist/css/primitives.css';
import '@primer/primitives/dist/css/functional/themes/light.css';
import '@primer/primitives/dist/css/functional/themes/dark.css';
import '../page-styles';

import { BaseStyles } from '@primer/react';
import { ThemeProvider } from '@primer/react/next';
import { expect, vi } from 'vitest';
import { render } from 'vitest-browser-react';

import type { Compiled, CompiledCell, CompiledGraph, CompiledRow, FlowVersion, FlowView, GraphView } from '../api';
import { Flow } from '../pages/Flow';
import type { Route } from '../route';

// The flow: a chain of cells, tall enough that the whole of it is far smaller than the zoom the chart opens at.
const row = (name: string, to: string): CompiledRow => ({
  name,
  if: [['all']],
  to,
  counts: null,
  waits: false,
  pauses: false,
  interject: false,
  main: true,
  common: false,
  note: null,
});

const cell = (id: string, title: string, rows: CompiledRow[]): CompiledCell => ({
  id,
  kind: 'engine',
  gate: false,
  title,
  step: null,
  line: null,
  stage: null,
  inputs: [],
  prompt_by: null,
  schema: null,
  rows,
});

const IDS = Array.from({ length: 24 }, (_, i) => `K${i + 1}`);
const cells = IDS.map((id, i) => cell(id, `Step ${i + 1} of the chain`, i + 1 < IDS.length ? [row('next', IDS[i + 1])] : []));

const graph: CompiledGraph = {
  name: 'chain',
  start: IDS[0],
  end: IDS[IDS.length - 1],
  within: 'outcome',
  asking: [],
  receive: null,
  counters: [],
  rules: [],
  interject: null,
  cells,
};

const graphView: GraphView = {
  name: graph.name,
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
  engine: { steps: {}, actions: {}, checks: {}, inputs: {}, values: {}, needs: {} },
  tools: [],
  graphs: [graph],
  settings: [],
};
const view: FlowView = { graphs: [graphView] };
const versions: FlowVersion[] = [{ id: 1, digest: '0'.repeat(64), works: 0 }];

/// What the interface answers, by path; any other path of the interface is a 404.
const served = new Map<string, unknown>([
  ['/api/v1/flows', versions],
  ['/api/v1/flows/1', compiled],
  ['/api/v1/flows/1/view', view],
  ['/api/v1/flows/lines', []],
]);

const ROUTE: Extract<Route, { page: 'flow' }> = { page: 'flow', version: null, graph: null, cell: null, project: null, ref: null, snapshot: null, from: null };

/// Every key the page saw, as it ended up once the whole page had handled it.
export const keys: Array<{ code: string; prevented: boolean }> = [];
/// Every key at the first place on its way (the window, before anything on the page), with where the keyboard was.
export const arrived: Array<{ code: string; target: string; active: string; focused: boolean }> = [];
let pointer: { x: number; y: number } | null = null;
/// Where the pointer last moved.
export const pointerAt = () => pointer;
/// An element as a short name for a message: its tag and first class.
export const describeElement = (e: Element | null) =>
  e === null ? 'none' : `${e.tagName.toLowerCase()}${e.className && typeof e.className === 'string' ? `.${e.className.split(' ')[0]}` : ''}`;
const noteKey = (e: KeyboardEvent) => keys.push({ code: e.code, prevented: e.defaultPrevented });
const noteArrival = (e: KeyboardEvent) =>
  arrived.push({
    code: e.code,
    target: describeElement(e.target as Element | null),
    active: describeElement(document.activeElement),
    focused: document.hasFocus(),
  });
const notePointer = (e: PointerEvent) => {
  pointer = { x: e.clientX, y: e.clientY };
};

/// Stands in for the interface and starts noting keys and the pointer; `unstubPage` ends it.
export function stubPage() {
  keys.length = 0;
  arrived.length = 0;
  pointer = null;
  const real = window.fetch.bind(window);
  vi.stubGlobal('fetch', async (input: RequestInfo | URL, init?: RequestInit) => {
    const path = typeof input === 'string' ? input : input instanceof URL ? input.pathname + input.search : input.url;
    // Only the interface is stood in for: Vitest and the page's modules are read as they are.
    if (!path.startsWith('/api/')) return real(input, init);
    const body = served.has(path) ? served.get(path) : { error: `no ${path} here` };
    return new Response(JSON.stringify(body), { status: served.has(path) ? 200 : 404 });
  });
  // On the window, after the document where the viewer listens, so that `defaultPrevented` is the page's whole answer.
  window.addEventListener('keydown', noteKey);
  window.addEventListener('keydown', noteArrival, true);
  document.addEventListener('pointermove', notePointer, true);
}

export function unstubPage() {
  window.removeEventListener('keydown', noteKey);
  window.removeEventListener('keydown', noteArrival, true);
  document.removeEventListener('pointermove', notePointer, true);
  vi.unstubAllGlobals();
}

export function find<T extends HTMLElement>(selector: string): T {
  const found = document.querySelector<T>(selector);
  if (found === null) throw new Error(`the page has no ${selector}`);
  return found;
}

/// The page's flow screen, as the page draws it under its bar, with the page's styles and Primer's theme.
export async function openFlowPage() {
  const root = document.documentElement;
  root.setAttribute('data-color-mode', 'auto');
  root.setAttribute('data-light-theme', 'light');
  root.setAttribute('data-dark-theme', 'dark');
  const screen = await render(
    <ThemeProvider colorMode="auto">
      <BaseStyles>
        <div className="app app-screen">
          <main>
            <Flow route={ROUTE} />
          </main>
        </div>
      </BaseStyles>
    </ThemeProvider>,
  );
  // Laid out, and the viewer has put the drawing where it opens.
  await landed(() => (document.querySelector<HTMLElement>('.viewer-stage')?.style.transform ?? '') !== '', 15_000);
  return screen;
}

/// Waits until `ready` holds and the drawing has stopped moving: two looks, 100 ms apart, find it in the same place. A
/// move of the view takes a fifth of a second, and what is measured while it goes on is not where it ends.
export async function landed(ready: () => boolean, timeout: number): Promise<void> {
  let last = '';
  await expect
    .poll(
      () => {
        const s = ready() ? seen() : null;
        const now = s === null ? '' : JSON.stringify([s.zoom, s.left, s.top]);
        const still = now !== '' && now === last;
        last = now;
        return still;
      },
      { timeout, interval: 100 },
    )
    .toBe(true);
}

/// Where the drawing is on the screen: its zoom (its width on the screen over its width in the layout), and how far
/// each of its edges is inside the viewer's (negative: outside it).
export function seen() {
  const sheets = find('.fc-sheets');
  const d = sheets.getBoundingClientRect();
  const v = find('.viewer').getBoundingClientRect();
  return {
    zoom: d.width / sheets.offsetWidth,
    left: d.left - v.left,
    top: d.top - v.top,
    right: v.right - d.right,
    bottom: v.bottom - d.bottom,
    d,
    v,
  };
}

/// The topmost part of a cell that is in the upper third of the viewer.
export function cellNearTop(): Element {
  const v = find('.viewer').getBoundingClientRect();
  const found = [...document.querySelectorAll('.fc-sheets [data-node]')]
    .map((e) => ({ e, r: e.getBoundingClientRect() }))
    .filter(({ r }) => r.width > 20 && r.height > 20 && r.top >= v.top && r.bottom <= v.top + v.height / 3)
    .sort((a, b) => a.r.top - b.r.top);
  if (found.length === 0) throw new Error('no cell is in the upper third of the viewer');
  return found[0].e;
}

/// Whether the whole drawing is in the viewer, framed as the viewer frames it: a 16 pixel margin at both sides of the
/// side it fills.
export function whole(): boolean {
  const s = seen();
  return Math.min(s.left, s.top, s.right, s.bottom) >= -0.5 && Math.abs(Math.min(s.left, s.top) - 16) <= 1.5;
}

/// Whether the drawing is where it was in the viewer (`before`, from `seen`), to within a fraction of a pixel.
export function unmoved(before: ReturnType<typeof seen>): boolean {
  const now = seen();
  return (
    Math.abs(now.zoom - before.zoom) < 0.001 &&
    Math.abs(now.left - before.left) < 0.5 &&
    Math.abs(now.top - before.top) < 0.5
  );
}

/// The window waits out the move a Space would start (a fifth of a second and the timer behind it) before it says that
/// nothing moved: a move that has not begun yet would look like none.
export const MOVE_WAIT_MS = 500;
export const settle = () => new Promise((done) => setTimeout(done, MOVE_WAIT_MS));
