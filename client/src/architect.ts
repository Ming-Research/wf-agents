// What an architect's page and its one-to-one work out from what the interface reads, apart from drawing it: how the
// architect stands, its queue in parts, each matter in words the owner reads at a glance, what became of each matter
// handled, the articles of the constitution in the order the owner reads them.

import type {
  Architect,
  ArchitectWork,
  MadeThing,
  Matter,
  MatterCard,
  MatterProgress,
  Status,
  WorkState,
  WorkSummary,
} from './api';
import { course, heldName } from './components';
import { runningWords } from './course';
import { isObj, str, words } from './labelled';
import { type Key, t, tn } from './messages';
import { href } from './route';

/// A work of the architect goes on no more once it reached its end or was ended.
export const overWork = (w: WorkSummary): boolean => w.state === 'done' || w.state === 'ended';

/// An architect's queue in its two parts: the matters in its queue, in the order they came; and those an ask of the
/// owner holds until answered.
export function queueParts(queue: Matter[]): { waiting: Matter[]; held: Matter[] } {
  const held = (m: Matter) => m.asked !== null && !m.answered;
  return { waiting: queue.filter((m) => !held(m)), held: queue.filter(held) };
}

/// The kinds of matter, as what is to be done.
const MATTER_KINDS: Record<string, Key> = {
  set_straight: 'matter_kind_set_straight',
  engineer_question: 'matter_kind_engineer_question',
  report: 'matter_kind_report',
  design_card: 'matter_kind_design_card',
  failure: 'matter_kind_failure',
  owner_words: 'matter_kind_owner_words',
  other_architect: 'matter_kind_other_architect',
  slots_free: 'matter_kind_slots_free',
  group_talk: 'matter_kind_group_talk',
  check_policy: 'matter_kind_check_policy',
  make_plan: 'matter_kind_make_plan',
  set_policy: 'matter_kind_set_policy',
};

/// The kinds of a place to set straight, as the flow means them.
const STRAIGHT: Record<string, Key> = {
  off_policy: 'straight_off_policy',
  drifted: 'straight_drifted',
  split: 'straight_split',
  dependencies: 'straight_dependencies',
  duplicate: 'straight_duplicate',
  order: 'straight_order',
  missing: 'straight_missing',
  mismatch: 'straight_mismatch',
};

/// The kinds of a failure.
const FAILURES: Record<string, Key> = {
  tool_failed: 'failure_tool_failed',
  environment_changed: 'failure_environment_changed',
};

/// The kind of a matter in words, as what is to be done, and what more its kind says where that helps: the kind of a
/// place to set straight and of a failure. An engineer's question carries no kind; one queued with a kind before asks
/// lost their kinds is named without it.
export function matterKind(kind: string, detail: string | null): string {
  // A fault of wf-agents the owner forwarded comes as a report, and is looked into rather than gone through.
  if (kind === 'report' && detail === 'fault') return t('matter_kind_fault');
  const key = MATTER_KINDS[kind];
  const name = key !== undefined ? t(key) : words(kind);
  if (detail === null) return name;
  const of = (keys: Record<string, Key>) => (keys[detail] !== undefined ? t(keys[detail]) : words(detail));
  const more = kind === 'set_straight' ? of(STRAIGHT) : kind === 'failure' ? of(FAILURES) : null;
  return more !== null ? t('matter_kind_more', { kind: name, more }) : name;
}

/// What the words of a matter are made from, alike in the queue and in its card in the conversation.
export type MatterWords = Pick<Matter, 'kind' | 'detail' | 'what' | 'fix'>;

/// What a matter asks to be done, in one sentence: for a place to set straight, where it is and what is wrong there, as
/// the check of what runs found it; for every other, what the engine wrote of it.
export function matterSentence(m: MatterWords): string {
  const fix = isObj(m.fix) ? m.fix : null;
  const where = fix !== null ? str(fix.where) : '';
  if (fix === null || where === '') return m.what;
  const wrong = str(fix.wrong);
  return wrong !== '' ? `${where}: ${wrong}` : where;
}

/// The steps a place to set straight is to be set straight by, in order, each whole.
export function matterSteps(m: MatterWords): string[] {
  const fix = isObj(m.fix) ? m.fix : null;
  return fix !== null && Array.isArray(fix.steps) ? fix.steps.map((x) => str(x)).filter((x) => x !== '') : [];
}

