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
  DoingPage,
  Engine,
  EngineAct,
  Items,
  Projects,
  RoundPage,
  Said,
  SystemPage,
  ToldPage,
  VersionPage,
  WorkPage,
} from '../works-api';

export interface Source {
  loadCopies(): Promise<Copies>;
  loadItems(): Promise<Items>;
  /// The projects the installation names.
  loadProjects(): Promise<Projects>;
  loadWork(work: number): Promise<WorkPage>;
  /// The content of the block's version of this number.
  loadVersion(work: number, block: string, number: number): Promise<VersionPage>;
  loadRound(work: number, run: number, round: number): Promise<RoundPage>;
  loadTold(work: number, run: number, round: number): Promise<ToldPage>;
  loadDoing(work: number, run: number, round: number): Promise<DoingPage>;
  loadSystem(work: number, line: string, session: number): Promise<SystemPage>;
  loadCall(work: number, call: number): Promise<CallPage>;
  /// What the owner does to a work: words said answer with the cell the engine gave them to, every other act with
  /// nothing.
  act(work: number, what: Act): Promise<Said | null>;
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
  loadProjects: api.loadProjects,
  loadWork: api.loadWork,
  loadVersion: api.loadVersion,
  loadRound: api.loadRound,
  loadTold: api.loadTold,
  loadDoing: api.loadDoing,
  loadSystem: api.loadSystem,
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
  loadProjects: async () => (await import('./fixtures')).fixtureProjects(),
  loadWork: async (work) => (await import('./fixtures')).fixtureWork(work),
  loadVersion: async (work, block, number) => (await import('./fixtures')).fixtureVersion(work, block, number),
  loadRound: async (work, run, round) => (await import('./fixtures')).fixtureRound(work, run, round),
  loadTold: async (work, run, round) => (await import('./fixtures')).fixtureTold(work, run, round),
  loadDoing: async (work, run, round) => (await import('./fixtures')).fixtureDoing(work, run, round),
  loadSystem: async (work, line, session) => (await import('./fixtures')).fixtureSystem(work, line, session),
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

/// A read that runs at most once at a time: asked for while it is under way, it runs once more after it, however many
/// times it was asked for meanwhile; and once stopped, it runs no more, the read under way left to end.
export interface OneAtATime {
  ask: () => void;
  stop: () => void;
}

/// Runs `read` at most once at a time ([`OneAtATime`]). `read` says itself what came of it: what it gives is not read.
export function oneAtATime(read: () => Promise<unknown>): OneAtATime {
  let going = false;
  let again = false;
  let stopped = false;
  const ask = () => {
    if (stopped) return;
    if (going) {
      again = true;
      return;
    }
    going = true;
    const done = () => {
      going = false;
      if (again && !stopped) {
        again = false;
        ask();
      }
    };
    read().then(done, done);
  };
  return {
    ask,
    stop: () => {
      stopped = true;
    },
  };
}

/// The reads of one load, at most one at a time.
interface Flight<T> {
  load: () => Promise<T>;
  reads: OneAtATime;
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
      flight.current?.reads.stop();
      const mine: Flight<T> = {
        load,
        reads: oneAtATime(() =>
          load().then(
            (value) => {
              if (flight.current === mine) setState({ value, failed: null });
            },
            (e: unknown) => {
              if (flight.current === mine) setState((s) => ({ value: s.value, failed: message(e) }));
            },
          ),
        ),
      };
      flight.current = mine;
    }
    flight.current.reads.ask();
  }, [load]);
  useEffect(read, [read]);
  return { ...state, read };
}

/// The items as read for every part that shows them, and the parts to tell when they are read again.
interface ItemsStore {
  state: { value: Items | null; failed: string | null };
  shown: Set<() => void>;
  reads: OneAtATime;
  stop: (() => void) | null;
}

const itemsStores = new WeakMap<Source, ItemsStore>();

function itemsStore(source: Source): ItemsStore {
  let store = itemsStores.get(source);
  if (store === undefined) {
    const made: ItemsStore = {
      state: { value: null, failed: null },
      shown: new Set(),
      reads: oneAtATime(() => readItems(source, made)),
      stop: null,
    };
    store = made;
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

/// Reads the items for every part that shows them, and tells each what came of it. The store's reads run it at most
/// once at a time.
function readItems(source: Source, store: ItemsStore): Promise<void> {
  const set = (state: ItemsStore['state']) => {
    store.state = state;
    [...store.shown].forEach((tell) => tell());
  };
  return source.loadItems().then(
    (fresh) => {
      const before = store.state.value;
      set({ value: before === null ? fresh : keptSame(before, fresh), failed: null });
    },
    (e: unknown) => set({ value: store.state.value, failed: message(e) }),
  );
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
            if (changed.items || changed.works.length > 0) store.reads.ask();
          },
          () => store.reads.ask(),
        );
        store.reads.ask();
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
  const read = useCallback(() => store.reads.ask(), [store]);
  return { ...state, read };
}
