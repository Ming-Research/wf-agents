// Where the console reads the items and the works, and sends what the owner does: the interface (`works-api.ts`), or,
// when the page's address has `?fixtures`, the made-up items of `fixtures.ts`, so that the console can be looked at
// with no server behind it.

import { usesFixtures } from '../mock-data';
import {
  type Act,
  type Changed,
  type Items,
  type VersionPage,
  type WorkPage,
  act,
  loadItems,
  loadVersion,
  loadWork,
} from '../works-api';
import { fixtureSource } from './fixtures';

/// What the stream says: something happened, or, when it opens again after it was cut, that anything may have, since
/// what happened meanwhile was not heard.
export type Heard = Changed | 'everything';

export interface Source {
  loadItems: () => Promise<Items>;
  loadWork: (work: number) => Promise<WorkPage>;
  /// The content of a version of a block of a work.
  loadVersion: (work: number, block: string, at: number) => Promise<VersionPage>;
  act: (work: number, what: Act) => Promise<unknown>;
  /// Calls `on` each time something happened; gives what stops it.
  changes: (on: (heard: Heard) => void) => () => void;
}

/// What happened, as an event of the stream says it; where it cannot be read, that anything may have.
function changedOf(data: string): Heard {
  try {
    const c = JSON.parse(data) as Partial<Changed>;
    return { works: Array.isArray(c.works) ? c.works : [], items: c.items ?? true, copies: c.copies ?? true };
  } catch {
    return 'everything';
  }
}

/// How long the page waits before it opens the stream again once the browser gave up on it, as the page on `main` did.
export const REOPEN_MS = 3000;

/// `EventSource.CLOSED`: the browser has given up on the stream and will not open it again by itself.
const CLOSED = 2;

/// The part of an `EventSource` the stream is read through.
export type Stream = Pick<EventSource, 'readyState' | 'close'> & {
  onopen: ((e: Event) => void) | null;
  onerror: ((e: Event) => void) | null;
  onmessage: ((e: MessageEvent<string>) => void) | null;
};

/// Listens to the stream of changes at `url`, as the page on `main` listened to its events: the browser opens it again
/// after a cut by itself, and each time it opens after the first, `on` is told that anything may have happened; a stream
/// the browser gives up on, which it does when it is answered with an error instead of being cut, is opened again after
/// `REOPEN_MS`. Gives what stops it.
export function listen(
  on: (heard: Heard) => void,
  url = '/api/v1/changes',
  open: (url: string) => Stream = (u) => new EventSource(u),
): () => void {
  let stream: Stream | null = null;
  let opened = 0;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let stopped = false;
  const connect = () => {
    timer = null;
    if (stopped) return;
    const s = open(url);
    stream = s;
    s.onopen = () => {
      opened += 1;
      if (opened > 1) on('everything');
    };
    s.onmessage = (e) => on(changedOf(e.data));
    s.onerror = () => {
      if (s.readyState !== CLOSED || stream !== s || stopped) return;
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

/// The interface: `GET /api/v1/changes` says, as server-sent events, each time something happened. Where there are no
/// server-sent events, as in a test drawn without a browser, nothing is heard.
export const interfaceSource: Source = {
  loadItems,
  loadWork,
  loadVersion,
  act,
  changes: (on) => (typeof EventSource === 'undefined' ? () => undefined : listen(on)),
};

/// The source the page's address asks for.
export function pageSource(): Source {
  return usesFixtures(window.location.search) ? fixtureSource() : interfaceSource;
}

/// Reads of one kind of thing, of which an answer is taken only when it is newer than the one shown: an answer to a read
/// begun before the read whose answer is shown is dropped, so that an older answer arriving late never replaces a newer
/// one. A failure counts as an answer.
export function inOrder() {
  let begun = 0;
  let shown = 0;
  return <T>(read: Promise<T>, then: (value: T) => void, failed: (why: string) => void) => {
    begun += 1;
    const n = begun;
    read.then(
      (value) => {
        if (n <= shown) return;
        shown = n;
        then(value);
      },
      (e: unknown) => {
        if (n <= shown) return;
        shown = n;
        failed(e instanceof Error ? e.message : String(e));
      },
    );
  };
}
