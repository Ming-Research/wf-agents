// The replies of the cells the owner has not read, in a real browser (Chromium), where layout and the page's visibility
// are the browser's own: m and M walk the items by the replies not read and bring one into view of the item's window; a
// reply that has stood in full view of the window for a moment while the page is seen is read and told to the engine,
// one that is not in view, or while the page is hidden, is not, and a reply taller than the window is read once the owner
// has read to its end.

import '../styles/tokens.css';
import '../styles/parts.css';
import './console.css';

import { createRoot } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { outlineOf } from '../testing';
import type { Block, Flow } from '../api';
import type { Act, Changed } from '../works-api';
import { FLOW, fixtureSource } from './fixtures';
import { ConsolePage } from './index';
import type { Source } from './source';

/// The research's own words, which the engine's source of the owner's words writes and the research writes too.
const SPOKEN: Flow = {
  ...FLOW,
  cells: FLOW.cells.map((c) => (c.id === 'research' ? { ...c, outputs: [...c.outputs, 'research_words'] } : c)),
  blocks: [
    ...FLOW.blocks,
    {
      name: 'research_words',
      values: null,
      default: null,
      what: null,
      writtenBy: { cells: ['research'], people: false, note: false, outside: 'words' },
      readBy: { cells: ['research'], conditions: [], tests: [], shown: false },
    } satisfies Block,
  ],
};

/// How many replies each item has, by its number: the first short, the others longer than the window.
const REPLIES: Record<number, number> = { 1: 2, 7: 1 };
const LONG = Array.from({ length: 120 }, (_, i) => `Paragraph ${i + 1} of a reply that is longer than the window.`).join('\n\n');
const CONTENT = (number: number): string => (number === 1 ? 'Please check the plan.' : number === 2 ? 'A short reply.' : LONG);

/// The made-up items with replies of the research's cell, which the owner reads one at a time: what they have read is
/// kept, the counts of the items follow it, and each act of reading is kept for the test to see.
function repliesSource(): { source: Source; acts: [number, Act][] } {
  const base = fixtureSource();
  const read = new Set<string>();
  const acts: [number, Act][] = [];
  const listeners = new Set<(c: Changed) => void>();
  const itemOf = new Map<number, number>();
  /// The versions of the research's words in the work of an item: the owner's words, then its replies.
  const versionsOf = (work: number) => {
    const n = REPLIES[itemOf.get(work) ?? 0] ?? 0;
    return Array.from({ length: n === 0 ? 0 : n + 1 }, (_, k) => ({
      at: 10 + k,
      number: k + 1,
      time: Date.now() - 1000 * (n + 1 - k),
      by: k === 0 ? ({ kind: 'owner' } as const) : ({ kind: 'run', run: 3, cell: 'research' } as const),
      meaning: k + 1,
      value: null,
      saidTo: null,
      chars: 20,
      empty: false,
      reply: k > 0,
      read: read.has(`${work}/${k + 1}`),
    }));
  };
  const told = (work: number) => {
    const changed: Changed = { works: [work], items: true, copies: false, accounts: false, observations: false };
    window.setTimeout(() => listeners.forEach((on) => on(changed)), 0);
  };
  const source: Source = {
    ...base,
    items: {
      loadItems: async (since) => {
        const items = await base.items.loadItems(since);
        for (const i of items.items) itemOf.set(i.work.number, i.number);
        return {
          ...items,
          items: items.items.map((i) => ({
            ...i,
            work: { ...i.work, repliesUnread: versionsOf(i.work.number).filter((v) => v.reply && !v.read).length },
          })),
        };
      },
      changes: (onChanged, onOpened) => {
        listeners.add(onChanged);
        const stop = base.items.changes(onChanged, onOpened);
        return () => {
          listeners.delete(onChanged);
          stop();
        };
      },
    },
    loadWork: async (work) => {
      const page = await base.loadWork(work);
      return { ...page, flow: SPOKEN, outline: outlineOf(SPOKEN), blocks: [{ name: 'research_words', versions: versionsOf(work) }] };
    },
    loadVersion: (work, block, number) => Promise.resolve({ block, number, content: CONTENT(number) }),
    act: (work, what) => {
      if (what.kind !== 'read') return base.act(work, what);
      acts.push([work, what]);
      for (const v of what.versions) read.add(`${work}/${v.version}`);
      told(work);
      return Promise.resolve(null);
    },
  };
  return { source, acts };
}

