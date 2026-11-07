// @vitest-environment happy-dom
// The owner's words not taken up yet, counted in a DOM: words nobody has can be taken back, which puts their text, their
// images and whom they were for in the box of the conversation and takes the bubble away; words an agent has, though
// they are still not recorded as taken up, say they cannot be taken back, keep their bubble and their text, and have no
// button. A refusal of the interface is said where the button is.

import './dom-setup';

import { act, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { AnswersHere } from './answer';
import type { Conversation, ConversationDetail, ConversationEntry, TakenBack } from './api';
import { forgetMemory, putDraft } from './drafts';
import { useResource } from './live';
import { Composer, Stream } from './pages/Messages';
import { t } from './text';

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

const posts: Array<{ path: string; body: unknown }> = [];
/// What the fake interface gives back for the words taken back; none refuses with `refusal`.
let back: TakenBack | null;
let refusal = '';
/// The words the conversation holds as it is read: they leave it once they are taken back.
let left: ConversationEntry[];

const words = (id: number, text: string, retract: ConversationEntry['retract']): ConversationEntry => ({
  source: 'interjection',
  id,
  sender: 'owner',
  recipient: 'engineer',
  text,
  at: id,
  work: 5,
  round: null,
  ask: null,
  design_card: null,
  matter: null,
  read_at: null,
  waiting: false,
  pending: true,
  undelivered: false,
  answerable: false,
  fault: null,
  misses: null,
  ask_detail: null,
  stage: null,
  matter_card: null,
  words_state: null,
  retract,
  policy: null,
  plan: null,
  passed: [],
  images: [],
});

const group: Conversation = {
  key: 'work:5',
  kind: 'work',
  project: 'p',
  projects: [],
  work: 5,
  item: null,
  title: null,
  level: null,
  over: false,
  waiting: 0,
  unread: 0,
  last_at: 1,
};

beforeEach(() => {
  vi.stubGlobal('localStorage', storage());
  forgetMemory();
  posts.length = 0;
  back = null;
  refusal = '';
  left = [words(1, 'Make it blue.', 'yes'), words(2, 'Why this size?', 'given')];
  vi.stubGlobal('fetch', async (path: string, init?: { method?: string; body?: string }) => {
    if (init?.method === 'POST') {
      posts.push({ path, body: JSON.parse(init.body ?? 'null') });
      if (back === null) {
        return { ok: false, status: 422, statusText: 'Unprocessable', json: async () => ({ error: refusal }) };
      }
      // Taken back, the words are no more in the conversation.
      left = left.filter((e) => e.id !== 1);
      return { ok: true, text: async () => JSON.stringify(back) };
    }
    if (path.startsWith('/api/v1/conversation?')) {
      const d: ConversationDetail = {
        key: group.key,
        entries: left,
        asks: [],
        design_cards: [],
        questions: [],
        waits_on: null,
        pass_to: null,
        read_to: 1_000_000,
        gaps: [],
      };
      return { ok: true, text: async () => JSON.stringify(d) };
    }
    return { ok: false, status: 404, statusText: 'Not Found', json: async () => ({ error: 'not found' }) };
  });
});

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

afterEach(async () => {
  const mine = root;
  if (mine !== null) await act(async () => mine.unmount());
  host?.remove();
  root = null;
  host = null;
  vi.unstubAllGlobals();
});

const settle = () =>
  act(async () => {
    await new Promise((r) => setTimeout(r, 0));
  });

/// The group's stream with its box at the bottom, read from the fake interface as the page reads a conversation.
function Talk() {
  const r = useResource<ConversationDetail>('messages', `/api/v1/conversation?key=${group.key}&limit=100`);
  if (r.data === undefined) return null;
  return (
    <AnswersHere.Provider value={true}>
      <Stream d={r.data} c={group} w={null} />
      <Composer c={group} w={null} />
    </AnswersHere.Provider>
  );
}

/// The bubbles of the owner's words and what each says.
const bubbles = (h: HTMLElement): string[] =>
  [...h.querySelectorAll('.talk-bubble-body')].map((b) => b.textContent?.trim() ?? '');

/// Whom the box says its words to: the names of the parties ticked.
const ticked = (h: HTMLElement): string[] =>
  [...h.querySelectorAll<HTMLLabelElement>('.talk-say-to label')]
    .filter((l) => (document.getElementById(l.htmlFor) as HTMLInputElement | null)?.checked === true)
    .map((l) => l.textContent?.trim() ?? '');

const takeBackButtons = (h: HTMLElement): HTMLButtonElement[] =>
  [...h.querySelectorAll('button')].filter((b) => b.textContent?.trim() === t('conv_take_back'));

describe("the owner's words not taken up yet", () => {
  it('come back into the box when no agent has them, and say they cannot when an agent has them', async () => {
    back = { text: 'Make it blue.', images: [{ media_type: 'image/png', data: 'iVBORw0KGgo=' }], to: ['architect'] };
    const page = await show(<Talk />);
    await settle();
    // The box already holds words the owner is writing, for the engineer (the box's own choice until the owner ticks).
    await act(async () => putDraft('conv:work:5', { text: 'Already here.' }, 'replace'));
    expect(ticked(page)).toEqual(['@engineer']);

    // Side by side: the words no agent has offer to be taken back; those an agent has say they cannot, and have no button.
    expect(bubbles(page)).toEqual(['Make it blue.', 'Why this size?']);
    expect(takeBackButtons(page)).toHaveLength(1);
    const heads = [...page.querySelectorAll('.talk-state')].map((s) => s.textContent);
    expect(heads).toEqual([t('conv_pending'), t('conv_given')]);

    await act(async () => takeBackButtons(page)[0].click());
    await settle();

    expect(posts).toEqual([{ path: '/api/v1/conversation/retract', body: { source: 'interjection', id: 1 } }]);
    // The bubble is gone, the other stays whole, and the words are in the box with their image.
    expect(bubbles(page)).toEqual(['Why this size?']);
    expect(takeBackButtons(page)).toHaveLength(0);
    expect(page.textContent).toContain(t('conv_given'));
    // The words come after what the box held, not in place of it, with their image and for whom they were said.
    const box = page.querySelector('textarea');
    expect(box?.value).toBe('Already here.\nMake it blue.');
    expect(page.querySelectorAll('.talk-say img').length).toBeGreaterThan(0);
    expect(ticked(page)).toEqual(['@architect']);
  });

  it('stay where they are, and say why, when the interface refuses to take them back', async () => {
    refusal = 'an agent has these words already';
    const page = await show(<Talk />);
    await settle();

    await act(async () => takeBackButtons(page)[0].click());
    await settle();

    expect(page.textContent).toContain('an agent has these words already');
    expect(bubbles(page)).toEqual(['Make it blue.', 'Why this size?']);
    expect(page.querySelector('textarea')?.value).toBe('');
  });
});
