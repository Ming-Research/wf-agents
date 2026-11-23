// @vitest-environment happy-dom
import '../dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import { ItemTitlesProvider } from '../itemNumbers';
import type { Observation } from '../observations-api';
import { type Source, SourceContext } from '../works/source';
import type { Changed, Item } from '../works-api';
import { fixtureObservations } from './fixtures';
import { type ObservationsSource, ObservationsView } from './ObservationsView';

let unmounts: (() => void)[] = [];
afterEach(() => {
  for (const u of unmounts) u();
  unmounts = [];
  document.body.innerHTML = '';
  localStorage.clear();
});

/// When the owner reads an observation in these tests.
const NOW = new Date(2026, 10, 14, 10, 30).getTime();

/// The engine as the view reaches it: the observations it holds, the readings it was asked to keep, and the way to
/// say that a work changed. A reading is kept once, at `NOW`; one of an observation it does not hold, or every one
/// where `refuse` says why, is refused.
function engine(refuse?: string) {
  const held: Observation[] = fixtureObservations().observations.map((o) => ({ ...o }));
  const asked: number[] = [];
  let changed: (c: Changed) => void = () => undefined;
  const observations: ObservationsSource = {
    load: async () => ({ observations: held.map((o) => ({ ...o })) }),
    read: async (number) => {
      asked.push(number);
      const o = held.find((x) => x.number === number);
      if (refuse !== undefined) throw new Error(refuse);
      if (!o) throw new Error(`There is no observation O${number}.`);
      o.read ??= NOW;
      return { read: o.read };
    },
  };
  const never = () => Promise.reject(new Error('not read'));
  // The works' source, of which the view listens only to the stream of changes.
  const works: Source = {
    loadCopies: never,
    loadItems: never,
    loadProjects: never,
    loadWork: never,
    loadVersion: never,
    loadRound: never,
    loadTold: never,
    loadDoing: never,
    followDoing: () => () => undefined,
    loadChanges: never,
    loadDiff: never,
    loadSystem: never,
    loadCall: never,
    act: async () => null,
    changes: (onChanged) => {
      changed = onChanged;
      return () => undefined;
    },
  };
  return { held, asked, observations, works, changed: (c: Changed) => changed(c) };
}

/// Lets what was set going (reads, a reading) come to its end.
async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

async function drawn(e: ReturnType<typeof engine>, items: Item[] = []): Promise<HTMLElement> {
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  act(() =>
    root.render(
      <SourceContext.Provider value={e.works}>
        <ItemTitlesProvider items={{ items }}>
          <ObservationsView source={e.observations} />
        </ItemTitlesProvider>
      </SourceContext.Provider>,
    ),
  );
  unmounts.push(() => act(() => root.unmount()));
  await settle();
  return container;
}

const row = (page: ParentNode, number: number) => page.querySelector(`[data-observation="${number}"]`);
const unread = (page: ParentNode) =>
  [...page.querySelectorAll('.observation')].map((r) => r.querySelector('.dot-unread') !== null);

