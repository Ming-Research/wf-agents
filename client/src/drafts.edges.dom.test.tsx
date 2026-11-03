// @vitest-environment happy-dom
// Edges of the store of drafts (`drafts.ts`) that the other tests leave out, counted in a DOM and in the `localStorage`
// of the DOM: a draft of images alone, a box whose key changes while it is shown, the day a draft expires, and sends of
// two drafts that are out at once, which must not touch each other.

import './dom-setup';

import { act, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { DRAFT_KEPT_MS, type Draft, forgetMemory, useDraft } from './drafts';
import type { PendingImage } from './images';

const WORDS_AT = 'wf-agents.draft.';
const IMAGES_AT = 'wf-agents.draft-images.';

const seen = new Map<string, Draft>();

/// A box that shows its draft as text, its images by their keys and whether it is greyed.
function Probe({ k, fallback }: { k: string; fallback?: string }) {
  const d = useDraft(k, fallback);
  seen.set(k, d);
  return (
    <p data-busy={String(d.busy)}>
      <span>{d.text}</span>
      <i>{d.images.map((i) => i.key).join(',')}</i>
    </p>
  );
}

let host: HTMLElement | null = null;
let root: Root | null = null;

async function show(ui: ReactNode): Promise<void> {
  host = document.createElement('div');
  document.body.append(host);
  const mine = createRoot(host);
  root = mine;
  await act(async () => mine.render(ui));
}

async function leave(): Promise<void> {
  const mine = root;
  if (mine !== null) await act(async () => mine.unmount());
  host?.remove();
  root = null;
  host = null;
}

const words = (n = 0): string => host?.querySelectorAll('span')[n]?.textContent ?? '(not drawn)';
const imagesShown = (n = 0): string => host?.querySelectorAll('i')[n]?.textContent ?? '(not drawn)';
const busy = (n = 0): string => host?.querySelectorAll('p')[n]?.getAttribute('data-busy') ?? '(not drawn)';

const draftOf = (k: string): Draft => {
  const d = seen.get(k);
  if (d === undefined) throw new Error(`no probe under ${k}`);
  return d;
};

const image = (n: number): PendingImage => ({
  key: `image-e-${n}`,
  name: `shot-${n}.png`,
  media_type: 'image/png',
  size: 10,
  data_url: `data:image/png;base64,AA${n}`,
});

function request(): { promise: Promise<void>; ok: () => void; fail: (why: string) => void } {
  let ok!: () => void;
  let fail!: (why: string) => void;
  const promise = new Promise<void>((resolve, reject) => {
    ok = resolve;
    fail = (why) => reject(new Error(why));
  });
  return { promise, ok, fail };
}

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

beforeEach(() => {
  vi.stubGlobal('localStorage', storage());
  forgetMemory();
  seen.clear();
});

afterEach(async () => {
  await leave();
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe('a draft of images alone', () => {
  it('is kept in the browser, is back after the page is loaded again, and is emptied, images too, by a send that came through', async () => {
    await show(<Probe k="conv:work:5" />);
    await act(async () => draftOf('conv:work:5').setImages([image(1), image(2)]));
    expect(words()).toBe('');
    expect(imagesShown()).toBe('image-e-1,image-e-2');
    expect(localStorage.getItem(IMAGES_AT + 'conv:work:5')).not.toBeNull();
    await leave();
    forgetMemory();
    await show(<Probe k="conv:work:5" />);
    expect(imagesShown()).toBe('image-e-1,image-e-2');
    const r = request();
    await act(async () => draftOf('conv:work:5').send(() => r.promise));
    r.ok();
    await act(async () => r.promise);
    expect(imagesShown()).toBe('');
    expect(localStorage.getItem(IMAGES_AT + 'conv:work:5')).toBeNull();
    expect(localStorage.getItem(WORDS_AT + 'conv:work:5')).toBeNull();
    await leave();
    forgetMemory();
    await show(<Probe k="conv:work:5" />);
    expect(imagesShown()).toBe('');
  });

  it('takes one image out and keeps the others, also after a reload', async () => {
    await show(<Probe k="ask:3" />);
    await act(async () => draftOf('ask:3').setImages([image(1), image(2), image(3)]));
    await act(async () => draftOf('ask:3').setImages((now) => now.filter((x) => x.key !== 'image-e-2')));
    await leave();
    forgetMemory();
    await show(<Probe k="ask:3" />);
    expect(imagesShown()).toBe('image-e-1,image-e-3');
  });
});

describe('a box whose key changes while it is shown', () => {
  it("shows the draft of the new key, and what is typed after goes to the new key alone", async () => {
    await show(<Probe k="ask:1" />);
    await act(async () => draftOf('ask:1').setText('for one'));
    await act(async () => {
      root?.render(<Probe k="ask:2" />);
    });
    expect(words()).toBe('');
    await act(async () => draftOf('ask:2').setText('for two'));
    expect(localStorage.getItem(WORDS_AT + 'ask:1')).toContain('for one');
    expect(localStorage.getItem(WORDS_AT + 'ask:2')).toContain('for two');
    await act(async () => {
      root?.render(<Probe k="ask:1" />);
    });
    expect(words()).toBe('for one');
  });

  it('shows a new fallback for a box nobody has written in, and the owner’s words, not the fallback, for one somebody has', async () => {
    await show(<Probe k="flow-compare-ref" fallback="main" />);
    expect(words()).toBe('main');
    await act(async () => {
      root?.render(<Probe k="flow-compare-ref" fallback="release" />);
    });
    expect(words()).toBe('release');
    await act(async () => draftOf('flow-compare-ref').setText('mine'));
    await act(async () => {
      root?.render(<Probe k="flow-compare-ref" fallback="other" />);
    });
    expect(words()).toBe('mine');
  });
});

describe('the day a draft expires', () => {
  it('is the 30th day after the last change: kept to the millisecond, gone one millisecond later', async () => {
    vi.useFakeTimers({ toFake: ['Date'] });
    vi.setSystemTime(1_800_000_000_000);
    const written = Date.now();
    localStorage.setItem(WORDS_AT + 'ask:1', JSON.stringify({ text: 'on the last day', at: written }));
    localStorage.setItem(WORDS_AT + 'ask:2', JSON.stringify({ text: 'a day too late', at: written }));
    vi.setSystemTime(written + DRAFT_KEPT_MS);
    await show(<Probe k="ask:1" />);
    expect(words()).toBe('on the last day');
    await leave();
    forgetMemory();
    vi.setSystemTime(written + DRAFT_KEPT_MS + 1);
    await show(<Probe k="ask:2" />);
    expect(words()).toBe('');
    expect(localStorage.getItem(WORDS_AT + 'ask:2')).toBeNull();
  });

  it('starts again at each change: words changed on the 29th day are kept to the 59th', async () => {
    vi.useFakeTimers({ toFake: ['Date'] });
    vi.setSystemTime(1_800_000_000_000);
    const start = Date.now();
    await show(<Probe k="ask:1" />);
    await act(async () => draftOf('ask:1').setText('first'));
    vi.setSystemTime(start + DRAFT_KEPT_MS - 1000);
    await act(async () => draftOf('ask:1').setText('second'));
    await leave();
    forgetMemory();
    vi.setSystemTime(start + 2 * DRAFT_KEPT_MS - 2000);
    await show(<Probe k="ask:1" />);
    expect(words()).toBe('second');
  });
});

describe('two sends out at once', () => {
  it('grey only their own boxes, end on their own, and a failure of one leaves the words of the other as they are', async () => {
    await show(
      <>
        <Probe k="ask:1" />
        <Probe k="ask:2" />
      </>,
    );
    await act(async () => draftOf('ask:1').setText('one'));
    await act(async () => draftOf('ask:2').setText('two'));
    const a = request();
    const b = request();
    await act(async () => draftOf('ask:1').send(() => a.promise));
    await act(async () => draftOf('ask:2').send(() => b.promise));
    expect([busy(0), busy(1)]).toEqual(['true', 'true']);
    a.fail('refused');
    await act(async () => a.promise.catch(() => undefined));
    expect([busy(0), busy(1)]).toEqual(['false', 'true']);
    expect([words(0), words(1)]).toEqual(['one', 'two']);
    b.ok();
    await act(async () => b.promise);
    expect([busy(0), busy(1)]).toEqual(['false', 'false']);
    expect([words(0), words(1)]).toEqual(['one', '']);
    expect(localStorage.getItem(WORDS_AT + 'ask:1')).toContain('one');
    expect(localStorage.getItem(WORDS_AT + 'ask:2')).toBeNull();
  });
});
