// @vitest-environment happy-dom
import './dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import { useKept, useKeptFolds, useKeptTicks } from './kept';

afterEach(() => {
  localStorage.clear();
  document.body.innerHTML = '';
});

/// Renders a hook and gives what it returns at the last render.
function hook<T>(use: () => T): { now: () => T; unmount: () => void } {
  let last: T | undefined;
  function Probe() {
    last = use();
    return null;
  }
  const root = createRoot(document.body.appendChild(document.createElement('div')));
  act(() => root.render(<Probe />));
  return { now: () => last as T, unmount: () => act(() => root.unmount()) };
}

const KINDS = ['open', 'done', 'cancelled'] as const;

describe('a choice the owner makes of how a view shows', () => {
  it('is the first until the owner picks another, kept in the browser for the next page, and a kept choice no longer allowed is the first', () => {
    const a = hook(() => useKept('k', 'matrix', ['matrix', 'flowchart']));
    expect(a.now()[0]).toBe('matrix');
    act(() => a.now()[1]('flowchart'));
    expect(a.now()[0]).toBe('flowchart');
    expect(localStorage.getItem('k')).toBe('flowchart');
    a.unmount();
    expect(hook(() => useKept('k', 'matrix', ['matrix', 'flowchart'])).now()[0]).toBe('flowchart');
    localStorage.setItem('k', 'table');
    expect(hook(() => useKept('k', 'matrix', ['matrix', 'flowchart'])).now()[0]).toBe('matrix');
  });
});

describe('the boxes the owner ticks of what a view shows', () => {
  it('are those given first until the owner ticks one, kept in the browser in the order of the choices, and stand on the next page', () => {
    const a = hook(() => useKeptTicks('k', ['open'], KINDS));
    expect([...a.now()[0]]).toEqual(['open']);
    expect(localStorage.getItem('k')).toBeNull();
    act(() => a.now()[1]('cancelled', true));
    act(() => a.now()[1]('done', true));
    expect(localStorage.getItem('k')).toBe('open,done,cancelled');
    act(() => a.now()[1]('open', false));
    expect([...a.now()[0]].sort()).toEqual(['cancelled', 'done']);
    expect(localStorage.getItem('k')).toBe('done,cancelled');
    a.unmount();
    expect([...hook(() => useKeptTicks('k', ['open'], KINDS)).now()[0]].sort()).toEqual(['cancelled', 'done']);
  });

  it('keep none ticked as none, take what a kept list allows, and take a kept list none of whose entries is allowed as none kept', () => {
    localStorage.setItem('k', '');
    expect([...hook(() => useKeptTicks('k', ['open'], KINDS)).now()[0]]).toEqual([]);
    localStorage.setItem('k', 'done,shut');
    expect([...hook(() => useKeptTicks('k', ['open'], KINDS)).now()[0]]).toEqual(['done']);
    localStorage.setItem('k', 'shut,gone');
    expect([...hook(() => useKeptTicks('k', ['open'], KINDS)).now()[0]]).toEqual(['open']);
  });
});

describe('the items the owner folds and unfolds in a tree', () => {
  /// What is kept, by the items' numbers in order.
  const inOrder = (folds: ReadonlyMap<number, boolean>) => [...folds].sort(([a], [b]) => a - b);

  it('are none until the owner folds or unfolds one, and keep the last he chose of each, in the browser for the next page', () => {
    const a = hook(() => useKeptFolds('k'));
    expect(inOrder(a.now()[0])).toEqual([]);
    expect(localStorage.getItem('k')).toBeNull();
    act(() => a.now()[1](27, false));
    act(() => a.now()[1](5, true));
    act(() => a.now()[1](27, true));
    expect(inOrder(a.now()[0])).toEqual([
      [5, true],
      [27, true],
    ]);
    expect(JSON.parse(localStorage.getItem('k') ?? '')).toEqual({ 5: 'folded', 27: 'folded' });
    act(() => a.now()[1](5, false));
    a.unmount();
    expect(inOrder(hook(() => useKeptFolds('k')).now()[0])).toEqual([
      [5, false],
      [27, true],
    ]);
  });

  it('take what an earlier page kept that is no such choice as none kept', () => {
    for (const kept of ['not json', '[1, 2]', 'null', '"folded"']) {
      localStorage.setItem('k', kept);
      expect(inOrder(hook(() => useKeptFolds('k')).now()[0])).toEqual([]);
    }
    localStorage.setItem('k', JSON.stringify({ 3: 'folded', 4: 'open', x: 'folded', 0: 'unfolded', 5: 'unfolded' }));
    expect(inOrder(hook(() => useKeptFolds('k')).now()[0])).toEqual([
      [3, true],
      [5, false],
    ]);
  });
});
