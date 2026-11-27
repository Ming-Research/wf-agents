// The Waited part of the System view drawn in a real browser (Chromium), whose layout is its own: the five causes and the
// four other numbers stand in rows of tiles that keep inside the window, and the week table of nine columns fits it with
// no cell cut, as a table squeezed until it cannot be read would be (principle `room-goes-to-content`). A stand-in for
// the browser has no layout, so only here can a cell cut at its edge, or a table wider than its window, be seen.
//
// A row of tiles wraps where it is wider than the window: that is the rule of every row of tiles on the System view
// (`.tiles` in system.css, `flex-wrap: wrap`, which this work did not change), the page fitting a narrow window by
// letting a row fold rather than a number be cut or the window scroll sideways. So the test of the narrow window asks
// only that no tile reach beyond the window; in a window wide enough for a row, each row stands on one line.

import '../styles/tokens.css';
import '../styles/parts.css';
import '../styles/system.css';

import { flushSync } from 'react-dom';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import { fixtureSystem } from './fixtures';
import { WaitedPart } from './SystemView';

const WEEK = 7 * 24 * 3_600_000;

describe('the Waited part of the System view', () => {
  let host: HTMLDivElement;
  afterEach(() => host.remove());

  /// The part drawn over three weeks in a host of this width, with the weeks' table in it.
  function drawn(width = 1100) {
    host = document.createElement('div');
    host.style.width = `${width}px`;
    document.body.append(host);
    const now = Date.now();
    const root = createRoot(host);
    flushSync(() => root.render(<WaitedPart s={fixtureSystem([now - 2 * WEEK, now - WEEK, now])} />));
    return root;
  }

  it('keeps its tiles inside the window and lays its week table across it with no cell cut at its edge', () => {
    const root = drawn();
    const rows = [...host.querySelectorAll<HTMLElement>('.tiles')];
    expect(rows.map((row) => row.querySelectorAll('.tile').length)).toEqual([5, 4]);
    // A row of tiles wraps where it is wider than the window, which it may; no tile reaches beyond the window's edges.
    const room = host.getBoundingClientRect();
    for (const tile of host.querySelectorAll<HTMLElement>('.tile')) {
      const box = tile.getBoundingClientRect();
      expect(box.left, `the tile "${tile.textContent}" begins inside the window`).toBeGreaterThanOrEqual(room.left);
      expect(box.right, `the tile "${tile.textContent}" ends inside the window`).toBeLessThanOrEqual(room.right);
    }
    const table = host.querySelector<HTMLTableElement>('table.view-table');
    expect(table, 'the week table is drawn over more than one week').not.toBeNull();
    expect(table!.scrollWidth, 'the table fits the window').toBeLessThanOrEqual(host.clientWidth);
    expect(table!.querySelectorAll('thead th').length).toBe(9);
    for (const cell of table!.querySelectorAll<HTMLElement>('td')) {
      expect(cell.scrollWidth, `the cell "${cell.textContent}" is cut at its edge`).toBeLessThanOrEqual(cell.clientWidth);
    }
    root.unmount();
  });

  it('stands the tiles of each row side by side in a window wide enough for the row', () => {
    const root = drawn(1600);
    const rows = [...host.querySelectorAll<HTMLElement>('.tiles')];
    expect(rows.map((row) => row.querySelectorAll('.tile').length)).toEqual([5, 4]);
    for (const row of rows) {
      const tops = new Set([...row.querySelectorAll<HTMLElement>('.tile')].map((tile) => tile.getBoundingClientRect().top));
      expect(tops.size, `the tiles of the row of ${row.children.length} stand side by side, none wrapped under another`).toBe(1);
    }
    root.unmount();
  });
});
