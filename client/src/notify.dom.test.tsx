// @vitest-environment happy-dom
import './dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { AGAIN_MS, heldByWork, newlyHeld, questionsHolding, useNotifications } from './notify';
import type { Item, Items, Now, Waited } from './works-api';
import { fixtureItems } from './works/fixtures';

/// The browser's notifications, as a test sees them: each one made, and the leave the page was given.
class Told {
  static permission: NotificationPermission = 'granted';
  static made: Told[] = [];
  static asked = 0;
  static requestPermission = async () => {
    Told.asked += 1;
    return Told.permission;
  };
  onclick: (() => void) | null = null;
  closed = false;
  constructor(
    readonly title: string,
    readonly options: NotificationOptions & { renotify?: boolean } = {},
  ) {
    Told.made.push(this);
  }
  close() {
    this.closed = true;
  }
}

let looking = false;
let unmounts: (() => void)[] = [];

beforeEach(() => {
  Told.permission = 'granted';
  Told.made = [];
  Told.asked = 0;
  looking = false;
  vi.stubGlobal('Notification', Told);
  vi.spyOn(document, 'hasFocus').mockImplementation(() => looking);
});

afterEach(() => {
  for (const u of unmounts) u();
  unmounts = [];
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  vi.useRealTimers();
  localStorage.clear();
  window.location.hash = '';
});

function Notifying({ items, on }: { items: Items | null; on: boolean }) {
  useNotifications(items, on);
  return null;
}

/// Draws the hook with these items, and gives the way to draw it again with others.
function drawn(items: Items | null, on = true) {
  const container = document.createElement('div');
  const root = createRoot(container);
  act(() => root.render(<Notifying items={items} on={on} />));
  unmounts.push(() => act(() => root.unmount()));
  return (next: Items | null) => act(() => root.render(<Notifying items={next} on={on} />));
}

/// A question to the owner by the call that puts it.
const question = (call: number, words = `Question ${call}?`): Waited => ({
  kind: 'call',
  call,
  who: 'owner',
  what: { kind: 'question', who: 'owner', question: words },
});

const waiting = (...on: Waited[]): Now => ({ kind: 'waits', at: 'plan', on });

/// The fixtures' items, with the work of this item doing this now.
function withNow(item: number, now: Now): Items {
  const items = fixtureItems().items.map((i): Item => (i.number === item ? { ...i, work: { ...i.work, now } } : i));
  return { items, outage: null };
}

describe('the questions that hold a work', () => {
  it('are the questions to the owner it waits for, and nothing else the owner gives', () => {
    expect(questionsHolding(waiting(question(3, 'Which?'), { kind: 'slot' }))).toEqual(['Which?']);
    expect(questionsHolding(waiting({ kind: 'answer', question: 'scope' }, question(4, 'Why?')))).toEqual([
      'scope',
      'Why?',
    ]);
    const limit = { kind: 'atLimit' as const, limit: { of: 'returns' as const, sentBack: 2 } };
    expect(questionsHolding(waiting({ kind: 'call', call: 4, who: 'owner', what: limit }))).toEqual([]);
    expect(questionsHolding(waiting({ kind: 'limit', runs: 3 }))).toEqual([]);
    expect(questionsHolding({ kind: 'paused', by: 'engine', why: 'A round failed.' })).toEqual([]);
    expect(questionsHolding({ kind: 'fault', words: 'Two entries at 3' })).toEqual([]);
  });

  it('are new where a work holds more of them than before, not where one of several was answered', () => {
    const items = fixtureItems().items;
    expect(newlyHeld(null, items)).toEqual([]);
    const before = heldByWork(items);
    expect(newlyHeld(before, items)).toEqual([]);
    const two = withNow(142, waiting(question(3), question(4))).items;
    expect(newlyHeld(before, two).map((i) => i.number)).toEqual([142]);
    // Call 3 answered, call 4 left: nothing new.
    expect(newlyHeld(heldByWork(two), withNow(142, waiting(question(4))).items)).toEqual([]);
  });
});

describe("the browser's notifications", () => {
  it('tell of each work that comes to hold a new question once, by its item and its question, and open the item', () => {
    const draw = drawn(fixtureItems());
    expect(Told.made).toEqual([]);
    draw(withNow(142, waiting(question(9, 'Keep the width per view?'))));
    expect(Told.made.map((n) => [n.title, n.options.body, n.options.tag, n.options.renotify])).toEqual([
      ["Waits for you: #142 Keep the panel's width", 'Keep the width per view?', 'wf-agents-item-142', true],
    ]);
    draw(withNow(142, waiting(question(9, 'Keep the width per view?'))));
    expect(Told.made).toHaveLength(1);
    act(() => Told.made[0].onclick?.());
    expect(window.location.hash).toBe('#/console/items/142');
    expect(Told.made[0].closed).toBe(true);
  });

  it('tell nothing of a fault, a pause of the engine or a command line that cannot run', () => {
    const draw = drawn(fixtureItems());
    draw(withNow(142, { kind: 'paused', by: 'engine', why: 'A round failed.' }));
    draw(withNow(142, { kind: 'fault', words: 'Two entries at 3' }));
    expect(Told.made).toEqual([]);
  });

  it('tell nothing while the owner looks at the page, or where the browser was given no leave', () => {
    const draw = drawn(fixtureItems());
    // The page is in sight, and has the focus.
    expect(document.visibilityState).toBe('visible');
    looking = true;
    draw(withNow(142, waiting(question(9))));
    looking = false;
    Told.permission = 'denied';
    draw(withNow(142, waiting(question(9), question(10))));
    expect(Told.made).toEqual([]);
  });

  it('tell again each hour how many questions are left, while any is', () => {
    vi.useFakeTimers();
    const left = [...heldByWork(fixtureItems().items).values()].reduce((a, b) => a + b, 0);
    expect(left).toBeGreaterThan(0);
    drawn(fixtureItems());
    act(() => {
      vi.advanceTimersByTime(AGAIN_MS);
    });
    const title = `${left} ${left === 1 ? 'question waits' : 'questions wait'} for you`;
    expect(Told.made.map((n) => [n.title, n.options.tag, n.options.renotify])).toEqual([
      [title, 'wf-agents-questions', true],
    ]);
  });

  it('ask the browser for leave once, at the first click on the page', () => {
    Told.permission = 'default';
    drawn(null);
    expect(Told.asked).toBe(0);
    act(() => {
      window.dispatchEvent(new Event('pointerdown'));
      window.dispatchEvent(new Event('pointerdown'));
    });
    expect(Told.asked).toBe(1);
  });

  it('neither tell nor ask on a page that is not the engine\'s own', () => {
    vi.useFakeTimers();
    Told.permission = 'default';
    const draw = drawn(fixtureItems(), false);
    act(() => {
      window.dispatchEvent(new Event('pointerdown'));
    });
    Told.permission = 'granted';
    draw(withNow(142, waiting(question(9))));
    act(() => {
      vi.advanceTimersByTime(AGAIN_MS);
    });
    expect([Told.asked, Told.made.length]).toEqual([0, 0]);
  });
});
