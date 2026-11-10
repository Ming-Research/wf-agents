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

  it('is opened on the interface, says each change and each time it opens, and is closed when no longer wanted', () => {
    global.EventSource = Stream;
    const changes: unknown[] = [];
    let opens = 0;
    const stop = sourceOf('').changes(
      (c) => changes.push(c),
      () => (opens += 1),
    );
    const stream = Stream.opened[0];
    expect(stream.url).toBe('/api/v1/changes');
    stream.onopen?.();
    stream.onmessage?.({ data: '{"works":[3],"items":false}' });
    stream.onmessage?.({ data: 'not json' });
    stream.onopen?.();
    expect(changes).toEqual([{ works: [3], items: false }]);
    expect(opens).toBe(2);
    stop();
    expect(stream.closed).toBe(true);
  });
});
