// What a work waits on the owner for, as the engine defines it once and gives it with each work (`Summary.waits`):
// the bar's count, the browser's notifications, the mark of an item's card, the cards of its panel and the Works list's
// order read it from here, and work out none of their own (principle one-count-of-what-waits).

import type { Act, Ends, Item, Summary, Wait } from '../works-api';

export type { Ends, Wait };

/// Everything a work waits on the owner for now, in the order it waits for them, as the engine says it.
export const waitsOf = (s: Pick<Summary, 'waits'>): Wait[] => s.waits ?? [];

/// Whether a work waits on the owner: the one test of the count, the marks and the notifications.
export const waitsOnOwner = (s: Pick<Summary, 'waits'>): boolean => waitsOf(s).length > 0;

/// How many things wait on the owner, over every work: the one count.
export const waitingCount = (summaries: Pick<Summary, 'waits'>[]): number =>
  summaries.reduce((n, s) => n + waitsOf(s).length, 0);

/// Whether the panel draws a wait as its own card where it arises, in the item's text: a question, as a choice where a
/// passage of the body or a message an agent wrote points to it, or else after the last section's passages, and a
/// block the owner rules on, at the end of that section. The rest are drawn under the work's state line.
export const drawnInBody = (w: Wait): boolean =>
  w.ends.kind === 'answer' || w.ends.kind === 'reply' || w.ends.kind === 'write';

/// The act on the work a card's button sends, by what ends the wait; none where the card's options say it (a question,
/// a ruling), or where nothing on the page does it.
export function actOf(ends: Ends): Act | null {
  switch (ends.kind) {
    case 'goOn':
      return { kind: 'result', call: ends.call, result: 'go on' };
    case 'letGo':
      return { kind: 'letGo', cell: ends.cell };
    case 'resume':
      return { kind: 'resume' };
    case 'answer':
    case 'reply':
    case 'write':
    case 'outside':
      return null;
  }
}

/// The address that opens the console on an item with the card of what it waits for.
export const targetOf = (item: Pick<Item, 'number'>, wait: Wait): { view: 'console'; item: number; wait: string } => ({
  view: 'console',
  item: item.number,
  wait: wait.key,
});
