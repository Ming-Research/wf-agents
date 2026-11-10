// @vitest-environment happy-dom
import '../dom-setup';

import { type ReactNode, act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import type { Act } from '../works-api';
import { fixtureCall, fixtureItems, fixtureRound, fixtureVersion, fixtureWork } from './fixtures';
import { type Source, SourceContext } from './source';
import { WorkScreen } from './WorkView';
import { WorksTable } from './WorksList';

let unmount: (() => void) | undefined;
afterEach(() => {
  unmount?.();
  unmount = undefined;
  document.body.innerHTML = '';
  localStorage.clear();
});

/// A source answered from the fixtures, which keeps what the owner did.
function fixtureSource(): Source & { acts: { work: number; what: Act }[] } {
  const acts: { work: number; what: Act }[] = [];
  return {
    acts,
    loadItems: async () => fixtureItems(),
    loadWork: async (n) => fixtureWork(n),
    loadVersion: async (n, block, at) => fixtureVersion(n, block, at),
    loadRound: async (n, run, round) => fixtureRound(n, run, round),
    loadCall: async (n, call) => fixtureCall(n, call),
    act: async (work, what) => {
      acts.push({ work, what });
      return null;
    },
    changes: () => () => undefined,
  };
}

function draw(element: ReactNode, source: Source): HTMLElement {
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  act(() => root.render(<SourceContext.Provider value={source}>{element}</SourceContext.Provider>));
  unmount = () => act(() => root.unmount());
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
  [...root.querySelectorAll('button')].find((b) => b.textContent?.trim() === words);

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
  });
}

/// Chooses the step at this place by its row's words at the left.
async function chooseStep(page: HTMLElement, index: number) {
  await click(page.querySelectorAll('tbody tr.step-row')[index]?.querySelector('.step-lead button'));
}

describe('the list of works', () => {
  it('lists every work, those waiting for the owner first and edged, and a row opens its work', async () => {
    const page = draw(<WorksTable items={fixtureItems()} />, fixtureSource());
    const rows = [...page.querySelectorAll('tbody tr.work-row')] as HTMLElement[];
    expect(rows.map((r) => Number(r.dataset.work))).toEqual([43, 44, 45, 41, 49, 42, 50, 46, 47, 48]);
    expect(rows.map((r) => r.classList.contains('for-owner'))).toEqual([true, true, true, true, false, false, false, false, false, false]);
    expect(rows[0].querySelector('.now-words')?.textContent).toBe("Waits at plan for the owner's answer to a question");
    expect(rows[0].querySelectorAll('.strip .state-mark')).toHaveLength(6);
    await click(rows[3]);
    expect(window.location.hash).toBe('#/works/41');
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
    expect(panel.textContent).toContain('it had no record');
    expect(texts(panel, '.panel-part-label')).toEqual(
      expect.arrayContaining(['Why it ran 1', 'Started from 5', 'Calls 2', 'How it ended']),
    );
    expect(panel.textContent).toContain('the results of 1 call');
    expect(panel.textContent).toContain('Should the cached tokens be shown apart from the others?');
    expect(panel.textContent).toContain('it wrote nothing');
    // A result is read only when it is opened.
    await click(button(panel, 'Show result'));
    expect(panel.querySelector('pre.content')?.textContent).toBe('Found 0 errors.');
  });

  it('shows why a return was sent back, and what a finished run wrote', async () => {
    const { page } = drawWork(41);
    await chooseStep(page, 2);
    const panel = page.querySelector('aside[aria-label="Run 2 of design"]') as HTMLElement;
    expect(panel.textContent).toContain('it handed in code, which the cell may not write');
    expect(panel.textContent).toContain('Finished at');
    await chooseStep(page, 9);
    const way = page.querySelector('aside[aria-label="Run 8 of implement"]') as HTMLElement;
    expect(way.textContent).toContain('Way round');
    expect(way.querySelector('.way-back')?.textContent).toContain('by a cell after it: a way round');
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
});

describe('what the owner does', () => {
  it('answers a question the work waits for', async () => {
    const { page, source } = drawWork(43);
    const box = page.querySelector('form.answer') as HTMLElement;
    expect(box.textContent).toContain('Should the list show the works of cancelled items');
    type(box.querySelector('textarea'), 'Leave them out');
    await submit(box);
    expect(source.acts).toEqual([{ work: 43, what: { kind: 'answer', call: 1, result: 'Leave them out' } }]);
  });

  it('lets a run go on at its limit, and a cell at its limit of ways round', async () => {
    const run = drawWork(44);
    await click(button(run.page, 'Let the run go on'));
    expect(run.source.acts).toEqual([{ work: 44, what: { kind: 'answer', call: 1, result: 'go on' } }]);
    unmount?.();
    const cell = drawWork(45);
    await click(button(cell.page, 'Let implement go on'));
    expect(cell.source.acts).toEqual([{ work: 45, what: { kind: 'letGo', cell: 'implement' } }]);
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

  it('writes the block it picks, by default the one of the cell the work is at, saying who reads it', async () => {
    const { page, source } = drawWork(41);
    const box = () => page.querySelector('form.write-box') as HTMLElement;
    // The work is at the merge, which reads the owner's approval, a choice.
    expect((box().querySelector('#write-block') as HTMLSelectElement).value).toBe('approval');
    expect(box().querySelector('.write-readers')?.textContent).toBe('Read by merge: each runs again if the value changes');
    type(box().querySelector('textarea'), 'Ship it');
    await submit(box());
    expect(source.acts.at(-1)).toEqual({ work: 41, what: { kind: 'write', block: 'approval', content: 'Ship it', value: 'yes' } });
    // Choosing a run of the implementation, the box follows it to its opinions.
    await chooseStep(page, 7);
    expect((box().querySelector('#write-block') as HTMLSelectElement).value).toBe('opinions_implement');
    expect(box().querySelector('.write-readers')?.textContent).toBe('Read by implement: each runs again for it');
    type(box().querySelector('textarea'), 'Keep the totals on one line.');
    // What was typed stays while another step is chosen.
    await chooseStep(page, 8);
    expect((box().querySelector('textarea') as HTMLTextAreaElement).value).toBe('Keep the totals on one line.');
    type(box().querySelector('#write-block'), 'opinions_design');
    await submit(box());
    expect(source.acts.at(-1)).toEqual({
      work: 41,
      what: { kind: 'write', block: 'opinions_design', content: 'Keep the totals on one line.', value: null },
    });
    expect((box().querySelector('textarea') as HTMLTextAreaElement).value).toBe('');
  });
});
