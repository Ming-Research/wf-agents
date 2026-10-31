// What keeps the page current: the interface's event stream. The page reads a view once, and reads it again only
// when an event names something it shows; the stream's ids let a reconnect miss nothing, and when events were lost
// the interface says to read everything again.

import { useEffect, useRef, useState, useSyncExternalStore } from 'react';

import type { Event, Status } from './api';
import { getJson } from './http';

/// What a running agent did, as its command line reported it, with the round's number in its work.
export type Activity = { round: number; what: string; at: number };

type State = {
  online: boolean;
  status: Status | null;
  /// The last things each work's running agent did, newest last, by work.
  activity: Map<number, Activity[]>;
  /// How many times each thing read was named by an event; a key `kind:*` stands for every `kind:<id>`.
  versions: Map<string, number>;
  /// Bumped when everything is to be read again.
  epoch: number;
};

let state: State = { online: false, status: null, activity: new Map(), versions: new Map(), epoch: 0 };

/// How many of a work's latest activities are kept.
const ACTIVITIES = 8;
const listeners = new Set<() => void>();

function set(next: Partial<State>) {
  state = { ...state, ...next };
  for (const l of listeners) l();
}

function subscribe(l: () => void): () => void {
  listeners.add(l);
  return () => listeners.delete(l);
}

function bump(...keys: string[]) {
  const versions = new Map(state.versions);
  for (const k of keys) versions.set(k, (versions.get(k) ?? 0) + 1);
  set({ versions });
}

/// The things an event names, as the keys views read them by. The System view sums up the works, the items, the
/// architects and what waits for the owner, so every event about one of them names it. A round names the observations
/// too, since the round that merges an item's work closes the observations the item took in.
export function named(e: Event): string[] {
  switch (e.kind) {
    case 'work':
      return ['works', `work:${e.work}`, 'items', 'item:*', 'system'];
    case 'round':
      return [
        'works',
        `work:${e.work}`,
        `round:${e.work}-${e.round}`,
        'items',
        'item:*',
        'messages',
        'system',
        'observations',
      ];
    case 'design_card':
    case 'ask':
      return e.work === null
        ? ['works', 'work:*', 'item:*', 'messages', 'system']
        : ['works', `work:${e.work}`, 'item:*', 'messages', 'system'];
    case 'owner_message':
      return e.work === null ? ['messages', 'work:*', 'system'] : ['messages', `work:${e.work}`, 'system'];
    case 'item':
      return e.item === null ? ['items', 'item:*', 'works', 'system'] : ['items', `item:${e.item}`, 'works', 'system'];
    case 'flow':
      return ['flows'];
    case 'architect':
      return ['architect', 'messages', 'system'];
    case 'observations':
      return ['observations'];
    default:
      return [];
  }
}

function readStatus() {
  getJson<Status>('/api/v1/status').then(
    (status) => set({ status }),
    () => {
      // The stream says when the engine is reachable again.
    },
  );
}

let source: EventSource | null = null;
let ready = false;
/// The id of the last event the stream gave, from which a stream opened again goes on.
let last = '';

/// How long the page waits before it opens the stream again once the browser gave up on it.
export const REOPEN_MS = 3000;

/// Opens the event stream, once for the page's life; the browser reconnects it, sending the last id it saw. A stream
/// the browser gives up on, which it does when it is answered with an error (as a proxy in front of `wf-agents run`
/// answers while it restarts) instead of being cut off, the page opens again itself, from the last event it saw, so
/// that nothing it shows stays behind until the owner reloads it.
export function connect(): void {
  if (source) return;
  const es = new EventSource(last === '' ? '/api/v1/events' : `/api/v1/events?after=${encodeURIComponent(last)}`);
  source = es;
  es.onopen = () => {
    set({ online: true });
    readStatus();
  };
  es.onerror = () => {
    set({ online: false });
    if (es.readyState === EventSource.CLOSED && source === es) {
      source = null;
      setTimeout(connect, REOPEN_MS);
    }
  };
  es.onmessage = (m: MessageEvent<string>) => {
    if (m.lastEventId !== '') last = m.lastEventId;
    let e: Event;
    try {
      e = JSON.parse(m.data) as Event;
    } catch {
      return;
    }
    if (e.kind === 'status') {
      set({ status: e.status, online: true });
    } else if (e.kind === 'activity') {
      const activity = new Map(state.activity);
      const was = (activity.get(e.work) ?? []).filter((a) => a.round === e.round);
      activity.set(e.work, [...was, { round: e.round, what: e.what, at: e.at }].slice(-ACTIVITIES));
      set({ activity });
    } else if (e.kind === 'resync' || (e.kind === 'ready' && ready)) {
      // What was read may be out of date: read everything again.
      set({ epoch: state.epoch + 1 });
      readStatus();
    } else if (e.kind === 'ready') {
      ready = true;
      set({ epoch: state.epoch + 1 });
    } else {
      bump(...named(e));
    }
  };
}

/// Reads again what `keys` name: after the page itself changed it, which no event tells.
export function reread(...keys: string[]): void {
  bump(...keys);
}

