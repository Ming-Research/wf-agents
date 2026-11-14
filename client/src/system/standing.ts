// What the system's view works out from what it reads, apart from how it draws it: where each work stands now, as the
// owner acts on it; where the span the view counts over starts; and what the span's merges and rounds add up to.

import type { Period } from '../route';
import type { Spent, Summary } from '../works-api';
import { waitsForOwner } from '../works/words';
import type { Merged } from './api';

/// Where a work stands now, as the owner acts on it, each work in one place: the engine halted it, at a fault or with
/// its pause for a round that failed; it waits for the owner; a round of it runs; its next round is due, as soon as a
/// slot is free or the pause of everything is lifted; it waits for what another gives; the owner paused it; nothing
/// runs a cell of it. What needs the owner comes first.
export const STANDINGS = ['halted', 'owner', 'running', 'due', 'others', 'paused', 'idle'] as const;
export type Standing = (typeof STANDINGS)[number];

export const STANDING_WORDS: Record<Standing, { name: string; title: string }> = {
  halted: {
    name: 'Halted',
    title: 'At a fault, or paused by the engine, as for a round that failed: the owner looks before it goes on',
  },
  owner: {
    name: 'Waits for the owner',
    title: 'Waits for an answer, a let-go at a limit, or a test only the owner can make hold',
  },
  running: { name: 'Running', title: 'A round of it is under way' },
  due: {
    name: 'Due',
    title: 'Its next round begins once a slot is free, or once the pause of everything is lifted',
  },
  others: {
    name: 'Waits on others',
    title: 'Waits for what the owner does not give: a tool, the items it depends on, a cell, or the wait every work shares',
  },
  paused: { name: 'Paused', title: 'Paused by the owner: only a note to a cell is answered until it is let on' },
  idle: { name: 'At rest', title: 'Nothing new runs a cell, until someone writes what runs one' },
};

/// Where a work stands now; none once it has ended.
export function standingOf(work: Summary): Standing | null {
  const now = work.now;
  switch (now.kind) {
    case 'merged':
    case 'cancelled':
      return null;
    case 'fault':
      return 'halted';
    case 'paused':
      return now.by === 'engine' ? 'halted' : 'paused';
    case 'round':
      return 'running';
    case 'waits':
      if (now.on.some(waitsForOwner)) return 'owner';
      return now.on.every((w) => w.kind === 'slot') ? 'due' : 'others';
    case 'everything':
    case 'next':
      return 'due';
    case 'idle':
      return work.paused ? 'paused' : 'idle';
  }
}

/// The weeks the view counts over, each by its Monday's midnight in the browser's time zone, the earliest first: the
/// week under way; the four to its end; or every week from the one `earliest` falls in, which is the earliest thing
/// the engine recorded, to the week under way. The browser knows when its clocks change, so a week begins at midnight
/// whatever the offset from UTC was then.
export function weekStarts(period: Period, now: number, earliest: number | null = null): number[] {
  const monday = (at: number) => {
    const d = new Date(at);
    d.setHours(0, 0, 0, 0);
    d.setDate(d.getDate() - ((d.getDay() + 6) % 7));
    return d;
  };
  const last = monday(now);
  const first =
    period === 'week'
      ? monday(now)
      : period === '4w'
        ? new Date(last.getFullYear(), last.getMonth(), last.getDate() - 21)
        : monday(Math.min(earliest ?? now, now));
  const starts: number[] = [];
  for (const d = first; d.getTime() <= last.getTime(); d.setDate(d.getDate() + 7)) starts.push(d.getTime());
  return starts;
}

/// The share of the tokens sent that were read from the cache, from 0 to 1; none where nothing was sent.
export function cacheShare(spent: Spent): number | null {
  const sent = spent.inputTokens + spent.cachedTokens;
  return sent === 0 ? null : spent.cachedTokens / sent;
}

/// A share as a percentage, whole.
export function percent(share: number): string {
  return `${Math.round(share * 100)}%`;
}

/// How long the merges took on average from their first round, and what one cost; none where none merged.
export function mergeAverages(merged: Merged[]): { took: number | null; usdMicros: number } | null {
  if (merged.length === 0) return null;
  const timed = merged.filter((m) => m.from !== null);
  const took =
    timed.length === 0 ? null : timed.reduce((sum, m) => sum + (m.at - (m.from ?? m.at)), 0) / timed.length;
  return { took, usdMicros: merged.reduce((sum, m) => sum + m.spent.usdMicros, 0) / merged.length };
}

/// What a list of things that took something took, together.
export function summed(list: { spent: Spent | null }[]): Spent {
  const sum: Spent = { usdMicros: 0, inputTokens: 0, cachedTokens: 0, outputTokens: 0, workingMs: 0, rounds: 0 };
  for (const { spent } of list) {
    if (spent === null) continue;
    sum.usdMicros += spent.usdMicros;
    sum.inputTokens += spent.inputTokens;
    sum.cachedTokens += spent.cachedTokens;
    sum.outputTokens += spent.outputTokens;
    sum.workingMs += spent.workingMs;
    sum.rounds += spent.rounds;
  }
  return sum;
}
