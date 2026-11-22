// What the owner chose of how a view shows, kept in this browser so that it stands when the view is left and shown
// again and when the page is loaded again: which of a switch's choices, whether a window is out, the order of a list,
// which items of a tree are folded.
// Kept only where the browser lets it be: a page that keeps nothing still works, at its first choices. Widths are kept
// the same way (`sizes.ts`).
import { useCallback, useState } from 'react';

/// What is kept under a key, or none.
export function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

/// Keeps a value under a key, or forgets it.
export function write(key: string, value: string | null) {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, value);
  } catch {
    // Kept for this page only.
  }
}

/// The choice kept under a key where it is one of `allowed`, else `first`.
export function keptChoice<T extends string>(key: string, first: T, allowed: readonly T[]): T {
  const v = read(key);
  return v !== null && (allowed as readonly string[]).includes(v) ? (v as T) : first;
}

/// The key under which the page keeps each choice.
export const CHOICE_KEYS = {
  /// The flow's view: its matrix or its flowchart.
  flowView: 'wf-agents.flow-view',
  /// Whether the flowchart draws every arrow a write runs.
  flowDataArrows: 'wf-agents.flow-data-arrows',
  /// Which of the kinds of item the console's tree shows, each one ticked.
  consoleShows: 'wf-agents.console-shows',
  /// Which items of the console's tree the owner folded, and which he unfolded.
  consoleFolds: 'wf-agents.console-folds',
  /// The tab of the console's item window.
  consoleTab: 'wf-agents.console-window-tab',
  /// The tab of a work's item window, and whether that window is out.
  workTab: 'wf-agents.work-window-tab',
  workWindow: 'wf-agents.work-window',
  /// The order of the list of works.
  worksOrder: 'wf-agents.works-order',
  /// The span the system's view counts over.
  systemPeriod: 'wf-agents.system-period',
  /// The address each tab of the bar was last at, after this key and the tab's own address.
  tabAddress: 'wf-agents.tab-address',
} as const;

/// One of a switch's choices, kept under `key`: `first` until the owner picks another of `allowed`.
export function useKept<T extends string>(key: string, first: T, allowed: readonly T[]) {
  const [value, setValue] = useState<T>(() => keptChoice(key, first, allowed));
  const set = useCallback(
    (v: T) => {
      setValue(v);
      write(key, v);
    },
    [key],
  );
  return [value, set] as const;
}

/// Which of `allowed` are ticked, kept under `key` as a list with commas: `first` until the owner ticks or unticks one.
/// A kept list none of whose entries is allowed, kept by an earlier page, is taken as none kept.
export function useKeptTicks<T extends string>(key: string, first: readonly T[], allowed: readonly T[]) {
  const [value, setValue] = useState<ReadonlySet<T>>(() => {
    const v = read(key);
    if (v === null) return new Set(first);
    const ticked = v.split(',').filter((t): t is T => (allowed as readonly string[]).includes(t));
    return new Set(v !== '' && ticked.length === 0 ? first : ticked);
  });
  const tick = useCallback(
    (t: T, on: boolean) =>
      setValue((was) => {
        const now = new Set(was);
        if (on) now.add(t);
        else now.delete(t);
        write(key, allowed.filter((a) => now.has(a)).join(','));
        return now;
      }),
    [key, allowed],
  );
  return [value, tick] as const;
}

/// Of each item the owner folded or unfolded, by its number, whether it is folded, as he last chose; an item he never
/// folded or unfolded is not in it. Kept under `key` as each number with `folded` or `unfolded`, in JSON; an entry kept
/// by an earlier page that is no such pair is taken as none kept.
export function useKeptFolds(key: string) {
  const [value, setValue] = useState<ReadonlyMap<number, boolean>>(() => keptFolds(key));
  const turn = useCallback(
    (id: number, folded: boolean) =>
      setValue((was) => {
        const now = new Map(was);
        now.set(id, folded);
        write(key, JSON.stringify(Object.fromEntries([...now].map(([n, f]) => [n, f ? 'folded' : 'unfolded']))));
        return now;
      }),
    [key],
  );
  return [value, turn] as const;
}

/// The folds kept under `key`, as `useKeptFolds` keeps them.
function keptFolds(key: string): Map<number, boolean> {
  const v = read(key);
  if (v === null) return new Map();
  let kept: unknown;
  try {
    kept = JSON.parse(v);
  } catch {
    return new Map();
  }
  if (typeof kept !== 'object' || kept === null || Array.isArray(kept)) return new Map();
  return new Map(
    Object.entries(kept).flatMap(([n, f]): [number, boolean][] =>
      /^[1-9]\d*$/.test(n) && (f === 'folded' || f === 'unfolded') ? [[Number(n), f === 'folded']] : [],
    ),
  );
}
