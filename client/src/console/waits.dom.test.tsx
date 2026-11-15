// @vitest-environment happy-dom
import '../dom-setup';

import { act } from 'react';
import { type Root, createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { hashOf, routeOf } from '../route';
import { standingOf } from '../system/standing';
import { rank } from '../works/placing';
import { waitsOf, waitsOnOwner } from '../works/waiting';
import { nowTone } from '../works/words';
import type { Flow } from '../api';
import type { Act, Block, EngineAct, Failure, Item, Now, Outage, Question, WorkPage } from '../works-api';
import { FLOW, fixtureItems } from './fixtures';
import { asksOf } from './items';
import { ItemPanel, stateOf } from './ItemWindow';
import type { LoadVersion } from './real';
import { moodOf } from './snap';

let root: Root | undefined;
afterEach(() => {
  if (root !== undefined) act(() => root?.unmount());
  root = undefined;
  document.body.innerHTML = '';
  localStorage.clear();
  vi.restoreAllMocks();
});

const base = fixtureItems().items.find((i) => i.number === 12) as Item;
const limit = { kind: 'atLimit' as const, limit: { of: 'returns' as const, sentBack: 2 } };

/// A work held by the wait every work shares, of this cause.
const shared = (failure: Failure): Now => {
  const outage: Outage = {
    service: 'agents',
    failure,
    why: 'Not logged in',
    work: 3,
    run: 2,
    round: 1,
    since: 0,
    checked: null,
    until: { kind: 'checked', at: 0 },
    holds: true,
  };
  return { kind: 'waits', at: 'plan', on: [{ kind: 'agents', outage }] };
};

/// Every cause of the wait every work shares but the command line that cannot run.
const LIFTED: Failure[] = [
  'usageLimit',
  'offline',
  'unavailable',
  'passing',
  'unread',
  'flagged',
  'guarded',
  'promptTooLong',
  'noModel',
  'other',
];

/// A work at the merge's check, which waits for the owner's answers to the questions that stand: a precondition only the
/// engine's source of how they stand makes hold.
const ANSWERS: Now = {
  kind: 'waits',
  at: 'merge',
  on: [{ kind: 'precondition', test: 'asked', words: 'asked = answered', by: [{ kind: 'outside', source: 'asked' }] }],
};

/// A stalled work: no cell can write what it waits for.
const STALLED: Now = {
  kind: 'stalled',
  at: 'merge',
  on: [{ test: 'checked = yes', words: 'checked = yes' }],
  by: [{ cell: 'check', why: { kind: 'nothingNew' } }],
};

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
    'a run at its limit of returns',
    { kind: 'waits', at: 'plan', on: [{ kind: 'call', call: 4, who: 'owner', what: limit }] },
    'limit',
  ],
  ['a cell at its limit of runs', { kind: 'waits', at: 'plan', on: [{ kind: 'limit', runs: 3 }] }, 'limit'],
  ['a pause the engine made', { kind: 'paused', by: 'engine', why: 'A round failed.' }, 'pause'],
  ['a work held after the engine started again', { kind: 'everything', by: 'engine' }, 'held'],
  [
    'a question the cell asked as data, as the engine asks of a change of a protected file',
    { kind: 'waits', at: 'plan', on: [{ kind: 'answer', question: 'protected_files' }] },
    'answer-protected_files',
  ],
  ["the agents' command line, which cannot run", shared('cannotRun'), 'agents'],
  ['a question that did not stop its cell, which the merge now waits for', ANSWERS, 'asked'],
];

/// A review's verdict that holds the work, for the owner to rule on: the review said it is not content, and the owner,
/// who may write the verdict as the review may, rules on it at the card of the panel's body that shows it.
const VERDICT: Now = {
  kind: 'waits',
  at: 'merge',
  on: [
    {
      kind: 'precondition',
      test: 'may_merge',
      words: 'The reviews are content and the owner lets it merge',
      by: [{ kind: 'owner' }, { kind: 'cell', cell: 'review' }],
    },
  ],
};
const RULED: Flow = {
  ...FLOW,
  blocks: FLOW.blocks.map((b) => (b.name === 'content' ? { ...b, writtenBy: { ...b.writtenBy, people: true } } : b)),
};
const SAID_NO: Block = {
  name: 'content',
  versions: [
    { at: 7, time: 0, by: { kind: 'run', run: 5, cell: 'review' }, meaning: 7, value: 'no', saidTo: null, chars: 0 },
  ],
};

