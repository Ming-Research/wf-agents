// The owner's conversations as the page works them out from what the interface reads, apart from drawing them: the
// list in its parts, what a conversation is called, who an entry is from and to whom it is said, in which voice the
// page draws it, what of it streams, what the box at the bottom says things to and does with them (in the one-to-one
// with an architect, interjecting into what of it runs or adding to its queue), what a stage card's facts are, what is
// new for the owner to be told of, and where a fault of wf-agents can go and went.

import type {
  Conversation,
  ConversationDetail,
  ConversationEntry,
  Forwarded,
  Forwarding,
  StageCard,
  Wake,
  WorkSummary,
} from './api';
import { overWake } from './architect';
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

/// The member a line speaks as: the architect for the line that sets its policy, since to the owner the architect is
/// one; any other line as itself.
export const member = (line: string): string => (line === 'policy' ? 'architect' : line);

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
      return MEMBERS.includes(member(e.sender)) ? 'agent' : 'system';
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
  return words(member(side));
}

/// To whom an entry is said, as a group says it at the head of a message: @ and the member, or both; in the one-to-one
/// with an architect, which of its works the owner's words were said to: its work in hand, its policy setting, or both.
export function mention(e: ConversationEntry, kind: Conversation['kind'] = 'work'): string {
  if (e.source === 'between') return `@${t('conv_architect_of', { project: e.recipient })}`;
  if (e.recipient === 'both') {
    return kind === 'architect'
      ? `@${t('conv_architect_short')} @${t('conv_policy_setting')}`
      : `@${t('conv_engineer')} @${t('conv_architect_short')}`;
  }
  if (e.recipient === 'owner') return `@${t('conv_you')}`;
  if (e.recipient === 'architects') return `@${t('conv_every_architect')}`;
  if (e.recipient === 'architect') return `@${t('conv_architect_short')}`;
  if (e.recipient === 'policy') return `@${t('conv_policy_setting')}`;
  return `@${words(e.recipient)}`;
}

/// Whether a message shows to whom it is said at its head: every one in a group; in the one-to-one with an architect,
/// the owner's words said to a work of the architect's while it runs.
export const mentions = (e: ConversationEntry, kind: Conversation['kind']): boolean =>
  kind === 'work' || kind === 'projects' || e.source === 'between' || (kind === 'architect' && e.source === 'interjection');

/// Whether an entry is what an engineer put to the architect (a question, a failure, a report), which the owner may
/// answer in the group while the architect holds it.
export const isQuestion = (e: ConversationEntry): boolean => e.source === 'question' || e.source === 'report';

/// The owner's replies to what an engineer put to the architect, by its matter, each shown in the bubble it answers.
export function ownerReplies(entries: ConversationEntry[]): Map<number, ConversationEntry> {
  const put = new Set(entries.filter(isQuestion).map((e) => e.matter ?? e.id));
  return new Map(
    entries
      .filter((e) => e.source === 'reply' && e.sender === 'owner' && put.has(e.matter ?? e.id))
      .map((e) => [e.matter ?? e.id, e]),
  );
}

/// What streams in a conversation, oldest first: everything but the owner's replies to what an engineer put to the
/// architect, which show in the bubble they answer, and the design cards its stage cards hold. An ask streams where it
/// was asked, waiting or answered, the owner's answer in it.
export function streamOf(d: ConversationDetail): ConversationEntry[] {
  const held = heldCards(d);
  const replies = ownerReplies(d.entries);
  return d.entries.filter((e) => !(e.source === 'reply' && replies.get(e.matter ?? e.id) === e) && voiceOf(e, held) !== 'none');
}

