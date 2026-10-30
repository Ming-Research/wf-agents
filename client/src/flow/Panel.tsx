// The panel of what is open on a chart, in the window floating over the chart's right side: its title and who does it,
// a way back to what it showed before, a close button (Escape closes it too); minimised, its title and its close
// alone. The edges that drag the window's width and the share of it the page's panel takes are here too, kept in this
// browser.

import { ArrowLeftIcon, XIcon } from '@primer/octicons-react';
import { Heading, IconButton } from '@primer/react';
import { type PointerEvent as Press, type ReactNode, type Ref, useEffect, useRef, useState } from 'react';

import { LinkButton } from '../parts';
import { t } from '../text';

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

/// The edge of a part of the page that drags its size: a left edge its width, from its right, and a bottom edge its
/// height, from its top. The size is kept in this browser when the drag ends; a double click gives the part its own
/// size again.
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
  /// Whether the window it is in is minimised: its head is then the window's one row, its title and its close.
  minimised?: boolean;
  /// The button that minimises or expands the window, at the head's right end while the head is the window's top row.
  fold?: ReactNode;
  children: ReactNode;
  ref?: Ref<HTMLElement>;
}) {
  const { title, who, back, onBack, onClose, shown, minimised = false, fold = null, children, ref } = props;
  const body = useRef<HTMLDivElement>(null);
  useEffect(() => {
    body.current?.scrollTo({ top: 0 });
  }, [shown]);
  useEffect(() => {
    // Escape pressed in a menu floating over the page closes the menu and leaves the panel open.
    const key = (e: KeyboardEvent) => {
      const floating = document.getElementById('__primerPortalRoot__');
      if (e.key === 'Escape' && !(e.target instanceof Node && floating?.contains(e.target))) onClose();
    };
    document.addEventListener('keydown', key);
    return () => document.removeEventListener('keydown', key);
  }, [onClose]);
  return (
    <section className="panel" ref={ref}>
      <div className="panel-head">
        {back !== null && !minimised ? (
          <LinkButton className="panel-back" onClick={onBack}>
            <ArrowLeftIcon /> <span className="panel-back-to">{t('panel_back', { title: back })}</span>
          </LinkButton>
        ) : null}
        <div className="panel-name">
          <Heading as="h2" variant="small" className="panel-title">
            {title}
          </Heading>
          {who && !minimised ? <p className="panel-who">{who}</p> : null}
        </div>
        <IconButton size="small" variant="invisible" icon={XIcon} aria-label={t('close')} onClick={onClose} />
        {fold}
      </div>
      <div className="panel-body" ref={body}>
        {children}
      </div>
    </section>
  );
}

/// What the panel shows, at first `first`, and the trail back through what it showed before. Closing it forgets the
/// trail.
export function useTrail<S>(same: (a: S, b: S) => boolean, first: S | null = null) {
  const [state, setState] = useState<{ now: S | null; trail: S[] }>(() => ({ now: first, trail: [] }));
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
