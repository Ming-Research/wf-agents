// A section of the item's window, in a real browser (Chromium): the window is the same on the console and on a work's
// page, so its section looks the same in both, in light and in dark: its name in the head's colour with a line after
// it to the edge, and it folds away at a press.

import '../styles/tokens.css';
import '../styles/parts.css';
import './console.css';

import { flushSync } from 'react-dom';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import { Fold } from '../parts';

/// `#rrggbb` as the browser gives a colour, `rgb(r, g, b)`.
const rgb = (hex: string) => {
  const n = Number.parseInt(hex.slice(1), 16);
  return `rgb(${n >> 16}, ${(n >> 8) & 255}, ${n & 255})`;
};

/// A section drawn inside an element of this class, none being outside the console as on a work's page, the look the
/// browser gives its head, and whether what it holds is still drawn after a press of its head.
function drawnIn(className: string) {
  const host = document.createElement('div');
  host.className = className;
  document.body.append(host);
  const root = createRoot(host);
  flushSync(() =>
    root.render(
      <Fold title="The cells' marks" count={3}>
        <p>What the section holds.</p>
      </Fold>,
    ),
  );
  const head = host.querySelector('.cw-fold-head') as HTMLButtonElement;
  const style = getComputedStyle(head);
  const look = {
    color: style.color,
    fontSize: style.fontSize,
    fontWeight: style.fontWeight,
    line: getComputedStyle(head, '::after').content,
    countWeight: getComputedStyle(head.querySelector('.cw-fold-count') as HTMLElement).fontWeight,
  };
  flushSync(() => head.click());
  const held = host.querySelector('.cw-fold-body') !== null;
  root.unmount();
  host.remove();
  try {
    localStorage.clear();
  } catch {
    // Kept only where the browser lets it be.
  }
  return { look, held };
}

describe("a section of the item's window", () => {
  afterEach(() => document.documentElement.removeAttribute('data-theme'));

  for (const [theme, head] of [
    ['light', '#285bb2'],
    ['dark', '#739ffa'],
  ]) {
    it(`looks the same on the console and on a work's page in ${theme}, and folds away at a press`, () => {
      document.documentElement.setAttribute('data-theme', theme);
      const onConsole = drawnIn('console');
      const onWork = drawnIn('');
      expect(onConsole.look).toEqual(onWork.look);
      expect(onConsole.look.color).toBe(rgb(head));
      expect(onConsole.look.fontSize).toBe('13px');
      expect(onConsole.look.line).not.toBe('none');
      expect(onConsole.look.countWeight).not.toBe(onConsole.look.fontWeight);
      expect(onConsole.held).toBe(false);
    });
  }
});
