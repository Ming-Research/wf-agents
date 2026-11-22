// The way back in every view, in a real browser (Chromium), in light and in dark: on the flowchart its line, thicker
// than an arrow a write runs, its head and its sample in the legend are each drawn in the accent, as the matrix's way
// back is (decision the-way-back-is-in-the-accent); in a work's history, where it is a send-back, the line and the arrow
// from a later cell's column to the run it woke, and the history's legend sample, are drawn in red (decision
// a-wake-is-an-arrow-from-the-writer); the history's arrow a write runs to a later cell is in neither.

import './styles/tokens.css';
import './styles/chart.css';
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
    it(`is in the accent in ${theme} on the flowchart, its line, head and legend, and in red on the history`, () => {
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
          <span class="legend-line back"></span>
          <span class="legend-line sent"></span>
        </div>
        <table class="matrix flow history"><tbody><tr>
          <td class="spot back"><span class="seg seg-left back"></span>
            <svg class="glyph row-arrow back"><polygon class="part arrow" points="0,0 1,1 0,1" /></svg></td>
          <td class="spot wakes"><span class="seg seg-left wakes"></span>
            <svg class="glyph row-arrow wakes"><polygon class="part arrow" points="0,0 1,1 0,1" /></svg></td>
        </tr></tbody></table>`;
      document.body.append(host);
      const accent = drawnIn('--accent');
      const look = (selector: string) => getComputedStyle(host.querySelector(selector) as Element);
      expect(look('.graph-edge-back').stroke).toBe(accent);
      expect(look('.graph-edge-back').strokeWidth).toBe(sizeOf('--chart-back'));
      expect(look('.graph-edge-data').strokeWidth).toBe(sizeOf('--chart-line'));
      expect(look('.graph-head-back path').fill).toBe(accent);
      expect(look('.legend-line.back').backgroundColor).toBe(accent);
      expect(look('.legend-line.back').height).toBe(sizeOf('--chart-back'));
      // On the history, a send-back's line, its arrow and its legend sample are red, the line as thick.
      const red = drawnIn('--red');
      expect(red).not.toBe(accent);
      expect(look('.history .seg.back').backgroundColor).toBe(red);
      expect(look('.history .seg.back').height).toBe(sizeOf('--chart-back'));
      expect(look('.history .row-arrow.back .part.arrow').fill).toBe(red);
      expect(look('.legend-line.sent').backgroundColor).toBe(red);
      expect(look('.legend-line.sent').height).toBe(sizeOf('--chart-back'));
      // An arrow a write runs to a later cell is in the colour of the data's lines.
      const data = drawnIn('--chart-data');
      expect(data).not.toBe(accent);
      expect(look('.seg.wakes').backgroundColor).toBe(data);
      expect(look('.row-arrow.wakes .part.arrow').fill).toBe(data);
    });
  }
});
