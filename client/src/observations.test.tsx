// The Observations view: each project's observations under its name, with the buttons that pick them by its tags and
// by state, nothing picked at first so that every one shows, and the clear of every pick while something is picked; a
// row for each, one block parted from the next by a line, which opens it beside the list; the one open in full, where
// the owner closes it, saying why, in a box of the page's own rather than the browser's prompt. While none is open the
// list fills the view.

import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { Observation, ProjectObservations } from './api';
import { filters, handled, NO_TAG, type Picks, rowLine, shownObservations, stateOf, tagsOf } from './observations';
import { Observations } from './pages/Observations';

const observation = (id: number, extra: Partial<Observation> = {}): Observation => ({
  id,
  title: null,
  what: `Observation ${id} says **this**.\n\nAnd more after it.`,
  tags: [],
  source: 'The review of #4 found it.',
  times_reported: 1,
  refs: [],
  folded_into: null,
  closed: null,
  closed_why: null,
  created: 1_700_000_000_000 + id,
  updated: null,
  ...extra,
});

const data: ProjectObservations[] = [
  {
    project: 'p',
    tags: [
      { name: 'ui', desc: 'What the owner sees.' },
      { name: 'perf', desc: 'How fast it is.' },
    ],
    observations: [
      observation(5, { title: 'Stale slot count', tags: ['ui', 'perf'], times_reported: 3 }),
      observation(4, { tags: ['perf'] }),
      observation(3, { tags: ['gone'] }),
      observation(2, { tags: ['ui'], folded_into: 9 }),
      observation(1, { tags: ['ui'], closed: 'outdated', closed_why: 'Gone with #7.' }),
    ],
  },
  { project: 'q', tags: [], observations: [observation(6)] },
];

vi.mock('./live', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./live')>()),
  useResource: () => ({ data, error: null }),
  useShared: () => undefined,
}));

const ids = (os: Observation[]) => os.map((o) => o.id);

const none: Picks = { tags: [], states: [] };

describe("a project's observations", () => {
  const all = data[0].observations;
  const tags = ['ui', 'perf'];

  it('are handled once an item took them in or they were closed, and open otherwise', () => {
    expect(all.map(handled)).toEqual([false, false, false, true, true]);
    expect(all.map(stateOf)).toEqual(['open', 'open', 'open', 'handled', 'handled']);
  });

  it('lead their rows with their titles, and one recorded before titles with what it is', () => {
    expect(rowLine(all[0])).toBe('Stale slot count');
    expect(rowLine(all[1])).toBe(all[1].what);
  });

  it("are sorted by the project's tags, those carrying none of them apart", () => {
    expect(tagsOf(all[0], tags)).toEqual(['ui', 'perf']);
    expect(tagsOf(all[2], tags)).toEqual([NO_TAG]);
    expect(tagsOf(data[1].observations[0], [])).toEqual([NO_TAG]);
  });

  it('all show while nothing is picked, the handled ones among them', () => {
    expect(ids(shownObservations(all, tags, none, null))).toEqual([5, 4, 3, 2, 1]);
  });

  it('show, once picked, those that match any pick of a group and every group picked in, and the one open always', () => {
    // Within the tags, any of those picked.
    expect(ids(shownObservations(all, tags, { tags: ['perf'], states: [] }, null))).toEqual([5, 4]);
    expect(ids(shownObservations(all, tags, { tags: ['perf', NO_TAG], states: [] }, null))).toEqual([5, 4, 3]);
    // Open or handled, either alone.
    expect(ids(shownObservations(all, tags, { tags: [], states: ['open'] }, null))).toEqual([5, 4, 3]);
    expect(ids(shownObservations(all, tags, { tags: [], states: ['handled'] }, null))).toEqual([2, 1]);
    expect(ids(shownObservations(all, tags, { tags: [], states: ['open', 'handled'] }, null))).toEqual([5, 4, 3, 2, 1]);
    // Across the groups, each.
    expect(ids(shownObservations(all, tags, { tags: ['ui'], states: ['open'] }, null))).toEqual([5]);
    expect(ids(shownObservations(all, tags, { tags: ['ui'], states: ['handled'] }, null))).toEqual([2, 1]);
    // The one open stays whatever is picked.
    expect(ids(shownObservations(all, tags, { tags: ['perf'], states: [] }, 1))).toEqual([5, 4, 1]);
  });

  it('count on each button what it would show under the picks of the other group', () => {
    const f = filters(all, tags, none);
    expect(f.tags.order).toEqual(['ui', 'perf', NO_TAG]);
    expect(f.tags.counts).toEqual(['ui', 'perf', 'perf', NO_TAG, 'ui', 'ui']);
    expect(f.states.order).toEqual(['open', 'handled']);
    expect(f.states.counts).toEqual(['open', 'open', 'open', 'handled', 'handled']);
    // A state picked narrows the tags' counts and leaves its own group's as they were.
    const open = filters(all, tags, { tags: [], states: ['open'] });
    expect(open.tags.counts).toEqual(['ui', 'perf', 'perf', NO_TAG]);
    expect(open.states.counts).toEqual(f.states.counts);
    // A tag picked narrows the states' counts and leaves the tags' as they were.
    const ui = filters(all, tags, { tags: ['ui'], states: [] });
    expect(ui.states.counts).toEqual(['open', 'handled', 'handled']);
    expect(ui.tags.counts).toEqual(f.tags.counts);
  });

  it('keep a button for each pick, though the project has nothing it matches', () => {
    const q = data[1].observations;
    const f = filters(q, [], { tags: ['ui'], states: ['handled'] });
    expect(f.tags.order).toEqual(['ui', NO_TAG]);
    expect(f.tags.counts).toEqual([]);
    expect(f.states.order).toEqual(['open', 'handled']);
    expect(f.states.counts).toEqual([]);
    // With nothing picked, only what the project has.
    expect(filters(q, [], none).states.order).toEqual(['open']);
  });
});

