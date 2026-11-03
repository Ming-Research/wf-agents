// What the owner has typed into a box and not sent, kept where it outlives the box that shows it. A box holds its words
// in a draft under a key made of the number of what the box belongs to (`ask:17`, `card:42`), never of its place on the
// page or its title, so that no box shows another's. The drafts live at module level: a box that is left (another page,
// another conversation) and shown again reads its draft, and one written from outside (`putDraft`) reaches a box that is
// not shown. Every change is written to this browser's `localStorage` at once, with no timer and no hook on leaving, since
// a browser that drops a tab does not promise to run one; the images of a draft go to a key of their own, written when
// they change, so that images too big for the browser cost the words nothing; where the browser keeps nothing, drafts
// live in memory for as long as the page does. A draft older than 30 days is not read, and a draft the box sent is gone
// from memory and from the browser. Another tab that changes or clears a draft is followed (the `storage` event).

import { type SetStateAction, useMemo, useSyncExternalStore } from 'react';

import { IMAGES_MOST, type PendingImage } from './images';

/// How long a draft is kept: a draft nothing sent or cleared (an ask answered elsewhere) goes after this.
export const DRAFT_KEPT_MS = 30 * 24 * 60 * 60 * 1000;

/// The keys of `localStorage` a draft's words and its images are kept under, after the box's key.
const WORDS_AT = 'wf-agents.draft.';
const IMAGES_AT = 'wf-agents.draft-images.';

/// What one box holds. `edited` tells words the owner wrote (even none) from a box nobody has written in, which shows its
/// fallback; `sending` and `error` live in memory only.
type Held = {
  text: string;
  images: PendingImage[];
  to: string[] | null;
  base: string | null;
  edited: boolean;
  sending: boolean;
  error: string | null;
};

const NOTHING: Held = { text: '', images: [], to: null, base: null, edited: false, sending: false, error: null };

/// A box's draft as the box uses it.
export type Draft = {
  /// The key it is kept under.
  readonly key: string;
  /// The words, or the fallback the box was given while nobody has written in it.
  readonly text: string;
  readonly images: PendingImage[];
  /// Whom the words are for, where a box lets the owner tick that; none until ticked.
  readonly to: string[] | null;
  /// What the words were written against (the configuration commit a form was filled from); none unless a box says.
  readonly base: string | null;
  /// Whether a request is out with these words: the box is greyed, also when it is shown again.
  readonly busy: boolean;
  /// Why the last request failed; the words stay.
  readonly error: string | null;
  setText: (text: string) => void;
  /// Like React's: the new images, or a function of the images the draft holds when it runs (an image read after the box
  /// went away still joins the draft).
  setImages: (next: SetStateAction<PendingImage[]>) => void;
  setTo: (to: string[]) => void;
  /// Changes the words and what they were written against in one write.
  write: (fields: { text?: string; base?: string | null }) => void;
  /// Empties the draft, as a send that came through does; the recipients ticked stay.
  clear: () => void;
  /// Sends: `request` is made at once and the box is `busy` until it ends. If it comes through the draft is emptied
  /// there, in the store, so that a box gone away meanwhile does not bring the words back, and `after` runs; if it
  /// fails the words stay and `error` says why. A request already out for this draft is not made again.
  send: (request: () => Promise<unknown>, after?: () => void) => void;
};

/// What is kept in the browser of a draft's words.
type Kept = { text: string; at: number; to?: string[]; base?: string };

const held = new Map<string, Held>();
const watchers = new Set<() => void>();
let swept = false;

/// The browser's storage, or none where it has none or refuses it.
function browser(): Storage | null {
  try {
    const s = typeof window === 'undefined' ? undefined : window.localStorage;
    return s !== undefined && s !== null && typeof s.getItem === 'function' ? s : null;
  } catch {
    return null;
  }
}

/// The words kept in `raw`: none when it is not what `persist` writes.
function parseKept(raw: string): Kept | null {
  let v: unknown;
  try {
    v = JSON.parse(raw);
  } catch {
    return null;
  }
  if (typeof v !== 'object' || v === null) return null;
  const o = v as Record<string, unknown>;
  if (typeof o.text !== 'string' || typeof o.at !== 'number' || !Number.isFinite(o.at)) return null;
  const to = Array.isArray(o.to) && o.to.every((x) => typeof x === 'string') ? (o.to as string[]) : undefined;
  return { text: o.text, at: o.at, to, base: typeof o.base === 'string' ? o.base : undefined };
}

const isImage = (x: unknown): x is PendingImage => {
  if (typeof x !== 'object' || x === null) return false;
  const o = x as Record<string, unknown>;
  return (
    typeof o.key === 'string' &&
    typeof o.name === 'string' &&
    typeof o.media_type === 'string' &&
    typeof o.size === 'number' &&
    typeof o.data_url === 'string'
  );
};

