// @vitest-environment happy-dom
import './dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { AGAIN_MS, firstOpening, heldByWork, newlyHeld, openingOf, useNotifications } from './notify';
import type { Item, Items, Now, Wait } from './works-api';
import { fixtureItems } from './works/fixtures';
import { waitCount } from './works/words';

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

/// A wait in the engine's list.
const wait = (key: string, why: string, ends: Wait['ends'] = { kind: 'resume' }): Wait => ({ key, why, ends });

/// The engine's wait for the answer to a question put as a call.
const question = (call: number, words = `Question ${call}?`): Wait => wait(`call-${call}`, words, { kind: 'reply', call });

/// The fixtures' items, with the work of this item waiting on the owner for these, as the engine's list says.
function withWaits(item: number, waits: Wait[]): Items {
  const items = fixtureItems().items.map((i): Item => (i.number === item ? { ...i, work: { ...i.work, waits } } : i));
  return { items, outage: null };
}

/// Each kind the engine's list names, as it gives it.
const KINDS: Wait[] = [
  question(3, 'Which?'),
  wait('answer-scope', 'What of scope?', { kind: 'answer', question: 'scope', version: 4 }),
  wait('limit', 'The run of plan had 2 returns sent back, its limit.', { kind: 'goOn', call: 4 }),
  wait('pause', 'The engine paused the work. A round failed.'),
  wait('held', 'The engine paused every work as it started again.', { kind: 'resumeEverything' }),
  wait('write-content', '`test = may_merge` does not hold, as `content` says, and the cell requires it.', {
    kind: 'write',
    block: 'content',
    value: 'yes',
  }),
  wait('agents', "The agents' command line cannot run. Not logged in.", { kind: 'outside', what: 'Sign it in again.' }),
];

describe('what waits for the owner in a work', () => {
  it("is counted as the engine's list says, whatever its kind, and nothing it leaves out", () => {
    const work = (fixtureItems().items.find((i) => i.number === 142) as Item).work.number;
    const n = (waits: Wait[]) => heldByWork(withWaits(142, waits).items).get(work) ?? 0;
    for (const w of KINDS) expect(n([w])).toBe(1);
    expect(n(KINDS)).toBe(KINDS.length);
    expect(n([])).toBe(0);
  });

  it('are new where a work holds more of them than before, not where one of several was answered', () => {
    const items = fixtureItems().items;
    expect(newlyHeld(null, items)).toEqual([]);
    const before = heldByWork(items);
    expect(newlyHeld(before, items)).toEqual([]);
    const two = withWaits(142, [question(3), question(4)]).items;
    expect(newlyHeld(before, two).map((i) => i.number)).toEqual([142]);
    // Call 3 answered, call 4 left: nothing new.
    expect(newlyHeld(heldByWork(two), withWaits(142, [question(4)]).items)).toEqual([]);
  });
});

