// @vitest-environment happy-dom
import './dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import type { Comparison, Definition } from './api';
import { Views } from './App';
import type { Route } from './route';
import { small } from './testing';
import { fixtureCall, fixtureItems, fixtureRound, fixtureVersion, fixtureWork } from './works/fixtures';
import { type Source, SourceContext } from './works/source';

let unmount: (() => void) | undefined;
afterEach(() => {
  unmount?.();
  unmount = undefined;
  document.body.innerHTML = '';
  localStorage.clear();
});

const source: Source = {
  loadItems: async () => fixtureItems(),
  loadWork: async (n) => fixtureWork(n),
  loadVersion: async (n, block, at) => fixtureVersion(n, block, at),
  loadRound: async (n, run, round) => fixtureRound(n, run, round),
  loadCall: async (n, call) => fixtureCall(n, call),
  act: async () => null,
  changes: () => () => undefined,
};

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

/// Draws the views and gives the way to go to another route, and how often the flow was read.
function drawViews(first: Route) {
  let reads = 0;
  const loadFlow = async (): Promise<{ definition: Definition; comparison: Comparison }> => {
    reads += 1;
    return { definition: { state: 'compiled', flow: small() }, comparison: { state: 'nothingToCompare' } };
  };
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  const go = (route: Route) =>
    act(() =>
      root.render(
        <SourceContext.Provider value={source}>
          <Views route={route} loadFlow={loadFlow} />
        </SourceContext.Provider>,
      ),
    );
  go(first);
  unmount = () => act(() => root.unmount());
  return { container, go, reads: () => reads };
}

const shown = (container: HTMLElement) => container.querySelector('.view:not([hidden])') as HTMLElement;

describe('the views', () => {
  it('shows the view each route names', async () => {
    const { container, go } = drawViews({ view: 'works' });
    await settle();
    expect(shown(container).querySelector('table.works')).not.toBeNull();
    go({ view: 'console' });
    // The console is read when it is first shown.
    expect(shown(container).textContent).toBe('Reading the console');
    go({ view: 'work', work: 43 });
    await settle();
    expect(shown(container).querySelector('.view-name')?.textContent).toBe('List every work');
    go({ view: 'flow' });
    await settle();
    expect(shown(container).querySelector('.matrix')).not.toBeNull();
  });

  it('keeps the flow as the owner left it while another tab is shown, and does not read it again', async () => {
    const { container, go, reads } = drawViews({ view: 'flow' });
    await settle();
    const plan = () =>
      [...container.querySelectorAll('tbody tr')].find((tr) => tr.querySelector('.block-name')?.textContent === 'plan');
    act(() => (plan()?.querySelector('.head-button') as HTMLElement).click());
    expect(plan()?.className).toContain('chosen');
    go({ view: 'works' });
    await settle();
    expect(container.querySelector('.matrix')?.closest('.view')?.hasAttribute('hidden')).toBe(true);
    // Escape on another tab is not the flow's.
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    });
    go({ view: 'flow' });
    await settle();
    expect(reads()).toBe(1);
    expect(plan()?.className).toContain('chosen');
  });
});
