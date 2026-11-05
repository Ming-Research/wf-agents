// The panel beside the owner's one-to-one with a project's architect, which the owner closes and opens again, as the
// browser keeps it: its notebook, its policy with the draft awaiting the owner's approval on top, and its queue, which
// left the one-to-one's head; and the one-to-one's head, which holds what the architect's page, with no line over its
// chart, no longer says: the holds on the project's new works, each with why.

import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { Architect, ArchitectPanel, ArchitectWork, Conversation, Matter, NotebookVersion, WorkSummary } from './api';
import { time } from './format';
import { ArchitectSide, keepSide, sideKept } from './pages/ArchitectSide';
import { ArchitectHead } from './pages/TalkHead';
import { href, parse, sideHref } from './route';

// Drawn once, outside a browser: the engine's state is not known, and the only thing read is the panel's own read of
// the architect, which is as `read` (below) has it.
const read: { panel: ArchitectPanel | undefined } = { panel: undefined };
vi.mock('./live', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./live')>()),
  useStatus: () => null,
  useResource: (_key: string | null, path: string | null) => ({
    data: path === '/api/v1/architect/panel?project=p' ? read.panel : undefined,
    error: null,
    stale: false,
  }),
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
  ran_ms: 0,
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

const going: ArchitectWork = { work: work(9), handling: 12, listens: false };

const architect: Architect = {
  project: 'p',
  projects: ['p'],
  works: [going],
  queue: [matter(11), matter(12, { what: 'The matter in hand' }), matter(13, { asked: 5, resume_cell: 'P1' })],
  holds: [],
  asks: [],
  draft: { work: 9, round: 7, at: 70, policy: { direction: 'Ship the parser.', why: 'It blocks all.', principles: [], signals: [] } },
};

const panel: ArchitectPanel = {
  handled: [],
  changes: [],
  policy: {
    work: 3,
    round: 4,
    cell: 'P12',
    at: 40,
    policy: { direction: 'Keep it small.', why: 'Small is quick.', principles: [], signals: [] },
  },
  appended: [{ at: 50, who: 'matter 4 (report)', text: 'Took the report in.', plan: 'The parser first.', understanding: null }],
  notebook: [
    {
      section: 'project_now',
      now: version('Two parsers.', 5, 'U1'),
      versions: [version('Two parsers.', 5, 'U1'), version('One parser.', 2, 'U1')],
    },
    { section: 'plan', now: version('Merge them.', 6, 'N1'), versions: [version('Merge them.', 6, 'N1')] },
  ],
  constitution: {
    name: 'CONSTITUTION.md',
    commit: '0123456789abcdef0123456789abcdef01234567',
    committed_at: 1_700_000_000_000,
    text: 'Article 1. Be quick.',
    error: null,
  },
};

/// The panel drawn on a tab, the architect it is of and what the read of the rest of the architect gave (`given`, none
/// while it is not read yet).
const side = (
  tab: 'notebook' | 'policy' | 'queue',
  a: Architect = architect,
  given: ArchitectPanel | null = panel,
): string => {
  read.panel = given ?? undefined;
  return renderToStaticMarkup(<ArchitectSide a={a} tab={tab} onClose={() => undefined} />);
};

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

  it('shows each part of the notebook folded under the round and cell that last wrote it, its earlier versions folded, then what came since the base', () => {
    const html = side('notebook');
    const titles = [...html.matchAll(/class="section-title[^"]*">([^<]*)</g)].map((m) => m[1]);
    expect(titles).toEqual(['How it understands the project', 'Earlier versions', 'The plan', 'Since the base']);
    const now = section(html, 'How it understands the project');
    expect(now).not.toMatch(/^<details class="section[^"]*"[^>]* open=""/);
    expect(now.slice(0, now.indexOf('Earlier versions'))).toContain('R9-5');
    expect(now).toContain('Two parsers.');
    const earlier = section(html, 'Earlier versions');
    expect(earlier).not.toMatch(/^<details[^>]* open=""/);
    expect(earlier.slice(0, earlier.indexOf('The plan'))).toContain('One parser.');
    expect(earlier.slice(0, earlier.indexOf('The plan'))).not.toContain('Two parsers.');
    expect(section(html, 'The plan')).not.toContain('Earlier versions');
    const since = section(html, 'Since the base');
    expect(since).toContain('Took the report in.');
    expect(since).toContain('The parser first.');
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

  it('names the commit of main the constitution was read at beside its path, short, with when it was committed', () => {
    const html = section(side('policy'), 'Constitution');
    expect(html).toMatch(/CONSTITUTION\.md on the project(&#x27;|')s main, at 0123456 \(/);
    expect(html).toContain(`(${time(1_700_000_000_000)})`);
    expect(html).not.toContain('0123456789abcdef');
    // A commit with no time known says so.
    const known = panel.constitution;
    expect(known).not.toBeNull();
    const unknown = { ...panel, constitution: known === null ? null : { ...known, committed_at: null } };
    expect(section(side('policy', architect, unknown), 'Constitution')).toContain('(time unknown)');
  });

  it('says why a constitution could not be read, opened, and not an empty one', () => {
    const why = 'project p: constitution: CONSTITUTION.md cannot be read at 0123456789abcdef: no such path';
    const unread = {
      ...panel,
      constitution: {
        name: 'CONSTITUTION.md',
        commit: '0123456789abcdef0123456789abcdef01234567',
        committed_at: 1_700_000_000_000,
        text: '',
        error: why,
      },
    };
    const html = section(side('policy', architect, unread), 'Constitution');
    expect(html).toMatch(/^<details[^>]* open=""/);
    expect(html).toContain(why);
    expect(html).not.toContain('is empty');
    // Without a commit of main, the path stands alone, and the reason is still given.
    const nowhere = { ...panel, constitution: { ...unread.constitution, commit: null } };
    const bare = section(side('policy', architect, nowhere), 'Constitution');
    expect(bare).toMatch(/CONSTITUTION\.md on the project(&#x27;|')s main</);
    expect(bare).not.toMatch(/main, at/);
    expect(bare).toContain(why);
    // A constitution that was read and holds nothing is said to be empty.
    const empty = { ...panel, constitution: { ...unread.constitution, text: '  ', error: null } };
    expect(section(side('policy', architect, empty), 'Constitution')).toMatch(/CONSTITUTION\.md on the project(&#x27;|')s main is empty\./);
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

  it('reads the notebook, the policy and the matters handled apart from the architect: until they are read it says so, and the queue it draws from the architect shows', () => {
    for (const tab of ['notebook', 'policy'] as const) {
      const html = side(tab, architect, null);
      expect(html).toContain('Loading…');
      expect(html).not.toContain('Two parsers.');
      expect(html).not.toContain('Keep it small.');
    }
    const queue = side('queue', architect, null);
    expect(queue).toContain('The matter in hand');
    expect(queue).not.toContain('handled today');
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

  it("holds what the architect's page, with no line over its chart, leaves to it: the holds on new works, with why", () => {
    const held: Architect = {
      ...architect,
      holds: [{ reason: 'architect', why: 'The plan is being set straight.', since: 5 }],
    };
    const html = renderToStaticMarkup(<ArchitectHead c={one} a={held} onSide={null} />);
    expect(html).toMatch(/class="talk-head-hold"[^>]*>[\s\S]*The plan is being set straight\./);
    // Its name leads to the architect's page, the way to each project's architect.
    expect(html).toMatch(/<a [^>]*href="\/works\/architect\?project=p"[^>]*>/);
  });

  // Item #239: the sign of each hold, whoever holds, says who and why and has the owner's Release; the button that holds
  // new starts is there while the owner has no hold of their own, and gives way to the owner's sign once they have.
  it('shows each hold with its source, its reason and a Release, and the owner\'s Hold new starts while they hold none', () => {
    const held: Architect = {
      ...architect,
      holds: [
        { reason: 'owner', why: 'Leaving for a week.', since: 3 },
        { reason: 'architect', why: 'Wrong order.', since: 4 },
        { reason: 'set_straight', why: null, since: 5 },
      ],
    };
    const html = renderToStaticMarkup(<ArchitectHead c={one} a={held} onSide={null} />);
    expect(html).toMatch(/class="talk-head-hold"[^>]*>[\s\S]*You stopped new works from starting: Leaving for a week\./);
    expect(html).toMatch(/class="talk-head-hold"[^>]*>[\s\S]*No new works start: the architect holds them: Wrong order\./);
    expect(html).toContain('No new works start until the plan fixes are done');
    expect(html.match(/>Release</g)?.length).toBe(3);
    // The owner has a hold of their own: nothing offers another.
    expect(html).not.toContain('>Hold new starts<');
    const others: Architect = { ...held, holds: held.holds.filter((h) => h.reason !== 'owner') };
    const html2 = renderToStaticMarkup(<ArchitectHead c={one} a={others} onSide={null} />);
    expect(html2).toContain('>Hold new starts<');
    expect(html2.match(/>Release</g)?.length).toBe(2);
    const none = renderToStaticMarkup(<ArchitectHead c={one} a={{ ...architect, holds: [] }} onSide={null} />);
    expect(none).toContain('>Hold new starts<');
    expect(none).not.toContain('>Release<');
  });
});
