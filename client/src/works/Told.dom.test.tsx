// @vitest-environment happy-dom
import '../dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import type { ToldPage } from '../works-api';
import { fixtureCall, fixtureItems, fixtureRound, fixtureTold, fixtureVersion, fixtureWork } from './fixtures';
import { type Source, SourceContext } from './source';
import { WorkScreen } from './WorkView';

let unmounts: (() => void)[] = [];
afterEach(() => {
  for (const u of unmounts) u();
  unmounts = [];
  document.body.innerHTML = '';
  localStorage.clear();
});

/// A source answered from the fixtures, what a round was told from `told` where it is given, which counts how often
/// what a round was told was read.
function sourceOf(told?: ToldPage): Source & { reads: number } {
  const s = {
    reads: 0,
    loadItems: async () => fixtureItems(),
    loadWork: async (n: number) => fixtureWork(n),
    loadVersion: async (n: number, block: string, at: number) => fixtureVersion(n, block, at),
    loadRound: async (n: number, run: number, round: number) => fixtureRound(n, run, round),
    loadTold: async (n: number, run: number, round: number) => {
      s.reads += 1;
      return told ?? fixtureTold(n, run, round);
    },
    loadCall: async (n: number, call: number) => fixtureCall(n, call),
    act: async () => null,
    changes: () => () => undefined,
  };
  return s;
}

/// Draws the work, chooses the run of its step at this place, and gives the run's panel.
async function runPanel(work: number, step: number, source: Source): Promise<HTMLElement> {
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  act(() =>
    root.render(
      <SourceContext.Provider value={source}>
        <WorkScreen page={fixtureWork(work)} onActed={() => undefined} />
      </SourceContext.Provider>,
    ),
  );
  unmounts.push(() => act(() => root.unmount()));
  await click(container.querySelectorAll('tbody tr.step-row')[step]?.querySelector('.step-lead button'));
  return container.querySelector('aside') as HTMLElement;
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

describe('what a round was told and returned', () => {
  it('is folded at first, read when first opened, the structured return folded on its own', async () => {
    const source = sourceOf();
    // The long history's plan, whose second round handed in the plan.
    const panel = await runPanel(41, 1, source);
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
    if (told.returned?.kind !== 'read') throw new Error('the fixture returned nothing');
    expect(texts(round, 'pre.content')).toEqual([told.message, told.returned.text]);
    expect(round.textContent).toContain('4 turns, 1m 58s, claude-haiku-5-5');
    await click(button(round, 'Show structured return'));
    expect(texts(round, 'pre.content').at(-1)).toBe(told.returned.structured);
    // What an ended round was told is read once.
    await click(button(round, 'Hide what it was told'));
    await click(button(round, 'Show what it was told'));
    expect(source.reads).toBe(1);
  });

  it('shows each try of a round in order, labelled, and a result that could not be read', async () => {
    const told: ToldPage = {
      tries: [
        { message: 'The first try.', returned: { kind: 'unreadable', why: 'the program printed nothing' } },
        {
          message: 'The second try.',
          returned: { kind: 'read', text: 'Done.', structured: null, turns: 1, durationMs: null, models: [] },
        },
      ],
    };
    const round = roundPart(await runPanel(41, 1, sourceOf(told)), 1);
    await click(button(round, 'Show what it was told'));
    expect(texts(round, '.label')).toEqual(['try 1', 'try 2']);
    expect(texts(round, 'pre.content')).toEqual(['The first try.', 'The second try.']);
    await click(button(round, 'Hide what it was told'));
    await click(button(round, 'Show what it returned'));
    expect(texts(round, '.label')).toEqual(['try 1', 'try 2']);
    expect(round.querySelector('.failed')?.textContent).toBe('The result could not be read: the program printed nothing');
    expect(texts(round, 'pre.content')).toEqual(['Done.']);
    expect(round.textContent).toContain('It gave no structured return');
    expect(round.textContent).toContain('1 turn');
  });

  it('says in one muted line that nothing was kept of a round', async () => {
    // The plan's first round asked for two reads; the fixtures kept nothing of it, as a script keeps nothing.
    const round = roundPart(await runPanel(41, 1, sourceOf()), 1);
    await click(button(round, 'Show what it was told'));
    expect(texts(round, 'p.muted')).toContain('Nothing was kept of this round');
    expect(round.querySelector('pre.content')).toBeNull();
  });

  it('says of a round under way that nothing has come back yet, and reads it again each time', async () => {
    const source = sourceOf({ tries: [{ message: 'Given the result of npm run typecheck.', returned: null }] });
    // Work 42's implementation has its second round under way.
    const panel = await runPanel(42, 2, source);
    const round = roundPart(panel, 2);
    await click(button(round, 'Show what it returned'));
    expect(round.textContent).toContain('Nothing yet: it is under way');
    await click(button(round, 'Show what it was told'));
    expect(source.reads).toBe(2);
  });
});
