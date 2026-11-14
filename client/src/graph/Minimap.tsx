// The map over the chart's top left corner: the whole chart drawn small, its cells and the ways into them as plain
// shapes with no arrows, each in the colours the chart draws it in, and the part of the chart in view as a frame. A
// click on the map brings the point clicked to the middle at the zoom the chart has; dragging the frame moves the
// chart. The map is drawn from the layout the chart already has, and its frame follows the view as it changes, without
// drawing the chart again. It hides while the whole chart is in view, and while what can be seen is too small for it
// to keep clear of the middle. Under it, or in its place while it hides, the chart's zoom as a percentage, which
// follows the view as the frame does.
//
// Brought over from the chart of `old-main` (`client/src/graph/Minimap.tsx`), but for the stage boxes and the stores,
// which this flow has none of.

import { memo, type RefObject, useCallback, useEffect, useLayoutEffect, useRef } from 'react';

import type { Box, Laid, Size } from './layout';
import type { Chart } from './model';
import type { Seen, View, ViewerHandle } from './Viewer';

/// The most room the map's drawing takes, in the page's pixels.
export const MAP_WIDTH = 200;
export const MAP_HEIGHT = 160;
/// The least a cell is drawn at either way, so that it stays a mark at the smallest scale.
export const LEAST = 2;
/// The room round the drawing, inside the map's border.
const INSET = 4;
/// How far a press may move and still be a click.
const CLICK = 4;
/// What the map keeps clear of in the middle of what can be seen: a cell at full size.
const MIDDLE = { width: 300, height: 120 };
/// The room between the map and the zoom under it.
const BELOW = 8;

/// The chart's zoom: a whole percentage, 100% at full size.
export function zoomText(s: number): string {
  return `${Math.round(s * 100)}%`;
}

/// The map's scale, the size of its drawing and the size of the chart it maps.
export type MapGeom = { k: number; width: number; height: number; chart: Size };

/// The map of a chart of `chart`'s size: as wide as the map may be, unless the chart's proportions would make it
/// taller than it may be.
export function mapGeom(chart: Size): MapGeom {
  const k = Math.min(MAP_WIDTH / Math.max(chart.width, 1), MAP_HEIGHT / Math.max(chart.height, 1));
  return { k, width: chart.width * k, height: chart.height * k, chart };
}

/// A shape on the map, in the map's pixels: a cell, an engine's cell, or the way into a cell.
export type MapShape = Box & { kind: 'agent' | 'engine' | 'way-in'; cell: string };

export type MapOf = { geom: MapGeom; shapes: MapShape[] };

/// The map of a chart of `size` whose layout starts `pad` in from its edges.
export function mapOf(size: Size, chart: Chart, laid: Laid, pad: number, engines: Set<string>): MapOf {
  const geom = mapGeom(size);
  const { k } = geom;
  const scaled = (b: Box): Box => {
    const width = Math.max(LEAST, b.width * k);
    const height = Math.max(LEAST, b.height * k);
    return {
      x: (b.x + pad + b.width / 2) * k - width / 2,
      y: (b.y + pad + b.height / 2) * k - height / 2,
      width,
      height,
    };
  };
  const shapes: MapShape[] = [];
  for (const n of chart.nodes) {
    const b = laid.nodes.get(n.id);
    if (b === undefined || n.kind === 'outside') continue;
    const kind = n.kind === 'way-in' || n.kind === 'fork' ? 'way-in' : engines.has(n.cell) ? 'engine' : 'agent';
    shapes.push({ ...scaled(b), kind, cell: n.cell });
  }
  return { geom, shapes };
}

/// The part of the chart in view, as a frame in the map's pixels, cut to the chart; none when none of it is in view.
export function viewFrame(m: MapGeom, v: View, seen: Seen): Box | null {
  const x0 = Math.max(0, -v.x / v.s);
  const y0 = Math.max(0, -v.y / v.s);
  const x1 = Math.min(m.chart.width, (seen.width - v.x) / v.s);
  const y1 = Math.min(m.chart.height, (seen.height - v.y) / v.s);
  if (x1 <= x0 || y1 <= y0) return null;
  return { x: x0 * m.k, y: y0 * m.k, width: (x1 - x0) * m.k, height: (y1 - y0) * m.k };
}

