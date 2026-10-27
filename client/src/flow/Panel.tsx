// The panel that floats over the right side of a chart while something on it is open: its title and who does it,
// a way back to what it showed before, a close button (Escape closes it too), and its left edge to drag to set its
// width, which this browser keeps.

import { type PointerEvent as Press, type ReactNode, type Ref, useEffect, useRef, useState } from 'react';

import { t } from '../text';

const WIDTH_KEY = 'wf-agents.panel-width';

function savedWidth(): number | null {
  try {
    const n = Number.parseInt(localStorage.getItem(WIDTH_KEY) ?? '', 10);
    return Number.isFinite(n) && n >= 320 ? n : null;
  } catch {
    return null;
  }
}

export function Panel(props: {
  title: ReactNode;
  who?: ReactNode;
  /// The title of what the panel showed before, to go back to.
  back: string | null;
  onBack: () => void;
  onClose: () => void;
  /// Bumped each time the panel shows something else, so that it starts at its top.
  shown: string;
  children: ReactNode;
  ref?: Ref<HTMLElement>;
}) {
  const { title, who, back, onBack, onClose, shown, children, ref } = props;
  const [width, setWidth] = useState<number | null>(savedWidth);
  const body = useRef<HTMLDivElement>(null);
  useEffect(() => {
    body.current?.scrollTo({ top: 0 });
  }, [shown]);
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    document.addEventListener('keydown', key);
    return () => document.removeEventListener('keydown', key);
  }, [onClose]);
  const grip = (e: Press<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.preventDefault();
    const el = e.currentTarget;
    const right = el.parentElement?.getBoundingClientRect().right ?? window.innerWidth;
    el.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => setWidth(Math.max(320, Math.round(right - ev.clientX)));
    const up = () => {
      el.removeEventListener('pointermove', move);
      el.removeEventListener('pointerup', up);
      el.removeEventListener('pointercancel', up);
      try {
        const w = el.parentElement?.getBoundingClientRect().width;
        if (w) localStorage.setItem(WIDTH_KEY, String(Math.round(w)));
      } catch {
        // The width is kept only where the browser lets it be.
      }
    };
    el.addEventListener('pointermove', move);
    el.addEventListener('pointerup', up);
    el.addEventListener('pointercancel', up);
  };
  const reset = () => {
    setWidth(null);
    try {
      localStorage.removeItem(WIDTH_KEY);
    } catch {
      // Nothing was kept.
    }
  };
  return (
    <aside className="panel" ref={ref} style={width !== null ? { width: `min(${width}px, calc(100vw - 24px))` } : undefined}>
      <div
        className="panel-grip"
        role="separator"
        aria-orientation="vertical"
        aria-label={t('panel_grip')}
        title={t('panel_grip')}
        onPointerDown={grip}
        onDoubleClick={reset}
      />
      <div className="panel-head">
        {back !== null ? (
          <button type="button" className="panel-back" onClick={onBack}>
            ← <span className="panel-back-to">{t('panel_back', { title: back })}</span>
          </button>
        ) : null}
        <div className="panel-name">
          <h3 className="panel-title">{title}</h3>
          {who ? <p className="panel-who">{who}</p> : null}
        </div>
        <button type="button" className="panel-close" aria-label={t('close')} onClick={onClose}>
          ×
        </button>
      </div>
      <div className="panel-body" ref={body}>
        {children}
      </div>
    </aside>
  );
}

/// What the panel shows, and the trail back through what it showed before. Closing it forgets the trail.
export function useTrail<S>(same: (a: S, b: S) => boolean) {
  const [state, setState] = useState<{ now: S | null; trail: S[] }>({ now: null, trail: [] });
  const open = (s: S, from?: S) =>
    setState((old) => {
      const trail = [...old.trail];
      if (old.now !== null && !same(old.now, s)) trail.push(old.now);
      // Opened from a circle beside a cell: back goes to that cell.
      const top = trail.length > 0 ? trail[trail.length - 1] : null;
      if (from !== undefined && !same(from, s) && (top === null || !same(top, from))) trail.push(from);
      return { now: s, trail };
    });
  const back = () =>
    setState((old) => {
      const trail = [...old.trail];
      const prev = trail.pop();
      return prev === undefined ? old : { now: prev, trail };
    });
  const close = () => setState({ now: null, trail: [] });
  const last = state.trail.length > 0 ? state.trail[state.trail.length - 1] : null;
  return { now: state.now, last, open, back, close };
}
