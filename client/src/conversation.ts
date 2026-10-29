// The owner's conversations as the page works them out from what the interface reads, apart from drawing them: the
// list in its parts, what a conversation is called, who an entry is from and to whom it is said, in which voice the
// page draws it, what of it is pinned and what streams, what the box at the bottom says things to, what a stage card's
// facts are, and what is new for the owner to be told of.

import type { Conversation, ConversationDetail, ConversationEntry, StageCard, WorkSummary } from './api';
import { words } from './labelled';
import { t } from './messages';

/// The parts of the list, in its order, which waiting and new entries never change: the one-to-ones with the
/// architects and wf-agents' own; a group for each work, newest first; the groups of connected projects.
export type ListPart = { part: 'top' | 'works' | 'projects'; conversations: Conversation[] };

export function listParts(list: Conversation[]): ListPart[] {
  const part = (p: ListPart['part'], kinds: string[]): ListPart => ({
    part: p,
    conversations: list.filter((c) => kinds.includes(c.kind)),
  });
  return [part('top', ['architect', 'wf_agents']), part('works', ['work']), part('projects', ['projects'])].filter(
    (p) => p.conversations.length > 0,
  );
}

/// How many things wait for the owner across the conversations, and how many entries are unread.
export function totals(list: Conversation[]): { waiting: number; unread: number } {
  return {
    waiting: list.reduce((n, c) => n + c.waiting, 0),
    unread: list.reduce((n, c) => n + c.unread, 0),
  };
}

/// What a conversation is called in the list and over it.
export function conversationTitle(c: Pick<Conversation, 'key' | 'kind' | 'project' | 'projects' | 'work' | 'title'>): string {
  switch (c.kind) {
    case 'architect':
      return t('conv_architect', { project: c.project ?? c.key.slice('architect:'.length) });
    case 'work':
      return t('conv_work_item', { id: c.work ?? '', title: c.title ?? '' });
    case 'projects':
      return t('conv_projects', { projects: c.projects.join(', ') });
    default:
      return t('conv_wf_agents');
  }
}

/// The voice the page draws an entry in: the owner's words, on the right; an agent's, beside its avatar; the page's
/// own line for what the engine or a helper says; a card the engine writes from the flow (a stage, a matter, the
/// policy, an observation); a fault of wf-agents, which the owner marks read; or nothing, for a design card the stage
/// card that holds it shows.
export type Voice = 'owner' | 'agent' | 'system' | 'card' | 'fault' | 'none';

/// The lines that are members of a work's group besides the owner: its engineer and its project's architect.
const MEMBERS = ['engineer', 'architect'];

export function voiceOf(e: ConversationEntry, held: ReadonlySet<number>): Voice {
  switch (e.source) {
    case 'stage':
    case 'matter':
    case 'policy':
    case 'observation':
      return 'card';
    case 'notice':
      return e.fault !== null ? 'fault' : 'system';
    case 'card':
      return e.design_card !== null && held.has(e.design_card) ? 'none' : 'system';
    case 'between':
      return 'agent';
    default:
      if (e.sender === 'owner') return 'owner';
      return MEMBERS.includes(e.sender) ? 'agent' : 'system';
  }
}

/// The design cards the stage cards of a conversation hold, which show there and not as entries of their own.
export function heldCards(d: ConversationDetail): Set<number> {
  return new Set(d.entries.flatMap((e) => e.stage?.design_cards.map((c) => c.id) ?? []));
}

/// Who a party of an entry is, in words: you, the engine, a line; in a group of projects, the architect of a project.
export function party(e: ConversationEntry, side: string): string {
  if (e.source === 'between') return t('conv_architect_of', { project: side });
  if (side === 'owner') return t('conv_you');
  if (side === 'engine') return t('conv_engine');
  if (side === 'wf-agents') return t('conv_wf_agents');
  if (side === 'architects') return t('conv_every_architect');
  if (side === 'both') return t('conv_both');
  return words(side);
}

