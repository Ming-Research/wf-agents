import { afterEach, describe, expect, it, vi } from 'vitest';

import { type Heard, REOPEN_MS, type Stream, listen } from './changes';

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
  const open: boolean[] = [];
  const stop = listen(
    (h) => heard.push(h),
    '/api/v1/changes',
    () => {
      const s = new Fake();
      streams.push(s);
      return s;
    },
    (o) => open.push(o),
  );
  return { streams, heard, open, stop };
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

  it('says the stream is lost while it is cut or given up on, and open again once it opens', () => {
    vi.useFakeTimers();
    const { streams, open } = opened();
    streams[0].open();
    streams[0].cut(false);
    streams[0].open();
    streams[0].cut(true);
    vi.advanceTimersByTime(REOPEN_MS);
    streams[1].open();
    expect(open).toEqual([true, false, true, false, true]);
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
