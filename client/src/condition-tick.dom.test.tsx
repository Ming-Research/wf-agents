// @vitest-environment happy-dom
// The tick of a merge condition on a box of the owner's words, counted in a DOM: ticked, a line holds the box's words until
// the owner writes in it, an empty line keeps the words from being sent, and what is sent carries the line's sentence;
// whether it is ticked and the line stay with the words when the box goes away, and go once they are sent. An ask about no
// one item takes no tick, and the merge gate's ask of a condition takes none either and sends its option apart.

import './dom-setup';

import { act, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { AskPicking } from './answer';
import type { Conversation, OwnerAsk } from './api';
import { forgetMemory } from './drafts';
import { AskAnswer, Composer, QuestionAnswer } from './pages/Messages';
import { t } from './text';

let root: Root | null = null;
let host: HTMLElement | null = null;

async function show(ui: ReactNode): Promise<HTMLElement> {
  host = document.createElement('div');
  document.body.append(host);
  const mine = createRoot(host);
  root = mine;
  await act(async () => mine.render(ui));
  return host;
}

async function leave(): Promise<void> {
  const mine = root;
  if (mine !== null) await act(async () => mine.unmount());
  host?.remove();
  root = null;
  host = null;
}

/// Types `value` into a box as the owner does: the browser's own setter, then the event React listens to.
async function type(box: HTMLTextAreaElement | HTMLInputElement, value: string): Promise<void> {
  const proto = box instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
  const set = Object.getOwnPropertyDescriptor(proto, 'value')?.set;
  await act(async () => {
    set?.call(box, value);
    box.dispatchEvent(new Event('input', { bubbles: true }));
  });
}

const click = (el: Element) => act(async () => (el as HTMLElement).click());

const settle = () =>
  act(async () => {
    await new Promise((r) => setTimeout(r, 0));
  });

function must<T>(found: T | null | undefined): T {
  if (found === null || found === undefined) throw new Error('not drawn');
  return found;
}

const words = (h: HTMLElement): HTMLTextAreaElement => must(h.querySelector('textarea'));
const tick = (h: HTMLElement): HTMLInputElement | null => h.querySelector('.condition-tick input[type="checkbox"]');
const line = (h: HTMLElement): HTMLInputElement | null => h.querySelector('.condition-tick input:not([type="checkbox"])');
const button = (h: HTMLElement, name: string): HTMLButtonElement =>
  must([...h.querySelectorAll('button')].find((b) => b.textContent?.trim() === name));

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

const posts: Array<{ path: string; body: Record<string, unknown> }> = [];

beforeEach(() => {
  vi.stubGlobal('localStorage', storage());
  forgetMemory();
  posts.length = 0;
  vi.stubGlobal('fetch', async (path: string, init?: { method?: string; body?: string }) => {
    if (init?.method === 'POST') {
      posts.push({ path, body: JSON.parse(init.body ?? 'null') as Record<string, unknown> });
      return { ok: true, text: async () => '' };
    }
    return { ok: false, status: 404, statusText: 'Not Found', json: async () => ({ error: 'not found' }) };
  });
});

afterEach(async () => {
  await leave();
  vi.unstubAllGlobals();
});

const ask = (extra: Partial<OwnerAsk> = {}): OwnerAsk => ({
  id: 17,
  round: 1,
  work: 5,
  item: 3,
  body: { to: 'owner' },
  answer: null,
  answer_images: [],
  asked_at: 1,
  condition_item: 3,
  ...extra,
});

describe('the tick of a merge condition', () => {
  it('holds the words in its line until the owner writes there, and an empty line keeps them from being sent', async () => {
    const h = await show(<AskAnswer a={ask()} />);
    await type(words(h), 'OK');
    expect(line(h)).toBeNull();
    await click(must(tick(h)));
    expect(must(line(h)).value).toBe('OK');
    await type(words(h), 'OK, yes');
    expect(must(line(h)).value).toBe('OK, yes');
    await type(must(line(h)), '');
    expect(button(h, t('answer_send')).disabled).toBe(true);
    await type(must(line(h)), 'Run it twice before the merge.');
    await type(words(h), 'OK');
    expect(must(line(h)).value).toBe('Run it twice before the merge.');
    await click(button(h, t('answer_send')));
    await settle();
    expect(posts.map((p) => p.path)).toEqual(['/api/v1/asks/17/answer']);
    expect(posts[0].body).toMatchObject({ text: 'OK', speaker: 'owner', condition: 'Run it twice before the merge.' });
    // Sent, the tick and the line are gone with the words.
    expect(must(tick(h)).checked).toBe(false);
    expect(line(h)).toBeNull();
  });

  it('sends no condition when not ticked, and stays with the words when the box goes away', async () => {
    let h = await show(<QuestionAnswer work={5} matter={9} />);
    await type(words(h), 'Yes.');
    await click(must(tick(h)));
    await type(must(line(h)), 'Keep the old path.');
    await leave();
    forgetMemory();
    h = await show(<QuestionAnswer work={5} matter={9} />);
    expect(must(tick(h)).checked).toBe(true);
    expect(must(line(h)).value).toBe('Keep the old path.');
    await click(must(tick(h)));
    expect(line(h)).toBeNull();
    await click(button(h, t('answer_send')));
    await settle();
    expect(posts[0].body).not.toHaveProperty('condition');
    expect(posts[0].body).toMatchObject({ question: 9 });
  });

  it('is not on an ask about no one item, nor on the ask of a condition, which sends its option apart', async () => {
    let h = await show(<AskAnswer a={ask({ condition_item: null })} />);
    expect(tick(h)).toBeNull();
    await leave();
    const judged = ask({ body: { to: 'owner', condition: 1 } });
    const picked = { ask: 17, picked: 'met', pick: () => undefined };
    h = await show(
      <AskPicking.Provider value={picked}>
        <AskAnswer a={judged} />
      </AskPicking.Provider>,
    );
    expect(tick(h)).toBeNull();
    expect(h.textContent).not.toContain(t('answer_to_architect'));
    await click(button(h, t('answer_send')));
    await settle();
    expect(posts[0].body).toMatchObject({ text: '', speaker: 'owner', choice: 'met' });
    expect(posts[0].body).not.toHaveProperty('condition');
  });

  it('is on the box at the bottom of a work group, and of the one-to-one only while it replies to a question about one item', async () => {
    const work = { key: 'work:5', kind: 'work', work: 5, project: 'p' } as unknown as Conversation;
    let h = await show(<Composer c={work} w={null} />);
    await type(words(h), 'Run it for real.');
    await click(must(tick(h)));
    await click(button(h, t('conv_send')));
    await settle();
    expect(posts[0].path).toBe('/api/v1/works/5/say');
    expect(posts[0].body).toMatchObject({ text: 'Run it for real.', condition: 'Run it for real.' });
    await leave();
    const one = { key: 'architect:p', kind: 'architect', project: 'p' } as unknown as Conversation;
    h = await show(<Composer c={one} w={null} />);
    expect(tick(h)).toBeNull();
    await leave();
    const reply = (aboutOneItem: boolean) => ({ question: 2, words: 'Which?', clear: () => undefined, aboutOneItem });
    h = await show(<Composer c={one} w={null} reply={reply(false)} />);
    expect(tick(h)).toBeNull();
    await leave();
    h = await show(<Composer c={one} w={null} reply={reply(true)} />);
    await type(words(h), 'Yes.');
    await click(must(tick(h)));
    await type(must(line(h)), 'Keep the old path.');
    await click(button(h, t('conv_send')));
    await settle();
    expect(posts[1].body).toMatchObject({ reply_to: 2, condition: 'Keep the old path.' });
  });
});
