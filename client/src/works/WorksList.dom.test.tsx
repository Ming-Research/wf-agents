// @vitest-environment happy-dom
import '../dom-setup';

import { act, Profiler, type ProfilerOnRenderCallback, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import { useBarItems } from '../Engine';
import type { Changed, Items } from '../works-api';
import { fixtureCopies, fixtureItems } from './fixtures';
import { keptSame, type Source, SourceContext } from './source';
import { WorksList } from './WorksList';

let unmounts: (() => void)[] = [];
afterEach(() => {
  for (const u of unmounts) u();
  unmounts = [];
  document.body.innerHTML = '';
});

/// How many works the list is measured with: several hundred, as the owner's engine comes to hold.
const MANY = 500;

/// The fixtures' items, again and again under new numbers, until there are `n`: each read gives new objects, as a read
/// of the interface does.
function manyItems(n: number, changedWork: number | null = null): Items {
  const base = fixtureItems().items;
  const items = Array.from({ length: n }, (_, i) => {
    const item = structuredClone(base[i % base.length]);
    item.number = 1000 + i;
    item.title = `${item.title} ${i}`;
    item.work.number = 2000 + i;
    item.work.item = item.number;
    if (item.work.number === changedWork) {
      item.work.last = (item.work.last ?? 0) + 60_000;
      item.work.now = { kind: 'round', cell: 'review', run: 9, round: 1, since: item.work.last };
    }
    return item;
  });
  return { items, outage: null };
}

/// A source whose items are `n` works, one of them changed once `changed` is set; it counts its reads of the items, and
/// tells the parts that listen of a change when the test says.
interface ManySource {
  reads: number;
  changedWork: number | null;
  changed: (c: Changed) => void;
  source: Source;
}

function manySource(n: number): ManySource {
  const listening = new Set<{ onChanged: (c: Changed) => void; onOpened: () => void }>();
  const s: ManySource = {
    reads: 0,
    changedWork: null,
    changed: (c: Changed) => listening.forEach((l) => l.onChanged(c)),
    source: {
      loadCopies: async () => fixtureCopies(),
      loadItems: async (): Promise<Items> => {
        s.reads += 1;
        return manyItems(n, s.changedWork);
      },
      loadWork: async () => {
        throw new Error('not read here');
      },
      loadVersion: async () => {
        throw new Error('not read here');
      },
      loadRound: async () => {
        throw new Error('not read here');
      },
      loadTold: async () => {
        throw new Error('not read here');
      },
      loadCall: async () => {
        throw new Error('not read here');
      },
      act: async () => null,
      changes: (onChanged, onOpened) => {
        const l = { onChanged, onOpened };
        listening.add(l);
        return () => listening.delete(l);
      },
    },
  };
  return s;
}

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

function draw(element: ReactNode, source: Source): HTMLElement {
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  act(() => root.render(<SourceContext.Provider value={source}>{element}</SourceContext.Provider>));
  unmounts.push(() => act(() => root.unmount()));
  return container;
}

/// The bar's reading of the items, which the test draws beside the list.
function BarProbe() {
  const { items } = useBarItems();
  return <span className="bar-probe">{items?.items.length ?? 0}</span>;
}

describe('the list of works with several hundred works', () => {
  it('draws them all, and a change of one work draws its row again and not the others', async () => {
    const s = manySource(MANY);
    const commits: { phase: string; ms: number }[] = [];
    const onRender: ProfilerOnRenderCallback = (_id, phase, actualDuration) => {
      commits.push({ phase, ms: actualDuration });
    };
    const first = performance.now();
    const page = draw(
      <Profiler id="works" onRender={onRender}>
        <WorksList />
      </Profiler>,
      s.source,
    );
    await settle();
    const drawn = performance.now() - first;
    expect(page.querySelectorAll('tbody tr.work-row')).toHaveLength(MANY);
    const whole = commits.reduce((most, c) => Math.max(most, c.ms), 0);

    commits.length = 0;
    const changed = 2000 + 7;
    s.changedWork = changed;
    const before = performance.now();
    act(() => s.changed({ works: [changed], items: false, copies: false }));
    await settle();
    const again = performance.now() - before;
    const row = page.querySelector(`tr[data-work="${changed}"]`);
    expect(row?.querySelector('.work-now')?.textContent).toContain('Running review');
    const update = commits.reduce((sum, c) => sum + c.ms, 0);
    // Measured on the page's own machine: the first drawing of 500 works some hundred milliseconds, the change of
    // one a few; the bounds leave room for a slower machine, and the share for the rows left as they were.
    console.info(
      `${MANY} works: drawn in ${drawn.toFixed(0)} ms (rendering ${whole.toFixed(0)} ms); one changed, drawn again ` +
        `in ${again.toFixed(0)} ms (rendering ${update.toFixed(1)} ms)`,
    );
    expect(drawn).toBeLessThan(5000);
    expect(again).toBeLessThan(1000);
    expect(update).toBeLessThan(whole / 5);
  });

  it('reads the items once for the bar and the list, once for a burst of changes, and not for the copies', async () => {
    const s = manySource(20);
    draw(
      <>
        <BarProbe />
        <WorksList />
      </>,
      s.source,
    );
    await settle();
    expect(s.reads).toBe(1);
    act(() => {
      for (let i = 0; i < 10; i += 1) s.changed({ works: [2000 + i], items: false, copies: false });
    });
    await settle();
    // The first change reads at once; the nine that came while it was under way, once after it.
    expect(s.reads).toBe(3);
    act(() => s.changed({ works: [], items: false, copies: true }));
    await settle();
    expect(s.reads).toBe(3);
  });

  it('keeps each item that did not change as the object it was, and the whole list where none did', () => {
    const before = manyItems(30);
    expect(keptSame(before, manyItems(30))).toBe(before);
    const after = keptSame(before, manyItems(30, 2003));
    expect(after).not.toBe(before);
    expect(after.items.filter((item, i) => item !== before.items[i]).map((item) => item.work.number)).toEqual([2003]);
  });
});
