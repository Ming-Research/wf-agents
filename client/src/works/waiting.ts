// What a work waits for from the owner, defined once. The bar's count, the browser's notifications, the mark of an item's
// card and the cards of its panel are all read from `waitsOf`, so that none of them shows what another does not
// (principle one-count-of-what-waits). It is the page's reading of what `wf-agents wait` calls a work that rests for a
// person.

import type { Act, EngineAct, Item, Now } from '../works-api';
import { failureWords, outageDown, waitsForOwner } from './words';

/// What kind of thing a work waits for from the owner.
export type WaitKind =
  /// A question to the owner, which a cell asked or which is put as a call, or the engine's question of a change of a
  /// protected file.
  | 'question'
  /// A cell at its limit, which waits for the owner to let it go on.
  | 'limit'
  /// A precondition the owner can make hold by writing a block its test reads, as a review's verdict that holds the
  /// work, for the owner to rule on, which the ruling card of the panel's body shows (principle
  /// reviews-can-be-disputed).
  | 'precondition'
  /// A pause the engine made, with its reason.
  | 'pause'
  /// The work is held by the pause of everything the engine set as it started again, until the owner lets them on.
  | 'held'
  /// The wait every work shares where the agents' command line cannot run, which only the owner can mend: a wait every
  /// work shares for any other cause is the engine's, which checks the outside itself and lifts it.
  | 'agents';

/// What the owner does on the card to let the work on: an act on the work, or on the engine as a whole.
export type WaitAct = { on: 'work'; act: Act; label: string } | { on: 'engine'; act: EngineAct; label: string };

/// One thing a work waits for from the owner.
export interface Wait {
  /// Names it within its work: in the address a notification opens, and as the mark of the card that shows it.
  key: string;
  kind: WaitKind;
  /// What it waits for, in a few words, for a notification and for its card.
  words: string;
  /// What the owner does on its card, where there is something to do beyond answering.
  act: WaitAct | null;
}

/// Everything a work waits for from the owner now, in the order the work waits for it.
export function waitsOf(now: Now): Wait[] {
  switch (now.kind) {
    case 'paused':
      return now.by === 'engine'
        ? [
            {
              key: 'pause',
              kind: 'pause',
              words: now.why === null ? 'paused by the engine, which gave no reason' : `paused by the engine: ${now.why}`,
              act: { on: 'work', act: { kind: 'resume' }, label: 'Resume' },
            },
          ]
        : [];
    case 'everything':
      return now.by === 'engine'
        ? [
            {
              key: 'held',
              kind: 'held',
              words: 'held back by the pause of everything, set as the engine started again',
              act: { on: 'engine', act: { kind: 'resume' }, label: 'Start all' },
            },
          ]
        : [];
    case 'waits':
      return now.on.flatMap((w): Wait[] => {
        if (!waitsForOwner(w)) return [];
        switch (w.kind) {
          case 'call':
            if (w.what.kind === 'atLimit') {
              return [
                {
                  key: 'limit',
                  kind: 'limit',
                  words: `${now.at} reached its limit, and waits for you to let it go on`,
                  act: { on: 'work', act: { kind: 'result', call: w.call, result: 'go on' }, label: 'Let it go on' },
                },
              ];
            }
            return w.what.kind === 'question'
              ? [{ key: `q${w.call}`, kind: 'question', words: w.what.question, act: null }]
              : [];
          // A question of the work's own, by its name, which its card at the plan's end answers.
          case 'answer':
            return [{ key: `answer-${w.question}`, kind: 'question', words: w.question, act: null }];
          case 'limit':
          case 'calls':
            return [
              {
                key: 'limit',
                kind: 'limit',
                words: `${now.at} reached its limit, and waits for you to let it go on`,
                act: { on: 'work', act: { kind: 'letGo', cell: now.at }, label: 'Let it go on' },
              },
            ];
          case 'precondition':
            return [{ key: 'precondition', kind: 'precondition', words: w.words, act: null }];
          case 'agents': {
            const down = outageDown(w.outage);
            return [
              {
                key: 'agents',
                kind: 'agents',
                words: `${failureWords(down.failure)} (${down.why}): sign it in again or install it, and the engine lets the work go on once its own check finds it runs`,
                act: null,
              },
            ];
          }
          // A free slot, and a source outside the work such as main moving, are not the owner's to give.
          case 'slot':
          case 'outside':
            return [];
        }
      });
    default:
      return [];
  }
}

/// Whether a work waits for the owner: the one test of the count, the mark and the notifications.
export const waitsOnOwner = (now: Now): boolean => waitsOf(now).length > 0;

/// The address that opens the console on an item with the card of what it waits for.
export const targetOf = (item: Pick<Item, 'number'>, wait: Wait): { view: 'console'; item: number; wait: string } => ({
  view: 'console',
  item: item.number,
  wait: wait.key,
});
