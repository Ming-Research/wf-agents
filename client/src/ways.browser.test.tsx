// The way back in every view, in a real browser (Chromium), in light and in dark: on the flowchart its line, thicker
// than an arrow a write runs, its head and its sample in the legend, and in a work's history a way round's line, its
// dot and its arrow, are each drawn in the accent, as the matrix's way back is (decision the-way-back-is-in-the-accent).

import './styles/tokens.css';
import './styles/graph.css';
import './styles/works.css';

import { afterEach, describe, expect, it } from 'vitest';

/// The colour the browser draws a token in, as it gives a colour.
function drawnIn(token: string): string {
  const probe = document.createElement('span');
  probe.style.color = `var(${token})`;
  document.body.append(probe);
  const colour = getComputedStyle(probe).color;
  probe.remove();
  return colour;
}

/// The size the browser gives a token, in pixels.
function sizeOf(token: string): string {
  const probe = document.createElement('span');
  probe.style.width = `var(${token})`;
  probe.style.display = 'inline-block';
  document.body.append(probe);
  const size = getComputedStyle(probe).width;
  probe.remove();
  return size;
}

describe('the way back in every view', () => {
  let host: HTMLDivElement;
  afterEach(() => {
    host.remove();
    document.documentElement.removeAttribute('data-theme');
  });

  for (const theme of ['light', 'dark']) {
    it(`is in the accent in ${theme}: the flowchart's line, head and legend, and the history's line, dot and arrow`, () => {
      document.documentElement.setAttribute('data-theme', theme);
      host = document.createElement('div');
      // The flowchart's way back, as the drawing gives it: its line and the head its marker draws, and an arrow a
      // write runs beside it; the legend's sample of it.
      host.innerHTML = `
        <div class="graph">
          <svg class="graph-lines">
            <marker class="graph-head graph-head-back"><path d="M 0 0 L 10 5 L 0 10 z" /></marker>
            <path class="graph-edge graph-edge-back" d="M 0 0 L 10 10" />
            <path class="graph-edge graph-edge-data" d="M 0 0 L 10 10" />
          </svg>
          <span class="legend-arrow back"></span>
        </div>
        <table><tbody><tr>
          <td class="step-spot back-from"><span class="seg-back"></span></td>
          <td class="step-spot"><span class="run-arrow"></span></td>
        </tr></tbody></table>`;
      document.body.append(host);
      const accent = drawnIn('--accent');
      const look = (selector: string) => getComputedStyle(host.querySelector(selector) as Element);
      expect(look('.graph-edge-back').stroke).toBe(accent);
      expect(look('.graph-edge-back').strokeWidth).toBe(sizeOf('--chart-back'));
      expect(look('.graph-edge-data').strokeWidth).toBe(sizeOf('--chart-line'));
      expect(look('.graph-head-back path').fill).toBe(accent);
      expect(look('.legend-arrow.back').backgroundColor).toBe(accent);
      expect(look('.seg-back').backgroundColor).toBe(accent);
      expect(getComputedStyle(host.querySelector('.back-from') as Element, '::after').backgroundColor).toBe(accent);
      expect(look('.run-arrow').borderRightColor).toBe(accent);
    });
  }
});