/// What a work's running agent did last, newest last; empty when nothing was reported since the page opened.
export function useActivity(work: number): Activity[] {
  return useSyncExternalStore(subscribe, () => state.activity.get(work) ?? NONE);
}

const NONE: Activity[] = [];

export function useOnline(): boolean {
  return useSyncExternalStore(subscribe, () => state.online);
}

/// The engine's status, as last told; the same when the page is drawn apart from a browser, as its tests draw it.
export function useStatus(): Status | null {
  const now = () => state.status;
  return useSyncExternalStore(subscribe, now, now);
}

/// How many times what `key` names has changed, everything read again included.
function version(key: string | null): number {
  if (key === null) return state.epoch;
  const kind = key.split(':')[0];
  return state.epoch * 1_000_000 + (state.versions.get(key) ?? 0) + (state.versions.get(`${kind}:*`) ?? 0);
}

export type Resource<T> = { data: T | undefined; error: string | null };

/// Reads `path` now, and again each time an event names `key`. A null path reads nothing; a null key is read
/// again only when everything is.
export function useResource<T>(key: string | null, path: string | null): Resource<T> {
  const v = useSyncExternalStore(subscribe, () => version(key));
  const [got, setGot] = useState<{ path: string | null; data: T | undefined; error: string | null }>({
    path: null,
    data: undefined,
    error: null,
  });
  useEffect(() => {
    if (path === null) return;
    let live = true;
    getJson<T>(path).then(
      (data) => {
        if (live) setGot({ path, data, error: null });
      },
      (e: unknown) => {
        if (live) {
          setGot((old) => ({
            path,
            data: old.path === path ? old.data : undefined,
            error: e instanceof Error ? e.message : String(e),
          }));
        }
      },
    );
    return () => {
      live = false;
    };
  }, [path, v]);
  return got.path === path ? { data: got.data, error: got.error } : { data: undefined, error: null };
}

/// What each path read by `useShared` held, at the count of changes it was read at.
const shared = new Map<string, { v: number; data: unknown }>();
/// The paths being read for `useShared`, each at the count of changes it is read at.
const reading = new Set<string>();

/// Reads `path` once for every part on the page that asks for it, and again each time an event names `key`, as
/// `useResource` reads it for one part: for what many parts read alike, such as the items every text an agent wrote may
/// name by number. Undefined until it is first read, and while it cannot be.
export function useShared<T>(key: string, path: string): T | undefined {
  const v = useSyncExternalStore(
    subscribe,
    () => version(key),
    () => version(key),
  );
  const got = useSyncExternalStore(
    subscribe,
    () => shared.get(path),
    () => shared.get(path),
  );
  useEffect(() => {
    const asked = `${v} ${path}`;
    if ((shared.get(path)?.v ?? -1) >= v || reading.has(asked)) return;
    reading.add(asked);
    getJson<T>(path).then(
      (data) => {
        reading.delete(asked);
        if ((shared.get(path)?.v ?? -1) > v) return;
        shared.set(path, { v, data });
        for (const l of listeners) l();
      },
      () => reading.delete(asked),
    );
  }, [path, v]);
  return got?.data as T | undefined;
}

/// Reads each of `items` now, and one again each time an event names its key, as `useResource` reads one; the data in
/// the order given once every one has been read. One read already and not named since is not read again.
export function useResources<T>(items: ReadonlyArray<{ key: string; path: string }>): Resource<T[]> {
  const paths = items.map((x) => x.path).join('\n');
  const keys = items.map((x) => x.key);
  const seen = useSyncExternalStore(subscribe, () => keys.map(version).join(' '));
  const [got, setGot] = useState<{ read: Map<string, { v: number; data: T }>; error: string | null }>(() => ({
    read: new Map(),
    error: null,
  }));
  // What was asked for, by path, at the count of changes it was asked at.
  const asked = useRef(new Map<string, number>());
  useEffect(() => {
    const vs = seen === '' ? [] : seen.split(' ').map(Number);
    (paths === '' ? [] : paths.split('\n')).forEach((path, i) => {
      const v = vs[i] ?? 0;
      if (asked.current.get(path) === v) return;
      asked.current.set(path, v);
      getJson<T>(path).then(
        (data) =>
          setGot((old) =>
            (old.read.get(path)?.v ?? -1) > v ? old : { read: new Map(old.read).set(path, { v, data }), error: null },
          ),
        (e: unknown) => setGot((old) => ({ ...old, error: e instanceof Error ? e.message : String(e) })),
      );
    });
  }, [paths, seen]);
  const all = items.map((x) => got.read.get(x.path));
  return all.every((x) => x !== undefined)
    ? { data: all.map((x) => (x as { data: T }).data), error: got.error }
    : { data: undefined, error: got.error };
}

/// The time now, ticking every second while `ticking`, so that what runs or waits shows how long it has.
export function useNow(ticking: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!ticking) return;
    const tick = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(tick);
  }, [ticking]);
  return now;
}
