// How wide the window is, as the page lays itself out by it: Primer's breakpoints, read as Primer's own media queries
// read them, so that a part the page draws by script and a Primer component laid out by its CSS (PageLayout's panes
// below the narrow breakpoint) change at the same width.

import { useSyncExternalStore } from 'react';

/// The window's width as the page tells widths apart: narrow, below Primer's narrow breakpoint (`--breakpoint-medium`,
/// 768px), where a split shows one side at a time; regular, from there to Primer's extra large one
/// (`--breakpoint-xlarge`, 1280px); and full from there on, where the bar holds all of the engine's controls.
export type Width = 'narrow' | 'regular' | 'full';

/// The media queries of the two breakpoints, written as Primer writes its own.
const NARROW = '(max-width: calc(48rem - 0.02px))';
const BELOW_FULL = '(max-width: calc(80rem - 0.02px))';

/// The width a window of so many CSS pixels is, at the browser's default text size, where 1rem is 16px.
export function widthOf(px: number): Width {
  return px < 768 ? 'narrow' : px < 1280 ? 'regular' : 'full';
}

function read(): Width {
  if (typeof window === 'undefined' || window.matchMedia === undefined) return 'full';
  return window.matchMedia(NARROW).matches ? 'narrow' : window.matchMedia(BELOW_FULL).matches ? 'regular' : 'full';
}

function subscribe(changed: () => void): () => void {
  if (typeof window === 'undefined' || window.matchMedia === undefined) return () => {};
  const queries = [NARROW, BELOW_FULL].map((q) => window.matchMedia(q));
  for (const q of queries) q.addEventListener('change', changed);
  return () => {
    for (const q of queries) q.removeEventListener('change', changed);
  };
}

/// The window's width now, changing as the window does; full where there is no window, as when drawn outside a browser.
export function useWidth(): Width {
  return useSyncExternalStore(subscribe, read, () => 'full');
}

/// Whether the window is below the narrow breakpoint.
export function useNarrow(): boolean {
  return useWidth() === 'narrow';
}
