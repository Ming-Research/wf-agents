// Where the works page reads the items and the works from, and sends what the owner does: the interface
// (works-api.ts), or, when the page's address has `?fixtures`, the fixtures (fixtures.ts), so that the page can be
// looked at before the interface serves the works. The page reaches it through a context, which a test sets to its
// own source.

import { createContext, useCallback, useContext, useEffect, useRef, useState } from 'react';

import * as api from '../works-api';
import type { Act, CallPage, Changed, Copies, Items, RoundPage, ToldPage, VersionPage, WorkPage } from '../works-api';

export interface Source {
  loadCopies(): Promise<Copies>;
  loadItems(): Promise<Items>;
  loadWork(work: number): Promise<WorkPage>;
  loadVersion(work: number, block: string, at: number): Promise<VersionPage>;
  loadRound(work: number, run: number, round: number): Promise<RoundPage>;
  loadTold(work: number, run: number, round: number): Promise<ToldPage>;
  loadCall(work: number, call: number): Promise<CallPage>;
  act(work: number, what: Act): Promise<null>;
  /// Tells `onChanged` each time something happened, and `onOpened` each time the stream of changes opens, the first
  /// time and again after it was lost, when changes may have been missed; gives the way to stop.
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
  changes(onChanged, onOpened) {
    if (typeof EventSource === 'undefined') return () => undefined;
    const events = new EventSource('/api/v1/changes');
    events.onopen = () => onOpened();
    events.onmessage = (e: MessageEvent<string>) => {
      try {
        onChanged(JSON.parse(e.data) as Changed);
      } catch {
        // What cannot be read says nothing happened.
      }
    };
    return () => events.close();
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
  return new URLSearchParams(search).has('fixtures') ? FIXTURES : INTERFACE;
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

/// What a view shows, read at once and again on `read`. A read that fails keeps what was shown and says why; an answer
/// that comes after a newer one is dropped, so that the view never goes back to an older state.
export function useReading<T>(load: () => Promise<T>): { value: T | null; failed: string | null; read: () => void } {
  const [state, setState] = useState<{ value: T | null; failed: string | null }>({ value: null, failed: null });
  const asked = useRef(0);
  const shown = useRef(0);
  const read = useCallback(() => {
    asked.current += 1;
    const n = asked.current;
    load().then(
      (value) => {
        if (n < shown.current) return;
        shown.current = n;
        setState({ value, failed: null });
      },
      (e: unknown) => {
        if (n < shown.current) return;
        shown.current = n;
        setState((s) => ({ value: s.value, failed: message(e) }));
      },
    );
  }, [load]);
  useEffect(read, [read]);
  return { ...state, read };
}
