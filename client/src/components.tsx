// Small parts every view uses.

import type { ReactNode } from 'react';

import type { ItemState, JsonValue, RoundOutcome, Status, Waits, WorkState, WorkSummary } from './api';
import type { Resource } from './live';
import { t } from './text';

/// A JSON value laid out as a tree: long or multi-line text keeps its lines, and nested objects fold, those deeper than
/// `open` levels folded at first.
export function Json({ value, depth = 0, open = 2 }: { value: JsonValue | undefined; depth?: number; open?: number }) {
  if (value === undefined || value === null) return <span className="j-null">null</span>;
  if (typeof value === 'string') {
    if (value.includes('\n') || value.length > 120) return <pre className="j-text">{value}</pre>;
    return <span className="j-str">{JSON.stringify(value)}</span>;
  }
  if (typeof value === 'number' || typeof value === 'boolean') return <span className="j-lit">{String(value)}</span>;
  const entries: Array<[string, JsonValue]> = Array.isArray(value)
    ? value.map((v, i) => [String(i), v])
    : Object.entries(value);
  if (entries.length === 0) return <span className="j-lit">{Array.isArray(value) ? '[]' : '{}'}</span>;
  const label = Array.isArray(value) ? `[${entries.length}]` : `{${entries.length}}`;
  return (
    <details className="j-tree" open={depth < open}>
      <summary>{label}</summary>
      <div className="j-entries">
        {entries.map(([k, v]) => (
          <div key={k} className="j-entry">
            <span className="j-key">{k}</span>
            <Json value={v} depth={depth + 1} open={open} />
          </div>
        ))}
      </div>
    </details>
  );
}

/// A part of a view that folds; open by default.
export function Section(props: { title: ReactNode; open?: boolean; extra?: ReactNode; children: ReactNode }) {
  return (
    <details className="section" open={props.open ?? true}>
      <summary>
        <span className="section-title">{props.title}</span>
        {props.extra}
      </summary>
      <div className="section-body">{props.children}</div>
    </details>
  );
}

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
/// owner's pause, every work paused, or the engine's pause for a problem), which Continue or Start all lifts; and what
/// it waits for, after which it goes on by itself. Both can hold at once.
export type Course = {
  /// Done or ended: it goes on no more.
  over: boolean;
  /// A round of it runs now: after it the work goes on by itself, or stops because it is held back, or the round is
  /// the one the owner asked for.
  running: 'goes_on' | 'pausing' | 'step' | null;
  held: 'you' | 'all' | 'engine' | null;
  waits: Waits | null;
  /// Its flow version is older than the one the engine runs: when it goes on, it is taken over.
  outdated: boolean;
};

export function course(w: WorkSummary, status: Status | null): Course {
  if (w.state === 'done' || w.state === 'ended') {
    return { over: true, running: null, held: null, waits: null, outdated: false };
  }
  const held: Course['held'] =
    w.paused_by === 'engine' ? 'engine' : w.paused_by === 'owner' ? 'you' : status?.paused ? 'all' : null;
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
  };
}

/// What the owner can do with a work now: pause one that goes on by itself; let go on one held back by itself (with
/// every work paused, Start all does it for every work); run one round of one that does not go on by itself and waits
/// for nothing, which can run only while no other round asked for is under way and nothing stops every work.
export function controls(c: Course, status: Status | null) {
  if (c.over || status === null) return { pause: false, resume: false, step: false, canStep: false };
  return {
    pause: c.held === null,
    resume: c.held === 'engine' || (c.held === 'you' && !status.paused),
    step: c.held !== null && c.running === null && c.waits === null,
    canStep: status.stepping === null && status.halt === null,
  };
}

/// Why the engine pauses a work the owner paused without saying why, as the engine writes it.
const PAUSED_BY_OWNER = 'the owner paused it';

/// Why a work is held back by itself, in words: stopped by the engine, or paused by the owner.
export function heldWords(c: Course, why: string | null): string {
  if (c.held === 'engine') return t('course_stopped_why', { why: why ?? '' });
  return why !== null && why !== PAUSED_BY_OWNER ? t('course_paused_why', { why }) : t('course_paused');
}

export function WorkBadge({ state }: { state: WorkState }) {
  return <span className={`badge state-${state}`}>{t(`state_${state}`)}</span>;
}

export function ItemBadge({ state }: { state: ItemState }) {
  return <span className={`badge item-${state}`}>{t(`item_${state}`)}</span>;
}

export function OutcomeBadge({ outcome }: { outcome: RoundOutcome }) {
  return <span className={`badge outcome-${outcome}`}>{t(`outcome_${outcome}`)}</span>;
}

/// What a read gives: its view when it came, or that it is still coming, or why it failed.
export function Loaded<T>({ r, children }: { r: Resource<T>; children: (data: T) => ReactNode }) {
  if (r.data !== undefined) return <>{children(r.data)}</>;
  if (r.error !== null) return <div className="error">{t('error', { error: r.error })}</div>;
  return <div className="muted">{t('loading')}</div>;
}
