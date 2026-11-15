// Where the console reads the items and the works, and sends what the owner does: the interface (`works-api.ts`), or,
// when the page's address has `?fixtures`, the made-up items of `fixtures.ts`, so that the console can be looked at
// with no server behind it.

import { type Heard, listenShared } from '../changes';
import { usesFixtures } from '../mock-data';
import {
  type Act,
  type Items,
  type VersionPage,
  type WorkPage,
  act,
  loadItems,
  loadVersion,
  loadWork,
} from '../works-api';
import { fixtureSource } from './fixtures';

export type { Heard };

export interface Source {
  loadItems: () => Promise<Items>;
  loadWork: (work: number) => Promise<WorkPage>;
  /// The content of a version of a block of a work.
  loadVersion: (work: number, block: string, at: number) => Promise<VersionPage>;
  act: (work: number, what: Act) => Promise<unknown>;
  /// Calls `on` each time something happened; gives what stops it.
  changes: (on: (heard: Heard) => void) => () => void;
}

/// The interface: `GET /api/v1/changes` says, as server-sent events, each time something happened, on the stream the
/// whole page shares (`changes.ts`).
export const interfaceSource: Source = {
  loadItems,
  loadWork,
  loadVersion,
  act,
  changes: listenShared,
};

/// The source the page's address asks for.
export function pageSource(): Source {
  return usesFixtures(window.location.search) ? fixtureSource() : interfaceSource;
}

/// Reads of one kind of thing, of which an answer is taken only when it is newer than the one shown: an answer to a read
/// begun before the read whose answer is shown is dropped, so that an older answer arriving late never replaces a newer
/// one. A failure counts as an answer.
export function inOrder() {
  let begun = 0;
  let shown = 0;
  return <T>(read: Promise<T>, then: (value: T) => void, failed: (why: string) => void) => {
    begun += 1;
    const n = begun;
    read.then(
      (value) => {
        if (n <= shown) return;
        shown = n;
        then(value);
      },
      (e: unknown) => {
        if (n <= shown) return;
        shown = n;
        failed(e instanceof Error ? e.message : String(e));
      },
    );
  };
}
