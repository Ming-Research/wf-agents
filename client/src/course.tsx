// How a work goes on, in words and controls, as its row in the list of works and its window show them: whether a
// round of it runs, what holds it back and what it waits for, and the owner's Start, Pause and Step, with the session
// Start and Step run the next round in, and Start over.

import { ConfirmationDialog, IconButton } from '@primer/react';
import { useEffect, useRef, useState, useSyncExternalStore } from 'react';

import type { RunningRound, Status, Waits, WorkSummary } from './api';
import { controls, type Course, course, heldName, heldWords, PAUSED_BY_OWNER } from './components';
import type { Wait } from './graph/flowModel';
import { postJson } from './http';
import { reread } from './live';
import { words } from './labelled';
import { forProject } from './route';
import { type Key, t } from './text';

/// What a work waits for, in words.
export function waitsWords(w: Waits): string {
  switch (w.kind) {
    case 'answer':
      return t('waits_answer', { id: w.ask });
    case 'ruling':
      return t('waits_ruling', { id: w.design_card });
    case 'architect':
      return w.project === undefined
        ? t('waits_architect', { id: w.matter })
        : t('waits_architect_of', { id: w.matter, project: w.project });
    case 'items':
      return t('waits_items', { items: w.items.map((i) => `#${i}`).join(', ') });
    default:
      return t('waits_row', { cell: w.cell, row: w.row });
  }
}

/// A round of the work runs now, in words, saying when the owner started it.
export function runningWords(c: Course): string {
  if (c.running === 'pausing') return t('course_pausing');
  if (c.running === 'step') return t('course_step');
  return c.started ? t('course_running_started') : t('course_running');
}

/// Who a line is, in the owner's words: the engineer, the review, adversarial testing, the advisor, the architect.
const WHO: Record<string, Key> = {
  engineer: 'who_engineer',
  review: 'who_review',
  adversarial: 'who_adversarial',
  advisor: 'who_advisor',
  architect: 'who_architect',
};

export const lineWho = (line: string): string => (WHO[line] !== undefined ? t(WHO[line]) : words(line));

/// Who is at work in the round that runs, in plain words: the line whose round it is, its tools when a tool it asked
/// for runs, or the engine for its own steps and the tools it runs itself.
export function atWork(r: RunningRound): string {
  if (r.line === null) return t('now_engine');
  return t(r.tool ? 'now_tools' : 'now_running', { who: lineWho(r.line) });
}

/// The reason given for what holds a work back, apart from its name: the engine's or the architect's or the owner's
/// own words, every work paused by Pause all, or the Start that waits for a free slot; none for the owner's pause
/// with no reason of the owner's.
function heldWhy(c: Course, w: WorkSummary): string | null {
  if (c.releasing) return t('course_releasing_why');
  if (c.held === 'all') return t('course_all_why');
  return w.paused !== null && w.paused !== PAUSED_BY_OWNER ? w.paused : null;
}

/// A sentence's first letter in capital.
const sentence = (s: string): string => (s === '' ? s : s[0].toUpperCase() + s.slice(1));

/// What holds a work and what it waits for, in the order they act, as the head of its group says it after its state's
/// chip, which names the hold: why it is held, then what it waits for once it goes on; or what it waits for alone.
/// None when nothing holds it and it waits for nothing.
export function holdsWords(c: Course, w: WorkSummary): string | null {
  if (c.over || c.running !== null) return null;
  const parts: string[] = [];
  if (c.held !== null) {
    const why = heldWhy(c, w);
    if (why !== null) parts.push(`${sentence(why.trim()).replace(/[.]$/, '')}.`);
    if (c.waits !== null) parts.push(t('head_once_started', { waits: waitsWords(c.waits) }));
  } else if (c.waits !== null) {
    parts.push(`${sentence(waitsWords(c.waits))}.`);
  }
  return parts.length > 0 ? parts.join(' ') : null;
}

