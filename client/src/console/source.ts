// Where the console reads the items and the works, and sends what the owner does: the interface (`works-api.ts`), or,
// when the page's address has `?fixtures`, the made-up items of `fixtures.ts`, so that the console can be looked at
// with no server behind it.

import { type Act, type Changed, type Items, type WorkPage, act, loadItems, loadWork } from '../works-api';
import { fixtureSource } from './fixtures';

export interface Source {
  loadItems: () => Promise<Items>;
  loadWork: (work: number) => Promise<WorkPage>;
  act: (work: number, what: Act) => Promise<unknown>;
  /// Calls `on` each time something happened; gives what stops it.
  changes: (on: (changed: Changed) => void) => () => void;
}

/// What happened, as an event of the stream says it; where it cannot be read, that anything may have.
function changedOf(data: string): Changed {
  try {
    const c = JSON.parse(data) as Partial<Changed>;
    return { works: Array.isArray(c.works) ? c.works : [], items: c.items ?? true };
  } catch {
    return { works: [], items: true };
  }
}

/// The interface: `GET /api/v1/changes` says, as server-sent events, each time something happened.
export const interfaceSource: Source = {
  loadItems,
  loadWork,
  act,
  changes: (on) => {
    const stream = new EventSource('/api/v1/changes');
    stream.onmessage = (e: MessageEvent<string>) => on(changedOf(e.data));
    return () => stream.close();
  },
};

/// The source the page's address asks for.
export function pageSource(): Source {
  return new URLSearchParams(window.location.search).has('fixtures') ? fixtureSource() : interfaceSource;
}
