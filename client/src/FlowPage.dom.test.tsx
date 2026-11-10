// @vitest-environment happy-dom
import './dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import type { Comparison, Definition } from './api';
import { FlowPage, Shell } from './App';
import { cell, flowOf, small } from './testing';

const NOTHING: Comparison = { state: 'nothingToCompare' };

let unmount: (() => void) | undefined;
afterEach(() => {
  unmount?.();
  unmount = undefined;
  document.body.innerHTML = '';
});

/// Draws the page for this definition and comparison, and gives its root.
function draw(definition: Definition, comparison: Comparison = NOTHING): HTMLElement {
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  act(() => root.render(<FlowPage definition={definition} comparison={comparison} />));
  unmount = () => act(() => root.unmount());
  return container;
}

const texts = (root: HTMLElement, selector: string) =>
  [...root.querySelectorAll(selector)].map((e) => e.textContent?.trim() ?? '');

function click(element: Element | null | undefined) {
  if (!element) throw new Error('nothing to click');
  act(() => (element as HTMLElement).click());
}

/// The row of a block.
const rowOf = (page: HTMLElement, block: string) =>
  [...page.querySelectorAll('tbody tr')].find((tr) => tr.querySelector('.block-name')?.textContent === block) as HTMLElement;

describe('the chart', () => {
  it('shows the blocks down the side, the cells across in the order of the walk, and what each cell does with each block', () => {
    const page = draw({ state: 'compiled', flow: small() });
    expect(texts(page, 'thead .cell-title')).toEqual([
      'Plan',
      'Judge the plan',
      'Design',
      'Answer comments',
      'Check with the latest main',
      'Merge',
    ]);
    expect(texts(page, 'tbody .block-name')).toEqual([
      'request',
      'plan',
      'needs_design',
      'opinions',
      'design',
      'comments',
      'reply',
      'main',
      'ci',
    ]);
    expect(texts(rowOf(page, 'plan'), '.spot .visually-hidden')).toEqual([
      'plan may write plan',
      'judge reads plan',
      'design reads plan',
      'answer reads plan',
    ]);
    expect(texts(rowOf(page, 'needs_design'), '.spot .visually-hidden')).toEqual([
      'judge may write needs_design',
      'design runs only by the value of needs_design',
    ]);
    expect(rowOf(page, 'needs_design').querySelector('.block-values')?.textContent).toBe('yes|no');
    const heads = [...page.querySelectorAll('thead .cell-head')];
    expect(heads[2].querySelector('.cell-when')?.textContent).toBe('runs when needs_design is yes');
    expect(heads[3].querySelector('.cell-waits')?.textContent).toBe('waits until The owner confirmed the plan');
    expect(heads[3].querySelector('.cell-waits')?.getAttribute('title')).toBe('waits until The owner confirmed the plan');
    // The one way back: judge writes the opinions plan reads, and the arrow is at plan's read.
    expect(page.querySelector('.legend')?.textContent).toContain('way back (1)');
    const arrows = [...page.querySelectorAll('.arrow')];
    expect(arrows).toHaveLength(1);
    expect(arrows[0].closest('td')?.textContent).toBe('plan reads opinions');
  });

  it('shows everything about a cell chosen by its column’s head, and about a block chosen from it', () => {
    const page = draw({ state: 'compiled', flow: small() });
    click(page.querySelectorAll('thead .cell-head .head-button')[0]);
    const panel = page.querySelector('aside[aria-label="Cell plan"]') as HTMLElement;
    expect(panel.textContent).toContain('Shares its session with design, answer');
    const rows = [...panel.querySelectorAll('.uses li')].map((li) =>
      [...li.children].map((c) => c.textContent?.trim()).join(' '),
    );
    expect(rows.slice(0, 3)).toEqual([
      'request written by people',
      'opinions written by judge, people way back from judge',
      'plan read by judge, design, answer',
    ]);
    click([...panel.querySelectorAll('button.link')].find((b) => b.textContent === 'opinions'));
    const block = page.querySelector('aside[aria-label="Block opinions"]') as HTMLElement;
    expect(block.textContent).toContain('judge → plan');
  });

  it('shows everything about a block chosen by its row’s name', () => {
    const page = draw({ state: 'compiled', flow: small() });
    click(rowOf(page, 'ci').querySelector('.head-button'));
    const panel = page.querySelector('aside[aria-label="Block ci"]') as HTMLElement;
    expect(panel.textContent).toContain('a choice of green, red');
    expect(rowOf(page, 'ci').className).toContain('chosen');
  });
});

describe('a comparison', () => {
  it('lists each change under its place and marks it on the chart', () => {
    const before = flowOf([cell('a', { title: 'A', outputs: ['x'] }), cell('b', { title: 'B', inputs: ['x'] })], { x: {} });
    const after = flowOf([cell('a', { title: 'First', outputs: ['x'] }), cell('b', { title: 'B', inputs: ['x', 'y'] })], {
      x: {},
      y: { people: true },
    });
    const page = draw(
      { state: 'compiled', flow: after },
      {
        state: 'compared',
        against: { state: 'compiled', flow: before },
        changes: [
          { kind: 'title', cell: 'a', before: 'A', after: 'First' },
          { kind: 'inputAdded', cell: 'b', block: 'y' },
          { kind: 'blockAdded', block: 'y' },
        ],
      },
    );
    expect(texts(page, '.change-group h3')).toEqual(['a First', 'b B', 'Block y']);
    expect(texts(page, '.changes .change')).toEqual(['TitleA → First', 'Readsy', 'Declared']);
    expect(page.querySelector('thead .cell-head')?.className).toContain('status-changed');
    expect(rowOf(page, 'y').className).toContain('status-added');
    expect(page.querySelectorAll('.part.added')).toHaveLength(1);

    click([...page.querySelectorAll('button')].find((b) => b.textContent === 'As it was'));
    expect(texts(page, 'thead .cell-title')).toEqual(['A', 'B']);
  });
});

