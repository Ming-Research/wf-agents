// What keeps the page current: the interface's event stream. The page reads a view once, and reads it again only
// when an event names something it shows; the stream's ids let a reconnect miss nothing, and when events were lost
// the interface says to read everything again. An event only says that a read is out of date: the reads themselves, one
// at a time for each path (`reader.ts`), are what the hooks here give the parts. How often events cause reads is
// `coalesced`'s: once in `FLUSH_MS`, never while the tab is hidden; what the page changes itself (`reread`) and
// everything read again after lost events are read at once.

import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from 'react';

import type { Event, Status } from './api';
import { getJson } from './http';
import { bump, hold, listen, NOTHING, peek, revisionOf, staleAll, watch } from './reader';

/// What a running agent did, as its command line reported it, with the round's number in its work.
export type Activity = { round: number; what: string; at: number };

/// How a running round stands after its last call to the model, as the event stream told it.
export type Measure = { context: number; added: number; cost_usd: number };

type State = {
  online: boolean;
  status: Status | null;
  /// The last things each work's running agent did, newest last, by work.
  activity: Map<number, Activity[]>;
  /// How each running round stands, by `measureKey`.
  measures: Map<string, Measure>;
};

let state: State = { online: false, status: null, activity: new Map(), measures: new Map() };

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

/// How long what events name waits before it is read again. A busy engine sends several events a second, and a view
/// can take half a second to read and more to draw: read at each event, the reads pile up behind one another until the
/// owner's page and the machine stop.
export const FLUSH_MS = 3000;

/// Gathers what is given to it and hands it to `run` at most once in `FLUSH_MS`, each key once, and not while `hidden`
/// says so: a tab the owner is not looking at reads nothing until it is looked at.
export function coalesced(run: (keys: string[]) => void, hidden: () => boolean): (keys: string[]) => void {
  const pending = new Set<string>();
  let waiting = false;
  const flush = () => {
    if (hidden()) {
      setTimeout(flush, FLUSH_MS);
      return;
    }
    waiting = false;
    const keys = [...pending];
    pending.clear();
    run(keys);
  };
  return (keys) => {
    for (const k of keys) pending.add(k);
    if (waiting) return;
    waiting = true;
    setTimeout(flush, FLUSH_MS);
  };
}

/// What events name, handed to the reader (`reader.ts`) once in `FLUSH_MS`, never while the tab is hidden. What the
/// reader is given is read at once, one read at a time for each path; the pace of what events cause is this one's alone.
const bumpSoon = coalesced(
  (keys) => bump(keys),
  () => typeof document !== 'undefined' && document.hidden,
);

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
    } else if (e.kind === 'measure') {
      const measures = new Map(state.measures);
      measures.set(measureKey(e.work, e.round), { context: e.context, added: e.added, cost_usd: e.cost_usd });
      set({ measures });
    } else if (e.kind === 'resync' || (e.kind === 'ready' && ready)) {
      // What was read may be out of date: read everything again.
      staleAll();
      readStatus();
    } else if (e.kind === 'ready') {
      ready = true;
      staleAll();
    } else {
      bumpSoon(named(e));
    }
  };
}

/// Reads again what `keys` name: after the page itself changed it, which no event tells.
export function reread(...keys: string[]): void {
  bump(keys);
}

/// How the running rounds stand as the event stream last told it: what came since the page read them.
export function useMeasures(): Map<string, Measure> {
  return useSyncExternalStore(subscribe, () => state.measures);
}

/// What `useMeasures` keeps a round under.
export const measureKey = (work: number, round: number): string => `${work}-${round}`;

/// What a work's running agent did last, newest last; empty when nothing was reported since the page opened.
export function useActivity(work: number): Activity[] {
  return useSyncExternalStore(subscribe, () => state.activity.get(work) ?? NONE);
}

const NONE: Activity[] = [];

export function useOnline(): boolean {
  return useSyncExternalStore(subscribe, () => state.online);
}

/// The engine's status, as last told, while the event stream is open; none while it is cut off or has not opened, since
/// the page cannot tell then what the engine does: the bar says the engine is offline, and no work is marked as working.
/// The same when the page is drawn apart from a browser, as its tests draw it.
export function useStatus(): Status | null {
  return useSyncExternalStore(subscribe, onlineStatus, onlineStatus);
}

/// The status of the engine while the stream is open, else none.
function onlineStatus(): Status | null {
  return state.online ? state.status : null;
}

