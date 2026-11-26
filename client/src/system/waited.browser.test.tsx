// The Waited part of the System view drawn in a real browser (Chromium), whose layout is its own: the five causes and the
// four other numbers stand in rows of tiles that share the window's width, and the week table of nine columns fits it
// with no cell cut, as a table squeezed until it cannot be read would be (principle `room-goes-to-content`). A stand-in
// for the browser has no layout, so only here can a cell cut at its edge, or a table wider than its window, be seen.

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

  /// The part drawn over three weeks in a host as wide as the window, with the weeks' table in it.
  function drawn() {
    host = document.createElement('div');
    host.style.width = '1100px';
    document.body.append(host);
    const now = Date.now();
    const root = createRoot(host);
    flushSync(() => root.render(<WaitedPart s={fixtureSystem([now - 2 * WEEK, now - WEEK, now])} />));
    return root;
  }

  it('lays its tiles in rows of the window and its week table across it with no cell cut at its edge', () => {
    const root = drawn();
    const rows = [...host.querySelectorAll<HTMLElement>('.tiles')];
    expect(rows.map((row) => row.querySelectorAll('.tile').length)).toEqual([5, 4]);
    for (const row of rows) {
      const tops = new Set([...row.querySelectorAll<HTMLElement>('.tile')].map((tile) => tile.getBoundingClientRect().top));
      expect(tops.size, 'the tiles of a row stand side by side, none wrapped under another').toBe(1);
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
});
