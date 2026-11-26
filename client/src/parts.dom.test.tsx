// @vitest-environment happy-dom
import './dom-setup';

import { act, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import { Bar, ProjectGroup, Tabs } from './parts';
import { aheadCount, aheadOf, repliesUnreadCount, repliesUnreadOf } from './works/waiting';

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
  function counts(count: number, ahead: number, unread = 0): string[] {
    const container = document.body.appendChild(document.createElement('div'));
    const root = createRoot(container);
    const views = [
      { name: 'Console', href: '#/console', count, countTitle: 'waits', ahead, aheadTitle: 'ahead', unread, unreadTitle: 'unread' },
      { name: 'Works', href: '#/works' },
    ];
    act(() => root.render(<Bar name="wf-agents" views={views} current="#/console" />));
    const drawn = [...container.querySelectorAll('.bar-count')].map(
      (c) => `${c.classList.contains('unread') ? 'accent' : c.classList.contains('ahead') ? 'orange' : 'red'} ${c.textContent}`,
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

  it('shows the replies the owner has not read in the accent, left of the orange and the red counts as a card does, and not at nought', () => {
    expect(counts(1, 2, 3)).toEqual(['accent 3', 'orange 2', 'red 1']);
    expect(counts(0, 0, 3)).toEqual(['accent 3']);
    expect(counts(1, 0, 0)).toEqual(['red 1']);
  });

  it('counts the questions to answer ahead as a card does: those of the works that go on', () => {
    const going = { kind: 'next', cell: 'plan' } as const;
    expect(aheadCount([{ now: going, ahead: 2 }, { now: { kind: 'merged' }, ahead: 3 }, { now: going }])).toBe(2);
    expect(aheadOf({ now: { kind: 'cancelled' }, ahead: 1 })).toBe(0);
  });

  it('counts the replies not read as a card does: those of the works the engine counts them for, however the work stands', () => {
    expect(repliesUnreadCount([{ repliesUnread: 2 }, { repliesUnread: 1 }, {}])).toBe(3);
    expect(repliesUnreadOf({ repliesUnread: 4 })).toBe(4);
    expect(repliesUnreadOf({})).toBe(0);
  });
});

describe("a project's group in a list grouped by project", () => {
  /// A group, folded and opened as the owner presses its head; and its head's parts.
  function group(project: string | null) {
    function Group() {
      const [open, setOpen] = useState(true);
      return (
        <ProjectGroup
          project={project}
          what="works"
          count={2}
          countTitle="2 works"
          note="$1.20"
          noteTitle="What its works cost"
          open={open}
          onToggle={() => setOpen((o) => !o)}
        >
          <table className="works" />
        </ProjectGroup>
      );
    }
    const container = document.body.appendChild(document.createElement('div'));
    const root = createRoot(container);
    act(() => root.render(<Group />));
    const head = () => container.querySelector('section.project-group > h2.project-head') as HTMLElement;
    const button = () => head().querySelector('button') as HTMLButtonElement;
    return { container, head, button };
  }

  it("is headed outside its rows by the project's name, then how many it holds and the note, which say on pointing what they are", () => {
    const { container, head, button } = group('wf-agents');
    const section = container.querySelector('section.project-group') as HTMLElement;
    // The head is no part of the rows: the table stands beside it in the group, not round it.
    expect(head().querySelector('table')).toBeNull();
    expect(section.querySelector('table.works')?.parentElement?.parentElement).toBe(section);
    expect(section.getAttribute('aria-labelledby')).toBe(head().id);
    expect(button().querySelector('.mono')?.textContent).toBe('wf-agents');
    expect(button().querySelector('.fold-mark')).not.toBeNull();
    const count = button().querySelector('.project-count') as HTMLElement;
    expect([count.textContent, count.title]).toEqual(['2', '2 works']);
    const note = button().querySelector('.project-note') as HTMLElement;
    expect([note.textContent, note.title]).toEqual(['$1.20', 'What its works cost']);
  });

  it('folds its rows away at a press on its head and opens them again at the next, saying what a press does', () => {
    const { container, button } = group('wf-agents');
    const rows = () => container.querySelector('table.works');
    expect(button().getAttribute('aria-expanded')).toBe('true');
    expect(button().title).toBe('Fold the works of wf-agents');
    expect(document.getElementById(button().getAttribute('aria-controls') ?? '')).toBe(rows()?.parentElement);
    act(() => button().click());
    expect(rows()).toBeNull();
    expect(button().getAttribute('aria-expanded')).toBe('false');
    expect(button().querySelector('.fold-mark')?.classList.contains('fold-open')).toBe(false);
    expect(button().title).toBe('Unfold the works of wf-agents');
    act(() => button().click());
    expect(rows()).not.toBeNull();
    expect(button().querySelector('.fold-mark')?.classList.contains('fold-open')).toBe(true);
  });

  it('names the group of what belongs to no project so, muted, and says on pointing what it holds', () => {
    const { button } = group(null);
    expect(button().querySelector('.mono:not(.project-count)')).toBeNull();
    expect(button().querySelector('.muted')?.textContent).toBe('no project');
    expect(button().title).toBe('Fold the works of no project: what was added before items had projects');
  });
});