/// Where an ask that waits for the owner is from the part of the stream in view, by its bounds and the stream's: above
/// it or below it, or none when any of it is in view.
export function awayOf(ask: { top: number; bottom: number }, view: { top: number; bottom: number }): 'above' | 'below' | null {
  if (ask.bottom <= view.top) return 'above';
  if (ask.top >= view.bottom) return 'below';
  return null;
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

/// One of an architect's works the owner's words can be put to now, as the box of its one-to-one names it: its work in
/// hand (`architect`) or its policy setting (`policy`), whether it waits for the owner, and whether a round of it runs.
export type LiveWork = { work: number; to: 'architect' | 'policy'; waits: boolean; runs: boolean };

/// The architect's works not over, which take what the owner says to them while they run: its work in hand first, then
/// its policy setting; none while nothing of it runs or waits for the owner.
export function liveWorks(wakes: Wake[]): LiveWork[] {
  const live = wakes
    .filter((w) => !overWake(w.work))
    .map((w): LiveWork => {
      const policy = w.work.graph === 'policy';
      return {
        work: w.work.id,
        to: policy ? 'policy' : 'architect',
        waits: policy && w.work.waits?.kind === 'answer',
        runs: w.work.state === 'running',
      };
    });
  return [...live.filter((l) => l.to === 'architect'), ...live.filter((l) => l.to === 'policy')];
}

/// What the box of the one-to-one with an architect does with the owner's words: interjects into what of it runs or
/// waits for the owner, which answers in the conversation and goes on, or adds them to its queue, a matter it takes up
/// in its turn; while nothing of it runs or waits, only the queue.
export type BoxMode = 'interject' | 'queue';

export const boxModes = (live: LiveWork[]): BoxMode[] => (live.length > 0 ? ['interject', 'queue'] : ['queue']);

/// Whom Interject is for until the owner ticks otherwise: the policy setting while it waits for the owner, else what
/// runs a round now, else the first that is live.
export function interjectFirst(live: LiveWork[]): number[] {
  const waiting = live.filter((l) => l.waits);
  if (waiting.length > 0) return waiting.map((l) => l.work);
  const running = live.filter((l) => l.runs);
  if (running.length > 0) return running.map((l) => l.work);
  return live.slice(0, 1).map((l) => l.work);
}

/// What the box says before anything is written in the one-to-one with an architect: to tick whom Interject is for,
/// what becomes of words interjected (said of what waits for the owner, its ask stays open) or added to the queue.
export function architectBoxHint(mode: BoxMode, live: LiveWork[], to: number[]): string {
  if (mode === 'queue') return t(live.length > 0 ? 'conv_queue_architect' : 'conv_say_architect');
  if (to.length === 0) return t('conv_say_nobody');
  return t(live.some((l) => l.waits && to.includes(l.work)) ? 'conv_interject_waiting' : 'conv_interject_architect');
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

/// One line of the menu a fault's Forward to… opens: a project, its architect in words, and whether the fault went
/// there already.
export type ForwardChoice = { project: string; label: string; sent: boolean };

/// The menu a fault's Forward to… opens: every project's architect, those the fault already went to marked, since it
/// goes to each once.
export function forwardMenu(projects: string[], forwards: Forwarded[]): ForwardChoice[] {
  return projects.map((p) => ({
    project: p,
    label: t('fault_forward_to', { project: p }),
    sent: forwards.some((f) => f.project === p),
  }));
}

/// What a fault's entry says of one place it went: to whose architect, by hand or by the owner's setting, and how it
/// stands there, in the queue or handled in a round, which the page links to.
export function forwardParts(f: Forwarded): { lead: string; state: string; round: string | null } {
  const lead = t(f.by === 'setting' ? 'fault_forwarded_auto' : 'fault_forwarded', { project: f.project });
  if (f.state === 'handled' && f.handled_work !== null && f.handled_round !== null) {
    return { lead, state: t('fault_handled_in'), round: `R${f.handled_work}-${f.handled_round}` };
  }
  return { lead, state: t(f.state === 'withdrawn' ? 'fault_withdrawn' : 'fault_in_queue'), round: null };
}

/// The same, as one line of words.
export function forwardWords(f: Forwarded): string {
  const p = forwardParts(f);
  return `${p.lead} · ${p.round !== null ? `${p.state} ${p.round}` : p.state}`;
}

/// The choices of the setting that forwards each new fault as it comes, each a value and its words: nobody (`''`)
/// first, as it is at first, then every project's architect, and the project it names now even when the
/// configuration no longer has it.
export function forwardNewChoices(f: Forwarding): Array<{ value: string; label: string }> {
  const projects = f.auto !== null && !f.projects.includes(f.auto) ? [...f.projects, f.auto] : f.projects;
  return [
    { value: '', label: t('fault_forward_nobody') },
    ...projects.map((p) => ({ value: p, label: t('fault_forward_to', { project: p }) })),
  ];
}
