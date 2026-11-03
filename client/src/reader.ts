// What the page reads from the interface, and when. An event only says that something a part shows is out of date; the
// read itself is made here, one for each path some part of the page shows, whatever the number of parts that show it
// and of events that name it: one read at a time for a path, one more when it was named while that was under way, and an
// answer that is the text of the last one changes nothing. How often events cause reads is not decided here: what the
// reader is told (`bump`) it reads at once, and `live.ts` tells it what events name once in `FLUSH_MS`. What the page no
// longer shows is not read: the read under way ends with its last part. Nothing here depends on React; `live.ts` makes
// the hooks that parts read through.

import { getBody } from './http';

/// What a path holds as last read: the data (undefined until it is first read) and why the last read failed, if it did.
/// A new object each time either changes, so that a part that reads it draws again only then.
export type Snapshot = { readonly data: unknown; readonly error: string | null };

/// What a path that is not read holds.
export const NOTHING: Snapshot = { data: undefined, error: null };

/// A part that reads a path, and the events that make what it shows out of date: `kind:*` stands for every `kind:<id>`;
/// none: only reading everything again does.
type Reader = { readonly key: string | null };

/// One for each path some part of the page shows.
type Slot = {
  readers: Set<Reader>;
  /// The last answer as text, which a new answer is compared with before it is parsed.
  text: string | null;
  snapshot: Snapshot;
  /// Counts every change to the snapshot across all paths, so that no two states of a path share a number.
  revision: number;
  /// Named by an event since the read under way, or the last, began.
  dirty: boolean;
  running: AbortController | null;
};

const slots = new Map<string, Slot>();
const listeners = new Map<string, Set<() => void>>();
let revisions = 0;

/// What `path` holds as last read: the same object until it changes.
export function peek(path: string): Snapshot {
  return slots.get(path)?.snapshot ?? NOTHING;
}

/// A number that differs for every state `path` has been in, 0 while it is not read.
export function revisionOf(path: string): number {
  return slots.get(path)?.revision ?? 0;
}

/// Calls `onChange` each time what `path` holds changes. Reads nothing: `hold` does.
export function listen(path: string, onChange: () => void): () => void {
  let set = listeners.get(path);
  if (set === undefined) {
    set = new Set();
    listeners.set(path, set);
  }
  const mine = set;
  // An entry of its own, so that the same function listening twice is called twice and leaves once for each.
  const entry = () => onChange();
  mine.add(entry);
  return () => {
    mine.delete(entry);
    if (mine.size === 0 && listeners.get(path) === mine) listeners.delete(path);
  };
}

function notify(path: string): void {
  for (const l of [...(listeners.get(path) ?? [])]) l();
}

/// Reads `path`, and keeps it read, until the returned function is called: it is read at once when no part holds it
/// yet, and again each time an event names `key` (`bump`). With the last part gone the read under way ends and the path
/// is forgotten.
export function hold(path: string, key: string | null): () => void {
  let slot = slots.get(path);
  const fresh = slot === undefined;
  if (slot === undefined) {
    slot = {
      readers: new Set(),
      text: null,
      snapshot: NOTHING,
      revision: ++revisions,
      dirty: false,
      running: null,
    };
    slots.set(path, slot);
  }
  const mine = slot;
  const reader: Reader = { key };
  mine.readers.add(reader);
  // A path nobody read is read now; so is one whose last read failed, since a part that comes to show it shows why.
  if (fresh || mine.snapshot.error !== null) stale(path, mine);
  return () => {
    if (!mine.readers.delete(reader) || mine.readers.size > 0) return;
    mine.running?.abort();
    mine.running = null;
    if (slots.get(path) === mine) slots.delete(path);
  };
}

/// Reads `path` with `listen` and `hold`: for a part that shows it.
export function watch(path: string, key: string | null, onChange: () => void): () => void {
  const unlisten = listen(path, onChange);
  const release = hold(path, key);
  return () => {
    release();
    unlisten();
  };
}

/// Whether an event that named `bumped` makes what a part with `key` shows out of date. A part with no key is made so
/// only by everything being read again.
function names(bumped: readonly string[], key: string | null): boolean {
  if (key === null) return false;
  return bumped.includes(key) || bumped.includes(`${key.split(':')[0]}:*`);
}

