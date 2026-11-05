// Space on the flow page, in a real browser (Chromium, through Vitest's browser mode). With the pointer on the chart,
// Space shows the whole flow and the next Space goes back to the zoom it left; with the pointer on something floating
// over the chart, or in a box the owner types in, Space does neither. Viewer.dom.test.tsx counts the same in happy-dom,
// where the layout, what is under a point and the keys are made up by the test; here the page is drawn by the browser,
// the pointer is moved and the key pressed by the browser's own input (the DevTools protocol), and what is checked is
// where the drawing is on the screen. The page is the real one (`Flow`, with the page's own styles), reading a flow of
// 24 cells from a stand-in for the interface.

import '@primer/primitives/dist/css/primitives.css';
import '@primer/primitives/dist/css/functional/themes/light.css';
import '@primer/primitives/dist/css/functional/themes/dark.css';
import '../page-styles';

import { BaseStyles } from '@primer/react';
import { ThemeProvider } from '@primer/react/next';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { userEvent } from 'vitest/browser';
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

/// Every key the page saw, as it ended up once the whole page had handled it, and where the pointer last moved.
const keys: Array<{ code: string; prevented: boolean }> = [];
let pointer: { x: number; y: number } | null = null;
const noteKey = (e: KeyboardEvent) => keys.push({ code: e.code, prevented: e.defaultPrevented });
const notePointer = (e: PointerEvent) => {
  pointer = { x: e.clientX, y: e.clientY };
};

beforeEach(() => {
  keys.length = 0;
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
  document.addEventListener('pointermove', notePointer, true);
});

afterEach(() => {
  window.removeEventListener('keydown', noteKey);
  document.removeEventListener('pointermove', notePointer, true);
  vi.unstubAllGlobals();
});

function find<T extends HTMLElement>(selector: string): T {
  const found = document.querySelector<T>(selector);
  if (found === null) throw new Error(`the page has no ${selector}`);
  return found;
}

/// The page's flow screen, as the page draws it under its bar, with the page's styles and Primer's theme.
async function openFlowPage() {
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
async function landed(ready: () => boolean, timeout: number): Promise<void> {
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
function seen() {
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
function cellNearTop(): Element {
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
function whole(): boolean {
  const s = seen();
  return Math.min(s.left, s.top, s.right, s.bottom) >= -0.5 && Math.abs(Math.min(s.left, s.top) - 16) <= 1.5;
}

/// Whether the drawing is where it was in the viewer (`before`, from `seen`), to within a fraction of a pixel.
function unmoved(before: ReturnType<typeof seen>): boolean {
  const now = seen();
  return (
    Math.abs(now.zoom - before.zoom) < 0.001 &&
    Math.abs(now.left - before.left) < 0.5 &&
    Math.abs(now.top - before.top) < 0.5
  );
}

/// The window waits out the move a Space would start (a fifth of a second and the timer behind it) before it says that
/// nothing moved: a move that has not begun yet would look like none.
const MOVE_WAIT_MS = 500;
const settle = () => new Promise((done) => setTimeout(done, MOVE_WAIT_MS));

describe('Space on the flow page, in a real browser', () => {
  it('shows the whole flow with the pointer on the chart and goes back to the zoom it left', async () => {
    await openFlowPage();
    const opened = seen();
    // The chart opens at a zoom one can read, on part of the flow.
    expect(whole()).toBe(false);

    // The pointer is on a cell near the top of the viewer, far from the viewer's middle, where Space goes back to.
    await userEvent.hover(cellNearTop());
    const at = pointer;
    if (at === null) throw new Error('the browser sent no pointer move');
    const start = find('.viewer').getBoundingClientRect();
    expect(Math.abs(at.y - (start.top + start.height / 2))).toBeGreaterThan(100);

    await userEvent.keyboard(' ');
    // The browser reported the key as Space, and the page took it.
    expect(keys[0]).toEqual({ code: 'Space', prevented: true });
    await landed(whole, 3000);
    const all = seen();
    expect(all.zoom).toBeLessThan(opened.zoom / 1.05);
    // The point of the drawing under the pointer now, in the drawing's own pixels.
    const under = { x: (at.x - all.d.left) / all.zoom, y: (at.y - all.d.top) / all.zoom };

    await userEvent.keyboard(' ');
    expect(keys[1]).toEqual({ code: 'Space', prevented: true });
    // Back at the zoom it left, with that point in the middle of the viewer's height; the move is under way until both
    // hold.
    await expect
      .poll(
        () => {
          const back = seen();
          const middle = back.d.top + under.y * back.zoom - (back.v.top + back.v.height / 2);
          return Math.abs(back.zoom - opened.zoom) < 0.001 && Math.abs(middle) < 1.5;
        },
        { timeout: 3000 },
      )
      .toBe(true);
  });

  it('works on a chart drawn again under a pointer that does not move', async () => {
    const page = await openFlowPage();
    await userEvent.hover(find('.viewer'));
    // The chart is drawn anew, as when it is laid out again: its viewer is new, and the pointer sends it no event.
    await page.unmount();
    await openFlowPage();
    expect(whole()).toBe(false);
    await userEvent.keyboard(' ');
    expect(keys[0]).toEqual({ code: 'Space', prevented: true });
    await expect.poll(whole, { timeout: 3000 }).toBe(true);
  });

  it('does nothing while the pointer is on what floats over the chart', async () => {
    await openFlowPage();
    const opened = seen();
    // The tools over the chart's top left corner are on the page above the viewer, not in it.
    await userEvent.hover(find('.flow-tools'));
    await userEvent.keyboard(' ');
    expect(keys[0]).toEqual({ code: 'Space', prevented: false });
    await settle();
    expect(unmoved(opened)).toBe(true);
  });

  it('leaves Space to a box the owner types in', async () => {
    await openFlowPage();
    const opened = seen();
    await userEvent.hover(find('.viewer'));
    const box = document.createElement('input');
    // Where it takes no room and focusing it scrolls nothing.
    box.style.position = 'fixed';
    box.style.bottom = '0';
    document.body.append(box);
    try {
      box.focus();
      await userEvent.keyboard(' ');
      expect(keys[0]).toEqual({ code: 'Space', prevented: false });
      expect(box.value).toBe(' ');
      await settle();
      expect(unmoved(opened)).toBe(true);
    } finally {
      box.remove();
    }
  });
});
