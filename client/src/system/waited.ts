// What the Waited part of the system's view says, apart from how it draws it: the causes a work stopped for, in the order
// the tiles and the week table list them, the words for each, and the figures the tiles and the table are made of.

import { duration } from '../works/words';
import type { Occupancy, Stopped, WaitReason, Waited, WaitedBy, Week } from './api';

/// The causes, in the order the engine counts them and the tiles and the week table list them: what waits on the owner
/// first.
export const WAIT_REASONS: WaitReason[] = ['owner', 'items', 'slot', 'turn', 'paused'];

/// The name each cause has in a tile and as a head of the week table, and what it counts, said on pointing at it.
export const WAIT_WORDS: Record<WaitReason, { name: string; title: string }> = {
  owner: { name: 'For you', title: 'A work stopped for what only you can give: an answer, a decision, a fault to mend' },
  items: { name: 'For items', title: 'A work stopped for the items its item depends on to merge or be cancelled' },
  slot: {
    name: 'For a slot',
    title: 'The engine would have begun a round of an agent, and no slot was free: the rounds under way took them all',
  },
  turn: {
    name: 'For its turn',
    title: "A work stopped for the turn of its project to check and merge, which another work of the project held",
  },
  paused: { name: 'Paused', title: "A pause held the work: its own, the pause of everything, or a stop of the engine" },
};

/// The words the part says besides: its tiles that are not a cause, its table's heads, and what it says where it does not
/// know.
export const WAITED_WORDS = {
  stops: 'stops',
  total: 'in all',
  median: 'median',
  longest: 'longest',
  ci: 'For CI',
  ciTitle: "A call or a round of the work waited for a run of the project's CI to end",
  queued: 'Job in the queue',
  queuedTitle: 'How long a job of those runs waited for a runner before it started',
  taking: 'Slots taken',
  takingTitle: 'How many works held a slot, on average over the span and at most at once, of the most slots there were',
  running: 'Works running',
  runningTitle: 'How many works had a round or a tool running, on average over the span and at most at once',
  week: 'Week of',
  slots: 'Slots',
  noneEnded: 'none ended',
  notKept: 'not kept',
  notKeptTitle: 'The engine kept the number of slots only from the time it began to',
  unseparated:
    'Waits of works that ended before a wait for a slot was told apart are in none of these: they are kept as anything else.',
  uncounted: 'works are not counted: their flow was not kept.',
};

/// How long a stretch took, or that it took none.
export function took(ms: number): string {
  return ms === 0 ? 'none' : duration(ms);
}

/// A tile's more: the median and the longest of the stops that ended, where any did.
export function endedWords(stopped: Stopped): string | undefined {
  if (stopped.medianMs === null || stopped.longestMs === null) return undefined;
  return `${WAITED_WORDS.median} ${duration(stopped.medianMs)}, ${WAITED_WORDS.longest} ${duration(stopped.longestMs)}`;
}

/// The cause's figures by its name, in the order of `WAIT_REASONS`.
export function waitedBy(waited: Waited): WaitedBy[] {
  return WAIT_REASONS.map(
    (reason) =>
      waited.by.find((by) => by.reason === reason) ?? { reason, stops: 0, totalMs: 0, medianMs: null, longestMs: null },
  );
}

/// Works, to one place after the point: the engine counts them in thousandths.
export function works(thousandths: number): string {
  return (thousandths / 1000).toFixed(1);
}

/// How many works held a slot, or ran: on average, and at most at once; and, for a slot, of how many there were: said
/// `not kept` where the engine kept none in the stretch.
export function crowdWords(average: number, peak: number, slots?: number | null): string {
  const of = slots === undefined ? '' : slots === null ? ` of ${WAITED_WORDS.notKept}` : ` of ${slots}`;
  return `${works(average)} avg, ${peak} at most${of}`;
}

/// The occupancy of a stretch in a table's cell: the slots taken on average and at most, of the slots there were.
export function takingWords(o: Occupancy): string {
  return crowdWords(o.takingAvg, o.takingPeak, o.slots);
}

/// What a week's row of the table says of each cause, in the order of `WAIT_REASONS`, as how long, or none.
export function weekCells(week: Week): string[] {
  return WAIT_REASONS.map((_, i) => (week.waited === undefined ? '' : took(week.waited.totalMs[i] ?? 0)));
}
