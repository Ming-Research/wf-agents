// The owner's conversations as the page works them out from what the interface reads, apart from drawing them: the
// list in its order and its groups, what a conversation is called, who an entry is from and to, what of it is pinned
// for the owner and what streams, the runs of what passes between agents, and what is to be marked read on opening it.

import type { Conversation, ConversationDetail, ConversationEntry } from './api';
import { words } from './labelled';
import { t } from './messages';

/// The conversations in the order the list shows them: those where something waits for the owner first, then the rest,
/// each part by its latest entry, newest first.
export function conversationOrder(list: Conversation[]): Conversation[] {
  const newest = [...list].sort((a, b) => b.last_at - a.last_at);
  return [...newest.filter((c) => c.waiting > 0), ...newest.filter((c) => c.waiting === 0)];
}

export type ConversationGroup = {
  /// The project; none for the group of conversations of no project, and for the one group when there is one project.
  project: string | null;
  conversations: Conversation[];
  waiting: number;
  unread: number;
};

/// The list in groups: by project when there are several, each in the order of its first conversation in the list,
/// those of no project (between two projects' architects, and wf-agents') in a group of their own; one group when there
/// is one project or none.
export function conversationGroups(list: Conversation[]): ConversationGroup[] {
  const ordered = conversationOrder(list);
  const group = (project: string | null, conversations: Conversation[]): ConversationGroup => ({
    project,
    conversations,
    waiting: conversations.reduce((n, c) => n + c.waiting, 0),
    unread: conversations.reduce((n, c) => n + c.unread, 0),
  });
  const projects = [...new Set(ordered.map((c) => c.project).filter((p): p is string => p !== null))];
  if (projects.length <= 1) return ordered.length === 0 ? [] : [group(null, ordered)];
  const out = projects.map((p) => group(p, ordered.filter((c) => c.project === p)));
  const none = ordered.filter((c) => c.project === null);
  if (none.length > 0) out.push(group(null, none));
  return out;
}

/// What a conversation is called in the list and over it.
export function conversationTitle(c: Pick<Conversation, 'key' | 'work' | 'item' | 'title'>): string {
  const at = c.key.indexOf(':');
  const kind = at < 0 ? c.key : c.key.slice(0, at);
  const rest = at < 0 ? '' : c.key.slice(at + 1);
  if (kind === 'work') {
    return c.item !== null
      ? t('conv_work_item', { id: c.work ?? rest, item: c.item, title: c.title ?? '' })
      : t('conv_work', { id: c.work ?? rest });
  }
  if (kind === 'architect') return t('conv_architect', { project: rest });
  if (kind === 'architects') {
    const [a, b = ''] = rest.split('|');
    return t('conv_architects', { a, b });
  }
  return t('conv_wf_agents');
}

/// Who a side of an entry is, in words: the owner, the engine, wf-agents, a line; for a message between two projects'
/// architects, the architect of the project.
export function party(e: ConversationEntry, side: string): string {
  if (e.source === 'between') return t('conv_architect_of', { project: side });
  if (side === 'owner') return t('conv_you');
  if (side === 'engine') return t('conv_engine');
  if (side === 'wf-agents') return t('conv_wf_agents');
  return words(side);
}

/// What of a conversation the owner sees pinned above its stream: the faults of wf-agents not read yet; the asks not
/// answered and the design cards not ruled come with their forms beside. Everything else streams, in time order.
export function pinnedOf(d: ConversationDetail): { faults: ConversationEntry[]; stream: ConversationEntry[] } {
  return {
    faults: d.entries.filter((e) => e.waiting && e.fault !== null),
    stream: d.entries.filter((e) => !e.waiting),
  };
}

/// The stream in runs: each entry to or from the owner on its own, and what passes between agents one after another in
/// one run, which the view folds.
export function runsOf(stream: ConversationEntry[]): Array<{ agents: boolean; entries: ConversationEntry[] }> {
  const out: Array<{ agents: boolean; entries: ConversationEntry[] }> = [];
  for (const e of stream) {
    const last = out[out.length - 1];
    if (e.agents && last !== undefined && last.agents) last.entries.push(e);
    else out.push({ agents: e.agents, entries: [e] });
  }
  return out;
}

/// What opening a conversation marks read: its agents' messages and the engine's notices unread, not the faults, which
/// the owner marks read where they are pinned.
export function toMarkRead(d: ConversationDetail): { ids: number[]; notices: number[] } {
  const unread = d.entries.filter((e) => e.read_at === null && e.fault === null);
  return {
    ids: unread.filter((e) => e.source === 'message').map((e) => e.id),
    notices: unread.filter((e) => e.source === 'notice').map((e) => e.id),
  };
}

/// The owner's words in a conversation that the agent they are for has not taken up yet.
export const pendingWords = (entries: ConversationEntry[]): ConversationEntry[] =>
  entries.filter((e) => e.pending && e.sender === 'owner');
