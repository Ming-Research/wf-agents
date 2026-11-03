// @vitest-environment happy-dom
// What the owner has typed and not sent, kept by the store of drafts (`drafts.ts`) and shown by a box that is left and
// shown again, counted in a DOM and in the `localStorage` of the DOM: a draft is the box's under a key made of what the box
// belongs to, is written at each change, survives the page being loaded again, is emptied by the send that came through
// wherever its box is, stays when the send failed, and is followed across tabs. A box here is a probe that shows the
// draft it is given as text; the boxes of the page are in boxes.dom.test.tsx.

import './dom-setup';

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { clearDraft, DRAFT_KEPT_MS, type Draft, forgetMemory, putDraft, useDraft } from './drafts';
import { IMAGES_MOST, type PendingImage } from './images';

const WORDS_AT = 'wf-agents.draft.';
const IMAGES_AT = 'wf-agents.draft-images.';

/// The latest draft each probe was given, by key: what a test calls to type, send or clear.
const seen = new Map<string, Draft>();

/// A box that shows its draft as text, its images by their keys, and whether it is greyed, and what failed.
function Probe({ k, fallback }: { k: string; fallback?: string }) {
  const d = useDraft(k, fallback);
  seen.set(k, d);
  return (
    <p data-draft={k} data-busy={String(d.busy)} data-to={d.to === null ? '' : d.to.join(',')} data-lost={d.lostImages}>
      <span>{d.text}</span>
      <i>{d.images.map((i) => i.key).join(',')}</i>
      <b>{d.error ?? ''}</b>
    </p>
  );
}

let host: HTMLElement | null = null;
let root: Root | null = null;

/// Draws the probes of `keys` in the document.
async function show(...keys: string[]): Promise<void> {
  host = document.createElement('div');
  document.body.append(host);
  const mine = createRoot(host);
  root = mine;
  await act(async () => mine.render(<>{keys.map((k) => <Probe key={k} k={k} />)}</>));
}

/// Takes the probes away, as leaving a page or a conversation does.
async function leave(): Promise<void> {
  const mine = root;
  if (mine !== null) await act(async () => mine.unmount());
  host?.remove();
  root = null;
  host = null;
}

/// What a probe shows: its words, the keys of its images, what failed, whether it is greyed.
const words = (k: string): string => host?.querySelector(`[data-draft="${k}"] span`)?.textContent ?? '(not drawn)';
const imagesShown = (k: string): string => host?.querySelector(`[data-draft="${k}"] i`)?.textContent ?? '(not drawn)';
const failure = (k: string): string => host?.querySelector(`[data-draft="${k}"] b`)?.textContent ?? '(not drawn)';
const busy = (k: string): string => host?.querySelector(`[data-draft="${k}"]`)?.getAttribute('data-busy') ?? '(not drawn)';
/// How many images lost a box says.
const lostShown = (k: string): string => host?.querySelector(`[data-draft="${k}"]`)?.getAttribute('data-lost') ?? '(not drawn)';

/// The draft of the probe under `k`.
const draftOf = (k: string): Draft => {
  const d = seen.get(k);
  if (d === undefined) throw new Error(`no probe under ${k}`);
  return d;
};

/// What the browser keeps of the words of a draft, parsed: none for nothing.
const kept = (k: string): { text: string; at: number; to?: string[]; base?: string } | null => {
  const raw = localStorage.getItem(WORDS_AT + k);
  return raw === null ? null : (JSON.parse(raw) as { text: string; at: number });
};

const image = (n: number): PendingImage => ({ key: `image-t-${n}`, name: `shot-${n}.png`, media_type: 'image/png', size: 10, data_url: `data:image/png;base64,AA${n}` });

/// A promise held back until it is settled by the test.
function request(): { promise: Promise<void>; ok: () => void; fail: (why: string) => void } {
  let ok!: () => void;
  let fail!: (why: string) => void;
  const promise = new Promise<void>((resolve, reject) => {
    ok = resolve;
    fail = (why) => reject(new Error(why));
  });
  return { promise, ok, fail };
}

/// A `storage` event as another tab's change gives it: the key, and the storage already changed.
function otherTab(key: string | null): void {
  const e = new Event('storage');
  Object.assign(e, { key });
  window.dispatchEvent(e);
}

