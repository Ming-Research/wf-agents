// @vitest-environment happy-dom
import '../dom-setup';

import { act, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import type { Changed, Engine } from '../works-api';
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
  it('says how the engine stands now: its pause, its slots, the copies, the works by where they stand, the rounds under way', async () => {
    const s = sources({ slots: 3, taken: 1, paused: { since: Date.now(), by: 'owner' } });
    const { container } = draw(<SystemView period="week" />, s);
    await settle();
    expect(container.querySelector('.view-facts')?.textContent).toContain('slots 1 taken of 3');
    const now = part(container, 'Now');
    const pairs = texts(now, 'dd');
    expect(pairs[0]).toContain('Every work paused since');
    expect(pairs[1]).toBe('1 taken of 3');
    expect(pairs[2]).toBe('none');
    expect(pairs[3]).toContain('The newest copy of the database was taken');
    // Each work in one place, counted under its standing.
    const items = fixtureItems().items;
    for (const [standing, words] of Object.entries(STANDING_WORDS)) {
      const n = items.filter((i) => standingOf(i.work) === standing).length;
      const tile = [...now.querySelectorAll('.tile')].find((t) => t.querySelector('.label')?.textContent === words.name);
      expect(tile?.querySelector('.tile-value')?.textContent, words.name).toBe(String(n));
    }
    const running = items.filter((i) => i.work.now.kind === 'round');
    expect(running.length).toBeGreaterThan(0);
    const under = now.querySelectorAll('table')[0];
    expect(texts(under, 'tbody td:first-child')).toEqual(running.map((i) => `W${i.work.number}`));
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
});
