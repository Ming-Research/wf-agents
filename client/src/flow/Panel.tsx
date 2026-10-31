// The panel of what is open on a chart, in the window floating over the chart's right side: its title and who does it
// and a way back to what it showed before; split under a page's panel, its close, which lets go of what is open and
// leaves the page's panel alone in the window; alone in the window, as on the flow page, no close, for the window has
// none and it stays open until something else is opened; minimised, its title alone. The edges that drag the window's
// width and the share of it the page's panel takes are here too, kept in this browser.

import { ArrowLeftIcon, XIcon } from '@primer/octicons-react';
import { Heading, IconButton } from '@primer/react';
import { type PointerEvent as Press, type ReactNode, type Ref, useEffect, useRef, useState } from 'react';

import { LinkButton } from '../parts';
import { t } from '../text';
import type { Fold } from './Dock';

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

/// The edge of a part of the page that drags its size: a left edge its width, from its right, a bottom edge its height,
/// from its top, and a top edge its height, from its bottom. The size dragged to is kept in this browser when the drag
/// ends; a double click gives the part its own size again.
export function Grip({ edge, min, storage, label, onSize }: {
  edge: 'left' | 'right' | 'bottom' | 'top';
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
    const bottom = box?.bottom ?? window.innerHeight;
    el.setPointerCapture(e.pointerId);
    // The size last dragged to, which a part as tall as what it holds, up to that size, may not have taken.
    let dragged: number | null = null;
    const move = (ev: PointerEvent) => {
      const at =
        edge === 'left'
          ? right - ev.clientX
          : edge === 'right'
            ? ev.clientX - left
            : edge === 'bottom'
              ? ev.clientY - top
              : bottom - ev.clientY;
      dragged = Math.max(min, Math.round(at));
      onSize(dragged);
    };
    const up = () => {
      el.removeEventListener('pointermove', move);
      el.removeEventListener('pointerup', up);
      el.removeEventListener('pointercancel', up);
      try {
        const r = el.parentElement?.getBoundingClientRect();
        const size = dragged ?? (edge === 'bottom' || edge === 'top' ? r?.height : r?.width);
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
      className={edge === 'bottom' || edge === 'top' ? `height-grip height-grip-${edge}` : `width-grip width-grip-${edge}`}
      role="separator"
      aria-orientation={edge === 'bottom' || edge === 'top' ? 'horizontal' : 'vertical'}
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
  /// Lets go of what is open, for a panel split under a page's panel; none for one alone in the window.
  onClose?: () => void;
  /// Bumped each time the panel shows something else, so that it starts at its top.
  shown: string;
  /// Whether the window it is in is minimised: its head is then the window's one row, its title alone.
  minimised?: boolean;
  /// What folds the window while the head is the window's top row: the button at the head's right end, and a click on
  /// the head, its title or where it stands, which does the same.
  fold?: Fold | null;
  children: ReactNode;
  ref?: Ref<HTMLElement>;
}) {
  const { title, who, back, onBack, onClose, shown, minimised = false, fold = null, children, ref } = props;
  const body = useRef<HTMLDivElement>(null);
  useEffect(() => {
    body.current?.scrollTo({ top: 0 });
  }, [shown]);
  return (
    <section className="panel" ref={ref}>
      <div
        className={fold !== null ? 'panel-head panel-head-folds' : 'panel-head'}
        onClick={fold?.onRow}
      >
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
        {onClose !== undefined && !minimised ? (
          <IconButton size="small" variant="invisible" icon={XIcon} aria-label={t('close')} onClick={onClose} />
        ) : null}
        {fold?.button ?? null}
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