/// Where the design card a matter is about stands, in words: waiting for a ruling; replaced, by which card; or ruled,
/// confirmed, or disagreed with, by whom and on which decisions and why.
export function cardWords(c: MatterCard): string {
  if (c.status === 'open') return t('matter_card_waits', { id: c.id });
  if (c.status === 'replaced') {
    return c.replaced_by !== null
      ? t('matter_card_replaced', { id: c.id, by: c.replaced_by })
      : t('matter_card_replaced_none', { id: c.id });
  }
  const rulings = Array.isArray(c.rulings) ? c.rulings.filter(isObj) : [];
  const last = rulings[rulings.length - 1];
  if (last === undefined) return t('matter_card_ruled', { id: c.id });
  const who = str(last.by) === 'owner' ? t('matter_card_you') : str(last.by);
  const each = Array.isArray(last.decisions) ? last.decisions.filter(isObj) : null;
  const why = (text: string) => (text !== '' ? `: ${text}` : '');
  const against =
    each !== null
      ? each
          .filter((d) => str(d.verdict) === 'disagree')
          .map((d) => `${t('matter_card_decision', { n: str(d.decision) })}${why(str(d.text))}`)
      : str(last.verdict) === 'disagree'
        ? [t('matter_card_whole') + why(str(last.text))]
        : [];
  if (against.length === 0) return t('matter_card_confirmed', { id: c.id, who });
  return t('matter_card_disagreed', { id: c.id, who, what: against.join('; ') });
}

export type ArchitectState =
  /// It never worked.
  | { kind: 'never'; waiting: number }
  /// Its last work is over, since when.
  | { kind: 'idle'; since: number | null; waiting: number }
  /// A work of it goes on, handling a matter or none.
  | { kind: 'working'; work: WorkSummary; handling: Matter | null; waiting: number };

/// How an architect stands: never worked; no work of it running since its last one ended; or a work of it running,
/// with the matter it handles; and how many matters are in its queue.
export function architectState(works: ArchitectWork[], queue: Matter[]): ArchitectState {
  const waiting = queueParts(queue).waiting.length;
  const going = works.find((w) => !overWork(w.work));
  if (going !== undefined) {
    const handling = going.handling !== null ? (queue.find((m) => m.id === going.handling) ?? null) : null;
    return { kind: 'working', work: going.work, handling, waiting };
  }
  if (works.length === 0) return { kind: 'never', waiting };
  return { kind: 'idle', since: works[0].work.last_at, waiting };
}

/// How an architect stands, as the state of a work it stands as, which gives its colour and its mark, and in the words
/// its chip says it in.
export type Standing = { state: WorkState; words: string };

/// How an architect stands now, as the head of its one-to-one says it: a round of it running; held back while it works
/// (paused by the owner, with every work or by itself, or stopped by the engine or by itself); between rounds; or, with
/// no work of it running, held back by every work paused while matters wait for it, else idle with nothing in its queue
/// or about to take up the matters waiting.
export function architectNow(a: Architect, status: Status | null): Standing {
  const state = architectState(a.works, a.queue);
  if (state.kind === 'working') {
    const c = course(state.work, status);
    if (c.running !== null) {
      return { state: 'running', words: c.running === 'step' ? runningWords(c) : t('talk_architect_working') };
    }
    if (c.held !== null) return { state: c.held === 'engine' ? 'halted' : 'paused', words: heldName(c).name };
    return { state: 'ready', words: t('talk_architect_between') };
  }
  const started = status !== null && status.architects_started.includes(a.project);
  if (state.waiting > 0 && status !== null && status.paused && !started) {
    return { state: 'paused', words: t('talk_architect_held') };
  }
  return { state: 'ready', words: t(state.waiting > 0 ? 'talk_architect_takes_up' : 'talk_architect_idle') };
}

/// How an architect stands as its mark in the list of conversations shows it, a work's mark for the same state: as its
/// head says, except that while it neither runs a round nor is held back, a question of its own pinned for the owner's
/// reply marks it asking, as a work that asked is.
export function architectMark(a: Architect, status: Status | null): Standing {
  const now = architectNow(a, status);
  if (now.state === 'ready' && a.asks.some((x) => x.answer === null)) return { state: 'asking', words: t('state_asking') };
  return now;
}

/// One part of what came of a matter, as the summary on its bubble's line says it: its words, and where it leads, if
/// anywhere.
export type ResultPart = { key: string; words: string; to: string | null };

/// What came of a matter done, in one shape for every kind of result and every kind of matter: the summary on its
/// bubble's line, its parts in the order the owner reads them (`parts`), and the things it changed by name, one to a
/// line under it in the same order (`things`); when it changed one thing and nothing else came of it, the summary is
/// that thing itself, by its number and title and leading to it, and no line under it says it again (`one`).
export type MatterResult = { parts: ResultPart[]; things: MadeThing[]; one: MadeThing | null };

