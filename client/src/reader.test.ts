// What the page reads from the interface, one read at a time for each path: however many parts show a path and however
// many events name it, a read under way is followed by one more at most, no sooner than the gap after the one before;
// an answer that is the last one's text changes nothing, and any other joins the data before it; with the last part
// gone, the read under way ends.

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { bump, hold, listen, MIN_GAP_MS, NOTHING, peek, share, staleAll } from './reader';

/// A request the interface has not answered yet: the test answers it, or fails it.
type Asked = { path: string; signal: AbortSignal | undefined; answer: (text: string) => void; fail: (why: string) => void };
let asked: Asked[];

beforeEach(() => {
  vi.useFakeTimers();
  asked = [];
  vi.stubGlobal(
    'fetch',
    (path: string, init?: RequestInit) =>
      new Promise<Response>((resolve, reject) => {
        const signal = init?.signal ?? undefined;
        signal?.addEventListener('abort', () => reject(new DOMException('aborted', 'AbortError')));
        asked.push({
          path,
          signal,
          answer: (text) => resolve(new Response(text, { status: 200 })),
          fail: (why) => resolve(new Response(JSON.stringify({ error: why }), { status: 500 })),
        });
      }),
  );
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

/// Lets what an answer set going (the promises that carry it, the body of the response among them) run, without time
/// passing.
async function settle(): Promise<void> {
  for (let i = 0; i < 5; i++) await vi.advanceTimersByTimeAsync(0);
}

describe('the reader', () => {
  it('reads a path once for any number of parts, and once more for any number of events that came meanwhile, after the gap', async () => {
    const path = '/api/v1/test/once';
    const release = [hold(path, 'works'), hold(path, 'works')];
    expect(asked.map((x) => x.path)).toEqual([path]);
    // 220 events while the read is under way: no second read, however many.
    for (let i = 0; i < 220; i++) bump(['works']);
    expect(asked.length).toBe(1);
    asked[0].answer('[1]');
    await settle();
    expect(peek(path).data).toEqual([1]);
    // The gap since the read began is not over: the one more read waits for it.
    expect(asked.length).toBe(1);
    await vi.advanceTimersByTimeAsync(MIN_GAP_MS - 1);
    expect(asked.length).toBe(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(asked.length).toBe(2);
    asked[1].answer('[2]');
    await settle();
    expect(peek(path).data).toEqual([2]);
    // Nothing named it since: no more reads.
    await vi.advanceTimersByTimeAsync(10 * MIN_GAP_MS);
    expect(asked.length).toBe(2);
    // Quiet for longer than the gap, an event reads it at once.
    bump(['works']);
    expect(asked.length).toBe(3);
    for (const r of release) r();
  });

  it('names a path by the keys of the parts that show it: `kind:*` for every `kind:<id>`, and none by an event', async () => {
    const path = '/api/v1/test/keys';
    const release = [hold(path, 'work:7'), hold('/api/v1/test/keyless', null)];
    asked[0].answer('1');
    asked[1].answer('1');
    await settle();
    await vi.advanceTimersByTimeAsync(MIN_GAP_MS);
    bump(['work:8', 'works', 'item:*']);
    expect(asked.length).toBe(2);
    bump(['work:*']);
    expect(asked.map((x) => x.path)).toEqual([path, '/api/v1/test/keyless', path]);
    asked[2].answer('2');
    await settle();
    await vi.advanceTimersByTimeAsync(MIN_GAP_MS);
    // Everything shown is read again when events were lost.
    staleAll();
    expect(asked.map((x) => x.path).slice(3)).toEqual([path, '/api/v1/test/keyless']);
    for (const r of release) r();
  });

  it('tells its listeners only when what a path holds changed, and shares what did not change', async () => {
    const path = '/api/v1/test/share';
    const changed = vi.fn();
    const stop = listen(path, changed);
    const release = hold(path, 'items');
    asked[0].answer('[{"id":1,"n":1},{"id":2,"n":2}]');
    await settle();
    expect(changed).toHaveBeenCalledTimes(1);
    const first = peek(path);
    const items = first.data as Array<{ id: number; n: number }>;
    // The same text: the same snapshot, nobody told.
    bump(['items']);
    await vi.advanceTimersByTimeAsync(MIN_GAP_MS);
    asked[1].answer('[{"id":1,"n":1},{"id":2,"n":2}]');
    await settle();
    expect(peek(path)).toBe(first);
    expect(changed).toHaveBeenCalledTimes(1);
    // The same data written another way: nothing to draw again.
    bump(['items']);
    await vi.advanceTimersByTimeAsync(MIN_GAP_MS);
    asked[2].answer('[ {"n":1,"id":1}, {"n":2,"id":2} ]');
    await settle();
    expect(peek(path)).toBe(first);
    expect(changed).toHaveBeenCalledTimes(1);
    // One item changed: it is new, the other is the one it was.
    bump(['items']);
    await vi.advanceTimersByTimeAsync(MIN_GAP_MS);
    asked[3].answer('[{"id":1,"n":1},{"id":2,"n":3}]');
    await settle();
    const now = peek(path).data as Array<{ id: number; n: number }>;
    expect(changed).toHaveBeenCalledTimes(2);
    expect(now[0]).toBe(items[0]);
    expect(now[1]).not.toBe(items[1]);
    expect(now[1]).toEqual({ id: 2, n: 3 });
    release();
    stop();
  });

  it('keeps the data when a read fails and says why, and reads again at the next event', async () => {
    const path = '/api/v1/test/fails';
    const release = hold(path, 'items');
    asked[0].answer('[1]');
    await settle();
    await vi.advanceTimersByTimeAsync(MIN_GAP_MS);
    bump(['items']);
    asked[1].fail('The engine is not reachable.');
    await settle();
    expect(peek(path)).toEqual({ data: [1], error: 'The engine is not reachable.' });
    // Not read again by itself.
    await vi.advanceTimersByTimeAsync(10 * MIN_GAP_MS);
    expect(asked.length).toBe(2);
    bump(['items']);
    asked[2].answer('[1]');
    await settle();
    expect(peek(path)).toEqual({ data: [1], error: null });
    release();
  });

  it('ends the read under way with the last part that shows the path, and forgets the path', async () => {
    const path = '/api/v1/test/leaves';
    const first = hold(path, 'items');
    const second = hold(path, 'items');
    first();
    expect(asked[0].signal?.aborted).toBe(false);
    second();
    expect(asked[0].signal?.aborted).toBe(true);
    // The answer of the read that ended, and events after, change nothing.
    await settle();
    bump(['items']);
    expect(asked.length).toBe(1);
    expect(peek(path)).toBe(NOTHING);
  });
});

describe('what an answer shares with the data before it', () => {
  it('is the data before it where nothing changed, and new only where something did', () => {
    const before = { a: [{ id: 1 }, { id: 2 }], b: 'x', c: null };
    expect(share(before, { a: [{ id: 1 }, { id: 2 }], b: 'x', c: null })).toBe(before);
    const after = share(before, { a: [{ id: 1 }, { id: 3 }], b: 'x', c: null }) as typeof before;
    expect(after).not.toBe(before);
    expect(after.a[0]).toBe(before.a[0]);
    expect(after.a[1]).toEqual({ id: 3 });
    // A member added or taken away, an item added, and another type are changes.
    expect(share(before, { ...before, d: 1 })).not.toBe(before);
    expect(share(before, { a: before.a, b: 'x' })).not.toBe(before);
    // Items that slid (one joined at the end and the first left) are the ones they were, at their new places.
    const slid = [{ id: 1 }, { id: 2 }, { id: 3 }];
    const moved = share(slid, [{ id: 2 }, { id: 3 }, { id: 4 }]) as Array<{ id: number }>;
    expect(moved[0]).toBe(slid[1]);
    expect(moved[1]).toBe(slid[2]);
    expect(moved[2]).toEqual({ id: 4 });
    expect(share([1, 2], [1, 2, 3])).toEqual([1, 2, 3]);
    expect(share([1, 2], [1])).toEqual([1]);
    expect(share({ a: 1 }, [1])).toEqual([1]);
    expect(share([], {})).toEqual({});
    // A name that every object has is no member of one that does not.
    expect(share({}, JSON.parse('{"constructor":1}'))).toEqual(JSON.parse('{"constructor":1}'));
  });
});
