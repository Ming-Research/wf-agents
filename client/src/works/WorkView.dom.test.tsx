// @vitest-environment happy-dom
import '../dom-setup';

import { type ReactNode, act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import type { Act, Changed, WorkPage } from '../works-api';
import { fixtureCall, fixtureItems, fixtureRound, fixtureVersion, fixtureWork } from './fixtures';
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
  const s: TestSource = {
    acts: [],
    reads: { work: 0, round: 0, items: 0 },
    changed: () => undefined,
    opened: () => undefined,
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
    loadCall: async (n, call) => fixtureCall(n, call),
    act: async (work, what) => {
      s.acts.push({ work, what });
      return null;
    },
    changes: (onChanged, onOpened) => {
      s.changed = onChanged;
      s.opened = onOpened;
      return () => undefined;
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

/// Sets what a box holds, as typing or choosing would.
function type(element: Element | null | undefined, value: string) {
  if (!element) throw new Error('nothing to type in');
  const field = element as HTMLTextAreaElement | HTMLSelectElement;
  const proto = field instanceof HTMLSelectElement ? HTMLSelectElement.prototype : HTMLTextAreaElement.prototype;
  act(() => {
    Object.getOwnPropertyDescriptor(proto, 'value')?.set?.call(field, value);
    field.dispatchEvent(new Event(field instanceof HTMLSelectElement ? 'change' : 'input', { bubbles: true }));
  });
}

async function submit(form: Element | null | undefined) {
  if (!form) throw new Error('no form');
  await act(async () => {
    form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

/// Chooses the step at this place by its row's words at the left.
async function chooseStep(page: HTMLElement, index: number) {
  await click(page.querySelectorAll('tbody tr.step-row')[index]?.querySelector('.step-lead button'));
}

async function chooseCell(page: HTMLElement, cell: string) {
  await click(page.querySelector(`thead [data-key="cell:${cell}"] .head-button`));
}

function escape() {
  act(() => {
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
  });
}

const writeBox = (page: HTMLElement) => page.querySelector('form.write-box') as HTMLFormElement;
const blockOf = (page: HTMLElement) => (writeBox(page).querySelector('#write-block') as HTMLSelectElement).value;
const typed = (page: HTMLElement) => (writeBox(page).querySelector('textarea') as HTMLTextAreaElement).value;

describe('the list of works', () => {
  it('lists the work of every item, those waiting for the owner first and edged, and a row opens its work', async () => {
    const page = draw(<WorksTable items={fixtureItems()} />, fixtureSource());
    const rows = [...page.querySelectorAll('tbody tr.work-row')] as HTMLElement[];
    expect(rows.map((r) => Number(r.dataset.work))).toEqual([43, 44, 45, 41, 49, 42, 46, 47, 48]);
    expect(rows.map((r) => r.classList.contains('for-owner'))).toEqual([true, true, true, true, false, false, false, false, false]);
    expect(rows[0].querySelector('.now-words')?.textContent).toBe("Waits at plan for the owner's answer to a question");
    expect(rows[0].querySelectorAll('.strip .state-mark')).toHaveLength(6);
    await click(rows[3]);
    expect(window.location.hash).toBe('#/works/41');
  });

  it('says a work is paused only where what it does now says something else', () => {
    expect(pauseDiffers({ kind: 'paused' }, true)).toBe(false);
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
    expect(page.querySelectorAll('tbody tr.work-row')).toHaveLength(9);
    fail = true;
    act(() => source.changed({ works: [41], items: true }));
    await settle();
    expect(page.querySelectorAll('tbody tr.work-row')).toHaveLength(9);
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
  it('says it is reading, then shows the work, and keeps what is typed when it is read again', async () => {
    const source = fixtureSource();
    const page = draw(<WorkView work={41} />, source);
    expect(page.textContent).toContain('Reading work 41');
    await settle();
    expect(page.querySelector('.view-name')?.textContent).toBe('Show the cost of each round on the works page');
    type(writeBox(page).querySelector('textarea'), 'Not yet');
    act(() => source.changed({ works: [41], items: false }));
    await settle();
    expect(source.reads.work).toBe(2);
    expect(typed(page)).toBe('Not yet');
    // A change to another work does not read this one again.
    act(() => source.changed({ works: [42], items: false }));
    await settle();
    expect(source.reads.work).toBe(2);
  });

  it('keeps what is shown and typed when a read fails, says so, and reads again when the stream opens again', async () => {
    let fail = false;
    const source = fixtureSource();
    const base = source.loadWork;
    source.loadWork = (n) => (fail ? Promise.reject(new Error('the server went away')) : base(n));
    const page = draw(<WorkView work={41} />, source);
    await settle();
    type(writeBox(page).querySelector('textarea'), 'Not yet');
    fail = true;
    act(() => source.changed({ works: [41], items: false }));
    await settle();
    expect(page.querySelector('.read-failed')?.textContent).toBe(
      'The work could not be read again: the server went away. What is shown is from before.',
    );
    expect(page.querySelectorAll('tbody tr.step-row')).toHaveLength(fixtureWork(41).steps.length);
    expect(typed(page)).toBe('Not yet');
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
    act(() => source.changed({ works: [41], items: false }));
    act(() => source.changed({ works: [41], items: false }));
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
    expect(texts(page, 'thead .cell-title')).toEqual(['Plan', 'Design', 'Implement', 'Review the code', 'Check with the latest main', 'Merge']);
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
    const panel = page.querySelector('aside[aria-label="Run 2 of design"]') as HTMLElement;
    expect(panel.textContent).toContain('it handed in code, which the cell may not write');
    await chooseStep(page, 9);
    const way = page.querySelector('aside[aria-label="Run 8 of implement"]') as HTMLElement;
    expect(way.textContent).toContain('Way round');
    expect(way.querySelector('.way-back')?.textContent).toContain('by a cell after it: a way round');
  });

  it('says what a run said of each change, and what the engine made of each block it wrote', async () => {
    const { page } = drawWork(41);
    // The design's second run said its change did not matter, and the design kept its meaning.
    await chooseStep(page, 6);
    const design = page.querySelector('aside[aria-label="Run 5 of design"]') as HTMLElement;
    expect(design.textContent).toContain('said not to matter');
    expect(design.textContent).toMatch(/kept the meaning of @\d+/);
    // The plan's second run wrote needs_design as it was: a choice keeps its meaning while its value does.
    await chooseStep(page, 5);
    const plan = page.querySelector('aside[aria-label="Run 4 of plan"]') as HTMLElement;
    const ended = [...plan.querySelectorAll('.panel-part')].find((p) => p.querySelector('h3')?.textContent === 'How it ended');
    const written = [...(ended?.querySelectorAll('.list-row') ?? [])];
    expect(written.map((r) => [r.querySelector('.mono')?.textContent, r.textContent?.includes('changed its meaning')])).toEqual([
      ['plan', true],
      ['needs_design', false],
    ]);
  });

  it('shows a write: the block, who, whether it changed the meaning, who reads it, and its content', async () => {
    const { page } = drawWork(41);
    await chooseStep(page, 4);
    const panel = page.querySelector('aside[aria-label="Write of opinions_plan"]') as HTMLElement;
    expect(panel.querySelector('.drawer-belongs')?.textContent).toBe('written by a person');
    expect(panel.textContent).toContain('Changed its meaning');
    expect(panel.textContent).toContain('Read by plan');
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
    await chooseCell(at.page, 'design');
    expect(at.page.querySelector('aside[aria-label="Cell design"]')?.textContent).toContain(
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
  it('answers a question the work waits for', async () => {
    const { page, source } = drawWork(43);
    const box = page.querySelector('form.answer') as HTMLElement;
    expect(box.textContent).toContain('Should the list show the works of cancelled items');
    type(box.querySelector('textarea'), 'Leave them out');
    await submit(box);
    expect(source.acts).toEqual([{ work: 43, what: { kind: 'answer', call: 1, result: 'Leave them out' } }]);
    expect((box.querySelector('textarea') as HTMLTextAreaElement).value).toBe('');
  });

  it('lets a run go on at its limit, and a cell at its limit of ways round', async () => {
    const run = drawWork(44);
    await click(button(run.page, 'Let the run go on'));
    expect(run.source.acts).toEqual([{ work: 44, what: { kind: 'answer', call: 1, result: 'go on' } }]);
    const cell = drawWork(45);
    await click(button(cell.page, 'Let implement go on'));
    expect(cell.source.acts).toEqual([{ work: 45, what: { kind: 'letGo', cell: 'implement' } }]);
  });

  it('sends an act once, its control disabled until the answer comes', async () => {
    const answer = deferred<null>();
    const source = fixtureSource();
    source.act = (work, what) => {
      source.acts.push({ work, what });
      return answer.promise;
    };
    const { page } = drawWork(45, source);
    const go = button(page, 'Let implement go on');
    await click(go);
    await click(go);
    expect(source.acts).toHaveLength(1);
    expect(button(page, 'Let implement go on')?.disabled).toBe(true);
    answer.resolve(null);
    await settle();
    expect(button(page, 'Let implement go on')?.disabled).toBe(false);
  });

  it('shows why the interface refused an act', async () => {
    const source = fixtureSource({ act: () => Promise.reject(new Error('the run is no longer at its limit')) });
    const { page } = drawWork(45, source);
    await click(button(page, 'Let implement go on'));
    expect(page.querySelector('.wait [role="alert"]')?.textContent).toBe('Refused: the run is no longer at its limit');
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

  it('keeps what is typed for a question with that question as the list of what the work waits for changes', () => {
    const source = fixtureSource();
    const work = fixtureWork(43);
    const container = document.createElement('div');
    document.body.append(container);
    const root = createRoot(container);
    const render = (page: WorkPage) =>
      act(() =>
        root.render(
          <SourceContext.Provider value={source}>
            <WorkScreen page={page} onActed={() => undefined} />
          </SourceContext.Provider>,
        ),
      );
    unmounts.push(() => act(() => root.unmount()));
    render(work);
    type(container.querySelector('form.answer textarea'), 'Leave them out');
    const now = work.summary.now;
    if (now.kind !== 'waits') throw new Error('the fixture waits');
    const other = { kind: 'precondition', test: 'reviewed', words: 'The plan was reviewed', who: 'owner' } as const;
    render({ ...work, summary: { ...work.summary, now: { ...now, on: [other, ...now.on] } } });
    expect((container.querySelector('form.answer textarea') as HTMLTextAreaElement).value).toBe('Leave them out');
  });
});

describe('the box to write a block', () => {
  it('starts at the block of the cell the work is at, a choice unset, and writes nothing until a value is picked', async () => {
    const { page, source } = drawWork(41);
    // The work is at the merge, which reads the owner's approval, a choice.
    expect(blockOf(page)).toBe('approval');
    expect((writeBox(page).querySelector('[aria-label="Value"]') as HTMLSelectElement).value).toBe('');
    expect(button(writeBox(page), 'Write')?.disabled).toBe(true);
    expect(writeBox(page).querySelector('.write-readers')?.textContent).toBe('Read by merge: each runs again if the value changes');
    await submit(writeBox(page));
    expect(source.acts).toEqual([]);
    type(writeBox(page).querySelector('[aria-label="Value"]'), 'yes');
    expect(button(writeBox(page), 'Write')?.disabled).toBe(false);
    await submit(writeBox(page));
    expect(source.acts).toEqual([{ work: 41, what: { kind: 'write', block: 'approval', content: '', value: 'yes' } }]);
  });

  it('follows the step chosen until the owner types, then holds its block whatever is chosen', async () => {
    const { page, source } = drawWork(41);
    // Choosing a run of the implementation, the box follows it to its opinions.
    await chooseStep(page, 7);
    expect(blockOf(page)).toBe('opinions_implement');
    expect(writeBox(page).querySelector('.write-readers')?.textContent).toBe('Read by implement: each runs again for it');
    type(writeBox(page).querySelector('textarea'), 'Keep the totals on one line.');
    // Another step, and Escape, leave it where it is.
    await chooseStep(page, 1);
    expect(blockOf(page)).toBe('opinions_implement');
    escape();
    expect(blockOf(page)).toBe('opinions_implement');
    expect(typed(page)).toBe('Keep the totals on one line.');
    await submit(writeBox(page));
    expect(source.acts.at(-1)).toEqual({
      work: 41,
      what: { kind: 'write', block: 'opinions_implement', content: 'Keep the totals on one line.', value: null },
    });
    // Written, the box is empty and follows again.
    expect(typed(page)).toBe('');
    expect(blockOf(page)).toBe('approval');
  });

  it('preselects no block where the cell reads none people write, and the owner picks', async () => {
    const { page, source } = drawWork(41);
    await chooseCell(page, 'review');
    expect(blockOf(page)).toBe('');
    expect(writeBox(page).querySelector('.write-readers')?.textContent).toBe('Pick the block to write');
    type(writeBox(page).querySelector('textarea'), 'Look at the narrow window too.');
    expect(button(writeBox(page), 'Write')?.disabled).toBe(true);
    type(writeBox(page).querySelector('#write-block'), 'opinions_design');
    await submit(writeBox(page));
    expect(source.acts).toEqual([
      { work: 41, what: { kind: 'write', block: 'opinions_design', content: 'Look at the narrow window too.', value: null } },
    ]);
  });

  it('sends a write once, and shows why it was refused, keeping what was typed', async () => {
    const answer = deferred<null>();
    const source = fixtureSource();
    source.act = (work, what) => {
      source.acts.push({ work, what });
      return answer.promise;
    };
    const { page } = drawWork(41, source);
    await chooseStep(page, 7);
    type(writeBox(page).querySelector('textarea'), 'Once');
    await submit(writeBox(page));
    await submit(writeBox(page));
    expect(source.acts).toHaveLength(1);
    expect(button(writeBox(page), 'Write')?.disabled).toBe(true);
    answer.reject(new Error('the work has ended'));
    await settle();
    expect(writeBox(page).querySelector('[role="alert"]')?.textContent).toBe('Refused: the work has ended');
    expect(typed(page)).toBe('Once');
  });
});
