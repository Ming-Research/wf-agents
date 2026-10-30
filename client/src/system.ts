// What the System view computes from what it reads, apart from how it draws it: the works by where they stand now, as
// the owner acts on them; what the view asks the interface for; what the period's weeks and merges add up to; and the
// anomalies by their kind.

import type { Anomaly, AnomalyKind, Merged, Status, Week, WorkSummary } from './api';
import { course } from './components';
import type { Period } from './route';

/// Where a work stands now, as the owner acts on it, each work in one place: a round of it runs; its next round is due;
/// it waits for the owner's answer or ruling; for the architect's reply; for other items or its turn; the owner or the
/// architect paused it; or the engine stopped it.
export const STANDINGS = ['running', 'ready', 'you', 'architect', 'items', 'paused', 'stopped'] as const;
export type Standing = (typeof STANDINGS)[number];

/// Where a work stands now; none once it is over. What needs the owner comes first: the engine's stop, then what the
/// owner is to give, then a pause, then what others are to give. A work held only because every work is paused stands
/// where it would without the pause, as the works list says it.
export function standingOf(w: WorkSummary, status: Status | null): Standing | null {
  const c = course(w, status);
  if (c.over) return null;
  if (c.held === 'engine') return 'stopped';
  const waits = c.waits;
  if (waits?.kind === 'answer' || waits?.kind === 'ruling') return 'you';
  if ((c.held === 'you' || c.held === 'architect') && !c.releasing) return 'paused';
  if (waits?.kind === 'architect') return 'architect';
  if (waits !== null) return 'items';
  return c.running !== null ? 'running' : 'ready';
}

/// The works not over, by where they stand, in the order of the standings.
export function byStanding(
  works: WorkSummary[],
  status: Status | null,
): Array<{ standing: Standing; works: WorkSummary[] }> {
  return STANDINGS.map((standing) => ({ standing, works: works.filter((w) => standingOf(w, status) === standing) }));
}

/// The browser's time zone now, in minutes east of UTC.
export function zone(): number {
  return -new Date().getTimezoneOffset();
}

/// What the view asks the interface for: the weeks the period holds, none for all time, and the owner's time zone
/// (`tz`, minutes east of UTC), which the weeks start in.
export function systemPath(period: Period, tz: number): string {
  const weeks = period === 'week' ? 1 : period === '4w' ? 4 : null;
  return `/api/v1/system?tz=${tz}${weeks !== null ? `&weeks=${weeks}` : ''}`;
}

/// What the period's weeks spent: the engineers' works, the architects' works, and both.
export function spentIn(weeks: Week[]): { engineers: number; architects: number; total: number } {
  const engineers = weeks.reduce((a, w) => a + w.engineers_usd, 0);
  const architects = weeks.reduce((a, w) => a + w.architects_usd, 0);
  return { engineers, architects, total: engineers + architects };
}

/// The average time from a merged work's first round to its merge, and what such a work cost on average; none with
/// nothing merged.
export function mergedAverages(merged: Merged[]): { took: number; cost: number } | null {
  if (merged.length === 0) return null;
  return {
    took: merged.reduce((a, m) => a + (m.merged_at - m.started_at), 0) / merged.length,
    cost: merged.reduce((a, m) => a + m.cost_usd, 0) / merged.length,
  };
}

/// The kinds of anomaly, in the order the view shows them.
export const ANOMALY_KINDS: ReadonlyArray<AnomalyKind['kind']> = [
  'flow_anomaly',
  'counter',
  'stopped',
  'tool_failed',
  'main_red',
];

/// The anomalies by their kind, in the order of the kinds, each kind's as they came.
export function anomaliesBy(list: Anomaly[]): Array<{ kind: AnomalyKind['kind']; list: Anomaly[] }> {
  return ANOMALY_KINDS.map((kind) => ({ kind, list: list.filter((a) => a.what.kind === kind) }));
}
