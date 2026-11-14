// Where the works page reads the items and the works from, and sends what the owner does: the interface
// (works-api.ts), or, when the page's address has `?fixtures`, the fixtures (fixtures.ts), so that the page can be
// looked at before the interface serves the works. The page reaches it through a context, which a test sets to its
// own source.
//
// The page is to stay quick with several hundred works, each change of any of them read again. So the interface's
// changes come on the page's one stream (changes.ts); a part reads again at most once at a time, and once more after
// a read where changes came while it was under way; and the items, which the bar and the list of works both show, are
// read once for both, each item that did not change kept as the very object it was, so that what draws it is not drawn
// again.

import { createContext, useCallback, useContext, useEffect, useRef, useState, useSyncExternalStore } from 'react';

import { listenShared } from '../changes';
import { usesFixtures } from '../mock-data';
import * as api from '../works-api';
import type {
  Act,
  CallPage,
  Changed,
  Copies,
  Engine,
  EngineAct,
  Items,
  RoundPage,
  ToldPage,
  VersionPage,
  WorkPage,
} from '../works-api';

export interface Source {
  loadCopies(): Promise<Copies>;
  loadItems(): Promise<Items>;
  loadWork(work: number): Promise<WorkPage>;
  loadVersion(work: number, block: string, at: number): Promise<VersionPage>;
  loadRound(work: number, run: number, round: number): Promise<RoundPage>;
  loadTold(work: number, run: number, round: number): Promise<ToldPage>;
  loadCall(work: number, call: number): Promise<CallPage>;
  act(work: number, what: Act): Promise<null>;
  /// How the engine stands as a whole, and the owner's pause of everything; a source that has no engine has neither.
  loadEngine?(): Promise<Engine>;
  actOnEngine?(what: EngineAct): Promise<null>;
  /// Tells `onChanged` each time something happened, and `onOpened` each time the stream of changes opens again after
  /// it was lost, when changes may have been missed; gives the way to stop.
  changes(onChanged: (changed: Changed) => void, onOpened: () => void): () => void;
}

/// The interface: its routes, and its stream of changes.
export const INTERFACE: Source = {
  loadCopies: api.loadCopies,
  loadItems: api.loadItems,
  loadWork: api.loadWork,
  loadVersion: api.loadVersion,
  loadRound: api.loadRound,
  loadTold: api.loadTold,
  loadCall: api.loadCall,
  act: api.act,
  loadEngine: api.loadEngine,
  actOnEngine: api.actOnEngine,
  changes(onChanged, onOpened) {
    return listenShared((heard) => (heard === 'everything' ? onOpened() : onChanged(heard)));
  },
};

/// The fixtures, loaded only when asked for, so that the page as built for the interface does not carry them. What the
/// owner does to a fixture is only logged, and no stream of changes is opened.
export const FIXTURES: Source = {
  loadCopies: async () => (await import('./fixtures')).fixtureCopies(),
  loadItems: async () => (await import('./fixtures')).fixtureItems(),
  loadWork: async (work) => (await import('./fixtures')).fixtureWork(work),
  loadVersion: async (work, block, at) => (await import('./fixtures')).fixtureVersion(work, block, at),
  loadRound: async (work, run, round) => (await import('./fixtures')).fixtureRound(work, run, round),
  loadTold: async (work, run, round) => (await import('./fixtures')).fixtureTold(work, run, round),
  loadCall: async (work, call) => (await import('./fixtures')).fixtureCall(work, call),
  act: async (work, what) => {
    console.info(`works/${work}/acts`, what);
    return null;
  },
  changes: () => () => undefined,
};

/// The source the page's address names.
export function sourceOf(search: string): Source {
  return usesFixtures(search) ? FIXTURES : INTERFACE;
}

export const SourceContext = createContext<Source>(
  typeof window === 'undefined' ? INTERFACE : sourceOf(window.location.search),
);

export function useSource(): Source {
  return useContext(SourceContext);
}

/// Calls `onChanged` each time the source says something happened, and `onOpened` each time its stream opens, for as
/// long as the part that asks is shown.
export function useChanges(onChanged: (changed: Changed) => void, onOpened: () => void) {
  const source = useSource();
  const latest = useRef({ onChanged, onOpened });
  useEffect(() => {
    latest.current = { onChanged, onOpened };
  });
  useEffect(
    () =>
      source.changes(
        (c) => latest.current.onChanged(c),
        () => latest.current.onOpened(),
      ),
    [source],
  );
}

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