describe('the Observations view', () => {
  // Nothing picked, unless a test keeps picks: the view reads the picks again once what the browser keeps changes.
  beforeEach(() => {
    kept([], []);
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  /// The picks this browser keeps, as the view reads them.
  const kept = (tags: string[], states: string[]) =>
    vi.stubGlobal('window', {
      localStorage: {
        getItem: (k: string) =>
          k === 'wf-agents.observations-picked-tags'
            ? JSON.stringify(tags)
            : k === 'wf-agents.observations-picked-states'
              ? JSON.stringify(states)
              : null,
        setItem: () => undefined,
      },
    });
  const view = (project: string | null, open: number | null) =>
    renderToStaticMarkup(<Observations route={{ page: 'observations', project, open }} />);
  const rows = (html: string) => [...html.matchAll(/class="obs-what" title="([^"]+)"/g)].map((m) => m[1]);
  const buttons = (html: string) =>
    [...html.matchAll(/<button([^>]*)>([\s\S]*?)<\/button>/g)].map((b) => ({
      text: b[2].replace(/<[^>]*>/g, '').replace(/\s+/g, ' '),
      pressed: /aria-pressed="(\w+)"/.exec(b[1])?.[1],
      title: /title="([^"]+)"/.exec(b[1])?.[1],
    }));
  /// The part of the view that is a project's: from its name to the next project's.
  const of = (html: string, project: string) => {
    const name = `>${project}</h3>`;
    const at = html.indexOf(name);
    const next = html.indexOf('</h3>', at + name.length);
    return html.slice(at, next < 0 ? undefined : next);
  };

  it('fills the view with every observation of every project while nothing is picked, the handled muted', () => {
    const html = view(null, null);
    expect(html).toContain('list-split list-split-whole');
    expect([...html.matchAll(/<h3[^>]*>([^<]+)<\/h3>/g)].map((m) => m[1])).toEqual(['p', 'q']);
    // A row leads with its title, and one with none with the words its text starts with.
    expect(rows(html)).toEqual([
      'Stale slot count',
      'Observation 4 says this.',
      'Observation 3 says this.',
      'Observation 2 says this.',
      'Observation 1 says this.',
      'Observation 6 says this.',
    ]);
    // A handled row is muted in place.
    expect([...html.matchAll(/class="[^"]*obs-row[^"]*"/g)].map((m) => m[0].includes('obs-handled'))).toEqual([
      false,
      false,
      false,
      true,
      true,
      false,
    ]);
    // Each button is its words and its count, in the neutral tone, none picked; the tags, then open and handled.
    const p = buttons(of(html, 'p'));
    // Each button's text is its words and its count, with the copy of it Primer's counter keeps for a screen reader.
    expect(p.map((b) => [b.text, b.pressed])).toEqual([
      ['ui3 (3)', 'false'],
      ['perf2 (2)', 'false'],
      ['no tag1 (1)', 'false'],
      ['open3 (3)', 'false'],
      ['handled2 (2)', 'false'],
    ]);
    expect(p[0].title).toBe('3 observations are tagged ui. Click to show only what it matches. What the owner sees.');
    expect(p[2].title).toBe('1 observation carries none of the project&#x27;s tags. Click to show only what it matches.');
    expect(p[4].title).toBe('2 observations were taken in by an item or closed. Click to show only what it matches.');
    expect(html).toContain('tree-state tree-tone-neutral');
    // Nothing to clear.
    expect(html).not.toContain('Clear filters');
    // A row's line under its words: its tags, then how many times it was reported, when, and where it came from,
    // whole on hovering.
    expect(html).toContain('reported 3 times');
    expect(html).toContain('title="ui, perf | reported 3 times | recorded');
    expect(html).toContain('The review of #4 found it.');
    expect(html).not.toContain('href="/items/4"');
  });

  it('shows what matches the picks, each button counting under the other picks, with the clear of every pick', () => {
    kept(['perf'], ['open']);
    const html = view(null, null);
    expect(rows(html)).toEqual(['Stale slot count', 'Observation 4 says this.']);
    const p = buttons(of(html, 'p'));
    expect(p.map((b) => [b.text, b.pressed])).toEqual([
      ['ui1 (1)', 'false'],
      ['perf2 (2)', 'true'],
      ['no tag1 (1)', 'false'],
      ['open2 (2)', 'true'],
      ['handled0 (0)', 'false'],
      ['Clear filters', undefined],
    ]);
    expect(p[0].title).toBe('1 observation is tagged ui. Click to also show what it matches. What the owner sees.');
    expect(p[1].title).toBe('2 observations are tagged perf. Click to take this pick back. How fast it is.');
    expect(p[5].title).toBe('Take back every pick and show every observation.');
    // A project with nothing that matches keeps the buttons of the picks, to take them back where it shows.
    const q = of(html, 'q');
    expect(buttons(q).map((b) => [b.text, b.pressed])).toEqual([
      ['perf0 (0)', 'true'],
      ['no tag1 (1)', 'false'],
      ['open0 (0)', 'true'],
      ['Clear filters', undefined],
    ]);
    expect(q).toContain('No observation matches what is picked.');
  });

  it("shows one project's alone when the address names it, with the way back to every project's", () => {
    const html = view('q', null);
    expect(rows(html)).toEqual(['Observation 6 says this.']);
    expect(html).toContain("q&#x27;s observations only.");
    expect(html).toContain('href="/observations"');
  });

  it("opens one beside the list, whole, with the owner's close folded and no prompt of the browser", () => {
    const html = view(null, 5);
    expect(html).not.toContain('list-split-whole');
    expect(html).toContain('aria-current="page"');
    expect(html).toContain('Observation 5</h1>');
    expect(html).toContain('And more after it.');
    expect(html).toContain('Where it came from');
    expect(html).toContain('Close it');
    expect(html).toContain('placeholder="Why close it?"');
  });

  it('leads from one an item took in to the item, says why one was closed, and offers no close of one closed', () => {
    const taken = view(null, 2);
    expect(taken).toContain('Taken in by');
    expect(taken).toContain('href="/items/9"');
    expect(taken).toContain('Close it');
    // The one open stays in its list though the picks leave it out.
    kept([], ['open']);
    expect(rows(view(null, 2))).toContain('Observation 2 says this.');
    const closed = view(null, 1);
    expect(closed).toContain('Closed: outdated');
    expect(closed).toContain('Gone with');
    expect(closed).not.toContain('Close it');
  });
});
