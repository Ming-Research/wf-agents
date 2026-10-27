// What keeps the page current: the interface's event stream. The page reads a view once, and reads it again only
// when an event names something it shows; the stream's ids let a reconnect miss nothing, and when events were lost
// the interface says to read everything again.

import { useEffect, useState, useSyncExternalStore } from 'react';

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

/// The things an event names, as the keys views read them by.
export function named(e: Event): string[] {
  switch (e.kind) {
    case 'work':
      return ['works', `work:${e.work}`, 'items', 'item:*'];
    case 'round':
      return ['works', `work:${e.work}`, `round:${e.work}-${e.round}`, 'items', 'item:*'];
    case 'design_card':
    case 'ask':
      return e.work === null ? ['works', 'work:*', 'item:*'] : ['works', `work:${e.work}`, 'item:*'];
    case 'owner_message':
      return e.work === null ? ['messages', 'work:*'] : ['messages', `work:${e.work}`];
    case 'item':
      return e.item === null ? ['items', 'item:*', 'works'] : ['items', `item:${e.item}`, 'works'];
    case 'flow':
      return ['flows'];
    case 'architect':
      return ['architect'];
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

/// Opens the event stream, once for the page's life; the browser reconnects it, sending the last id it saw.
export function connect(): void {
  if (source) return;
  source = new EventSource('/api/v1/events');
  source.onopen = () => {
    set({ online: true });
    readStatus();
  };
  source.onerror = () => set({ online: false });
  source.onmessage = (m: MessageEvent<string>) => {
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

export function useStatus(): Status | null {
  return useSyncExternalStore(subscribe, () => state.status);
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
