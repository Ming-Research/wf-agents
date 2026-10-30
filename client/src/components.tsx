// Small parts every view uses.

import {
  CheckCircleIcon,
  CircleIcon,
  ClockIcon,
  HourglassIcon,
  type Icon,
  PauseIcon,
  QuestionIcon,
  StopIcon,
} from '@primer/octicons-react';
import { Flash, Label, type LabelColorOptions } from '@primer/react';
import type { ReactNode } from 'react';

import type { ItemState, RoundOutcome, Status, Waits, WorkState, WorkSummary } from './api';
import { Halo } from './graph/flowDraw';
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
/// that round first; and a work the engine's status names as waiting for a free slot, what it waited for come, waits
/// for that slot, though as read it is ready or still waiting.
export function liveState(w: WorkSummary, status: Status | null): WorkState {
  const state = w.state === 'running' && status !== null && status.running !== w.id ? 'ready' : w.state;
  if (forSlot(w, status) && (state === 'ready' || state === 'waiting' || state === 'asking')) return 'waiting_for_slot';
  return state;
}

/// Whether the engine's status names a work as waiting for a free slot: what it waited for came, and it goes on once
/// its machine has a slot free, before any new work starts.
function forSlot(w: WorkSummary, status: Status | null): boolean {
  return status !== null && status.waiting_for_slot.includes(w.id);
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
  /// What it waits for, after which it goes on by itself; none once it came, though the work waits on for a slot.
  waits: Waits | null;
  /// What it waited for came, and it waits, holding no slot, until its machine has a slot free, as the engine's status
  /// says: it then goes on before any new work starts.
  slot: boolean;
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
    return {
      over: true,
      running: null,
      held: null,
      waits: null,
      slot: false,
      outdated: false,
      started: false,
      releasing: false,
    };
  }
  const started =
    status !== null &&
    (status.started.includes(w.id) ||
      (w.graph === 'architect' && w.project !== null && status.architects_started.includes(w.project)));
  const releasing = status !== null && status.releasing.includes(w.id);
  const slot = forSlot(w, status);
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
    waits: slot ? null : w.waits,
    slot,
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

/// The colour of a state's chip, as the UI spec gives each state its meaning: going on (accent), done (success), waiting
/// for someone (attention), gone wrong (danger), or none of these (neutral, grey); `done` is Primer's purple, for a
/// state Primer's own parts mark so.
export type Tone = 'accent' | 'success' | 'attention' | 'danger' | 'done' | 'neutral';

/// The Label variant a tone is drawn in: a state of no meaning of its own takes Primer's primary variant, which the page
/// fills grey (parts.css), so that it is a coloured chip as every state is, and not an uncoloured one, which is a tag's.
const VARIANT: Record<Tone, LabelColorOptions> = {
  accent: 'accent',
  success: 'success',
  attention: 'attention',
  danger: 'danger',
  done: 'done',
  neutral: 'primary',
};

/// The Label variant of a state's tone, for a part that draws the Label itself, such as a chip that is a link.
export const toneVariant = (tone: Tone): LabelColorOptions => VARIANT[tone];

/// A state, wherever the page shows one (a work's, an item's, a round's): a chip filled in the state's colour, the same
/// colour for the same state on every view. Every Label is drawn as a chip by parts.css.
export function StateChip({ tone, title, children }: { tone: Tone; title?: string; children: ReactNode }) {
  return (
    <Label variant={VARIANT[tone]} title={title}>
      {children}
    </Label>
  );
}

/// A tag or a kind (an observation's tag, a decision's change, a row's mark): an uncoloured chip, outlined, of a state's
/// chip's shape and height.
export function TagChip({ title, children }: { title?: string; children: ReactNode }) {
  return (
    <Label variant="secondary" title={title}>
      {children}
    </Label>
  );
}