/// What a work waits for, as the words after "waits for", with no cell or row of the flow named: the owner's answer or
/// ruling, an architect's reply, the items it depends on, or, at a row that waits for none of these, what its next
/// round names.
function waitsFor(w: Waits): string {
  switch (w.kind) {
    case 'answer':
      return t('now_for_answer', { id: w.ask });
    case 'ruling':
      return t('now_for_ruling', { id: w.design_card });
    case 'architect':
      return w.project === undefined
        ? t('now_for_architect', { id: w.matter })
        : t('now_for_architect_of', { id: w.matter, project: w.project });
    case 'items':
      return t('now_for_items', { items: w.items.map((i) => `#${i}`).join(', ') });
    default:
      return t('now_for_row');
  }
}

/// What happens to a work now, in a few words, as the live line of the bubble of the stage it is at says it, in step
/// with its head and in the owner's words, with no cell's title or code, which its rounds' link leads to: who is at
/// work while a round runs; what holds it, and what it waits for once it goes on; what it waits for; a free slot; or
/// its next round due. At the merge stage what it waits for is what it must have before it can merge, and the line says
/// so; at the others the stage the bubble names is where it waits.
export function nowWords(c: Course, w: WorkSummary): string {
  if (c.running !== null && w.running !== null) return atWork(w.running);
  if (c.running !== null) return runningWords(c);
  const until = (now: string) => (w.stage === 'merge' ? t('now_before_merge', { now }) : now);
  if (c.held !== null) {
    const held = c.held === 'engine' ? heldWords(c, w.paused) : heldName(c).name;
    return c.waits !== null ? until(t('now_held_then', { held, what: waitsFor(c.waits) })) : held;
  }
  if (c.waits !== null) return until(t('now_waits', { what: waitsFor(c.waits) }));
  if (c.slot) return t('state_waiting_for_slot');
  return t('now_ready');
}

/// What taking the work over will do, for the note beside a control and the question before it.
export function takeoverWords(w: WorkSummary, status: Status | null): string {
  return t('course_outdated_hint', { version: w.flow_version, now: status?.flow_version ?? '' });
}

/// The session each work's next Start or Step runs its round in, as the owner chose it, by the work, or `architect` for
/// the architect: every place that shows them shares the choice. It is resuming the session at first, and again once a
/// Start or Step asked.
const fresh = new Map<string, boolean>();
const listeners = new Set<() => void>();

function subscribe(f: () => void): () => void {
  listeners.add(f);
  return () => listeners.delete(f);
}

export function useNewSession(key: string): [boolean, (on: boolean) => void] {
  const on = useSyncExternalStore(subscribe, () => fresh.get(key) ?? false);
  const set = (v: boolean) => {
    if (v) fresh.set(key, true);
    else fresh.delete(key);
    for (const f of listeners) f();
  };
  return [on, set];
}

/// The key of a work's session choice: the architect's for one of its works, whose new session is its work's.
export function sessionKey(w: WorkSummary): string {
  return w.graph === 'architect' ? 'architect' : `work:${w.id}`;
}

