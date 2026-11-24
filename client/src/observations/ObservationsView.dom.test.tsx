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
/// where `refuse` says why, is refused. The projects the installation names are `projects`, or cannot be read.
function engine(refuse?: string, projects?: string[]) {
  const held: Observation[] = fixtureObservations().observations.map((o) => ({ ...o }));
  const asked: number[] = [];
  // Every part that listens to the stream: the view, and its reading of the projects.
  const listening = new Set<(c: Changed) => void>();
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
  // The works' source, of which the view reads only the projects and listens only to the stream of changes.
  const works: Source = {
    loadCopies: never,
    loadItems: never,
    loadProjects: projects === undefined ? never : async () => ({ names: projects }),
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
      listening.add(onChanged);
      return () => listening.delete(onChanged);
    },
  };
  return { held, asked, observations, works, changed: (c: Changed) => listening.forEach((on) => on(c)) };
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

/// The view's groups, each its section, in order.
const groupsOf = (page: ParentNode) => [...page.querySelectorAll<HTMLElement>('main > section.project-group')];

/// A group: the name its head says, its count with what it counts, its note, the button that folds it, and its rows'
/// numbers in order.
const groupOf = (group: HTMLElement) => {
  const count = group.querySelector('.project-head .project-count') as HTMLElement;
  return {
    name: group.querySelector('.project-head button > .mono, .project-head button > .muted')?.textContent,
    count: `${count.textContent} (${count.title})`,
    note: group.querySelector('.project-head .project-note')?.textContent,
    button: group.querySelector('.project-head button') as HTMLButtonElement,
    rows: [...group.querySelectorAll('.observation')].map((r) => r.getAttribute('data-observation')),
  };
};

const facts = (page: ParentNode, number: number) => row(page, number)?.querySelector('.observation-facts')?.textContent;

