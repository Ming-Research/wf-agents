// @vitest-environment happy-dom
import '../dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { DoingPage, WorkPage } from '../works-api';
import {
  fixtureCall,
  fixtureCopies,
  fixtureDoing,
  fixtureItems,
  fixtureProjects,
  fixtureRound,
  fixtureSystem,
  fixtureTold,
  fixtureVersion,
  fixtureWork,
} from './fixtures';
import { DOING_MS } from './Round';
import { type Source, SourceContext } from './source';
import { WorkScreen } from './WorkView';

let unmounts: (() => void)[] = [];
afterEach(() => {
  for (const u of unmounts) u();
  unmounts = [];
  document.body.innerHTML = '';
  localStorage.clear();
  vi.useRealTimers();
});

/// A source answered from the fixtures, with these reads over them, which counts how often each round's agent's doing
/// and each session's system prompt were read.
function sourceOf(over: Partial<Source> = {}): Source & { reads: { doing: number; system: string[] } } {
  const reads = { doing: 0, system: [] as string[] };
  return {
    reads,
    loadCopies: async () => fixtureCopies(),
    loadItems: async () => fixtureItems(),
    loadProjects: async () => fixtureProjects(),
    loadWork: async (n) => fixtureWork(n),
    loadVersion: async (n, block, at) => fixtureVersion(n, block, at),
    loadRound: async (n, run, round) => fixtureRound(n, run, round),
    loadTold: async (n, run, round) => fixtureTold(n, run, round),
    loadDoing: async (n, run, round) => {
      reads.doing += 1;
      return fixtureDoing(n, run, round);
    },
    loadSystem: async (n, line, session) => {
      reads.system.push(`${line} ${session}`);
      return fixtureSystem(n, line, session);
    },
    loadCall: async (n, call) => fixtureCall(n, call),
    act: async () => null,
    changes: () => () => undefined,
    ...over,
  };
}

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

async function click(element: Element | null | undefined) {
  if (!element) throw new Error('nothing to click');
  await act(async () => {
    (element as HTMLElement).click();
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

/// Draws the work as this page gives it.
function drawWork(page: WorkPage, source: Source): HTMLElement {
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  act(() =>
    root.render(
      <SourceContext.Provider value={source}>
        <WorkScreen page={page} onActed={() => undefined} />
      </SourceContext.Provider>,
    ),
  );
  unmounts.push(() => act(() => root.unmount()));
  return container;
}

/// Draws the work, chooses the run of its step at this place, and gives the panel.
async function runPanel(page: WorkPage, step: number, source: Source): Promise<HTMLElement> {
  const container = drawWork(page, source);
  await click(container.querySelectorAll('tbody tr.step-row')[step]?.querySelector('.step-lead button'));
  return container.querySelector('aside') as HTMLElement;
}

/// The part of the round of this number in a run's panel.
function roundPart(panel: HTMLElement, number: number): HTMLElement {
  const parts = [...panel.querySelectorAll('section.panel-part')];
  const part = parts.find((p) => p.querySelector('.panel-part-label')?.textContent?.startsWith(`Round ${number} `));
  if (!part) throw new Error(`no round ${number}`);
  return part as HTMLElement;
}

/// What a round's pair of this name holds; none where the round has no such pair.
function pair(round: HTMLElement, name: string): HTMLElement | undefined {
  const dt = [...round.querySelectorAll('dl.pairs > dt')].find((d) => d.textContent === name);
  return (dt?.nextElementSibling as HTMLElement | null) ?? undefined;
}

/// Each key and its value in a pair, as the page writes them.
const keys = (dd: HTMLElement | undefined) => [...(dd?.querySelectorAll('.keys > span') ?? [])].map((k) => k.textContent);

/// The rows of a pair's list.
const rows = (dd: HTMLElement | undefined) => [...(dd?.querySelectorAll('li.list-row') ?? [])].map((r) => r.textContent);

const button = (root: ParentNode, words: string) =>
  [...root.querySelectorAll('button')].find((b) => b.textContent?.trim() === words) as HTMLButtonElement | undefined;

describe("a round's part of a run's panel", () => {
  it('shows what it ran on, what it took and the context it ended with, by their keys', async () => {
    // The long history's plan: its first round runs in the engineer's first session, which the records count as none,
    // and asks for two calls.
    const panel = await runPanel(fixtureWork(41), 1, sourceOf());
    const first = roundPart(panel, 1);
    expect(keys(pair(first, 'Ran'))).toEqual([
      'line engineer',
      'session 0',
      'level 2',
      'model claude-sonnet-5-5',
      'effort high',
    ]);
    // The records' keys, and what the page works out from them said plainly.
    expect(keys(pair(first, 'Took'))).toEqual([
      'usd_micros in dollars $0.09',
      'input_tokens 20,200',
      'cached_tokens 14,140',
      'output_tokens 1,500',
      'cached_tokens / (input_tokens + cached_tokens) 41%',
    ]);
    expect(keys(pair(first, 'Context'))).toEqual(['context_tokens 34,340', 'compactions 0']);
    expect(pair(first, 'Missed')).toBeUndefined();
  });

  it('shows how a round missed its session\'s cache, and that what it took was priced from its calls', async () => {
    const page = fixtureWork(41);
    const run = page.steps.find((s) => s.kind === 'run');
    if (run?.kind !== 'run') throw new Error('no run');
    const second = run.rounds[1];
    second.missed = { read: 1_200, held: 34_340, differs: ['model'] };
    second.estimated = true;
    const panel = await runPanel(page, 1, sourceOf());
    const round = roundPart(panel, 2);
    expect(keys(pair(round, 'Missed'))).toEqual(['first_read 1,200', 'held 34,340', 'differs model']);
    expect(keys(pair(round, 'Took')).at(-1)).toBe('priced true');
  });

  it('shows what was filled into its session, each part as the flow\'s files name it, before what it was sent', async () => {
    const panel = await runPanel(fixtureWork(41), 1, sourceOf());
    const first = roundPart(panel, 1);
    expect(rows(pair(first, 'Filled'))).toEqual(['work', 'outputs', 'context request@1', 'task']);
    // The second round is given the results of the first one's calls.
    expect(rows(pair(roundPart(panel, 2), 'Filled'))).toEqual(['results [1, 2]']);
    const order = [...first.querySelectorAll('dt, button')].map((e) => e.textContent?.trim());
    expect(order.indexOf('Filled')).toBeLessThan(order.indexOf('Show what it was told'));
  });

  it("shows on each round the system prompt of its session, which each round is sent, read once when opened", async () => {
    const source = sourceOf();
    const panel = await runPanel(fixtureWork(41), 1, source);
    const [first, second] = [roundPart(panel, 1), roundPart(panel, 2)];
    expect(source.reads.system).toEqual([]);
    await click(button(first, 'Show system prompt'));
    await click(button(second, 'Show system prompt'));
    expect(source.reads.system).toEqual(['engineer 0']);
    const text = fixtureSystem(41, 'engineer', 0).text;
    expect([first, second].map((r) => r.querySelector('pre.content')?.textContent)).toEqual([text, text]);
  });

  it('shows what the agent of a round under way is doing, read again every few seconds while it is shown', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const said: DoingPage = { doing: [{ kind: 'said', text: 'The types check.' }, { kind: 'compacting' }] };
    let reads = 0;
    const source = sourceOf({
      loadDoing: async (n, run, round) => {
        reads += 1;
        return reads === 1 ? fixtureDoing(n, run, round) : said;
      },
    });
    // Work 42's implementation has its second round under way.
    const panel = await runPanel(fixtureWork(42), 2, source);
    await settle();
    expect(reads).toBe(1);
    expect(rows(pair(roundPart(panel, 2), 'Doing'))).toEqual([
      'Readclient/src/sizes.ts',
      'The width is kept under the key the panel names; I will read it back as the page loads.',
      'Editclient/src/sizes.ts',
      'Bashnpm run typecheck',
    ]);
    // A round that ended says nothing of what its agent is doing.
    expect(pair(roundPart(panel, 1), 'Doing')).toBeUndefined();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(DOING_MS);
    });
    expect(reads).toBe(2);
    expect(rows(pair(roundPart(panel, 2), 'Doing'))).toEqual(['The types check.', 'compact_boundary']);
    // Folded away, the panel reads nothing; brought back, it reads again at once.
    await click(panel.querySelector('.drawer-tab'));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(3 * DOING_MS);
    });
    expect(reads).toBe(2);
    await click(panel.querySelector('.drawer-tab'));
    expect(reads).toBe(3);
    // Let go of, the run's panel reads nothing more.
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(3 * DOING_MS);
    });
    expect(reads).toBe(3);
  });
});

