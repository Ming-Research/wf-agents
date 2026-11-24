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

/// The engine as the view reaches it: the observations it holds, and the way to say that a work changed. The projects
/// the installation names are `projects`, or cannot be read.
function engine(projects?: string[]) {
  const held: Observation[] = fixtureObservations().observations.map((o) => ({ ...o }));
  // Every part that listens to the stream: the view, and its reading of the projects.
  const listening = new Set<(c: Changed) => void>();
  const observations: ObservationsSource = {
    load: async () => ({ observations: held.map((o) => ({ ...o })) }),
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
  return { held, observations, works, changed: (c: Changed) => listening.forEach((on) => on(c)) };
}

/// Lets what was set going (reads) come to its end.
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

/// The items of the works fixtures that took observations in, as the page has read them: one whose round is under
/// way, one merged and one cancelled.
const TAKERS = [
  { number: 142, title: "Keep the panel's width" },
  { number: 147, title: "Show a block's signs" },
  { number: 148, title: 'Draw the chart in a canvas' },
] as Item[];

const row = (page: ParentNode, number: number) => page.querySelector(`[data-observation="${number}"]`);

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

/// Whether an observation is handled, as its row's end says it: its dot's class, its word and what it says on pointing.
const standing = (page: ParentNode, number: number) => {
  const end = row(page, number)?.querySelector('.observation-end') as HTMLElement;
  return { dot: end.querySelector('.dot')?.className, word: end.textContent, title: end.title };
};

describe('the observations', () => {
  it("lists every observation in its project's group, the newest first, each with where it came from", async () => {
    const page = await drawn(engine());
    // Where the projects cannot be read, the groups are those of the projects the observations are of.
    expect(groupsOf(page).map((g) => [groupOf(g).name, groupOf(g).rows])).toEqual([
      ['Whitefoot', ['4']],
      ['example', ['3', '2', '1']],
      ['wf-agents', ['5']],
    ]);
    expect(page.querySelector('.view-facts')?.textContent).toBe('5 observations');
    const two = row(page, 2) as HTMLElement;
    expect(two.querySelector('.observation-text')?.textContent).toBe('The greeting has no test of an empty name.');
    // Its source opens its work with its run and its round chosen.
    const source = two.querySelector('.observation-facts a.link');
    expect(source?.getAttribute('href')).toBe('#/works/1/runs/7/rounds/1');
    expect(source?.getAttribute('title')).toBe('Open the work with this round chosen');
    expect(source?.textContent).toBe('W1');
    expect(facts(page, 2)).toContain('O2');
    expect(facts(page, 2)).toContain('cell I1');
    expect(facts(page, 2)).toContain('#1 Greet the user');
  });

  it('says of each whether it is handled, as an item says it is done or open, and nothing of whether it was read', async () => {
    const e = engine();
    // The engine still says when the owner read one; the page shows none of it.
    for (const o of e.held) o.read = new Date(2026, 10, 14, 10, 30).getTime();
    const page = await drawn(e, TAKERS);
    const handled = {
      dot: 'dot dot-added',
      word: 'Handled',
      title: 'An item whose work is not cancelled took it in',
    };
    const open = {
      dot: 'dot dot-running',
      word: 'Open',
      title: 'No item took it in yet, or only one whose work was cancelled',
    };
    expect([5, 4, 3, 2, 1].map((n) => standing(page, n))).toEqual([open, handled, open, handled, open]);
    // Nothing on the page marks one read or unread, or asks for it to be.
    expect(page.querySelectorAll('.observation button')).toHaveLength(0);
    expect(page.querySelector('.legend')).toBeNull();
    expect(page.querySelector('.dot-unread')).toBeNull();
    for (const n of [5, 4, 3, 2, 1]) expect(facts(page, n)).not.toContain('read 11-14');
  });

  it('names the item that took one in, leading to it, and strikes its title through where its work was cancelled', async () => {
    const page = await drawn(engine(), TAKERS);
    const taken = (n: number) => row(page, n)?.querySelector('.observation-facts a[href^="#/console/"]');
    expect(facts(page, 2)).toContain("taken in by #142 Keep the panel's width");
    expect([taken(2)?.getAttribute('href'), taken(2)?.getAttribute('title'), taken(2)?.textContent]).toEqual([
      '#/console/items/142',
      "#142 Keep the panel's width",
      '#142',
    ]);
    expect(facts(page, 4)).toContain("taken in by #147 Show a block's signs");
    // Taken in by an item whose work was cancelled, it is open, and the item's title is struck through.
    expect(facts(page, 5)).toContain('taken in by #148 Draw the chart in a canvas');
    const struck = row(page, 5)?.querySelector('.observation-taken-cancelled .observation-taken-title') as HTMLElement;
    expect([struck.textContent, struck.title]).toEqual(['Draw the chart in a canvas', 'Its work was cancelled']);
    expect(row(page, 2)?.querySelector('.observation-taken-cancelled')).toBeNull();
    // One no item took in names none.
    expect(facts(page, 3)).not.toContain('taken in by');
  });

  it('names by its number alone an item that took one in which the page has not read, still leading to it', async () => {
    const page = await drawn(engine());
    expect(facts(page, 2)).toContain('taken in by #142');
    expect(row(page, 2)?.querySelector('.observation-taken-title')).toBeNull();
    expect(row(page, 2)?.querySelector('a[href="#/console/items/142"]')?.getAttribute('title')).toBe('#142');
  });

  it("heads each group outside its rows with the project's name, how many it holds and how many of them are open", async () => {
    const page = await drawn(engine());
    expect(groupsOf(page).map((g) => [groupOf(g).count, groupOf(g).note])).toEqual([
      ['1 (1 observation)', undefined],
      ['3 (3 observations)', '2 open'],
      ['1 (1 observation)', '1 open'],
    ]);
    expect((groupsOf(page)[1].querySelector('.project-note') as HTMLElement).title).toBe('2 not handled yet');
    for (const g of groupsOf(page)) expect(g.querySelector('.project-head')?.closest('ul')).toBeNull();
  });

  it('orders the groups as the owner ordered the projects in the console, the others after them as before', async () => {
    localStorage.setItem('wf-agents.project-order', JSON.stringify(['wf-agents', 'gone', 'Whitefoot']));
    const page = await drawn(engine());
    expect(groupsOf(page).map((g) => groupOf(g).name)).toEqual(['wf-agents', 'Whitefoot', 'example']);
  });

  it('shows a group for every project the installation names, one with no observation saying so', async () => {
    const page = await drawn(engine(['Whitefoot', 'example', 'project-4', 'wf-agents']));
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
      takenInBy: null,
      handled: false,
    });
    const page = await drawn(e);
    const six = row(page, 6) as HTMLElement;
    expect(six.querySelector('a.link')).toBeNull();
    expect(facts(page, 6)).toContain('reported by compiler');
    expect(facts(page, 6)).not.toContain('cell');
    // One a run kept has its work to open, with the round that kept it chosen.
    expect((row(page, 2) as HTMLElement).querySelector('.observation-facts a.link')?.getAttribute('href')).toBe(
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
