// Small parts every view uses.

import { Flash, Label, type LabelColorOptions } from '@primer/react';
import type { ReactNode } from 'react';

import type { ItemState, RoundOutcome, Status, Waits, WorkState, WorkSummary } from './api';
import type { Resource } from './live';
import { t } from './text';

/// Label and value pairs.
export function Facts({ rows }: { rows: Array<[ReactNode, ReactNode] | null | false> }) {
  return (
    <dl className="facts">
      {rows.map((r, i) =>
        r ? (
          <div key={i} className="fact">
            <dt>{r[0]}</dt>
            <dd>{r[1]}</dd>
          </div>
        ) : null,
      )}
    </dl>
  );
}

/// A work's state as it is now: a round of it runs only while the engine takes its step; a round left open while the
/// engine runs nothing of it (one the engine stopped in) is not running, and the work is ready, its next round closing
/// that round first.
export function liveState(w: WorkSummary, status: Status | null): WorkState {
  if (w.state === 'running' && status !== null && status.running !== w.id) return 'ready';
  return w.state;
}

/// How a work goes on, in the owner's terms. Two things are kept apart: what holds it back though it could go on (the
/// owner's pause, every work paused, or the engine's or the architect's stop), which the work's Start lifts, and Start
/// all lifts for every work paused with them all; and what it waits for, after which it goes on by itself. Both can
/// hold at once. The later of the owner's actions wins: a work the owner started goes on while every work is paused,
/// until every work is paused again; a work the owner paused stays paused through Start all.
export type Course = {
  /// Done or ended: it goes on no more.
  over: boolean;
  /// A round of it runs now: after it the work goes on by itself, or stops because it is held back, or the round is
  /// the one the owner asked for.
  running: 'goes_on' | 'pausing' | 'step' | null;
  held: 'you' | 'all' | 'engine' | 'architect' | null;
  waits: Waits | null;
  /// Its flow version is older than the one the engine runs: when it goes on, it is taken over.
  outdated: boolean;
  /// The owner started it since every work was last paused (a work of the architect: the architect): it goes on
  /// whatever the global state.
  started: boolean;
  /// The owner started it while it was paused: its pause is lifted once its machine has a slot free.
  releasing: boolean;
};

export function course(w: WorkSummary, status: Status | null): Course {
  if (w.state === 'done' || w.state === 'ended') {
    return { over: true, running: null, held: null, waits: null, outdated: false, started: false, releasing: false };
  }
  const started =
    status !== null &&
    (status.started.includes(w.id) ||
      (w.graph === 'architect' && w.project !== null && status.architects_started.includes(w.project)));
  const releasing = status !== null && status.releasing.includes(w.id);
  const held: Course['held'] =
    w.paused_by === 'engine'
      ? 'engine'
      : w.paused_by === 'architect'
        ? 'architect'
        : w.paused_by === 'owner'
          ? 'you'
          : status?.paused && !started
            ? 'all'
            : null;
  let running: Course['running'] = null;
  if (status !== null && status.running === w.id) {
    running = status.stepping === w.id ? 'step' : held !== null ? 'pausing' : 'goes_on';
  }
  return {
    over: false,
    running,
    held,
    waits: w.waits,
    outdated: status !== null && w.flow_version !== status.flow_version,
    started,
    releasing,
  };
}

/// What the owner can do with a work now; a work over, or any while the engine's state is not known, has nothing.
/// Otherwise it has exactly one of Start and Pause: Start while it is held back (paused by the owner, every work
/// paused, or stopped by the engine or the architect), after which it goes on whatever the global state, its own pause
/// lifted; Pause while it goes on, or the owner started it and it waits for a slot, after which it stays paused
/// whatever the global state. Step, one round of it, is for a work held back that no round of it runs and that waits
/// for nothing (`step`), and can run only while no other round asked for is under way and nothing stops every work
/// (`canStep`).
export function controls(c: Course, status: Status | null) {
  if (c.over || status === null) return { pause: false, start: false, step: false, canStep: false };
  const back = c.held !== null && !c.releasing;
  return {
    pause: !back,
    start: back,
    step: back && c.running === null && c.waits === null,
    canStep: status.stepping === null && status.halt === null,
  };
}

