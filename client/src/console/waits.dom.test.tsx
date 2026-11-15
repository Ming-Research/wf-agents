// @vitest-environment happy-dom
import '../dom-setup';

import { act } from 'react';
import { type Root, createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { hashOf, routeOf } from '../route';
import { standingOf } from '../system/standing';
import { rank } from '../works/placing';
import { waitingCount, waitsOf, waitsOnOwner } from '../works/waiting';
import { STATE, nowTone, stateOf } from '../works/words';
import type { Flow } from '../api';
import type { Act, Block, EngineAct, Failure, Item, Now, Outage, Pauser, Question, Wait, WorkPage } from '../works-api';
import { FLOW, fixtureItems } from './fixtures';
import { asksOf } from './items';
import { ItemPanel } from './ItemWindow';
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

/// What the owner does about the agents' command line, in the engine's words, which the page says once.
const MEND = 'Sign it in again or install it; the engine lets the work go on once its check finds it runs.';

/// A stalled work: no cell can write what it waits for.
const STALLED: Now = {
  kind: 'stalled',
  at: 'merge',
  on: [{ test: 'checked = yes', words: 'checked = yes' }],
  by: [{ cell: 'check', why: { kind: 'nothingNew' } }],
};

/// A review's verdict that holds the work, for the owner to rule on.
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
    { at: 7, time: 0, by: { kind: 'run', run: 5, cell: 'review' }, meaning: 7, value: 'no', saidTo: null, chars: 0, empty: false },
  ],
};

/// The merge's check, which waits for the owner's answers: what the flow declares of `asked` makes it the owner's.
const ANSWERS: Now = {
  kind: 'waits',
  at: 'merge',
  on: [{ kind: 'precondition', test: 'asked = answered', words: 'asked = answered', by: [{ kind: 'owner' }] }],
};

const wait = (key: string, why: string, ends: Wait['ends']): Wait => ({ key, why, ends });

/// Each kind of thing the engine's list names, as the engine gives it with what the work does now.
const OWNERS: [string, Now, Wait][] = [
  [
    'a question put as a call',
    {
      kind: 'waits',
      at: 'plan',
      on: [{ kind: 'call', call: 9, who: 'owner', what: { kind: 'question', who: 'owner', question: 'Which?' } }],
    },
    wait('call-9', 'Which?', { kind: 'reply', call: 9 }),
  ],
  [
    'a run at its limit of returns',
    { kind: 'waits', at: 'plan', on: [{ kind: 'call', call: 4, who: 'owner', what: limit }] },
    wait('limit', 'The run of plan had 2 returns sent back, its limit.', { kind: 'goOn', call: 4 }),
  ],
  [
    'a cell at its limit of runs',
    { kind: 'waits', at: 'plan', on: [{ kind: 'limit', runs: 3 }] },
    wait('limit', 'plan ran 3 times in a row for what a cell after it wrote, its limit.', { kind: 'letGo', cell: 'plan' }),
  ],
  [
    'a pause the engine made',
    { kind: 'paused', by: 'engine', why: 'a round failed' },
    wait('pause', 'The engine paused the work: a round failed', { kind: 'resume' }),
  ],
  [
    'the works held after the engine started again',
    { kind: 'everything', by: 'engine' },
    wait('held', 'The engine paused every work as it started again.', { kind: 'resumeEverything' }),
  ],
  [
    'a question of the work that holds it at its cell, as the engine asks of a change of a protected file',
    { kind: 'waits', at: 'plan', on: [{ kind: 'answer', question: 'protected_files' }] },
    wait('answer-protected_files', 'What of protected_files?', {
      kind: 'answer',
      question: 'protected_files',
      version: 4,
    }),
  ],
  [
    'a question the work went on past, which the merge now waits for',
    ANSWERS,
    wait('answer-measure', 'What of measure?', { kind: 'answer', question: 'measure', version: 4 }),
  ],
  [
    "a review's verdict for the owner to rule on",
    VERDICT,
    wait('write-content', 'merge requires test = may_merge, which content keeps from holding.', {
      kind: 'write',
      block: 'content',
      value: 'yes',
    }),
  ],
  ["the agents' command line, which cannot run", shared('cannotRun'), wait('agents', "The agents' command line cannot run: Not logged in", { kind: 'outside', what: MEND })],
];

