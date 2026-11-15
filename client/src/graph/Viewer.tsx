// A pan-and-zoom view of a drawing that fills the space it is given. Two fingers on a trackpad, or the wheel, move
// the drawing; a pinch, or Shift with two fingers, zooms at the pointer; dragging moves it. Space shows the whole
// drawing and remembers the zoom; Space again goes back to that zoom with the point under the pointer in the middle.
// A click that does not drag goes to what is under the pointer, as anywhere on the page; the click that ends a drag
// goes nowhere. What floats over the drawing, such as its map, stays where it is as the drawing moves under it, and is
// told the view each time it changes.
//
// Brought over from the chart of `old-main` (`client/src/graph/Viewer.tsx`), where a click that did not drag was handed
// to the chart as the element under the pointer, the pointer being held by the viewer from the press: here the pointer
// is held only once a press becomes a drag, so that a click reaches the button under it and the keyboard presses the
// same buttons; and nothing floats over the viewer's side, the panel being docked beside it.

import { type ReactNode, type Ref, useEffect, useImperativeHandle, useRef } from 'react';

export type Rect = { x: number; y: number; width: number; height: number };

/// The view: the zoom, and where the drawing's top left corner is, in the viewer's own pixels.
export type View = { s: number; x: number; y: number };

/// The size of what can be seen of the viewer, in its own pixels, from its top left corner.
export type Seen = { width: number; height: number };

/// A part of a viewer, in its own pixels: from its top left corner (`x0`, `y0`) to its bottom right (`x1`, `y1`).
export type Area = { x0: number; y0: number; x1: number; y1: number };

/// The part of a viewer of `box`'s size one can see: all of it.
export function seenOf(box: { width: number; height: number }): Area {
  return { x0: 0, y0: 0, x1: Math.max(0, box.width), y1: Math.max(0, box.height) };
}

/// How far, in pixels, a pointer may move between press and release for the press to still be a click, not a drag.
export const DRAG_LIMIT = 6;

/// Where the pointer was last seen, in the window's pixels; none when it is not known or it has left the window. It is
/// kept outside any viewer: a viewer is mounted again when its chart is laid out again, and a pointer that stays where it
/// is sends nothing to the new one, which so cannot learn from its own events whether the pointer is on it.
let pointer: { x: number; y: number } | null = null;
if (typeof document !== 'undefined') {
  const note = (e: MouseEvent) => {
    pointer = { x: e.clientX, y: e.clientY };
  };
  // In the capture phase, so that nothing on the page that stops an event keeps the pointer from being seen.
  document.addEventListener('pointermove', note, true);
  document.addEventListener('pointerdown', note, true);
  document.documentElement.addEventListener('pointerleave', () => {
    pointer = null;
  });
}

/// The view that shows the whole of a drawing of `drawing`'s size in the middle of `w`, with a margin round it and no
/// more than 1.6 times its size.
export function fitView(w: Area, drawing: { width: number; height: number }): View {
  const margin = 32;
  const s = Math.min(
    (w.x1 - w.x0 - margin) / Math.max(drawing.width, 1),
    (w.y1 - w.y0 - margin) / Math.max(drawing.height, 1),
    1.6,
  );
  return { s, x: w.x0 + (w.x1 - w.x0 - drawing.width * s) / 2, y: w.y0 + (w.y1 - w.y0 - drawing.height * s) / 2 };
}

/// The view at zoom `s` that brings the middle of `r` to the middle of what can be seen.
export function centredOn(r: Rect, s: number, w: Area): View {
  return { s, x: (w.x0 + w.x1) / 2 - (r.x + r.width / 2) * s, y: (w.y0 + w.y1) / 2 - (r.y + r.height / 2) * s };
}

/// The room `reveal` leaves round what it brings into sight.
const REVEAL_ROOM = 24;

/// The view `v` moved only as far as brings `r` wholly into `w`, with some room round it; where it is larger than `w`,
/// its top left corner.
export function revealed(r: Rect, v: View, w: Area): View {
  const along = (at: number, size: number, lo: number, hi: number) => {
    const room = Math.min(REVEAL_ROOM, Math.max(0, (hi - lo - size) / 2));
    if (at + size > hi - room) at = hi - room - size;
    if (at < lo + room) at = lo + room;
    return at;
  };
  const sx = r.x * v.s + v.x;
  const sy = r.y * v.s + v.y;
  const x = along(sx, r.width * v.s, w.x0, w.x1);
  const y = along(sy, r.height * v.s, w.y0, w.y1);
  return { s: v.s, x: v.x + x - sx, y: v.y + y - sy };
}

