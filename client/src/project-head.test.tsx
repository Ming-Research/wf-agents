// A project's head, the one part every view that groups its rows by project draws it with: the project's name at the
// left and the controls over its rows at the right end of the same row, the two siblings in one container, the controls
// not in the heading. The list of conversations, the works, the items' tree and the observations all use it, with no
// row of buttons of their own under a name.

import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { Conversation, ItemState, ItemSummary, Observation, ProjectObservations, WorkSummary } from './api';
import { Items } from './pages/Items';
import { List } from './pages/Messages';
import { Observations } from './pages/Observations';
import { Works } from './pages/Works';
import { ProjectHead } from './parts';

const item = (id: number, project: string, state: ItemState): ItemSummary => ({
  id,
  project,
  title: `Item ${id}`,
  deliver: 'change',
  level: 'l',
  route: 'normal',
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
  blocked_by: [],
});

const items = {
  items: [item(1, 'p', 'working'), item(2, 'p', 'done'), item(3, 'q', 'new')],
  links: [],
};

const observation = (id: number, tags: string[]): Observation => ({
  id,
  title: `Observation ${id}`,
  what: 'What.',
  tags,
  source: '',
  times_reported: 1,
  refs: [],
  folded_into: null,
  closed: null,
  closed_why: null,
  created: 1_700_000_000_000,
  updated: null,
});

const observations: ProjectObservations[] = [
  { project: 'p', tags: [{ name: 'ui', desc: 'What the owner sees.' }], observations: [observation(1, ['ui']), observation(2, [])] },
  { project: 'q', tags: [], observations: [observation(3, [])] },
];

const work = (id: number, project: string, state: WorkSummary['state']): WorkSummary => ({
  id,
  item: id,
  title: `Item ${id}`,
  project,
  graph: 'engineer',
  flow_version: 1,
  moved_from: null,
  at: 'I1',
  at_title: 'Implement',
  stage: 'implement',
  state,
  paused: null,
  paused_by: null,
  ended: null,
  waits: null,
  branch: null,
  pr: null,
  started_at: id,
  last_at: id,
  ran_ms: 1,
  rounds: 1,
  sent_back: 0,
  failed: 0,
  running: null,
  cost_usd: 1,
  open_asks: 0,
  open_design_cards: 0,
});

const works = [work(1, 'p', 'running'), work(2, 'p', 'waiting'), work(3, 'q', 'paused')];

vi.mock('./live', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./live')>()),
  useStatus: () => null,
  useResource: (_key: string | null, path: string | null) => ({
    data:
      path === '/api/v1/items'
        ? items
        : path === '/api/v1/observations'
          ? observations
          : path === '/api/v1/works'
            ? works
            : undefined,
    error: null,
  }),
  useShared: () => undefined,
}));

afterEach(() => {
  vi.unstubAllGlobals();
});

/// What is inside the `div` that opens just before `from`, found by counting the `div`s opened and closed after it.
function innerOf(html: string, from: number): string {
  let depth = 1;
  const tag = /<(\/?)div\b[^>]*>/g;
  tag.lastIndex = from;
  for (let m = tag.exec(html); m !== null; m = tag.exec(html)) {
    depth += m[1] === '' ? 1 : -1;
    if (depth === 0) return html.slice(from, m.index);
  }
  return html.slice(from);
}

/// The heads of the projects in a view's markup, each its name and the markup of its controls (none for a head with
/// none): the heading must be the name alone, closed before the container of the controls opens, the two siblings of
/// the head's container.
function heads(html: string): Array<{ name: string; controls: string | null }> {
  const out: Array<{ name: string; controls: string | null }> = [];
  const open = '<div class="project-head">';
  for (let at = html.indexOf(open); at >= 0; at = html.indexOf(open, at + open.length)) {
    const from = at + open.length;
    const m = /^<h2[^>]*>([^<]*)<\/h2>/.exec(html.slice(from));
    expect(m, 'the name is a heading of text alone, first in the head').not.toBeNull();
    const after = from + (m?.[0].length ?? 0);
    const container = '<div class="project-head-controls">';
    const controls = html.startsWith(container, after) ? innerOf(html, after + container.length) : null;
    out.push({ name: m?.[1] ?? '', controls });
    // The head's container closes right after its controls, or else its heading: nothing else is in it.
    const end = controls === null ? after : after + container.length + controls.length + '</div>'.length;
    expect(html.startsWith('</div>', end), 'nothing else in the head').toBe(true);
  }
  return out;
}