/// The view that brings the point of the chart under the map's point `x`, `y` to the middle of what is seen, at the
/// zoom the view has.
export function centredAt(m: MapGeom, x: number, y: number, v: View, seen: Seen): View {
  return { s: v.s, x: seen.width / 2 - (x / m.k) * v.s, y: seen.height / 2 - (y / m.k) * v.s };
}

/// The view `v` moved as far as the frame is dragged on the map, `dx`, `dy` in the map's pixels.
export function draggedBy(m: MapGeom, dx: number, dy: number, v: View): View {
  return { s: v.s, x: v.x - (dx / m.k) * v.s, y: v.y - (dy / m.k) * v.s };
}

/// Whether the whole chart is in view.
export function wholeInView(m: MapGeom, v: View, seen: Seen): boolean {
  const e = 1;
  const right = v.x + m.chart.width * v.s;
  const bottom = v.y + m.chart.height * v.s;
  return v.x >= -e && v.y >= -e && right <= seen.width + e && bottom <= seen.height + e;
}

/// Whether the map shows, its right and bottom edges where they are in the viewer: while some of the chart is out of
/// view, and the map keeps clear of the middle of what is seen, to its left or above it.
export function mapShown(m: MapGeom, v: View, seen: Seen, right: number, bottom: number): boolean {
  if (seen.width <= 0 || seen.height <= 0 || wholeInView(m, v, seen)) return false;
  return right <= (seen.width - MIDDLE.width) / 2 || bottom <= (seen.height - MIDDLE.height) / 2;
}

/// A press on the map: the pointer's, where it went down on the page and on the map, the view then, whether it was on
/// the frame, and whether it has moved further than a click.
type Press = { id: number; x: number; y: number; at: [number, number]; from: View; onFrame: boolean; moved: boolean };