describe('what a work used', () => {
  it("opens from what the view's line says the work has taken, and closes again", async () => {
    const page = drawWork(fixtureWork(41), sourceOf());
    const spend = page.querySelector('.view-end button.link') as HTMLButtonElement;
    expect(spend.getAttribute('aria-pressed')).toBe('false');
    await click(spend);
    const panel = page.querySelector('aside') as HTMLElement;
    expect(panel.getAttribute('aria-label')).toBe('What work W41 used');
    expect(spend.getAttribute('aria-pressed')).toBe('true');
    // What it took and its rounds, by the records' keys and what the page works out from them said plainly.
    const totals = [...panel.querySelectorAll('.usage-totals .keys > span > span:first-child')];
    expect(totals.map((k) => k.textContent)).toEqual([
      'usd_micros in dollars',
      'input_tokens',
      'cached_tokens',
      'output_tokens',
      'cached_tokens / (input_tokens + cached_tokens)',
      'rounds',
      "rounds of agents' cells",
      'sent_back',
      'compactions',
      "first round's begin to last round's end",
      'under way',
    ]);
    expect([...panel.querySelectorAll('.panel-part-label')].map((l) => l.firstChild?.textContent)).toEqual([
      'Where the time went',
      'Models',
      'Where the cost went',
      'Sessions',
      'Tools',
    ]);
    // Every model row says the level and effort its rounds recorded, by their keys.
    const models = [...panel.querySelectorAll('section.panel-part')][1];
    expect([...models.querySelectorAll('li.list-row > span:not(.row-note)')].map((k) => k.textContent)).toEqual([
      'model claude-sonnet-5-5',
      'level 2',
      'effort high',
    ]);
    // Each gap on the line of time says its own length on pointing at it.
    const gaps = [...panel.querySelectorAll('.usage-timeline .us-gap')].map((g) => g.getAttribute('title') ?? '');
    expect(gaps.length).toBeGreaterThan(0);
    for (const gap of gaps) expect(gap).toMatch(/^Not running: from .+, to .+, for \d+[smhd]/);
    await click(spend);
    expect(page.querySelector('aside')?.getAttribute('aria-label')).toBe('Blocks');
  });
});