/// Why the engine pauses a work the owner paused without saying why, as the engine writes it.
const PAUSED_BY_OWNER = 'the owner paused it';

/// What holds a work back, in a few words, and whether the engine stopped it for a problem the owner looks at: the
/// owner's Start waiting for a free slot, the owner's pause, the engine's or the architect's stop, or every work paused.
export function heldName(c: Course): { name: string; problem: boolean } {
  if (c.releasing) return { name: t('course_releasing'), problem: false };
  if (c.held === 'engine') return { name: t('course_stopped'), problem: true };
  if (c.held === 'architect') return { name: t('course_stopped_architect'), problem: false };
  if (c.held === 'all') return { name: t('course_all'), problem: false };
  return { name: t('course_paused'), problem: false };
}

/// Why a work is held back, in words: the owner's Start waiting for a free slot, stopped by the engine or by the
/// architect, every work paused, or paused by the owner.
export function heldWords(c: Course, why: string | null): string {
  if (c.releasing) return t('course_releasing_why');
  if (c.held === 'engine') return t('course_stopped_why', { why: why ?? '' });
  if (c.held === 'architect') return t('course_stopped_architect_why', { why: why ?? '' });
  if (c.held === 'all') return t('course_all_why');
  return why !== null && why !== PAUSED_BY_OWNER ? t('course_paused_why', { why }) : t('course_paused');
}

/// The colour of a state, as the UI spec gives each its meaning: going on, waiting for someone, done, gone wrong, or
/// none of these.
const WORK_TONE: Record<WorkState, LabelColorOptions> = {
  running: 'accent',
  ready: 'secondary',
  asking: 'attention',
  waiting: 'attention',
  paused: 'attention',
  done: 'success',
  ended: 'danger',
};

const ITEM_TONE: Record<ItemState, LabelColorOptions> = {
  new: 'secondary',
  queued: 'attention',
  working: 'accent',
  done: 'success',
  stopped: 'danger',
  cancelled: 'danger',
};

const OUTCOME_TONE: Record<RoundOutcome, LabelColorOptions> = {
  running: 'accent',
  took: 'success',
  sent_back: 'attention',
  failed: 'danger',
  interrupted: 'danger',
  ended: 'secondary',
};

export function WorkBadge({ state }: { state: WorkState }) {
  return <Label variant={WORK_TONE[state]}>{t(`state_${state}`)}</Label>;
}

export function ItemBadge({ state }: { state: ItemState }) {
  return <Label variant={ITEM_TONE[state]}>{t(`item_${state}`)}</Label>;
}

/// The kind of a dependency in a word, blocking or parallel, with what it means; nothing for none.
export function DependencyKind({ kind }: { kind: string | null }) {
  if (kind === null) return null;
  const blocking = kind === 'blocking';
  return (
    <span className="muted" title={t(blocking ? 'dependency_blocking_title' : 'dependency_parallel_title')}>
      {t(blocking ? 'dependency_blocking' : 'dependency_parallel')}
    </span>
  );
}

export function OutcomeBadge({ outcome }: { outcome: RoundOutcome }) {
  return <Label variant={OUTCOME_TONE[outcome]}>{t(`outcome_${outcome}`)}</Label>;
}

/// What a read gives: its view when it came, or that it is still coming, or why it failed.
export function Loaded<T>({ r, children }: { r: Resource<T>; children: (data: T) => ReactNode }) {
  if (r.data !== undefined) return <>{children(r.data)}</>;
  if (r.error !== null) return <Flash variant="danger">{t('error', { error: r.error })}</Flash>;
  return <div className="muted">{t('loading')}</div>;
}
