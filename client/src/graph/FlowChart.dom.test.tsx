// @vitest-environment happy-dom
import '../dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import { FlowPage } from '../App';
import { small } from '../testing';

let unmount: (() => void) | undefined;
afterEach(() => {
  unmount?.();
  unmount = undefined;
  document.body.innerHTML = '';
});

function draw(): HTMLElement {
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  act(() => root.render(<FlowPage definition={{ state: 'compiled', flow: small() }} />));
  unmount = () => act(() => root.unmount());
  return container;
}

function click(element: Element | null | undefined) {
  if (!element) throw new Error('nothing to click');
  act(() => (element as HTMLElement).click());
}

const button = (root: ParentNode, name: string) =>
  [...root.querySelectorAll('button')].find((b) => b.textContent === name);

/// Waits, letting the page go on, until `done` holds: the flowchart is read and laid out in its own time.
async function until(done: () => boolean) {
  for (let i = 0; i < 400 && !done(); i += 1) {
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 25));
    });
  }
  expect(done()).toBe(true);
}

/// Draws the page and switches it to the flowchart, once laid out.
async function flowchart(): Promise<HTMLElement> {
  const page = draw();
  click(button(page, 'Flowchart'));
  await until(() => page.querySelector('.graph-cell') !== null);
  return page;
}

const texts = (root: ParentNode, selector: string) =>
  [...root.querySelectorAll(selector)].map((e) => e.textContent?.trim() ?? '');

describe('the switch between the matrix and the flowchart', () => {
  it('shows the matrix first, the flowchart once switched to, and the matrix again as it was', async () => {
    const page = draw();
    const matrix = page.querySelector('.frame') as HTMLElement;
    expect(matrix.hidden).toBe(false);
    expect(button(page, 'Matrix')?.getAttribute('aria-pressed')).toBe('true');
    expect(page.querySelector('.graph-place')).toBeNull();

    click(button(page, 'Flowchart'));
    await until(() => page.querySelector('.graph-cell') !== null);
    expect(matrix.hidden).toBe(true);
    expect(button(page, 'Flowchart')?.getAttribute('aria-pressed')).toBe('true');
    expect(texts(page, '.graph-cell .cell-id')).toEqual(['plan', 'judge', 'design', 'answer', 'check', 'merge']);
    expect(texts(page, '.legend li')).toContain('way back (1)');

    click(button(page, 'Matrix'));
    expect(matrix.hidden).toBe(false);
    expect((page.querySelector('.graph-place') as HTMLElement).hidden).toBe(true);
    expect(texts(page, 'thead .cell-head .cell-id')).toEqual(['plan', 'judge', 'design', 'answer', 'check', 'merge']);
  });
});

describe('choosing on the flowchart', () => {
  it('opens the panel of the cell chosen, what it is given beside it and the cells that write that lit', async () => {
    const page = await flowchart();
    click(page.querySelector('.graph-cell[data-node="c:design"]'));
    expect(page.querySelector('section[aria-label="Cell design"]')).not.toBeNull();
    expect(page.querySelector('.graph-cell[data-node="c:design"]')?.getAttribute('aria-pressed')).toBe('true');
    expect(texts(page, '.graph-context .graph-context-name')).toEqual(['plan']);
    expect([...page.querySelectorAll('.graph-cell.lit')].map((e) => e.getAttribute('data-node'))).toEqual(['c:plan']);
    // Chosen again, it is let go of, as on the matrix.
    click(page.querySelector('.graph-cell[data-node="c:design"]'));
    expect(page.querySelector('section[aria-label="Cell design"]')).toBeNull();
    expect(page.querySelector('.graph-context')).toBeNull();
  });

  it('opens the panel of a block an arrow carries, of a key of the way into a cell, and of a tag', async () => {
    const page = await flowchart();
    const name = [...page.querySelectorAll('.graph-label .graph-name')].find((b) => b.textContent === 'opinions');
    click(name);
    expect(page.querySelector('section[aria-label="Block opinions"]')).not.toBeNull();
    const row = (key: string) =>
      [...page.querySelectorAll('.graph-way-in .graph-row')].find(
        (b) => b.querySelector('.graph-row-key')?.textContent === key,
      );
    // A fork on one block says the block, its values on its branches.
    click(page.querySelector('.graph-fork button[aria-label="fork on needs_design"]'));
    expect(page.querySelector('section[aria-label="Block needs_design"]')).not.toBeNull();
    expect(row('needs_design = yes')).toBeUndefined();
    // A fork on a named test lists its key.
    expect(row('test = has_comment')).toBeDefined();
    // A requires, after the matrix's square.
    expect(row('test = plan_confirmed')?.querySelector('.part.precondition')).not.toBeNull();
    click(row('test = plan_confirmed'));
    expect(page.querySelector('section[aria-label="Test plan_confirmed"]')).not.toBeNull();
    const tag = [...page.querySelectorAll('.graph-tag')].find(
      (b) => b.querySelector('.graph-tag-name')?.textContent === 'request',
    );
    expect(tag?.querySelector('.sign')?.getAttribute('aria-label')).toBe('people write it');
    click(tag);
    expect(page.querySelector('section[aria-label="Block request"]')).not.toBeNull();
  });
});