/// The images kept in `raw`: those that are what the box keeps, at most as many as a message carries.
function parseImages(raw: string): PendingImage[] {
  try {
    const v: unknown = JSON.parse(raw);
    return Array.isArray(v) ? v.filter(isImage).slice(0, IMAGES_MOST) : [];
  } catch {
    return [];
  }
}

/// Takes a draft out of the browser, words and images.
function drop(s: Storage, key: string): void {
  try {
    s.removeItem(WORDS_AT + key);
    s.removeItem(IMAGES_AT + key);
  } catch {
    // Nothing is kept where the browser refuses even that.
  }
}

/// What the browser keeps of a draft; an expired or unreadable one is taken out.
function load(key: string): Held {
  const s = browser();
  if (s === null) return NOTHING;
  try {
    const raw = s.getItem(WORDS_AT + key);
    if (raw === null) return NOTHING;
    const kept = parseKept(raw);
    if (kept === null || Date.now() - kept.at > DRAFT_KEPT_MS) {
      drop(s, key);
      return NOTHING;
    }
    const images = s.getItem(IMAGES_AT + key);
    return {
      ...NOTHING,
      text: kept.text,
      images: images === null ? NOTHING.images : parseImages(images),
      to: kept.to ?? null,
      base: kept.base ?? null,
      edited: true,
    };
  } catch {
    return NOTHING;
  }
}

/// Takes out of the browser, once for the page, what nothing will read: drafts past their time, words that cannot be
/// read, and images whose words are gone.
function sweep(): void {
  if (swept) return;
  swept = true;
  const s = browser();
  if (s === null) return;
  try {
    const mine: string[] = [];
    for (let i = 0; i < s.length; i++) {
      const k = s.key(i);
      if (k !== null && (k.startsWith(WORDS_AT) || k.startsWith(IMAGES_AT))) mine.push(k);
    }
    const now = Date.now();
    for (const k of mine) {
      if (k.startsWith(WORDS_AT)) {
        const raw = s.getItem(k);
        const kept = raw === null ? null : parseKept(raw);
        if (kept === null || now - kept.at > DRAFT_KEPT_MS) drop(s, k.slice(WORDS_AT.length));
      } else if (s.getItem(WORDS_AT + k.slice(IMAGES_AT.length)) === null) {
        s.removeItem(k);
      }
    }
  } catch {
    // Left for the next page.
  }
}

/// Keeps a draft in the browser, as far as the browser lets: the words, always; the images only when `withImages` says
/// they changed. A draft with neither words nor images is taken out instead.
function persist(key: string, h: Held, withImages: boolean): void {
  const s = browser();
  if (s === null) return;
  if (h.text === '' && h.images.length === 0) {
    drop(s, key);
    return;
  }
  try {
    const kept: Kept = { text: h.text, at: Date.now(), to: h.to ?? undefined, base: h.base ?? undefined };
    s.setItem(WORDS_AT + key, JSON.stringify(kept));
  } catch {
    // The words stay in memory only.
  }
  if (!withImages) return;
  try {
    if (h.images.length > 0) s.setItem(IMAGES_AT + key, JSON.stringify(h.images));
    else s.removeItem(IMAGES_AT + key);
  } catch {
    // Too big for the browser: the images stay in memory only, and the images kept before, which the box no longer holds
    // as they are, would come back, so they go.
    try {
      s.removeItem(IMAGES_AT + key);
    } catch {
      // Nothing is kept.
    }
  }
}

/// What the store holds of a draft, reading the browser's the first time.
function read(key: string): Held {
  let h = held.get(key);
  if (h === undefined) {
    sweep();
    h = load(key);
    held.set(key, h);
  }
  return h;
}

/// Makes `next` the draft and tells every box showing a draft.
function commit(key: string, next: Held, keep: 'words' | 'images' | 'memory'): void {
  held.set(key, next);
  if (keep !== 'memory') persist(key, next, keep === 'images');
  for (const w of [...watchers]) w();
}

const sameList = (a: readonly string[] | null, b: readonly string[] | null): boolean =>
  a === b || (a !== null && b !== null && a.length === b.length && a.every((x, i) => x === b[i]));

const sameImages = (a: readonly PendingImage[], b: readonly PendingImage[]): boolean =>
  a === b || (a.length === b.length && a.every((x, i) => x.key === b[i].key));

/// Changes fields of a draft; nothing happens, and nobody is told, when nothing changes.
function change(
  key: string,
  patch: { text?: string; images?: PendingImage[]; to?: string[]; base?: string | null },
): void {
  const h = read(key);
  const text = patch.text ?? h.text;
  const images = patch.images ?? h.images;
  const to = patch.to ?? h.to;
  const base = patch.base === undefined ? h.base : patch.base;
  if (h.edited && text === h.text && images === h.images && sameList(to, h.to) && base === h.base) return;
  commit(key, { ...h, text, images, to, base, edited: true }, images !== h.images ? 'images' : 'words');
}