/// A work's state's tone, which an architect takes for the state it stands in too.
export const WORK_TONE: Record<WorkState, Tone> = {
  running: 'accent',
  ready: 'neutral',
  asking: 'attention',
  waiting: 'attention',
  waiting_for_slot: 'attention',
  paused: 'attention',
  done: 'success',
  ended: 'neutral',
};

/// The Octicon of each state of a work but running, whose mark is the running halo: the one map for every place that
/// shows a work's state as an icon, as the UI spec names it. Ready is a ring at rest, the running halo's ring still;
/// asking a question; waiting a clock; waiting for a free slot an hourglass; paused two bars; done a tick in a ring;
/// ended GitHub's mark of a run stopped before its end, grey as GitHub draws a run cancelled.
export const WORK_MARK: Record<Exclude<WorkState, 'running'>, Icon> = {
  ready: CircleIcon,
  asking: QuestionIcon,
  waiting: ClockIcon,
  waiting_for_slot: HourglassIcon,
  paused: PauseIcon,
  done: CheckCircleIcon,
  ended: StopIcon,
};

/// The mark of a round running: the halo the chart draws round the cell a round runs at, at an icon's size round
/// nothing, its arcs as wide as they must be to read beside an Octicon's strokes; the one mark of a state that moves,
/// so that what runs looks the same on the chart and wherever else it shows.
export function RunningMark() {
  return (
    <svg className="running-mark" width={16} height={16} viewBox="0 0 16 16" aria-hidden="true">
      <Halo box={{ x: 5.5, y: 5.5, width: 5, height: 5 }} round small />
    </svg>
  );
}

/// A state as a mark, where a row has room for an icon and not for a chip (the list of conversations): a round running
/// as the running halo, every other state as its Octicon in the colour of its chip, saying on hovering what its chip
/// says (a work's state by default). Nothing known yet keeps the mark's room empty, so that the rows stay in line.
export function StateMark({ state, words }: { state: WorkState | null; words?: string }) {
  if (state === null) return <span className="state-mark" aria-hidden="true" />;
  const said = words ?? t(`state_${state}`);
  const Mark = state === 'running' ? null : WORK_MARK[state];
  return (
    <span className={`state-mark state-mark-${WORK_TONE[state]}`} title={said} role="img" aria-label={said}>
      {Mark === null ? <RunningMark /> : <Mark />}
    </span>
  );
}

/// An item's state's tone, which the items' tree gives the button that shows or hides the items in that state too.
export const ITEM_TONE: Record<ItemState, Tone> = {
  new: 'neutral',
  queued: 'attention',
  working: 'accent',
  done: 'success',
  stopped: 'danger',
  cancelled: 'danger',
};

const OUTCOME_TONE: Record<RoundOutcome, Tone> = {
  running: 'accent',
  took: 'success',
  sent_back: 'attention',
  failed: 'danger',
  interrupted: 'danger',
  ended: 'neutral',
};

/// A work's state as its chip; waiting for a free slot says on hovering why the work does not move, what it waited for
/// come.
export function WorkBadge({ state }: { state: WorkState }) {
  return (
    <StateChip tone={WORK_TONE[state]} title={state === 'waiting_for_slot' ? t('course_slot_why') : undefined}>
      {t(`state_${state}`)}
    </StateChip>
  );
}

export function ItemBadge({ state }: { state: ItemState }) {
  return <StateChip tone={ITEM_TONE[state]}>{t(`item_${state}`)}</StateChip>;
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
  return <StateChip tone={OUTCOME_TONE[outcome]}>{t(`outcome_${outcome}`)}</StateChip>;
}

/// What a read gives: its view when it came, or that it is still coming, or why it failed.
export function Loaded<T>({ r, children }: { r: Resource<T>; children: (data: T) => ReactNode }) {
  if (r.data !== undefined) return <>{children(r.data)}</>;
  if (r.error !== null) return <Flash variant="danger">{t('error', { error: r.error })}</Flash>;
  return <div className="muted">{t('loading')}</div>;
}