describe('the replies the owner has not read, in the console', () => {
  let container: HTMLDivElement;
  let unmount: () => void;
  let acts: [number, Act][];
  const hidden = (is: boolean) => {
    Object.defineProperty(document, 'visibilityState', { value: is ? 'hidden' : 'visible', configurable: true });
    document.dispatchEvent(new Event('visibilitychange'));
  };
  beforeEach(() => {
    localStorage.clear();
    window.location.hash = '';
    container = document.createElement('div');
    container.style.cssText = 'position: fixed; inset: 0; display: flex';
    document.body.append(container);
    delete (window as { consoleScene?: unknown }).consoleScene;
    const made = repliesSource();
    acts = made.acts;
    const root = createRoot(container);
    root.render(<ConsolePage source={made.source} />);
    unmount = () => root.unmount();
  });
  afterEach(() => {
    unmount();
    container.remove();
    localStorage.clear();
    window.location.hash = '';
    delete (document as { visibilityState?: unknown }).visibilityState;
  });

  const key = (k: string) =>
    document.body.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true }));
  const inAddress = () => Number(/#\/console\/items\/(\d+)/.exec(window.location.hash)?.[1] ?? Number.NaN);
  const pane = () => container.querySelector<HTMLElement>('.cw-item .drawer-body');
  const replies = () => [...container.querySelectorAll<HTMLElement>('.cw-message[data-reply]')];
  const unread = () => replies().filter((r) => r.hasAttribute('data-unread'));
  const strip = () => container.querySelector('.cw-fresh span')?.textContent ?? null;
  /// How far the top of a reply is below the top of the window's pane.
  const below = (el: HTMLElement) => el.getBoundingClientRect().top - (pane()?.getBoundingClientRect().top ?? 0);
  const drawn = () =>
    expect.poll(() => (window as { consoleScene?: { stats: { items: number } } }).consoleScene?.stats.items ?? 0).toBeGreaterThan(0);
  const sleep = (ms: number) => new Promise<void>((done) => setTimeout(done, ms));

  it('reads a reply that has stood in view for a moment while the page is seen, and none out of view or while it is hidden, a long one once its end is read to', async () => {
    await drawn();
    hidden(true);
    // m goes to the first item with replies not read and brings its first into view; the page being hidden, none is read.
    key('m');
    await expect.poll(inAddress).toBe(1);
    await expect.poll(() => replies().length).toBe(2);
    await expect.poll(() => strip()).toBe('2 replies you have not read');
    await sleep(1500);
    expect(unread()).toHaveLength(2);
    expect(acts).toEqual([]);
    // Seen again, the first, which stands in the window whole, is read after a moment; the second, longer than the
    // window, of which only the top is in view, is not.
    hidden(false);
    await expect.poll(() => unread().length, { timeout: 10_000 }).toBe(1);
    expect(acts).toEqual([[expect.any(Number), { kind: 'read', versions: [{ block: 'research_words', version: 2 }] }]]);
    expect(strip()).toBe('1 reply you have not read');
    // m brings the long one to the top of the window; with its end out of view, it stays unread.
    key('m');
    await expect.poll(() => Math.abs(below(unread()[0]))).toBeLessThan(4);
    await sleep(1500);
    expect(unread()).toHaveLength(1);
    expect(acts).toHaveLength(1);
    // Read to its end, it is read.
    (pane() as HTMLElement).scrollTop = (pane() as HTMLElement).scrollHeight;
    await expect.poll(() => unread().length, { timeout: 10_000 }).toBe(0);
    expect(acts.map(([, a]) => a)).toEqual([
      { kind: 'read', versions: [{ block: 'research_words', version: 2 }] },
      { kind: 'read', versions: [{ block: 'research_words', version: 3 }] },
    ]);
    expect(strip()).toBeNull();
    // With nothing left in this item, m goes on to the next item that has a reply not read.
    key('m');
    await expect.poll(inAddress).toBe(7);
  });

  it('walks back with M from the last item with a reply not read, to the last reply of the item before, round from the first to the last', async () => {
    await drawn();
    // With nothing selected, M goes to the last item with replies not read, and its last.
    key('M');
    await expect.poll(inAddress).toBe(7);
    await expect.poll(() => replies().length).toBe(1);
    // Item 7 has no reply before it, so M goes to the item before, at its last reply, the long one, at the top.
    key('M');
    await expect.poll(inAddress).toBe(1);
    await expect.poll(() => replies().length).toBe(2);
    await expect.poll(() => Math.abs(below(replies()[1]))).toBeLessThan(4);
  });
});