/// A storage that holds what it is given in memory and refuses a write to a key `refuse` says: the way a full browser
/// does. The tests run on one of their own, since Node may have a `localStorage` of its own that the DOM does not replace.
function storage(refuse: (key: string) => boolean = () => false): Storage {
  const m = new Map<string, string>();
  return {
    get length() {
      return m.size;
    },
    key: (i: number) => [...m.keys()][i] ?? null,
    getItem: (k: string) => m.get(k) ?? null,
    setItem: (k: string, v: string) => {
      if (refuse(k)) throw new DOMException('full', 'QuotaExceededError');
      m.set(k, v);
    },
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

describe('a draft of a box that goes away and is shown again', () => {
  it('is the same words, written to the browser at the change with no timer, and back after the page is loaded again', async () => {
    await show('ask:1');
    await act(async () => {
      draftOf('ask:1').setText('three paragraphs');
      // Written in the same turn as the change: no timer, no microtask and no leaving of the page came between.
      expect(kept('ask:1')?.text).toBe('three paragraphs');
    });
    await leave();
    await show('ask:1');
    expect(words('ask:1')).toBe('three paragraphs');
    // The store forgets what it holds in memory, as a page loaded again starts without it; only the browser's remains.
    await leave();
    forgetMemory();
    await show('ask:1');
    expect(words('ask:1')).toBe('three paragraphs');
  });

  it('keeps the images too, in a key of their own written when they change, and the recipients ticked', async () => {
    await show('conv:work:5');
    await act(async () => {
      draftOf('conv:work:5').setText('see');
      draftOf('conv:work:5').setImages([image(1)]);
      draftOf('conv:work:5').setTo(['engineer', 'architect']);
    });
    expect(JSON.parse(localStorage.getItem(IMAGES_AT + 'conv:work:5') ?? '[]')).toHaveLength(1);
    await leave();
    forgetMemory();
    await show('conv:work:5');
    expect(words('conv:work:5')).toBe('see');
    expect(imagesShown('conv:work:5')).toBe('image-t-1');
    expect(host?.querySelector('[data-draft]')?.getAttribute('data-to')).toBe('engineer,architect');
    // An image added by a function of the images the draft holds (as a file read late is) joins them, not replaces.
    await act(async () => draftOf('conv:work:5').setImages((now) => [...now, image(2)]));
    expect(imagesShown('conv:work:5')).toBe('image-t-1,image-t-2');
    // Taking them all out takes the images' key out; words and images both gone takes the draft out.
    await act(async () => draftOf('conv:work:5').setImages([]));
    expect(localStorage.getItem(IMAGES_AT + 'conv:work:5')).toBeNull();
    await act(async () => draftOf('conv:work:5').setText(''));
    expect(localStorage.getItem(WORDS_AT + 'conv:work:5')).toBeNull();
  });

  it("is never another box's: keys of numbers that begin alike, and two conversations, do not meet", async () => {
    const keys = ['ask:1', 'ask:11', 'question:1:2', 'question:1:22', 'conv:work:5', 'conv:work:50', 'card:1', 'obs-close:1'];
    await show(...keys);
    await act(async () => {
      for (const k of keys) draftOf(k).setText(`for ${k}`);
    });
    for (const k of keys) expect(words(k)).toBe(`for ${k}`);
    expect(new Set(keys.map((k) => localStorage.getItem(WORDS_AT + k))).size).toBe(keys.length);
    // Emptying one leaves the others.
    await act(async () => draftOf('ask:1').clear());
    expect(words('ask:1')).toBe('');
    expect(words('ask:11')).toBe('for ask:11');
    expect(localStorage.getItem(WORDS_AT + 'ask:1')).toBeNull();
    expect(localStorage.getItem(WORDS_AT + 'ask:11')).not.toBeNull();
  });

  it('shows its fallback until the owner writes in it, and then what the owner wrote even when that is nothing', async () => {
    host = document.createElement('div');
    document.body.append(host);
    root = createRoot(host);
    const mine = root;
    await act(async () => mine.render(<Probe k="flow-compare-ref" fallback="main" />));
    expect(words('flow-compare-ref')).toBe('main');
    // The fallback is no draft.
    expect(kept('flow-compare-ref')).toBeNull();
    await act(async () => draftOf('flow-compare-ref').setText('release'));
    expect(words('flow-compare-ref')).toBe('release');
    await act(async () => draftOf('flow-compare-ref').setText(''));
    expect(words('flow-compare-ref')).toBe('');
    await act(async () => draftOf('flow-compare-ref').clear());
    expect(words('flow-compare-ref')).toBe('main');
  });
});

describe('a send', () => {
  it('that came through empties the draft in the store: the box shown again, even after the page is loaded again, is empty', async () => {
    await show('ask:2');
    await act(async () => draftOf('ask:2').setText('my answer'));
    const r = request();
    const after = vi.fn();
    await act(async () => draftOf('ask:2').send(() => r.promise, after));
    expect(busy('ask:2')).toBe('true');
    // The box goes away while the request is on its way, and comes back while it still is: it is greyed, with its words.
    await leave();
    await show('ask:2');
    expect(busy('ask:2')).toBe('true');
    expect(words('ask:2')).toBe('my answer');
    // The box goes away again, the request comes through, and the box shown then is empty: nothing brings the words back.
    await leave();
    await act(async () => r.ok());
    expect(after).toHaveBeenCalledTimes(1);
    expect(kept('ask:2')).toBeNull();
    await show('ask:2');
    expect(words('ask:2')).toBe('');
    expect(busy('ask:2')).toBe('false');
    await leave();
    forgetMemory();
    await show('ask:2');
    expect(words('ask:2')).toBe('');
  });

  it('that failed leaves the words and says why, also in a box shown again; a new send clears what failed', async () => {
    await show('card:3');
    await act(async () => draftOf('card:3').setText('long reply'));
    const r = request();
    await act(async () => draftOf('card:3').send(() => r.promise));
    await act(async () => r.fail('the interface refused it'));
    expect(words('card:3')).toBe('long reply');
    expect(failure('card:3')).toBe('the interface refused it');
    expect(busy('card:3')).toBe('false');
    await leave();
    await show('card:3');
    expect(words('card:3')).toBe('long reply');
    expect(failure('card:3')).toBe('the interface refused it');
    expect(kept('card:3')?.text).toBe('long reply');
    const again = request();
    await act(async () => draftOf('card:3').send(() => again.promise));
    expect(failure('card:3')).toBe('');
    await act(async () => again.ok());
    expect(words('card:3')).toBe('');
  });

  it('that throws before it returns a promise is a failure too, and one out is not made twice', async () => {
    await show('obs-close:4');
    await act(async () => draftOf('obs-close:4').setText('why'));
    await act(async () =>
      draftOf('obs-close:4').send(() => {
        throw new Error('no address');
      }),
    );
    expect(failure('obs-close:4')).toBe('no address');
    expect(words('obs-close:4')).toBe('why');
    const r = request();
    const made = vi.fn(() => r.promise);
    await act(async () => {
      draftOf('obs-close:4').send(made);
      draftOf('obs-close:4').send(made);
    });
    expect(made).toHaveBeenCalledTimes(1);
    await act(async () => r.ok());
  });

  it('keeps words put in while it was out, which are not the words sent', async () => {
    await show('conv:work:5');
    await act(async () => draftOf('conv:work:5').setText('sent'));
    const r = request();
    await act(async () => draftOf('conv:work:5').send(() => r.promise));
    await act(async () => putDraft('conv:work:5', { text: '> a quote' }, 'append'));
    await act(async () => r.ok());
    expect(words('conv:work:5')).toBe('sent\n> a quote');
    expect(busy('conv:work:5')).toBe('false');
  });

  it('keeps the recipients ticked after it came through', async () => {
    await show('conv:work:5');
    await act(async () => {
      draftOf('conv:work:5').setText('x');
      draftOf('conv:work:5').setTo(['architect']);
    });
    const r = request();
    await act(async () => draftOf('conv:work:5').send(() => r.promise));
    await act(async () => r.ok());
    expect(words('conv:work:5')).toBe('');
    expect(host?.querySelector('[data-draft]')?.getAttribute('data-to')).toBe('architect');
  });
});

describe('a browser that keeps little or nothing', () => {
  it('keeps the words in memory for the page when it refuses every write, and throws nothing', async () => {
    vi.stubGlobal('localStorage', storage(() => true));
    await show('ask:5');
    await act(async () => draftOf('ask:5').setText('kept in memory'));
    await act(async () => draftOf('ask:5').setImages([image(1)]));
    await leave();
    await show('ask:5');
    expect(words('ask:5')).toBe('kept in memory');
    expect(imagesShown('ask:5')).toBe('image-t-1');
    // It still sends, and empties.
    const r = request();
    await act(async () => draftOf('ask:5').send(() => r.promise));
    await act(async () => r.ok());
    expect(words('ask:5')).toBe('');
  });

  it('keeps the words in memory for the page when the browser refuses even to be asked, and throws nothing', async () => {
    const real = Object.getOwnPropertyDescriptor(globalThis, 'localStorage');
    Object.defineProperty(globalThis, 'localStorage', {
      configurable: true,
      get() {
        throw new DOMException('denied', 'SecurityError');
      },
    });
    try {
      forgetMemory();
      await show('ask:6');
      await act(async () => draftOf('ask:6').setText('no storage'));
      await leave();
      await show('ask:6');
      expect(words('ask:6')).toBe('no storage');
      await act(async () => clearDraft('ask:6'));
      expect(words('ask:6')).toBe('');
    } finally {
      if (real !== undefined) Object.defineProperty(globalThis, 'localStorage', real);
    }
  });

  it('writes the words when the images do not fit, and the images kept before do not come back', async () => {
    vi.stubGlobal('localStorage', storage((k) => k.startsWith(IMAGES_AT) && k.includes('big')));
    const fake = window.localStorage;
    await show('conv:big');
    await act(async () => draftOf('conv:big').setText('the words'));
    await act(async () => draftOf('conv:big').setImages([image(1)]));
    expect(JSON.parse(fake.getItem(WORDS_AT + 'conv:big') ?? '{}').text).toBe('the words');
    expect(fake.getItem(IMAGES_AT + 'conv:big')).toBeNull();
    // They are still in the box for as long as the page is.
    expect(imagesShown('conv:big')).toBe('image-t-1');
    // Words typed after go on being written.
    await act(async () => draftOf('conv:big').setText('the words, more'));
    expect(JSON.parse(fake.getItem(WORDS_AT + 'conv:big') ?? '{}').text).toBe('the words, more');
    await leave();
    forgetMemory();
    await show('conv:big');
    expect(words('conv:big')).toBe('the words, more');
    expect(imagesShown('conv:big')).toBe('');
  });

  it("drops the images kept before when the new ones do not fit, since they are not the box's images any more", async () => {
    let full = false;
    vi.stubGlobal('localStorage', storage((k) => full && k.startsWith(IMAGES_AT)));
    const fake = window.localStorage;
    await show('ask:7');
    await act(async () => draftOf('ask:7').setImages([image(1)]));
    expect(fake.getItem(IMAGES_AT + 'ask:7')).not.toBeNull();
    full = true;
    await act(async () => draftOf('ask:7').setImages([image(2)]));
    expect(fake.getItem(IMAGES_AT + 'ask:7')).toBeNull();
  });
});

describe('a draft another tab changes', () => {
  it('is followed: the words, the images and a clear, in the box shown and in one that is not', async () => {
    await show('ask:8');
    await act(async () => draftOf('ask:8').setText('mine'));
    // Another tab writes its words under the key and tells this one.
    localStorage.setItem(WORDS_AT + 'ask:8', JSON.stringify({ text: 'theirs', at: Date.now() }));
    await act(async () => otherTab(WORDS_AT + 'ask:8'));
    expect(words('ask:8')).toBe('theirs');
    localStorage.setItem(IMAGES_AT + 'ask:8', JSON.stringify([image(3)]));
    await act(async () => otherTab(IMAGES_AT + 'ask:8'));
    expect(imagesShown('ask:8')).toBe('image-t-3');
    // It sent them there: the draft is gone, and so are the words here.
    localStorage.removeItem(WORDS_AT + 'ask:8');
    localStorage.removeItem(IMAGES_AT + 'ask:8');
    await act(async () => otherTab(WORDS_AT + 'ask:8'));
    expect(words('ask:8')).toBe('');
    expect(imagesShown('ask:8')).toBe('');
    // A change here is not written back by the event: the browser keeps what the other tab left.
    expect(localStorage.getItem(WORDS_AT + 'ask:8')).toBeNull();
  });

  it('is followed when the whole storage was cleared, and leaves a request that is out as it was', async () => {
    await show('ask:9', 'ask:10');
    await act(async () => {
      draftOf('ask:9').setText('one');
      draftOf('ask:10').setText('two');
    });
    const r = request();
    await act(async () => draftOf('ask:10').send(() => r.promise));
    localStorage.clear();
    await act(async () => otherTab(null));
    expect(words('ask:9')).toBe('');
    expect(words('ask:10')).toBe('');
    expect(busy('ask:10')).toBe('true');
    await act(async () => r.ok());
    expect(busy('ask:10')).toBe('false');
  });

  it("is left alone when the change is to a key that is no draft's, or to a draft no box has read", async () => {
    await show('ask:11');
    await act(async () => draftOf('ask:11').setText('mine'));
    localStorage.setItem('wf-agents.items-states', '[]');
    await act(async () => otherTab('wf-agents.items-states'));
    localStorage.setItem(WORDS_AT + 'ask:12', JSON.stringify({ text: 'unread', at: Date.now() }));
    await act(async () => otherTab(WORDS_AT + 'ask:12'));
    expect(words('ask:11')).toBe('mine');
    await leave();
    await show('ask:12');
    expect(words('ask:12')).toBe('unread');
  });
});

describe('words put in from outside', () => {
  it('append to, or replace, the words of a box that is shown, and are kept in the browser', async () => {
    await show('conv:work:5');
    await act(async () => draftOf('conv:work:5').setText('first line'));
    await act(async () => putDraft('conv:work:5', { text: '> quote\n\n' }, 'append'));
    expect(words('conv:work:5')).toBe('first line\n> quote\n\n');
    expect(kept('conv:work:5')?.text).toBe('first line\n> quote\n\n');
    // Words that end in a new line need no other between them.
    await act(async () => putDraft('conv:work:5', { text: 'more' }, 'append'));
    expect(words('conv:work:5')).toBe('first line\n> quote\n\nmore');
    await act(async () => putDraft('conv:work:5', { text: 'taken back' }, 'replace'));
    expect(words('conv:work:5')).toBe('taken back');
    expect(kept('conv:work:5')?.text).toBe('taken back');
  });

  it('reach a box that is not shown, which shows them when it is, also after the page is loaded again', async () => {
    putDraft('conv:work:6', { text: 'taken back', images: [image(1)], to: ['engineer'] }, 'replace');
    putDraft('conv:work:6', { text: 'and a quote' }, 'append');
    expect(kept('conv:work:6')?.text).toBe('taken back\nand a quote');
    await show('conv:work:6');
    expect(words('conv:work:6')).toBe('taken back\nand a quote');
    expect(imagesShown('conv:work:6')).toBe('image-t-1');
    expect(host?.querySelector('[data-draft]')?.getAttribute('data-to')).toBe('engineer');
    await leave();
    forgetMemory();
    await show('conv:work:6');
    expect(words('conv:work:6')).toBe('taken back\nand a quote');
    expect(imagesShown('conv:work:6')).toBe('image-t-1');
  });

  it('add images up to the most a message carries, and replace them with replace', async () => {
    putDraft('ask:13', { text: 'a', images: Array.from({ length: IMAGES_MOST - 1 }, (_, i) => image(i)) }, 'replace');
    putDraft('ask:13', { text: 'b', images: [image(100), image(101), image(102)] }, 'append');
    await show('ask:13');
    expect(imagesShown('ask:13').split(',')).toHaveLength(IMAGES_MOST);
    await act(async () => putDraft('ask:13', { text: 'c', images: [image(200)] }, 'replace'));
    expect(imagesShown('ask:13')).toBe('image-t-200');
  });
});

describe('what the browser keeps of a draft', () => {
  it('is not read once it is older than 30 days, and is taken out; younger is read', async () => {
    const now = Date.now();
    localStorage.setItem(WORDS_AT + 'ask:20', JSON.stringify({ text: 'old', at: now - DRAFT_KEPT_MS - 1000 }));
    localStorage.setItem(IMAGES_AT + 'ask:20', JSON.stringify([image(1)]));
    localStorage.setItem(WORDS_AT + 'ask:21', JSON.stringify({ text: 'young', at: now - DRAFT_KEPT_MS + 60_000 }));
    await show('ask:20', 'ask:21');
    expect(words('ask:20')).toBe('');
    expect(imagesShown('ask:20')).toBe('');
    expect(words('ask:21')).toBe('young');
    expect(localStorage.getItem(WORDS_AT + 'ask:20')).toBeNull();
    expect(localStorage.getItem(IMAGES_AT + 'ask:20')).toBeNull();
  });

  it('is taken out, without reading, when it is not what is written: no JSON, no words, no time, images of another shape', async () => {
    localStorage.setItem(WORDS_AT + 'ask:22', '{not json');
    localStorage.setItem(WORDS_AT + 'ask:23', JSON.stringify({ at: Date.now() }));
    localStorage.setItem(WORDS_AT + 'ask:24', JSON.stringify({ text: 'no time' }));
    localStorage.setItem(WORDS_AT + 'ask:25', JSON.stringify(['text']));
    localStorage.setItem(WORDS_AT + 'ask:26', JSON.stringify({ text: 'words', at: Date.now() }));
    localStorage.setItem(IMAGES_AT + 'ask:26', JSON.stringify([image(1), { key: 'x' }, 7, null]));
    localStorage.setItem(WORDS_AT + 'ask:27', JSON.stringify({ text: 'words', at: Date.now(), to: [1, 2] }));
    localStorage.setItem(IMAGES_AT + 'ask:27', '{not json');
    await show('ask:22', 'ask:23', 'ask:24', 'ask:25', 'ask:26', 'ask:27');
    for (const k of ['ask:22', 'ask:23', 'ask:24', 'ask:25']) {
      expect(words(k)).toBe('');
      expect(localStorage.getItem(WORDS_AT + k)).toBeNull();
    }
    // Words with images that are not what the box keeps keep the words and the images that are.
    expect(words('ask:26')).toBe('words');
    expect(imagesShown('ask:26')).toBe('image-t-1');
    // Recipients that are not names are not recipients; images that are no JSON are no images.
    expect(words('ask:27')).toBe('words');
    expect(imagesShown('ask:27')).toBe('');
    expect(host?.querySelector('[data-draft="ask:27"]')?.getAttribute('data-to')).toBe('');
  });

  it('is swept once for the page: drafts past their time and images with no words go, though no box reads them', async () => {
    const now = Date.now();
    localStorage.setItem(WORDS_AT + 'ask:30', JSON.stringify({ text: 'old', at: now - DRAFT_KEPT_MS - 1000 }));
    localStorage.setItem(IMAGES_AT + 'ask:30', JSON.stringify([image(1)]));
    localStorage.setItem(IMAGES_AT + 'ask:31', JSON.stringify([image(2)]));
    localStorage.setItem(WORDS_AT + 'ask:32', JSON.stringify({ text: 'young', at: now }));
    localStorage.setItem('wf-agents.items-states', '[]');
    await show('ask:99');
    expect(localStorage.getItem(WORDS_AT + 'ask:30')).toBeNull();
    expect(localStorage.getItem(IMAGES_AT + 'ask:30')).toBeNull();
    expect(localStorage.getItem(IMAGES_AT + 'ask:31')).toBeNull();
    expect(localStorage.getItem(WORDS_AT + 'ask:32')).not.toBeNull();
    expect(localStorage.getItem('wf-agents.items-states')).toBe('[]');
  });
});

/// A storage with room for `box.limit` characters, keys and values together, that throws a quota error for a write that
/// does not fit, as a full browser does (taking something out makes room, which the storage that refuses by key cannot show);
/// `box.quotaOn` makes it refuse the keys it says as full, and `box.denyOn` as forbidden, which no deleting cures.
function capped() {
  const m = new Map<string, string>();
  const box = { limit: Infinity, quotaOn: null as ((k: string) => boolean) | null, denyOn: null as ((k: string) => boolean) | null };
  const used = (): number => {
    let n = 0;
    for (const [k, v] of m) n += k.length + v.length;
    return n;
  };
  const s: Storage = {
    get length() {
      return m.size;
    },
    key: (i: number) => [...m.keys()][i] ?? null,
    getItem: (k: string) => m.get(k) ?? null,
    setItem: (k: string, v: string) => {
      if (box.denyOn?.(k)) throw new DOMException('denied', 'SecurityError');
      const before = m.has(k) ? k.length + (m.get(k) ?? '').length : 0;
      if (box.quotaOn?.(k) || used() - before + k.length + v.length > box.limit) throw new DOMException('full', 'QuotaExceededError');
      m.set(k, v);
    },
    removeItem: (k: string) => void m.delete(k),
    clear: () => m.clear(),
  };
  return { s, box, used };
}

/// An image whose bytes take up about a thousand characters of a storage.
const big = (n: number): PendingImage => ({ ...image(n), data_url: `data:image/png;base64,${'A'.repeat(1000)}` });

/// The entry a storage holds of the words of draft `k`, parsed.
const entryOf = (s: Storage, k: string): { text?: string; at?: number; lost?: number } | null => {
  const raw = s.getItem(WORDS_AT + k);
  return raw === null ? null : (JSON.parse(raw) as { text?: string; at?: number; lost?: number });
};

describe('words that the browser is full for', () => {
  /// Puts a draft with `images` in `s` as the store writes one, `at` the time it was last changed.
  const seed = (s: Storage, k: string, at: number, images: PendingImage[]) => {
    s.setItem(WORDS_AT + k, JSON.stringify({ text: k, at }));
    s.setItem(IMAGES_AT + k, JSON.stringify(images));
  };

  it('are written by taking out the images of the draft changed longest ago, and no other, and it records how many', async () => {
    const { s, box, used } = capped();
    vi.stubGlobal('localStorage', s);
    const now = Date.now();
    seed(s, 'old', now - 2000, [big(1), big(2)]);
    seed(s, 'new', now - 1000, [big(3)]);
    box.limit = used() + 120;
    await show('mine');
    await act(async () => draftOf('mine').setText('y'.repeat(400)));
    expect(s.getItem(IMAGES_AT + 'old')).toBeNull();
    expect(s.getItem(IMAGES_AT + 'new')).not.toBeNull();
    // The time it was changed is as it was, since the order drafts give way in and the 30 days go by it.
    expect(entryOf(s, 'old')).toEqual({ text: 'old', at: now - 2000, lost: 2 });
    expect(entryOf(s, 'new')).toEqual({ text: 'new', at: now - 1000 });
    expect(entryOf(s, 'mine')?.text).toBe('y'.repeat(400));
    await leave();
    forgetMemory();
    await show('mine');
    expect(words('mine')).toBe('y'.repeat(400));
  });

  it("takes out the draft's own images last, and says in its own entry how many", async () => {
    const { s, box, used } = capped();
    vi.stubGlobal('localStorage', s);
    await show('mine');
    await act(async () => {
      draftOf('mine').setText('a');
      draftOf('mine').setImages([big(1)]);
    });
    expect(s.getItem(IMAGES_AT + 'mine')).not.toBeNull();
    box.limit = used() + 50;
    await act(async () => draftOf('mine').setText('z'.repeat(300)));
    expect(s.getItem(IMAGES_AT + 'mine')).toBeNull();
    expect(entryOf(s, 'mine')).toMatchObject({ text: 'z'.repeat(300), lost: 1 });
    // Still in the box, so that nothing is said of them on this page.
    expect(imagesShown('mine')).toBe('image-t-1');
    expect(lostShown('mine')).toBe('0');
    await leave();
    forgetMemory();
    await show('mine');
    expect(words('mine')).toBe('z'.repeat(300));
    expect(imagesShown('mine')).toBe('');
    expect(lostShown('mine')).toBe('1');
  });

  it('stay in memory, with no exception, when there are no images left to take out and they still do not fit', async () => {
    const { s, box, used } = capped();
    vi.stubGlobal('localStorage', s);
    seed(s, 'other', Date.now() - 1000, [big(1)]);
    box.limit = used() + 5;
    await show('mine');
    await act(async () => draftOf('mine').setText('q'.repeat(5000)));
    expect(s.getItem(IMAGES_AT + 'other')).toBeNull();
    expect(entryOf(s, 'mine')).toBeNull();
    await leave();
    await show('mine');
    expect(words('mine')).toBe('q'.repeat(5000));
  });

  it('take no image out when the write fails for another reason than a full browser', async () => {
    const { s, box } = capped();
    vi.stubGlobal('localStorage', s);
    seed(s, 'other', Date.now() - 1000, [big(1)]);
    box.denyOn = (k) => k === WORDS_AT + 'mine';
    await show('mine');
    await act(async () => draftOf('mine').setText('x'));
    expect(s.getItem(IMAGES_AT + 'other')).not.toBeNull();
    expect(entryOf(s, 'other')).toEqual(expect.not.objectContaining({ lost: expect.anything() }));
    expect(words('mine')).toBe('x');
  });

  it("leave the images of a box that is shown in it, and its next write records how many it has lost", async () => {
    vi.useFakeTimers({ toFake: ['Date'] });
    const { s, box, used } = capped();
    vi.stubGlobal('localStorage', s);
    await show('old', 'new', 'mine');
    vi.setSystemTime(Date.now() + 10);
    await act(async () => {
      draftOf('old').setText('o');
      draftOf('old').setImages([big(1), big(2)]);
    });
    vi.setSystemTime(Date.now() + 10);
    await act(async () => {
      draftOf('new').setText('n');
      draftOf('new').setImages([big(3)]);
    });
    vi.setSystemTime(Date.now() + 10);
    box.limit = used() + 120;
    await act(async () => draftOf('mine').setText('y'.repeat(400)));
    expect(s.getItem(IMAGES_AT + 'old')).toBeNull();
    expect(s.getItem(IMAGES_AT + 'new')).not.toBeNull();
    // The box of the draft that gave way still has its images and says nothing of them; it keeps the count in memory.
    expect(imagesShown('old')).toBe('image-t-1,image-t-2');
    expect(lostShown('old')).toBe('0');
    box.limit = Infinity;
    await act(async () => draftOf('old').setText('o, more'));
    expect(entryOf(s, 'old')).toMatchObject({ text: 'o, more', lost: 2 });
    vi.useRealTimers();
  });
});

describe('images the browser did not keep', () => {
  it('are said after the page is loaded again and not on the page that has them, until an image is added or taken out', async () => {
    const { s, box } = capped();
    vi.stubGlobal('localStorage', s);
    box.quotaOn = (k) => k.startsWith(IMAGES_AT);
    await show('ask:40');
    await act(async () => {
      draftOf('ask:40').setText('hi');
      draftOf('ask:40').setImages([image(1), image(2)]);
    });
    expect(lostShown('ask:40')).toBe('0');
    expect(imagesShown('ask:40')).toBe('image-t-1,image-t-2');
    expect(entryOf(s, 'ask:40')?.lost).toBe(2);
    await act(async () => draftOf('ask:40').setText('hi there'));
    expect(entryOf(s, 'ask:40')).toMatchObject({ text: 'hi there', lost: 2 });
    await leave();
    forgetMemory();
    await show('ask:40');
    expect(lostShown('ask:40')).toBe('2');
    expect(imagesShown('ask:40')).toBe('');
    // Typing does not end it.
    await act(async () => draftOf('ask:40').setText('hi there, more'));
    expect(lostShown('ask:40')).toBe('2');
    expect(entryOf(s, 'ask:40')?.lost).toBe(2);
    // An image added, and kept this time, ends it, in the browser too.
    box.quotaOn = null;
    await act(async () => draftOf('ask:40').setImages([image(3)]));
    expect(lostShown('ask:40')).toBe('0');
    expect(entryOf(s, 'ask:40')?.lost).toBeUndefined();
    expect(s.getItem(IMAGES_AT + 'ask:40')).not.toBeNull();
  });

  it('leave the entry of a draft of images alone, until it is emptied, sent or past its 30 days', async () => {
    const { s, box } = capped();
    vi.stubGlobal('localStorage', s);
    box.quotaOn = (k) => k.startsWith(IMAGES_AT);
    await show('ask:41');
    await act(async () => draftOf('ask:41').setImages([image(1)]));
    expect(entryOf(s, 'ask:41')).toMatchObject({ text: '', lost: 1 });
    await leave();
    forgetMemory();
    await show('ask:41');
    expect(lostShown('ask:41')).toBe('1');
    expect(entryOf(s, 'ask:41')).not.toBeNull();
    // Emptied, it is gone from the browser.
    await act(async () => draftOf('ask:41').clear());
    expect(lostShown('ask:41')).toBe('0');
    expect(entryOf(s, 'ask:41')).toBeNull();
    // One sent ends it too.
    await act(async () => draftOf('ask:41').setImages([image(2)]));
    await leave();
    forgetMemory();
    await show('ask:41');
    expect(lostShown('ask:41')).toBe('1');
    const r = request();
    await act(async () => draftOf('ask:41').send(() => r.promise));
    await act(async () => r.ok());
    expect(lostShown('ask:41')).toBe('0');
    expect(entryOf(s, 'ask:41')).toBeNull();
    // Past its 30 days the entry is not read, as any other.
    await act(async () => draftOf('ask:41').setImages([image(3)]));
    s.setItem(WORDS_AT + 'ask:41', JSON.stringify({ text: '', at: Date.now() - DRAFT_KEPT_MS - 1000, lost: 1 }));
    await leave();
    forgetMemory();
    await show('ask:41');
    expect(lostShown('ask:41')).toBe('0');
    expect(entryOf(s, 'ask:41')).toBeNull();
  });

  it("are not taken for the owner's clear when another tab took them out to make room for its words", async () => {
    const { s } = capped();
    vi.stubGlobal('localStorage', s);
    await show('ask:42');
    await act(async () => {
      draftOf('ask:42').setText('mine');
      draftOf('ask:42').setImages([image(1)]);
    });
    // Another tab takes the images out of the browser and records how many.
    s.removeItem(IMAGES_AT + 'ask:42');
    s.setItem(WORDS_AT + 'ask:42', JSON.stringify({ text: 'mine', at: Date.now(), lost: 1 }));
    await act(async () => otherTab(IMAGES_AT + 'ask:42'));
    await act(async () => otherTab(WORDS_AT + 'ask:42'));
    expect(imagesShown('ask:42')).toBe('image-t-1');
    expect(lostShown('ask:42')).toBe('0');
    // What this page writes next still records them as not kept, once.
    await act(async () => draftOf('ask:42').setText('mine, more'));
    expect(entryOf(s, 'ask:42')).toMatchObject({ text: 'mine, more', lost: 1 });
    // The other tab sending the draft is a clear: the images go.
    s.removeItem(WORDS_AT + 'ask:42');
    await act(async () => otherTab(WORDS_AT + 'ask:42'));
    expect(imagesShown('ask:42')).toBe('');
    expect(words('ask:42')).toBe('');
  });
});
