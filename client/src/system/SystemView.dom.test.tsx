// @vitest-environment happy-dom
import '../dom-setup';

import { act, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { Changed, Engine, Item, Items, Outage, Wait } from '../works-api';
import { waitingCount } from '../works/waiting';
import { waitCount, waitCounted } from '../works/words';
import { fixtureCopies, fixtureItems, fixtureWork } from '../works/fixtures';
import { type Source, SourceContext } from '../works/source';
import { type SystemSource, SystemSourceContext } from './api';
import { fixtureAccounts, fixtureSettings, fixtureSystem } from './fixtures';
import { STANDING_WORDS, standingOf } from './standing';
import { SystemView } from './SystemView';

let unmounts: (() => void)[] = [];
afterEach(() => {
  for (const u of unmounts) u();
  unmounts = [];
  document.body.innerHTML = '';
  window.location.hash = '';
});

/// The works' fixtures with an engine, and the system's, each read counted; the parts that listen are told of a change
/// when the test says.
function sources(engine: Engine = { slots: 2, taken: 1, paused: null }) {
  const listening = new Set<{ onChanged: (c: Changed) => void; onOpened: () => void }>();
  const asked: number[][] = [];
  const works: Source = {
    loadCopies: async () => fixtureCopies(),
    loadItems: async () => fixtureItems(),
    loadProjects: async () => ({ names: [] }),
    loadWork: async (n) => fixtureWork(n),
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
    loadSystem: async () => {
      throw new Error('not read here');
    },
    loadCall: async () => {
      throw new Error('not read here');
    },
    act: async () => null,
    loadEngine: async () => engine,
    changes: (onChanged, onOpened) => {
      const l = { onChanged, onOpened };
      listening.add(l);
      return () => listening.delete(l);
    },
  };
  const system: SystemSource = {
    loadSystem: async (weeks) => {
      asked.push(weeks);
      return fixtureSystem(weeks);
    },
    loadSettings: async () => fixtureSettings(),
    saveSettings: async () => ({ said: '', committed: false }),
    loadAccounts: async () => fixtureAccounts(),
    chooseAccount: async () => ({ said: '', committed: false }),
  };
  return { works, system, asked, changed: (c: Changed) => listening.forEach((l) => l.onChanged(c)) };
}

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

function draw(element: ReactNode, s: ReturnType<typeof sources>) {
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  const render = (e: ReactNode) =>
    act(() =>
      root.render(
        <SourceContext.Provider value={s.works}>
          <SystemSourceContext.Provider value={s.system}>{e}</SystemSourceContext.Provider>
        </SourceContext.Provider>,
      ),
    );
  render(element);
  unmounts.push(() => act(() => root.unmount()));
  return { container, render };
}

const texts = (root: ParentNode, selector: string) => [...root.querySelectorAll(selector)].map((e) => e.textContent?.trim() ?? '');

/// A part of the view by its label.
function part(root: ParentNode, label: string): HTMLElement {
  const found = [...root.querySelectorAll('section.panel-part')].find(
    (s) => s.querySelector('.panel-part-label')?.textContent?.startsWith(label),
  );
  if (!found) throw new Error(`no part ${label}`);
  return found as HTMLElement;
}

describe('the System view', () => {
  it('shows no copies of the database where the engine keeps none, as one that runs a script does', async () => {
    const s = sources();
    s.works.loadCopies = async () => ({ ...fixtureCopies(), state: { kind: 'notKept' } });
    const { container } = draw(<SystemView period="week" />, s);
    await settle();
    expect(texts(part(container, 'Now'), 'dt')).not.toContain('Copies of the database');
    expect(container.textContent).not.toContain('could not be read');
  });

  it('says how the engine stands now: its pause, its slots, the copies, the works by where they stand, the rounds under way', async () => {
    const s = sources({ slots: 3, taken: 1, paused: { since: Date.now(), by: 'owner' } });
    const { container } = draw(<SystemView period="week" />, s);
    await settle();
    expect(container.querySelector('.view-facts')).toBeNull();
    const now = part(container, 'Now');
    const pairs = texts(now, 'dd');
    expect(pairs[0]).toContain('Every work paused since');
    expect(pairs[1]).toBe('1 taken of 3');
    expect(pairs[2]).toBe('none');
    expect(pairs[3]).toContain('The newest copy of the database was taken');
    // Each work in one place, counted under its standing.
    const items = fixtureItems().items;
    for (const [standing, words] of Object.entries(STANDING_WORDS)) {
      if (standing === 'owner') continue;
      const n = items.filter((i) => standingOf(i.work) === standing).length;
      const tile = [...now.querySelectorAll('.tile')].find((t) => t.querySelector('.label')?.textContent === words.name);
      // The works whose round is under way are in the list of the rounds under way, and nowhere else.
      if (standing === 'running') expect(tile).toBeUndefined();
      else expect(tile?.querySelector('.tile-value')?.textContent, words.name).toBe(String(n));
    }

    const running = items.filter((i) => i.work.now.kind === 'round');
    expect(running.length).toBeGreaterThan(0);
    const under = now.querySelectorAll('table')[0];
    expect(texts(under, 'tbody td:first-child')).toEqual(running.map((i) => `W${i.work.number}`));
  });

  it('counts what waits for the owner by the one count, the things that wait, as the bar says it', async () => {
    const s = sources();
    const items = fixtureItems();
    const first = items.items.find((i) => standingOf(i.work) === 'owner') as Item;
    const extra: Wait = { key: 'limit', why: 'The cell ran 3 times in a row, its limit.', ends: { kind: 'letGo', cell: 'plan' } };
    const more: Items = {
      ...items,
      items: items.items.map((i) => (i === first ? { ...i, work: { ...i.work, waits: [...(i.work.waits ?? []), extra] } } : i)),
    };
    s.works.loadItems = async () => more;
    const { container } = draw(<SystemView period="week" />, s);
    await settle();
    const works = more.items.filter((i) => standingOf(i.work) === 'owner');
    const waiting = waitingCount(works.map((i) => i.work));
    expect(waiting).toBe(works.length + 1);
    const tile = [...part(container, 'Now').querySelectorAll('.tile')].find(
      (t) => t.querySelector('.label')?.textContent === waitCounted(waiting),
    );
    expect(`${tile?.querySelector('.tile-value')?.textContent} ${tile?.querySelector('.label')?.textContent}`).toBe(
      waitCount(waiting),
    );
  });

  /// A wait every work shares, the check of the outside having found it down, as the latest check says it.
  const sharedWait = (holds: boolean): Outage => ({
    service: 'agents',
    failure: 'cannotRun',
    why: 'Not logged in',
    work: 3,
    run: 2,
    round: 1,
    since: 0,
    checked: {
      at: 0,
      taken: 2,
      found: holds
        ? { kind: 'down', failure: 'cannotRun', why: 'It is not signed in, as `claude auth status` says.' }
        : { kind: 'well' },
    },
    until: { kind: holds ? 'checked' : 'well', at: 0 },
    holds,
  });

  it('says none where the wait every work shares holds no work, in no colour of a problem', async () => {
    const s = sources();
    const items = fixtureItems();
    s.works.loadItems = async () => ({ ...items, outage: sharedWait(false) });
    const { container } = draw(<SystemView period="week" />, s);
    await settle();
    const shared = part(container, 'Now').querySelectorAll('dd')[2] as HTMLElement;
    expect(shared.textContent).toBe('none');
    expect(shared.classList.contains('failed')).toBe(false);
  });

  it("says the wait every work shares in sentences, the names and values it marks in the monospace", async () => {
    const s = sources();
    const items = fixtureItems();
    s.works.loadItems = async () => ({ ...items, outage: sharedWait(true) });
    const { container } = draw(<SystemView period="week" />, s);
    await settle();
    const shared = part(container, 'Now').querySelectorAll('dd')[2] as HTMLElement;
    expect([...shared.querySelectorAll('code')].map((c) => c.textContent)).toEqual(['claude auth status']);
    expect(shared.textContent).not.toContain('`');
    expect(shared.textContent).toMatch(
      / the check found: It is not signed in, as claude auth status says\. No agent's round begins until it runs again\.$/,
    );
    expect(shared.textContent).not.toContain('Not logged in');
    expect(shared.textContent).not.toContain('W3');
  });

  it('unfolds the works a count counts, and folds them again', async () => {
    const s = sources();
    const { container } = draw(<SystemView period="week" />, s);
    await settle();
    const now = part(container, 'Now');
    const halted = [...now.querySelectorAll('button.tile')].find((t) => t.textContent?.includes('Halted')) as HTMLElement;
    act(() => halted.click());
    expect(halted.getAttribute('aria-expanded')).toBe('true');
    const list = now.querySelector('table[aria-label="Halted"]');
    const expected = fixtureItems()
      .items.filter((i) => standingOf(i.work) === 'halted')
      .map((i) => `W${i.work.number}`);
    expect(texts(list!, 'tbody td:first-child')).toEqual(expected);
    act(() => halted.click());
    expect(now.querySelector('table[aria-label="Halted"]')).toBeNull();
  });

  it('counts over the span picked, kept in the address, and reads the span again when a work changes', async () => {
    const s = sources();
    const { container, render } = draw(<SystemView period="week" />, s);
    await settle();
    expect(s.asked).toHaveLength(1);
    expect(s.asked[0]).toHaveLength(1);
    expect(new Date(s.asked[0][0]).getDay()).toBe(1);
    const produced = part(container, 'Produced');
    expect(produced.querySelector('.tile-value')?.textContent).toBe(String(fixtureSystem(s.asked[0]).merged.length));
    const spent = part(container, 'Spent');
    expect(texts(spent, '.tile .label')).toEqual(['Cost', 'Rounds', 'Under way', 'Tokens', 'Read from the cache']);
    expect(texts(spent, 'h4')).toContain('By cell');
    const wrong = part(container, 'Gone wrong');
    expect(wrong.textContent).toContain("The agent's program could not start.");
    const all = [...container.querySelectorAll('.switch-choice')].find((b) => b.textContent === 'All time') as HTMLElement;
    act(() => all.click());
    expect(window.location.hash).toBe('#/system/all');
    render(<SystemView period="all" />);
    await settle();
    // All time from the week the earliest item was added in.
    const earliest = Math.min(...fixtureItems().items.map((i) => i.added));
    expect(s.asked.at(-1)?.[0]).toBeLessThanOrEqual(earliest);
    expect(s.asked.at(-1)?.[0]).toBeGreaterThan(earliest - 7 * 24 * 60 * 60 * 1000);
    const before = s.asked.length;
    act(() => s.changed({ works: [41], items: false, copies: false }));
    await settle();
    expect(s.asked.length).toBe(before + 1);
    act(() => s.changed({ works: [], items: false, copies: true }));
    await settle();
    expect(s.asked.length).toBe(before + 1);
  });

  it("cuts the week anew once Monday's midnight has passed, and reads the span then", async () => {
    vi.useFakeTimers({ toFake: ['Date', 'setTimeout', 'clearTimeout'] });
    try {
      // A Sunday a minute before midnight, in the browser's time zone.
      vi.setSystemTime(new Date(2026, 10, 15, 23, 59));
      const s = sources();
      draw(<SystemView period="week" />, s);
      await act(async () => {
        await vi.advanceTimersByTimeAsync(0);
      });
      expect(s.asked.at(-1)).toEqual([new Date(2026, 10, 9).getTime()]);
      await act(async () => {
        await vi.advanceTimersByTimeAsync(2 * 60 * 1000);
      });
      expect(s.asked.at(-1)).toEqual([new Date(2026, 10, 16).getTime()]);
    } finally {
      vi.useRealTimers();
    }
  });
});
