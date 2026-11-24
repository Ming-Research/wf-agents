// Where the works page reads the items and the works from, and sends what the owner does: the interface
// (works-api.ts), or, when the page's address has `?fixtures`, the fixtures (fixtures.ts), so that the page can be
// looked at before the interface serves the works. The page reaches it through a context, which a test sets to its
// own source. The item's window reads a work's contents through it, each that does not change read once (`Reader`).
//
// The page is to stay quick with several hundred works, each change of any of them read again. So the interface's
// changes come on the page's one stream (changes.ts); a part reads again at most once at a time, and once more after
// a read where changes came while it was under way; and the items, which the bar, the list of works and the console
// all show, are read once for all of them, each item that did not change kept as the very object it was, so that what
// draws it is not drawn again.

import { createContext, useCallback, useContext, useEffect, useRef, useState, useSyncExternalStore } from 'react';

import { listenShared } from '../changes';
import { projectsAsked, usesFixtures } from '../mock-data';
import * as api from '../works-api';
import type {
  Act,
  CallPage,
  Changed,
  ChangesPage,
  Copies,
  DiffPage,
  DoingPage,
  Done,
  Following,
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
import { workName } from './words';

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
  /// Follows what the round's agent does, from the round's start, as it comes; gives the way to stop following.
  followDoing(work: number, run: number, round: number, following: Following): () => void;
  /// Each file the work changed, and the diff of one of them.
  loadChanges(work: number): Promise<ChangesPage>;
  loadDiff(work: number, path: string): Promise<DiffPage>;
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
  followDoing: api.followDoing,
  loadChanges: api.loadChanges,
  loadDiff: api.loadDiff,
  loadSystem: api.loadSystem,
  loadCall: api.loadCall,
  act: api.act,
  loadEngine: api.loadEngine,
  actOnEngine: api.actOnEngine,
  changes(onChanged, onOpened) {
    return listenShared((heard) => (heard === 'everything' ? onOpened() : onChanged(heard)));
  },
};

/// The fixtures, of as many projects as the address asks for (`projectsAsked`) or else of their own number of them,
/// loaded only when asked for, so that the page as built for the interface does not carry them. What the owner does to
/// a fixture is only logged, and no stream of changes is opened.
export const fixturesOf = (projects: number | null): Source => ({
  loadCopies: async () => (await import('./fixtures')).fixtureCopies(),
  loadItems: async () => (await import('./fixtures')).fixtureItems(projects ?? undefined),
  loadProjects: async () => (await import('./fixtures')).fixtureProjects(projects ?? undefined),
  loadWork: async (work) => (await import('./fixtures')).fixtureWork(work, projects ?? undefined),
  loadVersion: async (work, block, number) => (await import('./fixtures')).fixtureVersion(work, block, number),
  loadRound: async (work, run, round) => (await import('./fixtures')).fixtureRound(work, run, round),
  loadTold: async (work, run, round) => (await import('./fixtures')).fixtureTold(work, run, round),
  loadDoing: async (work, run, round) => (await import('./fixtures')).fixtureDoing(work, run, round),
  // What the round did is told at once, and no more comes: the fixtures stand still.
  followDoing: (work, run, round, following) => {
    let stopped = false;
    void import('./fixtures').then((f) => {
      if (stopped) return;
      try {
        f.fixtureDoing(work, run, round).doing.forEach((d) => following.done(d));
      } catch (e) {
        following.failed(message(e));
      }
    });
    return () => {
      stopped = true;
    };
  },
  loadChanges: (work) => Promise.reject(new Error(`The made-up ${workName(work)} changed no files`)),
  loadDiff: (work) => Promise.reject(new Error(`The made-up ${workName(work)} changed no files`)),
  loadSystem: async (work, line, session) => (await import('./fixtures')).fixtureSystem(work, line, session),
  loadCall: async (work, call) => (await import('./fixtures')).fixtureCall(work, call),
  act: async (work, what) => {
    console.info(`works/${work}/acts`, what);
    return null;
  },
  changes: () => () => undefined,
});

