// @vitest-environment happy-dom
import '../dom-setup';

import { type ReactNode, act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import { age } from '../console/items';
import { levelWords } from '../marks';
import type { Act, Changed, Now, Pauser, Wait, WorkPage } from '../works-api';
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
import { pauseDiffers } from './marks';
import { placeSteps } from './placing';
import { type Source, SourceContext, readerOf } from './source';
import { NOT_KEPT, clock, duration, ranLast, runTook } from './words';
import { WorkScreen, WorkView } from './WorkView';
import { WorksList, WorksTable } from './WorksList';

let unmounts: (() => void)[] = [];
afterEach(() => {
  for (const u of unmounts) u();
  unmounts = [];
  document.body.innerHTML = '';
  localStorage.clear();
});

/// A promise kept apart, answered when the test says.
interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (e: Error) => void;
}

function deferred<T>(): Deferred<T> {
  let resolve: (value: T) => void = () => undefined;
  let reject: (e: Error) => void = () => undefined;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

/// A source answered from the fixtures, which keeps what the owner did and how often each thing was read, and lets a
/// test say that something changed or that the stream opened again.
interface TestSource extends Source {
  acts: { work: number; what: Act }[];
  reads: { work: number; round: number; items: number };
  changed: (c: Changed) => void;
  opened: () => void;
}

function fixtureSource(over: Partial<Source> = {}): TestSource {
  // Every part that listens, as each opens a stream of its own: the list of the works and the line of the copies in it.
  const listening = new Set<{ onChanged: (c: Changed) => void; onOpened: () => void }>();
  const s: TestSource = {
    acts: [],
    reads: { work: 0, round: 0, items: 0 },
    changed: (c) => listening.forEach((l) => l.onChanged(c)),
    opened: () => listening.forEach((l) => l.onOpened()),
    loadCopies: async () => fixtureCopies(),
    loadItems: async () => {
      s.reads.items += 1;
      return fixtureItems();
    },
    loadProjects: async () => fixtureProjects(),
    loadWork: async (n) => {
      s.reads.work += 1;
      return fixtureWork(n);
    },
    loadVersion: async (n, block, number) => fixtureVersion(n, block, number),
    loadRound: async (n, run, round) => {
      s.reads.round += 1;
      return fixtureRound(n, run, round);
    },
    loadTold: async (n, run, round) => fixtureTold(n, run, round),
    loadDoing: async (n, run, round) => fixtureDoing(n, run, round),
    followDoing: () => () => undefined,
    loadChanges: (n) => Promise.reject(new Error(`work ${n} changed no files`)),
    loadDiff: (n) => Promise.reject(new Error(`work ${n} changed no files`)),
    loadSystem: async (n, line, session) => fixtureSystem(n, line, session),
    loadCall: async (n, call) => fixtureCall(n, call),
    act: async (work, what) => {
      s.acts.push({ work, what });
      return null;
    },
    changes: (onChanged, onOpened) => {
      const listener = { onChanged, onOpened };
      listening.add(listener);
      return () => listening.delete(listener);
    },
    ...over,
  };
  return s;
}

/// Lets what was set going (reads, acts) come to its end.
async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

function draw(element: ReactNode, source: Source): HTMLElement {
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  act(() => root.render(<SourceContext.Provider value={source}>{element}</SourceContext.Provider>));
  unmounts.push(() => act(() => root.unmount()));
  return container;
}

/// Draws the work as the fixtures give it, its item read, and counts how often what the owner did had it read again.
async function drawWork(work: number | WorkPage, source = fixtureSource()) {
  const acted = { times: 0 };
  const page = draw(
    <WorkScreen page={typeof work === 'number' ? fixtureWork(work) : work} onActed={() => (acted.times += 1)} />,
    source,
  );
  await settle();
  return { page, source, acted };
}

/// Why the round of the fixtures' work 50 failed.
const FAILED = "The agent's program exited with status 1: the session could not be resumed.";

const texts = (root: ParentNode, selector: string) => [...root.querySelectorAll(selector)].map((e) => e.textContent?.trim() ?? '');

async function click(element: Element | null | undefined) {
  if (!element) throw new Error('nothing to click');
  await act(async () => {
    (element as HTMLElement).click();
    // What the click reads (a content, a round) comes after a few turns of the event loop.
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

const button = (root: ParentNode, words: string) =>
  [...root.querySelectorAll('button')].find((b) => b.textContent?.trim() === words) as HTMLButtonElement | undefined;

/// Picks an option of a choice as the browser would, so that React hears it.
function pick(select: Element | null, value: string) {
  const s = select as HTMLSelectElement;
  act(() => {
    s.value = value;
    s.dispatchEvent(new Event('change', { bubbles: true }));
  });
}

/// The works the list shows, by their numbers, in its order.
const listed = (page: HTMLElement) =>
  [...page.querySelectorAll<HTMLElement>('tbody tr.work-row')].map((r) => Number(r.dataset.work));

/// The rows of the work's history: a run's, and the lines between them.
const runRows = (page: HTMLElement) => [...page.querySelectorAll<HTMLElement>('tbody tr.step-row')];

/// Chooses the run of the row at this place among the runs' rows, by its names at the left.
async function chooseRun(page: HTMLElement, index: number) {
  await click(runRows(page)[index]?.querySelector('.step-lead button'));
}

/// What the item's window shows under its tabs: the Step, once a run is chosen.
const step = (page: HTMLElement) => page.querySelector('aside .drawer-body') as HTMLElement;

/// A part of the Step by its label's first words.
const part = (root: ParentNode, label: string) =>
  [...root.querySelectorAll<HTMLElement>('section.cw-fold')].find((p) =>
    p.querySelector('.cw-fold-head')?.textContent?.startsWith(label),
  ) as HTMLElement;

describe('the list of works', () => {
  it('lists the work of every item, what happened last first, those waiting for the owner edged, and a row opens its work', async () => {
    const page = draw(<WorksTable items={fixtureItems()} />, fixtureSource());
    const rows = [...page.querySelectorAll('tbody tr.work-row')] as HTMLElement[];
    // The list is the record of every work, ordered by when something last happened; a work's state orders it only by
    // its own column (decision the-list-is-ordered-by-a-column).
    expect(rows.map((r) => Number(r.dataset.work))).toEqual([42, 50, 43, 44, 45, 46, 49, 41, 47, 48]);
    expect(rows.map((r) => r.classList.contains('for-owner'))).toEqual([
      false,
      true,
      true,
      true,
      true,
      false,
      false,
      true,
      false,
      false,
    ]);
    // The column of states holds the state and the cell alone, never why (principle a-column-holds-what-is-alike).
    expect(rows[2].querySelector('.now-words')?.textContent).toBe('Waiting for you · plan');
    expect(rows[2].querySelectorAll('.strip .state-mark')).toHaveLength(6);
    await click(rows[7]);
    expect(window.location.hash).toBe('#/works/41');
  });

  it('orders the list by the column whose head is pressed, a second press turning it, and keeps the order in the browser', async () => {
    const page = draw(<WorksTable items={fixtureItems()} />, fixtureSource());
    const head = (name: string) => button(page, name) as HTMLButtonElement;
    await click(head('#'));
    expect(listed(page)).toEqual([50, 49, 48, 47, 46, 45, 44, 43, 42, 41]);
    expect(head('#').closest('th')?.getAttribute('aria-sort')).toBe('descending');
    await click(head('#'));
    expect(listed(page)).toEqual([41, 42, 43, 44, 45, 46, 47, 48, 49, 50]);
    expect(head('#').closest('th')?.getAttribute('aria-sort')).toBe('ascending');
    // By state: what waits for the owner first, then the halted, what runs, what waits for another, what is paused,
    // what merged and what was cancelled; works of one state as the list's first order has them.
    await click(head('State'));
    expect(listed(page)).toEqual([50, 43, 44, 45, 41, 49, 42, 46, 47, 48]);
    // Drawn anew, the list keeps the order last pressed.
    expect(listed(draw(<WorksTable items={fixtureItems()} />, fixtureSource()))).toEqual([
      50, 43, 44, 45, 41, 49, 42, 46, 47, 48,
    ]);
  });

  it("shows, with a project chosen, that project's works and every work waiting for the owner, and every work with none chosen", () => {
    const page = draw(<WorksTable items={fixtureItems()} projects={fixtureProjects().names} />, fixtureSource());
    const select = page.querySelector('.view-line select') as HTMLSelectElement;
    expect(select.closest('label')?.querySelector('.muted.mono')?.textContent).toBe('project');
    expect([...select.options].map((o) => o.textContent)).toEqual(['all', 'Whitefoot', 'wf-agents']);
    expect(select.value).toBe('');
    expect(listed(page)).toHaveLength(10);
    const facts = () => page.querySelector('.view-facts')?.textContent;
    expect(facts()).toBe('10 works, 5 things wait for you');
    // The works waiting for the owner of the other project, 50, 44 and 41, stay; the counts stay those of every work.
    pick(select, 'Whitefoot');
    expect(listed(page)).toEqual([50, 43, 44, 45, 41, 47]);
    expect(facts()).toBe('10 works, 5 things wait for you, 6 shown');
    // An item added before items had projects is shown only with none chosen, as it waits for nobody.
    pick(select, 'wf-agents');
    expect(listed(page)).toEqual([42, 50, 43, 44, 45, 46, 41, 48]);
    pick(select, '');
    expect(listed(page)).toHaveLength(10);
    expect(facts()).toBe('10 works, 5 things wait for you');
  });

  it('keeps no project chosen once the list is drawn anew, and takes no room for a choice with one project', async () => {
    const names = fixtureProjects().names;
    const first = draw(<WorksTable items={fixtureItems()} projects={names} />, fixtureSource());
    pick(first.querySelector('select'), 'Whitefoot');
    expect(listed(first)).toHaveLength(6);
    expect(listed(draw(<WorksTable items={fixtureItems()} projects={names} />, fixtureSource()))).toHaveLength(10);
    const one = draw(<WorksList />, fixtureSource({ loadProjects: async () => ({ names: ['Whitefoot'] }) }));
    await settle();
    expect(one.querySelector('select')).toBeNull();
    expect(listed(one)).toHaveLength(10);
  });

  it('shows every work, with no choice of project, where the projects cannot be read', async () => {
    const page = draw(<WorksList />, fixtureSource({ loadProjects: () => Promise.reject(new Error('no such route')) }));
    await settle();
    expect(page.querySelector('select')).toBeNull();
    expect(listed(page)).toHaveLength(10);
  });

  it("says that a work the engine paused waits for you in the owner's tone, its stop's cell in red, and a fault in the tone of what failed", () => {
    const page = draw(<WorksTable items={fixtureItems()} />, fixtureSource());
    const row = page.querySelector('[data-work="50"]') as HTMLElement;
    expect(row.querySelector('.now-words')?.textContent).toBe('Waiting for you · implement');
    expect(row.querySelector('.now')?.className).toContain('now-owner');
    expect(row.querySelector('.now-paused')).toBeNull();
    // The cell the work is stopped at is filled in the stop's red, and no other.
    const marks = [...row.querySelectorAll<HTMLElement>('.strip .state-mark')];
    expect(marks.filter((m) => m.style.background === 'var(--red)').map((m) => m.title)).toEqual([
      'implement: failed, 0 runs finished',
    ]);
    const fault = page.querySelector('[data-work="49"]') as HTMLElement;
    expect(fault.querySelector('.now-words')?.textContent).toBe('Halted');
    expect(fault.querySelector('.now')?.className).toContain('now-failed');
  });

  it("says the owner's pause beside its state wherever it holds the work, and the engine's pause by the state alone", () => {
    const at = (now: Now, paused: Pauser | null, waits: Wait[] = []) => ({ now, cells: [], waits, paused });
    const pause: Wait = { key: 'pause', why: 'x', ends: { kind: 'resume' } };
    const question: Wait = { key: 'answer-scope', why: 'Which scope?', ends: { kind: 'answer', question: 'scope', version: 3 } };
    const merge: Now = { kind: 'waits', at: 'merge', on: [{ kind: 'precondition', test: 'asked', words: 'asked', by: [{ kind: 'owner' }] }] };
    expect(pauseDiffers(at({ kind: 'paused', at: 'plan', by: 'owner', why: null }, 'owner'))).toBe(false);
    const engine = at({ kind: 'paused', at: 'plan', by: 'engine', why: 'A round failed.' }, 'engine', [pause]);
    expect(pauseDiffers(engine)).toBe(false);
    // The owner's pause holds a work that waits for the owner as well: both are said.
    expect(pauseDiffers(at(merge, 'owner', [question]))).toBe(true);
    // The engine's pause beside what else waits for the owner is said by the state alone.
    expect(pauseDiffers(at(merge, 'engine', [pause, question]))).toBe(false);
    expect(pauseDiffers(at({ kind: 'waits', at: 'plan', on: [{ kind: 'slot' }] }, 'owner'))).toBe(true);
    expect(pauseDiffers(at({ kind: 'round', cell: 'plan', run: 1, round: 1, since: 0 }, 'owner'))).toBe(true);
    expect(pauseDiffers(at({ kind: 'merged' }, 'owner'))).toBe(false);
    expect(pauseDiffers(at({ kind: 'waits', at: 'plan', on: [{ kind: 'slot' }] }, null))).toBe(false);
    const page = draw(<WorksTable items={fixtureItems()} />, fixtureSource());
    expect(page.querySelector('[data-work="46"] .now-paused')).toBeNull();
  });

  it('keeps the list when a read fails, says so, and reads again when the stream opens again', async () => {
    let fail = false;
    const source = fixtureSource();
    const base = source.loadItems;
    source.loadItems = () => (fail ? Promise.reject(new Error('the server went away')) : base());
    const page = draw(<WorksList />, source);
    await settle();
    expect(page.querySelectorAll('tbody tr.work-row')).toHaveLength(10);
    fail = true;
    act(() => source.changed({ works: [41], items: true, copies: false, accounts: false, observations: false }));
    await settle();
    expect(page.querySelectorAll('tbody tr.work-row')).toHaveLength(10);
    expect(page.querySelector('.read-failed')?.textContent).toContain('the server went away');
    fail = false;
    const before = source.reads.items;
    act(() => source.opened());
    await settle();
    expect(source.reads.items).toBe(before + 1);
    expect(page.querySelector('.read-failed')).toBeNull();
  });
});

describe("a work's line", () => {
  it('leads back where the owner came from and names the item by its number and its title, with the legend of the history at its right, and nothing the window says', async () => {
    const { page } = await drawWork(41);
    const line = page.querySelector('.view-line') as HTMLElement;
    expect(line.querySelector('a.link')?.getAttribute('href')).toBe('#/works');
    expect(texts(line, '.view-title > *')).toEqual([
      '← Back',
      '#141',
      'Show the cost of each round on the works page',
    ]);
    expect(texts(line, '.legend li').slice(0, 2)).toEqual(['wakes', 'way back']);
    // Where the page was opened on the work itself, with nothing of it to go back to, the way back leads to the works.
    await click(line.querySelector('.view-title a.link') as HTMLElement);
    expect(window.location.hash).toBe('#/works');
    // What the item's window says, the state, the cost and the owner's buttons, is not said again here.
    expect(line.querySelector('.now-words')).toBeNull();
    expect(button(line, 'Pause')).toBeUndefined();
  });
});

describe('a work as it is read', () => {
  it('says it is reading, then shows the work, and reads it again when it changed', async () => {
    const source = fixtureSource();
    const page = draw(<WorkView work={41} />, source);
    expect(page.textContent).toContain('Reading work 41');
    await settle();
    expect(page.querySelector('.view-name')?.textContent).toBe('Show the cost of each round on the works page');
    act(() => source.changed({ works: [41], items: false, copies: false, accounts: false, observations: false }));
    await settle();
    expect(source.reads.work).toBe(2);
    // A change to another work does not read this one again.
    act(() => source.changed({ works: [42], items: false, copies: false, accounts: false, observations: false }));
    await settle();
    expect(source.reads.work).toBe(2);
  });

  it('keeps what is shown when a read fails, says so, and reads again when the stream opens again', async () => {
    let fail = false;
    const source = fixtureSource();
    const base = source.loadWork;
    source.loadWork = (n) => (fail ? Promise.reject(new Error('the server went away')) : base(n));
    const page = draw(<WorkView work={41} />, source);
    await settle();
    fail = true;
    act(() => source.changed({ works: [41], items: false, copies: false, accounts: false, observations: false }));
    await settle();
    expect(page.querySelector('.read-failed')?.textContent).toBe(
      'The work could not be read again: the server went away. What is shown is from before.',
    );
    expect(runRows(page)).toHaveLength(placeSteps(fixtureWork(41)).filter((r) => r.divider === null).length);
    fail = false;
    act(() => source.opened());
    await settle();
    expect(page.querySelector('.read-failed')).toBeNull();
  });

  it('reads one at a time, and once more after a read that changes came during, so the newest is shown last', async () => {
    const answers: Deferred<WorkPage>[] = [];
    const source = fixtureSource({
      loadWork: () => {
        const d = deferred<WorkPage>();
        answers.push(d);
        return d.promise;
      },
    });
    const page = draw(<WorkView work={41} />, source);
    answers[0].resolve(fixtureWork(41));
    await settle();
    act(() => source.changed({ works: [41], items: false, copies: false, accounts: false, observations: false }));
    act(() => source.changed({ works: [41], items: false, copies: false, accounts: false, observations: false }));
    act(() => source.changed({ works: [41], items: false, copies: false, accounts: false, observations: false }));
    expect(answers).toHaveLength(2);
    answers[1].resolve({ ...fixtureWork(41), title: 'Older' });
    await settle();
    expect(answers).toHaveLength(3);
    answers[2].resolve({ ...fixtureWork(41), title: 'Newer' });
    await settle();
    expect(answers).toHaveLength(3);
    expect(page.querySelector('.view-name')?.textContent).toBe('Newer');
  });

  it('reads a round under way each time, and an ended round once', async () => {
    const source = fixtureSource();
    const reader = readerOf(source, 41);
    await reader.round(1, 2, true);
    await reader.round(1, 2, true);
    expect(source.reads.round).toBe(1);
    await reader.round(1, 2, false);
    await reader.round(1, 2, false);
    expect(source.reads.round).toBe(3);
  });
});

describe('the history of a work', () => {
  it("has a column for each cell, headed by its id, and a row for each run, named by what woke it, its mark in its cell's column", async () => {
    const { page } = await drawWork(41);
    expect(texts(page, 'thead .cell-id')).toEqual(['plan', 'deep_research', 'implement', 'review', 'check', 'merge']);
    // A head chooses nothing on the history: it says the cell's title on pointing at it.
    expect(page.querySelector('thead [data-key="cell:plan"] .head-still')?.getAttribute('title')).toContain('plan Plan');
    const rows = runRows(page);
    // A run is a row, and a write is none: the owner's request and words and what the outside wrote woke the runs
    // whose rows name them (decision a-row-is-a-run).
    expect(rows).toHaveLength(12);
    expect(rows.map((r) => texts(r, '.step-lead .block-name').join(' '))).toEqual([
      'request',
      'plan',
      'solution',
      'opinions_plan',
      'plan',
      'plan',
      'code',
      'opinions_implement',
      'code',
      'opinions_implement',
      'code',
      'main',
    ]);
    // Who wrote what woke a run, where it is not a run: the owner's sign and the outside's.
    expect(rows[0].querySelector('.step-lead svg')?.getAttribute('aria-label')).toBe('the owner');
    expect(rows[11].querySelector('.step-lead svg')?.getAttribute('aria-label')).toBe('the outside');
    // The run abandoned stands in the implementation's column, marked as one that runs again.
    const abandoned = rows[2].querySelectorAll('td')[2];
    expect(abandoned.querySelector('.state-mark')?.getAttribute('title')).toBe('implement: runs again');
    expect(rows[2].querySelectorAll('.state-mark')).toHaveLength(1);
  });

  it('draws an arrow from the column of the cell whose run wrote what woke a run to its mark, in the accent where it runs back', async () => {
    const { page } = await drawWork(41);
    const rows = runRows(page);
    // The review's opinions sent the implementation round twice: those two arrows run back.
    expect(rows.map((r, i) => (r.querySelector('.row-arrow.back') ? i : -1)).filter((i) => i >= 0)).toEqual([7, 9]);
    // Every other run woken by another cell's run has an arrow a write runs; those the owner and the outside woke, and
    // the plan's first run, none.
    expect(rows.map((r, i) => (r.querySelector('.row-arrow.wakes') ? i : -1)).filter((i) => i >= 0)).toEqual([
      1, 2, 4, 5, 6, 8, 10,
    ]);
    // The arrow's start says the run that wrote, on pointing at it.
    expect(rows[7].querySelectorAll('td')[3].getAttribute('title')).toBe('review: outputs opinions_implement v1');
  });

  it('says at the right how long each run took, coloured as a card colours its age, and its level, effort and model', async () => {
    const work = structuredClone(fixtureWork(41));
    // The plan's first run made to take two hours, past the hour after which a run is red.
    const first = work.steps.find((s) => s.kind === 'run');
    if (first?.kind !== 'run' || first.end === null) throw new Error('not the fixture');
    first.end = { ...first.end, time: first.time + 2 * 3_600_000 };
    const { page } = await drawWork(work);
    const runs = work.steps.flatMap((s) => (s.kind === 'run' ? [s] : []));
    const rows = runRows(page);
    const facts = rows.map((r) => [...r.querySelectorAll('td.words .run-facts > span')].map((s) => s.textContent));
    expect(facts).toEqual(
      runs.map((run) => {
        const ran = ranLast(run);
        return [
          duration(runTook(run, 0)),
          ran === null ? '' : levelWords(ran.level, 0),
          ran?.effort ?? '',
          ran?.model ?? '',
        ];
      }),
    );
    // Grey under half an hour, amber to an hour, red past it: the implementation's first run took 31m 36s, and the
    // plan's two hours.
    const tones = rows.map((r) => r.querySelector('.step-time')?.className.split(' ').at(-1));
    expect(tones).toEqual(runs.map((run) => age(runTook(run, 0)).tone));
    expect(tones.slice(0, 3)).toEqual(['danger', 'success', 'attention']);
    // The check, an engine's cell, ran on no model.
    expect(facts[11].slice(1)).toEqual(['', '', '']);
    expect(rows[11].querySelector('td.words')?.getAttribute('title')).toMatch(/^Began \d{4}-/);
  });

  it("draws a line between the runs where the work was paused, saying what was done at the right", async () => {
    const { page } = await drawWork(50);
    expect(page.querySelectorAll('tbody tr.divider-row')).toHaveLength(1);
    const divider = page.querySelector('tbody tr.divider-row') as HTMLElement;
    expect(divider.querySelector('.divider-words')?.textContent).toBe('paused');
    expect(divider.title).toContain(`the engine paused: ${FAILED}`);
    // The run the work is stopped at is marked so.
    expect(runRows(page)[1].querySelector('.state-mark')?.getAttribute('title')).toBe('implement: stopped');
    expect((await drawWork(41)).page.querySelectorAll('tbody tr.divider-row')).toHaveLength(0);
  });

  it('draws a work by the version of its flow it ran on, as its outline has it, and needs no flow for one that ended', async () => {
    const work = structuredClone(fixtureWork(41));
    // The version it ran on titled the plan otherwise than the flow the engine reads now.
    work.outline.cells = work.outline.cells.map((c) => (c.id === 'plan' ? { ...c, title: 'Plan as it ran' } : c));
    work.flow = null;
    const { page } = await drawWork(work);
    expect(page.querySelector('thead [data-key="cell:plan"] .head-still')?.getAttribute('title')).toContain(
      'plan Plan as it ran',
    );
    expect(runRows(page)).toHaveLength(12);
    expect(page.textContent).not.toContain(NOT_KEPT);
  });

  it('draws a work on no version kept with a column for each cell its runs name, in the order they first ran, and says why', async () => {
    const work = structuredClone(fixtureWork(41));
    const ran = [...new Set(work.steps.flatMap((s) => (s.kind === 'run' ? [s.cell] : [])))];
    work.outline = {
      kept: false,
      cells: ran.map((id) => ({ id, title: null, line: null, agent: false, given: [], watches: [], reads: [], words: null })),
      lines: [],
      groups: [],
      card: ['request'],
    };
    work.flow = null;
    const { page } = await drawWork(work);
    expect(texts(page, 'thead .cell-id')).toEqual(ran);
    expect(page.textContent).toContain(NOT_KEPT);
    // No title stands for one the version did not keep.
    expect(page.querySelector('thead [data-key="cell:plan"] .head-still')?.getAttribute('title')).toContain(
      'plan of a version not kept',
    );
    expect(runRows(page)).toHaveLength(12);
  });
});

describe("the Step of the work's item window", () => {
  /// The words of the Step that say when a run of a made-up work began, which tell that run from the cell's others.
  const began = (work: number, run: number) => {
    const s = fixtureWork(work).steps.find((x) => x.kind === 'run' && x.run === run);
    return `began ${clock(s?.time ?? 0)}`;
  };

  it('shows the run chosen: how it stands, why it ran, what it started from, what it handed in, its rounds and its calls', async () => {
    const { page } = await drawWork(41);
    await chooseRun(page, 2);
    const panel = step(page);
    expect(page.querySelector('aside [role="tab"][aria-selected="true"]')?.textContent).toBe('Step');
    expect(panel.querySelector('.status-line')?.textContent).toContain('Runs again');
    // A run is told by its cell and its time, never by the number the engine keeps it by.
    expect(panel.querySelector('.status-line')?.textContent).not.toMatch(/run \d/);
    expect(panel.textContent).toContain(began(41, 3));
    expect(part(panel, 'Why it ran').textContent).toContain('it had not run before');
    expect(texts(panel, '.cw-fold-head')).toEqual(expect.arrayContaining(['Why it ran 1', 'Handed in']));
    expect(texts(panel, '.cw-fold-head').some((l) => l.startsWith('Inputs'))).toBe(true);
    // No part of its own for its calls, nor a round by its number: each call stands under the time that asked for it.
    expect(texts(panel, '.cw-fold-head').some((l) => l.startsWith('Calls') || /Round \d/.test(l))).toBe(false);
    expect(panel.textContent).toContain('Should the cached tokens be shown apart from the others?');
    // The question it rested at was withdrawn with it.
    expect(panel.textContent).toContain('withdrawn with its run');
    expect(part(panel, 'Handed in').textContent).toContain('it wrote nothing');
    // A result is read only when it is opened.
    const calls = part(panel, 'Details');
    expect(calls.querySelector('pre.content')).toBeNull();
    await click(button(calls, 'Show result'));
    expect(calls.querySelector('pre.content')?.textContent).toBe('Found 0 errors.');
  });

  it('shows the run the address names, chosen as the work is first shown, and with nothing chosen the run the work stands at', async () => {
    const named = draw(<WorkScreen page={fixtureWork(41)} onActed={() => undefined} run={3} />, fixtureSource());
    await settle();
    expect(named.querySelector('aside [role="tab"][aria-selected="true"]')?.textContent).toBe('Step');
    expect(step(named).textContent).toContain(began(41, 3));
    expect(runRows(named)[2].classList.contains('chosen')).toBe(true);
    // Work 42 stands at its implementation's second run, which its Step shows with no row chosen.
    const { page } = await drawWork(42);
    await click(button(page.querySelector('aside') as HTMLElement, 'Step'));
    expect(step(page).textContent).toContain(began(42, 2));
  });

  it('shows why a return was sent back, and a way round', async () => {
    const { page } = await drawWork(41);
    await chooseRun(page, 1);
    // The reason is its key in `sent_back.toml` and the value recorded under its name, with no sentence of the page's own.
    const reason = step(page).querySelector('.reasons li');
    expect(Array.from(reason?.querySelectorAll('.mono') ?? []).map((e) => e.textContent)).toEqual([
      'not_an_output',
      'block',
      'code',
    ]);
    expect(step(page).textContent).not.toContain('which the cell may not write');
    await chooseRun(page, 7);
    expect(step(page).querySelector('.way-back')?.textContent).toContain('by a cell after it: a way round');
  });

  it('says what a run said of each change, and what the engine made of each block it wrote', async () => {
    const { page } = await drawWork(41);
    // The deep research's second run said its change did not matter, and the solution kept its meaning.
    await chooseRun(page, 4);
    expect(step(page).textContent).toContain('it said this changes nothing they read');
    expect(step(page).textContent).toMatch(/kept the meaning of v\d+/);
    // The plan's second run wrote needs_deep_research as it was: a choice keeps its meaning while its value does.
    await chooseRun(page, 3);
    const written = [...part(step(page), 'Handed in').querySelectorAll('.list-row')];
    expect(written.map((r) => [r.querySelector('.mono')?.textContent, r.textContent?.includes('changed its meaning')])).toEqual([
      ['plan', true],
      ['needs_deep_research', false],
    ]);
  });

  // A version whose meaning is none, a block's first version handed in as changing nothing, has said nothing yet.
  it('says of a version a run wrote that has said nothing yet just that', async () => {
    const work = structuredClone(fixtureWork(41));
    const run = work.steps.find((s) => s.kind === 'run' && s.run === 4);
    if (run?.kind !== 'run' || run.end?.kind !== 'finished') throw new Error('not the fixture');
    const at = run.end.at;
    const version = work.blocks.find((b) => b.name === 'needs_deep_research')?.versions.find((v) => v.at === at);
    if (!version) throw new Error('not the fixture');
    version.meaning = null;
    const { page } = await drawWork(work);
    await chooseRun(page, 3);
    const written = [...part(step(page), 'Handed in').querySelectorAll('.list-row')].find(
      (r) => r.querySelector('.mono')?.textContent === 'needs_deep_research',
    );
    expect(written?.textContent).toContain('said nothing yet');
  });

  // A version is shown by `v` and its number among its block's versions, never by its place among the work's steps; and
  // the inputs are the blocks the cell is given alone: the versions the run began with of what it may only write are
  // none of them, and what it wrote of those is under Handed in.
  it('names each input a run began with by v and its number among its block’s versions, those new to it first', async () => {
    const { page } = await drawWork(41);
    // The implementation's run that the review's first finding sent round again: it began from that finding, new to it,
    // from the plan and the solution as they stood, and from the code and the answer it may write, which it is given by
    // nothing.
    await chooseRun(page, 7);
    const started = part(step(page), 'Inputs');
    const rows = [...started.querySelectorAll('.list-row')];
    expect(rows.map((r) => r.querySelector('.mono')?.textContent)).toEqual([
      'opinions_implement v1',
      'plan v2',
      'solution v2',
    ]);
    expect(rows.map((r) => r.textContent?.includes('new to it'))).toEqual([true, false, false]);
    expect(started.textContent).not.toContain('@');
  });
});

describe('what the owner does', () => {
  it("does it in the work's item window, and reads the work again once it is taken", async () => {
    const { page, source, acted } = await drawWork(42);
    const window = page.querySelector('aside') as HTMLElement;
    expect(window.querySelector('.cw-acts .switch')?.getAttribute('aria-label')).toBe('Your own pause of the work');
    await click(button(window, 'Pause'));
    expect(source.acts).toEqual([{ work: 42, what: { kind: 'pause' } }]);
    expect(acted.times).toBe(1);
  });
});
