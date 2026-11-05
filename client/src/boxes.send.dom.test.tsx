// @vitest-environment happy-dom
// What a send does to the draft of each small box of the page, counted in a DOM: the boxes that answer an ask and an
// engineer's question, the box on a decision card (a ruling and a question about the card), and the reason for closing an
// observation. A send that came through leaves the box empty wherever the box is and is not asked twice; one on its way
// greys the box shown again and does not bring the words back when it ends with the box away; one that was refused keeps
// the words and says why, also in a box shown again and after the page is loaded again.

import './dom-setup';

import { act, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { DesignCard, Observation, OwnerAsk } from './api';
import { forgetMemory } from './drafts';
import { AskAnswer, QuestionAnswer } from './pages/Messages';
import { CardRule } from './pages/Decide';
import { Close } from './pages/Observations';
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

/// What the fake interface answers: a POST waits for `release` when `hold` is set, then comes through or is refused.
const posts: Array<{ path: string; body: unknown }> = [];
let hold: Promise<void> | null = null;
let refuse: string | null = null;

beforeEach(() => {
  vi.stubGlobal('localStorage', storage());
  forgetMemory();
  posts.length = 0;
  hold = null;
  refuse = null;
  vi.stubGlobal('fetch', async (path: string, init?: { method?: string; body?: string }) => {
    if (init?.method === 'POST') {
      posts.push({ path, body: JSON.parse(init.body ?? 'null') });
      await hold;
      if (refuse !== null) {
        return { ok: false, status: 500, statusText: 'Server Error', json: async () => ({ error: refuse }) };
      }
      return { ok: true, text: async () => '' };
    }
    return { ok: false, status: 404, statusText: 'Not Found', json: async () => ({ error: 'not found' }) };
  });
});

afterEach(async () => {
  await leave();
  vi.unstubAllGlobals();
});

const ask: OwnerAsk = {
  id: 17,
  round: 1,
  work: 5,
  item: null,
  body: { question: 'Which table?' },
  answer: null,
  answer_images: [],
  asked_at: 1,
  condition_item: null,
};
const card = { id: 42, work: 5 } as DesignCard;
const observation = { id: 9 } as Observation;

/// A box of the page that is sent by a button: what draws it, the key of its draft, the button, and where it posts.
type Sent = { name: string; ui: () => ReactNode; key: string; send: string; path: string };

const boxes: Sent[] = [
  { name: 'the answer to an ask', ui: () => <AskAnswer a={ask} />, key: 'ask:17', send: t('answer_send'), path: '/api/v1/asks/17/answer' },
  {
    name: "the answer to an engineer's question",
    ui: () => <QuestionAnswer work={5} matter={9} />,
    key: 'question:5:9',
    send: t('answer_send'),
    path: '/api/v1/works/5/say',
  },
  {
    name: 'the ruling on a decision card',
    ui: () => <CardRule c={card} picked="" recommended="a" />,
    key: 'card:42',
    send: t('card_disagree'),
    path: '/api/v1/design-cards/42/rule',
  },
  {
    name: 'the question about a decision card',
    ui: () => <CardRule c={card} picked="" recommended="a" />,
    key: 'card:42',
    send: t('card_ask'),
    path: '/api/v1/works/5/say',
  },
  {
    name: 'the closing of an observation',
    ui: () => <Close o={observation} />,
    key: 'obs-close:9',
    send: t('observation_close'),
    path: '/api/v1/observations/9/close',
  },
];

describe.each(boxes)('a send from $name', ({ ui, key, send, path }) => {
  it('that came through empties the box, here and in the browser, and a box shown again or after a reload is empty', async () => {
    let h = await show(ui());
    await type(textarea(h), 'my words');
    expect(keptWords(key)).toBe('my words');
    await click(button(h, send));
    await settle();
    expect(posts.map((p) => p.path)).toEqual([path]);
    expect(textarea(h).value).toBe('');
    expect(keptWords(key)).toBeUndefined();
    await leave();
    h = await show(ui());
    expect(textarea(h).value).toBe('');
    await leave();
    forgetMemory();
    h = await show(ui());
    expect(textarea(h).value).toBe('');
  });

  it('that is on its way greys the box shown again, is not made twice, and does not bring the words back when it ends with the box away', async () => {
    let release!: () => void;
    hold = new Promise<void>((r) => (release = r));
    let h = await show(ui());
    await type(textarea(h), 'on its way');
    await click(button(h, send));
    expect(posts).toHaveLength(1);
    // The button is greyed, so that a second click asks nothing more.
    await click(button(h, send));
    expect(posts).toHaveLength(1);
    await leave();
    h = await show(ui());
    expect(textarea(h).disabled).toBe(true);
    expect(textarea(h).value).toBe('on its way');
    expect(button(h, send).disabled).toBe(true);
    await leave();
    release();
    await settle();
    expect(posts).toHaveLength(1);
    h = await show(ui());
    expect(textarea(h).value).toBe('');
    expect(textarea(h).disabled).toBe(false);
    expect(keptWords(key)).toBeUndefined();
  });

  it('that was refused keeps the words and says why, in a box shown again and after a reload too, and a new send can be made', async () => {
    refuse = 'the engine said no';
    let h = await show(ui());
    await type(textarea(h), 'my words');
    await click(button(h, send));
    await settle();
    expect(textarea(h).value).toBe('my words');
    expect(textarea(h).disabled).toBe(false);
    expect(h.textContent).toContain('the engine said no');
    await leave();
    h = await show(ui());
    expect(textarea(h).value).toBe('my words');
    expect(h.textContent).toContain('the engine said no');
    // The words are still in the browser; the reason is only in memory.
    await leave();
    forgetMemory();
    h = await show(ui());
    expect(textarea(h).value).toBe('my words');
    expect(h.textContent).not.toContain('the engine said no');
    // Sent again once the engine takes it: the words go and so does the failure.
    refuse = null;
    await click(button(h, send));
    await settle();
    expect(posts).toHaveLength(2);
    expect(textarea(h).value).toBe('');
    expect(h.textContent).not.toContain('the engine said no');
  });
});
