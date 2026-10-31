// How the lines of a panel's From and To light their rows on the chart beside it, as hovering an arrow does there: the
// line under the pointer while it is there, and the lines picked, each picked and let go by a click, all let go when
// the panel shows another cell or round. The panel's tables (Rows.tsx) tell it what they point at and pick; the page
// that holds the chart and its panels (FlowScreen.tsx) keeps it and marks the chart with it.

import { createContext, useMemo, useState } from 'react';

import type { RowAt } from '../graph/flowModel';

/// A line of From or To: what tells it apart in its panel, the graph whose chart it lights, and the rows it stands for.
export type Line = { key: string; graph: string; rows: RowAt[] };

export type Lights = {
  /// The lines picked, by their keys.
  picked: ReadonlySet<string>;
  /// Tells the line under the pointer, and none once the pointer leaves it.
  point: (l: Line | null) => void;
  /// Picks a line, or lets it go.
  toggle: (l: Line) => void;
};

/// None where no chart is beside the tables.
export const LightsContext = createContext<Lights | null>(null);

/// The lines lit for the panel showing `shown` (a subject's key, none when nothing is open), and what the panel's tables
/// are given to change them; what was lit for another subject is let go.
export function useLights(shown: string | null): { lights: Lights; lit: Line[] } {
  const [state, setState] = useState<{ at: string | null; pointed: Line | null; picked: Line[] }>({
    at: null,
    pointed: null,
    picked: [],
  });
  const now = state.at === shown ? state : null;
  const pointed = now?.pointed ?? null;
  const picked = useMemo(() => now?.picked ?? [], [now?.picked]);
  const lights = useMemo<Lights>(
    () => ({
      picked: new Set(picked.map((l) => l.key)),
      point: (l) =>
        setState((s) => (s.at === shown ? { ...s, pointed: l } : { at: shown, pointed: l, picked: [] })),
      toggle: (l) =>
        setState((s) => {
          const was = s.at === shown ? s.picked : [];
          const picks = was.some((x) => x.key === l.key) ? was.filter((x) => x.key !== l.key) : [...was, l];
          return { at: shown, pointed: s.at === shown ? s.pointed : null, picked: picks };
        }),
    }),
    [picked, shown],
  );
  const lit = useMemo(() => (pointed !== null ? [...picked, pointed] : picked), [picked, pointed]);
  return { lights, lit };
}