/// What the engine's list leaves out, which is not the owner's to end whatever the work does.
const NOT_OWNERS: [string, Now][] = [
  ['a pause of the owner', { kind: 'paused', by: 'owner', why: null }],
  ['a pause of everything by the owner', { kind: 'everything', by: 'owner' }],
  ['a fault', { kind: 'fault', words: 'Two entries at 3' }],
  ['a free slot', { kind: 'waits', at: 'plan', on: [{ kind: 'slot' }] }],
  ['a stalled work', STALLED],
  ['the wait every work shares for the usage limit, which the engine lifts', shared('usageLimit')],
  [
    'a check held for main to move',
    { kind: 'waits', at: 'M2', on: [{ kind: 'outside', source: 'main', cell: 'M1', why: 'What fails also fails on main.' }] },
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

/// The questions of the work's own its page gives with each of these waits.
const questionsOf = (waits: Wait[]): Question[] =>
  waits.flatMap((w) =>
    w.ends.kind === 'answer'
      ? [asked(w.ends.question, w.ends.question === 'measure' ? 'plan' : 'plan', w.ends.question !== 'measure')]
      : [],
  );

function draw(
  now: Now,
  waits: Wait[],
  opens: string | undefined,
  send: (w: number, a: Act) => Promise<unknown>,
  o: { engine?: (a: EngineAct) => Promise<unknown>; paused?: Pauser; read?: { flow: Flow; blocks: Block[]; load?: LoadVersion } } = {},
) {
  const read = o.read ?? { flow: FLOW, blocks: [] };
  const item: Item = { ...base, work: { ...base.work, now, waits, paused: o.paused ?? null } };
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
    questions: questionsOf(waits),
  };
  act(() =>
    r.render(
      <ItemPanel
        item={item}
        page={{ page }}
        act={send}
        waitsFor={[]}
        onTitle={() => undefined}
        wait={opens}
        actEngine={o.engine}
        loadVersion={read.load}
      />,
    ),
  );
  return container;
}

/// The work's status line in the panel, under the item's.
const workLine = (el: HTMLElement): HTMLElement => el.querySelectorAll('.cw-item > .status-line')[1] as HTMLElement;

const unmount = () => {
  act(() => root?.unmount());
  root = undefined;
  document.body.innerHTML = '';
};

const none = () => Promise.resolve(null);

/// Words a card says around what it is about, which no card of a stop says.
const FILLER = [
  'The work waits for you',
  'The merge waits for this answer',
  'Still to confirm',
  'The work waits for this ruling',
];

describe("what waits for the owner, as the engine's one list names it", () => {
  it.each(OWNERS)('is counted, marked, carded once in the look of a stop and said on the state line alike: %s', async (_, now, w) => {
    const summary = { ...base.work, now, waits: [w] };
    expect(waitsOf(summary)).toEqual([w]);
    expect(waitsOnOwner(summary)).toBe(true);
    expect(asksOf(summary)).toBe(1);
    expect(waitingCount([summary, { waits: [] }])).toBe(1);
    expect(moodOf({ now, cells: [], waits: [w] })).toBe('owner');
    const container = draw(now, [w], w.key, none, { read: { flow: RULED, blocks: [SAID_NO] } });
    // A ruling is read with the body, once the page has drawn.
    await act(async () => undefined);
    const cards = container.querySelectorAll(`[data-wait="${w.key}"]`);
    expect(cards).toHaveLength(1);
    expect(cards[0].classList.contains('cw-choice-holds')).toBe(true);
    for (const filler of FILLER) expect(container.textContent).not.toContain(filler);
    expect(workLine(container).querySelector('.status-word')?.textContent).toBe(STATE.owner);
  });

  it.each(NOT_OWNERS)('is none where the list leaves it out: %s', (_, now) => {
    const summary = { ...base.work, now, waits: [] };
    expect(waitsOnOwner(summary)).toBe(false);
    expect(asksOf(summary)).toBe(0);
    expect(moodOf({ now, cells: [], waits: [] })).not.toBe('owner');
    const container = draw(now, [], undefined, none);
    expect(container.querySelectorAll('[data-wait]')).toHaveLength(0);
    expect(workLine(container).querySelector('.status-word')?.textContent).not.toBe(STATE.owner);
  });

  it("reads the list alone: a work that does what would wait on the owner, with nothing in the list, counts nothing", () => {
    const [, paused] = OWNERS[3];
    const summary = { ...base.work, now: paused, waits: [] };
    expect(waitsOnOwner(summary)).toBe(false);
    expect(asksOf(summary)).toBe(0);
    expect(rank(summary)).not.toBe(0);
    expect(standingOf(summary)).not.toBe('owner');
  });
});

describe("the work's state line", () => {
  it('is one line: the word of its state from the one catalogue, the cell where the state names one, and the work, with no keys under it', () => {
    const said = (now: Now, waits: Wait[] = []) => {
      const line = workLine(draw(now, waits, undefined, none));
      const word = line.querySelector('.status-word')?.textContent;
      const cell = line.querySelector('.cell-id')?.textContent ?? null;
      const keys = line.querySelector('.cw-keys, .keys');
      unmount();
      return { word, cell, keys };
    };
    expect(said({ kind: 'fault', words: 'Two entries at 3' })).toEqual({ word: 'Halted', cell: null, keys: null });
    expect(said(STALLED)).toEqual({ word: 'Halted', cell: 'merge', keys: null });
    expect(said({ kind: 'paused', by: 'owner', why: null })).toEqual({ word: 'Paused', cell: null, keys: null });
    expect(said({ kind: 'everything', by: 'owner' })).toEqual({ word: 'Paused', cell: null, keys: null });
    expect(said(OWNERS[3][1], [OWNERS[3][2]])).toEqual({ word: 'Waiting for you', cell: null, keys: null });
    expect(said(OWNERS[2][1], [OWNERS[2][2]])).toEqual({ word: 'Waiting for you', cell: 'plan', keys: null });
    expect(said({ kind: 'waits', at: 'M2', on: [{ kind: 'turn', holder: 12, since: 0 }] })).toEqual({
      word: 'Waiting for W12',
      cell: 'M2',
      keys: null,
    });
    expect(said({ kind: 'round', cell: 'plan', run: 1, round: 2, since: 0 })).toEqual({
      word: 'Running',
      cell: 'plan',
      keys: null,
    });
  });
});

describe('a stop that no one waits on', () => {
  it('is a card in the same look, saying what is wrong, for a fault and for a stall, and counts nothing', () => {
    for (const [now, why] of [
      [{ kind: 'fault', words: 'Two entries at 3' } as Now, 'Two entries at 3'],
      [STALLED, 'it waits for checked = yes, which only cell check can make hold; cell check does not run: nothing new runs it'],
    ] as const) {
      const container = draw(now, [], undefined, none);
      const card = container.querySelector('.cw-waits .cw-choice-holds');
      expect(card?.textContent).toBe(why);
      expect(card?.querySelector('button')).toBeNull();
      unmount();
    }
    const fault: Now = { kind: 'fault', words: 'Two entries at 3' };
    const summary = (now: Now) => ({ ...base.work, now, waits: [] });
    expect(moodOf({ now: STALLED, cells: [], waits: [] })).toBe(moodOf({ now: fault, cells: [], waits: [] }));
    expect(nowTone(summary(STALLED))).toBe(nowTone(summary(fault)));
    expect(rank(summary(STALLED))).toBe(rank(summary(fault)));
    expect(standingOf(summary(STALLED))).toBe('halted');
    expect(stateOf(summary(STALLED)).word).toBe(stateOf(summary(fault)).word);
  });
});

describe('the cards of what waits for the owner', () => {
  it("say the agents' command line's reason and what the owner does, each once, inside the card", () => {
    const [, now, w] = OWNERS[8];
    const container = draw(now, [w], undefined, none);
    const card = container.querySelector('[data-wait="agents"]');
    expect(card?.textContent).toBe(`The agents' command line cannot run: Not logged in${MEND}`);
    expect(container.textContent?.split('Sign it in again').length).toBe(2);
  });

  it('end it from a button inside the card, which says on pointing at it what it does', () => {
    const sent: Act[] = [];
    const engine: EngineAct[] = [];
    const send = (_: number, a: Act) => {
      sent.push(a);
      return Promise.resolve(null);
    };
    const press = (el: HTMLElement, key: string) => {
      const button = el.querySelector(`[data-wait="${key}"] button`) as HTMLButtonElement;
      expect(button.title).not.toBe('');
      act(() => button.click());
    };
    for (const i of [1, 2, 3]) {
      const [, now, w] = OWNERS[i];
      press(draw(now, [w], w.key, send), w.key);
      unmount();
    }
    expect(sent).toEqual([
      { kind: 'result', call: 4, result: 'go on' },
      { kind: 'letGo', cell: 'plan' },
      { kind: 'resume' },
    ]);
    const [, held, w] = OWNERS[4];
    press(
      draw(held, [w], 'held', send, {
        engine: (a) => {
          engine.push(a);
          return Promise.resolve(null);
        },
      }),
      'held',
    );
    expect(engine).toEqual([{ kind: 'resume' }]);
  });

  it("let the engine's pause on from its card alone: the pause switch shows only the owner's own pause", () => {
    const pressed = (el: HTMLElement) => el.querySelector('.cw-acts [aria-pressed="true"], .cw-acts [aria-checked="true"]')?.textContent;
    const [, now, w] = OWNERS[3];
    expect(pressed(draw(now, [w], undefined, none, { paused: 'engine' }))).toBe('Start');
    unmount();
    expect(pressed(draw({ kind: 'paused', by: 'owner', why: null }, [], undefined, none, { paused: 'owner' }))).toBe('Pause');
  });

  it('offer a ruling\'s values as options drawn as choices, the one that makes it hold picked, and write the one picked', async () => {
    const sent: Act[] = [];
    const [, now, w] = OWNERS[7];
    const container = draw(
      now,
      [w],
      undefined,
      (_, a) => {
        sent.push(a);
        return Promise.resolve(null);
      },
      { read: { flow: RULED, blocks: [SAID_NO] } },
    );
    await act(async () => undefined);
    const card = container.querySelector('[data-wait="write-content"]') as HTMLElement;
    const options = [...card.querySelectorAll('.cw-options .cw-option')];
    expect(options.map((o) => o.textContent)).toEqual(['content = yes']);
    expect(options[0].querySelector('.cw-radio')).not.toBeNull();
    expect(options[0].getAttribute('aria-checked')).toBe('true');
    act(() => (card.querySelector('.cw-stop-acts button') as HTMLButtonElement).click());
    expect(sent).toEqual([{ kind: 'write', block: 'content', content: '', value: 'yes' }]);
  });

  it('are shown at their card by the address a notification opens: the item and the thing waited for', async () => {
    for (const [, now, w] of OWNERS) {
      const hash = hashOf({ view: 'console', item: 12, wait: w.key });
      expect(routeOf(hash)).toEqual({ view: 'console', item: 12, wait: w.key });
      const scrolled = vi.fn();
      Element.prototype.scrollIntoView = scrolled;
      const container = draw(now, [w], w.key, none, { read: { flow: RULED, blocks: [SAID_NO] } });
      await act(async () => undefined);
      expect(scrolled).toHaveBeenCalled();
      expect(scrolled.mock.contexts[0]).toBe(container.querySelector(`[data-wait="${w.key}"]`));
      unmount();
    }
  });
});