/// The owner's Start, Pause and Step of a work, each a request, its Start over, which `StartOver` asks first, and what
/// failed. With `ask`, Start and Step ask first what they will do (the cell that runs again, the round that runs, the
/// takeover, the architect's stop the owner lifts); without, the place they are pressed from has said it. A work of the
/// architect is started and paused as the architect is: its later works go on the same way.
export function useActions(w: WorkSummary, status: Status | null, ask: boolean) {
  const c = course(w, status);
  const can = controls(c, status);
  const arch = w.graph === 'architect';
  const [newSession, setNewSession] = useNewSession(sessionKey(w));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const post = (path: string, body: object, then?: () => void) => {
    setBusy(true);
    setError(null);
    postJson(path, body).then(
      () => {
        setBusy(false);
        reread('works', `work:${w.id}`, 'architect');
        then?.();
      },
      (e: unknown) => {
        setBusy(false);
        setError(e instanceof Error ? e.message : String(e));
      },
    );
  };
  const cell = `${w.at}${w.at_title ? ` (${w.at_title})` : ''}`;
  const takeover = () =>
    window.confirm(
      t('confirm_takeover', { id: w.id, version: w.flow_version, now: status?.flow_version ?? '', item: w.item ?? '' }),
    );
  const pause = () => post(arch ? forProject('/api/v1/architect/pause', w.project) : `/api/v1/works/${w.id}/pause`, {});
  const start = () => {
    if (c.outdated && !takeover()) return;
    if (c.held === 'architect' && !window.confirm(t('start_release_confirm', { id: w.id }))) return;
    if (!c.outdated && c.held === 'engine' && !window.confirm(t('resume_confirm', { id: w.id, cell }))) return;
    post(arch ? forProject('/api/v1/architect/start', w.project) : `/api/v1/works/${w.id}/start`, { new_session: newSession }, () =>
      setNewSession(false),
    );
  };
  const step = (then?: () => void) => {
    if (c.outdated ? ask && !takeover() : ask && !window.confirm(t('confirm_step', { id: w.id, cell }))) return;
    post(`/api/v1/works/${w.id}/step`, { new_session: newSession }, () => {
      setNewSession(false);
      then?.();
    });
  };
  const startOver = () => post(`/api/v1/works/${w.id}/start-over`, {}, () => reread('messages', 'items'));
  return { c, can, busy, error, pause, start, step, startOver, newSession, setNewSession };
}

/// What the owner's Start over of a work asks first: its question, and what follows in plain words: a round under way
/// is cut off, and its agent's process finishes before the new work begins; a new work on the item takes over its
/// branch, worktree and pull request from the first cell, given the owner's rulings and answers; and it is paused only
/// as the owner had paused this one.
export function startOverWords(w: WorkSummary, status: Status | null): { title: string; body: string } {
  const c = course(w, status);
  const pause =
    c.held === 'you'
      ? t('start_over_paused')
      : c.held === 'engine' || c.held === 'architect'
        ? t('start_over_not_paused')
        : t('start_over_goes_on');
  const parts = [c.running !== null ? t('start_over_cut') : null, t('start_over_new', { item: w.item ?? '' }), pause];
  return { title: t('start_over_title', { id: w.id }), body: parts.filter((x) => x !== null).join(' ') };
}

/// The owner's Start over of a work on an item that goes on, beside its Start, Pause and Step, in its group's head only:
/// a small IconButton saying on hovering what it does, which asks first in Primer's confirmation dialog, the request
/// sent only once the owner confirms.
export function StartOver({ a, w, status }: {
  a: ReturnType<typeof useActions>;
  w: WorkSummary;
  status: Status | null;
}) {
  const [asking, setAsking] = useState(false);
  if (w.graph === 'architect' || !(a.can.start || a.can.pause)) return null;
  const q = startOverWords(w, status);
  return (
    <>
      <Control icon="over" hint={t('work_start_over_hint')} onClick={() => setAsking(true)} disabled={a.busy} />
      {asking ? (
        <ConfirmationDialog
          title={q.title}
          confirmButtonContent={t('work_start_over')}
          confirmButtonType="danger"
          onClose={(gesture) => {
            setAsking(false);
            if (gesture === 'confirm') a.startOver();
          }}
        >
          {q.body}
        </ConfirmationDialog>
      ) : null}
    </>
  );
}

/// What opens the round the owner's Step asked for once it has ended, where it ran: in the work that took the asked
/// one over, when it was on an older flow version. What it returns marks the Step sent at `at`; `asked` is the work
/// stepped, or null for the architect's Step, which may start a new work of the architect.
export function useOpenStepped(
  status: Status | null,
  asked: number | null,
  open: (work: number, round: number) => void,
): (at: number) => void {
  const since = useRef<number | null>(null);
  const last = status?.last_step ?? null;
  useEffect(() => {
    if (since.current === null || last === null || last.at < since.current) return;
    if (asked !== null && last.asked !== asked) return;
    since.current = null;
    const r = last.rounds[last.rounds.length - 1];
    if (r !== undefined) open(last.work, r);
  });
  return (at) => {
    since.current = at;
  };
}

