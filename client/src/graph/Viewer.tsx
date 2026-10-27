// A pan-and-zoom view of a drawing that fills the space it is given. Two fingers on a trackpad, or the wheel, move
// the drawing; a pinch, or Shift with two fingers, zooms at the pointer; dragging moves it. Space shows the whole
// drawing and remembers the zoom; Space again goes back to that zoom with the point under the pointer in the middle.
// A click that does not drag picks what is under the pointer.

import { type ReactNode, type Ref, useEffect, useImperativeHandle, useRef } from 'react';

export type Rect = { x: number; y: number; width: number; height: number };

export type ViewerHandle = {
  /// Shows the whole drawing, or, when that is smaller than `least`, the top of it at that zoom, centred on `around`.
  start: (least: number, around: Rect | null) => void;
  /// Shows a rectangle of the drawing in the middle of what can be seen, at zoom `s`, at once.
  place: (r: Rect, s: number) => void;
  /// Brings a rectangle of the drawing to the middle of what can be seen. With `keep`, it keeps the zoom and moves
  /// only when the rectangle is not wholly in sight; without, it zooms in to a zoom one can read at.
  focus: (r: Rect, keep: boolean) => void;
  toggle: () => void;
};

type View = { s: number; x: number; y: number };

export type Covered = { left: number | null; right: number | null };

