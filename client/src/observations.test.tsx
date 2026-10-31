// The Observations view: each project's observations under its name, with the buttons that show or hide those with each
// of its tags and, off at first, those handled; a row for each, which opens it beside the list; the one open in full,
// where the owner closes it, saying why, in a box of the page's own rather than the browser's prompt. While none is
// open the list fills the view.

import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

import type { Observation, ProjectObservations } from './api';
import { handled, NO_TAG, rowLine, shownObservations, tagCounts, tagsOf } from './observations';
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

describe("a project's observations", () => {
  const all = data[0].observations;
  const tags = ['ui', 'perf'];

  it('are handled once an item took them in or they were closed', () => {
    expect(all.map(handled)).toEqual([false, false, false, true, true]);
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

  it('show those with a tag shown, any of their tags, the handled ones only when asked, and the one open always', () => {
    expect(ids(shownObservations(all, tags, [], false, null))).toEqual([5, 4, 3]);
    expect(ids(shownObservations(all, tags, [], true, null))).toEqual([5, 4, 3, 2, 1]);
    expect(ids(shownObservations(all, tags, ['perf'], false, null))).toEqual([5, 3]);
    expect(ids(shownObservations(all, tags, ['perf', 'ui', NO_TAG], true, null))).toEqual([]);
    expect(ids(shownObservations(all, tags, ['perf', 'ui', NO_TAG], false, 1))).toEqual([1]);
  });

  it('count each tag among those the handled button lets show, and the handled ones whatever their tags', () => {
    expect(tagCounts(all, tags, false)).toEqual({ tags: ['ui', 'perf', 'perf', NO_TAG], handled: 2 });
    expect(tagCounts(all, tags, true).tags).toEqual(['ui', 'perf', 'perf', NO_TAG, 'ui', 'ui']);
  });
});

describe('the Observations view', () => {
  const view = (project: string | null, open: number | null) =>
    renderToStaticMarkup(<Observations route={{ page: 'observations', project, open }} />);
  const rows = (html: string) => [...html.matchAll(/class="obs-what" title="([^"]+)"/g)].map((m) => m[1]);
  const buttons = (html: string) =>
    [...html.matchAll(/<button([^>]*)>([\s\S]*?)<\/button>/g)].map((b) => ({
      text: b[2].replace(/<[^>]*>/g, '').replace(/\s+/g, ' '),
      pressed: /aria-pressed="(\w+)"/.exec(b[1])?.[1],
      title: /title="([^"]+)"/.exec(b[1])?.[1],
    }));

  it('fills the view with every project, its tags and the handled button over its rows, the open ones at first', () => {
    const html = view(null, null);
    expect(html).toContain('list-split list-split-whole');
    expect([...html.matchAll(/<h3[^>]*>([^<]+)<\/h3>/g)].map((m) => m[1])).toEqual(['p', 'q']);
    // A row leads with its title, and one with none with the words its text starts with.
    expect(rows(html)).toEqual(['Stale slot count', 'Observation 4 says this.', 'Observation 3 says this.', 'Observation 6 says this.']);
    // A tag is its name and its count, in the neutral tone, the description of what it covers said on hovering.
    const p = buttons(html.slice(0, html.indexOf('>q<')));
    // Each button's text is its words and its count, with the copy of it Primer's counter keeps for a screen reader.
    expect(p.map((b) => [b.text, b.pressed])).toEqual([
      ['ui1 (1)', 'true'],
      ['perf2 (2)', 'true'],
      ['no tag1 (1)', 'true'],
      ['handled2 (2)', 'false'],
    ]);
    expect(p[0].title).toBe('1 observation is tagged ui. Click to hide it. What the owner sees.');
    expect(p[3].title).toBe('2 observations were taken in by an item or closed. Click to show them.');
    expect(html).toContain('tree-state tree-tone-neutral');
    // A row's line under its words: its tags, then how many times it was reported, when, and where it came from.
    expect(html).toContain('reported 3 times');
    expect(html).toContain('The review of #4 found it.');
    expect(html).not.toContain('href="/items/4"');
  });

  it("shows one project's alone when the address names it, with the way back to every project's", () => {
    const html = view('q', null);
    expect(rows(html)).toEqual(['Observation 6 says this.']);
    expect(html).toContain("q&#x27;s observations only.");
    expect(html).toContain('href="/observations"');
  });

  it('opens one beside the list, whole, with the owner\'s close folded and no prompt of the browser', () => {
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
    // The one open stays in its list though the handled are hidden.
    expect(rows(taken)).toContain('Observation 2 says this.');
    const closed = view(null, 1);
    expect(closed).toContain('Closed: outdated');
    expect(closed).toContain('Gone with');
    expect(closed).not.toContain('Close it');
  });
});
