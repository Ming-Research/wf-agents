// The way back on the matrix in a real browser (Chromium), where the marks are laid out: its line and its arrow are in
// the accent, in light and in dark; and the arrow's point is on the edge of the mark it points to, a ring, a disc and a
// ring or the ring of `reads`, without reaching into it, at any zoom.

import './styles/tokens.css';
import './styles/parts.css';
import './styles/chart.css';

import { flushSync } from 'react-dom';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import type { Flow } from './api';
import { Chart } from './Chart';
import { matrix } from './matrix';
import { cell, everyAgent, flowOf } from './testing';

/// The colour the browser draws a token in, as it gives a colour.
function drawnIn(token: string): string {
  const probe = document.createElement('span');
  probe.style.color = `var(${token})`;
  document.body.append(probe);
  const colour = getComputedStyle(probe).color;
  probe.remove();
  return colour;
}

/// Three ways back, each to a cell with other marks or none: `ring` watches `a` alone; `both` writes and watches `b`;
/// `reads`, an agent's cell, watches `c_x` by `[every_agent]` and reads `d_x`, of the same name there, whose ring of
/// `reads` stands last on the name's row.
function flow(): Flow {
  const engine = { kind: 'engine' as const, line: null, task: null, act: 'merge' };
  return flowOf(
    [
      cell('ring', { ...engine, watches: ['a'] }),
      cell('both', { ...engine, watches: ['b'], outputs: ['b'] }),
      cell('reads', { reads: ['d_x'], kinds: { d_x: 'all' } }),
      cell('later', { ...engine, outputs: ['a', 'b', 'c_x'] }),
    ],
    { a: {}, b: {}, c_x: {}, d_x: {} },
    'small',
    {},
    everyAgent({ watches: [['{each}_x', ['c_x', 'd_x']]], kinds: { '{each}_x': 'latest' } }),
  );
}

describe('the way back on the matrix', () => {
  let host: HTMLDivElement;
  afterEach(() => {
    host.remove();
    document.documentElement.removeAttribute('data-theme');
  });

  const draw = (zoom: number) => {
    host = document.createElement('div');
    host.style.width = '1200px';
    host.style.zoom = String(zoom);
    document.body.append(host);
    const root = createRoot(host);
    flushSync(() => root.render(<Chart matrix={matrix(flow())} chosen={null} onChoose={() => undefined} />));
    return root;
  };

  for (const theme of ['light', 'dark']) {
    it(`is drawn in the accent in ${theme}, its line and its arrow`, () => {
      document.documentElement.setAttribute('data-theme', theme);
      const root = draw(1);
      const accent = drawnIn('--accent');
      const lines = [...host.querySelectorAll('.seg.back')];
      expect(lines.length).toBeGreaterThan(0);
      for (const line of lines) expect(getComputedStyle(line).backgroundColor).toBe(accent);
      const arrows = [...host.querySelectorAll('.part.arrow')];
      expect(arrows).toHaveLength(3);
      for (const arrow of arrows) expect(getComputedStyle(arrow).fill).toBe(accent);
      expect(getComputedStyle(host.querySelector('.legend-back') ?? document.body).backgroundColor).toBe(accent);
      root.unmount();
    });
  }

  for (const zoom of [0.5, 0.8, 1, 1.25, 1.5, 2, 3]) {
    it(`points at the edge of the mark it runs again at a zoom of ${zoom}, reaching into none`, () => {
      const root = draw(zoom);
      const arrows = [...host.querySelectorAll<SVGPolygonElement>('.part.arrow')];
      expect(arrows).toHaveLength(3);
      for (const arrow of arrows) {
        const crossing = arrow.closest('td') as HTMLElement;
        // The mark it points to is the last of the crossing's, the ring's edge half its line past its radius.
        const marks = [...crossing.querySelectorAll<SVGGraphicsElement>('circle')];
        const last = marks[marks.length - 1];
        const box = last.getBoundingClientRect();
        const line = Number.parseFloat(getComputedStyle(last).strokeWidth) || 0;
        const scale = box.width / (2 * Number(last.getAttribute('r')));
        const ringed = getComputedStyle(last).stroke !== 'none' && getComputedStyle(last).fill === 'none';
        const edge = box.right + (ringed ? (line / 2) * scale : 0);
        const point = arrow.getBoundingClientRect().left;
        expect(Math.abs(point - edge), crossing.textContent ?? '').toBeLessThan(0.5);
        // No mark of the crossing reaches past the point.
        for (const mark of marks) expect(mark.getBoundingClientRect().right).toBeLessThanOrEqual(point + 0.01);
      }
      root.unmount();
    });
  }
});