/// Whether `r` is wholly in what can be seen at the view `v`.
export function inSight(r: Rect, v: View, w: Area): boolean {
  const sx = r.x * v.s + v.x;
  const sy = r.y * v.s + v.y;
  return sx >= w.x0 && sy >= w.y0 && sx + r.width * v.s <= w.x1 && sy + r.height * v.s <= w.y1;
}

export type ViewerHandle = {
  /// Shows the whole drawing, or, when that is smaller than `least`, the top of it at that zoom, centred on `around`.
  start: (least: number, around: Rect | null) => void;
  /// Shows a rectangle of the drawing in the middle of what can be seen, at zoom `s`, at once.
  place: (r: Rect, s: number) => void;
  /// Brings a rectangle of the drawing to the middle of what can be seen. With `keep`, it keeps the zoom and moves
  /// only when the rectangle is not wholly in sight; without, it zooms in to a zoom one can read at.
  focus: (r: Rect, keep: boolean) => void;
  /// Brings a rectangle of the drawing to the middle of what can be seen at the zoom it has now.
  centre: (r: Rect) => void;
  /// Moves the drawing, at the zoom it has, only as far as brings a rectangle of it wholly into what can be seen but
  /// for `covered` pixels at the right, which a window over the viewer takes.
  reveal: (r: Rect, covered: number) => void;
  toggle: () => void;
  /// Calls `f` with the view and the size of what can be seen each time either changes, until the function returned
  /// is called.
  listen: (f: (v: View, seen: Seen) => void) => () => void;
  /// Moves to a view: in a fifth of a second with `smooth`, else at once.
  to: (v: View, smooth: boolean) => void;
};

