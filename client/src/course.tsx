// How a work goes on, in words and controls, as its row in the list of works and its window show them: whether a
// round of it runs, what holds it back and what it waits for, and the owner's Start, Pause and Step, with the session
// Start and Step run the next round in.

import { useState, useSyncExternalStore } from 'react';

import type { Status, Waits, WorkSummary } from './api';
import { controls, type Course, course, heldWords } from './components';
import { postJson } from './http';
import { reread } from './live';
import { t } from './text';

/// What a work waits for, in words.
export function waitsWords(w: Waits): string {
  switch (w.kind) {
    case 'answer':
      return t('waits_answer', { id: w.ask });
    case 'ruling':
      return t('waits_ruling', { id: w.design_card });
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

/// The key of a work's session choice: the architect's for one of its wakes, whose new session is its wake's.
export function sessionKey(w: WorkSummary): string {
  return w.graph === 'architect' ? 'architect' : `work:${w.id}`;
}

/// The owner's Start, Pause and Step of a work, each a request, and what failed. With `ask`, Start and Step ask first
/// what they will do (the cell that runs again, the round that runs, the takeover, the architect's stop the owner
/// lifts); without, the place they are pressed from has said it. A wake of the architect is started and paused as the
/// architect is: its later wakes go on the same way.
export function useActions(w: WorkSummary, status: Status | null, ask: boolean) {
  const c = course(w, status);
  const can = controls(c, status);
  const wake = w.graph === 'architect';
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
  const pause = () => post(wake ? '/api/v1/architect/pause' : `/api/v1/works/${w.id}/pause`, {});
  const start = () => {
    if (c.outdated && !takeover()) return;
    if (c.held === 'architect' && !window.confirm(t('start_release_confirm', { id: w.id }))) return;
    if (!c.outdated && c.held === 'engine' && !window.confirm(t('resume_confirm', { id: w.id, cell }))) return;
    post(wake ? '/api/v1/architect/start' : `/api/v1/works/${w.id}/start`, { new_session: newSession }, () =>
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

/// The owner's choice of the session the next Start or Step runs its round in: the one its line goes on in, or a new
/// one given everything its cell needs.
export function SessionChoice({ on, set, disabled }: { on: boolean; set: (on: boolean) => void; disabled?: boolean }) {
  return (
    <select
      className="control session-choice"
      value={on ? 'new' : 'resume'}
      disabled={disabled}
      title={t('session_choice_hint')}
      aria-label={t('session_choice')}
      onChange={(e) => set(e.target.value === 'new')}
    >
      <option value="resume">{t('session_resume')}</option>
      <option value="new">{t('session_new')}</option>
    </select>
  );
}

/// The buttons of the controls the owner can use on a work now, each saying on hovering what it does, with the session
/// choice beside Start and Step.
export function Controls({ a, w, status, onStep }: {
  a: ReturnType<typeof useActions>;
  w: WorkSummary;
  status: Status | null;
  /// Where a step is asked, when the place shows its own Step.
  onStep: (() => void) | null;
}) {
  const { c, can } = a;
  const startHint =
    c.held === 'architect'
      ? t('work_start_architect_hint')
      : c.held === 'engine'
        ? t('work_start_engine_hint', { cell: w.at })
        : t('work_start_hint');
  const stepShown = can.step && onStep !== null;
  return (
    <span className="controls">
      {can.start ? (
        <button
          type="button"
          className="control"
          onClick={a.start}
          disabled={a.busy}
          title={c.outdated ? takeoverWords(w, status) : startHint}
        >
          {t('work_start')}
        </button>
      ) : null}
      {can.pause ? (
        <button type="button" className="control" onClick={a.pause} disabled={a.busy} title={t('work_pause_hint')}>
          {t('work_pause')}
        </button>
      ) : null}
      {stepShown ? (
        <button
          type="button"
          className="control"
          onClick={onStep}
          disabled={a.busy || !can.canStep}
          title={can.canStep ? (c.outdated ? takeoverWords(w, status) : t('work_step_hint')) : t('work_step_later')}
        >
          {t('work_step')}
        </button>
      ) : null}
      {can.start || stepShown ? <SessionChoice on={a.newSession} set={a.setNewSession} disabled={a.busy} /> : null}
    </span>
  );
}

/// Why a work is held back, in words, for a work that is: paused by the owner, stopped by the engine or by the
/// architect, held by every work paused, or started by the owner and waiting for a free slot.
export function held(c: Course, w: WorkSummary): string | null {
  return c.held !== null ? heldWords(c, w.paused) : null;
}
