import { afterEach, describe, expect, it } from 'vitest';

import { sourceOf } from './source';

/// A stream of server-sent events that keeps what was opened, as the browser's would be.
class Stream {
  static opened: Stream[] = [];
  onopen: (() => void) | null = null;
  onmessage: ((e: { data: string }) => void) | null = null;
  closed = false;
  constructor(readonly url: string) {
    Stream.opened.push(this);
  }
  close() {
    this.closed = true;
  }
}

const global = globalThis as { EventSource?: unknown };

afterEach(() => {
  delete global.EventSource;
  Stream.opened = [];
});

describe('the stream of changes', () => {
  it('is not opened when the page is answered from the fixtures', () => {
    global.EventSource = Stream;
    const stop = sourceOf('?fixtures').changes(
      () => undefined,
      () => undefined,
    );
    stop();
    expect(Stream.opened).toHaveLength(0);
  });

  it('is opened once for every part that listens, says each change, and that anything may have changed as it opens again', () => {
    global.EventSource = Stream;
    const heard: unknown[] = [];
    const other: unknown[] = [];
    let opens = 0;
    const stop = sourceOf('').changes(
      (c) => heard.push(c),
      () => (opens += 1),
    );
    const stopOther = sourceOf('').changes(
      (c) => other.push(c),
      () => undefined,
    );
    expect(Stream.opened).toHaveLength(1);
    const stream = Stream.opened[0];
    expect(stream.url).toBe('/api/v1/changes');
    stream.onopen?.();
    stream.onmessage?.({ data: '{"works":[3],"items":false,"copies":false}' });
    // What cannot be read says that anything may have happened, as an opening again does.
    stream.onmessage?.({ data: 'not json' });
    stream.onopen?.();
    expect(heard).toEqual([{ works: [3], items: false, copies: false }]);
    expect(other).toEqual(heard);
    expect(opens).toBe(2);
    stop();
    expect(stream.closed).toBe(false);
    stopOther();
    expect(stream.closed).toBe(true);
  });
});
