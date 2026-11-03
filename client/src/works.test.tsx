// The works view: one table with a group of rows for each project, in the order the list of conversations has its
// projects. A group is the project's head (its name and the buttons that show or hide its works by state), then its
// architect, which opens the architect's page, then its works, newest first. The buttons are the list of conversations'
// own, with its one choice (done and ended hidden at first), and count by the state a work is in effect in.

import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { ArchitectStanding, ArchitectWork, Conversation, Matter, Status, WorkState, WorkSummary } from './api';
import { WORK_STATES } from './components';
import { cost, duration, time } from './format';
import { List } from './pages/Messages';
import { ArchitectRow, Works } from './pages/Works';

// What the page has read, which each test sets: the engine's status, the works, and how every architect stands.
const read = vi.hoisted(() => ({
  status: null as Status | null,
  works: [] as WorkSummary[],
  architects: undefined as ArchitectStanding[] | undefined,
  navigated: [] as string[],
}));

vi.mock('./live', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./live')>()),
  useStatus: () => read.status,
  useResource: (_key: string | null, path: string | null) => ({
    data: path === '/api/v1/works' ? read.works : undefined,
    error: null,
  }),
  useShared: (_key: string, path: string) => (path === '/api/v1/architects' ? read.architects : undefined),
}));

vi.mock('./route', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./route')>()),
  navigate: (to: string) => {
    read.navigated.push(to);
  },
}));

const status = (extra: Partial<Status> = {}): Status =>
  ({
    paused: false,
    running: [],
    started: [],
    releasing: [],
    waiting_for_slot: [],
    stepping: [],
    architects_started: [],
    flow_version: 2,
    halt: null,
    repos: { p: 'o/p', q: 'o/q' },
    ...extra,
  }) as unknown as Status;

const work = (id: number, extra: Partial<WorkSummary> = {}): WorkSummary => ({
  id,
  item: id,
  title: `Item ${id}`,
  project: 'p',
  graph: 'engineer',
  flow_version: 2,
  moved_from: null,
  at: 'I1',
  at_title: 'Implement',
  stage: 'implement',
  state: 'running',
  paused: null,
  paused_by: null,
  ended: null,
  waits: null,
  branch: null,
  pr: null,
  started_at: id * 100,
  last_at: id * 100 + 50,
  ran_ms: 1000,
  rounds: 3,
  sent_back: 0,
  failed: 0,
  running: null,
  cost_usd: 1,
  open_asks: 0,
  open_design_cards: 0,
  ...extra,
});

const architectWork = (id: number, project: string, extra: Partial<WorkSummary> = {}, listens = false): ArchitectWork => ({
  work: work(id, { item: null, title: null, graph: 'architect', project, at: 'Q0', at_title: 'Look at the queue', ...extra }),
  handling: null,
  listens,
});

const matter = (id: number): Matter => ({ id, kind: 'owner_words', asked: null, answered: false }) as unknown as Matter;

/// How an architect stands: its works, newest first, its queue and whether a question of its own waits.
const standing = (
  project: string,
  its: ArchitectWork[] = [],
  extra: Partial<ArchitectStanding> = {},
): ArchitectStanding => ({ project, works: its, queue: [], asking: false, ...extra });

// Project p: work 9 the engine runs, 8 done, 7 ended, 6 waiting. Project q: work 4 paused, and work 3 recorded as
// running while the engine runs nothing of it just now: it runs as it is in effect, the engine taking its next step,
// which closes that round, and is ready where the engine cannot take it up (halted).
const works = [
  work(3, { project: 'q', state: 'running' }),
  work(4, { project: 'q', state: 'paused', paused: 'the owner paused it', paused_by: 'owner' }),
  work(6, { state: 'waiting' }),
  work(7, { state: 'ended' }),
  work(8, { state: 'done', at: 'E1', at_title: 'Done' }),
  work(9, { state: 'running' }),
];