describe("the browser's notifications", () => {
  it('tell of each work that comes to wait for something new once, by its item and why, and open that card', () => {
    const draw = drawn(fixtureItems());
    expect(Told.made).toEqual([]);
    draw(withWaits(142, [question(9, 'Keep the width per view?')]));
    expect(Told.made.map((n) => [n.title, n.options.body, n.options.tag, n.options.renotify])).toEqual([
      ["Waiting for you: #142 Keep the panel's width", 'implement · Keep the width per view?', 'wf-agents-item-142', true],
    ]);
    draw(withWaits(142, [question(9, 'Keep the width per view?')]));
    expect(Told.made).toHaveLength(1);
    act(() => Told.made[0].onclick?.());
    expect(window.location.hash).toBe('#/console/items/142/waits/call-9');
    expect(Told.made[0].closed).toBe(true);
  });

  it('tell of each kind by its reason, the first line of what a question asks, the state said once in the title, and open its card', () => {
    const draw = drawn(fixtureItems());
    for (const w of KINDS) {
      draw(withWaits(142, [w]));
      draw(withWaits(142, []));
    }
    const ask = wait('answer-long', 'Which way?\n\nThe details.', { kind: 'answer', question: 'long', version: 2 });
    draw(withWaits(142, [ask]));
    // After the cell the work stands at, as a line names it: a name or a value in backticks is said bare, as a
    // notification has no monospace.
    const bare = KINDS.map((w) => `implement · ${w.why.replace(/`/g, '')}`);
    expect(bare[5]).toBe('implement · test = may_merge does not hold, as content says, and the cell requires it.');
    expect(Told.made.map((n) => n.options.body)).toEqual([...bare, 'implement · Which way?']);
    for (const n of Told.made) {
      expect(n.title).toBe("Waiting for you: #142 Keep the panel's width");
      expect(n.options.body).not.toContain('waits for you');
    }
    const opens = (w: Wait) => openingOf(withWaits(142, [w]).items.find((i) => i.number === 142) as Item)?.opens;
    for (const w of KINDS) expect(opens(w)).toBe(`#/console/items/142/waits/${w.key}`);
  });

  it('tell of a question the merge waits for by what it asks, at the cell the work waits at', () => {
    const draw = drawn(fixtureItems());
    const merge: Now = {
      kind: 'waits',
      at: 'M1',
      on: [{ kind: 'precondition', test: 'asked = answered', words: 'asked = answered', by: [{ kind: 'owner' }] }],
    };
    const asks: Wait = {
      ...wait('answer-scope', '选哪一种？', { kind: 'answer', question: 'scope', version: 4 }),
      under: 'The merge waits for this answer.',
    };
    const items = fixtureItems().items.map((i): Item =>
      i.number === 142 ? { ...i, work: { ...i.work, now: merge, waits: [asks] } } : i,
    );
    draw({ items, outage: null });
    expect(Told.made.map((n) => n.options.body)).toEqual(['M1 · 选哪一种？']);
  });

  it('tell nothing where the list holds nothing, whatever the work does', () => {
    const draw = drawn(fixtureItems());
    const at = (now: Now): Items => {
      const items = fixtureItems().items.map((i): Item => (i.number === 142 ? { ...i, work: { ...i.work, now, waits: [] } } : i));
      return { items, outage: null };
    };
    draw(at({ kind: 'stalled', at: 'M2', on: [], by: [], why: 'Nothing new runs `M2`.' }));
    draw(at({ kind: 'paused', by: 'owner', why: null }));
    draw(at({ kind: 'everything', by: 'owner' }));
    draw(at({ kind: 'fault', words: 'Two entries at 3' }));
    draw(at({ kind: 'waits', at: 'M1', on: [{ kind: 'outside', source: 'main', cell: 'M1', why: 'w' }] }));
    expect(Told.made).toEqual([]);
  });

  it('tell nothing while the owner looks at the page, or where the browser was given no leave', () => {
    const draw = drawn(fixtureItems());
    // The page is in sight, and has the focus.
    expect(document.visibilityState).toBe('visible');
    looking = true;
    draw(withWaits(142, [question(9)]));
    looking = false;
    Told.permission = 'denied';
    draw(withWaits(142, [question(9), question(10)]));
    expect(Told.made).toEqual([]);
  });

  it('tell again each hour how many things are left, while any is', () => {
    vi.useFakeTimers();
    const left = [...heldByWork(fixtureItems().items).values()].reduce((a, b) => a + b, 0);
    expect(left).toBeGreaterThan(0);
    drawn(fixtureItems());
    act(() => {
      vi.advanceTimersByTime(AGAIN_MS);
    });
    // In the words of the bar's count.
    expect(Told.made.map((n) => [n.title, n.options.tag, n.options.renotify])).toEqual([
      [waitCount(left), 'wf-agents-waits', true],
    ]);
    // A click opens the first of them, at its card.
    const first = firstOpening(fixtureItems().items);
    expect(first).toMatch(/^#\/console\/items\/\d+\/waits\//);
    act(() => Told.made[0].onclick?.());
    expect(window.location.hash).toBe(first);
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
    draw(withWaits(142, [question(9)]));
    act(() => {
      vi.advanceTimersByTime(AGAIN_MS);
    });
    expect([Told.asked, Told.made.length]).toEqual([0, 0]);
  });
});
