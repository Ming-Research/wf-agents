// What a work waits on the owner for, as the engine defines it once and gives it with each work (`Summary.waits`):
// the bar's count, the browser's notifications, the mark of an item's card, the cards of its panel and the Works list's
// order read it from here, and work out none of their own (principle one-count-of-what-waits). Beside it, how many
// questions the owner may answer ahead (`Summary.ahead`), which a card's orange tag and the bar beside Console read.

import type { Act, Ends, InstallationWait, Item, Items, Summary, Wait } from '../works-api';

export type { Ends, Wait };

/// Everything a work waits on the owner for now, in the order it waits for them, as the engine says it.
export const waitsOf = (s: Pick<Summary, 'waits'>): Wait[] => s.waits ?? [];

/// Whether a work waits on the owner: the one test of the count, the marks and the notifications.
export const waitsOnOwner = (s: Pick<Summary, 'waits'>): boolean => waitsOf(s).length > 0;

/// How many things wait on the owner, over every work: the one count.
export const waitingCount = (summaries: Pick<Summary, 'waits'>[]): number =>
  summaries.reduce((n, s) => n + waitsOf(s).length, 0);

/// What the whole installation waits on the owner for, as the engine gives it with the items: the engine's pause of every
/// work, which no work waits on the owner for. The bar's count and the notifications read it beside each work's own, and
/// work out no rule of what waits.
export const installationWaits = (items: Pick<Items, 'waits'> | null): InstallationWait[] => items?.waits ?? [];

/// How many of the cells' replies the owner has not read in a work, as the engine's one definition of it says
/// (`Summary.repliesUnread`).
export const repliesUnreadOf = (s: Pick<Summary, 'repliesUnread'>): number => s.repliesUnread ?? 0;

/// How many replies the owner has not read, over every work: the number of the bar's count in the accent.
export const repliesUnreadCount = (summaries: Pick<Summary, 'repliesUnread'>[]): number =>
  summaries.reduce((n, s) => n + repliesUnreadOf(s), 0);

/// How many of a work's questions the owner may answer ahead, which hold it at nothing yet, as the engine counts them
/// (`Summary.ahead`): only while the work goes on. A card's orange tag and the bar's count beside Console read it here.
export const aheadOf = (s: Pick<Summary, 'now' | 'ahead'>): number =>
  s.now.kind === 'merged' || s.now.kind === 'cancelled' ? 0 : (s.ahead ?? 0);

/// How many questions the owner may answer ahead, over every work.
export const aheadCount = (summaries: Pick<Summary, 'now' | 'ahead'>[]): number =>
  summaries.reduce((n, s) => n + aheadOf(s), 0);

/// Whether the panel draws a wait as its own card where it arises, in the item's text: a question, as a choice where a
/// passage of the body or a message an agent wrote points to it, or else after the last section's passages, and a
/// block the owner rules on, at the end of that section. The rest are drawn under the work's state line.
export const drawnInBody = (w: Wait): boolean =>
  w.ends.kind === 'answer' || w.ends.kind === 'write';

/// The act on the work a card's button sends, by what ends the wait; none where the card's options say it (a question,
/// a ruling), or where nothing on the page does it.
export function actOf(ends: Ends): Act | null {
  switch (ends.kind) {
    case 'goOn':
      return { kind: 'goOn', call: ends.call };
    case 'letGo':
      return { kind: 'letGo', cell: ends.cell };
    case 'resume':
      return { kind: 'resume' };
    case 'answer':
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
