// The four sections a cell's panel and a round's panel are both laid out in, in this order, from this one part: In,
// what the cell is given, or what the round was; Out, what comes out of it, as JSON (Returns.tsx); From, the rows of the
// flow that lead into the cell; To, the cell's own rows, each with what of its return it is worked out from and where
// it leads (Rows.tsx). All four are folded at first, as the owner asked, the tables too, which ran long beside the chart;
// what the owner opens or folds stays so from cell to cell, and from round to round.

import type { ReactNode } from 'react';

import { type Key, t } from '../text';
import { PanelSection } from '../voices';

export type Side = 'in' | 'out' | 'from' | 'to';

const SIDES: Array<[Side, Key]> = [
  ['in', 'side_in'],
  ['out', 'side_out'],
  ['from', 'side_from'],
  ['to', 'side_to'],
];

/// The sections open before the owner opens or folds any.
export const OPEN_FIRST: ReadonlySet<Side> = new Set<Side>();

/// One section: its count and what it holds in small muted words on the line that folds it, and what it holds.
export type SidePart = { n?: number; facts?: ReactNode; body: ReactNode };

export function Sides({ parts, open, onOpen }: {
  parts: Record<Side, SidePart>;
  open: ReadonlySet<Side>;
  onOpen: (side: Side, open: boolean) => void;
}) {
  return (
    <>
      {SIDES.map(([side, title]) => (
        <PanelSection
          key={side}
          title={t(title)}
          n={parts[side].n}
          facts={parts[side].facts}
          open={open.has(side)}
          onOpen={(o) => onOpen(side, o)}
        >
          {parts[side].body}
        </PanelSection>
      ))}
    </>
  );
}

/// A set of open sections with one opened or folded; the same set when nothing changes.
export function toggled(was: ReadonlySet<Side>, side: Side, open: boolean): ReadonlySet<Side> {
  if (was.has(side) === open) return was;
  const n = new Set(was);
  if (open) n.add(side);
  else n.delete(side);
  return n;
}