describe('the observations', () => {
  it('lists every observation, the newest first, the unread marked, each with where it came from', async () => {
    const page = await drawn(engine());
    const rows = [...page.querySelectorAll('.observation')];
    expect(rows.map((r) => r.getAttribute('data-observation'))).toEqual(['3', '2', '1']);
    expect(unread(page)).toEqual([true, true, false]);
    expect(page.querySelector('.view-facts')?.textContent).toBe('3 observations');
    // The mark is explained where it is used.
    expect(page.querySelector('.legend')?.textContent?.trim()).toBe('unread');
    const two = row(page, 2) as HTMLElement;
    expect(two.querySelector('.observation-text')?.textContent).toBe('The greeting has no test of an empty name.');
    // Its source opens its work with its run and its round chosen.
    const source = two.querySelector('a.link');
    expect(source?.getAttribute('href')).toBe('#/works/1/runs/7/rounds/1');
    expect(source?.getAttribute('title')).toBe('Open the work with this round chosen');
    expect(source?.textContent).toBe('W1');
    const facts = two.querySelector('.observation-facts')?.textContent ?? '';
    expect(facts).toContain('O2');
    expect(facts).toContain('cell I1');
    expect(facts).toContain('#1 Greet the user');
    // One read is muted, says when, and has nothing to press.
    const one = row(page, 1) as HTMLElement;
    expect(one.classList.contains('observation-read')).toBe(true);
    expect(one.querySelector('button')).toBeNull();
    expect(one.querySelector('.observation-end')?.textContent).toMatch(/^read \d\d-\d\d \d\d:\d\d$/);
    expect(two.querySelector('.observation-end button')?.textContent).toBe('Mark read');
  });

  it('marks one read as the owner asks, and shows it read once the engine has kept it', async () => {
    const e = engine();
    const page = await drawn(e);
    await act(async () => (row(page, 2)?.querySelector('button') as HTMLButtonElement).click());
    await settle();
    expect(e.asked).toEqual([2]);
    expect(unread(page)).toEqual([true, false, false]);
    const two = row(page, 2) as HTMLElement;
    expect(two.classList.contains('observation-read')).toBe(true);
    expect(two.querySelector('button')).toBeNull();
    expect(two.querySelector('.observation-end')?.textContent).toMatch(/^read /);
  });

  it('says why a reading was refused, and keeps the observation unread', async () => {
    const e = engine('the disk is full');
    const page = await drawn(e);
    await act(async () => (row(page, 3)?.querySelector('button') as HTMLButtonElement).click());
    await settle();
    expect(page.querySelector('[role="alert"]')?.textContent).toBe('O3 could not be marked read: the disk is full');
    expect(unread(page)).toEqual([true, true, false]);
    expect((row(page, 3)?.querySelector('button') as HTMLButtonElement).disabled).toBe(false);
  });

  it('reads the list again when a work changes, as a run that finishes keeps the observations of its return', async () => {
    const e = engine();
    const page = await drawn(e);
    e.held.unshift({ ...e.held[1], number: 4, text: 'A run kept another.' });
    act(() => e.changed({ works: [1], items: false, copies: false, accounts: false, observations: false }));
    await settle();
    expect([...page.querySelectorAll('.observation')].map((r) => r.getAttribute('data-observation'))).toEqual([
      '4',
      '3',
      '2',
      '1',
    ]);
    expect(row(page, 4)?.querySelector('.observation-text')?.textContent).toBe('A run kept another.');
  });

  it('says who reported one an architect added, which has no work to open, and one a run kept still opens its work', async () => {
    const e = engine();
    e.held.unshift({
      ...e.held[0],
      number: 4,
      text: 'The merge cell names no tests.',
      project: 'wf-agents',
      reportedBy: 'compiler',
      round: null,
    });
    const page = await drawn(e);
    const four = row(page, 4) as HTMLElement;
    expect(four.querySelector('a.link')).toBeNull();
    expect(four.querySelector('.observation-facts')?.textContent).toContain('reported by compiler');
    expect(four.querySelector('.observation-facts')?.textContent).not.toContain('cell');
    // One a run kept has its work to open, with the round that kept it chosen.
    expect((row(page, 2) as HTMLElement).querySelector('a.link')?.getAttribute('href')).toBe(
      '#/works/1/runs/7/rounds/1',
    );
  });

  it('reads the list again when the stream says an observation was added or changed, and no work changed', async () => {
    const e = engine();
    const page = await drawn(e);
    e.held.unshift({ ...e.held[0], number: 4, text: 'An architect added another.', round: null, reportedBy: 'compiler' });
    act(() => e.changed({ works: [], items: false, copies: false, accounts: false, observations: true }));
    await settle();
    expect([...page.querySelectorAll('.observation')].map((r) => r.getAttribute('data-observation'))).toEqual([
      '4',
      '3',
      '2',
      '1',
    ]);
  });

  it('draws the number of an item in the text as a link to the item, where the page has read the item', async () => {
    const e = engine();
    const text = 'Item #46 and #9999 both say it; the test of #46 is missing.';
    (e.held.find((o) => o.number === 2) as Observation).text = text;
    const linked = await drawn(e, [{ number: 46, title: 'Link the numbers' } as Item]);
    const links = [...(row(linked, 2)?.querySelectorAll('.observation-text a') ?? [])];
    expect(links.map((a) => [a.getAttribute('href'), a.getAttribute('title'), a.textContent])).toEqual([
      ['#/console/items/46', '#46 Link the numbers', '#46'],
      ['#/console/items/46', '#46 Link the numbers', '#46'],
    ]);
    expect(row(linked, 2)?.querySelector('.observation-text')?.textContent).toBe(text);
    // With no item read, the text is as it was written.
    const plain = await drawn(e);
    expect(row(plain, 2)?.querySelectorAll('.observation-text a').length).toBe(0);
  });

  it('says so when there is none', async () => {
    const e = engine();
    e.held.splice(0);
    const page = await drawn(e);
    expect(page.querySelector('.list-view')?.textContent).toBe('No observation yet');
    expect(page.querySelector('.view-facts')?.textContent).toBe('0 observations');
  });
});