/// The map over the chart's top left corner, moving the view of `viewer`.
export const Minimap = memo(function Minimap({ map, viewer }: { map: MapOf; viewer: RefObject<ViewerHandle | null> }) {
  const root = useRef<HTMLDivElement>(null);
  const drawing = useRef<SVGSVGElement>(null);
  const frame = useRef<SVGRectElement>(null);
  const zoom = useRef<HTMLDivElement>(null);
  const geom = useRef(map.geom);
  useLayoutEffect(() => {
    geom.current = map.geom;
  }, [map.geom]);
  // The view the frame was last drawn for, the frame, and the map's own top, right and bottom edges in the viewer, read
  // again only when what can be seen or the map changes size.
  const last = useRef({
    v: null as View | null,
    seen: { width: 0, height: 0 } as Seen,
    frame: null as Box | null,
    edges: { key: '', top: 0, right: 0, bottom: 0 },
  });
  const draw = useCallback((v: View, seen: Seen) => {
    const el = root.current;
    const f = frame.current;
    if (el === null || f === null) return;
    const m = geom.current;
    const l = last.current;
    l.v = v;
    l.seen = seen;
    const key = `${seen.width} ${seen.height} ${m.width} ${m.height}`;
    if (l.edges.key !== key) {
      l.edges = { key, top: el.offsetTop, right: el.offsetLeft + el.offsetWidth, bottom: el.offsetTop + el.offsetHeight };
    }
    const shown = mapShown(m, v, seen, l.edges.right, l.edges.bottom);
    el.toggleAttribute('data-on', shown);
    // The zoom under the map, or in its place while it hides.
    const z = zoom.current;
    if (z !== null) {
      z.textContent = zoomText(v.s);
      z.style.top = `${shown ? l.edges.bottom + BELOW : l.edges.top}px`;
    }
    const b = viewFrame(m, v, seen);
    l.frame = b;
    if (b === null) {
      f.setAttribute('visibility', 'hidden');
      return;
    }
    f.removeAttribute('visibility');
    f.setAttribute('x', String(b.x));
    f.setAttribute('y', String(b.y));
    f.setAttribute('width', String(b.width));
    f.setAttribute('height', String(b.height));
  }, []);
  useEffect(() => viewer.current?.listen(draw), [viewer, draw]);
  // A new layout draws the frame again for the view as it is.
  useLayoutEffect(() => {
    const l = last.current;
    if (l.v !== null) draw(l.v, l.seen);
  }, [map, draw]);
  // A press on the zoom is nobody's: it neither moves the chart nor goes to what is under it.
  useEffect(() => {
    const z = zoom.current;
    if (z === null) return;
    const down = (e: PointerEvent) => e.stopPropagation();
    z.addEventListener('pointerdown', down);
    return () => z.removeEventListener('pointerdown', down);
  }, []);
  // A press here is the map's, not the chart's: a click brings the point clicked to the middle, and dragging the frame
  // moves the chart; the wheel and a pinch go on to the chart as anywhere over it.
  useEffect(() => {
    const el = root.current;
    if (el === null) return;
    let press: Press | null = null;
    const down = (e: PointerEvent) => {
      e.stopPropagation();
      const l = last.current;
      if (e.button !== 0 && e.pointerType === 'mouse') return;
      if (press !== null || l.v === null || drawing.current === null) return;
      const r = drawing.current.getBoundingClientRect();
      const at: [number, number] = [e.clientX - r.left - INSET, e.clientY - r.top - INSET];
      const b = l.frame;
      const onFrame = b !== null && at[0] >= b.x && at[0] <= b.x + b.width && at[1] >= b.y && at[1] <= b.y + b.height;
      press = { id: e.pointerId, x: e.clientX, y: e.clientY, at, from: l.v, onFrame, moved: false };
      el.setPointerCapture?.(e.pointerId);
    };
    const move = (e: PointerEvent) => {
      if (press === null || e.pointerId !== press.id) return;
      const dx = e.clientX - press.x;
      const dy = e.clientY - press.y;
      if (!press.moved && Math.hypot(dx, dy) <= CLICK) return;
      press.moved = true;
      if (!press.onFrame) return;
      el.toggleAttribute('data-dragging', true);
      viewer.current?.to(draggedBy(geom.current, dx, dy, press.from), false);
    };
    const up = (e: PointerEvent) => {
      if (press === null || e.pointerId !== press.id) return;
      const p = press;
      press = null;
      el.toggleAttribute('data-dragging', false);
      const l = last.current;
      if (e.type === 'pointerup' && !p.moved && l.v !== null) {
        viewer.current?.to(centredAt(geom.current, p.at[0], p.at[1], l.v, l.seen), true);
      }
    };
    el.addEventListener('pointerdown', down);
    el.addEventListener('pointermove', move);
    el.addEventListener('pointerup', up);
    el.addEventListener('pointercancel', up);
    return () => {
      el.removeEventListener('pointerdown', down);
      el.removeEventListener('pointermove', move);
      el.removeEventListener('pointerup', up);
      el.removeEventListener('pointercancel', up);
    };
  }, [viewer]);
  const { geom: m, shapes } = map;
  return (
    <>
      <div className="graph-map" ref={root} aria-hidden="true">
        <svg ref={drawing} width={m.width + 2 * INSET} height={m.height + 2 * INSET}>
          <g transform={`translate(${INSET} ${INSET})`}>
            {shapes.map((s, i) => (
              <rect
                key={i}
                className={`graph-map-shape graph-map-${s.kind}`}
                x={s.x}
                y={s.y}
                width={s.width}
                height={s.height}
              />
            ))}
            <rect ref={frame} className="graph-map-frame" />
          </g>
        </svg>
      </div>
      <div className="graph-zoom" ref={zoom} />
    </>
  );
});
