// @vitest-environment happy-dom
import './dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { AGAIN_MS, newlyWaiting, useNotifications, waitingByWork, waitingKey } from './notify';
import type { Item, Items, Now } from './works-api';
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
    readonly options: NotificationOptions = {},
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

function Notifying({ items }: { items: Items | null }) {
  useNotifications(items);
  return null;
}

/// Draws the hook with these items, and gives the way to draw it again with others.
function drawn(items: Items | null) {
  const container = document.createElement('div');
  const root = createRoot(container);
  act(() => root.render(<Notifying items={items} />));
  unmounts.push(() => act(() => root.unmount()));
  return (next: Items | null) => act(() => root.render(<Notifying items={next} />));
}

/// The fixtures' items, with the work of this item doing this now.
function withNow(item: number, now: Now): Items {
  const items = fixtureItems().items.map((i): Item => (i.number === item ? { ...i, work: { ...i.work, now } } : i));
  return { items, outage: null };
}

const question = (call: number): Now => ({
  kind: 'waits',
  at: 'plan',
  on: [{ kind: 'call', call, who: 'owner', what: { kind: 'question', who: 'owner', question: 'Which?' } }],
});

describe('what newly waits for the owner', () => {
  it("is what the bar counts, told apart by what the owner's answer is for", () => {
    expect(waitingKey(question(3))).toBe('waits plan: call 3');
    expect(waitingKey(question(4))).not.toBe(waitingKey(question(3)));
    expect(waitingKey({ kind: 'paused', by: 'engine', why: 'A round failed.' })).toBe('paused A round failed.');
    expect(waitingKey({ kind: 'fault', words: 'Two entries at 3' })).toBe('fault Two entries at 3');
    expect(waitingKey({ kind: 'paused', by: 'owner', why: null })).toBeNull();
    expect(waitingKey({ kind: 'waits', at: 'plan', on: [{ kind: 'slot' }] })).toBeNull();
    expect(waitingKey({ kind: 'round', cell: 'plan', run: 1, round: 1, since: 0 })).toBeNull();
  });

  it('is nothing at the first reading, and then each work that waits for something it did not wait for before', () => {
    const items = fixtureItems().items;
    expect(newlyWaiting(null, items)).toEqual([]);
    const before = waitingByWork(items);
    expect(newlyWaiting(before, items)).toEqual([]);
    const again = withNow(142, question(9)).items;
    expect(newlyWaiting(before, again).map((i) => i.number)).toEqual([142]);
    expect(newlyWaiting(waitingByWork(again), withNow(142, question(10)).items).map((i) => i.number)).toEqual([142]);
  });
});

describe("the browser's notifications", () => {
  it('tell of each work that newly waits for the owner once, by its item, and a click opens the work', () => {
    const draw = drawn(fixtureItems());
    expect(Told.made).toEqual([]);
    draw(withNow(142, question(9)));
    expect(Told.made.map((n) => [n.title, n.options.body])).toEqual([
      ["Waits for you: #142 Keep the panel's width", "Waits at plan for the owner's answer to a question"],
    ]);
    draw(withNow(142, question(9)));
    expect(Told.made).toHaveLength(1);
    act(() => Told.made[0].onclick?.());
    expect(window.location.hash).toBe('#/works/42');
    expect(Told.made[0].closed).toBe(true);
  });

  it('tell nothing while the owner looks at the page, or where the browser was given no leave', () => {
    const draw = drawn(fixtureItems());
    // The page is in sight, and has the focus.
    expect(document.visibilityState).toBe('visible');
    looking = true;
    draw(withNow(142, question(9)));
    looking = false;
    Told.permission = 'denied';
    draw(withNow(142, question(10)));
    expect(Told.made).toEqual([]);
  });

  it('tell again each hour how many works wait, while any does', () => {
    vi.useFakeTimers();
    drawn(fixtureItems());
    const waiting = waitingByWork(fixtureItems().items).size;
    expect(waiting).toBeGreaterThan(0);
    act(() => {
      vi.advanceTimersByTime(AGAIN_MS);
    });
    expect(Told.made.map((n) => n.title)).toEqual([`${waiting} works wait for you`]);
    act(() => Told.made[0].onclick?.());
    expect(window.location.hash).toBe('#/console');
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
});
