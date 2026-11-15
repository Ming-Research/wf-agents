// The engine's stream of changes, `GET /api/v1/changes`, which says as server-sent events each time something happened,
// and on which every part of the page reads again what it shows. The page opens it once, for every part that listens:
// a browser keeps only a few connections to one host open at once, and a stream for each part took them all. A stream
// the browser gives up on is opened again after a few seconds, and while it is lost the page says so.

import { useSyncExternalStore } from 'react';

import type { Changed } from './works-api';

/// What the stream says: something happened, or, when it opens again after it was cut, that anything may have, since
/// what happened meanwhile was not heard.
export type Heard = Changed | 'everything';

/// What happened, as an event of the stream says it; where it cannot be read, that anything may have.
function changedOf(data: string): Heard {
  try {
    const c = JSON.parse(data) as Partial<Changed>;
    return { works: Array.isArray(c.works) ? c.works : [], items: c.items ?? true, copies: c.copies ?? true };
  } catch {
    return 'everything';
  }
}

/// How long the page waits before it opens the stream again once the browser gave up on it.
export const REOPEN_MS = 3000;

/// `EventSource.OPEN` and `EventSource.CLOSED`: the stream is open; the browser has given up on it and will not open it
/// again by itself.
const OPEN = 1;
const CLOSED = 2;

/// The part of an `EventSource` the stream is read through.
export type Stream = Pick<EventSource, 'readyState' | 'close'> & {
  onopen: ((e: Event) => void) | null;
  onerror: ((e: Event) => void) | null;
  onmessage: ((e: MessageEvent<string>) => void) | null;
};

/// Listens to the stream of changes at `url`: the browser opens it again after a cut by itself, and each time it opens
/// after the first, `on` is told that anything may have happened; a stream the browser gives up on, which it does when
/// it is answered with an error instead of being cut, is opened again after `REOPEN_MS`. `open` is told whether the
/// stream is open each time that changes. Gives what stops it.
export function listen(
  on: (heard: Heard) => void,
  url = '/api/v1/changes',
  connectTo: (url: string) => Stream = (u) => new EventSource(u),
  open: (open: boolean) => void = () => undefined,
): () => void {
  let stream: Stream | null = null;
  let opened = 0;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let stopped = false;
  const connect = () => {
    timer = null;
    if (stopped) return;
    const s = connectTo(url);
    stream = s;
    s.onopen = () => {
      opened += 1;
      open(true);
      if (opened > 1) on('everything');
    };
    s.onmessage = (e) => on(changedOf(e.data));
    s.onerror = () => {
      if (stream !== s || stopped) return;
      if (s.readyState !== OPEN) open(false);
      if (s.readyState !== CLOSED) return;
      stream = null;
      timer = setTimeout(connect, REOPEN_MS);
    };
  };
  connect();
  return () => {
    stopped = true;
    if (timer !== null) clearTimeout(timer);
    stream?.close();
    stream = null;
  };
}

/// Everything that listens to the page's one stream, whether the stream is open, and what stops it.
const listening = new Set<(heard: Heard) => void>();
const watching = new Set<() => void>();
let lost = false;
let stop: (() => void) | null = null;

function setLost(now: boolean) {
  if (lost === now) return;
  lost = now;
  [...watching].forEach((tell) => tell());
}

/// Listens to the page's one stream of changes, opening it for the first that listens and closing it after the last;
/// gives what stops it. Where there are no server-sent events, as in a test drawn without a browser, nothing is heard.
export function listenShared(on: (heard: Heard) => void): () => void {
  if (typeof EventSource === 'undefined') return () => undefined;
  listening.add(on);
  if (stop === null) {
    stop = listen(
      (heard) => [...listening].forEach((l) => l(heard)),
      '/api/v1/changes',
      undefined,
      (open) => setLost(!open),
    );
  }
  return () => {
    listening.delete(on);
    if (listening.size === 0 && stop !== null) {
      stop();
      stop = null;
      setLost(false);
    }
  };
}

/// Whether the page's stream of changes is lost now: what the page shows may then be older than what is.
export function useStreamLost(): boolean {
  return useSyncExternalStore(
    (tell) => {
      watching.add(tell);
      return () => watching.delete(tell);
    },
    () => lost,
  );
}
