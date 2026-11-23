// The console in a real browser, opened at an item the address names, and moved to another by a link in a text. The
// item may be of a kind the tree does not show (its box is not ticked): the box is ticked and the item selected, for
// the first address and for each address that follows while the page stays (a link to an item in the window's text
// changes the address only). The reference is the design tree: decision an-item-is-found-from-the-trees-corner (a pick
// selects the item, its kind ticked) and decision the-boxes-choose-the-items-shown (Open, Done and Cancelled).

import '../styles/tokens.css';
import '../styles/parts.css';

import { createRoot } from 'react-dom/client';
import { describe, expect, it } from 'vitest';

import { fixtureSource } from './fixtures';
import { ConsolePage } from './index';
import type { SnapScene } from './snap';

/// The console page in a container of its own, with a way to name another item as the address does.
function opened(named: number) {
  localStorage.clear();
  const container = document.createElement('div');
  container.style.cssText = 'position: fixed; inset: 0; display: flex';
  document.body.append(container);
  delete (window as { consoleScene?: SnapScene }).consoleScene;
  const root = createRoot(container);
  const source = fixtureSource();
  const name = (n: number) => root.render(<ConsolePage source={source} named={n} />);
  name(named);
  return {
    name,
    title: () => container.querySelector('.drawer-title')?.textContent ?? null,
    ticked: (box: string) =>
      [...container.querySelectorAll<HTMLLabelElement>('.console-shows label')]
        .find((l) => l.textContent === box)
        ?.querySelector('input')?.checked,
    close: () => {
      root.unmount();
      container.remove();
      localStorage.clear();
    },
  };
}

describe('the console page, moved by the address to an item of a kind the tree does not show', () => {
  it('ticks Done for a merged item, and not Cancelled', async () => {
    const page = opened(10);
    try {
      await expect.poll(page.title).toBe('Show the cells a work went through');
      expect(page.ticked('Done')).toBe(true);
      expect(page.ticked('Cancelled')).toBe(false);
    } finally {
      page.close();
    }
  });

  it('follows an address that names a cancelled item while the page stays, and keeps the box ticked after it', async () => {
    const page = opened(12);
    try {
      await expect.poll(page.title).toBe('Show the cost of a work');
      expect(page.ticked('Cancelled')).toBe(false);
      page.name(35);
      await expect.poll(page.title).toBe('Try the tree as a flat list');
      expect(page.ticked('Cancelled')).toBe(true);
      page.name(12);
      await expect.poll(page.title).toBe('Show the cost of a work');
      expect(page.ticked('Cancelled')).toBe(true);
    } finally {
      page.close();
    }
  });

  it('draws the tree and selects nothing for an address that names no item', async () => {
    const page = opened(9999);
    try {
      await expect.poll(() => document.querySelector('.console-shows label') !== null).toBe(true);
      await new Promise((resolve) => setTimeout(resolve, 300));
      expect(page.title() ?? '').toBe('');
      expect(page.ticked('Done')).toBe(false);
      expect(page.ticked('Cancelled')).toBe(false);
    } finally {
      page.close();
    }
  });
});
