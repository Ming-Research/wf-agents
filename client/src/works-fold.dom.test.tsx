// @vitest-environment happy-dom
// A project's group in the works list, folded and unfolded by its name, counted in a DOM: the click on the name hides
// the group's table and a second click shows it again, the buttons that show or hide its works stay in the head, the
// other groups are left alone, and the browser keeps which groups are folded.

import './dom-setup';

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { WorkSummary } from './api';
import { Works } from './pages/Works';

const read = vi.hoisted(() => ({ works: [] as WorkSummary[] }));

// The engine's state is not known here; the works are what the page has read.
vi.mock('./live', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./live')>()),
  useStatus: () => null,
  useResource: (_key: string | null, path: string | null) => ({
    data: path === '/api/v1/works' ? read.works : undefined,
    error: null,
    stale: false,
  }),
  useShared: () => undefined,
}));

const work = (id: number, project: string): WorkSummary => ({
  id,
  item: id,
  title: `Item ${id}`,
  project,
  graph: 'engineer',
  flow_version: 3,
  moved_from: null,
  at: 'D1',
  at_title: 'Design',
  stage: 'design',
  state: 'ready',
  paused: null,
  paused_by: null,
  ended: null,
  waits: null,
  branch: null,
  pr: null,
  started_at: 0,
  last_at: 0,
  ran_ms: 0,
  time: { from: 0, until: 0, model_ms: 0, tool_ms: 0, engine_ms: 0, bar: '' },
  rounds: 1,
  sent_back: 0,
  failed: 0,
  running: null,
  cost_usd: 0,
  open_asks: 0,
  open_design_cards: 0,
});

let root: Root | null = null;
let host: HTMLElement | null = null;

/// Draws the works list in the document, inside `act`.
async function show(): Promise<HTMLElement> {
  host = document.createElement('div');
  document.body.append(host);
  const mine = createRoot(host);
  root = mine;
  await act(async () => mine.render(<Works work={null} />));
  return host;
}

const click = (el: Element) => act(async () => (el as HTMLElement).click());

/// The button that folds a project's group, by the project's name.
function folder(within: Element, project: string): HTMLButtonElement {
  const found = [...within.querySelectorAll('h2 button')].find((b) => b.textContent === project);
  if (found === undefined) throw new Error(`no fold button of ${project}`);
  return found as HTMLButtonElement;
}

/// Whether a project's group shows its table.
const unfolded = (within: Element, project: string) =>
  within.querySelector(`#works-body-${project} table`) !== null && folder(within, project).getAttribute('aria-expanded') === 'true';

beforeEach(() => {
  window.localStorage.clear();
  read.works = [work(2, 'p'), work(1, 'q')];
});

afterEach(async () => {
  await act(async () => root?.unmount());
  host?.remove();
  root = null;
  host = null;
  window.localStorage.clear();
});

describe("a project's group in the works list", () => {
  it('folds and unfolds by its name, leaving the other groups and its own buttons alone, and the browser keeps it', async () => {
    const page = await show();
    expect(unfolded(page,'p') && unfolded(page,'q')).toBe(true);
    await click(folder(page, 'p'));
    // Its table is gone, its work with it, and its buttons stay in its head; the other group is as it was.
    expect(unfolded(page,'p')).toBe(false);
    expect(folder(page, 'p').getAttribute('aria-expanded')).toBe('false');
    expect(page.querySelector('#works-body-p')).toBeNull();
    expect(page.querySelector('.project-head-controls .tree-state')).not.toBeNull();
    expect(unfolded(page,'q')).toBe(true);
    expect(page.textContent).not.toContain('Item 2');
    expect(page.textContent).toContain('Item 1');
    expect(window.localStorage.getItem('wf-agents.works-folded')).toBe('["p"]');
    // A second click shows it again.
    await click(folder(page, 'p'));
    expect(unfolded(page,'p')).toBe(true);
    expect(page.textContent).toContain('Item 2');
    expect(window.localStorage.getItem('wf-agents.works-folded')).toBe('[]');
  });

  it('is drawn folded again when the browser kept it so', async () => {
    window.localStorage.setItem('wf-agents.works-folded', '["q"]');
    const page = await show();
    expect(unfolded(page,'p')).toBe(true);
    expect(unfolded(page,'q')).toBe(false);
  });
});