/// To whom an entry is said, as a group says it at the head of a message: @ and the member, or both.
export function mention(e: ConversationEntry): string {
  if (e.source === 'between') return `@${t('conv_architect_of', { project: e.recipient })}`;
  if (e.recipient === 'both') return `@${t('conv_engineer')} @${t('conv_architect_short')}`;
  if (e.recipient === 'owner') return `@${t('conv_you')}`;
  if (e.recipient === 'architects') return `@${t('conv_every_architect')}`;
  if (e.recipient === 'architect') return `@${t('conv_architect_short')}`;
  return `@${words(e.recipient)}`;
}

/// What streams in a conversation, oldest first: everything but the asks pinned at its top, which take their place in
/// the stream once answered, and the design cards its stage cards hold.
export function streamOf(d: ConversationDetail): ConversationEntry[] {
  const pinned = new Set(d.asks.map((a) => a.id));
  const held = heldCards(d);
  return d.entries.filter((e) => !(e.source === 'ask' && pinned.has(e.id)) && voiceOf(e, held) !== 'none');
}

/// The time of the latest entry of a conversation, up to which opening it reads it.
export function latestAt(d: ConversationDetail): number | null {
  return d.entries.reduce<number | null>((m, e) => (m === null || e.at > m ? e.at : m), null);
}

/// The owner's words in a conversation that the agent they are for has not taken up yet.
export const pendingWords = (entries: ConversationEntry[]): ConversationEntry[] =>
  entries.filter((e) => e.pending && e.sender === 'owner');

/// Whom the box at the bottom of a conversation can say something to, each one ticked or not: in a work's group its
/// engineer (ticked at first) and its architect; in a group of projects each project's architect; in the one-to-one the
/// architect alone; in wf-agents' own, nobody.
export function sayTo(c: Conversation): string[] {
  if (c.kind === 'work') return ['engineer', 'architect'];
  if (c.kind === 'projects') return [...c.projects];
  if (c.kind === 'architect') return ['architect'];
  return [];
}

/// What the parties ticked come to as whom a group's words are for: in a work's group `engineer`, `architect` or
/// `both`; in a group of projects `all` when every architect is ticked, else each ticked project; none when nobody is.
export function sayToTicked(c: Conversation, ticked: string[]): string[] {
  const all = sayTo(c);
  const on = all.filter((x) => ticked.includes(x));
  if (on.length === 0) return [];
  if (c.kind === 'work') return on.length === 2 ? ['both'] : on;
  if (c.kind === 'projects') return on.length === all.length && all.length > 1 ? ['all'] : on;
  return on;
}

/// Why the box of a work's group cannot say anything now, or null when it can: the work is at its end or over, and
/// its way on no longer changes.
export function closedWhy(c: Conversation, w: WorkSummary | null): string | null {
  if (c.kind !== 'work') return null;
  if (c.over || (w !== null && (w.state === 'done' || w.state === 'ended'))) return t('conv_closed_over');
  if (w !== null && w.at === 'E1') return t('conv_closed_end');
  return null;
}

/// The facts the engine knows of a stretch of a work at a stage, each a few words: the design cards handed in, the
/// fixes and reviews, the row it left by. Its time and cost are said apart.
export function stageFacts(s: StageCard): string[] {
  const out: string[] = [];
  for (const c of s.design_cards) out.push(t('stage_card', { id: c.id, status: words(c.status) }));
  if (s.reviews > 0) out.push(t('stage_reviews', { n: s.reviews }));
  if (s.fixes > 0) out.push(t('stage_fixes', { n: s.fixes }));
  if (s.left !== null) out.push(t('stage_left', { cell: s.left.cell, row: words(s.left.row) }));
  return out;
}

/// What the page tells the owner of, by the browser's notifications: each thing that waits for the owner in a
/// conversation and did not wait there when the list was read last, by the conversation; none the first time, when
/// everything is old.
export function newlyWaiting(before: ReadonlyMap<string, number> | null, now: Conversation[]): Conversation[] {
  if (before === null) return [];
  return now.filter((c) => c.waiting > (before.get(c.key) ?? 0));
}

/// What waits in each conversation, by its key, for the next comparison.
export const waitingByKey = (list: Conversation[]): Map<string, number> => new Map(list.map((c) => [c.key, c.waiting]));
