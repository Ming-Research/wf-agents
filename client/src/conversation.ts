// The owner's conversations as the page works them out from what the interface reads, apart from drawing them: the
// list in its parts, what a conversation is called, who an entry is from and to whom it is said, in which voice the
// page draws it, what of it streams, which question an architect pinned at the bottom of its one-to-one is open and
// whether the owner's words reply to it, what the box at the bottom says things to and sends, what a stage card's facts
// are, what is new for the owner to be told of, and where a fault of wf-agents can go and went.

import type {
  Conversation,
  ConversationDetail,
  ConversationEntry,
  Forwarded,
  Forwarding,
  OwnerAsk,
  PolicyDraft,
  StageCard,
  WordsState,
  WorkSummary,
} from './api';
import { isObj, words } from './labelled';
import { type Key, t, tn } from './messages';
import { joinedWords } from './voices';

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

/// The member a line speaks as: the architect for the line a flow version before set the architect's policy on, since
/// to the owner the architect is one; any other line as itself.
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
/// with an architect, the architect, to whose works the owner's words were said while they ran, as a flow version before
/// let the owner interject into them.
export function mention(e: ConversationEntry, kind: Conversation['kind'] = 'work'): string {
  if (e.source === 'between') return `@${t('conv_architect_of', { project: e.recipient })}`;
  if (e.recipient === 'both') {
    return kind === 'architect' ? `@${t('conv_architect_short')}` : `@${t('conv_engineer')} @${t('conv_architect_short')}`;
  }
  if (e.recipient === 'owner') return `@${t('conv_you')}`;
  if (e.recipient === 'architects') return `@${t('conv_every_architect')}`;
  if (e.recipient === 'architect') return `@${t('conv_architect_short')}`;
  if (e.recipient === 'policy') return `@${t('conv_architect_short')}`;
  return `@${words(e.recipient)}`;
}

/// Whether a message shows to whom it is said at its head: every one in a group; in the one-to-one with an architect,
/// the owner's words said to a work of the architect's while it ran, as a flow version before let the owner interject.
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
/// architect, which show in the bubble they answer, the design cards its stage cards hold, and the questions an
/// architect pinned for the owner's reply, which show at the bottom while they wait. An ask streams where it was asked,
/// waiting or answered, the owner's answer in it; a question once replied to, with the reply in it.
export function streamOf(d: ConversationDetail): ConversationEntry[] {
  const held = heldCards(d);
  const replies = ownerReplies(d.entries);
  const pinned = new Set(d.questions.map((q) => q.id));
  return d.entries.filter(
    (e) =>
      !(e.source === 'reply' && replies.get(e.matter ?? e.id) === e) &&
      !(e.source === 'ask' && pinned.has(e.id)) &&
      voiceOf(e, held) !== 'none',
  );
}

/// Whether an ask is one an architect put to the owner: answered by the owner's words to the architect with it ticked,
/// never in its bubble.
export const architectAsk = (a: OwnerAsk): boolean => isObj(a.body) && a.body.from === 'architect';

/// Whether an entry is the question whether the draft of a new policy that waits (`draft`) is approved: the ask the round
/// that wrote the draft put.
export const asksApproval = (e: ConversationEntry, draft: PolicyDraft | null): boolean =>
  draft !== null && e.source === 'ask' && e.work === draft.work && e.round === draft.round;

/// The questions an architect pinned for the owner's reply, as entries of its one-to-one, newest first: each shows at
/// the bottom while it waits, not where it was asked.
export function pinnedQuestions(d: ConversationDetail): ConversationEntry[] {
  return d.questions.flatMap((q) => d.entries.filter((e) => e.source === 'ask' && e.id === q.id).slice(0, 1));
}

/// The pinned question open at the bottom, the one the tick replies to: the one the owner opened, while it is still
/// pinned, else the newest; the others are folded.
export function openQuestion(pinned: ConversationEntry[], opened: number | null): ConversationEntry | null {
  return pinned.find((e) => e.id === opened) ?? pinned[0] ?? null;
}

/// How the owner set the tick for a question: on or off.
export type Ticked = { question: number; on: boolean };

/// Whether the owner's words reply to the question open: as the owner set the tick for it, else only while the
/// architect waits on it.
export function replyTicked(open: ConversationEntry | null, waitsOn: number | null, ticked: Ticked | null): boolean {
  if (open === null) return false;
  return ticked !== null && ticked.question === open.id ? ticked.on : waitsOn === open.id;
}

/// What the box at the bottom of the one-to-one sends to the architect: the owner's words, and the question they reply
/// to when it is ticked.
export const toArchitect = (text: string, replyTo: number | null): { text: string; reply_to?: number } =>
  replyTo === null ? { text } : { text, reply_to: replyTo };

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

/// What the line under the owner's words says of where they stand with the architect they were said to.
const WORDS_STATES: Record<string, Key> = {
  replying: 'words_replying',
  replied: 'words_replied',
  asked: 'matter_held',
  withdrawn: 'matter_withdrawn',
};

export const wordsStateText = (w: WordsState): string =>
  WORDS_STATES[w.state] !== undefined ? t(WORDS_STATES[w.state]) : words(w.state);

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
  if (s.reviews > 0) out.push(tn('stage_reviews', s.reviews));
  if (s.fixes > 0) out.push(tn('stage_fixes', s.fixes));
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
  return joinedWords([p.lead, p.round !== null ? `${p.state} ${p.round}` : p.state]);
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
