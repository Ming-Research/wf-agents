// @vitest-environment happy-dom
// A browser that is full, against the store of drafts (`drafts.ts`) and the box that says what it could not keep, counted
// in a DOM on a storage that refuses a write when keys and values together pass a capacity (a full browser does; taking
// something out makes room, which a storage that refuses by key cannot show): the order drafts give their images up in
// when words need room, what is left of an entry that gave them up, the writes that must take nothing out, the words put
// in from outside, and the line the box draws.

import './dom-setup';

import { act, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { Conversation } from './api';
import { type Draft, forgetMemory, putDraft, useDraft } from './drafts';
import type { PendingImage } from './images';
import { Composer } from './pages/Messages';
import { t, tn } from './text';

const WORDS_AT = 'wf-agents.draft.';
const IMAGES_AT = 'wf-agents.draft-images.';

const seen = new Map<string, Draft>();

/// A box that shows its draft as text, its images by their keys, and how many images it says were lost.
function Probe({ k }: { k: string }) {
  const d = useDraft(k);
  seen.set(k, d);
  return (
    <p data-draft={k} data-lost={d.lostImages}>
      <span>{d.text}</span>
      <i>{d.images.map((i) => i.key).join(',')}</i>
    </p>
  );
}

let host: HTMLElement | null = null;
let root: Root | null = null;

async function show(ui: ReactNode): Promise<HTMLElement> {
  host = document.createElement('div');
  document.body.append(host);
  const mine = createRoot(host);
  root = mine;
  await act(async () => mine.render(ui));
  return host;
}

const probes = (...keys: string[]) => (
  <>
    {keys.map((k) => (
      <Probe key={k} k={k} />
    ))}
  </>
);

async function leave(): Promise<void> {
  const mine = root;
  if (mine !== null) await act(async () => mine.unmount());
  host?.remove();
  root = null;
  host = null;
}

const probe = (k: string) => host?.querySelector(`[data-draft="${k}"]`);
const words = (k: string): string => probe(k)?.querySelector('span')?.textContent ?? '(not drawn)';
const imagesShown = (k: string): string => probe(k)?.querySelector('i')?.textContent ?? '(not drawn)';
const lostShown = (k: string): string => probe(k)?.getAttribute('data-lost') ?? '(not drawn)';

const draftOf = (k: string): Draft => {
  const d = seen.get(k);
  if (d === undefined) throw new Error(`no probe under ${k}`);
  return d;
};

const image = (n: number): PendingImage => ({
  key: `image-f-${n}`,
  name: `shot-${n}.png`,
  media_type: 'image/png',
  size: 10,
  data_url: `data:image/png;base64,AA${n}`,
});

/// An image whose bytes take up about a thousand characters of a storage.
const big = (n: number): PendingImage => ({ ...image(n), data_url: `data:image/png;base64,${'A'.repeat(1000)}` });

/// A storage with room for `box.limit` characters, keys and values together, that throws a quota error for a write that
/// does not fit; `box.quotaOn` makes it refuse the keys it says as full, `box.denyOn` as forbidden.
function capped() {
  const m = new Map<string, string>();
  const box = { limit: Infinity, quotaOn: null as ((k: string) => boolean) | null, denyOn: null as ((k: string) => boolean) | null };
  const size = (k: string): number => (m.has(k) ? k.length + (m.get(k) ?? '').length : 0);
  const used = (): number => [...m.keys()].reduce((n, k) => n + size(k), 0);
  const s: Storage = {
    get length() {
      return m.size;
    },
    key: (i: number) => [...m.keys()][i] ?? null,
    getItem: (k: string) => m.get(k) ?? null,
    setItem: (k: string, v: string) => {
      if (box.denyOn?.(k)) throw new DOMException('denied', 'SecurityError');
      if (box.quotaOn?.(k) || used() - size(k) + k.length + v.length > box.limit) throw new DOMException('full', 'QuotaExceededError');
      m.set(k, v);
    },
    removeItem: (k: string) => void m.delete(k),
    clear: () => m.clear(),
  };
  return { s, box, used, size };
}

type Entry = { text?: string; at?: number; lost?: number; to?: string[]; base?: string };
const entryOf = (s: Storage, k: string): Entry | null => {
  const raw = s.getItem(WORDS_AT + k);
  return raw === null ? null : (JSON.parse(raw) as Entry);
};

/// Puts a draft with `images` in `s` as the store writes one.
const seed = (s: Storage, k: string, entry: Entry, images: PendingImage[]) => {
  s.setItem(WORDS_AT + k, JSON.stringify({ text: k, ...entry }));
  s.setItem(IMAGES_AT + k, JSON.stringify(images));
};

beforeEach(() => {
  forgetMemory();
  seen.clear();
  vi.stubGlobal('fetch', async (_path: string, init?: { method?: string }) =>
    init?.method === 'POST'
      ? { ok: true, text: async () => '' }
      : { ok: false, status: 404, statusText: 'Not Found', json: async () => ({ error: 'not found' }) },
  );
});

afterEach(async () => {
  await leave();
  vi.unstubAllGlobals();
});

describe('a draft that needs room for its words', () => {
  it('takes out the images of the drafts changed longest ago, the one whose words cannot be read first, and stops when the words fit', async () => {
    const { s, box, used, size } = capped();
    vi.stubGlobal('localStorage', s);
    const now = Date.now();
    // The box is read first, which sweeps the browser once for the page; what is put in after it is not swept.
    await show(probes('mine'));
    // `gone` has words nobody can read, so it gives way first; `old` carries what its entry must not lose, and had lost one.
    s.setItem(WORDS_AT + 'gone', '{not json');
    s.setItem(IMAGES_AT + 'gone', JSON.stringify([big(1)]));
    seed(s, 'old', { at: now - 4000, to: ['engineer'], base: 'abc', lost: 1 }, [big(2), big(3)]);
    seed(s, 'mid', { at: now - 3000 }, [big(4)]);
    seed(s, 'new', { at: now - 1000 }, [big(5)]);
    seed(s, 'newest', { at: now }, [big(6)]);
    const before = JSON.stringify([...['mid', 'new', 'newest'].map((k) => [s.getItem(WORDS_AT + k), s.getItem(IMAGES_AT + k)])]);
    // Room for the new words once the images of `gone` and `old` are out, and not before: less than one image over.
    const text = 'y'.repeat(1500);
    const final = (WORDS_AT + 'mine').length + JSON.stringify({ text, at: now }).length;
    const freed = size(IMAGES_AT + 'gone') + size(IMAGES_AT + 'old');
    box.limit = used() - freed + final + 60;
    await act(async () => draftOf('mine').setText(text));
    expect(s.getItem(IMAGES_AT + 'gone')).toBeNull();
    expect(s.getItem(IMAGES_AT + 'old')).toBeNull();
    // The images of the drafts changed later are all there, and so are their entries as they were.
    expect(JSON.stringify([...['mid', 'new', 'newest'].map((k) => [s.getItem(WORDS_AT + k), s.getItem(IMAGES_AT + k)])])).toBe(before);
    // The entry that gave its images up keeps its time, its recipients and what it was written against; its count adds up.
    expect(entryOf(s, 'old')).toEqual({ text: 'old', at: now - 4000, to: ['engineer'], base: 'abc', lost: 3 });
    expect(entryOf(s, 'mine')?.text).toBe(text);
    // A page loaded later says how many that draft lost.
    await leave();
    forgetMemory();
    await show(probes('old'));
    expect(lostShown('old')).toBe('3');
    expect(imagesShown('old')).toBe('');
  });

  it("takes out its own images only when no other draft has any left, after the others'", async () => {
    const { s, box, used, size } = capped();
    vi.stubGlobal('localStorage', s);
    const now = Date.now();
    seed(s, 'other', { at: now - 1000 }, [big(1)]);
    await show(probes('mine'));
    await act(async () => {
      draftOf('mine').setText('a');
      draftOf('mine').setImages([big(2)]);
    });
    expect(s.getItem(IMAGES_AT + 'mine')).not.toBeNull();
    const words = 'z'.repeat(1800);
    const final = (WORDS_AT + 'mine').length + JSON.stringify({ text: words, at: now, lost: 1 }).length;
    // Both are needed: with the images of `other` out the words still do not fit.
    box.limit = used() - size(IMAGES_AT + 'other') - size(IMAGES_AT + 'mine') + final + 60;
    await act(async () => draftOf('mine').setText(words));
    expect(s.getItem(IMAGES_AT + 'other')).toBeNull();
    expect(s.getItem(IMAGES_AT + 'mine')).toBeNull();
    expect(entryOf(s, 'other')).toMatchObject({ lost: 1 });
    expect(entryOf(s, 'mine')).toMatchObject({ text: words, lost: 1 });
    // The images are still in the box that has them, and it says nothing.
    expect(imagesShown('mine')).toBe('image-f-2');
    expect(lostShown('mine')).toBe('0');
  });

  it('takes out nothing, and does not hang, when it cannot even take images out', async () => {
    const { s, box } = capped();
    vi.stubGlobal('localStorage', s);
    seed(s, 'other', { at: Date.now() - 1000 }, [big(1)]);
    box.quotaOn = (k) => k === WORDS_AT + 'mine';
    s.removeItem = () => {
      throw new DOMException('denied', 'SecurityError');
    };
    await show(probes('mine'));
    await act(async () => draftOf('mine').setText('kept in memory'));
    expect(words('mine')).toBe('kept in memory');
    expect(s.getItem(IMAGES_AT + 'other')).not.toBeNull();
    expect(s.getItem(WORDS_AT + 'mine')).toBeNull();
  });
});

describe('images that do not fit in a full browser', () => {
  it("are left out without taking another draft's images out, whether the browser says it is full or forbids them", async () => {
    for (const how of ['full', 'forbidden'] as const) {
      const { s, box } = capped();
      vi.stubGlobal('localStorage', s);
      forgetMemory();
      seed(s, 'other', { at: Date.now() - 1000 }, [big(1)]);
      const mineImages = (k: string) => k === IMAGES_AT + 'mine';
      if (how === 'full') box.quotaOn = mineImages;
      else box.denyOn = mineImages;
      await show(probes('mine'));
      await act(async () => {
        draftOf('mine').setText('hi');
        draftOf('mine').setImages([image(2), image(3)]);
      });
      expect(s.getItem(IMAGES_AT + 'other')).not.toBeNull();
      expect(entryOf(s, 'other')).toEqual({ text: 'other', at: expect.any(Number) });
      expect(entryOf(s, 'mine')).toMatchObject({ text: 'hi', lost: 2 });
      expect(imagesShown('mine')).toBe('image-f-2,image-f-3');
      expect(lostShown('mine')).toBe('0');
      await leave();
      forgetMemory();
      await show(probes('mine'));
      expect(lostShown('mine')).toBe('2');
      await leave();
    }
  });
});

describe('words put in from outside a draft that lost images', () => {
  it('leave what is said of them when only words are put in, and end it when images are', async () => {
    const { s } = capped();
    vi.stubGlobal('localStorage', s);
    s.setItem(WORDS_AT + 'ask:5', JSON.stringify({ text: 'hi', at: Date.now(), lost: 2 }));
    await show(probes('ask:5'));
    expect(lostShown('ask:5')).toBe('2');
    await act(async () => putDraft('ask:5', { text: 'a quote' }, 'append'));
    expect(words('ask:5')).toBe('hi\na quote');
    expect(lostShown('ask:5')).toBe('2');
    expect(entryOf(s, 'ask:5')).toMatchObject({ lost: 2 });
    await act(async () => putDraft('ask:5', { text: 'with a picture', images: [image(1)] }, 'append'));
    expect(lostShown('ask:5')).toBe('0');
    expect(entryOf(s, 'ask:5')?.lost).toBeUndefined();
    expect(s.getItem(IMAGES_AT + 'ask:5')).not.toBeNull();
  });
});

describe('the line a box draws of images it could not keep', () => {
  const conv: Conversation = {
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
  const entry = (lost: number | undefined) => JSON.stringify({ text: 'hello', at: Date.now(), lost });
  const textarea = (h: HTMLElement): HTMLTextAreaElement => {
    const x = h.querySelector('textarea');
    if (x === null) throw new Error('not drawn');
    return x;
  };

  it('says how many, one or more, in the box of the draft that lost them and in no other, and nothing when none were', async () => {
    const { s } = capped();
    vi.stubGlobal('localStorage', s);
    s.setItem(WORDS_AT + 'conv:work:5', entry(2));
    s.setItem(WORDS_AT + 'conv:work:6', entry(1));
    let h = await show(<Composer c={conv} w={null} />);
    expect(h.textContent).toContain(tn('say_images_lost', 2));
    await leave();
    h = await show(<Composer c={{ ...conv, key: 'work:6', work: 6 }} w={null} />);
    expect(h.textContent).toContain(t('say_images_lost_one'));
    await leave();
    h = await show(<Composer c={{ ...conv, key: 'work:7', work: 7 }} w={null} />);
    expect(h.textContent).not.toContain('could not be kept');
    await leave();
    s.setItem(WORDS_AT + 'conv:work:8', entry(undefined));
    h = await show(<Composer c={{ ...conv, key: 'work:8', work: 8 }} w={null} />);
    expect(textarea(h).value).toBe('hello');
    expect(h.textContent).not.toContain('could not be kept');
  });

  it('goes with the send that came through', async () => {
    const { s } = capped();
    vi.stubGlobal('localStorage', s);
    s.setItem(WORDS_AT + 'conv:work:5', entry(3));
    const h = await show(<Composer c={conv} w={null} />);
    expect(h.textContent).toContain(tn('say_images_lost', 3));
    const send = [...h.querySelectorAll('button')].find((b) => b.textContent?.trim() === t('conv_send'));
    if (send === undefined) throw new Error('not drawn');
    await act(async () => send.click());
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(h.textContent).not.toContain('could not be kept');
    expect(s.getItem(WORDS_AT + 'conv:work:5')).toBeNull();
  });
});
