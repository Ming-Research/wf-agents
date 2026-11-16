// The accent in a real browser (Chromium): with a colour picked, the tokens the page draws by are the tones it gives,
// in light and in dark, as the browser works them out from the root; what runs and the model's time take the accent,
// and the colours of the meanings stay as they were. Only a browser lays the tokens' blocks over the root's own.

import './styles/tokens.css';

import { afterEach, describe, expect, it } from 'vitest';

import { DEFAULT_TONES, pickAccent, PRESETS, type Theme, TONES, tones } from './accent';

/// The colour the browser draws a token in, as it gives a colour.
function drawnIn(token: string): string {
  const probe = document.createElement('span');
  probe.style.color = `var(${token})`;
  document.body.append(probe);
  const colour = getComputedStyle(probe).color;
  probe.remove();
  return colour;
}

/// `#rrggbb` as the browser gives a colour, `rgb(r, g, b)`.
const rgb = (hex: string) => {
  const n = Number.parseInt(hex.slice(1), 16);
  return `rgb(${n >> 16}, ${(n >> 8) & 255}, ${n & 255})`;
};

const MEANINGS = [
  '--red',
  '--red-text',
  '--orange',
  '--orange-text',
  '--yellow',
  '--green',
  '--green-text',
  '--track-tool',
];

describe('the tokens, with an accent picked', () => {
  afterEach(() => {
    pickAccent(null);
    document.documentElement.removeAttribute('data-theme');
  });

  for (const theme of ['light', 'dark'] as Theme[]) {
    it(`are the tones it gives in ${theme}, what follows the accent with them, and the meanings as they were`, () => {
      document.documentElement.setAttribute('data-theme', theme);
      const before = Object.fromEntries(MEANINGS.map((m) => [m, drawnIn(m)]));
      const byDefault = drawnIn('--accent');
      expect(byDefault).toBe(rgb(DEFAULT_TONES[theme].accent));
      const violet = PRESETS[2].colour;
      expect(pickAccent(violet)).toBeNull();
      const t = tones(violet)[theme];
      for (const tone of Object.keys(TONES[theme])) expect(drawnIn(`--${tone}`), tone).toBe(rgb(t[tone]));
      const accent = rgb(t.accent);
      expect(accent).not.toBe(byDefault);
      expect(drawnIn('--running')).toBe(accent);
      expect(drawnIn('--track-model')).toBe(accent);
      expect(drawnIn('--running-text')).toBe(theme === 'light' ? rgb(t['accent-text']) : accent);
      if (theme === 'dark') {
        expect(drawnIn('--accent-text')).toBe(accent);
        expect(drawnIn('--head')).toBe(accent);
      }
      expect(Object.fromEntries(MEANINGS.map((m) => [m, drawnIn(m)]))).toEqual(before);
      // The default again: its accent, and nothing left on the root.
      pickAccent(null);
      expect(drawnIn('--accent')).toBe(byDefault);
      expect(document.documentElement.getAttribute('style') ?? '').toBe('');
    });
  }
});
