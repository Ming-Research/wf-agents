// How a cell's state is drawn as a mark, said once for the two places that draw it: the works page's marks
// (`marks.tsx`), as elements styled from it, and the console's cards (`console/snap.ts`), on a canvas that reads the
// same tokens. Each measure and colour is the name of a token of the page (styles/tokens.css), so that both follow
// light and dark. A mark is a square of `side` with `corner` corners, its ring drawn inside that side (the page's boxes
// are `border-box`), filled or only ringed; a skipped cell's is a dash of the square's width, `dash` high, in its
// middle.

import type { CellState } from '../works-api';

/// The measures of a mark and of a strip of them: the square's side and corners, the room between two marks, the width
/// of a ring and of a hairline ring, and the height of the dash.
export const MARK = {
  side: '--space-8',
  corner: '--space-2',
  gap: '--space-2',
  ring: '--focus',
  hairline: '--hairline',
  dash: '--focus',
} as const;

/// A state's mark: a square ringed in `edge`, filled in `fill` or left open, its ring a ring or a hairline wide; or a
/// dash filled in `fill`. Colours are tokens' names.
export type MarkLook =
  | { shape: 'square'; fill: string | null; edge: string; width: 'ring' | 'hairline' }
  | { shape: 'dash'; fill: string };

/// Done filled in a light step of the accent, running filled in a deep one, at rest a ring of that deep one, failed a
/// red ring, runs again an orange ring, not run yet a hairline ring, skipped a muted dash.
export const MARKS: Record<CellState, MarkLook> = {
  done: { shape: 'square', fill: '--mark-done', edge: '--mark-done', width: 'ring' },
  running: { shape: 'square', fill: '--mark-running', edge: '--mark-running', width: 'ring' },
  resting: { shape: 'square', fill: null, edge: '--mark-running', width: 'ring' },
  failed: { shape: 'square', fill: null, edge: '--red', width: 'ring' },
  again: { shape: 'square', fill: null, edge: '--orange', width: 'ring' },
  notYet: { shape: 'square', fill: null, edge: '--muted', width: 'hairline' },
  skipped: { shape: 'dash', fill: '--muted' },
};

const v = (token: string) => `var(${token})`;

/// The style of a state's mark as an element: its size, ring, corners and fill, from `MARKS` and `MARK`.
export function markStyle(state: CellState): Record<string, string> {
  const look = MARKS[state];
  if (look.shape === 'dash')
    return {
      width: v(MARK.side),
      height: v(MARK.dash),
      margin: `calc((${v(MARK.side)} - ${v(MARK.dash)}) / 2) 0`,
      background: v(look.fill),
    };
  return {
    width: v(MARK.side),
    height: v(MARK.side),
    border: `${v(look.width === 'ring' ? MARK.ring : MARK.hairline)} solid ${v(look.edge)}`,
    borderRadius: v(MARK.corner),
    background: look.fill === null ? 'transparent' : v(look.fill),
  };
}

/// The style of a strip of marks as an element: the room between its marks.
export const stripStyle = (): Record<string, string> => ({ gap: v(MARK.gap) });
