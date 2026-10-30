// How a work goes on, in words and controls, as its row in the list of works and its window show them: whether a
// round of it runs, what holds it back and what it waits for, and the owner's Start, Pause and Step, with the session
// Start and Step run the next round in.

import { IconButton } from '@primer/react';
import { useEffect, useRef, useState, useSyncExternalStore } from 'react';

import type { Status, Waits, WorkSummary } from './api';
import { controls, type Course, course, heldWords } from './components';
import type { Wait } from './graph/flowModel';
import { postJson } from './http';
import { reread } from './live';
import { forProject } from './route';
import { t } from './text';

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

/// The owner's Start, Pause and Step of a work, each a request, and what failed. With `ask`, Start and Step ask first
/// what they will do (the cell that runs again, the round that runs, the takeover, the architect's stop the owner
/// lifts); without, the place they are pressed from has said it. A work of the architect is started and paused as the
/// architect is: its later works go on the same way.
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
  return { c, can, busy, error, pause, start, step, newSession, setNewSession };
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
/// triangle, Pause two bars, Step a triangle against a bar, and the new session a speech bubble holding a plus. Octicons
/// has no step and no new session, and its play and pause are in circles, so the four are drawn alike here.
function glyph(of: 'start' | 'pause' | 'step' | 'session') {
  return function ControlIcon() {
    return (
      <svg viewBox="0 0 16 16" width="16" height="16" fill="currentColor" aria-hidden="true" focusable="false">
        {of === 'start' ? <path d="M5 3.2v9.6l7.6-4.8z" /> : null}
        {of === 'pause' ? <path d="M4.5 3.5h2.4v9H4.5zM9.1 3.5h2.4v9H9.1z" /> : null}
        {of === 'step' ? <path d="M3.5 3.5v9l6.4-4.5zM10.6 3.5h2v9h-2z" /> : null}
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

const ICONS = { start: glyph('start'), pause: glyph('pause'), step: glyph('step'), session: glyph('session') };

/// The names of Start, Pause and Step, said on hovering each and to a screen reader.
const NAMES = { start: 'work_start', pause: 'work_pause', step: 'work_step' } as const;

/// One of the owner's Start, Pause and Step, a small IconButton: hovering says its name and what it does, or why it
/// cannot now, and a screen reader is told its name.
export function Control({ icon, hint, onClick, disabled }: {
  icon: 'start' | 'pause' | 'step';
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
/// after which it goes on by itself; a free slot for the Start the owner gave; or, held back by the owner's pause or
/// every work's, Step while Step can run a round of it now, and else paused. None for a work that runs, goes on by
/// itself or is over.
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
  if (c.releasing) return { tag: t('wait_slot'), why };
  if (c.held === null) return null;
  const can = controls(c, status);
  return { tag: t(can.step && can.canStep ? 'wait_step' : 'wait_paused'), why };
}