/// A read of one load under way, and whether it is to be read once more when it ends.
interface Flight<T> {
  load: () => Promise<T>;
  going: boolean;
  again: boolean;
}

/// What a view shows, read at once and again on `read`. A read that fails keeps what was shown and says why. One read
/// is under way at a time: a read asked for meanwhile is made once, after it, however many were asked; and once the
/// view reads something else (another `load`), what comes of the reads before is dropped, so that the view never goes
/// back to an older state.
export function useReading<T>(load: () => Promise<T>): { value: T | null; failed: string | null; read: () => void } {
  const [state, setState] = useState<{ value: T | null; failed: string | null }>({ value: null, failed: null });
  const flight = useRef<Flight<T> | null>(null);
  const read = useCallback(() => {
    if (flight.current === null || flight.current.load !== load) {
      flight.current = { load, going: false, again: false };
    }
    const mine = flight.current;
    if (mine.going) {
      mine.again = true;
      return;
    }
    const go = () => {
      mine.going = true;
      mine
        .load()
        .then(
          (value) => {
            if (flight.current === mine) setState({ value, failed: null });
          },
          (e: unknown) => {
            if (flight.current === mine) setState((s) => ({ value: s.value, failed: message(e) }));
          },
        )
        .finally(() => {
          mine.going = false;
          if (mine.again && flight.current === mine) {
            mine.again = false;
            go();
          }
        });
    };
    go();
  }, [load]);
  useEffect(read, [read]);
  return { ...state, read };
}

/// The items as read for every part that shows them, and the parts to tell when they are read again.
interface ItemsStore {
  state: { value: Items | null; failed: string | null };
  shown: Set<() => void>;
  going: boolean;
  again: boolean;
  stop: (() => void) | null;
}

const itemsStores = new WeakMap<Source, ItemsStore>();

function itemsStore(source: Source): ItemsStore {
  let store = itemsStores.get(source);
  if (store === undefined) {
    store = { state: { value: null, failed: null }, shown: new Set(), going: false, again: false, stop: null };
    itemsStores.set(source, store);
  }
  return store;
}

/// The items read anew, each that is as it was kept as the object it was, and the list kept whole where none changed:
/// what draws an item that did not change is not drawn again.
export function keptSame(before: Items, now: Items): Items {
  const was = new Map(before.items.map((item) => [item.number, item]));
  let changed = before.items.length !== now.items.length;
  const items = now.items.map((item, i) => {
    const old = was.get(item.number);
    if (old !== undefined && JSON.stringify(old) === JSON.stringify(item)) {
      if (before.items[i] !== old) changed = true;
      return old;
    }
    changed = true;
    return item;
  });
  const outage = JSON.stringify(before.outage ?? null) === JSON.stringify(now.outage ?? null) ? before.outage : now.outage;
  if (!changed && outage === before.outage) return before;
  return { ...now, items, outage };
}

/// Reads the items again for every part that shows them: at once, or once more after the read under way.
function readItems(source: Source, store: ItemsStore) {
  if (store.going) {
    store.again = true;
    return;
  }
  store.going = true;
  const set = (state: ItemsStore['state']) => {
    store.state = state;
    [...store.shown].forEach((tell) => tell());
  };
  source
    .loadItems()
    .then(
      (fresh) => {
        const before = store.state.value;
        set({ value: before === null ? fresh : keptSame(before, fresh), failed: null });
      },
      (e: unknown) => set({ value: store.state.value, failed: message(e) }),
    )
    .finally(() => {
      store.going = false;
      if (store.again) {
        store.again = false;
        readItems(source, store);
      }
    });
}

/// The items, read once for every part of the page that shows them, as the list of works and the bar both do: at
/// once, again each time an item or a work changed and each time the stream of changes opens again, and on `read`.
/// A read that fails keeps what was shown and says why.
export function useItems(): { value: Items | null; failed: string | null; read: () => void } {
  const source = useSource();
  const store = itemsStore(source);
  const subscribe = useCallback(
    (tell: () => void) => {
      store.shown.add(tell);
      if (store.shown.size === 1) {
        store.stop = source.changes(
          (changed) => {
            if (changed.items || changed.works.length > 0) readItems(source, store);
          },
          () => readItems(source, store),
        );
        readItems(source, store);
      }
      return () => {
        store.shown.delete(tell);
        if (store.shown.size === 0) {
          store.stop?.();
          store.stop = null;
        }
      };
    },
    [source, store],
  );
  const state = useSyncExternalStore(subscribe, () => store.state);
  const read = useCallback(() => readItems(source, store), [source, store]);
  return { ...state, read };
}
