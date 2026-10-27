// Small parts every view uses.

import type { ReactNode } from 'react';

import type { ItemState, JsonValue, RoundOutcome, WorkState } from './api';
import type { Resource } from './live';
import { useT } from './text';

/// A JSON value laid out as a tree: long or multi-line text keeps its lines, and nested objects fold.
export function Json({ value, depth = 0 }: { value: JsonValue | undefined; depth?: number }) {
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
    <details className="j-tree" open={depth < 2}>
      <summary>{label}</summary>
      <div className="j-entries">
        {entries.map(([k, v]) => (
          <div key={k} className="j-entry">
            <span className="j-key">{k}</span>
            <Json value={v} depth={depth + 1} />
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

export function WorkBadge({ state }: { state: WorkState }) {
  const t = useT();
  return <span className={`badge state-${state}`}>{t(`state_${state}`)}</span>;
}

export function ItemBadge({ state }: { state: ItemState }) {
  const t = useT();
  return <span className={`badge item-${state}`}>{t(`item_${state}`)}</span>;
}

export function OutcomeBadge({ outcome }: { outcome: RoundOutcome }) {
  const t = useT();
  return <span className={`badge outcome-${outcome}`}>{t(`outcome_${outcome}`)}</span>;
}

/// What a read gives: its view when it came, or that it is still coming, or why it failed.
export function Loaded<T>({ r, children }: { r: Resource<T>; children: (data: T) => ReactNode }) {
  const t = useT();
  if (r.data !== undefined) return <>{children(r.data)}</>;
  if (r.error !== null) return <div className="error">{t('error', { error: r.error })}</div>;
  return <div className="muted">{t('loading')}</div>;
}
