// @vitest-environment happy-dom
import './dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import type { Comparison, Definition } from './api';
import { FlowPage } from './App';
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

describe('the chart', () => {
  it('shows the cells in the order of the walk, the blocks as columns, and what each cell does with each block', () => {
    const page = draw({ state: 'compiled', flow: small() });
    expect(texts(page, 'tbody .cell-title')).toEqual([
      'Plan',
      'Judge the plan',
      'Design',
      'Answer comments',
      'Check with the latest main',
      'Merge',
    ]);
    expect(texts(page, 'thead .block-name')).toEqual([
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
    const plan = page.querySelectorAll('tbody tr')[0];
    expect(texts(plan as HTMLElement, '.spot .visually-hidden')).toEqual([
      'plan reads request',
      'plan may write plan',
      'plan reads opinions',
    ]);
    const design = page.querySelectorAll('tbody tr')[2];
    expect(design.textContent).toContain('needs_design is yes');
    expect(texts(design as HTMLElement, '.spot .visually-hidden')[1]).toBe('design runs only by the value of needs_design');
    expect(page.querySelectorAll('tbody tr')[3].textContent).toContain('The owner confirmed the plan');
    // The one way back: judge writes the opinions plan reads.
    expect(page.querySelector('.legend')?.textContent).toContain('way back (1)');
    expect(page.querySelectorAll('.arrow')).toHaveLength(1);
  });

  it('shows everything about a cell chosen, and about a block chosen from it', () => {
    const page = draw({ state: 'compiled', flow: small() });
    click(page.querySelectorAll('tbody .cell-button')[0]);
    const panel = page.querySelector('section[aria-label="Cell plan"]') as HTMLElement;
    expect(panel.textContent).toContain('sharing its session with design, answer');
    expect(texts(panel, '.uses li')).toEqual([
      'request written by people',
      'opinions written by judge, people way back from judge',
      'plan read by judge, design, answer',
    ]);
    click([...panel.querySelectorAll('button.link')].find((b) => b.textContent === 'opinions'));
    const block = page.querySelector('section[aria-label="Block opinions"]') as HTMLElement;
    expect(block.textContent).toContain('judge → plan');
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
    expect(page.querySelector('tbody tr')?.className).toContain('status-changed');
    expect(page.querySelectorAll('.part.added')).toHaveLength(1);

    click([...page.querySelectorAll('button')].find((b) => b.textContent === 'As it was'));
    expect(texts(page, 'tbody .cell-title')).toEqual(['A', 'B']);
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
