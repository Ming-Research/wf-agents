// @vitest-environment happy-dom
import './dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import { tableWidth } from './Chart';
import { useKeptWidth, useKeptWidths } from './sizes';

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

describe('a width the owner drags', () => {
  it('is kept in the browser within its bounds, read again on the next page, and forgotten on reset', () => {
    const a = hook(() => useKeptWidth('k', 280, 720));
    expect(a.now()[0]).toBeNull();
    act(() => a.now()[1](900));
    expect(a.now()[0]).toBe(720);
    expect(localStorage.getItem('k')).toBe('720');
    a.unmount();
    const b = hook(() => useKeptWidth('k', 280, 720));
    expect(b.now()[0]).toBe(720);
    act(() => b.now()[1](null));
    expect(b.now()[0]).toBeNull();
    expect(localStorage.getItem('k')).toBeNull();
    b.unmount();
  });

  it('keeps each cell’s column apart, by its id', () => {
    const a = hook(() => useKeptWidths('cells', 64, 480));
    act(() => a.now()[1]('R1', 200));
    act(() => a.now()[1]('D2', 30));
    expect(a.now()[0]).toEqual({ R1: 200, D2: 64 });
    act(() => a.now()[1]('R1', null));
    expect(JSON.parse(localStorage.getItem('cells') ?? '{}')).toEqual({ D2: 64 });
    a.unmount();
  });
});

describe('the width of the chart', () => {
  it('takes the whole frame, the cells sharing it evenly up to 160px each, and no less than each at its least', () => {
    expect(tableWidth(200, ['a', 'b', 'c'], {})).toEqual({ width: 'min(100%, 680px)', minWidth: '464px' });
    expect(tableWidth(200, ['a', 'b', 'c'], { b: 300 })).toEqual({ width: 'min(100%, 820px)', minWidth: '676px' });
  });
});
