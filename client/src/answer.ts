// How the owner answers in a conversation, the messages view being the one place the owner acts: an ask shows where it
// was asked, as the asking agent's bubble, and is answered in that bubble, by the buttons of what it offers (the
// options of its choice) and a small box of the owner's words beside an Answer button; the study and the text of a new
// policy are decided by the buttons of the choices their cell offers alone, a click sending the decision, the owner's
// words on them going to the architect in their talk; a question of an engineer's that the architect holds is answered
// in a small box in its bubble too. What is written there answers that bubble, and the box at the bottom of the
// conversation only ever says something. Everything that
// answers reads this; a view with no conversation round it (a work's page, an item's, the architect's) shows what
// waits and where to answer it, and gives no form.

import { createContext, useContext } from 'react';

import type { Choice, JsonValue, OwnerAsk } from './api';
import { rulingAnswer } from './architect';
import { isObj, str, type Obj } from './labelled';
import { t } from './messages';

/// Whether what is drawn here is in a conversation, where the owner answers it; elsewhere it only reads.
export const AnswersHere = createContext<boolean>(false);

export const useAnswersHere = (): boolean => useContext(AnswersHere);

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

/// Whether an ask is an architect's that the owner rules on with one of the choices its cell offers: the study of a
/// new policy, or a policy to approve.
const isRuling = (body: JsonValue): boolean =>
  isObj(body) && body.from === 'architect' && (body.kind === 'study' || body.kind === 'policy_approval');

/// How an ask is answered, which the buttons in its bubble offer: by one of the choices its cell offers, each saying
/// what it does and what happens next; by one of the options of its choice, by name, or in words alone; or in words
/// alone.
export type Way =
  | { by: 'ruling'; choices: Choice[] }
  | { by: 'option'; options: string[] }
  | { by: 'words' };

export function answerWay(a: OwnerAsk): Way {
  if (isRuling(a.body)) return { by: 'ruling', choices: a.choices };
  const choice = askChoice(a.body);
  const options = (choice !== null && Array.isArray(choice.options) ? choice.options.filter(isObj) : [])
    .map((o) => str(o.name))
    .filter((n) => n !== '');
  return options.length > 0 ? { by: 'option', options } : { by: 'words' };
}

/// What is picked after a click on a button in an ask's bubble: what was clicked, or nothing when it was the one
/// picked already.
export const pickAfter = (now: string, clicked: string): string => (now === clicked ? '' : clicked);

/// Whether the answer to an ask needs the owner's words with what is picked: an ask in words alone and an option ask
/// with nothing picked; a decision on a study or a text never, being its button alone.
export function needsWords(way: Way, pick: string): boolean {
  if (way.by === 'ruling') return false;
  return pick === '';
}

/// What an ask's bubble sends to answer it: an architect's choice (a study, a policy to approve) with the ruling
/// clicked alone; any other ask with the option picked and words, or the words alone, and an engineer's handed back to
/// the architect with the words as the decision it carries out (`back`). None while it cannot be sent.
export function askAnswer(a: OwnerAsk, pick: string, text: string, back: boolean): object | null {
  if (isRuling(a.body)) return rulingAnswer(a.choices, pick === '' ? null : pick);
  const words = text.trim();
  const answer = pick !== '' ? `${t('answer_chosen', { option: pick })}${words !== '' ? ` ${words}` : ''}` : words;
  if (answer === '') return null;
  return back ? { text: answer, to: 'architect' } : { text: answer };
}

/// Whether an ask of an engineer's that the owner holds can be handed back to the architect with the owner's answer.
export const handsBack = (a: OwnerAsk): boolean => isObj(a.body) && a.body.to === 'owner';

/// Whether a key pressed in a box of the owner's words sends them: Enter sends and Shift with Enter starts a new line;
/// an Enter that ends a word typed through an input method (Chinese, for one) only ends the word.
export const enterSends = (e: { key: string; shiftKey: boolean; keyCode: number; nativeEvent: { isComposing: boolean } }): boolean =>
  e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing && e.keyCode !== 229;
