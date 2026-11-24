// Where the console reads the items and the works, and sends what the owner does: the interface (`works-api.ts`), or,
// when the page's address has `?fixtures`, the made-up items of `fixtures.ts`, so that the console can be looked at
// with no server behind it. The items of the interface are read from the feed the bar and the list of works read them
// from, once for the whole page (works/source.ts).

import { type Heard, listenShared } from '../changes';
import { usesFixtures } from '../mock-data';
import { type Act, type VersionPage, type WorkPage, act, loadProjects, loadVersion, loadWork } from '../works-api';
import { INTERFACE, type ItemsFeed } from '../works/source';
import { fixtureSource } from './fixtures';

export type { Heard };

export interface Source {
  /// What the items are read from: every part of the page that reads them from one feed shares one read of them
  /// (works/source.ts, `useItemsOf`).
  items: ItemsFeed;
  loadWork: (work: number) => Promise<WorkPage>;
  /// The content of a version of a block of a work, by its number among the block's versions.
  loadVersion: (work: number, block: string, number: number) => Promise<VersionPage>;
  act: (work: number, what: Act) => Promise<unknown>;
  /// Calls `on` each time something happened; gives what stops it.
  changes: (on: (heard: Heard) => void) => () => void;
  /// The projects the installation names, those with no item yet among them; none read where it is not given.
  projects?: () => Promise<string[]>;
}

/// The interface: `GET /api/v1/changes` says, as server-sent events, each time something happened, on the stream the
/// whole page shares (`changes.ts`); the items are read from the interface as the bar and the list of works read them.
export const interfaceSource: Source = {
  items: INTERFACE,
  loadWork,
  loadVersion,
  act,
  changes: listenShared,
  projects: () => loadProjects().then((p) => p.names),
};

/// The source the page's address asks for: with `?fixtures`, the made-up items, of as many projects as `projects=`
/// asks.
export function pageSource(): Source {
  const asked = Number(new URLSearchParams(window.location.search).get('projects') ?? '1');
  return usesFixtures(window.location.search) ? fixtureSource(Date.now(), asked) : interfaceSource;
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
