// @vitest-environment happy-dom
import '../dom-setup';

import { type ReactNode, act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import type { Act, Changed, WorkPage } from '../works-api';
import {
  fixtureCall,
  fixtureCopies,
  fixtureItems,
  fixtureRound,
  fixtureTold,
  fixtureVersion,
  fixtureWork,
} from './fixtures';
import { pauseDiffers } from './marks';
import { type Source, SourceContext } from './source';
import { WorkScreen, WorkView, readerOf } from './WorkView';
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
    loadWork: async (n) => {
      s.reads.work += 1;
      return fixtureWork(n);
    },
    loadVersion: async (n, block, at) => fixtureVersion(n, block, at),
    loadRound: async (n, run, round) => {
      s.reads.round += 1;
      return fixtureRound(n, run, round);
    },
    loadTold: async (n, run, round) => fixtureTold(n, run, round),
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

function drawWork(work: number, source = fixtureSource()) {
  const page = draw(<WorkScreen page={fixtureWork(work)} onActed={() => undefined} />, source);
  return { page, source };
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

/// Chooses the step at this place by its row's words at the left.
async function chooseStep(page: HTMLElement, index: number) {
  await click(page.querySelectorAll('tbody tr.step-row')[index]?.querySelector('.step-lead button'));
}

async function chooseCell(page: HTMLElement, cell: string) {
  await click(page.querySelector(`thead [data-key="cell:${cell}"] .head-button`));
}


describe('the list of works', () => {
  it('lists the work of every item, those waiting for the owner first and edged, and a row opens its work', async () => {
    const page = draw(<WorksTable items={fixtureItems()} />, fixtureSource());
    const rows = [...page.querySelectorAll('tbody tr.work-row')] as HTMLElement[];
    expect(rows.map((r) => Number(r.dataset.work))).toEqual([43, 44, 45, 41, 50, 49, 42, 46, 47, 48]);
    expect(rows.map((r) => r.classList.contains('for-owner'))).toEqual([true, true, true, true, false, false, false, false, false, false]);
    expect(rows[0].querySelector('.now-words')?.textContent).toBe("Waits at plan for the owner's answer to a question");
    expect(rows[0].querySelectorAll('.strip .state-mark')).toHaveLength(6);
    await click(rows[3]);
    expect(window.location.hash).toBe('#/works/41');
  });

  it('says why the engine paused a work, in the tone of what failed', () => {
    const page = draw(<WorksTable items={fixtureItems()} />, fixtureSource());
    const row = page.querySelector('[data-work="50"]') as HTMLElement;
    expect(row.querySelector('.now-words')?.textContent).toBe(`Paused by the engine: ${FAILED}`);
    expect(row.querySelector('.now')?.className).toContain('now-failed');
    expect(row.querySelector('.now-paused')).toBeNull();
    expect(row.querySelectorAll('.strip .state-failed')).toHaveLength(1);
  });

  it('says a work is paused only where what it does now says something else', () => {
    expect(pauseDiffers({ kind: 'paused', by: 'owner', why: null }, true)).toBe(false);
    expect(pauseDiffers({ kind: 'paused', by: 'engine', why: 'A round failed.' }, true)).toBe(false);
    expect(pauseDiffers({ kind: 'waits', at: 'plan', on: [] }, true)).toBe(true);
    expect(pauseDiffers({ kind: 'round', cell: 'plan', run: 1, round: 1, since: 0 }, true)).toBe(true);
    expect(pauseDiffers({ kind: 'merged' }, true)).toBe(false);
    expect(pauseDiffers({ kind: 'waits', at: 'plan', on: [] }, false)).toBe(false);
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
    act(() => source.changed({ works: [41], items: true, copies: false }));
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

describe('a work as it is read', () => {
  it('says it is reading, then shows the work, and reads it again when it changed', async () => {
    const source = fixtureSource();
    const page = draw(<WorkView work={41} />, source);
    expect(page.textContent).toContain('Reading work 41');
    await settle();
    expect(page.querySelector('.view-name')?.textContent).toBe('Show the cost of each round on the works page');
    act(() => source.changed({ works: [41], items: false, copies: false }));
    await settle();
    expect(source.reads.work).toBe(2);
    // A change to another work does not read this one again.
    act(() => source.changed({ works: [42], items: false, copies: false }));
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
    act(() => source.changed({ works: [41], items: false, copies: false }));
    await settle();
    expect(page.querySelector('.read-failed')?.textContent).toBe(
      'The work could not be read again: the server went away. What is shown is from before.',
    );
    expect(page.querySelectorAll('tbody tr.step-row')).toHaveLength(fixtureWork(41).steps.length);
    fail = false;
    act(() => source.opened());
    await settle();
    expect(page.querySelector('.read-failed')).toBeNull();
  });

  it('drops an answer older than the one shown', async () => {
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
    act(() => source.changed({ works: [41], items: false, copies: false }));
    act(() => source.changed({ works: [41], items: false, copies: false }));
    answers[2].resolve({ ...fixtureWork(41), title: 'Newer' });
    await settle();
    answers[1].resolve({ ...fixtureWork(41), title: 'Older' });
    await settle();
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
  it('has a column for each cell with how it stands now, and a row for each step', () => {
    const { page } = drawWork(41);
    expect(texts(page, 'thead .cell-title')).toEqual(['Plan', 'Deep research', 'Implement', 'Review the code', 'Check with the latest main', 'Merge']);
    expect(texts(page, 'thead .cell-state').at(-1)).toBe('not run yet');
    const rows = [...page.querySelectorAll('tbody tr.step-row')];
    expect(rows).toHaveLength(fixtureWork(41).steps.length);
    // The run abandoned is in the implementation's column, and says so.
    expect(rows[3].querySelectorAll('td')[2].querySelector('.run')?.className).toContain('run-abandoned');
    expect(texts(page, '.step-lead .step-words')[3]).toBe('3 implement had no record');
    // The ways round are drawn in the way back's purple, from the review to the implementation.
    expect(page.querySelectorAll('.run-arrow')).toHaveLength(2);
    expect(texts(page, '.step-words.way-back')).toEqual(['8 way round for opinions_implement', '10 way round for opinions_implement']);
    expect(texts(page, '.step-lead .step-words')[13]).toBe('the outside wrote main');
  });
});

describe('a work whose round failed', () => {
  it("draws the run as failed in its cell's column, its cell's head as failed, and the engine's pause with the reason", () => {
    const { page } = drawWork(50);
    expect(texts(page, 'thead .cell-state')[2]).toBe('failed');
    const rows = [...page.querySelectorAll('tbody tr.step-row')];
    const run = rows[2].querySelectorAll('td')[2].querySelector('.run');
    expect(run?.className).toContain('run-failed');
    expect(run?.querySelector('.run-word')?.textContent).toBe('round failed');
    expect(texts(page, '.step-lead .step-words')[3]).toBe(`the engine paused the work: ${FAILED}`);
  });

  it('shows in the panel of the run the round that failed, why, and that it is given again once the work is let on', async () => {
    const { page } = drawWork(50);
    await chooseStep(page, 2);
    const panel = page.querySelector('aside[aria-label="Run 2 of implement"]') as HTMLElement;
    expect(panel.querySelector('.status-line')?.textContent).toContain('Round failed');
    expect(panel.querySelector('.round .failed')?.textContent).toBe(`nothing; the round failed: ${FAILED}`);
    expect(panel.textContent).toContain('the results of 1 call');
    expect(panel.textContent).toContain('its last round failed, and the round is given again once the work is let on');
    await chooseStep(page, 3);
    const pause = page.querySelector('aside[aria-label="Paused"]') as HTMLElement;
    expect(pause.textContent).toContain(
      `The engine paused the work: only a note to a cell is answered until the owner lets it on. Why: ${FAILED}`,
    );
  });
});

describe('the panel of a step', () => {
  it('shows a run: why it ran, what it started from, each round with what it was given and gave back, its calls and its end', async () => {
    const { page } = drawWork(41);
    await chooseStep(page, 3);
    const panel = page.querySelector('aside[aria-label="Run 3 of implement"]') as HTMLElement;
    expect(panel.querySelector('.status-line')?.textContent).toContain('Abandoned');
    expect(panel.textContent).toContain('the cell had no record');
    expect(texts(panel, '.panel-part-label')).toEqual(
      expect.arrayContaining(['Why it ran 1', 'Started from 5', 'Calls 2', 'How it ended']),
    );
    expect(panel.textContent).toContain('the results of 1 call');
    expect(panel.textContent).toContain('Should the cached tokens be shown apart from the others?');
    // The question it rested at was withdrawn with it.
    expect(panel.textContent).toContain('withdrawn with its run');
    expect(panel.textContent).toContain('it wrote nothing');
    // A result is read only when it is opened.
    await click(button(panel, 'Show result'));
    expect(panel.querySelector('pre.content')?.textContent).toBe('Found 0 errors.');
  });

  it('shows why a return was sent back, and a way round', async () => {
    const { page } = drawWork(41);
    await chooseStep(page, 2);
    const panel = page.querySelector('aside[aria-label="Run 2 of deep_research"]') as HTMLElement;
    expect(panel.textContent).toContain('it handed in code, which the cell may not write');
    await chooseStep(page, 9);
    const way = page.querySelector('aside[aria-label="Run 8 of implement"]') as HTMLElement;
    expect(way.textContent).toContain('Way round');
    expect(way.querySelector('.way-back')?.textContent).toContain('by a cell after it: a way round');
  });

  it('says what a run said of each change, and what the engine made of each block it wrote', async () => {
    const { page } = drawWork(41);
    // The deep research's second run said its change did not matter, and the solution kept its meaning.
    await chooseStep(page, 6);
    const deep = page.querySelector('aside[aria-label="Run 5 of deep_research"]') as HTMLElement;
    expect(deep.textContent).toContain('said not to matter');
    expect(deep.textContent).toMatch(/kept the meaning of @\d+/);
    // The plan's second run wrote needs_deep_research as it was: a choice keeps its meaning while its value does.
    await chooseStep(page, 5);
    const plan = page.querySelector('aside[aria-label="Run 4 of plan"]') as HTMLElement;
    const ended = [...plan.querySelectorAll('.panel-part')].find((p) => p.querySelector('h3')?.textContent === 'How it ended');
    const written = [...(ended?.querySelectorAll('.list-row') ?? [])];
    expect(written.map((r) => [r.querySelector('.mono')?.textContent, r.textContent?.includes('changed its meaning')])).toEqual([
      ['plan', true],
      ['needs_deep_research', false],
    ]);
  });

  it('shows a write: the block, who, whether it changed the meaning, who reads it, and its content', async () => {
    const { page } = drawWork(41);
    await chooseStep(page, 4);
    const panel = page.querySelector('aside[aria-label="Write of opinions_plan"]') as HTMLElement;
    expect(panel.querySelector('.drawer-belongs')?.textContent).toBe('written by a person');
    expect(panel.textContent).toContain('Changed its meaning');
    expect(panel.textContent).toContain('Runs plan; given to plan');
    await click(button(panel, 'Show content'));
    expect(panel.querySelector('pre.content')?.textContent).toBe(
      'The plan must say where the totals of a whole item are shown too.',
    );
  });

  it('shows a cell: why it would run, or the true reason it would not', async () => {
    const at = drawWork(45);
    await chooseCell(at.page, 'implement');
    const implement = at.page.querySelector('aside[aria-label="Cell implement"]') as HTMLElement;
    expect(implement.querySelector('.status-word')?.textContent).toContain('Runs again');
    expect(implement.querySelector('.way-back')?.textContent).toContain('by a cell after it: a way round');
    await chooseCell(at.page, 'deep_research');
    expect(at.page.querySelector('aside[aria-label="Cell deep_research"]')?.textContent).toContain(
      'It does not run: its condition is not met, so it is skipped',
    );
    await chooseCell(at.page, 'plan');
    expect(at.page.querySelector('aside[aria-label="Cell plan"]')?.textContent).toContain(
      'It would not: its record knows the current meaning of every block it reads',
    );
    const open = drawWork(42);
    await chooseCell(open.page, 'implement');
    expect(open.page.querySelector('aside[aria-label="Cell implement"]')?.textContent).toContain(
      'It starts no other run: run 2 of it is open',
    );
  });

  it('shows, with nothing chosen, every block with its versions, each opening its content', async () => {
    const { page } = drawWork(43);
    const panel = page.querySelector('aside[aria-label="Blocks"]') as HTMLElement;
    expect(panel.querySelector('.status-word')?.textContent).toBe('1 block');
    expect(texts(panel, '.panel-part-label')).toEqual(['request 1']);
    expect(panel.querySelector('.list-row')?.textContent).toContain('a person');
    await click(button(panel, 'Show content'));
    expect(panel.querySelector('pre.content')?.textContent).toContain('List the works of every item');
  });
});

describe('what the owner does', () => {
  it("leaves what the work waits for, and writing to it, to the console's item window", () => {
    for (const n of [41, 43, 44, 45]) {
      const { page } = drawWork(n);
      expect(page.querySelector('.waits')).toBeNull();
      expect(page.querySelector('form')).toBeNull();
      expect(page.querySelector('textarea')).toBeNull();
    }
  });

  it('sends an act once, its control disabled until the answer comes', async () => {
    const answer = deferred<null>();
    const source = fixtureSource();
    source.act = (work, what) => {
      source.acts.push({ work, what });
      return answer.promise;
    };
    const { page } = drawWork(42, source);
    const pause = button(page, 'Pause');
    await click(pause);
    await click(pause);
    expect(source.acts).toHaveLength(1);
    expect(button(page, 'Pause')?.disabled).toBe(true);
    answer.resolve(null);
    await settle();
    expect(button(page, 'Pause')?.disabled).toBe(false);
  });

  it('shows why the interface refused an act', async () => {
    const source = fixtureSource({ act: () => Promise.reject(new Error('the work has ended')) });
    const { page } = drawWork(42, source);
    await click(button(page, 'Pause'));
    expect(page.querySelector('.acts [role="alert"]')?.textContent).toBe('Refused: the work has ended');
  });

  it('pauses, and cancels only once asked again', async () => {
    const { page, source } = drawWork(42);
    await click(button(page, 'Pause'));
    await click(button(page, 'Cancel'));
    expect(source.acts.map((a) => a.what.kind)).toEqual(['pause']);
    expect(page.textContent).toContain('Cancel the work? It cannot be taken back');
    await click(button(page, 'Yes, cancel it'));
    expect(source.acts.map((a) => a.what.kind)).toEqual(['pause', 'cancel']);
  });
});
