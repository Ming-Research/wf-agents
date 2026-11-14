// @vitest-environment happy-dom
import '../dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import type { ToldPage, TryBack, WorkPage } from '../works-api';
import {
  fixtureCall,
  fixtureCopies,
  fixtureItems,
  fixtureProjects,
  fixtureRound,
  fixtureTold,
  fixtureVersion,
  fixtureWork,
} from './fixtures';
import { type Source, SourceContext } from './source';
import { WorkScreen } from './WorkView';

let unmounts: (() => void)[] = [];
afterEach(() => {
  for (const u of unmounts) u();
  unmounts = [];
  document.body.innerHTML = '';
  localStorage.clear();
});

/// A source answered from the fixtures, what a round was told from `told()` where it is given, which counts how often
/// what a round was told was read.
function sourceOf(told?: () => ToldPage): Source & { reads: number } {
  const s = {
    reads: 0,
    loadCopies: async () => fixtureCopies(),
    loadItems: async () => fixtureItems(),
    loadProjects: async () => fixtureProjects(),
    loadWork: async (n: number) => fixtureWork(n),
    loadVersion: async (n: number, block: string, at: number) => fixtureVersion(n, block, at),
    loadRound: async (n: number, run: number, round: number) => fixtureRound(n, run, round),
    loadTold: async (n: number, run: number, round: number) => {
      s.reads += 1;
      return told ? told() : fixtureTold(n, run, round);
    },
    loadCall: async (n: number, call: number) => fixtureCall(n, call),
    act: async () => null,
    changes: () => () => undefined,
  };
  return s;
}

/// Draws the work as this page gives it, chooses the run of its step at this place, and gives the panel and the way
/// to draw the work again as another page gives it.
async function runPanel(page: WorkPage, step: number, source: Source) {
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  const draw = (p: WorkPage) =>
    act(() =>
      root.render(
        <SourceContext.Provider value={source}>
          <WorkScreen page={p} onActed={() => undefined} />
        </SourceContext.Provider>,
      ),
    );
  draw(page);
  unmounts.push(() => act(() => root.unmount()));
  await click(container.querySelectorAll('tbody tr.step-row')[step]?.querySelector('.step-lead button'));
  return { panel: container.querySelector('aside') as HTMLElement, draw };
}

