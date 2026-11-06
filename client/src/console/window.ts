// What the window beside the console shows of one item: what the item is and where it stands, the work going on on it,
// its body (where it came from, the problem it solves and the plan for it, with the choices that wait for the owner
// where they stand in it), and the notes under it, the owner's and the replies. Pure, so that the window draws mock
// items as it will draw real ones.

import type { WorkState } from '../api';
import type { ItemStanding } from './items';

/// Who writes a note, or is named in one to receive it.
export type Who = 'owner' | 'engineer' | 'architect' | 'assistant';

/// A part of the body: a passage written in markdown, or a choice that waits for the owner where it stands in the
/// text. Changed when it changed since the owner last marked the item read.
export type Part =
  | { kind: 'text'; text: string; changed?: boolean }
  | {
      kind: 'choice';
      question: string;
      options: { label: string; detail: string }[];
      recommended: number | null;
      chosen: number | null;
      changed?: boolean;
    };

/// A note under the body: who wrote it, when, to whom, what of the body it quotes, and what it says.
export type Note = { who: Who; at: number; to: Who[]; quote: string | null; text: string };

export type WindowItem = {
  id: number;
  title: string;
  item: {
    standing: ItemStanding;
    goal: boolean;
    project: string;
    level: string | null;
    /// When the architect added it.
    madeAt: number;
    /// The items it waits for to merge before it can start.
    waitsFor: number[];
    unseen: boolean;
  };
  work: {
    id: number;
    state: WorkState;
    /// What it waits for, in words, while it waits.
    waits: string | null;
    stage: string | null;
    stages: string[];
    from: number;
    rounds: number;
    costUsd: number;
    bar: string;
    sentBack: number;
    tier: string;
  } | null;
  background: Part[];
  problem: Part[];
  plan: Part[];
  /// Whether the plan waits for the owner to confirm it: every work stops once its plan is written.
  planWaits: boolean;
  notes: Note[];
};

/// How many parts of the body changed since the owner last read the item.
export const changedIn = (w: WindowItem): number =>
  [...w.background, ...w.problem, ...w.plan].filter((p) => p.changed === true).length;

/// How many choices the plan has, and how many of them have their pick.
export function picks(w: WindowItem): { picked: number; all: number } {
  const choices = w.plan.filter((p) => p.kind === 'choice');
  return { picked: choices.filter((p) => p.chosen !== null).length, all: choices.length };
}

/// Who a note goes to: those named in it with `@`, or, with none named, the engineer while a work goes on and the
/// assistant otherwise.
export function recipients(text: string, working: boolean): Who[] {
  const named = new Set<Who>();
  for (const m of text.matchAll(/@(engineer|architect|assistant)\b/gi)) named.add(m[1].toLowerCase() as Who);
  if (!working) named.delete('engineer');
  if (named.size > 0) return [...named];
  return [working ? 'engineer' : 'assistant'];
}
