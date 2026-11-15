// @vitest-environment happy-dom
import '../dom-setup';

import { act } from 'react';
import { type Root, createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { hashOf, routeOf } from '../route';
import { waitsOf, waitsOnOwner } from '../works/waiting';
import type { Act, EngineAct, Item, Now, WorkPage } from '../works-api';
import { FLOW, fixtureItems } from './fixtures';
import { asksOf } from './items';
import { ItemPanel } from './ItemWindow';
import { moodOf } from './snap';

let root: Root | undefined;
afterEach(() => {
  if (root !== undefined) act(() => root?.unmount());
  root = undefined;
  document.body.innerHTML = '';
  localStorage.clear();
  vi.restoreAllMocks();
});

const base = fixtureItems().items.find((i) => i.number === 142) as Item;
const limit = { kind: 'atLimit' as const, limit: { of: 'returns' as const, sentBack: 2 } };

/// Each thing that waits for the owner, as the work's `now` says it, and the key its card is known by.
const OWNERS: [string, Now, string][] = [
  [
    'a question that holds a work',
    {
      kind: 'waits',
      at: 'plan',
      on: [{ kind: 'call', call: 9, who: 'owner', what: { kind: 'question', who: 'owner', question: 'Which?' } }],
    },
    'q9',
  ],
  [
    'a cell at its limit, as a call',
    { kind: 'waits', at: 'plan', on: [{ kind: 'call', call: 4, who: 'owner', what: limit }] },
    'limit',
  ],
  ['a cell at its limit, as runs', { kind: 'waits', at: 'plan', on: [{ kind: 'limit', runs: 3 }] }, 'limit'],
  ['a pause the engine made', { kind: 'paused', by: 'engine', why: 'A round failed.' }, 'pause'],
  ['a work held after the engine started again', { kind: 'everything', by: 'engine' }, 'held'],
];

/// What is not the owner's to let on.
const NOT_OWNERS: [string, Now][] = [
  ['a pause of the owner', { kind: 'paused', by: 'owner', why: null }],
  ['a pause of everything by the owner', { kind: 'everything', by: 'owner' }],
  ['a fault', { kind: 'fault', words: 'Two entries at 3' }],
  ['a free slot', { kind: 'waits', at: 'plan', on: [{ kind: 'slot' }] }],
  [
    'a question to another',
    {
      kind: 'waits',
      at: 'plan',
      on: [{ kind: 'call', call: 2, who: 'architect', what: { kind: 'question', who: 'architect', question: 'Where?' } }],
    },
  ],
];

function draw(
  now: Now,
  wait: string | undefined,
  send: (w: number, a: Act) => Promise<unknown>,
  engine?: (a: EngineAct) => Promise<unknown>,
) {
  const item: Item = { ...base, work: { ...base.work, now } };
  const container = document.createElement('div');
  document.body.append(container);
  root = createRoot(container);
  const r = root;
  const page: WorkPage = {
    summary: item.work,
    title: item.title,
    level: item.level,
    project: item.project,
    history: item.history,
    flow: FLOW,
    cells: [],
    blocks: [],
    steps: [],
  };
  act(() =>
    r.render(
      <ItemPanel
        item={item}
        page={{ page }}
        act={send}
        waitsFor={[]}
        onTitle={() => undefined}
        wait={wait}
        actEngine={engine}
      />,
    ),
  );
  return container;
}

const unmount = () => {
  act(() => root?.unmount());
  root = undefined;
  document.body.innerHTML = '';
};

const none = () => Promise.resolve(null);

describe('what waits for the owner', () => {
  it.each(OWNERS)('is counted, marked and carded alike: %s', (_, now, key) => {
    const waits = waitsOf(now);
    expect(waits.map((w) => w.key)).toEqual([key]);
    expect(waitsOnOwner(now)).toBe(true);
    expect(asksOf(now)).toBe(waits.length);
    expect(moodOf({ now })).toBe('owner');
    const container = draw(now, key, none);
    expect(container.querySelectorAll(`[data-wait="${key}"]`)).toHaveLength(1);
  });

  it.each(NOT_OWNERS)("is none of the three where it is not the owner's to let on: %s", (_, now) => {
    expect(waitsOf(now)).toEqual([]);
    expect(waitsOnOwner(now)).toBe(false);
    expect(asksOf(now)).toBe(0);
    expect(moodOf({ now })).not.toBe('owner');
    const container = draw(now, undefined, none);
    expect(container.querySelectorAll('[data-wait]')).toHaveLength(0);
  });

  it("is let on from its card: the limit let go, the engine's pause resumed, the held works started", () => {
    const sent: Act[] = [];
    const engine: EngineAct[] = [];
    const send = (_: number, a: Act) => {
      sent.push(a);
      return Promise.resolve(null);
    };
    const press = (el: HTMLElement, key: string) =>
      act(() =>
        (el.querySelector(`[data-wait="${key}"]`)?.parentElement?.querySelector('button') as HTMLButtonElement).click(),
      );
    press(draw(OWNERS[2][1], 'limit', send), 'limit');
    expect(sent).toEqual([{ kind: 'letGo', cell: 'plan' }]);
    unmount();
    press(draw(OWNERS[3][1], 'pause', send), 'pause');
    expect(sent[1]).toEqual({ kind: 'resume' });
    unmount();
    const held = draw(OWNERS[4][1], 'held', send, (a) => {
      engine.push(a);
      return Promise.resolve(null);
    });
    press(held, 'held');
    expect(engine).toEqual([{ kind: 'resume' }]);
  });

  it('is shown at its card by the address a notification opens: the item and the thing waited for', () => {
    for (const [, now, key] of OWNERS) {
      const [wait] = waitsOf(now);
      const hash = hashOf({ view: 'console', item: 142, wait: wait.key });
      expect(routeOf(hash)).toEqual({ view: 'console', item: 142, wait: key });
      const scrolled = vi.fn();
      Element.prototype.scrollIntoView = scrolled;
      const container = draw(now, key, none);
      expect(scrolled).toHaveBeenCalled();
      expect(scrolled.mock.contexts[0]).toBe(container.querySelector(`[data-wait="${key}"]`));
      unmount();
    }
  });
});