/// The icons of the owner's controls, at Octicons' size and drawn in the colour of their button's text: Start a
/// triangle, Pause two bars, Step a triangle against a bar, the new session a speech bubble holding a plus, and Start
/// over an arrow that turns back round to where it began. Octicons has no step, no new session and no start over, and
/// its play and pause are in circles, so the five are drawn alike here.
function glyph(of: 'start' | 'pause' | 'step' | 'session' | 'over') {
  return function ControlIcon() {
    return (
      <svg viewBox="0 0 16 16" width="16" height="16" fill="currentColor" aria-hidden="true" focusable="false">
        {of === 'start' ? <path d="M5 3.2v9.6l7.6-4.8z" /> : null}
        {of === 'pause' ? <path d="M4.5 3.5h2.4v9H4.5zM9.1 3.5h2.4v9H9.1z" /> : null}
        {of === 'step' ? <path d="M3.5 3.5v9l6.4-4.5zM10.6 3.5h2v9h-2z" /> : null}
        {of === 'over' ? (
          <g fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
            <path d="M8 3.5a4.5 4.5 0 1 1-4.5 4.5" />
            <path d="M9.75 1.75 8 3.5l1.75 1.75" />
          </g>
        ) : null}
        {of === 'session' ? (
          <g fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round">
            <path d="M2.5 4q0-1.5 1.5-1.5h8q1.5 0 1.5 1.5v5.5q0 1.5-1.5 1.5H7l-3 2.5V11q-1.5 0-1.5-1.5z" />
            <path d="M8 4.75v4M6 6.75h4" />
          </g>
        ) : null}
      </svg>
    );
  };
}

const ICONS = {
  start: glyph('start'),
  pause: glyph('pause'),
  step: glyph('step'),
  session: glyph('session'),
  over: glyph('over'),
};

/// The new session's speech bubble holding a plus, which also marks a round that started a new session in a list of
/// rounds.
export const SessionIcon = ICONS.session;

/// The names of Start, Pause, Step and Start over, said on hovering each and to a screen reader.
const NAMES = { start: 'work_start', pause: 'work_pause', step: 'work_step', over: 'work_start_over' } as const;

/// One of the owner's Start, Pause, Step and Start over, a small IconButton: hovering says its name and what it does,
/// or why it cannot now, and a screen reader is told its name.
export function Control({ icon, hint, onClick, disabled }: {
  icon: 'start' | 'pause' | 'step' | 'over';
  hint: string;
  onClick: () => void;
  disabled: boolean;
}) {
  const name = t(NAMES[icon]);
  return (
    <IconButton
      size="small"
      icon={ICONS[icon]}
      onClick={onClick}
      disabled={disabled}
      title={`${name}\n${hint}`}
      aria-label={name}
      unsafeDisableTooltip
    />
  );
}

/// The owner's choice of the session the next Start or Step runs its round in, as a small IconButton that stays
/// pressed: pressed, a new one given everything its cell needs; else the one its line goes on in.
export function SessionChoice({ on, set, disabled }: { on: boolean; set: (on: boolean) => void; disabled?: boolean }) {
  return (
    <IconButton
      size="small"
      icon={ICONS.session}
      className="session-choice"
      aria-pressed={on}
      disabled={disabled}
      title={`${t('session_new')}\n${t('session_choice_hint')}`}
      aria-label={t('session_new')}
      onClick={() => set(!on)}
      unsafeDisableTooltip
    />
  );
}

