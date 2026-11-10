// The window beside the console's tree, kept at the right of the screen like a page of a binder: a narrow tab stuck to
// its left edge, which stays on the screen while the window is out of it; a press on the tab slides the window in along
// the edge, the tab with it, and another slides it back. Its left edge dragged changes its width, which the browser
// keeps. It floats over the tree, which keeps its whole width under it.
//
// Brought over from the console of the engine on `main` (`client/src/console/Drawer.tsx`), as it was; its chevrons are
// drawn here, as the page has no icon set.

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

/// A chevron 12 pixels square, pointing left or right, as Primer's octicon.
function Chevron({ to }: { to: 'left' | 'right' }) {
  return (
    <svg width="12" height="12" viewBox="0 0 12 12" fill="currentColor" aria-hidden="true">
      <path
        transform={to === 'left' ? 'matrix(-1 0 0 1 12 0)' : undefined}
        d="M4.7 10c-.2 0-.4-.1-.5-.2a.75.75 0 0 1 0-1.06L6.94 6 4.2 3.26a.75.75 0 0 1 1.06-1.06l3.27 3.27a.75.75 0 0 1 0 1.06L5.26 9.8a.75.75 0 0 1-.56.2Z"
      />
    </svg>
  );
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
        <Chevron to={open ? 'right' : 'left'} />
      </button>
      <div className="cw-edge" title="Drag to change the width" onPointerDown={drag} />
      <div className="cw-inner" aria-hidden={!open} inert={!open}>
        {children}
      </div>
    </aside>
  );
}
