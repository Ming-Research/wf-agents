// The panel beside the owner's one-to-one with a project's architect, which the owner closes and opens again, as the
// browser keeps it: its notebook, its policy with the draft awaiting the owner's approval on top, and its queue, which
// left the one-to-one's head; and the architect's page, which no longer draws the policy or what to do next and leads
// to the panel's tabs instead.

import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { Architect, ArchitectWork, Conversation, Matter, NotebookVersion, WorkSummary } from './api';
import { Line } from './pages/Architect';
import { ArchitectSide, keepSide, sideKept } from './pages/ArchitectSide';
import { ArchitectHead } from './pages/TalkHead';
import { href, parse, sideHref } from './route';

// Drawn once, outside a browser: nothing is read, and the engine's state is not known.
vi.mock('./live', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./live')>()),
  useStatus: () => null,
  useResource: () => ({ data: undefined, error: null }),
}));
// The session the owner's Start and Step run in is a store the page reads in a browser, read here as it would be there.
vi.mock('react', async (importOriginal) => {
  const react = await importOriginal<typeof import('react')>();
  return {
    ...react,
    useSyncExternalStore: <T,>(subscribe: (f: () => void) => () => void, get: () => T, server?: () => T) =>
      react.useSyncExternalStore(subscribe, get, server ?? get),
  };
});

const work = (id: number, extra: Partial<WorkSummary> = {}): WorkSummary => ({
  id,
  item: null,
  title: null,
  project: 'p',
  graph: 'architect',
  flow_version: 2,
  moved_from: null,
  at: 'H8',
  at_title: 'What you ask',
  stage: null,
  state: 'ready',
  paused: null,
  paused_by: null,
  ended: null,
  waits: null,
  branch: null,
  pr: null,
  started_at: id * 100,
  last_at: id * 100 + 50,
  rounds: 10,
  sent_back: 0,
  failed: 0,
  running: null,
  cost_usd: 1,
  open_asks: 0,
  open_design_cards: 0,
  ...extra,
});

const matter = (id: number, extra: Partial<Matter> = {}): Matter => ({
  id,
  kind: 'owner_words',
  detail: 'said',
  what: `matter ${id}`,
  source: 'owner',
  item: null,
  title: null,
  work: null,
  round: null,
  at: id,
  asked: null,
  resume_cell: null,
  answered: false,
  waits: 'owner',
  waited_ms: 0,
  urgent: false,
  fix: null,
  card: null,
  ...extra,
});

const version = (text: string, round: number, cell: string): NotebookVersion => ({ text, work: 9, round, cell, at: round * 10 });

const going: ArchitectWork = { work: work(9), handled: 0, handling: 12 };

const architect: Architect = {
  project: 'p',
  projects: ['p'],
  works: [going],
  queue: [matter(11), matter(12, { what: 'The matter in hand' }), matter(13, { asked: 5, resume_cell: 'P1' })],
  holds: [],
  asks: [],
  handled: [],
  changes: [],
  policy: {
    work: 3,
    round: 4,
    cell: 'P12',
    at: 40,
    policy: { direction: 'Keep it small.', why: 'Small is quick.', principles: [], signals: [] },
  },
  draft: { work: 9, round: 7, at: 70, policy: { direction: 'Ship the parser.', why: 'It blocks all.', principles: [], signals: [] } },
  notebook: [
    {
      section: 'project_now',
      now: version('Two parsers.', 5, 'U1'),
      versions: [version('Two parsers.', 5, 'U1'), version('One parser.', 2, 'U1')],
    },
    { section: 'plan', now: version('Merge them.', 6, 'N1'), versions: [version('Merge them.', 6, 'N1')] },
    { section: 'unclear', now: null, versions: [] },
    { section: 'other', now: null, versions: [] },
  ],
  constitution: { name: 'CONSTITUTION.md', text: 'Article 1. Be quick.' },
};

const side = (tab: 'notebook' | 'policy' | 'queue', a: Architect = architect): string =>
  renderToStaticMarkup(<ArchitectSide a={a} tab={tab} onClose={() => undefined} />);

/// The tabs drawn, each its words and its count, the one shown marked.
const tabs = (html: string): string[] =>
  [...html.matchAll(/<a [^>]*href="([^"]*)"[^>]*>(.*?)<\/a>/g)]
    .filter((m) => m[1].includes('side='))
    .map((m) => {
      const words = m[2].replace(/<span class="[^"]*VisuallyHidden[^"]*">[^<]*<\/span>/g, '').replace(/<[^>]+>/g, ' ');
      return `${words.replace(/\s+/g, ' ').trim()}${/aria-current="page"/.test(m[0]) ? ' (shown)' : ''}`;
    });

/// A section drawn, from its heading to the next section of the same level.
function section(html: string, title: string): string {
  const at = html.indexOf(`>${title}<`);
  expect(at).toBeGreaterThanOrEqual(0);
  const begin = html.lastIndexOf('<details', at);
  return html.slice(begin);
}