describe('the observations', () => {
  it("lists every observation in its project's group, the newest first, the unread marked, each with where it came from", async () => {
    const page = await drawn(engine());
    // Where the projects cannot be read, the groups are those of the projects the observations are of.
    expect(groupsOf(page).map((g) => [groupOf(g).name, groupOf(g).rows])).toEqual([
      ['Whitefoot', ['4']],
      ['example', ['3', '2', '1']],
      ['wf-agents', ['5']],
    ]);
    expect(unread(page)).toEqual([false, true, true, false, true]);
    expect(page.querySelector('.view-facts')?.textContent).toBe('5 observations');
    // The mark is explained where it is used.
    expect(page.querySelector('.legend')?.textContent?.trim()).toBe('unread');
    const two = row(page, 2) as HTMLElement;
    expect(two.querySelector('.observation-text')?.textContent).toBe('The greeting has no test of an empty name.');
    // Its source opens its work with its run and its round chosen.
    const source = two.querySelector('a.link');
    expect(source?.getAttribute('href')).toBe('#/works/1/runs/7/rounds/1');
    expect(source?.getAttribute('title')).toBe('Open the work with this round chosen');
    expect(source?.textContent).toBe('W1');
    expect(facts(page, 2)).toContain('O2');
    expect(facts(page, 2)).toContain('cell I1');
    expect(facts(page, 2)).toContain('#1 Greet the user');
    // One read is muted, says when, and has nothing to press.
    const one = row(page, 1) as HTMLElement;
    expect(one.classList.contains('observation-read')).toBe(true);
    expect(one.querySelector('button')).toBeNull();
    expect(one.querySelector('.observation-end')?.textContent).toMatch(/^read \d\d-\d\d \d\d:\d\d$/);
    expect(two.querySelector('.observation-end button')?.textContent).toBe('Mark read');
  });

  it("heads each group outside its rows with the project's name, how many it holds and how many of them are unread", async () => {
    const page = await drawn(engine());
    expect(groupsOf(page).map((g) => [groupOf(g).count, groupOf(g).note])).toEqual([
      ['1 (1 observation)', undefined],
      ['3 (3 observations)', '2 unread'],
      ['1 (1 observation)', '1 unread'],
    ]);
    for (const g of groupsOf(page)) expect(g.querySelector('.project-head')?.closest('ul')).toBeNull();
  });

  it('shows a group for every project the installation names, one with no observation saying so', async () => {
    const page = await drawn(engine(undefined, ['Whitefoot', 'example', 'project-4', 'wf-agents']));
    const groups = groupsOf(page);
    expect(groups.map((g) => groupOf(g).name)).toEqual(['Whitefoot', 'example', 'project-4', 'wf-agents']);
    expect([groupOf(groups[2]).count, groupOf(groups[2]).note]).toEqual(['0 (0 observations)', undefined]);
    expect(groups[2].querySelector('.message')?.textContent).toBe('No observation of project project-4 yet');
  });

  it('folds a group away at a press on its head and opens it again at the next, and keeps which are folded in the browser', async () => {
    const page = await drawn(engine());
    const example = () => groupOf(groupsOf(page)[1]);
    await act(async () => example().button.click());
    expect(example().button.getAttribute('aria-expanded')).toBe('false');
    expect(example().rows).toEqual([]);
    expect(example().count).toBe('3 (3 observations)');
    expect(groupOf(groupsOf(page)[0]).rows).toEqual(['4']);
    expect(JSON.parse(localStorage.getItem('wf-agents.observations-folded') ?? '')).toEqual(['example']);
    // Drawn anew, the view keeps the group folded; the list of works keeps its folds apart.
    const again = await drawn(engine());
    expect(groupsOf(again).map((g) => groupOf(g).button.getAttribute('aria-expanded'))).toEqual(['true', 'false', 'true']);
    expect(localStorage.getItem('wf-agents.works-folded')).toBeNull();
    await act(async () => example().button.click());
    expect(example().rows).toEqual(['3', '2', '1']);
  });

  it('names the project that reported one only where that is another than the one it is of', async () => {
    const e = engine();
    e.held.unshift({ ...e.held[0], number: 6, text: 'Reported by its own project.', project: 'example', reportedBy: 'example' });
    const page = await drawn(e);
    // O5 is of wf-agents, and Whitefoot reported it; its group's head says the project it is of.
    expect(facts(page, 5)).toContain('reported by Whitefoot');
    expect(facts(page, 5)).not.toContain('wf-agents');
    expect(facts(page, 6)).not.toContain('reported by');
    // One a run kept is reported by the project of its work's item, which is its own.
    expect(facts(page, 4)).not.toContain('reported by');
  });

  it('marks one read as the owner asks, and shows it read once the engine has kept it', async () => {
    const e = engine();
    const page = await drawn(e);
    await act(async () => (row(page, 2)?.querySelector('button') as HTMLButtonElement).click());
    await settle();
    expect(e.asked).toEqual([2]);
    expect(unread(page)).toEqual([false, true, false, false, true]);
    expect(groupOf(groupsOf(page)[1]).note).toBe('1 unread');
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
    expect(unread(page)).toEqual([false, true, true, false, true]);
    expect((row(page, 3)?.querySelector('button') as HTMLButtonElement).disabled).toBe(false);
  });

  it('reads the list again when a work changes, as a run that finishes keeps the observations of its return', async () => {
    const e = engine();
    const page = await drawn(e);
    e.held.unshift({ ...e.held[2], number: 6, text: 'A run kept another.' });
    act(() => e.changed({ works: [1], items: false, copies: false, accounts: false, observations: false }));
    await settle();
    expect(groupOf(groupsOf(page)[1]).rows).toEqual(['6', '3', '2', '1']);
    expect(row(page, 6)?.querySelector('.observation-text')?.textContent).toBe('A run kept another.');
  });

  // What the calls of the rounds under way have come to comes with every call and holds no observation: a change that
  // says only that does not read the list again; any other does. Fails where the list is read for each call.
  it('is not read again for what the calls of the rounds under way have come to', async () => {
    const e = engine();
    const page = await drawn(e);
    e.held.unshift({ ...e.held[2], number: 6, text: 'A run kept another.' });
    const only = { works: [], items: false, copies: false, accounts: false, observations: false, going: true };
    act(() => e.changed(only));
    await settle();
    expect(row(page, 6)).toBeNull();
    act(() => e.changed({ ...only, works: [1] }));
    await settle();
    expect(row(page, 6)?.querySelector('.observation-text')?.textContent).toBe('A run kept another.');
  });

  it('says who reported one an architect added, which has no work to open, and one a run kept still opens its work', async () => {
    const e = engine();
    e.held.unshift({
      ...e.held[0],
      number: 6,
      text: 'The merge cell names no tests.',
      project: 'wf-agents',
      reportedBy: 'compiler',
      round: null,
    });
    const page = await drawn(e);
    const six = row(page, 6) as HTMLElement;
    expect(six.querySelector('a.link')).toBeNull();
    expect(facts(page, 6)).toContain('reported by compiler');
    expect(facts(page, 6)).not.toContain('cell');
    // One a run kept has its work to open, with the round that kept it chosen.
    expect((row(page, 2) as HTMLElement).querySelector('a.link')?.getAttribute('href')).toBe(
      '#/works/1/runs/7/rounds/1',
    );
  });

  it('reads the list again when the stream says an observation was added or changed, and no work changed', async () => {
    const e = engine();
    const page = await drawn(e);
    e.held.unshift({ ...e.held[0], number: 6, text: 'An architect added another.', round: null, reportedBy: 'compiler' });
    act(() => e.changed({ works: [], items: false, copies: false, accounts: false, observations: true }));
    await settle();
    expect(groupOf(groupsOf(page)[2]).rows).toEqual(['6', '5']);
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
