// The window beside the console's tree: a small tab at the top right that slides it out over the right of the tree, and
// slides back in; its left edge dragged to change its width, which this browser keeps.

import { type PointerEvent as ReactPointerEvent, type ReactNode, type RefObject, useState } from 'react';

import { t } from '../text';

const KEY = 'console-window-width';
const LEAST = 360;
const FIRST = 560;

/// The width kept from last time, or the first width.
function kept(): number {
  try {
    const v = Number(localStorage.getItem(KEY));
    return Number.isFinite(v) && v >= LEAST ? v : FIRST;
  } catch {
    return FIRST;
  }
}

export function Drawer({
  open,
  onOpen,
  panel,
  children,
}: {
  open: boolean;
  onOpen: () => void;
  panel: RefObject<HTMLElement | null>;
  children: ReactNode;
}) {
  const [width, setWidth] = useState(kept);
  // Dragging the left edge: the window keeps its right side and takes the rest, within its least width and most of
  // the screen; the width is kept once the drag ends.
  const drag = (e: ReactPointerEvent<HTMLDivElement>) => {
    e.preventDefault();
    const from = e.clientX;
    const was = width;
    let now = was;
    const move = (m: PointerEvent) => {
      now = Math.round(Math.min(window.innerWidth * 0.85, Math.max(LEAST, was + from - m.clientX)));
      setWidth(now);
    };
    const up = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      try {
        localStorage.setItem(KEY, String(now));
      } catch {
        // Kept for this page only.
      }
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  };
  return (
    <>
      <button type="button" className="cw-tab" hidden={open} onClick={onOpen}>
        {t('console_window_tab')}
      </button>
      <aside ref={panel} className={open ? 'cw cw-open' : 'cw'} style={{ width }} aria-hidden={!open} inert={!open}>
        <div className="cw-edge" title={t('console_window_resize')} onPointerDown={drag} />
        <div className="cw-inner">{children}</div>
      </aside>
    </>
  );
}
