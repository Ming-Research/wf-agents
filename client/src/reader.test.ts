// What the page reads from the interface. The reader reads a path one read at a time however many parts show it and
// however often it is named, tells its parts only when the answer changed and keeps what did not; it has no pace of its
// own. How often events cause reads is `coalesced`'s (once in `FLUSH_MS`, never in a hidden tab), and what the page
// changes itself is read at once: the second half of this file runs the whole way, from a fake event stream through
// `connect()` and `coalesced` to the requests the reader makes.

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { bump, hold, listen, NOTHING, peek, revisionOf, share, staleAll } from './reader';

/// A request the interface has not answered yet, or has: the test answers it, or fails it.
type Req = { path: string; at: number; signal: AbortSignal | undefined; answer: (text: string) => void; fail: (why: string) => void };
let reqs: Req[];
/// Whether requests wait for the test to answer them; else each is answered at once, with its own number.
let manual: boolean;

const answered = (text: string) => ({ ok: true, status: 200, statusText: 'OK', text: async () => text }) as Response;
const failed = (why: string) =>
  ({ ok: false, status: 500, statusText: 'Error', json: async () => ({ error: why }) }) as unknown as Response;

beforeEach(() => {
  vi.useFakeTimers();
  reqs = [];
  manual = true;
  vi.stubGlobal(
    'fetch',
    (path: string, init?: RequestInit) =>
      new Promise<Response>((resolve, reject) => {
        const signal = init?.signal ?? undefined;
        signal?.addEventListener('abort', () => reject(new DOMException('aborted', 'AbortError')));
        const req: Req = {
          path,
          at: Date.now(),
          signal,
          answer: (text) => resolve(answered(text)),
          fail: (why) => resolve(failed(why)),
        };
        reqs.push(req);
        if (!manual) req.answer(String(reqs.length));
      }),
  );
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

/// Lets what an answer set going (the promises that carry it) run, without time passing.
async function settle(): Promise<void> {
  for (let i = 0; i < 5; i++) await vi.advanceTimersByTimeAsync(0);
}

describe('the reader', () => {
  it('reads a path once for any number of parts, and once more, at once, for any number of times it was named meanwhile', async () => {
    const path = '/api/v1/test/once';
    const release = [hold(path, 'works'), hold(path, 'works')];
    expect(reqs.map((x) => x.path)).toEqual([path]);
    // Named 220 times while the read is under way: no second read, however many.
    for (let i = 0; i < 220; i++) bump(['works']);
    expect(reqs.length).toBe(1);
    reqs[0].answer('[1]');
    await settle();
    expect(peek(path).data).toEqual([1]);
    // The one more read begins when the first ends: nothing here paces it.
    expect(reqs.length).toBe(2);
    reqs[1].answer('[2]');
    await settle();
    expect(peek(path).data).toEqual([2]);
    // Nothing named it since: no more reads, however long.
    await vi.advanceTimersByTimeAsync(60_000);
    expect(reqs.length).toBe(2);
    // Named with none under way, it is read at once.
    bump(['works']);
    expect(reqs.length).toBe(3);
    for (const r of release) r();
  });

  it('names a path by the keys of the parts that show it: `kind:*` for every `kind:<id>`, and none by a name', async () => {
    const path = '/api/v1/test/keys';
    const release = [hold(path, 'work:7'), hold('/api/v1/test/keyless', null)];
    reqs[0].answer('1');
    reqs[1].answer('1');
    await settle();
    bump(['work:8', 'works', 'item:*']);
    expect(reqs.length).toBe(2);
    bump(['work:*']);
    expect(reqs.map((x) => x.path)).toEqual([path, '/api/v1/test/keyless', path]);
    reqs[2].answer('2');
    await settle();
    // Everything shown is read again when events were lost.
    staleAll();
    expect(reqs.map((x) => x.path).slice(3)).toEqual([path, '/api/v1/test/keyless']);
    for (const r of release) r();
  });

  it('tells its listeners only when what a path holds changed, and shares what did not change', async () => {
    const path = '/api/v1/test/share';
    const changed = vi.fn();
    const stop = listen(path, changed);
    const release = hold(path, 'items');
    reqs[0].answer('[{"id":1,"n":1},{"id":2,"n":2}]');
    await settle();
    expect(changed).toHaveBeenCalledTimes(1);
    const first = peek(path);
    const items = first.data as Array<{ id: number; n: number }>;
    // The same text: the same snapshot, nobody told.
    bump(['items']);
    reqs[1].answer('[{"id":1,"n":1},{"id":2,"n":2}]');
    await settle();
    expect(peek(path)).toBe(first);
    expect(changed).toHaveBeenCalledTimes(1);
    // The same data written another way: nothing to draw again.
    bump(['items']);
    reqs[2].answer('[ {"n":1,"id":1}, {"n":2,"id":2} ]');
    await settle();
    expect(peek(path)).toBe(first);
    expect(changed).toHaveBeenCalledTimes(1);
    // One item changed: it is new, the other is the one it was.
    bump(['items']);
    reqs[3].answer('[{"id":1,"n":1},{"id":2,"n":3}]');
    await settle();
    const now = peek(path).data as Array<{ id: number; n: number }>;
    expect(changed).toHaveBeenCalledTimes(2);
    expect(now[0]).toBe(items[0]);
    expect(now[1]).not.toBe(items[1]);
    expect(now[1]).toEqual({ id: 2, n: 3 });
    release();
    stop();
  });

  it('keeps the data when a read fails and says why, and reads again when it is named next', async () => {
    const path = '/api/v1/test/fails';
    const release = hold(path, 'items');
    reqs[0].answer('[1]');
    await settle();
    bump(['items']);
    reqs[1].fail('The engine is not reachable.');
    await settle();
    expect(peek(path)).toEqual({ data: [1], error: 'The engine is not reachable.' });
    // Not read again by itself.
    await vi.advanceTimersByTimeAsync(60_000);
    expect(reqs.length).toBe(2);
    bump(['items']);
    reqs[2].answer('[1]');
    await settle();
    expect(peek(path)).toEqual({ data: [1], error: null });
    release();
  });

  it('ends the read under way with the last part that shows the path, and forgets the path', async () => {
    const path = '/api/v1/test/leaves';
    const first = hold(path, 'items');
    const second = hold(path, 'items');
    first();
    expect(reqs[0].signal?.aborted).toBe(false);
    second();
    expect(reqs[0].signal?.aborted).toBe(true);
    // The answer of the read that ended, and names after, change nothing.
    await settle();
    bump(['items']);
    expect(reqs.length).toBe(1);
    expect(peek(path)).toBe(NOTHING);
  });
});

describe('the reader when reads fail, cross and are left', () => {
  it('reads once more after a read that failed when it was named meanwhile, and then no more', async () => {
    const path = '/api/v1/test/failed-dirty';
    const release = hold(path, 'items');
    reqs[0].answer('[1]');
    await settle();
    bump(['items']);
    // Named twice while the read is under way, and the read fails.
    bump(['items']);
    bump(['items']);
    expect(reqs.length).toBe(2);
    reqs[1].fail('The engine is not reachable.');
    await settle();
    expect(reqs.length).toBe(3);
    reqs[2].answer('[1]');
    await settle();
    expect(peek(path)).toEqual({ data: [1], error: null });
    await vi.advanceTimersByTimeAsync(60_000);
    expect(reqs.length).toBe(3);
    release();
  });

  it('does not read again what was named while the read was under way when the last part leaves, and reads afresh for the next', async () => {
    const path = '/api/v1/test/left-dirty';
    const release = hold(path, 'items');
    bump(['items']);
    release();
    await settle();
    expect(reqs.length).toBe(1);
    bump(['items']);
    expect(reqs.length).toBe(1);
    // A part that comes to show the path later reads it afresh, and the answer of the read that ended is not its own.
    const again = hold(path, 'items');
    expect(reqs.length).toBe(2);
    reqs[0].answer('["late"]');
    await settle();
    expect(peek(path).data).toBeUndefined();
    reqs[1].answer('["now"]');
    await settle();
    expect(peek(path).data).toEqual(['now']);
    again();
  });

  it('names a path by the key of each part that shows it, and by none that left', async () => {
    const path = '/api/v1/test/many-keys';
    const parts = [hold(path, 'works'), hold(path, 'items'), hold(path, null)];
    reqs[0].answer('1');
    await settle();
    bump(['items']);
    expect(reqs.length).toBe(2);
    reqs[1].answer('1');
    await settle();
    bump(['works']);
    expect(reqs.length).toBe(3);
    reqs[2].answer('1');
    await settle();
    bump(['system', 'flows']);
    expect(reqs.length).toBe(3);
    // The part that showed it by `works` left: that name no longer reaches it.
    parts[0]();
    bump(['works']);
    expect(reqs.length).toBe(3);
    bump(['items']);
    expect(reqs.length).toBe(4);
    parts[1]();
    parts[2]();
  });

  it('keeps the data and says why when an answer is no JSON, says it once, and clears it when the data is read again', async () => {
    const path = '/api/v1/test/broken';
    const changed = vi.fn();
    const stop = listen(path, changed);
    const release = hold(path, 'items');
    reqs[0].answer('[1]');
    await settle();
    const good = peek(path);
    bump(['items']);
    reqs[1].answer('<html>not json');
    await settle();
    const broken = peek(path);
    expect(broken.data).toBe(good.data);
    expect(broken.error).not.toBeNull();
    expect(changed).toHaveBeenCalledTimes(2);
    // The same broken answer again says nothing new.
    bump(['items']);
    reqs[2].answer('<html>not json');
    await settle();
    expect(peek(path)).toBe(broken);
    expect(changed).toHaveBeenCalledTimes(2);
    // The answer before the broken one, again: the data is the one it was, and the failure is gone.
    bump(['items']);
    reqs[3].answer('[1]');
    await settle();
    expect(peek(path)).toEqual({ data: [1], error: null });
    expect(peek(path).data).toBe(good.data);
    expect(changed).toHaveBeenCalledTimes(3);
    release();
    stop();
  });

  it('reads a path whose last read failed again for a part that comes to show it', async () => {
    const path = '/api/v1/test/failed-first';
    const first = hold(path, 'items');
    reqs[0].fail('The engine is not reachable.');
    await settle();
    expect(peek(path).error).toBe('The engine is not reachable.');
    const second = hold(path, 'items');
    expect(reqs.length).toBe(2);
    reqs[1].answer('[1]');
    await settle();
    expect(peek(path)).toEqual({ data: [1], error: null });
    // A path that reads fine is not read again for a part that comes to show it.
    const third = hold(path, 'items');
    expect(reqs.length).toBe(2);
    first();
    second();
    third();
  });

  it('gives each state a path has been in a number of its own, also after the path was left and shown again', async () => {
    const path = '/api/v1/test/revisions';
    expect(revisionOf(path)).toBe(0);
    const seen = new Set<number>();
    for (let round = 0; round < 3; round++) {
      const release = hold(path, 'items');
      seen.add(revisionOf(path));
      reqs[reqs.length - 1].answer('[1]');
      await settle();
      seen.add(revisionOf(path));
      release();
      expect(revisionOf(path)).toBe(0);
    }
    // Two states for each of the three times it was shown (not read yet, read), none the number of another.
    expect(seen.size).toBe(6);
  });
});

/// A small generator of the same numbers each run, for data that is built to be compared.
function numbers(seed: number): () => number {
  let state = seed;
  return () => {
    state = (state * 1664525 + 1013904223) % 4294967296;
    return state / 4294967296;
  };
}

/// JSON data written in one way whatever the order its members were made in, which two equal values share.
function canon(x: unknown): string {
  if (Array.isArray(x)) return `[${x.map(canon).join(',')}]`;
  if (typeof x === 'object' && x !== null) {
    const o = x as Record<string, unknown>;
    return `{${Object.keys(o)
      .sort()
      .map((k) => `${JSON.stringify(k)}:${canon(o[k])}`)
      .join(',')}}`;
  }
  return JSON.stringify(x);
}

describe('what an answer shares with the data before it, for data made at random', () => {
  type Rnd = () => number;
  const value = (rnd: Rnd, depth: number): unknown => {
    const pick = rnd();
    if (depth === 0 || pick < 0.4) return [1, 'a', null, true, 'b', 0][Math.floor(rnd() * 6)];
    if (pick < 0.7) return Array.from({ length: Math.floor(rnd() * 5) }, () => value(rnd, depth - 1));
    const o: Record<string, unknown> = {};
    for (const k of ['a', 'b', 'c', 'id']) if (rnd() < 0.6) o[k] = value(rnd, depth - 1);
    return o;
  };
  /// A copy of `x` with a few changes: a member changed, added or taken away; items that joined or left at either end.
  const changed = (rnd: Rnd, x: unknown, depth: number): unknown => {
    if (Array.isArray(x)) {
      let items = x.map((y) => changed(rnd, y, depth - 1));
      if (rnd() < 0.3) items = items.slice(1);
      if (rnd() < 0.3) items = [...items, value(rnd, depth - 1)];
      if (rnd() < 0.2) items = [value(rnd, depth - 1), ...items];
      return items;
    }
    if (typeof x === 'object' && x !== null) {
      const o: Record<string, unknown> = {};
      for (const [k, v] of Object.entries(x)) if (rnd() > 0.1) o[k] = rnd() < 0.2 ? value(rnd, depth - 1) : changed(rnd, v, depth - 1);
      if (rnd() < 0.2) o.c = value(rnd, depth - 1);
      return o;
    }
    return rnd() < 0.15 ? value(rnd, 0) : x;
  };

  it('is always the data that was answered, is the data before it exactly when they are equal, and never changes the data before it', () => {
    const rnd = numbers(20260101);
    let same = 0;
    let different = 0;
    for (let n = 0; n < 400; n++) {
      const prev = value(rnd, 4);
      const before = canon(prev);
      const next = rnd() < 0.2 ? JSON.parse(JSON.stringify(prev)) : changed(rnd, prev, 4);
      const shared = share(prev, next);
      // What the page shows is what was answered.
      expect(canon(shared), `${before} -> ${canon(next)}`).toBe(canon(next));
      // Nothing is left out as unchanged that changed, and nothing that did not change is called changed.
      expect(shared === prev, `${before} -> ${canon(next)}`).toBe(before === canon(next));
      // The data before is not written into.
      expect(canon(prev)).toBe(before);
      if (before === canon(next)) same++;
      else different++;
    }
    // The generator makes both.
    expect(same).toBeGreaterThan(20);
    expect(different).toBeGreaterThan(30);
  });

  it('keeps the object of each item that is equal to one before it, also among items that repeat', () => {
    const prev = [{ id: 1 }, { id: 1 }, { id: 2 }, { id: 3 }];
    const next = share(prev, [{ id: 1 }, { id: 2 }, { id: 2 }, { id: 3 }, { id: 4 }]) as Array<{ id: number }>;
    expect(next).toEqual([{ id: 1 }, { id: 2 }, { id: 2 }, { id: 3 }, { id: 4 }]);
    expect(next[0]).toBe(prev[0]);
    expect(next[1]).toBe(prev[2]);
    expect(next[3]).toBe(prev[3]);
    // A member named like one every object has is a member like any other.
    const proto = JSON.parse('{"__proto__":{"x":1},"y":1}') as object;
    expect(share(proto, JSON.parse('{"__proto__":{"x":1},"y":1}'))).toBe(proto);
    const other = share(proto, JSON.parse('{"__proto__":{"x":2},"y":1}')) as object;
    expect(other).not.toBe(proto);
    expect(JSON.stringify(other)).toBe('{"__proto__":{"x":2},"y":1}');
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

/// The fake event stream the page opens: the test sends it what the engine would.
type Source = { onmessage: ((m: { data: string; lastEventId: string }) => void) | null };
let sources: Source[];

class FakeSource {
  static readonly CLOSED = 2;
  readyState = 1;
  onopen: (() => void) | null = null;
  onerror: (() => void) | null = null;
  onmessage: ((m: { data: string; lastEventId: string }) => void) | null = null;
  constructor() {
    sources.push(this);
  }
}

const send = (e: object) => sources[0].onmessage?.({ data: JSON.stringify(e), lastEventId: '' });
const reads = (path: string) => reqs.filter((r) => r.path === path);

/// The page as it starts, fresh: its event stream open on `FakeSource`, its hooks' module and the reader it feeds.
async function start() {
  vi.resetModules();
  sources = [];
  vi.stubGlobal('EventSource', FakeSource);
  const live = await import('./live');
  const reader = await import('./reader');
  live.connect();
  return { live, reader };
}

describe('what events and the page cause, from the stream to the requests', () => {
  beforeEach(() => {
    manual = false;
  });

  it('reads what events name once in FLUSH_MS and no oftener, the first read no sooner than FLUSH_MS after the first event', async () => {
    const { live, reader } = await start();
    const path = '/api/v1/test/works';
    const release = reader.hold(path, 'works');
    const first = Date.now();
    // A `round` event every 100 ms for 30 s, each read finished at once.
    for (let t = 0; t < 30_000; t += 100) {
      send({ kind: 'round', work: 2, round: t / 100 });
      await vi.advanceTimersByTimeAsync(100);
    }
    const starts = reads(path).map((r) => r.at);
    // The read of the part that came to show the path, then those the events caused.
    expect(starts[0]).toBe(first);
    const caused = starts.slice(1);
    expect(caused[0]).toBeGreaterThanOrEqual(first + live.FLUSH_MS);
    for (let i = 1; i < caused.length; i++) expect(caused[i] - caused[i - 1]).toBeGreaterThanOrEqual(live.FLUSH_MS);
    expect(caused.length).toBeGreaterThanOrEqual(9);
    release();
  });

  it('reads each path 220 item events name once after FLUSH_MS, and not before; once more when one was under way', async () => {
    const { live, reader } = await start();
    const named = ['/api/v1/test/items', '/api/v1/test/item/5', '/api/v1/test/works'];
    const keys = ['items', 'item:5', 'works'];
    const other = '/api/v1/test/flows';
    const release = [...named.map((p, i) => reader.hold(p, keys[i])), reader.hold(other, 'flows')];
    await settle();
    for (let i = 1; i <= 220; i++) send({ kind: 'item', item: i });
    await vi.advanceTimersByTimeAsync(live.FLUSH_MS - 1);
    for (const p of [...named, other]) expect(reads(p).length, p).toBe(1);
    await vi.advanceTimersByTimeAsync(1);
    await settle();
    for (const p of named) expect(reads(p).length, p).toBe(2);
    expect(reads(other).length).toBe(1);
    for (const r of release) r();

    // A read under way when they are read again: it ends, and one more follows, two in all.
    manual = true;
    const busy = '/api/v1/test/busy';
    const stop = reader.hold(busy, 'items');
    expect(reads(busy).length).toBe(1);
    for (let i = 1; i <= 220; i++) send({ kind: 'item', item: i });
    await vi.advanceTimersByTimeAsync(live.FLUSH_MS);
    expect(reads(busy).length).toBe(1);
    reads(busy)[0].answer('[1]');
    await settle();
    expect(reads(busy).length).toBe(2);
    reads(busy)[1].answer('[1]');
    await settle();
    await vi.advanceTimersByTimeAsync(60_000);
    expect(reads(busy).length).toBe(2);
    stop();
  });

  it('reads nothing a name causes after the last part that showed the path left, and everything shown again after lost events', async () => {
    const { live, reader } = await start();
    const gone = '/api/v1/test/gone';
    const a = '/api/v1/test/a';
    const b = '/api/v1/test/b';
    const leave = reader.hold(gone, 'items');
    const stay = [reader.hold(a, 'works'), reader.hold(b, null)];
    await settle();
    leave();
    send({ kind: 'item', item: 1 });
    await vi.advanceTimersByTimeAsync(live.FLUSH_MS * 2);
    await settle();
    expect(reads(gone).length).toBe(1);
    // The stream lost events (`resync`), and the first and every later `ready`: each path shown is read again, once,
    // even the one no name reaches.
    send({ kind: 'resync' });
    await settle();
    expect([reads(a).length, reads(b).length]).toEqual([3, 2]);
    send({ kind: 'ready' });
    await settle();
    expect([reads(a).length, reads(b).length]).toEqual([4, 3]);
    send({ kind: 'ready' });
    await settle();
    expect([reads(a).length, reads(b).length]).toEqual([5, 4]);
    for (const r of stay) r();
  });

  it('reads nothing a name causes while the tab is hidden, however long, and each path it names once, within FLUSH_MS, when it is seen', async () => {
    const doc = { hidden: true };
    vi.stubGlobal('document', doc);
    const { live, reader } = await start();
    const path = '/api/v1/test/hidden';
    const release = reader.hold(path, 'works');
    await settle();
    for (let i = 0; i < 20; i++) {
      send({ kind: 'round', work: 2, round: i });
      await vi.advanceTimersByTimeAsync(1000);
    }
    await vi.advanceTimersByTimeAsync(5 * 60_000);
    expect(reads(path).length).toBe(1);
    doc.hidden = false;
    await vi.advanceTimersByTimeAsync(live.FLUSH_MS);
    await settle();
    expect(reads(path).length).toBe(2);
    await vi.advanceTimersByTimeAsync(60_000);
    expect(reads(path).length).toBe(2);
    release();
  });

  it('reads at once what the page changed itself, whatever was read lately and whatever is waiting to be named; one more when one was under way', async () => {
    const { live, reader } = await start();
    const path = '/api/v1/test/mine';
    const release = reader.hold(path, 'messages');
    await settle();
    await vi.advanceTimersByTimeAsync(1000);
    // A name is waiting in `coalesced`, and the last read began a second ago, less than FLUSH_MS.
    send({ kind: 'owner_message', message: 7, work: null });
    live.reread('messages');
    expect(reads(path).length).toBe(2);
    await settle();

    manual = true;
    const busy = '/api/v1/test/mine-busy';
    const stop = reader.hold(busy, 'messages');
    expect(reads(busy).length).toBe(1);
    live.reread('messages');
    expect(reads(busy).length).toBe(1);
    reads(busy)[0].answer('[1]');
    await settle();
    expect(reads(busy).length).toBe(2);
    release();
    stop();
  });

  it('never has two reads of a path under way when a read takes longer than FLUSH_MS and events keep coming, and ends when they stop', async () => {
    manual = true;
    const { reader } = await start();
    const path = '/api/v1/test/slow';
    const release = reader.hold(path, 'works');
    const done = new Set<Req>();
    let most = 0;
    // A `round` event every 100 ms for 30 s; every read takes 5 s to answer.
    const answerTheOld = async () => {
      most = Math.max(most, reads(path).filter((r) => !done.has(r)).length);
      for (const r of reads(path)) {
        if (!done.has(r) && Date.now() - r.at >= 5000) {
          done.add(r);
          r.answer(`[${done.size}]`);
        }
      }
      await settle();
    };
    for (let t = 0; t < 30_000; t += 100) {
      send({ kind: 'round', work: 2, round: t / 100 });
      await vi.advanceTimersByTimeAsync(100);
      await answerTheOld();
    }
    // 30 s of events: at most one read for each of the ten times they were handed on, and the first read.
    expect(most).toBe(1);
    expect(reads(path).length).toBeGreaterThanOrEqual(5);
    expect(reads(path).length).toBeLessThanOrEqual(12);
    // No event since: the read under way ends, at most one more follows it, and then nothing more is read.
    for (let t = 0; t < 60_000; t += 1000) {
      await vi.advanceTimersByTimeAsync(1000);
      await answerTheOld();
    }
    const total = reads(path).length;
    expect(most).toBe(1);
    await vi.advanceTimersByTimeAsync(60_000);
    await answerTheOld();
    expect(reads(path).length).toBe(total);
    expect(reads(path).filter((r) => !done.has(r))).toEqual([]);
    release();
  });

  it('reads a path once more, not once for each, when lost events and a first and a later `ready` come while a read of it is under way', async () => {
    manual = true;
    const { reader } = await start();
    const path = '/api/v1/test/resynced';
    const release = reader.hold(path, 'works');
    expect(reads(path).length).toBe(1);
    send({ kind: 'resync' });
    send({ kind: 'ready' });
    send({ kind: 'ready' });
    expect(reads(path).length).toBe(1);
    reads(path)[0].answer('[1]');
    await settle();
    expect(reads(path).length).toBe(2);
    reads(path)[1].answer('[1]');
    await settle();
    await vi.advanceTimersByTimeAsync(60_000);
    expect(reads(path).length).toBe(2);
    release();
  });
});