describe('the lines of the flowchart', () => {
  /// How many lines of a kind are drawn, and the words on the labels of that kind.
  const lines = (page: HTMLElement, line: string) => page.querySelectorAll(`.graph-lines g[data-line="${line}"]`).length;
  const labels = (page: HTMLElement, line: string) => texts(page, `.graph-label.${line}`);
  const toggle = (page: HTMLElement) => page.querySelector('.view-toggle input') as HTMLInputElement;
  const chooseCell = (page: HTMLElement, id: string) => click(page.querySelector(`.graph-cell[data-node="c:${id}"]`));

  it('draws the main path through one fork for each decision, a branch for each value, and the ways back', async () => {
    const page = await flowchart();
    // Nine steps, two of them out of the fork on needs_design and two out of the fork on has_comment, and the ways
    // through the two gates.
    expect(lines(page, 'main')).toBe(11);
    expect(labels(page, 'main')).toEqual(expect.arrayContaining(['plan', 'yes', 'no', '✓', '✗ design']));
    // plan's write runs judge, the next cell on the path: its name is on the step, with no second line.
    const onPath = [...page.querySelectorAll('.graph-label.main .graph-name')].find((b) => b.textContent === 'plan');
    expect(onPath?.getAttribute('aria-label')).toBe('Block plan, plan to judge');
    expect(lines(page, 'back')).toBe(1);
    expect(texts(page, '.legend li')).toEqual(expect.arrayContaining(['main path', 'triggered_by', 'way back (1)']));
  });

  it('draws the arrows a write runs only for the cell chosen, its trunk named once, and all of them when switched on', async () => {
    const page = await flowchart();
    expect(page.querySelector('.view-toggle')?.textContent).toBe('Data arrows');
    expect(toggle(page).checked).toBe(false);
    // Nothing chosen: the main path and the ways back alone.
    expect(lines(page, 'data')).toBe(0);
    // plan writes what design and answer run: one trunk, drawn as two since design and answer stand on either side of
    // the main path, its name beside each.
    chooseCell(page, 'plan');
    expect(lines(page, 'data')).toBe(2);
    expect(labels(page, 'data')).toEqual(['plan', 'plan']);
    // design is run by plan's write: that arrow alone.
    chooseCell(page, 'design');
    expect(lines(page, 'data')).toBe(1);
    // judge writes nothing a cell after it runs but along the main path, and its way back is drawn anyway.
    chooseCell(page, 'judge');
    expect(lines(page, 'data')).toBe(0);
    expect(lines(page, 'back')).toBe(1);
    // Switched on, every one, the cells where they were.
    const before = (page.querySelector('.graph-cell[data-node="c:judge"]') as HTMLElement).style.cssText;
    click(toggle(page));
    expect(toggle(page).checked).toBe(true);
    expect(lines(page, 'data')).toBe(2);
    expect((page.querySelector('.graph-cell[data-node="c:judge"]') as HTMLElement).style.cssText).toBe(before);
    click(toggle(page));
    expect(lines(page, 'data')).toBe(0);
  });

  it('keeps the toggle out of the view line while the matrix is shown', () => {
    const page = draw();
    expect(page.querySelector('.view-toggle')).toBeNull();
  });
});