export function Viewer(props: {
  /// The drawing's size, in its own pixels.
  width: number;
  height: number;
  children: ReactNode;
  /// What floats over the drawing, which neither moves nor zooms with it.
  over?: ReactNode;
  ref?: Ref<ViewerHandle>;
}) {
  const { width, height, ref } = props;
  const box = useRef<HTMLDivElement>(null);
  const stage = useRef<HTMLDivElement>(null);
  const view = useRef<View>({ s: 1, x: 0, y: 0 });
  const size = useRef({ w: width, h: height });
  size.current = { w: width, h: height };
  const anim = useRef({ id: 0, goal: null as View | null });
  const saved = useRef<number | null>(null);
  const dims = useRef<Seen>({ width: 0, height: 0 });
  const watchers = useRef(new Set<(v: View, seen: Seen) => void>());

  const api = useRef<
    (ViewerHandle & { whole: () => View; apply: () => void; measure: () => void; pointed: () => [number, number] | null }) | null
  >(null);
  if (api.current === null) {
    const apply = () => {
      const v = view.current;
      if (stage.current) stage.current.style.transform = `translate(${v.x}px, ${v.y}px) scale(${v.s})`;
      for (const f of watchers.current) f(v, dims.current);
    };
    // The part of the viewer one can see, in the viewer's own pixels.
    const seen = () => seenOf(box.current?.getBoundingClientRect() ?? new DOMRect(0, 0, 800, 600));
    // What those who listen are told can be seen, read again when the viewer changes size.
    const measure = () => {
      const el = box.current;
      if (!el) return;
      const w = seenOf({ width: el.clientWidth, height: el.clientHeight });
      dims.current = { width: w.x1 - w.x0, height: w.y1 - w.y0 };
    };
    // Where the pointer is on the viewer, in the viewer's own pixels; none when it is not on it: outside the viewer, out of
    // the window, or under something that floats over the page, such as a dialog.
    const pointed = (): [number, number] | null => {
      const el = box.current;
      if (!el || pointer === null) return null;
      const under = document.elementFromPoint(pointer.x, pointer.y);
      if (!under || !el.contains(under)) return null;
      const r = el.getBoundingClientRect();
      return [pointer.x - r.left, pointer.y - r.top];
    };
    const centre = (): [number, number] => {
      const w = seen();
      return [(w.x0 + w.x1) / 2, (w.y0 + w.y1) / 2];
    };
    const whole = (): View => fitView(seen(), { width: size.current.w, height: size.current.h });
    const land = (id: number, t: View) => {
      if (id !== anim.current.id) return;
      view.current = { ...t };
      anim.current.goal = null;
      apply();
    };
    // Moves to a view in a fifth of a second, so the eye can follow; the timer lands it there even when the browser
    // holds animation frames back.
    const moveTo = (t: View) => {
      const id = ++anim.current.id;
      const from = { ...view.current };
      const t0 = performance.now();
      anim.current.goal = t;
      const step = (now: number) => {
        if (id !== anim.current.id) return;
        const k = Math.min(1, (now - t0) / 200);
        const e = 1 - Math.pow(1 - k, 3);
        if (k >= 1) {
          land(id, t);
          return;
        }
        view.current = { s: from.s + (t.s - from.s) * e, x: from.x + (t.x - from.x) * e, y: from.y + (t.y - from.y) * e };
        apply();
        requestAnimationFrame(step);
      };
      requestAnimationFrame(step);
      window.setTimeout(() => land(id, t), 260);
    };
    const settle = () => {
      if (anim.current.goal) land(anim.current.id, anim.current.goal);
    };
    api.current = {
      whole,
      measure,
      apply,
      pointed,
      start: (least, around) => {
        const w = whole();
        if (w.s >= least) {
          view.current = w;
        } else {
          const [cx] = centre();
          const s = least;
          const focusX = around ? around.x + around.width / 2 : size.current.w / 2;
          const top = around ? around.y : 0;
          view.current = { s, x: cx - focusX * s, y: 24 - top * s };
        }
        apply();
      },
      place: (r, s) => {
        view.current = centredOn(r, s, seen());
        apply();
      },
      focus: (r, keep) => {
        settle();
        const v = view.current;
        const w = seen();
        if (keep && inSight(r, v, w)) return;
        const s = keep ? v.s : v.s < 0.5 ? Math.max(0.85, saved.current ?? 0) : v.s;
        moveTo(centredOn(r, s, w));
      },
      centre: (r) => {
        settle();
        moveTo(centredOn(r, view.current.s, seen()));
      },
      reveal: (r, covered) => {
        settle();
        const v = view.current;
        const all = seen();
        const w = { ...all, x1: Math.max(all.x0, all.x1 - covered) };
        if (inSight(r, v, w)) return;
        moveTo(revealed(r, v, w));
      },
      toggle: () => {
        settle();
        const t = whole();
        const v = view.current;
        if (Math.abs(v.s - t.s) > t.s * 0.01) {
          saved.current = v.s;
          moveTo(t);
          return;
        }
        const s = saved.current && saved.current > t.s * 1.05 ? saved.current : 1;
        const m = pointed() ?? centre();
        const [cx, cy] = centre();
        const px = (m[0] - v.x) / v.s;
        const py = (m[1] - v.y) / v.s;
        moveTo({ s, x: cx - px * s, y: cy - py * s });
      },
      listen: (f) => {
        watchers.current.add(f);
        f(view.current, dims.current);
        return () => {
          watchers.current.delete(f);
        };
      },
      to: (v, smooth) => {
        if (smooth) {
          moveTo(v);
          return;
        }
        anim.current.id++;
        anim.current.goal = null;
        view.current = { ...v };
        apply();
      },
    };
  }

  useImperativeHandle(ref, () => api.current as ViewerHandle, []);

  useEffect(() => {
    const el = box.current;
    const a = api.current;
    if (!el || !a) return;
    const local = (e: { clientX: number; clientY: number }): [number, number] => {
      const r = el.getBoundingClientRect();
      return [e.clientX - r.left, e.clientY - r.top];
    };
    const zoomAt = (f: number, cx: number, cy: number) => {
      anim.current.id++;
      anim.current.goal = null;
      const v = view.current;
      const n = Math.max(Math.min(0.15, a.whole().s * 0.8), Math.min(4, v.s * f));
      view.current = { s: n, x: cx - (cx - v.x) * (n / v.s), y: cy - (cy - v.y) * (n / v.s) };
      a.apply();
    };
    const clamp = (d: number) => Math.max(-50, Math.min(50, d));
    // A trackpad's pinch comes as the wheel with Ctrl. Shift turns a vertical scroll into a horizontal one in some
    // browsers, so either way zooms.
    const wheel = (e: WheelEvent) => {
      e.preventDefault();
      const k = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? el.clientHeight : 1;
      const dx = e.deltaX * k;
      const dy = e.deltaY * k;
      const [cx, cy] = local(e);
      if (e.ctrlKey || e.metaKey) {
        zoomAt(Math.exp(-clamp(dy) * 0.01), cx, cy);
        return;
      }
      if (e.shiftKey) {
        zoomAt(Math.exp(-clamp(Math.abs(dy) >= Math.abs(dx) ? dy : dx) * 0.004), cx, cy);
        return;
      }
      anim.current.id++;
      anim.current.goal = null;
      view.current = { ...view.current, x: view.current.x - dx, y: view.current.y - dy };
      a.apply();
    };
    const pts = new Map<number, [number, number]>();
    let last: [number, number] | null = null;
    let pinch: number | null = null;
    // Where the press that may become a drag went down; whether the press now is one, which the click that ends it
    // does not go on from.
    let down: { x: number; y: number } | null = null;
    let dragged = false;
    const hold = (id: number) => {
      if (!el.hasPointerCapture?.(id)) el.setPointerCapture?.(id);
      el.classList.add('dragging');
    };
    const pointerDown = (e: PointerEvent) => {
      if (e.button !== 0 && e.pointerType === 'mouse') return;
      anim.current.id++;
      anim.current.goal = null;
      if (pts.size === 0) {
        down = { x: e.clientX, y: e.clientY };
        dragged = false;
      }
      pts.set(e.pointerId, [e.clientX, e.clientY]);
      if (pts.size === 1) last = [e.clientX, e.clientY];
      if (pts.size === 2) {
        const p = [...pts.values()];
        pinch = Math.hypot(p[0][0] - p[1][0], p[0][1] - p[1][1]);
        dragged = true;
        for (const id of pts.keys()) hold(id);
      }
    };
    const pointerMove = (e: PointerEvent) => {
      if (!pts.has(e.pointerId)) return;
      pts.set(e.pointerId, [e.clientX, e.clientY]);
      if (!dragged && down && Math.hypot(e.clientX - down.x, e.clientY - down.y) > DRAG_LIMIT) {
        dragged = true;
        hold(e.pointerId);
      }
      if (!dragged) return;
      if (pts.size === 2 && pinch) {
        const p = [...pts.values()];
        const d = Math.hypot(p[0][0] - p[1][0], p[0][1] - p[1][1]);
        const [cx, cy] = local({ clientX: (p[0][0] + p[1][0]) / 2, clientY: (p[0][1] + p[1][1]) / 2 });
        zoomAt(d / pinch, cx, cy);
        pinch = d;
      } else if (pts.size === 1 && last) {
        view.current = {
          ...view.current,
          x: view.current.x + e.clientX - last[0],
          y: view.current.y + e.clientY - last[1],
        };
        last = [e.clientX, e.clientY];
        a.apply();
      }
    };
    const up = (e: PointerEvent) => {
      pts.delete(e.pointerId);
      if (pts.size < 2) pinch = null;
      if (pts.size === 1) last = [...pts.values()][0];
      if (pts.size === 0) {
        last = null;
        down = null;
        el.classList.remove('dragging');
        // The click that ends a drag comes with its release, or never where the release was outside the viewer.
        window.setTimeout(() => {
          if (pts.size === 0) dragged = false;
        }, 0);
      }
    };
    // The click that ends a drag goes on to nothing under it.
    const click = (e: MouseEvent) => {
      if (!dragged) return;
      dragged = false;
      e.stopPropagation();
      e.preventDefault();
    };
    const key = (e: KeyboardEvent) => {
      if (e.code !== 'Space' || a.pointed() === null) return;
      const t = e.target as HTMLElement | null;
      if (t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName))) return;
      e.preventDefault();
      // A button keeps focus after a click, and Space would press it again.
      if (t && t.tagName === 'BUTTON') t.blur();
      if (!e.repeat) a.toggle();
    };
    // Resizing keeps the zoom, and the point in the middle in the middle.
    let was = { w: el.clientWidth, h: el.clientHeight };
    a.measure();
    const resized =
      typeof ResizeObserver === 'undefined'
        ? null
        : new ResizeObserver(() => {
            const now = { w: el.clientWidth, h: el.clientHeight };
            a.measure();
            view.current = {
              ...view.current,
              x: view.current.x + (now.w - was.w) / 2,
              y: view.current.y + (now.h - was.h) / 2,
            };
            was = now;
            a.apply();
          });
    resized?.observe(el);
    el.addEventListener('wheel', wheel, { passive: false });
    el.addEventListener('pointerdown', pointerDown);
    el.addEventListener('pointermove', pointerMove);
    el.addEventListener('pointerup', up);
    el.addEventListener('pointercancel', up);
    el.addEventListener('click', click, true);
    document.addEventListener('keydown', key);
    return () => {
      resized?.disconnect();
      el.removeEventListener('wheel', wheel);
      el.removeEventListener('pointerdown', pointerDown);
      el.removeEventListener('pointermove', pointerMove);
      el.removeEventListener('pointerup', up);
      el.removeEventListener('pointercancel', up);
      el.removeEventListener('click', click, true);
      document.removeEventListener('keydown', key);
    };
  }, []);

  return (
    <div className="viewer" ref={box}>
      <div className="viewer-stage" ref={stage}>
        {props.children}
      </div>
      {props.over}
    </div>
  );
}
