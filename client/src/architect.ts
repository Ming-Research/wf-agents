// What an architect's page and its one-to-one work out from what the interface reads, apart from drawing it: the stretch
// of its work the chart shows, how the architect stands, its queue in parts, each matter in words the owner reads at a
// glance, what became of each matter handled, the observations the owner looks for, the articles of the constitution
// in the order the owner reads them.

import type {
  ArchitectWork,
  Handled,
  JsonValue,
  Matter,
  MatterCard,
  Observation,
  WorkSummary,
} from './api';
import { isObj, str, words, type Obj } from './labelled';
import { type Key, t, tn } from './messages';
import type { ArchitectTab } from './route';

/// A work of the architect goes on no more once it reached its end or was ended.
export const overWork = (w: WorkSummary): boolean => w.state === 'done' || w.state === 'ended';

/// The architect's work the page shows: the one the address names; else the one not over; else the latest; none before
/// the first.
export function shownWork(works: ArchitectWork[], named: number | null): ArchitectWork | null {
  const asked = named !== null ? works.find((x) => x.work.id === named) : undefined;
  return asked ?? works.find((x) => !overWork(x.work)) ?? works[0] ?? null;
}

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
/// place to set straight, of a failure and of an engineer's question.
export function matterKind(kind: string, detail: string | null): string {
  // A fault of wf-agents the owner forwarded comes as a report, and is looked into rather than gone through.
  if (kind === 'report' && detail === 'fault') return t('matter_kind_fault');
  const key = MATTER_KINDS[kind];
  const name = key !== undefined ? t(key) : words(kind);
  if (detail === null) return name;
  const of = (keys: Record<string, Key>) => (keys[detail] !== undefined ? t(keys[detail]) : words(detail));
  const more =
    kind === 'set_straight' ? of(STRAIGHT) : kind === 'failure' ? of(FAILURES) : kind === 'engineer_question' ? words(detail) : null;
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

/// One kind of outcome of a matter handled, on one line: what it is, what came of it, and the tab that holds the rest of
/// it, if one does.
export type Outcome = { key: string; label: string; text: string; tab: ArchitectTab | null };

const ACTIONS: Record<string, Key> = {
  pause_work: 'handled_pause_work',
  resume_work: 'handled_resume_work',
  hold_starts: 'handled_hold_starts',
  release_starts: 'handled_release_starts',
  interject: 'handled_interject',
};

/// What became of a matter handled, one line for each kind of outcome it had, in the order the owner reads them: the
/// reply taken back to the work that asked; what its path told the owner and the ask it put, both in the conversation;
/// the item changes it collected; the changes to the observations; what it did at once; whom it wrote to. When nothing
/// came of it, why, as the cell that closed it said.
export function outcomes(h: Handled): Outcome[] {
  const out: Outcome[] = [];
  const counted = (parts: Array<[number, Key]>) =>
    parts
      .filter(([n]) => n > 0)
      .map(([n, key]) => t(key, { n }))
      .join(', ');
  if (h.reply !== null) {
    out.push({ key: 'reply', label: t('handled_reply', { work: h.reply_to ?? '' }), text: h.reply, tab: null });
  }
  if (h.told > 0) out.push({ key: 'told', label: tn('handled_told', h.told), text: '', tab: null });
  if (h.ask !== null) out.push({ key: 'ask', label: t('handled_ask', { id: h.ask }), text: '', tab: null });
  const changes = counted([
    [h.made, 'handled_made'],
    [h.dropped, 'handled_dropped'],
    [h.collected, 'handled_collected'],
  ]);
  if (changes !== '') out.push({ key: 'changes', label: t('handled_changes'), text: changes, tab: 'changes' });
  const kinds = (kind: string) => h.observations.filter((k) => k === kind).length;
  const observed = counted([
    [kinds('new'), 'handled_recorded'],
    [kinds('merge'), 'handled_merged'],
    [kinds('close'), 'handled_closed'],
  ]);
  if (observed !== '') out.push({ key: 'observations', label: t('handled_observations'), text: observed, tab: 'observations' });
  if (h.actions.length > 0) {
    const did = h.actions.map((a) => (ACTIONS[a.kind] !== undefined ? t(ACTIONS[a.kind], { item: a.item ?? '' }) : a.kind));
    out.push({ key: 'actions', label: t('handled_did'), text: did.join('; '), tab: null });
  }
  if (h.messages.length > 0) {
    out.push({ key: 'messages', label: t('handled_wrote'), text: [...new Set(h.messages)].join(', '), tab: null });
  }
  if (out.length === 0) out.push({ key: 'nothing', label: t('handled_nothing'), text: h.why ?? '', tab: null });
  return out;
}

/// The observations the owner looks for: with the tag chosen, if one is, and the words searched for, if any, in what
/// it is, where it came from or what it refers to; those closed only when asked for. The order stays as read: the
/// open ones first, each newest first.
export function observationsShown(all: Observation[], tag: string, search: string, closed: boolean): Observation[] {
  const words = search.trim().toLowerCase();
  return all.filter(
    (o) =>
      (closed || o.closed === null) &&
      (tag === '' || o.tags.includes(tag)) &&
      (words === '' || [o.what, o.source, ...o.refs].some((x) => x.toLowerCase().includes(words))),
  );
}

/// The articles of the constitution as the architect last checked them: those broken first, then those at risk, then
/// those kept, each in the order checked.
export function againstInOrder(rows: JsonValue): Obj[] {
  const rank = (s: string) => (s === 'broken' ? 0 : s === 'at_risk' ? 1 : 2);
  const list = Array.isArray(rows) ? rows.filter(isObj) : [];
  return list
    .map((r, i) => ({ r, i }))
    .sort((a, b) => rank(str(a.r.status)) - rank(str(b.r.status)) || a.i - b.i)
    .map((x) => x.r);
}

/// What stops a project's new works from starting, in words: the architect's own hold, or setting the project straight;
/// any other reason as it is written.
export function holdWords(reason: string): string {
  if (reason === 'architect') return t('hold_architect');
  if (reason === 'set_straight') return t('hold_set_straight');
  return t('hold_other', { reason: words(reason) });
}
