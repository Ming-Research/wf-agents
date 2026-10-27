// The panel that floats over the right side of a chart while something on it is open: its title and who does it,
// a way back to what it showed before, a close button (Escape closes it too), its left edge to drag to set its width
// and its bottom edge to set its height, which this browser keeps. The window over a work's chart on the left drags
// its width the same way, by its right edge, and its height by its bottom edge.

import { type PointerEvent as Press, type ReactNode, type Ref, useEffect, useRef, useState } from 'react';

import { t } from '../text';

const WIDTH_KEY = 'wf-agents.panel-width';
const HEIGHT_KEY = 'wf-agents.panel-height';
const PANEL_MIN = 320;
const PANEL_MIN_HEIGHT = 160;

/// A floating window's width or height as the owner dragged it, kept in this browser where it lets the page keep it;
/// none for the window's own.
export function savedSize(key: string, min: number): number | null {
  try {
    const n = Number.parseInt(localStorage.getItem(key) ?? '', 10);
    return Number.isFinite(n) && n >= min ? n : null;
  } catch {
    return null;
  }
}

/// The edge of a floating window that drags its size: the left edge of a window on the right of the chart and the right
/// edge of one on its left drag its width, the bottom edge of either its height, from its top. The size is kept in
/// this browser when the drag ends; a double click gives the window its own size again.
export function Grip({ edge, min, storage, label, onSize }: {
  edge: 'left' | 'right' | 'bottom';
  min: number;
  /// The key the size is kept under.
  storage: string;
  label: string;
  onSize: (size: number | null) => void;
}) {
  const grip = (e: Press<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.preventDefault();
    const el = e.currentTarget;
    const box = el.parentElement?.getBoundingClientRect();
    const right = box?.right ?? window.innerWidth;
    const left = box?.left ?? 0;
    const top = box?.top ?? 0;
    el.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) =>
      onSize(
        Math.max(
          min,
          Math.round(edge === 'left' ? right - ev.clientX : edge === 'right' ? ev.clientX - left : ev.clientY - top),
        ),
      );
    const up = () => {
      el.removeEventListener('pointermove', move);
      el.removeEventListener('pointerup', up);
      el.removeEventListener('pointercancel', up);
      try {
        const r = el.parentElement?.getBoundingClientRect();
        const size = edge === 'bottom' ? r?.height : r?.width;
        if (size) localStorage.setItem(storage, String(Math.round(size)));
      } catch {
        // The size is kept only where the browser lets it be.
      }
    };
    el.addEventListener('pointermove', move);
    el.addEventListener('pointerup', up);
    el.addEventListener('pointercancel', up);
  };
  const reset = () => {
    onSize(null);
    try {
      localStorage.removeItem(storage);
    } catch {
      // Nothing was kept.
    }
  };
  return (
    <div
      className={edge === 'bottom' ? 'height-grip' : `width-grip width-grip-${edge}`}
      role="separator"
      aria-orientation={edge === 'bottom' ? 'horizontal' : 'vertical'}
      aria-label={label}
      title={label}
      onPointerDown={grip}
      onDoubleClick={reset}
    />
  );
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
  const [width, setWidth] = useState<number | null>(() => savedSize(WIDTH_KEY, PANEL_MIN));
  const [height, setHeight] = useState<number | null>(() => savedSize(HEIGHT_KEY, PANEL_MIN_HEIGHT));
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
  return (
    <aside
      className="panel"
      ref={ref}
      style={{
        ...(width !== null ? { width: `min(${width}px, calc(100vw - 24px))` } : {}),
        ...(height !== null ? { height: `min(${height}px, calc(100% - 20px))`, bottom: 'auto' } : {}),
      }}
    >
      <Grip edge="left" min={PANEL_MIN} storage={WIDTH_KEY} label={t('panel_grip')} onSize={setWidth} />
      <Grip
        edge="bottom"
        min={PANEL_MIN_HEIGHT}
        storage={HEIGHT_KEY}
        label={t('panel_height_grip')}
        onSize={setHeight}
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