/// The draft emptied: no words, no images, no failure; the recipients and whether a request is out stay as they are.
const emptied = (h: Held): Held => ({ ...NOTHING, to: h.to, sending: h.sending });

/// Empties a draft, from the browser too, wherever its box is or is not shown.
export function clearDraft(key: string): void {
  const h = read(key);
  if (!h.edited && h.error === null && h.images.length === 0) return;
  commit(key, emptied(h), 'words');
}

/// Makes the request for a draft and ends it as `Draft.send` says.
function send(key: string, request: () => Promise<unknown>, after?: () => void): void {
  const before = read(key);
  if (before.sending) return;
  commit(key, { ...before, sending: true, error: null }, 'memory');
  let asked: Promise<unknown>;
  try {
    asked = request();
  } catch (e) {
    asked = Promise.reject(e);
  }
  asked.then(
    () => {
      const now = read(key);
      // Words put in while the request was out (a quote) are not the words sent: they stay.
      if (now.text === before.text && sameImages(now.images, before.images)) commit(key, emptied({ ...now, sending: false }), 'words');
      else commit(key, { ...now, sending: false, error: null }, 'memory');
      after?.();
    },
    (e: unknown) => commit(key, { ...read(key), sending: false, error: e instanceof Error ? e.message : String(e) }, 'memory'),
  );
}

/// Puts words, and images, into the draft under `key` from outside its box (a quote, words taken back): after the words
/// there already (a new line between them when they do not end in one) with `append`, in place of them with `replace`.
/// Images and recipients given are added to the images or replace them, and set; those not given stay. It works
/// whether the box is shown or not: a box shown later reads it, and the browser keeps it as it keeps what is typed.
export function putDraft(
  key: string,
  put: { text: string; images?: PendingImage[]; to?: string[] },
  how: 'append' | 'replace',
): void {
  const h = read(key);
  const text =
    how === 'replace' || h.text === '' ? put.text : h.text.endsWith('\n') ? h.text + put.text : `${h.text}\n${put.text}`;
  const images =
    put.images === undefined ? undefined : (how === 'replace' ? put.images : [...h.images, ...put.images]).slice(0, IMAGES_MOST);
  change(key, { text, images, to: put.to });
}

/// Makes a draft from what the store holds of it: its fields and the ways to change them.
function view(key: string, h: Held, fallback: string): Draft {
  return {
    key,
    text: h.edited ? h.text : fallback,
    images: h.images,
    to: h.to,
    base: h.base,
    busy: h.sending,
    error: h.error,
    setText: (text) => change(key, { text }),
    setImages: (next) => change(key, { images: typeof next === 'function' ? next(read(key).images) : next }),
    setTo: (to) => change(key, { to }),
    write: (fields) => change(key, fields),
    clear: () => clearDraft(key),
    send: (request, after) => send(key, request, after),
  };
}

function subscribe(watcher: () => void): () => void {
  watchers.add(watcher);
  return () => {
    watchers.delete(watcher);
  };
}

/// The draft of the box under `key`: made of the number of what the box belongs to, `ask:17`, so that it is the same box
/// whenever it is shown and no other's. Until the owner writes in it, `fallback` is its words, which are then not a draft
/// and are not kept.
export function useDraft(key: string, fallback = ''): Draft {
  const h = useSyncExternalStore(
    subscribe,
    () => read(key),
    () => NOTHING,
  );
  return useMemo(() => view(key, h, fallback), [key, h, fallback]);
}

/// Reads a draft again from the browser, where another tab changed it, keeping what is only in memory (a request out,
/// why one failed, the recipients when the other tab has none).
function follow(key: string): void {
  const h = held.get(key);
  if (h === undefined) return;
  const kept = load(key);
  const next: Held = {
    ...kept,
    images: sameImages(kept.images, h.images) ? h.images : kept.images,
    to: kept.to ?? h.to,
    sending: h.sending,
    error: h.error,
  };
  const unchanged =
    next.text === h.text && next.images === h.images && next.base === h.base && next.edited === h.edited && sameList(next.to, h.to);
  if (!unchanged) commit(key, next, 'memory');
}

function onStorage(e: StorageEvent): void {
  if (e.key === null) {
    for (const key of [...held.keys()]) follow(key);
    return;
  }
  if (e.key.startsWith(WORDS_AT)) follow(e.key.slice(WORDS_AT.length));
  else if (e.key.startsWith(IMAGES_AT)) follow(e.key.slice(IMAGES_AT.length));
}

if (typeof window !== 'undefined' && typeof window.addEventListener === 'function') {
  window.addEventListener('storage', onStorage);
}

/// Forgets what the store holds in memory, as a page that is loaded again starts without it; what the browser keeps
/// stays, and is read when a draft is next asked for. For the tests, which cannot load the page again.
export function forgetMemory(): void {
  held.clear();
  swept = false;
  for (const w of [...watchers]) w();
}
