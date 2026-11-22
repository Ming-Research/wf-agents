// @vitest-environment happy-dom
import './dom-setup';

import { act, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import { Bar, Tabs } from './parts';
import { aheadCount, aheadOf } from './works/waiting';

afterEach(() => {
  document.body.innerHTML = '';
});

type Tab = 'item' | 'work' | 'step';

/// The tabs of a window, the one shown kept as the owner chooses it; and each choice told.
function draw() {
  const told: Tab[] = [];
  function Window() {
    const [tab, setTab] = useState<Tab>('item');
    return (
      <Tabs
        label="What the window shows"
        choices={[
          { value: 'item', name: 'Item' },
          { value: 'work', name: 'Work', title: 'What of the work the lines do not say' },
          { value: 'step', name: 'Step' },
        ]}
        value={tab}
        onChange={(to) => {
          told.push(to);
          setTab(to);
        }}
      />
    );
  }
  const container = document.body.appendChild(document.createElement('div'));
  const root = createRoot(container);
  act(() => root.render(<Window />));
  const tabs = () => [...container.querySelectorAll<HTMLButtonElement>('[role="tab"]')];
  const shown = () => tabs().filter((t) => t.getAttribute('aria-selected') === 'true').map((t) => t.textContent);
  const key = (key: string) =>
    act(() => {
      (document.activeElement ?? tabs()[0]).dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }));
    });
  return { container, tabs, shown, key, told };
}

describe("a window's tabs", () => {
  it('are a list of tabs by its name, the one shown selected and alone in the order of the keys, each saying what it is where it says', () => {
    const { container, tabs, shown } = draw();
    const list = container.querySelector('[role="tablist"]');
    expect(list?.getAttribute('aria-label')).toBe('What the window shows');
    expect(tabs().map((t) => t.textContent)).toEqual(['Item', 'Work', 'Step']);
    expect(shown()).toEqual(['Item']);
    expect(tabs().map((t) => t.tabIndex)).toEqual([0, -1, -1]);
    expect(tabs()[1].title).toBe('What of the work the lines do not say');
  });

  it('show the one pressed', () => {
    const { tabs, shown, told } = draw();
    act(() => tabs()[2].click());
    expect(shown()).toEqual(['Step']);
    expect(tabs().map((t) => t.tabIndex)).toEqual([-1, -1, 0]);
    expect(told).toEqual(['step']);
  });

  it('move to the next and the one before by the arrows, round from the last to the first, the keys going with them', () => {
    const { tabs, shown, key, told } = draw();
    act(() => tabs()[0].focus());
    key('ArrowRight');
    expect(shown()).toEqual(['Work']);
    expect(document.activeElement).toBe(tabs()[1]);
    key('ArrowRight');
    key('ArrowRight');
    expect(shown()).toEqual(['Item']);
    expect(document.activeElement).toBe(tabs()[0]);
    key('ArrowLeft');
    expect(shown()).toEqual(['Step']);
    expect(document.activeElement).toBe(tabs()[2]);
    // Another key moves nothing.
    key('ArrowDown');
    key('Enter');
    expect(shown()).toEqual(['Step']);
    expect(told).toEqual(['work', 'step', 'item', 'step']);
  });
});

describe("the bar's counts beside Console", () => {
  /// The counts the bar draws beside Console, given these, in order, each by its colour and number.
  function counts(count: number, ahead: number): string[] {
    const container = document.body.appendChild(document.createElement('div'));
    const root = createRoot(container);
    const views = [
      { name: 'Console', href: '#/console', count, countTitle: 'waits', ahead, aheadTitle: 'ahead' },
      { name: 'Works', href: '#/works' },
    ];
    act(() => root.render(<Bar name="wf-agents" views={views} current="#/console" />));
    const drawn = [...container.querySelectorAll('.bar-count')].map(
      (c) => `${c.classList.contains('ahead') ? 'orange' : 'red'} ${c.textContent}`,
    );
    act(() => root.unmount());
    container.remove();
    return drawn;
  }

  it('shows the questions to answer ahead in orange, left of what waits in red, as a card does, and neither at nought', () => {
    expect(counts(1, 2)).toEqual(['orange 2', 'red 1']);
    expect(counts(1, 0)).toEqual(['red 1']);
    expect(counts(0, 2)).toEqual(['orange 2']);
    expect(counts(0, 0)).toEqual([]);
  });

  it('counts the questions to answer ahead as a card does: those of the works that go on', () => {
    const going = { kind: 'next', cell: 'plan' } as const;
    expect(aheadCount([{ now: going, ahead: 2 }, { now: { kind: 'merged' }, ahead: 3 }, { now: going }])).toBe(2);
    expect(aheadOf({ now: { kind: 'cancelled' }, ahead: 1 })).toBe(0);
  });
});
