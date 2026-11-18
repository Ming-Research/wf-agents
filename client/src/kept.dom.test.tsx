// @vitest-environment happy-dom
import './dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import { useKept, useKeptTicks } from './kept';

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