/// What is not the owner's to let on.
const NOT_OWNERS: [string, Now][] = [
  ['a pause of the owner', { kind: 'paused', by: 'owner', why: null }],
  ['a pause of everything by the owner', { kind: 'everything', by: 'owner' }],
  ['a fault', { kind: 'fault', words: 'Two entries at 3' }],
  ['a free slot', { kind: 'waits', at: 'plan', on: [{ kind: 'slot' }] }],
  ['a stalled work, which is shown as a fault is', STALLED],
  // Every other cause of the wait every work shares is the engine's, which checks the outside itself and lifts it.
  ...LIFTED.map((failure): [string, Now] => [`the wait every work shares, for ${failure}`, shared(failure)]),
  [
    'a check held for main to move',
    {
      kind: 'waits',
      at: 'M2',
      on: [{ kind: 'outside', source: 'main', cell: 'M1', why: 'What fails also fails on main.' }],
    },
  ],
  [
    'a question to another',
    {
      kind: 'waits',
      at: 'plan',
      on: [{ kind: 'call', call: 2, who: 'architect', what: { kind: 'question', who: 'architect', question: 'Where?' } }],
    },
  ],
];

/// A question of the work's own, unanswered, as its page gives it.
const asked = (name: string, cell: string, waits: boolean): Question => ({
  name,
  cell,
  version: 4,
  engine: false,
  waits,
  withdrawn: null,
  ask: `What of ${name}?`,
  options: [{ does: 'Keep it', follows: 'Nothing changes' }],
  recommends: waits ? null : 1,
  answer: null,
});

/// The questions of the work's own it waits for the answer to, as its page gives them: each it waits for at its cell,
/// and one it went on past while the merge waits for the owner's answers.
const questionsOf = (now: Now): Question[] =>
  now.kind !== 'waits'
    ? []
    : now.on.flatMap((w): Question[] =>
        w.kind === 'answer'
          ? [asked(w.question, now.at, true)]
          : w.kind === 'precondition' && w.by.some((m) => m.kind === 'outside' && m.source === 'asked')
            ? [asked('measure', 'plan', false)]
            : [],
      );

function draw(
  now: Now,
  wait: string | undefined,
  send: (w: number, a: Act) => Promise<unknown>,
  engine?: (a: EngineAct) => Promise<unknown>,
  read: { flow: Flow; blocks: Block[]; load?: LoadVersion } = { flow: FLOW, blocks: [] },
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
    flow: read.flow,
    cells: [],
    blocks: read.blocks,
    steps: [],
    questions: questionsOf(now),
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
        loadVersion={read.load}
      />,
    ),
  );
  return container;
}

/// The work's status line in the panel, under the item's: its word, and what follows it.
function workLine(el: HTMLElement): { word: string; rest: string } {
  const line = el.querySelectorAll('.cw-item > .status-line')[1];
  const word = line.querySelector('.status-word')?.textContent ?? '';
  return { word, rest: line.textContent?.slice(word.length) ?? '' };
}

const unmount = () => {
  act(() => root?.unmount());
  root = undefined;
  document.body.innerHTML = '';
};

const none = () => Promise.resolve(null);

