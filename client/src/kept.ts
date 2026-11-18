// What the owner chose of how a view shows, kept in this browser so that it stands when the view is left and shown
// again and when the page is loaded again: which of a switch's choices, whether a window is out, the order of a list.
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
  /// Which items the console's tree shows.
  consoleShows: 'wf-agents.console-shows',
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