// The architect of p has two works, one waiting for the owner's next words and an older, done one.
const listening = architectWork(20, 'p', { state: 'waiting', rounds: 4, ran_ms: 2000, cost_usd: 1.5, last_at: 2050 }, true);
const older = architectWork(18, 'p', { state: 'done', at: 'E2', rounds: 6, ran_ms: 1000, cost_usd: 0.25, last_at: 1850 });

/// The text of each state's hidden choice this browser keeps, as the page reads it: the states hidden.
const kept = (hidden: readonly WorkState[]) =>
  vi.stubGlobal('window', {
    localStorage: {
      getItem: (k: string) => (k === 'wf-agents.talks-hidden-states' ? JSON.stringify(hidden) : null),
      setItem: () => undefined,
    },
  });

beforeEach(() => {
  read.status = status({ running: [9] });
  read.works = works;
  read.architects = [standing('p', [listening, older]), standing('q')];
  read.navigated = [];
  kept(['done', 'ended']);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

const view = () => renderToStaticMarkup(<Works work={null} />);

/// The groups of the table, one for each project: from its `tbody` to the next.
const groups = (html: string) => html.split('<tbody').slice(1);

/// The first cell of each row of a group: empty for the project's head, "Architect" for its architect, "W9" for a work.
const firsts = (group: string) => [...group.matchAll(/<tr[^>]*>(?:<td[^>]*>)([^<]*)/g)].map((m) => m[1]);

/// The buttons of a group, each its name and whether it is pressed.
const buttons = (group: string) =>
  [...group.matchAll(/<button[^>]*aria-pressed="(true|false)"[^>]*title="([^"]+)"/g)].map((m) => [m[2], m[1]]);

/// The works a view lists, by number, in its order.
const listed = (html: string) => [...html.matchAll(/<td>W(\d+)<\/td>/g)].map((m) => Number(m[1]));

/// A group's architect row, and its cells with their markup taken out.
function architectRow(group: string): { row: string; cells: string[] } {
  const row = group.split('<tr ').find((r) => r.includes('works-architect')) ?? '';
  return { row, cells: [...row.matchAll(/<td[^>]*>([\s\S]*?)<\/td>/g)].map((m) => m[1].replace(/<[^>]*>/g, '')) };
}

describe('the works table', () => {
  it('is one table with a group for each project, by name: its head, its architect, then its works, newest first', () => {
    const html = view();
    expect((html.match(/<table/g) ?? []).length).toBe(1);
    expect(html).not.toContain('Running and waiting');
    expect(html).not.toContain('<details');
    const [p, q] = groups(html);
    expect(groups(html)).toHaveLength(2);
    expect([...p.matchAll(/<h2[^>]*>([^<]+)<\/h2>/g)].map((m) => m[1])).toEqual(['p']);
    expect([...q.matchAll(/<h2[^>]*>([^<]+)<\/h2>/g)].map((m) => m[1])).toEqual(['q']);
    // Done 8 and ended 7 are hidden at first; the rest are in the order of their numbers, newest first.
    expect(firsts(p)).toEqual(['', 'Architect', 'W9', 'W6']);
    expect(firsts(q)).toEqual(['', 'Architect', 'W4', 'W3']);
  });

  it('orders the projects by name whatever order they are read in, and keeps a project whose architect is not read yet', () => {
    read.architects = [standing('q'), standing('p')];
    expect(groups(view()).map((g) => /<h2[^>]*>([^<]+)<\/h2>/.exec(g)?.[1])).toEqual(['p', 'q']);
    // Before the architects are read, the groups are the works' projects, with no architect row yet.
    read.architects = undefined;
    const [p, q] = groups(view());
    expect(firsts(p)).toEqual(['', 'W9', 'W6']);
    expect(firsts(q)).toEqual(['', 'W4', 'W3']);
    // A project with an architect and no work still has its group, with the architect alone and no button.
    read.works = [];
    read.architects = [standing('p')];
    const [only] = groups(view());
    expect(firsts(only)).toEqual(['', 'Architect']);
    expect(buttons(only)).toEqual([]);
  });

  it("does not say a work's project after its item, which the group it is in says", () => {
    const html = view();
    expect(html).not.toMatch(/<span class="muted"> \| (p|q)<\/span>/);
    expect(html).toContain('title="#9 Item 9\nOpen the item"');
  });

  it("shows the works the buttons show, and the buttons count the project's works in every state, hidden or not", () => {
    const [p, q] = groups(view());
    expect(buttons(p)).toEqual([
      ['1 work is running. Click to hide it.', 'true'],
      ['1 work is waiting. Click to hide it.', 'true'],
      ['1 work is done. Click to show it.', 'false'],
      ['1 work is ended. Click to show it.', 'false'],
    ]);
    expect(buttons(q)).toEqual([
      ['1 work is running. Click to hide it.', 'true'],
      ['1 work is paused. Click to hide it.', 'true'],
    ]);
    // Showing done and ended shows their rows, the buttons pressed; hiding running hides its row, newest first throughout.
    kept([]);
    expect(listed(view())).toEqual([9, 8, 7, 6, 4, 3]);
    expect(buttons(groups(view())[0]).map((b) => b[1])).toEqual(['true', 'true', 'true', 'true']);
    kept(['running', 'paused']);
    expect(listed(view())).toEqual([8, 7, 6]);
    expect(buttons(groups(view())[0]).map((b) => b[1])).toEqual(['false', 'true', 'true', 'true']);
  });

  it("is the list of conversations' choice: the same works show in both, whichever states the browser keeps hidden", () => {
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
      ...works.map((w) => talk(`work:${w.id}`, { project: w.project, work: w.id, item: w.id, title: `Item ${w.id}` })),
    ];
    const shownInList = () =>
      [...renderToStaticMarkup(<List list={list} open={null} works={works} />).matchAll(/href="\/messages\?c=work%3A(\d+)"/g)]
        .map((m) => Number(m[1]))
        .sort((a, b) => b - a);
    for (const hidden of [['done', 'ended'], [], ['running', 'paused', 'ready'], WORK_STATES] as WorkState[][]) {
      kept(hidden);
      expect(listed(view()), hidden.join()).toEqual(shownInList());
    }
    kept(['done', 'ended']);
    expect(shownInList()).toEqual([9, 6, 4, 3]);
  });

  it('counts a work recorded as running that the engine cannot take up as ready: in its row and in both lists of buttons', () => {
    // The engine has halted every work: no round starts, so the round the work's last step left open is not run.
    read.status = status({ running: [9], halt: { kind: 'offline', until: 1, why: 'no network' } });
    const html = view();
    const q = groups(html)[1];
    // Its row says ready, not running, and its project's button counts it so.
    const row = q.split('<tr ').find((r) => r.includes('<td>W3</td>')) ?? '';
    expect(row).toContain('ready');
    expect(row).not.toContain('running');
    expect(buttons(q).map((b) => b[0])).toContain('1 work is ready. Click to hide it.');
    expect(buttons(q).map((b) => b[0]).join()).not.toContain('running');
    // The list of conversations counts it the same.
    const talk = (key: string, kind: Conversation['kind'], project: string, w: number | null): Conversation => ({
      key,
      kind,
      project,
      projects: [],
      work: w,
      item: w,
      title: w === null ? null : `Item ${w}`,
      level: null,
      over: false,
      waiting: 0,
      unread: 0,
      last_at: 1,
    });
    const list = [talk('architect:q', 'architect', 'q', null), talk('work:4', 'work', 'q', 4), talk('work:3', 'work', 'q', 3)];
    const messages = renderToStaticMarkup(<List list={list} open={null} works={works} />);
    const heads = messages.split('project-head-item').slice(1);
    expect(buttons(heads[0]).map((b) => b[0])).toEqual([
      '1 work is ready. Click to hide it.',
      '1 work is paused. Click to hide it.',
    ]);
  });
});

describe('the works table while every work is paused', () => {
  it('counts a work Pause all holds as paused, in its row and in the buttons, as its words say', () => {
    read.status = status({ paused: true, running: [] });
    const [p, q] = groups(view());
    // Work 9, recorded as running with no round running, and work 6, which waits, are held by Pause all.
    expect(buttons(p).map((b) => b[0])).toEqual([
      '2 works are paused. Click to hide them.',
      '1 work is done. Click to show it.',
      '1 work is ended. Click to show it.',
    ]);
    expect(buttons(q).map((b) => b[0])).toEqual(['2 works are paused. Click to hide them.']);
    const row = p.split('<tr ').find((r) => r.includes('<td>W9</td>')) ?? '';
    expect(row).toContain('paused by Pause all');
  });
});

describe("a project's architect in the works table", () => {
  it("is a row in the work's columns, summed over all its works, and not counted by the buttons", () => {
    const p = groups(view())[0];
    const { row, cells } = architectRow(p);
    expect(cells).toHaveLength(11);
    expect(cells[0]).toBe('Architect');
    expect(cells[1]).toBe('Architect of p');
    expect(cells[2]).toBe('waiting for your next words');
    expect(cells[3]).toBe('Conversation');
    // At the cell of the work that goes on; no stage; the sums of its works' rounds, run time and cost, and the latest
    // of their last activity; no pull request.
    expect(cells[4]).toContain('Q0');
    expect(cells.slice(5)).toEqual(['', '10', duration(3000), time(2050), cost(1.75), '']);
    // It leads to its page and to its one-to-one.
    expect(row).toContain('href="/works/architect?project=p"');
    expect(row).toContain('href="/messages?c=architect%3Ap"');
    // The buttons of the project count its works alone: none of its 2 architect works is among them.
    expect(buttons(p).map((b) => b[0])).toEqual([
      '1 work is running. Click to hide it.',
      '1 work is waiting. Click to hide it.',
      '1 work is done. Click to show it.',
      '1 work is ended. Click to show it.',
    ]);
  });

  it("opens the architect's page when the row is clicked, and the page it is the first row of is under the works", () => {
    const a = standing('p', [listening]);
    // The row is a function of its props and the mocked reads, so that it can be called and its handler pressed.
    const el = ArchitectRow({ a }) as unknown as { type: string; props: { onClick: () => void } };
    expect(el.type).toBe('tr');
    el.props.onClick();
    expect(read.navigated).toEqual(['/works/architect?project=p']);
  });

  it('is there whatever states the buttons hide, with no work row when all are hidden', () => {
    kept(WORK_STATES);
    const html = view();
    expect(listed(html)).toEqual([]);
    expect(groups(html).map(firsts)).toEqual([
      ['', 'Architect'],
      ['', 'Architect'],
    ]);
    expect(buttons(groups(html)[0]).every((b) => b[1] === 'false')).toBe(true);
  });

  it("says how it stands as its mark in the list of conversations does: waiting for the owner's words, held back, asking", () => {
    const talk = (key: string, extra: Partial<Conversation> = {}): Conversation => ({
      key,
      kind: 'architect',
      project: 'p',
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
    /// What the row says of the architect, and what its mark in the list of conversations says on hovering.
    const says = (): [string, string | undefined] => {
      const list = renderToStaticMarkup(<List list={[talk('architect:p')]} open={null} works={[]} />);
      return [architectRow(groups(view())[0]).cells[2], /class="state-mark[^"]*" title="([^"]+)"/.exec(list)?.[1]];
    };
    // Waiting for the owner's next words.
    read.architects = [standing('p', [listening])];
    expect(says()).toEqual(['waiting for your next words', 'waiting for your next words']);
    // Matters in its queue and every work paused, with no work of it going on.
    read.architects = [standing('p', [older], { queue: [matter(1)] })];
    read.status = status({ paused: true, running: [] });
    expect(says()).toEqual(['held back: every work is paused', 'held back: every work is paused']);
    // A question of its own waiting for the reply, while it neither runs nor is held back.
    read.architects = [standing('p', [listening], { asking: true })];
    read.status = status({ running: [] });
    expect(says()).toEqual(['asking', 'asking']);
    // A round of it running: the same on both.
    read.architects = [standing('p', [architectWork(20, 'p', { state: 'running' })], { asking: true })];
    read.status = status({ running: [20] });
    expect(says()[0]).toBe(says()[1]);
    expect(says()[0]).toBe('working');
  });
});
