import { afterEach, describe, expect, it, vi } from 'vitest';

import { type Heard, REOPEN_MS, type Stream, inOrder, listen } from './source';

/// A stand-in for the browser's `EventSource`: the test opens it, sends on it, cuts it or gives it up.
class Fake implements Stream {
  readyState = 0;
  closed = false;
  onopen: ((e: Event) => void) | null = null;
  onerror: ((e: Event) => void) | null = null;
  onmessage: ((e: MessageEvent<string>) => void) | null = null;
  close() {
    this.closed = true;
    this.readyState = 2;
  }
  open() {
    this.readyState = 1;
    this.onopen?.(new Event('open'));
  }
  send(data: string) {
    this.onmessage?.(new MessageEvent('message', { data }));
  }
  /// Cut: the browser tries again by itself (CONNECTING), or gives up (CLOSED).
  cut(givenUp: boolean) {
    this.readyState = givenUp ? 2 : 0;
    this.onerror?.(new Event('error'));
  }
}

afterEach(() => vi.useRealTimers());

function opened() {
  const streams: Fake[] = [];
  const heard: Heard[] = [];
  const stop = listen(
    (h) => heard.push(h),
    '/api/v1/changes',
    () => {
      const s = new Fake();
      streams.push(s);
      return s;
    },
  );
  return { streams, heard, stop };
}

describe('the stream of changes', () => {
  it('tells what happened, and that anything may have each time the stream opens again after a cut', () => {
    const { streams, heard } = opened();
    streams[0].open();
    expect(heard).toEqual([]);
    streams[0].send(JSON.stringify({ works: [3], items: false, copies: false }));
    expect(heard).toEqual([{ works: [3], items: false, copies: false }]);
    // The browser opens it again by itself, and what happened meanwhile was not heard.
    streams[0].cut(false);
    streams[0].open();
    expect(heard).toEqual([{ works: [3], items: false, copies: false }, 'everything']);
    expect(streams).toHaveLength(1);
  });

  it('opens the stream again itself once the browser gave up on it, and reads everything then', () => {
    vi.useFakeTimers();
    const { streams, heard } = opened();
    streams[0].open();
    streams[0].cut(true);
    expect(streams).toHaveLength(1);
    vi.advanceTimersByTime(REOPEN_MS - 1);
    expect(streams).toHaveLength(1);
    vi.advanceTimersByTime(1);
    expect(streams).toHaveLength(2);
    streams[1].open();
    expect(heard).toEqual(['everything']);
    // Given up on again, it is opened again again.
    streams[1].cut(true);
    vi.advanceTimersByTime(REOPEN_MS);
    expect(streams).toHaveLength(3);
  });

  it('reads everything on an event it cannot read', () => {
    const { streams, heard } = opened();
    streams[0].open();
    streams[0].send('not json');
    expect(heard).toEqual(['everything']);
  });

  it('closes the stream when stopped, and opens no other', () => {
    vi.useFakeTimers();
    const { streams, stop } = opened();
    streams[0].open();
    streams[0].cut(true);
    stop();
    vi.advanceTimersByTime(10 * REOPEN_MS);
    expect(streams).toHaveLength(1);
    const again = opened();
    again.stop();
    expect(again.streams[0].closed).toBe(true);
  });
});

/// Lets every answer given so far be taken.
const flush = () => new Promise<void>((done) => setTimeout(done, 0));

describe('reads in order', () => {
  /// A read the test answers when it likes.
  function deferred<T>() {
    let answer!: (v: T) => void;
    let refuse!: (e: Error) => void;
    const promise = new Promise<T>((yes, no) => {
      answer = yes;
      refuse = no;
    });
    return { promise, answer, refuse };
  }

  it('drops an answer older than the one shown, and a failure counts as an answer', async () => {
    const take = inOrder();
    const shown: string[] = [];
    const first = deferred<string>();
    const second = deferred<string>();
    const third = deferred<string>();
    take(first.promise, (v) => shown.push(v), (why) => shown.push(`failed: ${why}`));
    take(second.promise, (v) => shown.push(v), (why) => shown.push(`failed: ${why}`));
    take(third.promise, (v) => shown.push(v), (why) => shown.push(`failed: ${why}`));
    second.refuse(new Error('gone'));
    await flush();
    first.answer('first');
    await flush();
    third.answer('third');
    await flush();
    expect(shown).toEqual(['failed: gone', 'third']);
  });
});
