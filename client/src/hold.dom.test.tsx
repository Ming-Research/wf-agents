// @vitest-environment happy-dom
// The owner's hold on a project's new works, counted in a DOM, in the head of the one-to-one with the project's
// architect: the button that holds opens a form that says what follows and cannot be sent without the words saying
// why; a Release asks first, saying what it lets happen, and is sent on the owner's confirmation; each is sent to the
// project's own address, and the form closes once the request came through.

import './dom-setup';

import { act, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { Architect, Conversation } from './api';
import { forgetMemory } from './drafts';
import { ArchitectHead } from './pages/TalkHead';

// The engine's state is not known here, and nothing else of the page's live reads is needed.
vi.mock('./live', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./live')>()),
  useStatus: () => null,
  useResource: () => ({ data: undefined, error: null, stale: false }),
}));

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

const click = (el: Element) => act(async () => (el as HTMLElement).click());

/// Lets the requests under way end and what they changed be drawn.
const settle = () =>
  act(async () => {
    await new Promise((r) => setTimeout(r, 0));
  });

/// Types `value` into the box as the owner does: the browser's own setter, then the event React listens to.
async function type(box: HTMLTextAreaElement, value: string): Promise<void> {
  const set = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set;
  await act(async () => {
    set?.call(box, value);
    box.dispatchEvent(new Event('input', { bubbles: true }));
  });
}

function must<T>(found: T | null | undefined): T {
  if (found === null || found === undefined) throw new Error('not drawn');
  return found;
}

/// The button of a name within `within`.
const button = (within: Element, name: string): HTMLButtonElement =>
  must([...within.querySelectorAll('button')].find((b) => b.textContent?.trim() === name));

/// The form under the head's row, where a hold or a release asks first.
const form = (h: HTMLElement): HTMLElement => must(h.querySelector<HTMLElement>('.talk-head-ask'));

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
let refuse: string | null = null;

beforeEach(() => {
  vi.stubGlobal('localStorage', storage());
  forgetMemory();
  posts.length = 0;
  refuse = null;
  vi.stubGlobal('fetch', async (path: string, init?: { method?: string; body?: string }) => {
    if (init?.method === 'POST') {
      posts.push({ path, body: JSON.parse(init.body ?? 'null') });
      if (refuse !== null) {
        return { ok: false, status: 422, statusText: 'Unprocessable', json: async () => ({ error: refuse }) };
      }
      return { ok: true, text: async () => '' };
    }
    return { ok: false, status: 404, statusText: 'Not Found', json: async () => ({ error: 'not found' }) };
  });
});

afterEach(async () => {
  const mine = root;
  if (mine !== null) await act(async () => mine.unmount());
  host?.remove();
  root = null;
  host = null;
  vi.unstubAllGlobals();
});

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

const architect = (holds: Architect['holds']): Architect => ({
  project: 'p',
  projects: ['p'],
  works: [],
  queue: [],
  holds,
  asks: [],
  draft: null,
});

describe('the owner holds the new starts of a project', () => {
  it('asks first, in a form that says what follows, and sends the words only once they are written', async () => {
    const h = await show(<ArchitectHead c={one} a={architect([])} onSide={null} />);
    expect(h.querySelector('.talk-head-ask')).toBeNull();
    await click(button(h, 'Hold new starts'));
    const ask = form(h);
    expect(ask.textContent).toContain('No new work of p will start.');
    expect(ask.textContent).toContain('Works already running go on and merge');
    expect(ask.textContent).toContain('Pause all in the top bar');
    // No words: the button that sends is greyed, and pressing it sends nothing.
    const send = button(ask, 'Hold new starts');
    expect(send.disabled).toBe(true);
    await click(send);
    expect(posts).toEqual([]);
    const box = must(ask.querySelector('textarea'));
    await type(box, '   ');
    expect(button(form(h), 'Hold new starts').disabled).toBe(true);
    await type(box, 'Leaving for a week.');
    expect(button(form(h), 'Hold new starts').disabled).toBe(false);
    await click(button(form(h), 'Hold new starts'));
    await settle();
    expect(posts).toEqual([{ path: '/api/v1/projects/p/hold', body: { why: 'Leaving for a week.' } }]);
    // It came through: the form is gone, and the words with it.
    expect(h.querySelector('.talk-head-ask')).toBeNull();
  });

  it('closes on Cancel without sending, and keeps the words for the next time', async () => {
    const h = await show(<ArchitectHead c={one} a={architect([])} onSide={null} />);
    await click(button(h, 'Hold new starts'));
    await type(must(form(h).querySelector('textarea')), 'Maybe.');
    await click(button(form(h), 'Cancel'));
    expect(h.querySelector('.talk-head-ask')).toBeNull();
    expect(posts).toEqual([]);
    await click(button(h, 'Hold new starts'));
    expect(must(form(h).querySelector('textarea')).value).toBe('Maybe.');
  });

  it('says why a refused hold failed and keeps the form', async () => {
    refuse = 'no project p';
    const h = await show(<ArchitectHead c={one} a={architect([])} onSide={null} />);
    await click(button(h, 'Hold new starts'));
    await type(must(form(h).querySelector('textarea')), 'Leaving.');
    await click(button(form(h), 'Hold new starts'));
    await settle();
    expect(form(h).textContent).toContain('no project p');
  });
});

describe('the owner releases a hold on the new starts of a project', () => {
  it('asks first for any of the three, saying what it lets happen, and sends the reason on the confirmation', async () => {
    const holds: Architect['holds'] = [
      { reason: 'owner', why: 'Leaving for a week.', since: 3 },
      { reason: 'architect', why: 'Wrong order.', since: 4 },
      { reason: 'set_straight', why: null, since: 5 },
    ];
    const h = await show(<ArchitectHead c={one} a={architect(holds)} onSide={null} />);
    const signs = [...h.querySelectorAll('.talk-head-hold')];
    expect(signs).toHaveLength(3);
    const words = ['unless the architect holds them too', 'the architect reads the hold in its board', 'may hold them again'];
    for (const [k, reason] of ['owner', 'architect', 'set_straight'].entries()) {
      posts.length = 0;
      await click(button(signs[k] as HTMLElement, 'Release'));
      const ask = form(h);
      expect(ask.textContent).toContain(words[k]);
      // Nothing is sent before the confirmation.
      expect(posts).toEqual([]);
      await click(button(ask, 'Release'));
      await settle();
      expect(posts).toEqual([{ path: '/api/v1/projects/p/hold/release', body: { reason } }]);
      expect(h.querySelector('.talk-head-ask')).toBeNull();
    }
  });

  it('closes on Cancel without sending, and says why a refused release failed', async () => {
    const holds: Architect['holds'] = [{ reason: 'architect', why: 'Wrong order.', since: 4 }];
    const h = await show(<ArchitectHead c={one} a={architect(holds)} onSide={null} />);
    await click(button(must(h.querySelector<HTMLElement>('.talk-head-hold')), 'Release'));
    await click(button(form(h), 'Cancel'));
    expect(h.querySelector('.talk-head-ask')).toBeNull();
    expect(posts).toEqual([]);
    refuse = 'p has no hold of the reason architect';
    await click(button(must(h.querySelector<HTMLElement>('.talk-head-hold')), 'Release'));
    await click(button(form(h), 'Release'));
    await settle();
    expect(form(h).textContent).toContain('p has no hold of the reason architect');
  });
});
