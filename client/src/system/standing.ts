// What the system's view works out from what it reads, apart from how it draws it: where each work stands now, as the
// owner acts on it; where the span the view counts over starts; what the span's merges and rounds add up to; and the
// rounds sent back, by item.

import type { Period } from '../route';
import type { Spent, Summary } from '../works-api';
import { waitsOnOwner } from '../works/waiting';
import { STATE, stateOf, waitingFor } from '../works/words';
import type { Merged, SentBack } from './api';

/// Where a work stands now, as the owner acts on it, each work in one place: it is halted at a fault, or stalled, no
/// cell able to move it on; its state cannot be worked out, though the engine goes on with it; it waits for the owner, as the engine's one list of it says; a round of it runs; its next
/// round is due, as soon as a slot is free or the owner's pause of everything is lifted; it waits for what another
/// gives; the owner paused it; it is paused and no round of it has begun. What needs the owner comes first.
export const STANDINGS = ['halted', 'broken', 'owner', 'running', 'due', 'others', 'paused', 'notStarted'] as const;
export type Standing = (typeof STANDINGS)[number];

/// The tiles' names, the states' own words, and what each counts, said on pointing at it.
export const STANDING_WORDS: Record<Standing, { name: string; title: string }> = {
  halted: { name: STATE.halted, title: 'At a fault, or stalled: no cell can move it on' },
  broken: { name: STATE.broken, title: 'Its state cannot be worked out; the card on its item says why' },
  owner: { name: STATE.owner, title: 'Each has a card on its item in the console, where you act on it' },
  running: { name: STATE.running, title: 'A round of it is under way' },
  due: {
    name: 'Due',
    title: "Its next round begins once a slot is free, or once your pause of everything is lifted",
  },
  others: { name: waitingFor('others'), title: 'Waits for what another gives: a tool, a source outside the work, or the engine' },
  paused: { name: STATE.paused, title: 'Paused by you: only a note to a cell is answered until it is let on' },
  notStarted: {
    name: STATE.notStarted,
    title: 'Paused, and no round of it has begun: it begins once you let it on',
  },
};

/// Where a work stands now; none once it has ended.
export function standingOf(work: Summary): Standing | null {
  const now = work.now;
  if (waitsOnOwner(work)) return 'owner';
  if (stateOf(work).which === 'notStarted') return 'notStarted';
  switch (now.kind) {
    case 'merged':
    case 'cancelled':
      return null;
    case 'fault':
      return 'halted';
    case 'broken':
      return 'broken';
    case 'paused':
      return 'paused';
    case 'round':
      return 'running';
    case 'waits':
      return now.on.every((w) => w.kind === 'slot') ? 'due' : 'others';
    case 'everything':
    case 'next':
      return 'due';
    case 'stalled':
      return 'halted';
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

/// When the week after the one under way begins: its Monday's midnight in the browser's time zone.
export function nextWeekStart(now: number): number {
  const d = new Date(weekStarts('week', now)[0]);
  d.setDate(d.getDate() + 7);
  return d.getTime();
}

/// The share of the tokens sent that were read from the cache, from 0 to 1; none where nothing was sent.
export function cacheShare(spent: Spent): number | null {
  const sent = spent.inputTokens + spent.cacheWrittenTokens + spent.unsplitTokens + spent.cachedTokens;
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

/// What the span spent in all, the rounds that ended in it and the keep-warm pings, whatever they were for: the works that
/// did not merge, the rounds sent back and the rounds that failed among them, for each work merged in it; none where
/// none merged. What the merged works cost themselves is [`mergeAverages`]'s, over their whole lives.
export function spentPerMerge(spent: Spent, merged: Merged[]): number | null {
  return merged.length === 0 ? null : spent.usdMicros / merged.length;
}

/// A name with how many of an item's rounds sent back it holds of: a reason by its key, or a cell.
export interface Counted {
  name: string;
  rounds: number;
}

/// What one item's rounds sent back in the span come to: how many there were; each reason they were told, by its key in
/// `[faults]` of `agents/return/sent_back.toml`, with how many of them were told it; each cell they ran, with how many
/// ran it; what they took, the rounds still under way counted in `underWay` and not yet in `spent`; when the latest
/// began; and the work of the latest, which the item's row opens.
export interface ItemSentBack {
  item: number;
  work: number;
  rounds: number;
  reasons: Counted[];
  cells: Counted[];
  spent: Spent;
  underWay: number;
  latest: number;
}

/// The names counted, the most first, and of as many the first by name.
function countedOf(counts: Map<string, number>): Counted[] {
  return [...counts]
    .map(([name, rounds]) => ({ name, rounds }))
    .sort((a, b) => b.rounds - a.rounds || a.name.localeCompare(b.name));
}

/// The rounds sent back, by item: the items most sent back first, and of as many the one sent back latest first. A
/// reason a round was told more than once, as of two blocks, counts that round once.
export function sentBackByItem(list: SentBack[]): ItemSentBack[] {
  const by = new Map<number, SentBack[]>();
  for (const round of list) by.set(round.item, [...(by.get(round.item) ?? []), round]);
  const items = [...by].map(([item, rounds]): ItemSentBack => {
    const latest = rounds.reduce((a, b) => (b.began > a.began ? b : a));
    const reasons = new Map<string, number>();
    const cells = new Map<string, number>();
    for (const round of rounds) {
      for (const key of new Set(round.unmet.map((u) => u.key))) reasons.set(key, (reasons.get(key) ?? 0) + 1);
      cells.set(round.cell, (cells.get(round.cell) ?? 0) + 1);
    }
    return {
      item,
      work: latest.work,
      rounds: rounds.length,
      reasons: countedOf(reasons),
      cells: countedOf(cells),
      spent: summed(rounds),
      underWay: rounds.filter((r) => r.spent === null).length,
      latest: latest.began,
    };
  });
  return items.sort((a, b) => b.rounds - a.rounds || b.latest - a.latest);
}

/// What a list of things that took something took, together.
export function summed(list: { spent: Spent | null }[]): Spent {
  const sum: Spent = {
    usdMicros: 0,
    inputTokens: 0,
    cacheWrittenTokens: 0,
    unsplitTokens: 0,
    cachedTokens: 0,
    outputTokens: 0,
    workingMs: 0,
    rounds: 0,
  };
  for (const { spent } of list) {
    if (spent === null) continue;
    sum.usdMicros += spent.usdMicros;
    sum.inputTokens += spent.inputTokens;
    sum.cacheWrittenTokens += spent.cacheWrittenTokens;
    sum.unsplitTokens += spent.unsplitTokens;
    sum.cachedTokens += spent.cachedTokens;
    sum.outputTokens += spent.outputTokens;
    sum.workingMs += spent.workingMs;
    sum.rounds += spent.rounds;
  }
  return sum;
}