export type Resource<T> = {
  data: T | undefined;
  error: string | null;
  /// Whether `data` is what was read at the path the part showed before this one (`ReadOptions.staleOk`), the new path
  /// not read yet.
  stale: boolean;
};

/// How a part reads a path.
export type ReadOptions = {
  /// Keep showing what was read at the path the part showed before, until the new path is read, instead of nothing: for
  /// a part that checks the data is what it is to show, as the conversation open does by its key.
  staleOk?: boolean;
};

const nothing = (): void => undefined;

/// What `path` holds as read for a part that shows it, which reads it while it does (`key` names the events that make it
/// out of date): nothing for no path.
function useRead(key: string | null, path: string | null) {
  const subscribeTo = useCallback(
    (onChange: () => void) => (path === null ? nothing : watch(path, key, onChange)),
    [key, path],
  );
  const get = () => (path === null ? NOTHING : peek(path));
  return useSyncExternalStore(subscribeTo, get, get);
}

/// Reads `path` now, and again each time an event names `key`, one read at a time for each path however many parts show
/// it. A null path reads nothing; a null key is read again only when everything is. While the path was changed and the
/// new one is not read yet, the data is none, or, with `staleOk`, what was read at the one before, marked `stale`.
export function useResource<T>(key: string | null, path: string | null, options: ReadOptions = {}): Resource<T> {
  const read = useRead(key, path);
  // What was last read at the path this part showed, for `staleOk`.
  const shown = useRef<{ path: string; data: T } | null>(null);
  if (path !== null && read.data !== undefined) shown.current = { path, data: read.data as T };
  const before = shown.current;
  if (read.data === undefined && options.staleOk === true && path !== null && before !== null && before.path !== path) {
    return { data: before.data, error: read.error, stale: true };
  }
  return { data: read.data as T | undefined, error: read.error, stale: false };
}

/// What `path` holds, read once for every part on the page that asks for it, and again each time an event names `key`,
/// as `useResource` reads it for one part: for what many parts read alike, such as the items every text an agent wrote
/// may name by number. Undefined until it is first read, and while it cannot be.
export function useShared<T>(key: string, path: string): T | undefined {
  return useRead(key, path).data as T | undefined;
}

/// The paths read ahead, which stay read for the page's life.
const ahead = new Set<string>();

/// Reads `path` for `useShared` before any part asks for it, and keeps it read: for what the page reads as it starts, so
/// that it is there once a part is drawn.
export function readAhead(key: string, path: string): void {
  if (ahead.has(path)) return;
  ahead.add(path);
  hold(path, key);
}

/// The lines of a `useResources` spec: each item's key, a space and its path.
const specLines = (spec: string): string[] => (spec === '' ? [] : spec.split('\n'));
const pathOf = (line: string): string => line.slice(line.indexOf(' ') + 1);

/// Reads each of `items` now, and one again each time an event names its key, as `useResource` reads one; the data in
/// the order given once every one has been read. An item that stays among `items` as they change is not read again.
export function useResources<T>(items: ReadonlyArray<{ key: string; path: string }>): Resource<T[]> {
  const spec = items.map((x) => `${x.key} ${x.path}`).join('\n');
  const paths = items.map((x) => x.path).join('\n');
  // Draws again when any of them changes.
  const subscribeTo = useCallback(
    (onChange: () => void) => {
      const stops = (paths === '' ? [] : paths.split('\n')).map((p) => listen(p, onChange));
      return () => {
        for (const stop of stops) stop();
      };
    },
    [paths],
  );
  const changes = () => (paths === '' ? '' : paths.split('\n').map(revisionOf).join(' '));
  useSyncExternalStore(subscribeTo, changes, changes);
  // What it holds read, by item line; one that stays as the items change is held on, not read again.
  const held = useRef(new Map<string, () => void>());
  useEffect(() => {
    const wanted = specLines(spec);
    for (const [line, release] of held.current) {
      if (!wanted.includes(line)) {
        release();
        held.current.delete(line);
      }
    }
    for (const line of wanted) {
      if (!held.current.has(line)) held.current.set(line, hold(pathOf(line), line.slice(0, line.indexOf(' '))));
    }
  }, [spec]);
  useEffect(() => {
    const mine = held.current;
    return () => {
      for (const release of mine.values()) release();
      mine.clear();
    };
  }, []);
  const read = specLines(spec).map((line) => peek(pathOf(line)));
  const error = read.find((x) => x.error !== null)?.error ?? null;
  return {
    data: read.every((x) => x.data !== undefined) ? read.map((x) => x.data as T) : undefined,
    error,
    stale: false,
  };
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
