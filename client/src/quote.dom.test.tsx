// @vitest-environment happy-dom
// Quoting a bubble in a conversation, counted in a DOM: the button at the head of a message puts the bubble's words into
// the draft of the box at the bottom, after what is there, kept as any draft is; sent, they go to the interface as the
// words of the message, with what the owner wrote, and no reply to anything; the page then draws them as a quote. Where
// the box takes no words, or the entry is no message, there is no button. The selection of a part of a bubble is counted
// in a real browser (pages/Quote.browser.test.tsx).

import './dom-setup';

import { act, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { Conversation, ConversationDetail, ConversationEntry } from './api';
import { forgetMemory } from './drafts';
import { Open } from './pages/Messages';
import { t } from './text';

const WORDS_AT = 'wf-agents.draft.';

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

/// Takes what is drawn away, as leaving the page does.
async function leave(): Promise<void> {
  const mine = root;
  if (mine !== null) await act(async () => mine.unmount());
  host?.remove();
  root = null;
  host = null;
}

/// Types `value` into the box as the owner does: the browser's own setter, then the event React listens to.
async function type(box: HTMLTextAreaElement, value: string): Promise<void> {
  const set = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set;
  await act(async () => {
    set?.call(box, value);
    box.dispatchEvent(new Event('input', { bubbles: true }));
  });
}

const click = (el: Element) => act(async () => (el as HTMLElement).click());

/// Lets the requests under way end and what they changed be drawn.
const settle = () =>
  act(async () => {
    await new Promise((r) => setTimeout(r, 0));
  });

function must<T>(found: T | null | undefined): T {
  if (found === null || found === undefined) throw new Error('not drawn');
  return found;
}

const textarea = (h: HTMLElement): HTMLTextAreaElement => must(h.querySelector('textarea'));
const button = (h: HTMLElement, name: string): HTMLButtonElement =>
  must([...h.querySelectorAll('button')].find((b) => b.textContent?.trim() === name));
/// What labels a button drawn as an icon: the tooltip Primer gives it (`aria-labelledby`), or its own label.
const labelOf = (b: Element): string => {
  const by = b.getAttribute('aria-labelledby');
  return by !== null ? (document.getElementById(by)?.textContent ?? '') : (b.getAttribute('aria-label') ?? '');
};
const quoteButtons = (h: HTMLElement): HTMLButtonElement[] =>
  [...h.querySelectorAll<HTMLButtonElement>('button')].filter((b) => labelOf(b) === t('conv_quote'));

/// What the browser keeps of the words of the draft under `key`.
const keptWords = (key: string): string | undefined => {
  const raw = localStorage.getItem(WORDS_AT + key);
  return raw === null ? undefined : (JSON.parse(raw) as { text: string }).text;
};

/// A storage that holds what it is given in memory: the tests run on one of their own.
function storage(): Storage {
  const m = new Map<string, string>();
  return {
    get length() {
      return m.size;
    },
    key: (i: number) => [...m.keys()][i] ?? null,
    getItem: (k: string) => m.get(k) ?? null,
    setItem: (k: string, v: string) => void m.set(k, v),
    removeItem: (k: string) => void m.delete(k),
    clear: () => m.clear(),
  };
}

const entry = (id: number, extra: Partial<ConversationEntry> = {}): ConversationEntry => ({
  source: 'message',
  id,
  sender: 'engineer',
  recipient: 'owner',
  text: `said ${id}`,
  at: id,
  work: 5,
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
  policy: null,
  plan: null,
  passed: [],
  images: [],
  retract: 'no',
  ...extra,
});

const detail = (key: string, entries: ConversationEntry[]): ConversationDetail => ({
  key,
  entries,
  asks: [],
  design_cards: [],
  questions: [],
  waits_on: null,
  pass_to: null,
  // Everything is read, so that opening it posts nothing but what the owner sends.
  read_to: 1_000_000,
  gaps: [],
});

const conversation = (key: string, kind: string, extra: Partial<Conversation> = {}): Conversation => ({
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
  ...extra,
});

/// What the fake interface holds: the entries of each conversation by its key, and the POSTs it was sent.
const entries = new Map<string, ConversationEntry[]>();
const posts: Array<{ path: string; body: Record<string, unknown> }> = [];

beforeEach(() => {
  vi.stubGlobal('localStorage', storage());
  forgetMemory();
  entries.clear();
  posts.length = 0;
  vi.stubGlobal('fetch', async (path: string, init?: { method?: string; body?: string }) => {
    if (init?.method === 'POST') {
      posts.push({ path, body: JSON.parse(init.body ?? 'null') as Record<string, unknown> });
      return { ok: true, text: async () => '' };
    }
    const at = /^\/api\/v1\/conversation\?key=([^&]+)&/.exec(path);
    const held = at !== null ? entries.get(decodeURIComponent(at[1])) : undefined;
    if (at !== null && held !== undefined) {
      return { ok: true, text: async () => JSON.stringify(detail(decodeURIComponent(at[1]), held)) };
    }
    return { ok: false, status: 404, statusText: 'Not Found', json: async () => ({ error: 'not found' }) };
  });
});

afterEach(async () => {
  await leave();
  vi.unstubAllGlobals();
});

const ASKED = 'Which table?\n\nOr both.';
/// The engineer's question as the owner quotes it, written out, and the owner's own words the same way.
const quote = '> **engineer**\n>\n> Which table?\n>\n> Or both.\n\n';
const OWNER_QUOTE = '> **owner**\n>\n> Use the old one.\n\n';
const ARCHITECT_QUOTE = '> **architect**\n>\n> Should it wait?\n\n';

/// A work's group where the engineer asked something and the engine said something.
const group = () => {
  const c = conversation('work:5', 'work');
  entries.set(c.key, [
    entry(1, { text: ASKED }),
    entry(2, { source: 'notice', sender: 'engine', recipient: 'owner', text: 'Round 3 began.' }),
    entry(3, { sender: 'owner', recipient: 'engineer', source: 'interjection', text: 'Use the old one.' }),
  ]);
  return c;
};

describe('the button that quotes a bubble', () => {
  it('is on the messages of the group, not on a line of the engine', async () => {
    const c = group();
    const page = await show(<Open c={c} works={undefined} side={null} />);
    await settle();
    expect(page.textContent).toContain('Round 3 began.');
    // The engineer's message and the owner's own: two bubbles, two buttons.
    expect(quoteButtons(page)).toHaveLength(2);
  });

  it('is not there where the box takes no words: a work that is over', async () => {
    const c = conversation('work:5', 'work', { over: true });
    entries.set(c.key, [entry(1, { text: ASKED })]);
    const page = await show(<Open c={c} works={undefined} side={null} />);
    await settle();
    expect(page.textContent).toContain('Which table?');
    expect(quoteButtons(page)).toHaveLength(0);
  });
});

describe('quoting a bubble', () => {
  it('puts its words in the box, quoted, and the keyboard there, ready for what the owner writes', async () => {
    const page = await show(<Open c={group()} works={undefined} side={null} />);
    await settle();
    await click(quoteButtons(page)[0]);
    const box = textarea(page);
    expect(box.value).toBe(quote);
    expect(document.activeElement).toBe(box);
  });

  it('goes after the words already in the box, and the owner may quote more', async () => {
    const page = await show(<Open c={group()} works={undefined} side={null} />);
    await settle();
    await type(textarea(page), 'Before');
    await click(quoteButtons(page)[0]);
    expect(textarea(page).value).toBe(`Before\n${quote}`);
    await click(quoteButtons(page)[1]);
    expect(textarea(page).value).toBe(`Before\n${quote}${OWNER_QUOTE}`);
  });

  it('is kept with the draft when the conversation is left and shown again, and in the browser', async () => {
    const c = group();
    const page = await show(<Open c={c} works={undefined} side={null} />);
    await settle();
    await click(quoteButtons(page)[0]);
    expect(keptWords('conv:work:5')).toBe(quote);
    await leave();
    const again = await show(<Open c={c} works={undefined} side={null} />);
    await settle();
    expect(textarea(again).value).toBe(quote);
  });

  it('is sent with what the owner wrote, to the engineer, as words and no reply, and the box is emptied', async () => {
    const c = group();
    const page = await show(<Open c={c} works={undefined} side={null} />);
    await settle();
    await click(quoteButtons(page)[0]);
    await type(textarea(page), `${quote}Both.`);
    await click(button(page, t('conv_send')));
    await settle();
    expect(posts).toHaveLength(1);
    expect(posts[0].path).toBe('/api/v1/works/5/say');
    // The words the engineer is given: the quote, a blank line, then what the owner wrote.
    expect(posts[0].body).toMatchObject({
      text: '> **engineer**\n>\n> Which table?\n>\n> Or both.\n\nBoth.',
      to: 'engineer',
    });
    expect(posts[0].body).not.toHaveProperty('reply_to');
    expect(textarea(page).value).toBe('');
    expect(keptWords('conv:work:5')).toBeUndefined();
  });

  it('is sent to the architect in the one-to-one as words of the owner, not as the reply to a question', async () => {
    const c = conversation('architect:p', 'architect');
    entries.set(c.key, [entry(1, { sender: 'architect', work: null, text: 'Should it wait?' })]);
    const page = await show(<Open c={c} works={undefined} side={null} />);
    await settle();
    await click(quoteButtons(page)[0]);
    await type(textarea(page), `${ARCHITECT_QUOTE}No.`);
    await click(button(page, t('conv_send')));
    await settle();
    expect(posts).toHaveLength(1);
    expect(posts[0].path).toBe('/api/v1/architect/say?project=p');
    expect(posts[0].body).toMatchObject({ text: '> **architect**\n>\n> Should it wait?\n\nNo.' });
    expect(posts[0].body).not.toHaveProperty('reply_to');
  });
});

describe('the words the owner sent with a quote', () => {
  it('are drawn in their bubble as a quote with what the owner wrote after it, not as lines of marks', async () => {
    const c = conversation('work:5', 'work');
    entries.set(c.key, [entry(1, { sender: 'owner', source: 'interjection', recipient: 'engineer', text: `${quote}Both.` })]);
    const page = await show(<Open c={c} works={undefined} side={null} />);
    await settle();
    const quoteBlock = must(page.querySelector('.talk-bubble-body blockquote.md-quote'));
    expect(quoteBlock.textContent).toContain('engineer');
    expect(quoteBlock.textContent).toContain('Which table?');
    expect(quoteBlock.textContent).toContain('Or both.');
    expect(quoteBlock.textContent).not.toContain('>');
    // What the owner wrote is in the bubble, outside the quote.
    expect(page.querySelector('.talk-bubble-body')?.textContent).toContain('Both.');
    expect(quoteBlock.textContent).not.toContain('Both.');
  });
});