/// The things a matter changed, in the order they are read, by their kind and change, each with the words that count
/// them: the items its batch made, cancelled and moved in the queue, then the observations it recorded, merged a report
/// into and closed.
const THINGS: Array<{ kind: string; change: string; count: (n: number) => string }> = [
  { kind: 'item', change: 'new', count: (n) => tn('result_items_made', n) },
  { kind: 'item', change: 'cancelled', count: (n) => tn('result_items_cancelled', n) },
  { kind: 'item', change: 'reordered', count: (n) => tn('result_items_moved', n) },
  { kind: 'observation', change: 'new', count: (n) => tn('result_observations_recorded', n) },
  { kind: 'observation', change: 'merge', count: (n) => tn('result_observations_merged', n) },
  { kind: 'observation', change: 'close', count: (n) => tn('result_observations_closed', n) },
];

const ACTIONS: Record<string, Key> = {
  pause_work: 'handled_pause_work',
  resume_work: 'handled_resume_work',
  start_work_over: 'handled_start_work_over',
  hold_starts: 'handled_hold_starts',
  release_starts: 'handled_release_starts',
  interject: 'handled_interject',
};

/// What came of a matter done, from what its path did (`handled`) and the things it changed by name (`made`), each
/// part in the order the owner reads it: the reply taken back to the work that asked, leading to that work's group;
/// what its path told the owner and the ask it put; the items it made, cancelled and moved, its other item changes,
/// those still waiting for the review of their batch and those dropped; the observations it recorded, merged a report
/// into and closed; what it did at once; whom it wrote to. When nothing came of it, that, and why as the cell that
/// closed it said; when what came of it is not known any more, that it is done.
export function matterResult(m: MatterProgress): MatterResult {
  const h = m.handled;
  const things = THINGS.flatMap((k) => m.made.filter((x) => x.kind === k.kind && x.change === k.change));
  const parts: ResultPart[] = [];
  const counted = (kind: string) => {
    for (const k of THINGS.filter((x) => x.kind === kind)) {
      const n = things.filter((x) => x.kind === k.kind && x.change === k.change).length;
      if (n > 0) parts.push({ key: `${k.kind} ${k.change}`, words: k.count(n), to: null });
    }
  };
  if (h !== null && h.reply !== null) {
    const to = h.reply_to !== null ? href({ page: 'messages', open: `work:${h.reply_to}` }) : null;
    parts.push({ key: 'reply', words: t('result_replied', { work: h.reply_to ?? '' }), to });
  }
  if (h !== null && h.told > 0) parts.push({ key: 'told', words: tn('result_told', h.told), to: null });
  if (h !== null && h.ask !== null) parts.push({ key: 'ask', words: t('result_asked', { id: h.ask }), to: null });
  counted('item');
  if (h !== null) {
    const named = things.filter((x) => x.kind === 'item').length;
    const other = h.made - named;
    if (other > 0) {
      parts.push({ key: 'item other', words: tn(named > 0 ? 'result_changes_other' : 'result_changes', other), to: null });
    }
    if (h.collected > 0) parts.push({ key: 'item waiting', words: tn('result_changes_waiting', h.collected), to: null });
    if (h.dropped > 0) parts.push({ key: 'item dropped', words: tn('result_changes_dropped', h.dropped), to: null });
  }
  counted('observation');
  if (h !== null && h.actions.length > 0) {
    const did = h.actions.map((a) => (ACTIONS[a.kind] !== undefined ? t(ACTIONS[a.kind], { item: a.item ?? '' }) : a.kind));
    parts.push({ key: 'actions', words: t('result_did', { what: did.join('; ') }), to: null });
  }
  if (h !== null && h.messages.length > 0) {
    parts.push({ key: 'messages', words: t('result_wrote', { to: [...new Set(h.messages)].join(', ') }), to: null });
  }
  if (parts.length === 0) {
    const nothing = h === null ? t('matter_line_done') : h.why !== null ? t('result_nothing_why', { why: h.why }) : t('result_nothing');
    parts.push({ key: 'nothing', words: nothing, to: null });
  }
  return { parts, things, one: things.length === 1 && parts.length === 1 ? things[0] : null };
}

/// What stops a project's new works from starting, in words: the architect's own hold, or setting the project straight;
/// any other reason as it is written.
export function holdWords(reason: string): string {
  if (reason === 'architect') return t('hold_architect');
  if (reason === 'set_straight') return t('hold_set_straight');
  return t('hold_other', { reason: words(reason) });
}