/// The fixtures, of their own number of projects.
export const FIXTURES: Source = fixturesOf(null);

/// What the item's window reads a work's contents through, each read once.
export interface Reader {
  /// The content of the block's version of this number.
  version(block: string, number: number): Promise<string>;
  /// A round in full; kept once read only where the round had ended, since one under way has more to come.
  round(run: number, number: number, ended: boolean): Promise<RoundPage>;
  /// What a round was told and returned, as its carrier kept them; kept once read only where the round had ended.
  told(run: number, number: number, ended: boolean): Promise<ToldPage>;
  call(call: number): Promise<string | null>;
  /// The system prompt the line's session of this number, the line's first being none, began with; kept once read.
  system(line: string, session: number): Promise<string>;
  /// What the agent of a round that ended did, from its start, every try in order; kept once read.
  doing(run: number, number: number): Promise<Done[]>;
  /// Follows what the agent of a round does, from its start, as it comes; gives the way to stop following.
  follow(run: number, number: number, following: Following): () => void;
  /// Each file the work changed, and the diff of one of them; read anew each time, as the work's head moves.
  changes(): Promise<ChangesPage>;
  diff(path: string): Promise<DiffPage>;
}

/// The reads of a work's contents, each made once: a version, an ended round, what it was told and returned and what
/// its agent did, and a call's result do not change once there. A round under way is read again each time, since it
/// has more to come, and what its agent does is followed as it comes.
export function readerOf(source: Source, work: number): Reader {
  const once = new Map<string, Promise<unknown>>();
  const cached = <T,>(key: string, load: () => Promise<T>): Promise<T> => {
    let p = once.get(key) as Promise<T> | undefined;
    if (!p) {
      p = load();
      once.set(key, p);
      p.catch(() => once.delete(key));
    }
    return p;
  };
  return {
    version: (block, number) =>
      cached(`v:${block}/${number}`, async () => (await source.loadVersion(work, block, number)).content),
    round: (run, number, ended) =>
      ended
        ? cached<RoundPage>(`r:${run}/${number}`, () => source.loadRound(work, run, number))
        : source.loadRound(work, run, number),
    told: (run, number, ended) =>
      ended
        ? cached<ToldPage>(`t:${run}/${number}`, () => source.loadTold(work, run, number))
        : source.loadTold(work, run, number),
    call: (call) => cached(`c:${call}`, async () => (await source.loadCall(work, call)).result),
    system: (line, session) =>
      cached(`s:${line}/${session}`, async () => (await source.loadSystem(work, line, session)).text),
    doing: (run, number) =>
      cached(`d:${run}/${number}`, async () => (await source.loadDoing(work, run, number)).doing),
    follow: (run, number, following) => source.followDoing(work, run, number, following),
    changes: () => source.loadChanges(work),
    diff: (path) => source.loadDiff(work, path),
  };
}

/// The source the page's address names.
export function sourceOf(search: string): Source {
  return usesFixtures(search) ? fixturesOf(projectsAsked(search)) : INTERFACE;
}

export const SourceContext = createContext<Source>(
  typeof window === 'undefined' ? INTERFACE : sourceOf(window.location.search),
);

export function useSource(): Source {
  return useContext(SourceContext);
}

/// Whether a change says only that what the calls of an agent's round under way have come to changed: it names no work
/// and sets no other flag, since no record of a work holds it, and only a part that shows it has anything to read.
export function onlyGoing(changed: Changed): boolean {
  return (
    changed.going === true &&
    changed.works.length === 0 &&
    !changed.items &&
    !changed.copies &&
    !changed.accounts &&
    !changed.observations
  );
}

/// Calls `onChanged` each time the source says something happened, and `onOpened` each time its stream opens, for as
/// long as the part that asks is shown. A change that says only what the rounds under way have come to is not told to
/// it ([`useGoings`] is told): it comes with every call of every round, and a part that reads the engine, the items or
/// a work again for each would read for nothing.
export function useChanges(onChanged: (changed: Changed) => void, onOpened: () => void) {
  const source = useSource();
  const latest = useRef({ onChanged, onOpened });
  useEffect(() => {
    latest.current = { onChanged, onOpened };
  });
  useEffect(
    () =>
      source.changes(
        (c) => {
          if (!onlyGoing(c)) latest.current.onChanged(c);
        },
        () => latest.current.onOpened(),
      ),
    [source],
  );
}