describe('a definition that does not compile', () => {
  it('shows its faults, each with its file and field, in place of the chart', () => {
    const page = draw({
      state: 'faults',
      faults: [{ file: 'plan.toml', field: 'inputs[2]', what: { kind: 'noSuchBlock', block: 'ghost' } }],
    });
    expect(page.querySelector('.matrix')).toBeNull();
    expect(texts(page, '.faults tbody td')).toEqual(['plan.toml', 'inputs[2]', 'The block ghost is not declared in flow.toml.']);
  });
});

describe('the page round the view', () => {
  it('shows the views in the bar, the one shown marked, and switches light and dark, following the system at first', () => {
    const container = document.createElement('div');
    document.body.append(container);
    const root = createRoot(container);
    act(() =>
      root.render(
        <Shell route={{ view: 'work', work: 3 }}>
          <p>view</p>
        </Shell>,
      ),
    );
    unmount = () => act(() => root.unmount());
    expect(texts(container, '.bar-tab')).toEqual(['Console', 'Works', 'Flow']);
    expect(texts(container, '.bar-tab[aria-current="page"]')).toEqual(['Works']);
    expect(document.documentElement.hasAttribute('data-theme')).toBe(false);
    click([...container.querySelectorAll('.switch-choice')].find((b) => b.textContent === 'Dark'));
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
    click([...container.querySelectorAll('.switch-choice')].find((b) => b.textContent === 'Auto'));
    expect(document.documentElement.hasAttribute('data-theme')).toBe(false);
  });

  it('folds the panel away by its handle, brings it back, and lets go of what is chosen with Escape', () => {
    const page = draw({ state: 'compiled', flow: small() });
    click(rowOf(page, 'plan').querySelector('.head-button'));
    const drawer = () => page.querySelector('aside[aria-label="Block plan"]') as HTMLElement | null;
    click(drawer()?.querySelector('.drawer-tab'));
    expect(drawer()?.querySelector('.drawer-inner')?.hasAttribute('hidden')).toBe(true);
    click(drawer()?.querySelector('.drawer-tab'));
    expect(drawer()?.querySelector('.drawer-inner')?.hasAttribute('hidden')).toBe(false);
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    });
    expect(drawer()).toBeNull();
  });
});

describe('the panel and the widths', () => {
  afterEach(() => localStorage.clear());

  it('has its handle from the start, folded, and opened with nothing chosen says what to choose', () => {
    const page = draw({ state: 'compiled', flow: small() });
    const drawer = page.querySelector('aside[aria-label="Nothing chosen"]') as HTMLElement;
    expect(drawer.querySelector('.drawer-tab')).not.toBeNull();
    expect(drawer.querySelector('.drawer-inner')?.hasAttribute('hidden')).toBe(true);
    expect(page.querySelector('.view-body')?.className).toContain('panel-folded');
    click(drawer.querySelector('.drawer-tab'));
    expect(drawer.textContent).toContain("Choose a cell's head or a block's name");
    expect(page.querySelector('.view-body')?.className).not.toContain('panel-folded');
  });

  it('opens with the list of changes when comparing', () => {
    const flow = small();
    const page = draw(
      { state: 'compiled', flow },
      { state: 'compared', against: { state: 'compiled', flow }, changes: [] },
    );
    const drawer = page.querySelector('aside[aria-label="What changed"]') as HTMLElement;
    expect(drawer.querySelector('.drawer-inner')?.hasAttribute('hidden')).toBe(false);
  });

  it('says the flow’s limits once, in the view’s line, and a cell’s own limit in its panel', () => {
    const page = draw({ state: 'compiled', flow: small() });
    expect(page.querySelector('.view-facts')?.textContent).toBe(
      'Limits of a cell: 3 ways round in a row, 2 returns sent back, 40 calls',
    );
    click(page.querySelectorAll('thead .cell-head .head-button')[0]);
    const plan = page.querySelector('aside[aria-label="Cell plan"]') as HTMLElement;
    expect(plan.textContent).not.toContain('Limit');
    click(page.querySelectorAll('thead .cell-head .head-button')[3]);
    const answer = page.querySelector('aside[aria-label="Cell answer"]') as HTMLElement;
    expect(texts(answer, '.status-line')).toContain(
      "Limits of its own5 runs in a row that are ways round (the flow's: 3)",
    );
  });

  it('labels the corner for both: the cells to its right, the blocks under it', () => {
    const page = draw({ state: 'compiled', flow: small() });
    expect(texts(page, '.corner .label')).toEqual(['Cells →', 'Blocks ↓']);
  });

  it('marks the lines’ names, not the engine', () => {
    const page = draw({ state: 'compiled', flow: small() });
    const lines = [...page.querySelectorAll('thead .cell-line')].map((e) => [
      e.textContent,
      e.classList.contains('line-name'),
    ]);
    expect(lines).toEqual([
      ['engineer', true],
      ['review', true],
      ['engineer', true],
      ['engineer', true],
      ['engine', false],
      ['engine', false],
    ]);
  });
});