async function click(element: Element | null | undefined) {
  if (!element) throw new Error('nothing to click');
  await act(async () => {
    (element as HTMLElement).click();
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

/// The part of the round of this number in a run's panel.
function roundPart(panel: HTMLElement, number: number): HTMLElement {
  const parts = [...panel.querySelectorAll('section.panel-part')];
  const part = parts.find((p) => p.querySelector('.panel-part-label')?.textContent?.startsWith(`Round ${number} `));
  if (!part) throw new Error(`no round ${number}`);
  return part as HTMLElement;
}

const button = (root: ParentNode, words: string) =>
  [...root.querySelectorAll('button')].find((b) => b.textContent?.trim() === words);

const texts = (root: ParentNode, selector: string) => [...root.querySelectorAll(selector)].map((e) => e.textContent ?? '');

const returned = (over: Partial<TryBack & { kind: 'returned' }> = {}): TryBack => ({
  kind: 'returned',
  text: null,
  structured: null,
  turns: null,
  durationMs: null,
  models: [],
  ...over,
});

describe('what a round was told and returned', () => {
  it('is folded at first, read when first opened, a structured return that is the closing words shown once', async () => {
    const source = sourceOf();
    // The long history's plan, whose second round handed in the plan.
    const { panel } = await runPanel(fixtureWork(41), 1, source);
    const round = roundPart(panel, 2);
    expect(button(round, 'Show what it was told')?.getAttribute('aria-expanded')).toBe('false');
    expect(button(round, 'Show what it returned')?.getAttribute('aria-expanded')).toBe('false');
    expect(round.querySelector('pre.content')).toBeNull();
    expect(source.reads).toBe(0);

    const told = fixtureTold(41, 1, 2).tries[0];
    await click(button(round, 'Show what it was told'));
    expect(source.reads).toBe(1);
    expect(texts(round, 'pre.content')).toEqual([told.message]);

    await click(button(round, 'Show what it returned'));
    if (told.back.kind !== 'returned') throw new Error('the fixture returned nothing');
    expect(texts(round, 'pre.content')).toEqual([told.message, told.back.structured]);
    expect(button(round, 'Show structured return')).toBeUndefined();
    expect(round.textContent).toContain('4 turns, 1m 58s, claude-haiku-5-5');
    // What an ended round was told is read once.
    await click(button(round, 'Hide what it was told'));
    await click(button(round, 'Show what it was told'));
    expect(source.reads).toBe(1);
  });

  it('shows each try in order, labelled: a cut message, a failure, a result that could not be read, and what is left out', async () => {
    const told: ToldPage = {
      tries: [
        {
          message: 'The first try.',
          cut: 3 * 1024 * 1024,
          back: { kind: 'unreadable', why: 'it is not a result: EOF' },
          ran: null,
        },
        { message: 'The second try.', cut: null, back: { kind: 'failed', why: 'API Error: 529 Overloaded' }, ran: null },
        {
          message: 'The third try.',
          cut: null,
          back: returned({ text: 'Done.', structured: '{\n  "asks": []\n}', turns: 1 }),
          ran: null,
        },
      ],
      leftOut: 2,
    };
    const { panel } = await runPanel(fixtureWork(41), 1, sourceOf(() => told));
    const round = roundPart(panel, 1);
    await click(button(round, 'Show what it was told'));
    expect(texts(round, '.label')).toEqual(['try 1', 'try 2', 'try 3']);
    expect(texts(round, 'pre.content')).toEqual(['The first try.', 'The second try.', 'The third try.']);
    expect(round.textContent).toContain('Cut: only its start is shown; the whole message is 3.0 MiB');
    expect(round.textContent).toContain('2 later tries are left out');
    await click(button(round, 'Hide what it was told'));
    await click(button(round, 'Show what it returned'));
    expect(texts(round, '.label')).toEqual(['try 1', 'try 2', 'try 3']);
    expect(texts(round, '.failed')).toEqual([
      'The result could not be read: it is not a result: EOF',
      'The try failed: API Error: 529 Overloaded',
    ]);
    // Closing words other than the structured return are shown, the structured return folded under them.
    expect(texts(round, 'pre.content')).toEqual(['Done.']);
    await click(button(round, 'Show structured return'));
    expect(texts(round, 'pre.content')).toEqual(['Done.', '{\n  "asks": []\n}']);
    expect(round.textContent).toContain('1 turn');
  });

  it('says in one muted line that nothing was kept of a round that ended', async () => {
    // The plan's first round asked for two reads; the fixtures kept nothing of it, as a script keeps nothing.
    const { panel } = await runPanel(fixtureWork(41), 1, sourceOf());
    const round = roundPart(panel, 1);
    await click(button(round, 'Show what it was told'));
    expect(texts(round, 'p.muted')).toContain('Nothing was kept of this round');
    expect(round.querySelector('pre.content')).toBeNull();
  });

  it('says of a round under way with no try yet that it is under way', async () => {
    // Work 42's implementation has its second round under way, waiting for its turn.
    const { panel } = await runPanel(fixtureWork(42), 2, sourceOf(() => ({ tries: [], leftOut: 0 })));
    const round = roundPart(panel, 2);
    await click(button(round, 'Show what it returned'));
    expect(texts(round, 'p.muted')).toContain('Nothing yet: it is under way');
  });

  it('reads a round again once it ends while it is open, and shows what came back', async () => {
    let ended = false;
    const source = sourceOf(() => ({
      tries: [
        {
          message: 'Given the result of npm run typecheck.',
          cut: null,
          back: ended ? returned({ text: 'Done.' }) : { kind: 'nothing' },
          ran: null,
        },
      ],
      leftOut: 0,
    }));
    // Work 42's implementation has its second round under way.
    const underWay = fixtureWork(42);
    const { panel, draw } = await runPanel(underWay, 2, source);
    await click(button(roundPart(panel, 2), 'Show what it returned'));
    expect(roundPart(panel, 2).textContent).toContain('Nothing yet: it is under way');
    expect(source.reads).toBe(1);

    // The round ends: the work is read again with the round's end.
    ended = true;
    const page: WorkPage = structuredClone(underWay);
    const run = page.steps[2];
    if (run.kind !== 'run') throw new Error('step 2 is no run');
    const last = run.rounds[1];
    last.ended = last.began + 60_000;
    last.back = { kind: 'handedIn', blocks: [] };
    draw(page);
    await settle();
    const round = roundPart(panel, 2);
    expect(source.reads).toBe(2);
    expect(round.textContent).not.toContain('Nothing yet');
    expect(round.textContent).not.toContain('Nothing came back');
    expect(texts(round, 'pre.content')).toEqual(['Done.']);
  });
});

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}