/// Calls `onGoing` each time the source says what the calls of the rounds under way have come to changed, and each
/// time its stream opens, while `shown`: a part that is hidden is told nothing, and is told once it is shown again by
/// reading when it is.
export function useGoings(onGoing: () => void, shown: boolean) {
  const source = useSource();
  const latest = useRef(onGoing);
  useEffect(() => {
    latest.current = onGoing;
  });
  // Shown again after being hidden, a part reads once for what it was not told.
  const was = useRef(shown);
  useEffect(() => {
    if (shown && !was.current) latest.current();
    was.current = shown;
  }, [shown]);
  useEffect(() => {
    if (!shown) return undefined;
    return source.changes(
      (c) => {
        if (c.going === true) latest.current();
      },
      () => latest.current(),
    );
  }, [source, shown]);
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

/// No change the stream tells of is a change of the projects.
const noChange = () => undefined;

/// No project read, the same list each time, so that what is worked out from it is not worked out again.
const NO_NAMES: string[] = [];

/// The projects the installation names, in order of their names, for a list grouped by project: read at once and again
/// whenever the stream of changes opens again; none while they are not read, or where they cannot be, when the list
/// has only the projects its rows name.
export function useProjects(): string[] {
  const source = useSource();
  const load = useCallback(() => source.loadProjects(), [source]);
  const names = useReading(load);
  useChanges(noChange, names.read);
  return names.value?.names ?? NO_NAMES;
}

/// Where the items are read from, and what says when to read them again. A source of the works page is one; the
/// console's source names the one it reads its items from (console/source.ts).
export type ItemsFeed = Pick<Source, 'loadItems' | 'changes'>;

/// The items as read for every part that shows them, and the parts to tell when they are read again.
interface ItemsStore {
  state: { value: Items | null; failed: string | null };
  shown: Set<() => void>;
  reads: OneAtATime;
  stop: (() => void) | null;
}

const itemsStores = new WeakMap<ItemsFeed, ItemsStore>();

function itemsStore(feed: ItemsFeed): ItemsStore {
  let store = itemsStores.get(feed);
  if (store === undefined) {
    const made: ItemsStore = {
      state: { value: null, failed: null },
      shown: new Set(),
      reads: oneAtATime(() => readItems(feed, made)),
      stop: null,
    };
    store = made;
    itemsStores.set(feed, store);
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
function readItems(feed: ItemsFeed, store: ItemsStore): Promise<void> {
  const set = (state: ItemsStore['state']) => {
    store.state = state;
    [...store.shown].forEach((tell) => tell());
  };
  return feed.loadItems().then(
    (fresh) => {
      const before = store.state.value;
      set({ value: before === null ? fresh : keptSame(before, fresh), failed: null });
    },
    (e: unknown) => set({ value: store.state.value, failed: message(e) }),
  );
}

/// The items of the page's source, read once for every part of the page that shows them ([`useItemsOf`]).
export function useItems(): { value: Items | null; failed: string | null; read: () => void } {
  return useItemsOf(useSource());
}

/// The items `feed` gives, read once for every part of the page that reads them from it: at once, again each time an
/// item or a work changed and each time the stream of changes opens again, and on `read`. A read that fails keeps
/// what was shown and says why.
export function useItemsOf(feed: ItemsFeed): { value: Items | null; failed: string | null; read: () => void } {
  const store = itemsStore(feed);
  const subscribe = useCallback(
    (tell: () => void) => {
      store.shown.add(tell);
      if (store.shown.size === 1) {
        store.stop = feed.changes(
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
    [feed, store],
  );
  const state = useSyncExternalStore(subscribe, () => store.state);
  const read = useCallback(() => store.reads.ask(), [store]);
  return { ...state, read };
}
