// A work's page opened on the address of one of its rounds, in a real browser (Chromium), whose layout and scrolling are
// its own: the round the address names is brought into view in the item's window, which scrolls to it, and the page
// itself does not move, as it would if the window were scrolled to the round while it was still out of the page's view.
// The browser is made short, so that the round lies below what the window shows until the window is scrolled to it; and
// the round is judged against the browser's own view as well as the window's, so that a window left out of the page fails.

import '../styles/tokens.css';
import '../styles/parts.css';
import '../styles/chart.css';
import '../styles/graph.css';
import '../styles/works.css';

import { flushSync } from 'react-dom';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';
import { page as browserPage } from 'vitest/browser';

import {
  fixtureCall,
  fixtureCopies,
  fixtureDoing,
  fixtureItems,
  fixtureProjects,
  fixtureRound,
  fixtureSystem,
  fixtureTold,
  fixtureVersion,
  fixtureWork,
} from './fixtures';
import { type Source, SourceContext } from './source';
import { WorkScreen } from './WorkView';

const source: Source = {
  loadCopies: async () => fixtureCopies(),
  loadItems: async () => fixtureItems(),
  loadProjects: async () => fixtureProjects(),
  loadWork: async (n) => fixtureWork(n),
  loadVersion: async (n, block, number) => fixtureVersion(n, block, number),
  loadRound: async (n, run, round) => fixtureRound(n, run, round),
  loadTold: async (n, run, round) => fixtureTold(n, run, round),
  loadDoing: async (n, run, round) => fixtureDoing(n, run, round),
  followDoing: () => () => undefined,
  loadChanges: async () => {
    throw new Error('not read here');
  },
  loadDiff: async () => {
    throw new Error('not read here');
  },
  loadSystem: async (n, line, session) => fixtureSystem(n, line, session),
  loadCall: async (n, call) => fixtureCall(n, call),
  act: async () => null,
  changes: () => () => undefined,
};

let unmount: (() => void) | undefined;
afterEach(async () => {
  await browserPage.viewport(1280, 800);
  unmount?.();
  unmount = undefined;
  document.body.innerHTML = '';
  localStorage.clear();
});

describe("a work's page opened on a round's address", () => {
  it("scrolls the item's window to the round, in view, and leaves the page where it was", async () => {
    await browserPage.viewport(1280, 360);
    const page = fixtureWork(41);
    const run = page.steps.find((s) => s.kind === 'run');
    if (run?.kind !== 'run' || run.rounds.length < 2) throw new Error('the first run did not run twice');
    const host = document.createElement('div');
    document.body.append(host);
    const root = createRoot(host);
    flushSync(() =>
      root.render(
        <SourceContext.Provider value={source}>
          <WorkScreen page={page} onActed={() => undefined} run={run.run} round={run.rounds[1].number} />
        </SourceContext.Provider>,
      ),
    );
    unmount = () => flushSync(() => root.unmount());
    await expect.poll(() => host.querySelector('section.cw-fold-chosen')).not.toBeNull();
    const body = host.querySelector('aside .drawer-body') as HTMLElement;
    const lit = host.querySelector('section.cw-fold-chosen') as HTMLElement;
    // The window has scrolled, and the section's middle lies in the part of it that is seen, inside the browser's view.
    expect(body.scrollHeight).toBeGreaterThan(body.clientHeight);
    await expect
      .poll(() => {
        const seen = body.getBoundingClientRect();
        const at = lit.getBoundingClientRect();
        const middle = (at.top + at.bottom) / 2;
        return (
          body.scrollTop > 0 &&
          middle >= seen.top &&
          middle <= seen.bottom &&
          at.left >= Math.max(seen.left, 0) &&
          at.right <= Math.min(seen.right, window.innerWidth)
        );
      })
      .toBe(true);
    // The page itself has not been scrolled to it.
    expect(window.scrollX).toBe(0);
    expect(window.scrollY).toBe(0);
  });
});
