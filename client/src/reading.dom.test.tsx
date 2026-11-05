// @vitest-environment happy-dom
// What the page draws when what it reads changes, counted in a DOM: an entry of a conversation that did not change is
// not drawn again when the conversation is read anew, the line that says entries are folded away asks for the next
// window without a flash or a jump, and a part that reads a path through `useResource` with `staleOk` keeps the data of
// the path before until the new one is read. The page reads through the real reader (`reader.ts`) from a fake `fetch`.

import './dom-setup';

import { act, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { AnswersHere } from './answer';
import type { Architect, Conversation, ConversationDetail, ConversationEntry, WorkSummary } from './api';
import { useResource } from './live';
import { Open, Stream } from './pages/Messages';
import { bump } from './reader';
import { t, tn } from './text';

// Every time an entry of the stream is drawn, `Entry` asks `mentions` about it, once: the entries drawn are counted
// there, by the entry's row.
const drawn = vi.hoisted(() => [] as number[]);
vi.mock('./conversation', async (importOriginal) => {
  const real = await importOriginal<typeof import('./conversation')>();
  return {
    ...real,
    mentions: (e: ConversationEntry, kind: Conversation['kind']) => {
      drawn.push(e.id);
      return real.mentions(e, kind);
    },
  };
});

/// How many times the entry with row `id` was drawn.
const drawnTimes = (id: number): number => drawn.filter((x) => x === id).length;

/// What the fake interface answers at a path: the body (a promise of it holds the answer back), or none for a path it
/// does not know, which it answers as not found.
let serve: (path: string) => unknown;
const asked: string[] = [];

beforeEach(() => {
  asked.length = 0;
  drawn.length = 0;
  serve = () => undefined;
  vi.stubGlobal('fetch', async (path: string, init?: { method?: string }) => {
    asked.push(path);
    if (init?.method === 'POST') return { ok: true, text: async () => '' };
    const body = await serve(path);
    if (body === undefined) {
      return { ok: false, status: 404, statusText: 'Not Found', json: async () => ({ error: 'not found' }) };
    }
    return { ok: true, text: async () => JSON.stringify(body) };
  });
});

let root: Root | null = null;
let host: HTMLElement | null = null;

/// Draws `ui` in the document, inside `act`.
async function show(ui: ReactNode): Promise<HTMLElement> {
  host = document.createElement('div');
  document.body.append(host);
  const mine = createRoot(host);
  root = mine;
  await act(async () => mine.render(ui));
  return host;
}

afterEach(async () => {
  // Leaving ends every read the parts held.
  const mine = root;
  if (mine !== null) await act(async () => mine.unmount());
  host?.remove();
  root = null;
  host = null;
  vi.unstubAllGlobals();
});

/// Lets the reads under way end and what they changed be drawn.
const settle = () =>
  act(async () => {
    await new Promise((r) => setTimeout(r, 0));
  });

/// A promise held back until `release`.
function held<T>(): { promise: Promise<T>; release: (v: T) => void } {
  let release!: (v: T) => void;
  const promise = new Promise<T>((r) => (release = r));
  return { promise, release };
}

const entry = (id: number, extra: Partial<ConversationEntry> = {}): ConversationEntry => ({
  source: 'message',
  id,
  sender: 'architect',
  recipient: 'owner',
  text: `said ${id}`,
  at: id,
  work: null,
  round: null,
  ask: null,
  design_card: null,
  matter: null,
  read_at: null,
  waiting: false,
  pending: false,
  undelivered: false,
  answerable: false,
  fault: null,
  misses: null,
  ask_detail: null,
  stage: null,
  matter_card: null,
  words_state: null,
  retract: 'no',
  policy: null,
  plan: null,
  forwards: [],
  images: [],
  ...extra,
});

const detail = (key: string, entries: ConversationEntry[], gaps: ConversationDetail['gaps'] = []): ConversationDetail => ({
  key,
  entries,
  asks: [],
  design_cards: [],
  questions: [],
  waits_on: null,
  forwarding: null,
  // Everything is read, so that opening it posts nothing and reads nothing again.
  read_to: 1_000_000,
  gaps,
});

const conversation = (key: string, kind: string): Conversation => ({
  key,
  kind,
  project: 'p',
  projects: [],
  work: kind === 'work' ? 5 : null,
  item: null,
  title: null,
  level: null,
  over: false,
  waiting: 0,
  unread: 0,
  last_at: 1,
});

const architectWork = (ran_ms: number): WorkSummary => ({
  id: 9,
  item: null,
  title: null,
  project: 'p',
  graph: 'architect',
  flow_version: 2,
  moved_from: null,
  at: 'H8',
  at_title: 'What you ask',
  stage: null,
  state: 'running',
  paused: null,
  paused_by: null,
  ended: null,
  waits: null,
  branch: null,
  pr: null,
  started_at: 100,
  last_at: 150,
  ran_ms,
  rounds: 10,
  sent_back: 0,
  failed: 0,
  running: null,
  cost_usd: 1,
  open_asks: 0,
  open_design_cards: 0,
});

describe('an entry that did not change', () => {
  /// The conversation the architect's page reads and draws as `Open` does: the stream, with the architect's read beside it.
  function Talk({ c }: { c: Conversation }) {
    const r = useResource<ConversationDetail>('messages', `/api/v1/conversation?key=${c.key}&limit=100`);
    const a = useResource<Architect>('architect', '/api/v1/architect?project=p');
    if (r.data === undefined) return null;
    return (
      <AnswersHere.Provider value={true}>
        <Stream d={r.data} c={c} w={null} architect={a.data ?? null} replyTo={reply} />
      </AnswersHere.Provider>
    );
  }
  // One function for as long as the page is open, as `Open` holds it.
  const reply = () => undefined;

  it('is not drawn again when the conversation is read anew with one more entry, while the architect runs', async () => {
    const c = conversation('architect:p', 'architect');
    const first = [1, 2, 3, 4, 5, 6].map((id) => entry(id));
    let conversations = 0;
    let architects = 0;
    serve = (path) => {
      if (path.startsWith('/api/v1/conversation?')) {
        conversations += 1;
        // The second read has one more entry, the others as they were.
        return detail(c.key, conversations === 1 ? first : [...first, entry(7)]);
      }
      if (path.startsWith('/api/v1/architect?')) {
        architects += 1;
        // The architect runs: how long it has run is different at every read.
        const architect: Architect = {
          project: 'p',
          projects: ['p'],
          works: [{ work: architectWork(1000 * architects), handling: null, listens: false }],
          queue: [],
          holds: [],
          asks: [],
          draft: null,
        };
        return architect;
      }
      return undefined;
    };

    const page = await show(<Talk c={c} />);
    await settle();
    expect(page.textContent).toContain('said 6');
    expect(page.textContent).not.toContain('said 7');
    const before = [1, 2, 3, 4, 5, 6].map(drawnTimes);
    // Each of them is drawn, and now.
    expect(before.every((n) => n >= 1)).toBe(true);

    // Both are named, as a round of the architect's does: both are read anew, and the answers differ.
    await act(async () => bump(['messages', 'architect']));
    await settle();

    expect(conversations).toBe(2);
    expect(architects).toBe(2);
    expect(page.textContent).toContain('said 7');
    // The new entry is drawn once; no other is drawn again.
    expect(drawnTimes(7)).toBe(1);
    expect([1, 2, 3, 4, 5, 6].map(drawnTimes)).toEqual(before);
  });

  it('is not drawn again when a full window slides by one entry, the oldest leaving and the gap growing', async () => {
    const c = conversation('architect:p', 'architect');
    // The window of 100 holds entries 1 to 100 with 255 folded away before them; with one more it holds 2 to 101.
    const all = Array.from({ length: 101 }, (_, k) => entry(k + 1));
    let conversations = 0;
    let architects = 0;
    serve = (path) => {
      if (path.startsWith('/api/v1/conversation?')) {
        conversations += 1;
        return conversations === 1
          ? detail(c.key, all.slice(0, 100), [{ source: 'message', id: 1, count: 255 }])
          : detail(c.key, all.slice(1, 101), [{ source: 'message', id: 2, count: 256 }]);
      }
      if (path.startsWith('/api/v1/architect?')) {
        architects += 1;
        const architect: Architect = {
          project: 'p',
          projects: ['p'],
          works: [{ work: architectWork(1000 * architects), handling: null, listens: false }],
          queue: [],
          holds: [],
          asks: [],
          draft: null,
        };
        return architect;
      }
      return undefined;
    };

    const page = await show(<Talk c={c} />);
    await settle();
    expect(page.textContent).toContain(tn('conv_gap', 255));
    const kept = Array.from({ length: 99 }, (_, k) => k + 2);
    const before = kept.map(drawnTimes);
    expect(before.every((n) => n >= 1)).toBe(true);

    await act(async () => bump(['messages', 'architect']));
    await settle();

    expect(conversations).toBe(2);
    expect(architects).toBe(2);
    expect(page.textContent).toContain('said 101');
    expect(page.textContent).toContain(tn('conv_gap', 256));
    // The new entry is drawn once; none of the 99 that stay is drawn again, though the run they are in now begins with
    // another entry.
    expect(drawnTimes(101)).toBe(1);
    expect(kept.map(drawnTimes)).toEqual(before);
  });
});

describe('the line that says entries are folded away', () => {
  /// A scroll position the page can be asked for and set, since a DOM does no layout: the height of what is scrolled is
  /// fixed, and the position is whatever was last set.
  const positions = new WeakMap<Element, number>();
  const HEIGHT = 5000;
  beforeEach(() => {
    Object.defineProperty(HTMLElement.prototype, 'scrollHeight', { configurable: true, get: () => HEIGHT });
    Object.defineProperty(HTMLElement.prototype, 'scrollTop', {
      configurable: true,
      get(this: Element) {
        return positions.get(this) ?? 0;
      },
      set(this: Element, v: number) {
        positions.set(this, v);
      },
    });
  });
  afterEach(() => {
    Reflect.deleteProperty(HTMLElement.prototype, 'scrollHeight');
    Reflect.deleteProperty(HTMLElement.prototype, 'scrollTop');
  });

  const c = conversation('work:5', 'work');
  const line = (id: number) =>
    entry(id, { sender: 'engineer', recipient: 'owner', work: 5, text: `${id > 7 ? 'newer' : 'older'} ${id}` });

  it('adds the next window of entries when pressed, with no loading and no scroll to the bottom', async () => {
    const newer = [8, 9, 10].map(line);
    const older = [1, 2, 3, 4, 5, 6, 7].map(line);
    const later = held<ConversationDetail>();
    serve = (path) => {
      if (path === `/api/v1/conversation?key=work%3A5&limit=100`) {
        // The latest three, with seven before them folded away.
        return detail(c.key, newer, [{ source: 'message', id: 8, count: 7 }]);
      }
      if (path === `/api/v1/conversation?key=work%3A5&limit=200`) return later.promise;
      return undefined;
    };

    const page = await show(<Open c={c} works={undefined} side={null} />);
    await settle();
    expect(page.textContent).toContain(tn('conv_gap', 7));
    expect(page.textContent).toContain('newer 10');
    expect(page.textContent).not.toContain('older 1');
    const stream = page.querySelector('.talk-stream');
    expect(stream).not.toBeNull();
    // It was scrolled to the newest when it opened, which shows that the position is one the page sets.
    expect(stream?.scrollTop).toBe(HEIGHT);
    // The owner scrolled up to read.
    if (stream !== null && stream !== undefined) stream.scrollTop = 321;

    const more = [...page.querySelectorAll('button')].find((b) => b.textContent === t('conv_gap_more'));
    expect(more).toBeDefined();
    await act(async () => more?.click());
    await settle();
    // The next window is asked for, and while it is not read the stream stays as it was, not a loading.
    expect(asked).toContain('/api/v1/conversation?key=work%3A5&limit=200');
    expect(page.textContent).not.toContain(t('loading'));
    expect(page.textContent).toContain(tn('conv_gap', 7));
    expect(page.textContent).toContain('newer 10');

    await act(async () => later.release(detail(c.key, [...older, ...newer])));
    await settle();
    expect(page.textContent).toContain('older 1');
    expect(page.textContent).toContain('older 7');
    expect(page.textContent).not.toContain(tn('conv_gap', 7));
    expect(page.textContent).not.toContain(t('loading'));
    // Older entries joined above; the stream did not go to the bottom.
    expect(page.querySelector('.talk-stream')?.scrollTop).toBe(321);
  });
});

describe('useResource with staleOk', () => {
  /// What a part reads at `path` is shown as `<stale or fresh>:<n or none>`.
  function Show({ path, staleOk }: { path: string; staleOk: boolean }) {
    const r = useResource<{ n: number }>('x', path, { staleOk });
    return <p>{`${r.stale ? 'stale' : 'fresh'}:${r.data?.n ?? 'none'}`}</p>;
  }

  /// Draws the part at `/a` (read), then at `/b` (held back), and says what it shows while `/b` is not read, and when it is.
  async function changePath(staleOk: boolean): Promise<[string, string]> {
    const b = held<{ n: number }>();
    serve = (path) => (path === '/a' ? { n: 1 } : path === '/b' ? b.promise : undefined);
    const page = await show(<Show path="/a" staleOk={staleOk} />);
    await settle();
    expect(page.textContent).toBe('fresh:1');
    await act(async () => root?.render(<Show path="/b" staleOk={staleOk} />));
    const meanwhile = page.textContent ?? '';
    await act(async () => b.release({ n: 2 }));
    await settle();
    expect(page.textContent).toBe('fresh:2');
    return [meanwhile, page.textContent ?? ''];
  }

  it('gives the data of the path before, marked stale, until the new path is read', async () => {
    expect((await changePath(true))[0]).toBe('stale:1');
  });

  it('gives none for the new path until it is read when it is not set', async () => {
    expect((await changePath(false))[0]).toBe('fresh:none');
  });
});
