// The widths the owner drags (the panel's, the names' column's, each cell's column's), kept in
// this browser so that they stand when the page is loaded again; a double click on an edge forgets its width, which
// the page then works out again. Kept only where the browser lets it be: a page that keeps nothing still works, at the
// widths it works out.
import { useCallback, useState } from 'react';

/// The docked panel's width at first, and the least and most it is dragged to; one width for the panel of every view.
export const PANEL = { first: 360, min: 280, max: 720 };

/// The key under which the page keeps a width.
export const WIDTH_KEYS = {
  panel: 'wf-agents.panel-width',
  names: 'wf-agents.names-width',
  /// The flow's chart: each cell's column, which starts as wide as its marks.
  flowCells: 'wf-agents.flow-cell-widths',
  /// The history of a work: its column of times and words, and each cell's column.
  steps: 'wf-agents.steps-width',
  historyCells: 'wf-agents.history-cell-widths',
  /// The flow view's window, the console's (`Drawer`), which keeps its own width under its own key.
  flowWindow: 'wf-agents.flow-window-width',
} as const;

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string | null) {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, value);
  } catch {
    // Kept for this page only.
  }
}

/// A number kept within `min` and `max`, or none.
function within(n: unknown, min: number, max: number): number | null {
  return typeof n === 'number' && Number.isFinite(n) && n >= min && n <= max ? Math.round(n) : null;
}

/// One width the owner drags, between `min` and `max`: none until dragged, and none again once reset.
export function useKeptWidth(key: string, min: number, max: number) {
  const [width, setWidth] = useState<number | null>(() => within(Number(read(key) ?? NaN), min, max));
  const set = useCallback(
    (w: number | null) => {
      const kept = w === null ? null : Math.min(max, Math.max(min, Math.round(w)));
      setWidth(kept);
      write(key, kept === null ? null : String(kept));
    },
    [key, min, max],
  );
  return [width, set] as const;
}

/// The widths of many things the owner drags, each by its name, between `min` and `max`.
export function useKeptWidths(key: string, min: number, max: number) {
  const [widths, setWidths] = useState<Record<string, number>>(() => {
    try {
      const all = JSON.parse(read(key) ?? '{}') as Record<string, unknown>;
      return Object.fromEntries(
        Object.entries(all).flatMap(([k, v]) => {
          const w = within(v, min, max);
          return w === null ? [] : [[k, w]];
        }),
      );
    } catch {
      return {};
    }
  });
  const set = useCallback(
    (name: string, w: number | null) => {
      setWidths((was) => {
        const next = { ...was };
        if (w === null) delete next[name];
        else next[name] = Math.min(max, Math.max(min, Math.round(w)));
        write(key, Object.keys(next).length > 0 ? JSON.stringify(next) : null);
        return next;
      });
    },
    [key, min, max],
  );
  return [widths, set] as const;
}
