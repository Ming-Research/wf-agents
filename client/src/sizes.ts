// The widths the owner drags (the names' column's, each cell's column's, the column of a work's times, each window's,
// each column of the list of works), kept in this browser so that they stand when the page is loaded again; a double
// click on an edge forgets its width, which the page then works out again. Kept only where the browser lets it be: a
// page that keeps nothing still works, at the widths it works out.
import { useCallback, useState } from 'react';

/// The key under which the page keeps a width.
export const WIDTH_KEYS = {
  names: 'wf-agents.names-width',
  /// The flow's chart and a work's history, which draws the flow's cells as it does: each cell's column, which starts as
  /// wide as its marks.
  flowCells: 'wf-agents.flow-cell-widths',
  /// The history of a work: its column of times.
  times: 'wf-agents.history-times-width',
  /// The flow view's window, the console's (`Drawer`), which keeps its own width under its own key.
  flowWindow: 'wf-agents.flow-window-width',
  /// The window of an item, the console's and a work's page's alike, which show the same item window.
  itemWindow: 'console-window-width',
  /// The list of works: each of its columns.
  worksColumns: 'wf-agents.works-column-widths',
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
