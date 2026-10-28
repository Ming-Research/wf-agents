// How a work goes on, in words and controls, as its row in the list of works and its window show them: whether a
// round of it runs, what holds it back and what it waits for, and the owner's Pause, Continue and Step.

import { useState } from 'react';

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

/// A round of the work runs now, in words.
export function runningWords(c: Course): string {
  return c.running === 'pausing' ? t('course_pausing') : c.running === 'step' ? t('course_step') : t('course_running');
}

/// What taking the work over will do, for the note beside a control and the question before it.
export function takeoverWords(w: WorkSummary, status: Status | null): string {
  return t('course_outdated_hint', { version: w.flow_version, now: status?.flow_version ?? '' });
}

/// The owner's Pause, Continue and Step of a work, each a request, and what failed. With `ask`, Continue and Step ask
/// first what they will do (the cell that runs again, the round that runs, the takeover); without, the place they are
/// pressed from has said it.
export function useActions(w: WorkSummary, status: Status | null, ask: boolean) {
  const c = course(w, status);
  const can = controls(c, status);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const post = (path: string, then?: () => void) => {
    setBusy(true);
    setError(null);
    postJson(path, {}).then(
      () => {
        setBusy(false);
        reread('works', `work:${w.id}`);
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
  const pause = () => post(`/api/v1/works/${w.id}/pause`);
  const resume = () => {
    if (c.outdated && !takeover()) return;
    if (!c.outdated && c.held === 'engine' && !window.confirm(t('resume_confirm', { id: w.id, cell }))) return;
    post(`/api/v1/works/${w.id}/resume`);
  };
  const step = (then?: () => void) => {
    if (c.outdated ? ask && !takeover() : ask && !window.confirm(t('confirm_step', { id: w.id, cell }))) return;
    post(`/api/v1/works/${w.id}/step`, then);
  };
  return { c, can, busy, error, pause, resume, step };
}

/// The buttons of the controls the owner can use on a work now, each saying on hovering what it does.
export function Controls({ a, w, status, onStep }: {
  a: ReturnType<typeof useActions>;
  w: WorkSummary;
  status: Status | null;
  /// Where a step is asked, when the place shows its own Step.
  onStep: (() => void) | null;
}) {
  const { c, can } = a;
  const resumeHint =
    c.held === 'engine'
      ? status?.paused
        ? t('work_resume_engine_all_hint', { cell: w.at })
        : t('work_resume_engine_hint', { cell: w.at })
      : t('work_resume_hint');
  return (
    <span className="controls">
      {can.pause ? (
        <button type="button" className="control" onClick={a.pause} disabled={a.busy} title={t('work_pause_hint')}>
          {t('work_pause')}
        </button>
      ) : null}
      {can.resume ? (
        <button type="button" className="control" onClick={a.resume} disabled={a.busy} title={resumeHint}>
          {t('work_resume')}
        </button>
      ) : null}
      {can.step && onStep !== null ? (
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
    </span>
  );
}

/// Why a work is held back by itself, in words, for a work that is: paused by the owner, or stopped by the engine. A
/// work held only because every work is paused is not: it reads as what it is, ready or waiting for something, and the
/// bar says that nothing runs until Start all.
export function held(c: Course, w: WorkSummary): string | null {
  return c.held !== null && c.held !== 'all' ? heldWords(c, w.paused) : null;
}