describe('the panel beside the one-to-one with an architect', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('has three tabs, its notebook, its policy and its queue, which counts the matters left, and a close', () => {
    const html = side('notebook');
    expect(tabs(html)).toEqual(['Notebook (shown)', 'Policy', 'Queue 2']);
    expect(html).toContain('href="/messages?c=architect%3Ap&amp;side=queue"');
    expect(html).toMatch(/octicon-x"[\s\S]*>Close</);
  });

  it('shows each section of the notebook folded under the round and cell that last wrote it, its earlier versions folded', () => {
    const html = side('notebook');
    const titles = [...html.matchAll(/class="section-title[^"]*">([^<]*)</g)].map((m) => m[1]);
    expect(titles).toEqual(['What the project is now', 'Earlier versions', 'The plan', 'What is unclear', 'Anything else']);
    const now = section(html, 'What the project is now');
    expect(now).not.toMatch(/^<details class="section[^"]*"[^>]* open=""/);
    expect(now.slice(0, now.indexOf('Earlier versions'))).toContain('R9-5');
    expect(now).toContain('Two parsers.');
    const earlier = section(html, 'Earlier versions');
    expect(earlier).not.toMatch(/^<details[^>]* open=""/);
    expect(earlier.slice(0, earlier.indexOf('The plan'))).toContain('One parser.');
    expect(earlier.slice(0, earlier.indexOf('The plan'))).not.toContain('Two parsers.');
    expect(section(html, 'The plan')).not.toContain('Earlier versions');
    expect(section(html, 'What is unclear')).toContain('Nothing written yet.');
  });

  it('shows the draft awaiting the owner’s approval on top of its policy, then the policy in force, the constitution folded', () => {
    const html = side('policy');
    expect(tabs(html)).toEqual(['Notebook', 'Policy (shown)', 'Queue 2']);
    const draft = html.indexOf('Draft awaiting your approval');
    expect(draft).toBeGreaterThanOrEqual(0);
    expect(draft).toBeLessThan(html.indexOf('Ship the parser.'));
    expect(html.indexOf('Ship the parser.')).toBeLessThan(html.indexOf('Policy in force'));
    expect(html.indexOf('Policy in force')).toBeLessThan(html.indexOf('Keep it small.'));
    expect(section(html, 'Constitution')).not.toMatch(/^<details[^>]* open=""/);
    expect(section(html, 'Constitution')).toContain('Article 1. Be quick.');
    // With no draft waiting, the policy in force is first.
    const none = side('policy', { ...architect, draft: null });
    expect(none).not.toContain('Draft awaiting your approval');
  });

  it('shows the queue as the head showed it: its facts, then its matters in order, the one in hand marked', () => {
    const html = side('queue');
    expect(html).toContain('2 matters left');
    expect(html).toContain('1 held by your ask');
    const rows = [...html.matchAll(/<li( class="talk-queue-current")?>(.*?)<\/li>/g)];
    expect(rows.map((m) => (m[2].match(/matter \d+|The matter in hand/) ?? [''])[0])).toEqual([
      'matter 11',
      'The matter in hand',
      'matter 13',
    ]);
    expect(rows[1][1]).toBeDefined();
    expect(rows[1][2]).toContain('in hand');
    expect(rows[2][2]).toContain('held by your ask');
  });

  it('is open unless this browser kept it closed, and keeps what the owner sets', () => {
    const kept = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (k: string) => kept.get(k) ?? null,
      setItem: (k: string, v: string) => kept.set(k, v),
    });
    expect(sideKept()).toBe(true);
    keepSide(false);
    expect(sideKept()).toBe(false);
    keepSide(true);
    expect(sideKept()).toBe(true);
    // A browser that keeps nothing has it open, and setting it throws nothing.
    vi.stubGlobal('localStorage', {
      getItem: () => {
        throw new Error('blocked');
      },
      setItem: () => {
        throw new Error('blocked');
      },
    });
    expect(sideKept()).toBe(true);
    expect(() => keepSide(false)).not.toThrow();
  });

  it('is named by the address of the one-to-one, which opens it on a tab', () => {
    expect(sideHref('p', 'policy')).toBe('/messages?c=architect%3Ap&side=policy');
    expect(parse('/messages', '?c=architect%3Ap&side=queue')).toEqual({ page: 'messages', open: 'architect:p', side: 'queue' });
    expect(parse('/messages', '?c=architect%3Ap&side=asks')).toEqual({ page: 'messages', open: 'architect:p' });
    expect(href({ page: 'messages', open: 'architect:p' })).toBe('/messages?c=architect%3Ap');
  });
});

describe('the head of the one-to-one with an architect', () => {
  const one: Conversation = {
    key: 'architect:p',
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
    last_at: null,
  };

  it('no longer holds the queue, and opens the panel while it is closed', () => {
    const closed = renderToStaticMarkup(<ArchitectHead c={one} a={architect} onSide={() => undefined} />);
    expect(closed).not.toContain('matters left');
    expect(closed).not.toContain('talk-queue');
    expect(closed).toMatch(/octicon-sidebar-expand"[\s\S]*>Show the notebook, policy and queue</);
    const open = renderToStaticMarkup(<ArchitectHead c={one} a={architect} onSide={null} />);
    expect(open).not.toContain('Show the notebook, policy and queue');
  });
});

describe("the architect's page", () => {
  it('leads from its line to the queue, the notebook and the policy in the panel beside the one-to-one', () => {
    const html = renderToStaticMarkup(<Line a={architect} shown={going} current={going.work} pop={null} setPop={() => undefined} />);
    expect(html).toMatch(/<a [^>]*href="\/messages\?c=architect%3Ap&amp;side=queue"[^>]*>2 matters left<\/a>/);
    expect(html).toMatch(/<a [^>]*href="\/messages\?c=architect%3Ap&amp;side=notebook"[^>]*>Notebook<\/a>/);
    expect(html).toMatch(/<a [^>]*href="\/messages\?c=architect%3Ap&amp;side=policy"[^>]*>Policy<\/a>/);
  });
});
