// @vitest-environment happy-dom
import '../dom-setup';

import { act } from 'react';
import { type Root, createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { hashOf, routeOf } from '../route';
import { standingOf } from '../system/standing';
import { outlineOf } from '../testing';
import { orderWorks } from '../works/placing';
import { waitingCount, waitsOf, waitsOnOwner } from '../works/waiting';
import { STATE, nowTone, stateOf } from '../works/words';
import type { Flow } from '../api';
import type { Act, Block, Failure, Item, Now, Outage, Pauser, Question, Step, Summary, Wait, WorkPage } from '../works-api';
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

/// The works of these summaries, numbered in their order, as the list of works orders them by its column of states.
const byState = (works: Summary[]): number[] =>
  orderWorks(
    works.map((w, i) => ({ ...base, number: i + 1, work: { ...w, number: i + 1, last: 0 } })),
    { by: 'state', turned: false },
  ).map((w) => w.work.number);
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
  why: 'It waits for `checked = yes`, which only `check` can make hold, and `check` does not run, because nothing new runs it.',
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
      by: [{ kind: 'owner' }, { kind: 'cell', cell: 'review_code' }],
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
    { at: 7, number: 1, time: 0, by: { kind: 'run', run: 5, cell: 'review_code' }, meaning: 1, value: 'no', saidTo: null, chars: 0, empty: false },
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
    'a run at its limit of returns',
    { kind: 'waits', at: 'plan', on: [{ kind: 'call', call: 4, who: 'owner', what: limit }] },
    wait('limit', "The cell's run had 2 returns sent back, which is its limit.", { kind: 'goOn', call: 4 }),
  ],
  [
    'a cell at its limit of runs',
    { kind: 'waits', at: 'plan', on: [{ kind: 'limit', runs: 3 }] },
    wait('limit', 'The cell ran 3 times in a row for what a cell after it wrote, which is its limit.', { kind: 'letGo', cell: 'plan' }),
  ],
  [
    'a pause the engine made',
    { kind: 'paused', at: 'plan', by: 'engine', why: 'a round failed' },
    wait('pause', 'The engine paused the work. A round failed.', { kind: 'resume' }),
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
    wait('write-content', '`content` keeps `test = may_merge` from holding.', {
      kind: 'write',
      block: 'content',
      value: 'yes',
    }),
  ],
  [
    "the agents' command line, which cannot run",
    shared('cannotRun'),
    wait('agents', "The agents' command line cannot run. Not logged in.", { kind: 'outside', what: MEND }),
  ],
];

/// What the engine's list leaves out, which is not the owner's to end whatever the work does.
const NOT_OWNERS: [string, Now][] = [
  ['a pause of the owner', { kind: 'paused', at: 'plan', by: 'owner', why: null }],
  ['a pause of everything by the owner', { kind: 'everything', at: 'plan', by: 'owner' }],
  ['a fault', { kind: 'fault', words: 'Two entries at 3' }],
  ['a free slot', { kind: 'waits', at: 'plan', on: [{ kind: 'slot' }] }],
  ['a stalled work', STALLED],
  ['the wait every work shares for the usage limit, which the engine lifts', shared('usageLimit')],
  [
    'a check held for main to move',
    { kind: 'waits', at: 'M2', on: [{ kind: 'outside', source: 'main', cell: 'M1', why: 'What fails also fails on main.' }] },
  ],
  [
    'a tool the engine runs',
    { kind: 'waits', at: 'plan', on: [{ kind: 'call', call: 2, who: null, what: { kind: 'tool', name: 'ci', input: 'all' } }] },
  ],
];

