// @vitest-environment happy-dom
import '../dom-setup';

import { act, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { Changed, Engine, Item, Items, Outage, Wait } from '../works-api';
import { waitingCount } from '../works/waiting';
import { waitCount, waitCounted } from '../works/words';
import { fixtureCopies, fixtureItems, fixtureWork } from '../works/fixtures';
import { type Source, SourceContext } from '../works/source';
import type { Spent } from '../works-api';
import { FIXTURES, type ProjectSpent, type SystemSource, SystemSourceContext } from './api';
import { fixtureAccounts, fixtureSettings, fixtureSystem, fixtureUnderWay } from './fixtures';
import { STANDING_WORDS, standingOf } from './standing';
import { SystemView } from './SystemView';

let unmounts: (() => void)[] = [];
afterEach(() => {
  for (const u of unmounts) u();
  unmounts = [];
  document.body.innerHTML = '';
  window.location.hash = '';
});

/// The works' fixtures with an engine, and the system's, each read counted; the parts that listen are told of a change
/// when the test says.
function sources(engine: Engine = { slots: 2, taken: 1, paused: null, keepWarm: true }) {
  const listening = new Set<{ onChanged: (c: Changed) => void; onOpened: () => void }>();
  const asked: number[][] = [];
  const goingRead = { count: 0 };
  const works: Source = {
    loadCopies: async () => fixtureCopies(),
    loadItems: async () => fixtureItems(),
    loadProjects: async () => ({ names: [] }),
    loadWork: async (n) => fixtureWork(n),
    loadVersion: async () => {
      throw new Error('not read here');
    },
    loadRound: async () => {
      throw new Error('not read here');
    },
    loadTold: async () => {
      throw new Error('not read here');
    },
    loadDoing: async () => {
      throw new Error('not read here');
    },
    followDoing: () => () => undefined,
    loadChanges: async () => {
      throw new Error('not read here');
    },
    loadDiff: async () => {
      throw new Error('not read here');
    },
    loadSystem: async () => {
      throw new Error('not read here');
    },
    loadCall: async () => {
      throw new Error('not read here');
    },
    act: async () => null,
    loadEngine: async () => engine,
    changes: (onChanged, onOpened) => {
      const l = { onChanged, onOpened };
      listening.add(l);
      return () => listening.delete(l);
    },
  };
  const system: SystemSource = {
    loadSystem: async (weeks) => {
      asked.push(weeks);
      return fixtureSystem(weeks);
    },
    loadUnderWay: async () => {
      goingRead.count += 1;
      return fixtureUnderWay();
    },
    loadSettings: async () => fixtureSettings(),
    saveSettings: async () => ({ said: '', committed: false }),
    loadAccounts: async () => fixtureAccounts(),
    account: async () => ({ atLimit: false, resets: null, notLetGo: null }),
    look: async () => ({ looking: [] }),
  };
  return {
    works,
    system,
    asked,
    goingRead,
    changed: (c: Changed) => listening.forEach((l) => l.onChanged(c)),
    opened: () => listening.forEach((l) => l.onOpened()),
  };
}

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

function draw(element: ReactNode, s: ReturnType<typeof sources>) {
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  const render = (e: ReactNode) =>
    act(() =>
      root.render(
        <SourceContext.Provider value={s.works}>
          <SystemSourceContext.Provider value={s.system}>{e}</SystemSourceContext.Provider>
        </SourceContext.Provider>,
      ),
    );
  render(element);
  unmounts.push(() => act(() => root.unmount()));
  return { container, render };
}

/// The columns of what a round's calls have come to so far.
const CALLS = ['Context', 'Added', 'Per call', 'Cost'];

const texts = (root: ParentNode, selector: string) => [...root.querySelectorAll(selector)].map((e) => e.textContent?.trim() ?? '');

/// The table under a heading of the view.
function tableUnder(root: ParentNode, heading: string): HTMLElement {
  const head = [...root.querySelectorAll('h4')].find((h) => h.textContent === heading);
  const table = head?.nextElementSibling;
  if (table?.tagName !== 'TABLE') throw new Error(`no table under ${heading}`);
  return table as HTMLElement;
}

/// What was spent: these millionths of a dollar, rounds and milliseconds under way, and nothing else.
const spending = (usdMicros: number, rounds: number, workingMs: number): Spent => ({
  usdMicros,
  inputTokens: 0,
  cacheWrittenTokens: 0,
  unsplitTokens: 0,
  cachedTokens: 0,
  outputTokens: 0,
  workingMs,
  rounds,
});

/// A part of the view by its label.
function part(root: ParentNode, label: string): HTMLElement {
  const found = [...root.querySelectorAll('section.panel-part')].find(
    (s) => s.querySelector('.panel-part-label')?.textContent?.startsWith(label),
  );
  if (!found) throw new Error(`no part ${label}`);
  return found as HTMLElement;
}

describe('the System view', () => {
  it('shows no copies of the database where the engine keeps none, as one that runs a script does', async () => {
    const s = sources();
    s.works.loadCopies = async () => ({ ...fixtureCopies(), state: { kind: 'notKept' } });
    const { container } = draw(<SystemView period="week" />, s);
    await settle();
    expect(texts(part(container, 'Now'), 'dt')).not.toContain('Copies of the database');
    expect(container.textContent).not.toContain('could not be read');
  });

  it('says how the engine stands now: its pause, its slots, the copies, the works by where they stand, the rounds under way', async () => {
    const s = sources({ slots: 3, taken: 1, paused: { since: Date.now(), by: 'owner' }, keepWarm: true });
    const { container } = draw(<SystemView period="week" />, s);
    await settle();
    expect(container.querySelector('.view-facts')).toBeNull();
    const now = part(container, 'Now');
    const pairs = texts(now, 'dd');
    expect(pairs[0]).toContain('Every work paused since');
    expect(pairs[1]).toBe('1 taken of 3');
    expect(pairs[2]).toBe('none');
    expect(pairs[3]).toContain('The newest copy of the database was taken');
    // Each work in one place, counted under its standing.
    const items = fixtureItems().items;
    for (const [standing, words] of Object.entries(STANDING_WORDS)) {
      if (standing === 'owner') continue;
      const n = items.filter((i) => standingOf(i.work) === standing).length;
      const tile = [...now.querySelectorAll('.tile')].find((t) => t.querySelector('.label')?.textContent === words.name);
      // The works whose round is under way are in the list of the rounds under way, and nowhere else.
      if (standing === 'running') expect(tile).toBeUndefined();
      else expect(tile?.querySelector('.tile-value')?.textContent, words.name).toBe(String(n));
    }

    const running = items.filter((i) => i.work.now.kind === 'round');
    expect(running.length).toBeGreaterThan(0);
    const under = now.querySelectorAll('table')[0];
    expect(texts(under, 'tbody td:first-child')).toEqual(running.map((i) => `W${i.work.number}`));
  });

  it('opens the work with the round of its row chosen, from each row of the rounds under way, sent back and failed', async () => {
    const s = sources();
    const sentBack = { work: 41, item: 141, cell: 'plan', run: 2, round: 3, began: Date.now(), spent: null };
    s.system.loadSystem = async (weeks) => ({ ...fixtureSystem(weeks), sentBack: [sentBack] });
    const { container } = draw(<SystemView period="week" />, s);
    await settle();
    const hrefs = (root: ParentNode) =>
      [...root.querySelectorAll('tbody tr')].map((tr) => tr.querySelector('td a')?.getAttribute('href'));
    // The rounds under way: each work's round, as the work's state says it.
    const running = fixtureItems().items.flatMap((i) => (i.work.now.kind === 'round' ? [{ work: i.work.number, now: i.work.now }] : []));
    expect(running.length).toBeGreaterThan(0);
    expect(hrefs(part(container, 'Now').querySelectorAll('table')[0])).toEqual(
      running.map((r) => `#/works/${r.work}/runs/${r.now.run}/rounds/${r.now.round}`),
    );
    const wrong = part(container, 'Gone wrong');
    // The rounds sent back, unfolded by their count.
    const tile = [...wrong.querySelectorAll('button.tile')].find((t) => t.textContent?.includes('Rounds sent back')) as HTMLElement;
    act(() => tile.click());
    const failed = fixtureSystem([0]).failed;
    expect(failed).toHaveLength(1);
    expect(hrefs(wrong.querySelectorAll('table')[0])).toEqual(['#/works/41/runs/2/rounds/3']);
    expect(hrefs(wrong.querySelectorAll('table')[1])).toEqual([
      `#/works/${failed[0].work}/runs/${failed[0].run}/rounds/${failed[0].round}`,
    ]);
    expect(wrong.querySelector('table a')?.getAttribute('title')).toBe('Open the work with this round chosen');
  });

  it('says, while a stop of the engine stands, when the engine is expected to end and how long each round has to go', async () => {
    const s = sources({ slots: 3, taken: 1, paused: null, keepWarm: true, stopping: { since: Date.now(), atOnce: false } });
    const running = fixtureItems().items.filter((i) => i.work.now.kind === 'round');
    expect(running.length).toBeGreaterThan(0);
    s.system.loadSystem = async (weeks) => ({
      ...fixtureSystem(weeks),
      underWay: running.map((i, n) => {
        const now = i.work.now;
        if (now.kind !== 'round') throw new Error('a round');
        return {
          kind: 'round' as const,
          work: i.work.number,
          item: i.number,
          cell: now.cell,
          run: now.run,
          round: now.round,
          began: now.since,
          time: null,
          rest: n === 0 ? { kind: 'expected' as const, ms: 12 * 60_000 } : { kind: 'longer' as const },
        };
      }),
      stopping: { since: Date.now(), atOnce: false, rest: { kind: 'expected', ms: 12 * 60_000 } },
    });
    const { container } = draw(<SystemView period="week" />, s);
    await settle();
    const now = part(container, 'Now');
    expect(texts(now, 'dd')[0]).toMatch(
      /^Stopping since [^:]+:\d\d: no round begins, and the engine ends once the rounds under way have ended, expected in about 12m 0s$/,
    );
    const under = now.querySelectorAll('table')[0];
    expect(texts(under, 'thead th')).toEqual(['Work', 'Item', 'Project', 'Cell', 'Since', 'To go', ...CALLS]);
    expect(texts(under, 'thead th.num')).toEqual(CALLS);
    const rests = texts(under, 'tbody td:nth-child(6)');
    expect(rests[0]).toBe('about 12m 0s');
    expect(rests.slice(1)).toEqual(running.slice(1).map(() => 'longer than any before'));
  });

  it('says, of a project whose main is red, the tests, the head and the works that wait, in red; of one whose main is not, nothing', async () => {
    const s = sources();
    s.system.loadSystem = async (weeks) => ({
      ...fixtureSystem(weeks),
      redMain: [
        {
          project: 'Firn-wf',
          since: Date.now() - 3_600_000,
          tests: [{ unit: 'firn', name: 'firn_runs_scripts', head: 'f4e3d6c0123456789' }],
          waiting: [184, 190],
        },
      ],
    });
    const { container } = draw(<SystemView period="week" />, s);
    await settle();
    const now = part(container, 'Now');
    const heading = [...now.querySelectorAll('dt')].find((dt) => dt.textContent === 'Main of Firn-wf');
    expect(heading).toBeDefined();
    const said = heading?.nextElementSibling;
    expect(said?.className).toBe('failed');
    expect(said?.textContent).toContain('firn_runs_scripts fails on its head f4e3d6c01234');
    expect(said?.textContent).toContain('2 works wait for it to pass');

    const quiet = draw(<SystemView period="week" />, sources());
    await settle();
    expect(texts(part(quiet.container, 'Now'), 'dt').filter((dt) => dt.startsWith('Main of'))).toEqual([]);
  });

  it('says no rest of a round, and nothing of a stop, while no stop stands', async () => {
    const s = sources();
    const { container } = draw(<SystemView period="week" />, s);
    await settle();
    const now = part(container, 'Now');
    expect(texts(now, 'dd')[0]).toBe('Every work goes on');
    expect(texts(now.querySelectorAll('table')[0], 'thead th')).toEqual(['Work', 'Item', 'Project', 'Cell', 'Since', ...CALLS]);
  });

  it('counts a work whose state could not be worked out in a tile of its own and lists it there, not among the halted', async () => {
    const s = sources();
    const items = fixtureItems();
    const first = items.items.find((i) => standingOf(i.work) === 'due' || standingOf(i.work) === 'paused') as Item;
    const broken: Items = {
      ...items,
      items: items.items.map((i) =>
        i === first
          ? { ...i, work: { ...i.work, waits: [], now: { kind: 'broken', words: 'the driver names no turn' } as const } }
          : i,
      ),
    };
    s.works.loadItems = async () => broken;
    const { container } = draw(<SystemView period="week" />, s);
    await settle();
    const now = part(container, 'Now');
    const count = (name: string) =>
      [...now.querySelectorAll('.tile')]
        .find((t) => t.querySelector('.label')?.textContent === name)
        ?.querySelector('.tile-value')?.textContent;
    const among = (standing: string) => broken.items.filter((i) => standingOf(i.work) === standing).length;
    expect(among('broken')).toBe(1);
    expect(count(STANDING_WORDS.broken.name)).toBe('1');
    expect(count(STANDING_WORDS.halted.name)).toBe(String(among('halted')));
    const tile = [...now.querySelectorAll('button.tile')].find((t) => t.textContent?.includes('Broken')) as HTMLElement;
    act(() => tile.click());
    const list = now.querySelector('table[aria-label="Broken"]');
    expect(texts(list!, 'tbody td:first-child')).toEqual([`W${first.work.number}`]);
  });

  it('counts what waits for the owner by the one count, the things that wait, as the bar says it', async () => {
    const s = sources();
    const items = fixtureItems();
    const first = items.items.find((i) => standingOf(i.work) === 'owner') as Item;
    const extra: Wait = { key: 'limit', why: 'The cell ran 3 times in a row, its limit.', ends: { kind: 'letGo', cell: 'plan' } };
    const more: Items = {
      ...items,
      items: items.items.map((i) => (i === first ? { ...i, work: { ...i.work, waits: [...(i.work.waits ?? []), extra] } } : i)),
    };
    s.works.loadItems = async () => more;
    const { container } = draw(<SystemView period="week" />, s);
    await settle();
    const works = more.items.filter((i) => standingOf(i.work) === 'owner');
    const waiting = waitingCount(works.map((i) => i.work));
    expect(waiting).toBe(works.length + 1);
    const tile = [...part(container, 'Now').querySelectorAll('.tile')].find(
      (t) => t.querySelector('.label')?.textContent === waitCounted(waiting),
    );
    expect(`${tile?.querySelector('.tile-value')?.textContent} ${tile?.querySelector('.label')?.textContent}`).toBe(
      waitCount(waiting),
    );
  });

  /// A wait every work shares, the check of the outside having found it down, as the latest check says it.
  const sharedWait = (holds: boolean): Outage => ({
    service: 'agents',
    failure: 'cannotRun',
    why: 'Not logged in',
    work: 3,
    run: 2,
    round: 1,
    since: 0,
    checked: {
      at: 0,
      taken: 2,
      found: holds
        ? { kind: 'down', failure: 'cannotRun', why: 'It is not signed in, as `claude auth status` says.' }
        : { kind: 'well' },
    },
    until: { kind: holds ? 'checked' : 'well', at: 0 },
    holds,
    mayMove: false,
  });

  it('says none where the wait every work shares holds no work, in no colour of a problem', async () => {
    const s = sources();
    const items = fixtureItems();
    s.works.loadItems = async () => ({ ...items, outage: sharedWait(false) });
    const { container } = draw(<SystemView period="week" />, s);
    await settle();
    const shared = part(container, 'Now').querySelectorAll('dd')[2] as HTMLElement;
    expect(shared.textContent).toBe('none');
    expect(shared.classList.contains('failed')).toBe(false);
  });

  it("says the wait every work shares in sentences, the names and values it marks in the monospace", async () => {
    const s = sources();
    const items = fixtureItems();
    s.works.loadItems = async () => ({ ...items, outage: sharedWait(true) });
    const { container } = draw(<SystemView period="week" />, s);
    await settle();
    const shared = part(container, 'Now').querySelectorAll('dd')[2] as HTMLElement;
    expect([...shared.querySelectorAll('code')].map((c) => c.textContent)).toEqual(['claude auth status']);
    expect(shared.textContent).not.toContain('`');
    expect(shared.textContent).toMatch(
      / the check found: It is not signed in, as claude auth status says\. No agent's round begins until it runs again\.$/,
    );
    expect(shared.textContent).not.toContain('Not logged in');
    expect(shared.textContent).not.toContain('W3');
  });

  it('unfolds the works a count counts, and folds them again', async () => {
    const s = sources();
    const { container } = draw(<SystemView period="week" />, s);
    await settle();
    const now = part(container, 'Now');
    const halted = [...now.querySelectorAll('button.tile')].find((t) => t.textContent?.includes('Halted')) as HTMLElement;
    act(() => halted.click());
    expect(halted.getAttribute('aria-expanded')).toBe('true');
    const list = now.querySelector('table[aria-label="Halted"]');
    const expected = fixtureItems()
      .items.filter((i) => standingOf(i.work) === 'halted')
      .map((i) => `W${i.work.number}`);
    expect(texts(list!, 'tbody td:first-child')).toEqual(expected);
    act(() => halted.click());
    expect(now.querySelector('table[aria-label="Halted"]')).toBeNull();
  });

  it('counts over the span picked, kept in the address, and reads the span again when a work changes', async () => {
    const s = sources();
    const { container, render } = draw(<SystemView period="week" />, s);
    await settle();
    expect(s.asked).toHaveLength(1);
    expect(s.asked[0]).toHaveLength(1);
    expect(new Date(s.asked[0][0]).getDay()).toBe(1);
    const produced = part(container, 'Produced');
    expect(produced.querySelector('.tile-value')?.textContent).toBe(String(fixtureSystem(s.asked[0]).merged.length));
    const spent = part(container, 'Spent');
    expect(texts(spent, '.tile .label')).toEqual(['Cost', 'Rounds', 'Under way', 'Tokens', 'Read from the cache']);
    expect(texts(spent, 'h4')).toContain('By cell');
    const wrong = part(container, 'Gone wrong');
    expect(wrong.textContent).toContain("The agent's program could not start.");
    // The spans are links, each to its own address, the one in use marked.
    const links = [...container.querySelectorAll('a.span-link')];
    expect(links.map((a) => [a.textContent, a.getAttribute('href'), a.getAttribute('aria-current')])).toEqual([
      ['This week', '#/system/week', 'true'],
      ['4 weeks', '#/system/4w', null],
      ['All time', '#/system/all', null],
    ]);
    render(<SystemView period="all" />);
    await settle();
    // All time from the week the earliest item was added in.
    const earliest = Math.min(...fixtureItems().items.map((i) => i.added));
    expect(s.asked.at(-1)?.[0]).toBeLessThanOrEqual(earliest);
    expect(s.asked.at(-1)?.[0]).toBeGreaterThan(earliest - 7 * 24 * 60 * 60 * 1000);
    expect(
      [...container.querySelectorAll('a.span-link')].map((a) => [a.textContent, a.getAttribute('aria-current')]),
    ).toEqual([
      ['This week', null],
      ['4 weeks', null],
      ['All time', 'true'],
    ]);
    const before = s.asked.length;
    act(() => s.changed({ works: [41], items: false, copies: false, accounts: false, observations: false }));
    await settle();
    expect(s.asked.length).toBe(before + 1);
    act(() => s.changed({ works: [], items: false, copies: true, accounts: false, observations: false }));
    await settle();
    expect(s.asked.length).toBe(before + 1);
  });

  it("cuts the week anew once Monday's midnight has passed, and reads the span then", async () => {
    vi.useFakeTimers({ toFake: ['Date', 'setTimeout', 'clearTimeout'] });
    try {
      // A Sunday a minute before midnight, in the browser's time zone.
      vi.setSystemTime(new Date(2026, 10, 15, 23, 59));
      const s = sources();
      draw(<SystemView period="week" />, s);
      await act(async () => {
        await vi.advanceTimersByTimeAsync(0);
      });
      expect(s.asked.at(-1)).toEqual([new Date(2026, 10, 9).getTime()]);
      await act(async () => {
        await vi.advanceTimersByTimeAsync(2 * 60 * 1000);
      });
      expect(s.asked.at(-1)).toEqual([new Date(2026, 10, 16).getTime()]);
    } finally {
      vi.useRealTimers();
    }
  });

  /// The fixtures' round under way, cloned into four works, each with what its calls have come to in one of the cases a
  /// figure is in: past its lines, within them, with no line, and none told yet; the works' own round has none.
  function fourRounds(s: ReturnType<typeof sources>) {
    const items = fixtureItems();
    const base = items.items.find((i) => i.work.now.kind === 'round') as Item;
    const clones = [500, 501, 502, 503].map((n) => ({ ...base, number: n + 1000, work: { ...base.work, number: n } }));
    s.works.loadItems = async () => ({ ...items, items: [...items.items, ...clones] });
    const now = base.work.now;
    if (now.kind !== 'round') throw new Error('a round');
    const at = { run: now.run, round: now.round };
    const line = (value: number, scope: 'cell' | 'flow') => ({ value, samples: 12, scope });
    s.system.loadUnderWay = async () => {
      s.goingRead.count += 1;
      return [
        {
          work: 500,
          ...at,
          context: 182_400,
          added: { value: 96_000, line: line(60_000, 'cell'), past: true },
          perCall: { value: 8_000, line: line(5_000, 'cell'), past: true },
          cost: { value: 1_240_000, line: line(900_000, 'flow'), past: true },
        },
        {
          work: 501,
          ...at,
          context: 64_000,
          added: { value: 21_000, line: line(60_000, 'cell'), past: false },
          perCall: { value: 3_000, line: line(5_000, 'cell'), past: false },
          cost: { value: 310_000, line: line(900_000, 'flow'), past: false },
        },
        {
          work: 502,
          ...at,
          context: 12_000,
          added: { value: 4_000, line: null, past: false },
          perCall: { value: 2_000, line: null, past: false },
          cost: { value: 52_000, line: null, past: false },
        },
        { work: 503, ...at, context: null, added: null, perCall: null, cost: null },
      ];
    };
  }

  const rowOf = (container: ParentNode, work: number) => {
    const rows = [...part(container, 'Now').querySelectorAll('table')[0].querySelectorAll('tbody tr')];
    const row = rows.find((r) => r.querySelector('td a')?.textContent === `W${work}`);
    if (!row) throw new Error(`no row of W${work}`);
    return row;
  };

  it('says of each round under way what its calls have come to, with the line beside each figure, and marks a figure past its line', async () => {
    const s = sources();
    fourRounds(s);
    const { container } = draw(<SystemView period="week" />, s);
    await settle();
    // Past: each figure beside its line, in the colour of a warning.
    const past = rowOf(container, 500);
    expect(texts(past, 'td').slice(5)).toEqual(['182k', '96k of 60k', '8.0k of 5.0k', '$1.24 of $0.90']);
    expect(past.querySelectorAll('.warn-text')).toHaveLength(3);
    const added = past.querySelectorAll('td')[6].querySelector('span') as HTMLElement;
    expect(added.getAttribute('title')).toContain("The tokens the round's context has grown by since it began.");
    expect(added.getAttribute('title')).toContain(
      'stops a round only where its growth over the round and its growth for each call',
    );
    expect(added.getAttribute('title')).toContain('Its line is 60k, which 12 rounds of its cell give; it is past it.');
    const cost = past.querySelectorAll('td')[8].querySelector('span') as HTMLElement;
    expect(cost.getAttribute('title')).toContain("12 agents' rounds of its flow give");
    // Within: the line beside each, in no colour.
    const within = rowOf(container, 501);
    expect(texts(within, 'td').slice(5)).toEqual(['64k', '21k of 60k', '3.0k of 5.0k', '$0.31 of $0.90']);
    expect(within.querySelectorAll('.warn-text')).toHaveLength(0);
    expect(within.querySelectorAll('td')[6].querySelector('span')?.getAttribute('title')).toContain('it is within it.');
    // No line yet: the figure alone, and the title says why.
    const alone = rowOf(container, 502);
    expect(texts(alone, 'td').slice(5)).toEqual(['12k', '4.0k', '2.0k', '$0.05']);
    expect(alone.querySelectorAll('.warn-text')).toHaveLength(0);
    expect(alone.querySelectorAll('td')[6].querySelector('span')?.getAttribute('title')).toContain('No line yet');
    // No call told yet, as of an engine's act, which makes none: said so, not left empty or shown as nought.
    const none = rowOf(container, 503);
    expect(texts(none, 'td').slice(5)).toEqual(['none yet', 'none yet', 'none yet', 'none yet']);
    expect(none.querySelectorAll('td')[5].querySelector('.muted')?.getAttribute('title')).toContain(
      'Told once the round has made a call',
    );
  });

  it('reads what the rounds have come to when the stream says it changed, for no other change and not while hidden', async () => {
    const s = sources();
    const { render } = draw(<SystemView period="week" />, s);
    await settle();
    expect(s.goingRead.count).toBe(1);
    const read = s.asked.length;
    const only = { works: [], items: false, copies: false, accounts: false, observations: false, going: true };
    act(() => s.changed(only));
    await settle();
    expect(s.goingRead.count).toBe(2);
    expect(s.asked.length, 'what the span holds is not read again for it').toBe(read);
    // A change of a work reads the span and not what the rounds have come to.
    act(() => s.changed({ ...only, works: [41], going: false }));
    await settle();
    expect(s.goingRead.count).toBe(2);
    expect(s.asked.length).toBe(read + 1);
    // One that says both is for both.
    act(() => s.changed({ ...only, works: [41] }));
    await settle();
    expect(s.goingRead.count).toBe(3);
    expect(s.asked.length).toBe(read + 2);
    // The stream opening again, when changes may have been missed, reads it too.
    act(() => s.opened());
    await settle();
    expect(s.goingRead.count).toBe(4);
    // Hidden, the view is told nothing; shown again, it reads once what it missed.
    render(<SystemView period="week" active={false} />);
    await settle();
    act(() => s.changed(only));
    await settle();
    expect(s.goingRead.count).toBe(4);
    render(<SystemView period="week" active />);
    await settle();
    expect(s.goingRead.count).toBe(5);
    act(() => s.changed(only));
    await settle();
    expect(s.goingRead.count).toBe(6);
  });

  it('says what a merge cost on average as what the whole span spent for each merge, beside what the merged works cost themselves', async () => {
    const s = sources();
    const { container } = draw(<SystemView period="week" />, s);
    await settle();
    const system = fixtureSystem(s.asked[0]);
    expect(system.merged.length).toBeGreaterThan(0);
    const tileOf = (root: ParentNode) =>
      [...part(root, 'Produced').querySelectorAll('.tile')].find(
        (t) => t.querySelector('.label')?.textContent === "A merge's cost, on average",
      ) as HTMLElement;
    const per = system.spent.usdMicros / system.merged.length;
    const own = system.merged.reduce((sum, m) => sum + m.spent.usdMicros, 0) / system.merged.length;
    expect(per).not.toBe(own);
    const dollar = (micros: number) => (micros > 0 && micros < 10_000 ? '<$0.01' : `$${(micros / 1_000_000).toFixed(2)}`);
    const tile = tileOf(container);
    expect(tile.querySelector('.tile-value')?.firstChild?.textContent).toBe(dollar(per));
    expect(tile.querySelector('.tile-more')?.textContent?.trim()).toBe(`the merged works' own: ${dollar(own)}`);
    expect(tile.getAttribute('title')).toContain('for each work merged in it');
    // No merge in the span: none, and nothing beside it.
    s.system.loadSystem = async (weeks) => ({ ...fixtureSystem(weeks), merged: [] });
    const none = draw(<SystemView period="4w" />, s);
    await settle();
    const empty = tileOf(none.container);
    expect(empty.querySelector('.tile-value')?.textContent).toBe('none');
  });

  it("shows what each project spent before the cells and the models, the costliest first as given, each project's share of the totals, which count every project", async () => {
    const s = sources();
    const byProject: ProjectSpent[] = [
      { project: 'wf-agents', spent: spending(3_000_000, 12, 3_600_000) },
      { project: 'Whitefoot', spent: spending(1_000_000, 5, 600_000) },
      { project: null, spent: spending(500_000, 2, 90_000) },
    ];
    s.system.loadSystem = async (weeks) => ({ ...fixtureSystem(weeks), spent: spending(4_500_000, 19, 4_290_000), byProject });
    const { container } = draw(<SystemView period="week" />, s);
    await settle();
    const spent = part(container, 'Spent');
    expect(texts(spent, 'h4')).toEqual(['By project', 'By cell', 'By model', 'The costliest works']);
    const table = tableUnder(spent, 'By project');
    expect(texts(table, 'thead th')).toEqual(['Project', 'Rounds', 'Under way', 'Cost', 'Share']);
    expect(texts(table, 'thead th.num')).toEqual(['Rounds', 'Under way', 'Cost', 'Share']);
    const rows = [...table.querySelectorAll('tbody tr')].map((r) => texts(r, 'td'));
    expect(rows).toEqual([
      ['wf-agents', '12', '1h 0m', '$3.00', '67%'],
      ['Whitefoot', '5', '10m 0s', '$1.00', '22%'],
      ['none', '2', '1m 30s', '$0.50', '11%'],
    ]);
    // A project by its name in the monospace, whole on the pointer's tip; the items of none muted, saying why.
    const names = [...table.querySelectorAll('tbody td:first-child > span')];
    expect(names[0].classList.contains('mono')).toBe(true);
    expect(names[0].getAttribute('title')).toBe('wf-agents');
    expect(names[2].classList.contains('muted')).toBe(true);
    expect(names[2].getAttribute('title')).toBe('Items added before items had projects belong to none');
    // The totals at the top are over every project.
    const cost = [...spent.querySelectorAll('.tile')].find((t) => t.querySelector('.label')?.textContent === 'Cost');
    expect(cost?.querySelector('.tile-value')?.textContent).toBe('$4.50');
  });

  it('shows the one project as all of what was spent, and no table by project where nothing was spent', async () => {
    const s = sources();
    const one = spending(2_000_000, 7, 60_000);
    s.system.loadSystem = async (weeks) => ({ ...fixtureSystem(weeks), spent: one, byProject: [{ project: 'wf-agents', spent: one }] });
    const { container } = draw(<SystemView period="week" />, s);
    await settle();
    const rows = [...tableUnder(part(container, 'Spent'), 'By project').querySelectorAll('tbody tr')];
    expect(rows.map((r) => texts(r, 'td'))).toEqual([['wf-agents', '7', '1m 0s', '$2.00', '100%']]);
    s.system.loadSystem = async (weeks) => ({ ...fixtureSystem(weeks), spent: spending(0, 0, 0), byProject: [], byWork: [] });
    const none = draw(<SystemView period="4w" />, s);
    await settle();
    expect(texts(part(none.container, 'Spent'), 'h4')).not.toContain('By project');
  });

  it("names the project of each round's item among the rounds under way, and none, muted, for an item of none", async () => {
    const s = sources();
    const items = fixtureItems();
    const base = items.items.find((i) => i.work.now.kind === 'round') as Item;
    expect(base.project).not.toBeNull();
    const clones = [
      { ...base, number: 1600, project: 'Whitefoot', work: { ...base.work, number: 600 } },
      { ...base, number: 1601, project: null, work: { ...base.work, number: 601 } },
    ];
    s.works.loadItems = async () => ({ ...items, items: [...items.items, ...clones] });
    const { container } = draw(<SystemView period="week" />, s);
    await settle();
    const under = part(container, 'Now').querySelectorAll('table')[0];
    expect(texts(under, 'thead th')).toEqual(['Work', 'Item', 'Project', 'Cell', 'Since', ...CALLS]);
    expect(texts(under, 'thead th.num')).toEqual(CALLS);
    const project = (work: number) => rowOf(container, work).querySelectorAll('td')[2].querySelector('span') as HTMLElement;
    expect([base.work.number, 600, 601].map((w) => project(w).textContent)).toEqual([base.project, 'Whitefoot', 'none']);
    expect(project(600).classList.contains('mono')).toBe(true);
    expect(project(601).classList.contains('muted')).toBe(true);
    expect(project(601).getAttribute('title')).toBe('Items added before items had projects belong to none');
  });

  it('spends, in the fixtures, over as many projects as the address asks for, else three and the items of none, adding up to what was spent', async () => {
    const added = (list: ProjectSpent[]) =>
      list.reduce(
        (sum, p) => ({
          usdMicros: sum.usdMicros + p.spent.usdMicros,
          inputTokens: sum.inputTokens + p.spent.inputTokens,
          cacheWrittenTokens: sum.cacheWrittenTokens + p.spent.cacheWrittenTokens,
          unsplitTokens: sum.unsplitTokens + p.spent.unsplitTokens,
          cachedTokens: sum.cachedTokens + p.spent.cachedTokens,
          outputTokens: sum.outputTokens + p.spent.outputTokens,
          workingMs: sum.workingMs + p.spent.workingMs,
          rounds: sum.rounds + p.spent.rounds,
        }),
        spending(0, 0, 0),
      );
    const where = window.location.href;
    try {
      for (const [address, named, none] of [
        ['/?fixtures', 3, 1],
        ['/?fixtures&projects=1', 1, 0],
        ['/?fixtures&projects=12', 12, 0],
      ] as const) {
        window.history.replaceState(null, '', address);
        const system = await FIXTURES.loadSystem([Date.now()]);
        const projects = system.byProject.map((p) => p.project);
        expect(projects.filter((p) => p !== null), address).toHaveLength(named);
        expect(new Set(projects).size, address).toBe(projects.length);
        expect(projects.filter((p) => p === null), address).toHaveLength(none);
        expect(added(system.byProject), address).toEqual(system.spent);
        const costs = system.byProject.map((p) => p.spent.usdMicros);
        expect(costs, address).toEqual([...costs].sort((a, b) => b - a));
      }
    } finally {
      window.history.replaceState(null, '', where);
    }
    // With one project asked for, the project of the works' fixtures' round under way is that one alone.
    const running = fixtureItems(1).items.find((i) => i.work.now.kind === 'round');
    expect(fixtureSystem([Date.now()], 1).byProject.map((p) => p.project)).toEqual([running?.project]);
  });
});