export function Viewer(props: {
  /// The drawing's size, in its own pixels.
  width: number;
  height: number;
  children: ReactNode;
  /// Called with the element under the pointer when a click does not drag.
  onPick: (target: Element) => void;
  /// What floats over the drawing, in page pixels: where a drawer over its left side ends, and where a panel over
  /// its right side begins; none for what is not open.
  covered: () => Covered;
  ref?: Ref<ViewerHandle>;
}) {
  const { width, height, onPick, covered, ref } = props;
  const box = useRef<HTMLDivElement>(null);
  const stage = useRef<HTMLDivElement>(null);
  const view = useRef<View>({ s: 1, x: 0, y: 0 });
  const size = useRef({ w: width, h: height });
  size.current = { w: width, h: height };
  const anim = useRef({ id: 0, goal: null as View | null });
  const saved = useRef<number | null>(null);
  const mouse = useRef<[number, number] | null>(null);
  const pick = useRef(onPick);
  pick.current = onPick;
  const cover = useRef(covered);
  cover.current = covered;

  const api = useRef<ViewerHandle & { whole: () => View; apply: () => void } | null>(null);
  if (api.current === null) {
    const apply = () => {
      const v = view.current;
      if (stage.current) stage.current.style.transform = `translate(${v.x}px, ${v.y}px) scale(${v.s})`;
    };
    // The part of the viewer one can see, not under the drawer or the panel, in the viewer's own pixels.
    const seen = () => {
      const r = box.current?.getBoundingClientRect() ?? new DOMRect(0, 0, 800, 600);
      let x0 = 0;
      let x1 = r.width;
      const { left, right } = cover.current();
      if (window.innerWidth > 600) {
        if (right !== null && right - r.left > 160) x1 = Math.min(x1, right - r.left - 8);
        if (left !== null && x1 - (left - r.left) > 240) x0 = Math.max(x0, left - r.left + 8);
      }
      return { x0, y0: 0, x1, y1: r.height };
    };
    const centre = (): [number, number] => {
      const w = seen();
      return [(w.x0 + w.x1) / 2, (w.y0 + w.y1) / 2];
    };
    const whole = (): View => {
      const w = seen();
      const { w: dw, h: dh } = size.current;
      const s = Math.min((w.x1 - w.x0 - 32) / Math.max(dw, 1), (w.y1 - w.y0 - 32) / Math.max(dh, 1), 1.6);
      return { s, x: w.x0 + (w.x1 - w.x0 - dw * s) / 2, y: w.y0 + (w.y1 - w.y0 - dh * s) / 2 };
    };
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
      apply,
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
        const [cx, cy] = centre();
        view.current = { s, x: cx - (r.x + r.width / 2) * s, y: cy - (r.y + r.height / 2) * s };
        apply();
      },
      focus: (r, keep) => {
        settle();
        const v = view.current;
        const w = seen();
        const [cx, cy] = centre();
        const sx = r.x * v.s + v.x;
        const sy = r.y * v.s + v.y;
        const inside = sx >= w.x0 && sy >= w.y0 && sx + r.width * v.s <= w.x1 && sy + r.height * v.s <= w.y1;
        if (keep && inside) return;
        const s = keep ? v.s : v.s < 0.5 ? Math.max(0.85, saved.current ?? 0) : v.s;
        const px = r.x + r.width / 2;
        const py = r.y + r.height / 2;
        moveTo({ s, x: cx - px * s, y: cy - py * s });
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
        const m = mouse.current ?? centre();
        const [cx, cy] = centre();
        const px = (m[0] - v.x) / v.s;
        const py = (m[1] - v.y) / v.s;
        moveTo({ s, x: cx - px * s, y: cy - py * s });
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
    let down: { x: number; y: number; t: EventTarget | null } | null = null;
    const pointerDown = (e: PointerEvent) => {
      if (e.button !== 0 && e.pointerType === 'mouse') return;
      anim.current.id++;
      anim.current.goal = null;
      down = pts.size === 0 ? { x: e.clientX, y: e.clientY, t: e.target } : null;
      el.setPointerCapture(e.pointerId);
      pts.set(e.pointerId, [e.clientX, e.clientY]);
      el.classList.add('dragging');
      if (pts.size === 1) last = [e.clientX, e.clientY];
      if (pts.size === 2) {
        const p = [...pts.values()];
        pinch = Math.hypot(p[0][0] - p[1][0], p[0][1] - p[1][1]);
      }
    };
    const pointerMove = (e: PointerEvent) => {
      mouse.current = local(e);
      if (!pts.has(e.pointerId)) return;
      pts.set(e.pointerId, [e.clientX, e.clientY]);
      if (down && (pts.size > 1 || Math.hypot(e.clientX - down.x, e.clientY - down.y) > 6)) down = null;
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
      // What was pressed, not what the captured pointer reports.
      if (e.type === 'pointerup' && pts.size === 0 && down) {
        const under = document.elementFromPoint(e.clientX, e.clientY);
        if (under) pick.current(under);
      }
      down = null;
      if (pts.size < 2) pinch = null;
      if (pts.size === 1) last = [...pts.values()][0];
      if (pts.size === 0) {
        last = null;
        el.classList.remove('dragging');
      }
    };
    let hovered = false;
    const enter = () => {
      hovered = true;
    };
    const leave = () => {
      hovered = false;
    };
    const key = (e: KeyboardEvent) => {
      if (e.code !== 'Space' || !hovered) return;
      const t = e.target as HTMLElement | null;
      if (t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName))) return;
      e.preventDefault();
      // A button keeps focus after a click, and Space would press it again.
      if (t && t.tagName === 'BUTTON') t.blur();
      if (!e.repeat) a.toggle();
    };
    // Resizing keeps the zoom, and the point in the middle in the middle.
    let was = { w: el.clientWidth, h: el.clientHeight };
    const resized = new ResizeObserver(() => {
      const now = { w: el.clientWidth, h: el.clientHeight };
      view.current = {
        ...view.current,
        x: view.current.x + (now.w - was.w) / 2,
        y: view.current.y + (now.h - was.h) / 2,
      };
      was = now;
      a.apply();
    });
    resized.observe(el);
    el.addEventListener('wheel', wheel, { passive: false });
    el.addEventListener('pointerdown', pointerDown);
    el.addEventListener('pointermove', pointerMove);
    el.addEventListener('pointerup', up);
    el.addEventListener('pointercancel', up);
    el.addEventListener('pointerenter', enter);
    el.addEventListener('pointerleave', leave);
    document.addEventListener('keydown', key);
    return () => {
      resized.disconnect();
      el.removeEventListener('wheel', wheel);
      el.removeEventListener('pointerdown', pointerDown);
      el.removeEventListener('pointermove', pointerMove);
      el.removeEventListener('pointerup', up);
      el.removeEventListener('pointercancel', up);
      el.removeEventListener('pointerenter', enter);
      el.removeEventListener('pointerleave', leave);
      document.removeEventListener('keydown', key);
    };
  }, []);

  return (
    <div className="viewer" ref={box}>
      <div className="viewer-stage" ref={stage}>
        {props.children}
      </div>
    </div>
  );
}