describe('what waits for the owner', () => {
  it.each(OWNERS)('is counted, marked, carded and said on the status line alike: %s', (_, now, key) => {
    const waits = waitsOf(now);
    expect(waits.map((w) => w.key)).toEqual([key]);
    expect(waitsOnOwner(now)).toBe(true);
    expect(asksOf(now)).toBe(waits.length);
    expect(moodOf({ now, cells: [] })).toBe('owner');
    const container = draw(now, key, none);
    expect(container.querySelectorAll(`[data-wait="${key}"]`)).toHaveLength(1);
    // The state line says that the work waits for you, as the card does.
    expect(workLine(container).word).toBe('Waiting for you');
  });

  it("says on the state line by its keys what a pause of the engine's and its hold wait for", () => {
    const keys = (now: Now) => {
      const container = draw(now, undefined, none);
      const said = [...container.querySelectorAll('.cw-item > .status-line .keys > span')].map((k) => k.textContent);
      unmount();
      return said;
    };
    expect(keys({ kind: 'paused', by: 'engine', why: 'A round failed.' })).toEqual([
      'paused by the engine',
      'why A round failed.',
    ]);
    expect(keys({ kind: 'everything', by: 'engine' })).toEqual([
      'paused every work',
      'by the engine, as it started again',
    ]);
  });

  it("is counted, marked and carded alike where it is a review's verdict that holds the work, for the owner to rule on", async () => {
    const waits = waitsOf(VERDICT);
    expect(waits.map((w) => w.key)).toEqual(['precondition']);
    expect(waitsOnOwner(VERDICT)).toBe(true);
    expect(asksOf(VERDICT)).toBe(1);
    expect(moodOf({ now: VERDICT, cells: [] })).toBe('owner');
    const scrolled = vi.fn();
    Element.prototype.scrollIntoView = scrolled;
    const container = draw(VERDICT, 'precondition', none, undefined, { flow: RULED, blocks: [SAID_NO] });
    // The ruling is read with the body, once the page has drawn.
    await act(async () => undefined);
    const card = container.querySelector('[data-wait="precondition"]');
    // It holds the work, as a question that blocks does: on the red tint, edged in red, with no line to say so.
    expect(card?.className).toBe('cw-choice cw-choice-holds');
    expect(card?.querySelector('.cw-choice-stops')).toBeNull();
    expect(scrolled.mock.contexts[0]).toBe(card);
    expect(workLine(container).word).toBe('Waiting for you');
  });

  it.each(NOT_OWNERS)("is none of the three where it is not the owner's to let on: %s", (_, now) => {
    expect(waitsOf(now)).toEqual([]);
    expect(waitsOnOwner(now)).toBe(false);
    expect(asksOf(now)).toBe(0);
    expect(moodOf({ now, cells: [] })).not.toBe('owner');
    const container = draw(now, undefined, none);
    expect(container.querySelectorAll('[data-wait]')).toHaveLength(0);
    expect(workLine(container).word).not.toBe('Waiting for you');
  });

  it("says a fault and a stalled work halted by the engine, and the owner's own pause paused, on the status line", () => {
    const word = (now: Now) => {
      const container = draw(now, undefined, none);
      const said = workLine(container).word;
      unmount();
      return said;
    };
    expect(word({ kind: 'fault', words: 'Two entries at 3' })).toBe('Halted by the engine');
    expect(word(STALLED)).toBe('Halted by the engine');
    expect(word({ kind: 'paused', by: 'owner', why: null })).toBe('Paused');
    expect(word({ kind: 'everything', by: 'owner' })).toBe('Paused');
  });

  it("says on the card of the agents' command line what the owner is to do", () => {
    const container = draw(shared('cannotRun'), undefined, none);
    expect(container.querySelector('[data-wait="agents"]')?.textContent).toBe(
      "The work waits for you: the agents' command line cannot run (Not logged in): sign it in again or install it, and the engine lets the work go on once its own check finds it runs",
    );
  });

  it('shows a stalled work as a fault is: its tone, its standing and its place on each view', () => {
    const fault: Now = { kind: 'fault', words: 'Two entries at 3' };
    const summary = (now: Now) => ({ ...base.work, now });
    expect(moodOf({ now: STALLED, cells: [] })).toBe(moodOf({ now: fault, cells: [] }));
    expect(nowTone(STALLED)).toBe(nowTone(fault));
    expect(rank(summary(STALLED))).toBe(rank(summary(fault)));
    expect(standingOf(summary(STALLED))).toBe('halted');
    expect(stateOf(summary(STALLED), null).word).toBe(stateOf(summary(fault), null).word);
    expect(stateOf(summary(STALLED), null).tone).toBe(stateOf(summary(fault), null).tone);
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
      const hash = hashOf({ view: 'console', item: 12, wait: wait.key });
      expect(routeOf(hash)).toEqual({ view: 'console', item: 12, wait: key });
      const scrolled = vi.fn();
      Element.prototype.scrollIntoView = scrolled;
      const container = draw(now, key, none);
      expect(scrolled).toHaveBeenCalled();
      expect(scrolled.mock.contexts[0]).toBe(container.querySelector(`[data-wait="${key}"]`));
      unmount();
    }
  });
});
