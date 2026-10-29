// What the owner answers with the one box at the bottom of a conversation, the messages view being the one place the
// owner acts: an ask pinned at its top, with the option or the ruling picked on it, or a question of an engineer's that
// the architect holds, which the owner may answer in the work's group instead. Everything that answers reads this; a
// view with no conversation round it (a work's page, an item's, the architect's) shows what waits and where to answer
// it, and gives no form.

import { createContext, useContext } from 'react';

import type { OwnerAsk } from './api';

/// What the box at the bottom answers instead of saying something: an ask, with the option picked among its choice's
/// (`option`) or the ruling picked among those its cell offers (`ruling`); or an engineer's question, by the
/// architect's matter that holds it, with its words.
export type Target =
  | { kind: 'ask'; ask: OwnerAsk; option: string; ruling: string }
  | { kind: 'question'; work: number; matter: number; text: string };

/// What answers in the conversation open: what the box answers now, and what sets it.
export type Answering = { target: Target | null; set: (t: Target | null) => void };

export const AnsweringContext = createContext<Answering | null>(null);

/// What answers here: none outside a conversation, where nothing is answered.
export const useAnswering = (): Answering | null => useContext(AnsweringContext);

/// The box's target for an ask: the same ask keeps what was picked on it; another starts with nothing picked.
export function askTarget(now: Target | null, ask: OwnerAsk, pick: { option?: string; ruling?: string } = {}): Target {
  const was = now !== null && now.kind === 'ask' && now.ask.id === ask.id ? now : null;
  return {
    kind: 'ask',
    ask,
    option: pick.option ?? was?.option ?? '',
    ruling: pick.ruling ?? was?.ruling ?? '',
  };
}

/// Whether the box answers this ask now.
export const answersAsk = (t: Target | null, ask: number): boolean => t !== null && t.kind === 'ask' && t.ask.id === ask;
