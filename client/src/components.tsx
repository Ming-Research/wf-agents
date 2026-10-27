// Small parts every view uses.

import type { ReactNode } from 'react';

import type { ItemState, JsonValue, RoundOutcome, Status, WorkState, WorkSummary } from './api';
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

/// A work's state as it is now: a round left open while the engine holds and runs nothing (one the engine stopped in,
/// or one a pause left open) is not running; the work is ready, and its next round closes that round first.
export function liveState(w: WorkSummary, status: Status | null): WorkState {
  if (w.state === 'running' && status !== null && status.mode === 'hold' && status.stepping !== w.id) return 'ready';
  return w.state;
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
