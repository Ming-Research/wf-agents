// The window beside the console's tree, kept at the right of the screen like a page of a binder: a narrow tab stuck to
// its left edge, which stays on the screen while the window is out of it; a press on the tab slides the window in along
// the edge, the tab with it, and another slides it back. Its left edge dragged changes its width, which the browser
// keeps. It floats over the tree, which keeps its whole width under it.
//
// Brought over from the console of the engine on `main` (`client/src/console/Drawer.tsx`); its tab is the handle every
// panel of the page has.

import { type PointerEvent as ReactPointerEvent, type ReactNode, type RefObject, useState } from 'react';

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
  onToggle,
  panel,
  children,
}: {
  open: boolean;
  onToggle: () => void;
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
    <aside ref={panel} className={open ? 'cw cw-open' : 'cw'} style={{ width }} aria-label="The item selected">
      <button
        type="button"
        className="cw-tab"
        aria-expanded={open}
        aria-label={open ? 'Put the item away' : 'Show the item selected'}
        title={open ? 'Put the item away' : 'Show the item selected'}
        onClick={onToggle}
      >
        {open ? '›' : '‹'}
      </button>
      <div className="cw-edge" title="Drag to change the width" onPointerDown={drag} />
      <div className="cw-inner" aria-hidden={!open} inert={!open}>
        {children}
      </div>
    </aside>
  );
}