/// Why Step cannot run a round of a work now, in words, or null when it can: the work goes on, a round of it runs, it
/// waits for something, or another round asked for is under way or something stops every work.
export function stepWhyNot(c: Course, can: ReturnType<typeof controls>): string | null {
  if (!can.start) return t('work_step_goes_on');
  if (c.running !== null) return runningWords(c);
  if (c.waits !== null) return waitsWords(c.waits);
  if (!can.canStep) return t('work_step_later');
  return null;
}

/// The controls the owner has on a work not over, as icons, each saying on hovering what it does: Start while it is
/// held back or Pause while it goes on; Step, greyed with why while it cannot run a round; and beside it the choice of
/// a new session, greyed while the work goes on, since it is only for the next Start or Step.
export function Controls({ a, w, status, onStep }: {
  a: ReturnType<typeof useActions>;
  w: WorkSummary;
  status: Status | null;
  /// Where a step is asked, when the place shows its own Step.
  onStep: (() => void) | null;
}) {
  const { c, can } = a;
  const live = can.start || can.pause;
  const startHint = c.outdated
    ? takeoverWords(w, status)
    : c.held === 'architect'
      ? t('work_start_architect_hint')
      : c.held === 'engine'
        ? t('work_start_engine_hint', { cell: w.at })
        : t('work_start_hint');
  const whyNot = stepWhyNot(c, can);
  return (
    <span className="controls">
      {can.start ? <Control icon="start" hint={startHint} onClick={a.start} disabled={a.busy} /> : null}
      {can.pause ? (
        <Control
          icon="pause"
          hint={c.releasing ? `${t('course_releasing_why')}\n${t('work_pause_hint')}` : t('work_pause_hint')}
          onClick={a.pause}
          disabled={a.busy}
        />
      ) : null}
      {live && onStep !== null ? (
        <Control
          icon="step"
          hint={whyNot ?? (c.outdated ? t('step_takeover_note', { now: status?.flow_version ?? '' }) : t('work_step_hint'))}
          onClick={onStep}
          disabled={a.busy || whyNot !== null}
        />
      ) : null}
      {live ? <SessionChoice on={a.newSession} set={a.setNewSession} disabled={a.busy || !can.start} /> : null}
    </span>
  );
}

/// Why a work is held back, in words, for a work that is: paused by the owner, stopped by the engine or by the
/// architect, held by every work paused, or started by the owner and waiting for a free slot.
export function held(c: Course, w: WorkSummary): string | null {
  return c.held !== null ? heldWords(c, w.paused) : null;
}

/// Why a work waits at the cell it is at while no round of it runs, for the tag beside that cell on its chart, in a
/// few words and in full: stopped by the engine or the architect, which the owner looks at first; what it waits for,
/// after which it goes on by itself; a free slot, what it waited for come; a free slot for the Start the owner gave;
/// or, held back by the owner's pause or every work's, Step while Step can run a round of it now, and else paused.
/// None for a work that runs, goes on by itself or is over.
export function waitOf(c: Course, w: WorkSummary, status: Status | null): Wait | null {
  if (c.over || c.running !== null) return null;
  const why = held(c, w) ?? '';
  if (c.held === 'engine') return { tag: t('wait_engine'), why };
  if (c.held === 'architect') return { tag: t('wait_architect_stop'), why };
  if (c.waits !== null) {
    const kind = c.waits.kind;
    const tag =
      kind === 'answer' || kind === 'ruling'
        ? t('wait_you')
        : kind === 'architect'
          ? t('wait_architect')
          : kind === 'items'
            ? t('wait_items')
            : t('wait_row');
    return { tag, why: waitsWords(c.waits) };
  }
  if (c.slot) return { tag: t('wait_slot'), why: t('course_slot_why') };
  if (c.releasing) return { tag: t('wait_slot'), why };
  if (c.held === null) return null;
  const can = controls(c, status);
  return { tag: t(can.step && can.canStep ? 'wait_step' : 'wait_paused'), why };
}
