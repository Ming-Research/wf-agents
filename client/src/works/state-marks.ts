// How a run ended or stands, drawn as a mark: one set of looks, which a work's history draws for its runs and the strip
// of a work's cells draws for each cell, a cell standing as its run does. Said once for the places that draw it: the
// page's parts (`StateMark` in `marks.tsx`), as elements styled from it, and the console's cards (`console/snap.ts`), on
// a canvas that reads the same tokens. Each measure and colour is the name of a token of the page (styles/tokens.css),
// so that every place follows light and dark. A mark is a square of `side` with `corner` corners, its ring drawn inside
// that side (the page's boxes are `border-box`), filled or only ringed; a dash is the square's width, `dash` high, in
// its middle.

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

/// How a run ended or stands: a round of it runs; the engine carries out a call or an act for it, which it waits for;
/// it is at rest between two rounds; its last round failed, or what it handed in last was sent back, and it is given
/// its round again; the work is stopped at it, for the owner or by the engine; it finished and changed a block's
/// meaning, or changed none; it was abandoned. A cell that has not run yet stands as it will: it runs again, done
/// before and to be run anew; it has not run yet; it is skipped.
export type RunState =
  | 'running'
  | 'waiting'
  | 'atRest'
  | 'failed'
  | 'stopped'
  | 'handedIn'
  | 'nothingNew'
  | 'abandoned'
  | 'again'
  | 'notYet'
  | 'skipped';

/// Filled is a run that ran or runs, hollow one still to run. Handed in is filled in a light step of the accent, and
/// handed in nothing new is a dash of it; a run under way in its deep step, a round of it running or at rest; waiting
/// for a call in the yellow of a call; failed a hollow ring in red, stopped filled in red; abandoned filled in the
/// muted grey. Runs again is a hollow ring of handed in's light step; not run yet a hairline ring; skipped a muted
/// dash.
export const RUN_STATES: Record<RunState, MarkLook> = {
  running: { shape: 'square', fill: '--mark-running', edge: '--mark-running', width: 'ring' },
  waiting: { shape: 'square', fill: '--yellow', edge: '--yellow', width: 'ring' },
  atRest: { shape: 'square', fill: '--mark-running', edge: '--mark-running', width: 'ring' },
  failed: { shape: 'square', fill: null, edge: '--red', width: 'ring' },
  stopped: { shape: 'square', fill: '--red', edge: '--red', width: 'ring' },
  handedIn: { shape: 'square', fill: '--mark-done', edge: '--mark-done', width: 'ring' },
  nothingNew: { shape: 'dash', fill: '--mark-done' },
  abandoned: { shape: 'square', fill: '--muted', edge: '--muted', width: 'ring' },
  again: { shape: 'square', fill: null, edge: '--mark-done', width: 'ring' },
  notYet: { shape: 'square', fill: null, edge: '--muted', width: 'hairline' },
  skipped: { shape: 'dash', fill: '--muted' },
};

/// How a cell stands, as the state of its run: done is handed in, a call under way waiting.
export const STATE_OF_CELL: Record<CellState, RunState> = {
  running: 'running',
  calling: 'waiting',
  resting: 'atRest',
  failed: 'failed',
  done: 'handedIn',
  again: 'again',
  notYet: 'notYet',
  skipped: 'skipped',
};

/// The look of each cell's state, for the console's cards. The cell a work is stopped at is drawn as `stopped`,
/// whatever its state, so that where the work stands shows in its strip as on its card.
export const CELL_LOOKS = Object.fromEntries(
  Object.entries(STATE_OF_CELL).map(([cell, run]) => [cell, RUN_STATES[run]]),
) as Record<CellState, MarkLook>;

/// Whether a run's mark pulses: running and waiting, the bright marks, wherever they are drawn.
export const pulses = (state: RunState): boolean => state === 'running' || state === 'waiting';

/// Whether a cell's mark pulses, as its run's does.
export const pulsing = (state: CellState): boolean => pulses(STATE_OF_CELL[state]);

const v = (token: string) => `var(${token})`;

/// The style of a mark of this look as an element.
export function lookStyle(look: MarkLook): Record<string, string> {
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