/// A question of the work's own, unanswered, as its page gives it.
const asked = (name: string, cell: string, waits: boolean): Question => ({
  name,
  cell,
  version: 4,
  engine: false,
  waits,
  tree: false,
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
  o: {
    paused?: Pauser;
    steps?: Step[];
    read?: { flow: Flow; blocks: Block[]; load?: LoadVersion };
  } = {},
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
    outline: outlineOf(read.flow),
    flow: read.flow,
    cells: [],
    blocks: read.blocks,
    steps: o.steps ?? [],
    questions: questionsOf(waits),
  };
  act(() =>
    r.render(
      <ItemPanel
        item={item}
        page={{ page }}
        act={send}
        onTitle={() => undefined}
        wait={opens}
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

/// Words a card says around what it is about, which no card of a stop says: a card says the engine's one sentence of
/// why, and nothing of the page's own.
const FILLER = ['The work waits for you', 'Still to confirm', 'The work waits for this ruling'];

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
    const [, paused, pause] = OWNERS.find(([what]) => what === 'a pause the engine made') as [string, Now, Wait];
    const summary = { ...base.work, now: paused, waits: [] };
    expect(waitsOnOwner(summary)).toBe(false);
    expect(asksOf(summary)).toBe(0);
    // In the list's column of states it stands after a work the list says waits for the owner.
    const owner = { ...base.work, waits: [pause] };
    expect(byState([summary, owner])).toEqual([2, 1]);
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
    expect(said({ kind: 'paused', at: 'plan', by: 'owner', why: null })).toEqual({
      word: 'Paused',
      cell: 'plan',
      keys: null,
    });
    expect(said({ kind: 'everything', at: 'plan', by: 'owner' })).toEqual({ word: 'Paused', cell: 'plan', keys: null });
    expect(said(OWNERS[2][1], [OWNERS[2][2]])).toEqual({ word: 'Waiting for you', cell: 'plan', keys: null });
    expect(said(OWNERS[1][1], [OWNERS[1][2]])).toEqual({ word: 'Waiting for you', cell: 'plan', keys: null });
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

describe('the pause of every work', () => {
  it('is no card of a work it holds, and no wait of its own: the bar says it and lifts it', () => {
    const container = draw({ kind: 'everything', at: 'plan', by: 'engine' }, [], undefined, none);
    expect(container.querySelector('.cw-waits')).toBeNull();
    expect(container.textContent).not.toContain('Resume all');
  });
});

describe('a stop that no one waits on', () => {
  it('is a card in the same look, saying what is wrong, for a fault and for a stall, and counts nothing', () => {
    for (const [now, why] of [
      [{ kind: 'fault', words: 'Two entries at 3' } as Now, 'Two entries at 3'],
      [STALLED, 'It waits for checked = yes, which only check can make hold, and check does not run, because nothing new runs it.'],
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
    // In the list's column of states the two stand together, after what waits for the owner and before what runs.
    const running = summary({ kind: 'round', cell: 'plan', run: 1, round: 1, since: 0 });
    expect(byState([running, summary(STALLED), summary(fault), { ...base.work, waits: [OWNERS[2][2]] }])).toEqual([
      4, 3, 2, 1,
    ]);
    expect(standingOf(summary(STALLED))).toBe('halted');
    expect(stateOf(summary(STALLED)).word).toBe(stateOf(summary(fault)).word);
  });
});

describe('the cards of what waits for the owner', () => {
  it("say the agents' command line's reason and what the owner does, each once, inside the card", () => {
    const [, now, w] = OWNERS[6];
    const container = draw(now, [w], undefined, none);
    const card = container.querySelector('[data-wait="agents"]');
    expect(card?.textContent).toBe(`The agents' command line cannot run. Not logged in.${MEND}`);
    expect(container.textContent?.split('Sign it in again').length).toBe(2);
  });

  it('end it from a button inside the card, which says on pointing at it what it does', () => {
    const sent: Act[] = [];
    const send = (_: number, a: Act) => {
      sent.push(a);
      return Promise.resolve(null);
    };
    const press = (el: HTMLElement, key: string) => {
      const button = el.querySelector(`[data-wait="${key}"] button`) as HTMLButtonElement;
      expect(button.title).not.toBe('');
      act(() => button.click());
    };
    const labels: (string | null | undefined)[] = [];
    for (const i of [0, 1, 2]) {
      const [, now, w] = OWNERS[i];
      const el = draw(now, [w], w.key, send);
      labels.push(el.querySelector(`[data-wait="${w.key}"] button`)?.textContent);
      press(el, w.key);
      unmount();
    }
    // The engine's pause is lifted by Resume, which no switch of the owner's own pause is named.
    expect(labels).toEqual(['Let it go on', 'Let it go on', 'Resume']);
    expect(sent).toEqual([
      { kind: 'result', call: 4, result: 'go on' },
      { kind: 'letGo', cell: 'plan' },
      { kind: 'resume' },
    ]);
  });

  it('say a question the merge waits for by what it asks and nothing more, as at its cell', async () => {
    const [, now, w] = OWNERS[4];
    const container = draw(now, [w], undefined, none);
    // A question is drawn once the texts that may point to it are read.
    await act(async () => undefined);
    const card = container.querySelector('[data-wait="answer-measure"]') as HTMLElement;
    expect(card.querySelector('.cw-choice-q')?.textContent).toBe('What of measure?');
    // No line says why the merge waits for it: the card is red and the line names the cell.
    expect(container.textContent).not.toContain('The merge waits for this answer');
  });

  it("set a reason's names and values in the monospace, as the engine marks them, and a ruling's values as keys", async () => {
    const [, now, w] = OWNERS[5];
    const container = draw(now, [w], undefined, none, { read: { flow: RULED, blocks: [SAID_NO] } });
    await act(async () => undefined);
    const card = container.querySelector('[data-wait="write-content"]') as HTMLElement;
    expect([...card.querySelectorAll('.cw-choice-q code')].map((c) => c.textContent)).toEqual([
      'content',
      'test = may_merge',
    ]);
    expect(card.querySelector('.cw-choice-q')?.textContent).not.toContain('`');
    expect([...card.querySelectorAll('.cw-option-label code')].map((c) => c.textContent)).toEqual(['content = yes']);
  });

  it("draw the engine's pause of an ended work whose letting go failed as its one card, which alone lets it on", () => {
    const sent: Act[] = [];
    const failed = 'GitHub did not close the pull request: HTTP 502.';
    const why = `The engine paused the work. Letting go of what the work held failed. ${failed}`;
    const steps: Step[] = [{ kind: 'paused', at: 9, time: 0, by: 'engine', why: failed }];
    const container = draw(
      { kind: 'merged' },
      [wait('pause', why, { kind: 'resume' })],
      undefined,
      (_, a) => {
        sent.push(a);
        return Promise.resolve(null);
      },
      { paused: 'engine', steps },
    );
    expect(container.querySelectorAll('[data-wait="pause"]')).toHaveLength(1);
    expect(container.textContent?.split('Letting go').length).toBe(2);
    const buttons = [...container.querySelectorAll('.cw-item > .cw-acts button, .cw-waits button')].map((b) => b.textContent);
    expect(buttons).toEqual(['Resume']);
    act(() => (container.querySelector('[data-wait="pause"] button') as HTMLButtonElement).click());
    expect(sent).toEqual([{ kind: 'resume' }]);
  });

  it("draw a reason's marked names in the monospace and the rest as the plain text it is, a program's own words whole", () => {
    const said =
      'The engine paused the work. `pytest` failed in __init__: **kwargs is not a **mapping**.\n```\n# not a heading\n- not a list';
    const container = draw(OWNERS[2][1], [wait('pause', said, { kind: 'resume' })], undefined, none);
    const reason = container.querySelector('[data-wait="pause"] .cw-choice-q') as HTMLElement;
    expect([...reason.querySelectorAll('code')].map((c) => c.textContent)).toEqual(['pytest']);
    expect(reason.querySelector('strong, em, h1, h2, h3, h4, ul, ol, li, pre')).toBeNull();
    expect(reason.textContent).toBe(said.replace('`pytest`', 'pytest'));
  });

  it('are opened by an address that keeps the name of what waits whole, whatever its characters', () => {
    for (const key of ['answer-read time', 'answer-a/b', 'answer-über?#x', 'answer-"quoted"']) {
      const hash = hashOf({ view: 'console', item: 12, wait: key });
      expect(routeOf(hash)).toEqual({ view: 'console', item: 12, wait: key });
    }
  });

  it("let the engine's pause on from its card alone: the pause switch shows only the owner's own pause, and says so", () => {
    const pressed = (el: HTMLElement) => el.querySelector('.cw-acts [aria-pressed="true"], .cw-acts [aria-checked="true"]')?.textContent;
    const [, now, w] = OWNERS[2];
    const engine = draw(now, [w], undefined, none, { paused: 'engine' });
    expect(engine.querySelector('.cw-acts .switch')?.getAttribute('aria-label')).toBe('Your own pause of the work');
    expect(pressed(engine)).toBe('Start');
    unmount();
    const own = draw({ kind: 'paused', at: 'plan', by: 'owner', why: null }, [], undefined, none, { paused: 'owner' });
    expect(pressed(own)).toBe('Pause');
  });

  it('offer a ruling\'s values as options drawn as choices, the one that makes it hold picked, and write the one picked', async () => {
    const sent: Act[] = [];
    const [, now, w] = OWNERS[5];
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
