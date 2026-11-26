// @vitest-environment happy-dom
import '../dom-setup';

import { act, type ReactNode, useCallback, useEffect } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { useNotifications } from '../notify';
import type { Changed, Engine, InstallationWait, Item, Items, ItemsRead, Wait, WorkPage } from '../works-api';
import { fixtureItems } from './fixtures';
import {
  fixturesOf,
  mergedItems,
  type Source,
  SourceContext,
  useChanges,
  useItems,
  useItemsOf,
  useProjects,
  useReading,
  useSource,
} from './source';

// What the page asks of the engine's items: the first read whole, then only what changed since, the one read for every
// part that shows them, the wait the engine names while a round is under way, and nothing but the items read while the
// page is hidden.

/// What the engine says the page waits before it reads again while a round is under way.
const AGAIN = 30_000;

let unmounts: (() => void)[] = [];
beforeEach(() => {
  Told.made = [];
  vi.spyOn(document, 'hasFocus').mockImplementation(() => false);
});
afterEach(() => {
  for (const u of unmounts) u();
  unmounts = [];
  document.body.innerHTML = '';
  Reflect.deleteProperty(document, 'visibilityState');
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  vi.useRealTimers();
  localStorage.clear();
});

/// Lets what a read set going end, with the clock the test has.
async function settle() {
  await act(async () => {
    if (vi.isFakeTimers()) await vi.advanceTimersByTimeAsync(0);
    else await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

/// The page seen again, or hidden, as a browser tells it.
function seeing(state: 'visible' | 'hidden') {
  Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => state });
  act(() => {
    document.dispatchEvent(new Event('visibilitychange'));
  });
}

const notice = (works: number[] = [], items = true): Changed => ({
  works,
  items,
  copies: false,
  accounts: false,
  observations: false,
});

/// Items of the fixtures that have cells, each with every cell in this state: none of a round or a call under way, or one.
const base = fixtureItems().items.filter((i) => i.work.cells.length > 0);
const withState = (item: Item, state: 'done' | 'running' | 'calling'): Item => ({
  ...item,
  work: { ...item.work, cells: item.work.cells.map((c) => ({ ...c, state })) },
});
const calm = (item: Item) => withState(item, 'done');
const retitled = (item: Item): Item => ({ ...item, title: `${item.title}, said again` });

/// The engine as the page sees it: its answers to the reads of items given by `serve`, which each read of the items
/// asks with the `since` it sent and its number; the reads the page makes of the engine, the projects and a work; and
/// the stream of changes, which the test tells.
interface Wire {
  asked: (string | undefined)[];
  engine: number;
  projects: number;
  work: number;
  tell: (c: Changed) => void;
  open: () => void;
  source: Source;
}

function wire(serve: (since: string | undefined, n: number) => ItemsRead | Error): Wire {
  const listening = new Set<{ onChanged: (c: Changed) => void; onOpened: () => void }>();
  const w: Wire = {
    asked: [],
    engine: 0,
    projects: 0,
    work: 0,
    tell: (c: Changed) => act(() => listening.forEach((l) => l.onChanged(c))),
    open: () => act(() => listening.forEach((l) => l.onOpened())),
    source: {
      loadCopies: async () => {
        throw new Error('not read here');
      },
      loadItems: async (since?: string) => {
        w.asked.push(since);
        const read = serve(since, w.asked.length);
        if (read instanceof Error) throw read;
        return structuredClone(read);
      },
      loadProjects: async () => {
        w.projects += 1;
        return { names: [] };
      },
      loadWork: async () => {
        w.work += 1;
        return {} as WorkPage;
      },
      loadVersion: async () => {
        throw new Error('not read here');
      },
      loadRound: async () => {
        throw new Error('not read here');
      },
      loadTold: async () => {
        throw new Error('not read here');
      },
      loadDoing: async () => {
        throw new Error('not read here');
      },
      followDoing: () => () => undefined,
      loadChanges: async () => {
        throw new Error('not read here');
      },
      loadDiff: async () => {
        throw new Error('not read here');
      },
      loadSystem: async () => {
        throw new Error('not read here');
      },
      loadCall: async () => {
        throw new Error('not read here');
      },
      act: async () => null,
      loadEngine: async () => {
        w.engine += 1;
        return {} as Engine;
      },
      changes: (onChanged, onOpened) => {
        const l = { onChanged, onOpened };
        listening.add(l);
        return () => listening.delete(l);
      },
    },
  };
  return w;
}

function draw(element: ReactNode, source: Source) {
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  act(() => root.render(<SourceContext.Provider value={source}>{element}</SourceContext.Provider>));
  unmounts.push(() => act(() => root.unmount()));
}

/// What the bar does with the items: reads them as every part does, and says what it holds.
type Held = { value: Items | null; failed: string | null };
function Bar({ onHeld }: { onHeld: (held: Held) => void }) {
  const { value, failed } = useItems();
  useEffect(() => {
    onHeld({ value, failed });
  }, [onHeld, value, failed]);
  return null;
}

/// The console reads the items from the source it names, which is the page's: the same feed as the bar's.
function Console({ onHeld }: { onHeld: (held: Held) => void }) {
  const { value, failed } = useItemsOf(useSource());
  useEffect(() => {
    onHeld({ value, failed });
  }, [onHeld, value, failed]);
  return null;
}

/// What the engine's part of the bar and a work's page do: read what they show when something changed, as they do in
/// Engine.tsx and WorkView.tsx.
function Reads({ work }: { work: number }) {
  const source = useSource();
  const loadEngine = useCallback(() => (source.loadEngine as () => Promise<Engine>)(), [source]);
  const loadWork = useCallback(() => source.loadWork(work), [source, work]);
  const engine = useReading(loadEngine);
  const page = useReading(loadWork);
  useProjects();
  useChanges(engine.read, engine.read);
  useChanges((c) => {
    if (c.works.includes(work)) page.read();
  }, page.read);
  return null;
}

/// The notifications of the browser, as a test sees them: the title of each one made.
class Told {
  static permission: NotificationPermission = 'granted';
  static made: string[] = [];
  static requestPermission = async () => Told.permission;
  onclick: (() => void) | null = null;
  closed = false;
  constructor(title: string) {
    Told.made.push(title);
  }
  close() {
    this.closed = true;
  }
}

/// What tells the owner of the items it reads, as the page does.
function Notifying() {
  useNotifications(useItems().value, true);
  return null;
}

/// A holder the test reads what the bar holds from.
function holder() {
  const held: { latest: Held } = { latest: { value: null, failed: null } };
  return {
    held,
    onHeld: (h: Held) => {
      held.latest = h;
    },
  };
}

const numbers = (items: Items | null) => items?.items.map((i) => i.number);

describe('the items the page holds', () => {
  it('are read whole once, then only what changed since, each other item staying the same object', async () => {
    const [a, b, c, d] = base;
    const w = wire((since) =>
      since === undefined
        ? { cursor: 'p.1', full: true, items: [a, b, c], outage: null, againAfterMs: AGAIN }
        : { cursor: 'p.2', full: false, items: [retitled(b), d], outage: null, againAfterMs: AGAIN },
    );
    const { held, onHeld } = holder();
    draw(<Bar onHeld={onHeld} />, w.source);
    await settle();
    const first = held.latest.value as Items;
    expect(w.asked).toEqual([undefined]);
    expect(numbers(first)).toEqual([a.number, b.number, c.number]);

    w.tell(notice([b.work.number]));
    await settle();
    expect(w.asked).toEqual([undefined, 'p.1']);
    const now = held.latest.value as Items;
    expect(numbers(now)).toEqual([a.number, b.number, c.number, d.number]);
    expect(now.items[0]).toBe(first.items[0]);
    expect(now.items[2]).toBe(first.items[2]);
    expect(now.items[1]).not.toBe(first.items[1]);
    expect(now.items[1].title).toBe(retitled(b).title);
  });

  it("carry what the installation waits on the owner for as the last read gave it, the same list while it is the same, and none once it is gone", async () => {
    const [a, b] = base;
    const stopped: InstallationWait = { key: 'everything', why: 'The engine paused every work.', ends: { kind: 'resume' }, since: 5000 };
    const w = wire((since) =>
      since === undefined
        ? { cursor: 'p.1', full: true, items: [a, b], outage: null, waits: [stopped], againAfterMs: AGAIN }
        : since === 'p.1'
          ? { cursor: 'p.2', full: false, items: [retitled(b)], outage: null, waits: [{ ...stopped }], againAfterMs: AGAIN }
          : { cursor: 'p.3', full: false, items: [], outage: null, waits: [], againAfterMs: AGAIN },
    );
    const { held, onHeld } = holder();
    draw(<Bar onHeld={onHeld} />, w.source);
    await settle();
    const first = held.latest.value as Items;
    expect(first.waits).toEqual([stopped]);
    // Read again as it was, it is the list the page held, so that nothing that reads it is drawn again.
    w.tell(notice([b.work.number]));
    await settle();
    expect((held.latest.value as Items).waits).toBe(first.waits);
    w.tell(notice([b.work.number]));
    await settle();
    expect((held.latest.value as Items).waits).toEqual([]);
  });

  it('are the whole list where the engine says so, and a read that failed keeps what was shown and the cursor', async () => {
    const [a, b, c] = base;
    const w = wire((_since, n) => {
      if (n === 1) return { cursor: 'p.1', full: true, items: [a, b, c], outage: null };
      if (n === 2) return new Error('the server went away');
      // The engine started again: it does not know the cursor, and says the whole list is here.
      if (n === 3) return { cursor: 'q.1', full: true, items: [a, c], outage: null };
      return { cursor: 'q.2', full: false, items: [], outage: null };
    });
    const { held, onHeld } = holder();
    draw(<Bar onHeld={onHeld} />, w.source);
    await settle();
    w.tell(notice());
    await settle();
    expect(held.latest.failed).toBe('the server went away');
    expect(numbers(held.latest.value)).toEqual([a.number, b.number, c.number]);
    w.tell(notice());
    await settle();
    expect(held.latest.failed).toBeNull();
    expect(numbers(held.latest.value)).toEqual([a.number, c.number]);
    w.tell(notice());
    await settle();
    expect(w.asked).toEqual([undefined, 'p.1', 'p.1', 'q.1']);
  });

  it('are read once for the bar and the console, carrying the cursor of the read before', async () => {
    const [a, b] = base;
    const w = wire((since) => ({ cursor: since === undefined ? 'p.1' : 'p.2', full: since === undefined, items: [a, b], outage: null }));
    const bar = holder();
    const consoleHeld = holder();
    draw(
      <>
        <Bar onHeld={bar.onHeld} />
        <Console onHeld={consoleHeld.onHeld} />
      </>,
      w.source,
    );
    await settle();
    expect(w.asked).toEqual([undefined]);
    w.tell(notice([a.work.number]));
    await settle();
    expect(w.asked).toEqual([undefined, 'p.1']);
    expect(consoleHeld.held.latest.value).toBe(bar.held.latest.value);
  });

  it('are what a source that says nothing of a cursor gave at the last read, each read being the whole list', async () => {
    const [a, b, c] = base;
    const before = mergedItems(null, { items: [a, b, c], outage: null });
    expect(numbers(mergedItems(before, { items: [a, c], outage: null }))).toEqual([a.number, c.number]);
    // The fixtures take no `since` and give everything, so a page built on them holds everything at each read.
    const fixtures = fixturesOf(null);
    expect(numbers(await fixtures.loadItems('p.1'))).toEqual(numbers(await fixtures.loadItems()));
    expect((await fixtures.loadItems('p.1')).full).not.toBe(false);
  });
});

describe('the wait for the next read of the items', () => {
  const stands = (n: number, ...states: ('done' | 'running' | 'calling')[]) => {
    const [a, b] = base;
    return { cursor: `p.${n}`, full: n === 1, items: [withState(a, states[0] ?? 'done'), calm(b)], outage: null, againAfterMs: AGAIN };
  };
  const after = async (ms: number) => {
    await act(async () => {
      await vi.advanceTimersByTimeAsync(ms);
    });
  };

  it('is the engine\'s, from the last read, while the page is seen and a round or a call is under way, and no longer once none is', async () => {
    vi.useFakeTimers();
    const w = wire((_since, n) => stands(n, n <= 2 ? 'running' : n === 3 ? 'calling' : 'done'));
    const { onHeld } = holder();
    draw(<Bar onHeld={onHeld} />, w.source);
    await settle();
    expect(w.asked).toEqual([undefined]);
    await after(AGAIN - 1);
    expect(w.asked).toHaveLength(1);
    await after(1);
    expect(w.asked).toEqual([undefined, 'p.1']);
    await after(AGAIN);
    expect(w.asked).toHaveLength(3);
    // A call of a tool under way is waited for as a round is: the third read says it, the fourth says nothing is.
    await after(AGAIN);
    expect(w.asked).toHaveLength(4);
    await after(10 * AGAIN);
    expect(w.asked).toHaveLength(4);
  });

  it('is not set where no round or call is under way, however long the page stays', async () => {
    vi.useFakeTimers();
    const w = wire((_since, n) => stands(n, 'done'));
    const { onHeld } = holder();
    draw(<Bar onHeld={onHeld} />, w.source);
    await settle();
    await after(10 * AGAIN);
    expect(w.asked).toHaveLength(1);
  });

  it('is not set where the engine names none, as with made-up items', async () => {
    vi.useFakeTimers();
    const w = wire((_since, n) => ({ ...stands(n, 'running'), againAfterMs: undefined }));
    const { onHeld } = holder();
    draw(<Bar onHeld={onHeld} />, w.source);
    await settle();
    await after(10 * AGAIN);
    expect(w.asked).toHaveLength(1);
  });

  it('is not kept while the page is hidden, and once it is seen again the items are read once and the wait goes on', async () => {
    vi.useFakeTimers();
    const w = wire((_since, n) => stands(n, 'running'));
    const { onHeld } = holder();
    draw(<Bar onHeld={onHeld} />, w.source);
    await settle();
    seeing('hidden');
    await after(10 * AGAIN);
    expect(w.asked).toHaveLength(1);
    seeing('visible');
    await settle();
    expect(w.asked).toHaveLength(2);
    await after(AGAIN);
    expect(w.asked).toHaveLength(3);
  });

  it('is not kept after the last part that shows the items is gone', async () => {
    vi.useFakeTimers();
    const w = wire((_since, n) => stands(n, 'running'));
    const { onHeld } = holder();
    draw(<Bar onHeld={onHeld} />, w.source);
    await settle();
    for (const u of unmounts) u();
    unmounts = [];
    await after(10 * AGAIN);
    expect(w.asked).toHaveLength(1);
  });
});

describe('a page that is hidden', () => {
  it('reads nothing of the engine, the projects or a work, only the items at each change, and reads each once it is seen again', async () => {
    const [a, b] = base;
    const w = wire((since) => ({ cursor: since === undefined ? 'p.1' : 'p.2', full: since === undefined, items: [a, b], outage: null }));
    const { onHeld } = holder();
    draw(
      <>
        <Bar onHeld={onHeld} />
        <Reads work={a.work.number} />
      </>,
      w.source,
    );
    await settle();
    const reads = () => [w.asked.length, w.engine, w.work, w.projects];
    expect(reads()).toEqual([1, 1, 1, 1]);

    seeing('hidden');
    w.tell(notice([a.work.number]));
    await settle();
    w.open();
    await settle();
    // The items are read at the change and when the stream opens again; the engine and the work are not.
    expect(reads()).toEqual([3, 1, 1, 1]);

    seeing('visible');
    await settle();
    expect(reads()).toEqual([4, 2, 2, 2]);
    w.tell(notice([a.work.number]));
    await settle();
    expect(reads()).toEqual([5, 3, 3, 2]);
  });

  it('still tells the owner, by a notification, of a work that comes to wait for them, from the one read the change brings', async () => {
    vi.stubGlobal('Notification', Told);
    const [a, b] = base;
    const quiet = { ...a, work: { ...a.work, waits: [] } };
    const asks: Wait = { key: 'answer-q1', why: 'Which way?', ends: { kind: 'answer', question: 'q1', version: 1 } };
    const w = wire((since) =>
      since === undefined
        ? { cursor: 'p.1', full: true, items: [quiet, b], outage: null }
        : { cursor: 'p.2', full: false, items: [{ ...a, work: { ...a.work, waits: [asks] } }], outage: null },
    );
    draw(<Notifying />, w.source);
    await settle();
    seeing('hidden');
    w.tell(notice([a.work.number]));
    await settle();
    expect(w.asked).toEqual([undefined, 'p.1']);
    expect(Told.made).toHaveLength(1);
    expect(Told.made[0]).toContain(`#${a.number}`);
  });
});