/// Says that what `keys` name changed: every path some part shows by one of them is read again at once, or, when a read
/// of it is under way, once when that ends, however many times it is named meanwhile.
export function bump(keys: readonly string[]): void {
  for (const [path, slot] of [...slots]) {
    if ([...slot.readers].some((r) => names(keys, r.key))) stale(path, slot);
  }
}

/// Says that anything read may be out of date, as when events were lost: every path shown is read again.
export function staleAll(): void {
  for (const [path, slot] of [...slots]) stale(path, slot);
}

function stale(path: string, slot: Slot): void {
  slot.dirty = true;
  schedule(path, slot);
}

/// Begins the read of a path that is out of date now, when none is under way; else `finish` does when the one under way
/// ends. There is no timer: nothing here paces reads, only the one at a time.
function schedule(path: string, slot: Slot): void {
  if (slot.dirty && slot.running === null) begin(path, slot);
}

function begin(path: string, slot: Slot): void {
  slot.dirty = false;
  const mine = new AbortController();
  slot.running = mine;
  getBody(path, mine.signal).then(
    (text) => finish(path, slot, mine, text, null),
    (error: unknown) => finish(path, slot, mine, null, error),
  );
}

const why = (error: unknown): string => (error instanceof Error ? error.message : String(error));

/// Takes the answer of the read `mine`, unless it was ended meanwhile: a failure keeps the data and says why; an answer
/// whose text is the last answer's changes nothing; any other is parsed and joined to the data before it (`share`).
function finish(path: string, slot: Slot, mine: AbortController, text: string | null, failed: unknown): void {
  if (slot.running !== mine) return;
  slot.running = null;
  const before = slot.snapshot;
  let next: Snapshot = before;
  if (text === null) {
    const error = why(failed);
    if (before.error !== error) next = { data: before.data, error };
  } else if (text !== slot.text) {
    try {
      const data = share(before.data, JSON.parse(text));
      slot.text = text;
      // A text that differs only in how it is written holds the same data: nothing changed to draw.
      if (data !== before.data || before.error !== null) next = { data, error: null };
    } catch (e) {
      const error = why(e);
      if (before.error !== error) next = { data: before.data, error };
    }
  } else if (before.error !== null) {
    next = { data: before.data, error: null };
  }
  if (next !== before) {
    slot.snapshot = next;
    slot.revision = ++revisions;
    notify(path);
  }
  schedule(path, slot);
}

const isRecord = (x: unknown): x is Record<string, unknown> => typeof x === 'object' && x !== null && !Array.isArray(x);

/// The items of `next` as `share` joins them to those of `prev`: each to the item at its place; and, for an item that is
/// not what is at its place, as when the array slid because items joined or left at its start (the latest entries of
/// a conversation, one more each time), to the item of `prev` that holds exactly what it holds, wherever that was.
function shareItems(prev: unknown[], next: unknown[]): unknown[] {
  // The objects of `prev` by what they hold, found only when an item is not at its place.
  let held: Map<string, unknown> | null = null;
  return next.map((x, i) => {
    const there = i < prev.length ? share(prev[i], x) : x;
    if (i < prev.length && there === prev[i]) return there;
    if (typeof x !== 'object' || x === null) return there;
    if (held === null) {
      held = new Map();
      for (const p of prev) {
        if (typeof p !== 'object' || p === null) continue;
        const text = JSON.stringify(p);
        if (!held.has(text)) held.set(text, p);
      }
    }
    return held.get(JSON.stringify(x)) ?? there;
  });
}

/// `next` with each part that equals the same part of `prev` replaced by it, so that what did not change keeps its
/// identity and a part that shows only that does not draw again; `prev` itself when nothing changed. Equal means equal
/// in value, for what JSON holds: the same type, the same items in the same order, the same members.
export function share(prev: unknown, next: unknown): unknown {
  if (prev === next) return prev;
  if (Array.isArray(next)) {
    if (!Array.isArray(prev)) return next;
    const items = shareItems(prev, next);
    return items.length === prev.length && items.every((x, i) => x === prev[i]) ? prev : items;
  }
  if (isRecord(next)) {
    if (!isRecord(prev)) return next;
    const keys = Object.keys(next);
    const members = keys.map((k): [string, unknown] => [k, Object.hasOwn(prev, k) ? share(prev[k], next[k]) : next[k]]);
    const same = keys.length === Object.keys(prev).length && members.every(([k, v]) => Object.hasOwn(prev, k) && v === prev[k]);
    return same ? prev : Object.fromEntries(members);
  }
  return next;
}
