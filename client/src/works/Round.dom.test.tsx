// @vitest-environment happy-dom
import '../dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { REOPEN_MS } from '../changes';
import type { Done, Following, WorkPage } from '../works-api';
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

/// A source answered from the fixtures, with these reads over them, which counts how often what each round's agent did
/// and each session's system prompt were read.
function sourceOf(over: Partial<Source> = {}): Source & { reads: { doing: number; system: string[] } } {
  const reads = { doing: 0, system: [] as string[] };
  return {
    reads,
    loadCopies: async () => fixtureCopies(),
    loadItems: async () => fixtureItems(),
    loadProjects: async () => fixtureProjects(),
    loadWork: async (n) => fixtureWork(n),
    loadVersion: async (n, block, number) => fixtureVersion(n, block, number),
    loadRound: async (n, run, round) => fixtureRound(n, run, round),
    loadTold: async (n, run, round) => fixtureTold(n, run, round),
    loadDoing: async (n, run, round) => {
      reads.doing += 1;
      return fixtureDoing(n, run, round);
    },
    followDoing: () => () => undefined,
    loadChanges: () => Promise.reject(new Error('not read here')),
    loadDiff: () => Promise.reject(new Error('not read here')),
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

/// Draws the work, its item read, chooses the run of the row at this place among the runs' rows, and gives the item's
/// window, whose Step then shows it.
async function runPanel(page: WorkPage, run: number, source: Source): Promise<HTMLElement> {
  const container = drawWork(page, source);
  await settle();
  await click(container.querySelectorAll('tbody tr.step-row')[run]?.querySelector('.step-lead button'));
  return container.querySelector('aside') as HTMLElement;
}

/// The part of a run's panel of the time its cell ran of this number: the first what it did, the others each time it
/// ran again.
function roundPart(panel: HTMLElement, number: number): HTMLElement {
  const parts = [...panel.querySelectorAll('section.panel-part')].filter((p) =>
    /^(What it did|Ran again) /.test(p.querySelector('.panel-part-label')?.textContent ?? ''),
  );
  const part = parts[number - 1];
  if (!part) throw new Error(`no time ${number}`);
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
    // The long history's plan: its first round runs in the builder's first session, which the records count as none,
    // and asks for two calls.
    const panel = await runPanel(fixtureWork(41), 0, sourceOf());
    const first = roundPart(panel, 1);
    // What it ran on is the run's, said once above its times.
    expect(pair(first, 'Ran')).toBeUndefined();
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
    const panel = await runPanel(page, 0, sourceOf());
    const round = roundPart(panel, 2);
    expect(keys(pair(round, 'Missed'))).toEqual(['first_read 1,200', 'held 34,340', 'differs model']);
    expect(keys(pair(round, 'Took')).at(-1)).toBe('priced true');
  });

  it('names its times by what they were and never by a number, and says why it ran again', async () => {
    const panel = await runPanel(fixtureWork(41), 0, sourceOf());
    const labels = [...panel.querySelectorAll('.panel-part-label')].map((l) => l.textContent ?? '');
    expect(labels.filter((l) => l.startsWith('What it did ')).length).toBe(1);
    expect(labels.filter((l) => l.startsWith('Ran again ')).length).toBe(1);
    expect(labels.some((l) => /Round \d/.test(l))).toBe(false);
    // The second time is given the results of the first one's calls; nothing filled into the session is listed.
    expect(pair(roundPart(panel, 2), 'Why')?.textContent).toBe('the results of the calls it asked for came back');
    expect(pair(roundPart(panel, 1), 'Filled')).toBeUndefined();
    // The calls the first time asked for are under it, by what they are, with no number.
    expect(rows(pair(roundPart(panel, 1), 'Back')).length).toBeGreaterThan(0);
    expect(pair(roundPart(panel, 1), 'Back')?.textContent).not.toMatch(/call \d/);
  });

  it("shows on each round the system prompt of its session, which each round is sent, read once when opened", async () => {
    const source = sourceOf();
    const panel = await runPanel(fixtureWork(41), 0, source);
    const [first, second] = [roundPart(panel, 1), roundPart(panel, 2)];
    expect(source.reads.system).toEqual([]);
    await click(button(first, 'Show system prompt'));
    await click(button(second, 'Show system prompt'));
    expect(source.reads.system).toEqual(['builder 0']);
    const text = fixtureSystem(41, 'builder', 0).text;
    expect([first, second].map((r) => r.querySelector('pre.content')?.textContent)).toEqual([text, text]);
  });

  it('follows what the agent of a round under way does as it comes, while the Step is shown, the latest two open', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const streams: { following: Following; stopped: boolean }[] = [];
    const source = sourceOf({
      followDoing: (_work, _run, _round, following) => {
        const stream = { following, stopped: false };
        streams.push(stream);
        return () => {
          stream.stopped = true;
        };
      },
    });
    // Work 42's implementation has its second round under way: it is followed from its start, and its first round,
    // which ended, is read once, whole.
    const panel = await runPanel(fixtureWork(42), 1, source);
    expect(streams).toHaveLength(1);
    expect(source.reads.doing).toBe(1);
    expect(pair(roundPart(panel, 1), 'Did')?.textContent).toBe('[]');
    expect(pair(roundPart(panel, 2), 'Doing')?.textContent).toBe('[]');
    const told: Done[] = [
      { kind: 'tool', name: 'Read', on: 'client/src/sizes.ts' },
      { kind: 'said', text: 'The width is kept under its key.' },
      { kind: 'tool', name: 'Edit', on: 'client/src/sizes.ts' },
    ];
    act(() => told.forEach((d) => streams[0].following.done(d)));
    const doing = () => pair(roundPart(panel, 2), 'Doing');
    // The latest two stand open; those before them are folded behind one line that opens them all.
    expect(rows(doing())).toEqual(['The width is kept under its key.', 'Editclient/src/sizes.ts']);
    await click(button(doing() as HTMLElement, 'Show 1 earlier'));
    expect(rows(doing())).toEqual(['Readclient/src/sizes.ts', 'The width is kept under its key.', 'Editclient/src/sizes.ts']);
    await click(button(doing() as HTMLElement, 'Hide 1 earlier'));
    // The window put away, nothing is followed; brought back, the round is followed anew from its start, passing over
    // what the list holds.
    await click(panel.querySelector('.cw-tab'));
    expect(streams[0].stopped).toBe(true);
    await click(panel.querySelector('.cw-tab'));
    expect(streams).toHaveLength(2);
    const fourth: Done = { kind: 'compacting' };
    act(() => [...told, fourth].forEach((d) => streams[1].following.done(d)));
    expect(rows(doing())).toEqual(['Editclient/src/sizes.ts', 'compact_boundary']);
    expect(button(doing() as HTMLElement, 'Show 2 earlier')).toBeDefined();
    // A stream the browser lost is followed again a few seconds later; once all was told, nothing more is followed.
    act(() => streams[1].following.lost());
    expect(streams).toHaveLength(2);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(REOPEN_MS);
    });
    expect(streams).toHaveLength(3);
    act(() => streams[2].following.end());
    await click(panel.querySelector('.cw-tab'));
    await click(panel.querySelector('.cw-tab'));
    expect(streams).toHaveLength(3);
  });

  it('says why what the agent did could not be read', async () => {
    const held: { following: Following | null } = { following: null };
    const source = sourceOf({
      followDoing: (_work, _run, _round, following) => {
        held.following = following;
        return () => undefined;
      },
    });
    const panel = await runPanel(fixtureWork(42), 1, source);
    act(() => held.following?.failed('the round kept nothing that can be read'));
    expect(pair(roundPart(panel, 2), 'Doing')?.querySelector('.failed')?.textContent).toBe(
      'It could not be read: the round kept nothing that can be read',
    );
  });
});

describe('what a work used', () => {
  it("is the Usage of the work's item window", async () => {
    const page = drawWork(fixtureWork(41), sourceOf());
    await settle();
    const window = page.querySelector('aside') as HTMLElement;
    await click(button(window, 'Usage'));
    expect(window.querySelector('[role="tab"][aria-selected="true"]')?.textContent).toBe('Usage');
    const panel = window.querySelector('.drawer-body') as HTMLElement;
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
  });
});
