// How the owner answers in a conversation, the messages view being the one place the owner acts: an ask of a work's
// shows where it was asked, as the asking agent's bubble, and is answered in that bubble, by the buttons of what it
// offers (the options of its choice) and a small box of the owner's words beside an Answer button; a question of an
// engineer's that the architect holds is answered in a small box in its bubble too. What is written there answers that
// bubble; the box at the bottom of the conversation says something, and in the one-to-one with an architect replies to
// the question of the architect's the strip over the box names. Everything that answers reads this; a view with no conversation
// round it (a work's page, an item's, the architect's) shows what waits and where to answer it, and gives no form.

import { createContext, useContext } from 'react';

import type { JsonValue, OwnerAsk } from './api';
import { SPEAKER } from './http';
import { isObj, type Obj } from './labelled';
import { t } from './messages';

/// Whether what is drawn here is in a conversation, where the owner answers it; elsewhere it only reads.
export const AnswersHere = createContext<boolean>(false);

export const useAnswersHere = (): boolean => useContext(AnswersHere);

/// The owner's questions about a decision card and their answers, as the work's group holds them: each by the entry it
/// is, when it was said, and whether it is an answer.
export type CardAsked = { entry: string; at: number; answer: boolean };

/// What a work's group tells the decision cards and the messages in it of each other: each card's question, by its
/// number; the owner's questions about each card and their answers; and what brings a message into view (`go`, by its
/// entry). None outside a conversation.
export type CardTalkOf = {
  questions: ReadonlyMap<number, string>;
  asked: ReadonlyMap<number, CardAsked[]>;
  go: (entry: string) => void;
};
export const CardTalk = createContext<CardTalkOf | null>(null);

/// The option picked of an ask that waits, shared in its bubble between the boxes of its options in its card, which
/// pick it, and what answers the ask under them, which sends it: the ask, what is picked, and what a click on a box
/// does. None outside a bubble that answers an ask.
export type AskPick = { ask: number; picked: string; pick: (name: string) => void };
export const AskPicking = createContext<AskPick | null>(null);
export const useAskPick = (ask: number): AskPick | null => {
  const p = useContext(AskPicking);
  return p !== null && p.ask === ask ? p : null;
};

/// What an ask holds: what the cells handed in, each with its points and its choice. The engine keeps them under
/// `asked`, each with what it is and its value; an ask written as one (an architect's open choice) is its own.
export function askParts(body: Obj): Obj[] {
  const asked = Array.isArray(body.asked)
    ? body.asked
        .filter(isObj)
        .map((x) => x.value)
        .filter((v): v is Obj => v !== undefined && isObj(v))
    : [];
  return asked.length > 0 ? asked : [body];
}

/// The choice an ask puts, its question and its options, when it puts one.
export function askChoice(body: JsonValue): Obj | null {
  if (!isObj(body)) return null;
  return askParts(body).map((x) => x.choice).find((c): c is Obj => c !== undefined && isObj(c)) ?? null;
}


/// What is picked after a click on an option's box in an ask's bubble: what was clicked, or nothing when it was the one
/// picked already.
export const pickAfter = (now: string, clicked: string): string => (now === clicked ? '' : clicked);

/// Whether the answer to an ask needs the owner's words with what is picked: an ask in words alone, and an option ask
/// with nothing picked.
export const needsWords = (pick: string): boolean => pick === '';

/// Whether an ask is the merge gate's ask of a merge condition, whose option the engine reads as it was chosen.
export const judgesCondition = (a: OwnerAsk): boolean => isObj(a.body) && typeof a.body.condition === 'number';

/// What an ask's bubble sends to answer it: the option picked and words, or the words alone, and an engineer's handed
/// back to the architect with the words as the decision it carries out (`back`); images may go with them, or alone
/// (`images`). The merge gate's ask of a merge condition (`judges`) sends the option apart from the words, needing no words
/// with it. None while it cannot be sent.
export function askAnswer(pick: string, text: string, back: boolean, images = false, judges = false): object | null {
  const words = text.trim();
  if (judges) {
    if (pick !== '') return { text: words, speaker: SPEAKER, choice: pick };
    return words === '' && !images ? null : { text: words, speaker: SPEAKER };
  }
  const answer = pick !== '' ? `${t('answer_chosen', { option: pick })}${words !== '' ? ` ${words}` : ''}` : words;
  if (answer === '' && !images) return null;
  return back ? { text: answer, speaker: SPEAKER, to: 'architect' } : { text: answer, speaker: SPEAKER };
}

/// Whether an ask of an engineer's that the owner holds can be handed back to the architect with the owner's answer: not
/// a merge condition, which the architect does not judge.
export const handsBack = (a: OwnerAsk): boolean => isObj(a.body) && a.body.to === 'owner' && !judgesCondition(a);

/// Whether a key pressed in a box of the owner's words sends them: Enter sends and Shift with Enter starts a new line;
/// an Enter that ends a word typed through an input method (Chinese, for one) only ends the word.
export const enterSends = (e: { key: string; shiftKey: boolean; keyCode: number; nativeEvent: { isComposing: boolean } }): boolean =>
  e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing && e.keyCode !== 229;