/// How many of a view's state buttons (the part the project's rows are shown or hidden or picked by) the markup has.
const stateButtons = (html: string) => (html.match(/<button[^>]*class="[^"]*\btree-state\b/g) ?? []).length;

/// What a view must show of its project heads: a head for each project, by name, every state button of the view inside
/// a head's controls and none anywhere else, and none of the rows of buttons the views had under a name.
function expectHeads(html: string, names: string[]) {
  const found = heads(html);
  expect(found.map((h) => h.name)).toEqual(names);
  expect(found.every((h) => h.controls !== null && stateButtons(h.controls) > 0)).toBe(true);
  expect(stateButtons(html)).toBe(found.reduce((n, h) => n + stateButtons(h.controls ?? ''), 0));
  for (const gone of ['talks-states', 'obs-filters', 'tree-head']) expect(html).not.toContain(gone);
}

describe('a project head', () => {
  it('is the name and the controls, two siblings of one container, the name first and the controls not in its heading', () => {
    const html = renderToStaticMarkup(
      <ProjectHead id="h" name="wf-agents">
        <button type="button">one</button>
      </ProjectHead>,
    );
    // The heading holds the name alone and has the id the group is labelled by; the controls follow it in the container.
    expect(html).toMatch(
      /^<div class="project-head"><h2 (?=[^>]*\bid="h")(?=[^>]*class="[^"]*project-head-name)[^>]*>wf-agents<\/h2><div class="project-head-controls"><button type="button">one<\/button><\/div><\/div>$/,
    );
    expect(heads(html)).toEqual([{ name: 'wf-agents', controls: '<button type="button">one</button>' }]);
  });

  it('is the name alone for a project with no control', () => {
    for (const none of [undefined, null, false]) {
      const html = renderToStaticMarkup(<ProjectHead name="p">{none}</ProjectHead>);
      expect(html).not.toContain('project-head-controls');
      expect(heads(html)).toEqual([{ name: 'p', controls: null }]);
    }
  });

  it('heads each project of the list of conversations, the works, the items and the observations, buttons in the head', () => {
    const talk = (key: string, extra: Partial<Conversation>): Conversation => ({
      key,
      kind: 'work',
      project: null,
      projects: [],
      work: null,
      item: null,
      title: null,
      level: null,
      over: false,
      waiting: 0,
      unread: 0,
      last_at: 1,
      ...extra,
    });
    const list = [
      talk('architect:p', { kind: 'architect', project: 'p' }),
      talk('architect:q', { kind: 'architect', project: 'q' }),
      ...works.map((w) => talk(`work:${w.id}`, { project: w.project, work: w.id, item: w.id, title: w.title })),
    ];
    expectHeads(renderToStaticMarkup(<List list={list} open={null} works={works} />), ['p', 'q']);
    expectHeads(renderToStaticMarkup(<Works work={null} />), ['p', 'q']);
    // No item shown, so that no item's page is drawn beside the tree; the buttons are drawn all the same.
    vi.stubGlobal('window', {
      localStorage: { getItem: (k: string) => (k === 'wf-agents.items-states' ? '[]' : null), setItem: () => undefined },
    });
    expectHeads(renderToStaticMarkup(<Items selected={null} />), ['p', 'q']);
    vi.unstubAllGlobals();
    expectHeads(renderToStaticMarkup(<Observations route={{ page: 'observations', project: null, open: null }} />), ['p', 'q']);
  });
});
