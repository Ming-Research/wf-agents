// A section of the console's item window, in a real browser (Chromium): the console is settled, so its window keeps
// the section as it was drawn before the panel's heads were made to stand out, in light and in dark, while a panel of
// the other views draws its heads in the head's colour with a line after them.

import '../styles/tokens.css';
import '../styles/parts.css';
import './console.css';

import { flushSync } from 'react-dom';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import { PanelPart } from '../parts';

/// `#rrggbb` as the browser gives a colour, `rgb(r, g, b)`.
const rgb = (hex: string) => {
  const n = Number.parseInt(hex.slice(1), 16);
  return `rgb(${n >> 16}, ${(n >> 8) & 255}, ${n & 255})`;
};

/// A section drawn inside an element of this class, and the look the browser gives its head.
function drawnIn(className: string) {
  const host = document.createElement('div');
  host.className = className;
  document.body.append(host);
  const root = createRoot(host);
  flushSync(() =>
    root.render(
      <PanelPart label="The cells' marks" count={3}>
        <p>What the section holds.</p>
      </PanelPart>,
    ),
  );
  const head = host.querySelector('.panel-part-label') as HTMLElement;
  const style = getComputedStyle(head);
  const look = {
    display: style.display,
    margin: [style.marginTop, style.marginRight, style.marginBottom, style.marginLeft],
    color: style.color,
    fontSize: style.fontSize,
    fontWeight: style.fontWeight,
    line: getComputedStyle(head, '::after').content,
    countWeight: getComputedStyle(head.querySelector('.muted') as HTMLElement).fontWeight,
    around: getComputedStyle(host).color,
  };
  root.unmount();
  host.remove();
  return look;
}

describe("a section of the console's window", () => {
  afterEach(() => document.documentElement.removeAttribute('data-theme'));

  for (const theme of ['light', 'dark']) {
    it(`keeps its look in ${theme}: a small semibold label in the colour of the text round it`, () => {
      document.documentElement.setAttribute('data-theme', theme);
      const look = drawnIn('console');
      expect(look.display).toBe('block');
      expect(look.margin).toEqual(['0px', '0px', '4px', '0px']);
      expect(look.color).toBe(look.around);
      expect(look.fontSize).toBe('12px');
      expect(look.fontWeight).toBe('600');
      expect(look.line).toBe('none');
      expect(look.countWeight).toBe('600');
    });
  }

  it("leaves the other views' panels their heads in the head's colour, with a line after them", () => {
    for (const [theme, head] of [
      ['light', '#6a3fd0'],
      ['dark', '#a78bfa'],
    ]) {
      document.documentElement.setAttribute('data-theme', theme);
      const look = drawnIn('panel');
      expect(look.color).toBe(rgb(head));
      expect(look.fontSize).toBe('13px');
      expect(look.line).not.toBe('none');
    }
  });
});
