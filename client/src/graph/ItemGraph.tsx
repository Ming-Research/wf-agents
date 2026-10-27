// The items' dependency graph: an arrow goes from an item to the item that depends on it, so what is needed comes
// first. elkjs places the items and routes the arrows, React Flow draws them. It moves and zooms as every chart does:
// two fingers on a trackpad, or the wheel, move it; Shift with two fingers, or a pinch, zooms at the pointer;
// dragging moves it; Space shows the whole graph and remembers the zoom, and Space again goes back to that zoom with
// the point under the pointer in the middle.

import {
  Background,
  Controls,
  Handle,
  MarkerType,
  Position,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
  type Node,
  type NodeProps,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { useEffect, useMemo, useRef, useState } from 'react';

import type { Items, ItemSummary } from '../api';
import { ItemBadge } from '../components';
import { bounds, layout, type Laid } from './layout';
import { RoutedEdge, type Routed } from './RoutedEdge';

const WIDTH = 220;
const HEIGHT = 64;

type ItemData = { item: ItemSummary; selected: boolean };
type ItemFlowNode = Node<ItemData, 'item'>;

function ItemNode({ data }: NodeProps<ItemFlowNode>) {
  const { item, selected } = data;
  return (
    <div
      className={`item-node item-node-${item.state}${selected ? ' cell-selected' : ''}`}
      style={{ width: WIDTH, height: HEIGHT }}
      title={item.title}
    >
      <Handle type="target" position={Position.Top} />
      <div className="cell-head">
        <span className="cell-id">#{item.id}</span>
        <span className="cell-kind">{item.project}</span>
        <ItemBadge state={item.state} />
      </div>
      <div className="cell-title">{item.title}</div>
      <Handle type="source" position={Position.Bottom} />
    </div>
  );
}

const nodeTypes = { item: ItemNode };
const edgeTypes = { routed: RoutedEdge };

type Props = { items: Items; selected?: number | null; onSelect: (item: number) => void };

const linkId = (needed: number, needing: number) => `${needed}->${needing}`;

function Graph({ items, selected, onSelect }: Props) {
  const shape = useMemo(
    () =>
      items.items.map((i) => i.id).join(' ') +
      '|' +
      items.links.map((l) => linkId(l.depends_on, l.item)).join(' '),
    [items],
  );
  const [laid, setLaid] = useState<{ shape: string; laid: Laid } | null>(null);
  useEffect(() => {
    let live = true;
    layout(
      items.items.map((i) => ({ id: String(i.id), width: WIDTH, height: HEIGHT })),
      items.links.map((l) => ({
        id: linkId(l.depends_on, l.item),
        source: String(l.depends_on),
        target: String(l.item),
      })),
      'DOWN',
      // Items that depend on nothing are many: they are packed in rows across the wide panel.
      8,
    ).then(
      (l) => {
        if (live) setLaid({ shape, laid: l });
      },
      () => {
        if (live) setLaid({ shape, laid: { places: new Map(), routes: new Map() } });
      },
    );
    return () => {
      live = false;
    };
    // The items change with every work's step; the layout only when the shape does.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [shape]);
  const current = laid !== null && laid.shape === shape ? laid.laid : null;
  // Once laid out, the whole graph is shown, no larger than it is.
  const rf = useReactFlow();
  const ready = rf.viewportInitialized;
  useEffect(() => {
    if (current === null || !ready) return;
    const b = bounds([...current.places.values()], WIDTH, HEIGHT);
    if (b !== null) void rf.fitBounds(b, { padding: 0.1, duration: 200 }).then(() => {
      if (rf.getZoom() > 1) void rf.zoomTo(1);
    });
  }, [current, ready, rf]);
  const nodes: ItemFlowNode[] = useMemo(
    () =>
      items.items.map((item, i) => ({
        id: String(item.id),
        type: 'item' as const,
        position: current?.places.get(String(item.id)) ?? {
          x: (i % 4) * (WIDTH + 30),
          y: Math.floor(i / 4) * (HEIGHT + 40),
        },
        width: WIDTH,
        height: HEIGHT,
        data: { item, selected: item.id === selected },
        draggable: false,
        connectable: false,
      })),
    [items, current, selected],
  );
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    let saved: number | null = null;
    let hovered = false;
    let pointer = { x: 0, y: 0 };
    const clamp = (d: number) => Math.max(-50, Math.min(50, d));
    const zoomAt = (f: number, cx: number, cy: number) => {
      const v = rf.getViewport();
      const z = Math.max(0.1, Math.min(4, v.zoom * f));
      void rf.setViewport({ zoom: z, x: cx - (cx - v.x) * (z / v.zoom), y: cy - (cy - v.y) * (z / v.zoom) });
    };
    // A trackpad's pinch comes as the wheel with Ctrl. Shift turns a vertical scroll into a horizontal one in some
    // browsers, so either way zooms.
    const wheel = (e: WheelEvent) => {
      e.preventDefault();
      const k = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? el.clientHeight : 1;
      const dx = e.deltaX * k;
      const dy = e.deltaY * k;
      const r = el.getBoundingClientRect();
      const [cx, cy] = [e.clientX - r.left, e.clientY - r.top];
      if (e.ctrlKey || e.metaKey) {
        zoomAt(Math.exp(-clamp(dy) * 0.01), cx, cy);
      } else if (e.shiftKey) {
        zoomAt(Math.exp(-clamp(Math.abs(dy) >= Math.abs(dx) ? dy : dx) * 0.004), cx, cy);
      } else {
        const v = rf.getViewport();
        void rf.setViewport({ ...v, x: v.x - dx, y: v.y - dy });
      }
    };
    const key = (e: KeyboardEvent) => {
      if (e.code !== 'Space' || !hovered) return;
      const target = e.target as HTMLElement | null;
      if (target && (target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName))) return;
      e.preventDefault();
      // A button keeps focus after a click, and Space would press it again.
      if (target && target.tagName === 'BUTTON') target.blur();
      if (e.repeat) return;
      if (saved === null) {
        saved = rf.getViewport().zoom;
        void rf.fitView({ padding: 0.1, duration: 200 });
        return;
      }
      const z = saved;
      saved = null;
      const p = rf.screenToFlowPosition(pointer);
      const r = el.getBoundingClientRect();
      void rf.setViewport({ zoom: z, x: r.width / 2 - p.x * z, y: r.height / 2 - p.y * z }, { duration: 200 });
    };
    const move = (e: PointerEvent) => {
      pointer = { x: e.clientX, y: e.clientY };
    };
    const enter = () => {
      hovered = true;
    };
    const leave = () => {
      hovered = false;
    };
    el.addEventListener('wheel', wheel, { passive: false });
    el.addEventListener('pointermove', move);
    el.addEventListener('pointerenter', enter);
    el.addEventListener('pointerleave', leave);
    document.addEventListener('keydown', key);
    return () => {
      el.removeEventListener('wheel', wheel);
      el.removeEventListener('pointermove', move);
      el.removeEventListener('pointerenter', enter);
      el.removeEventListener('pointerleave', leave);
      document.removeEventListener('keydown', key);
    };
  }, [rf]);
  const edges: Routed[] = useMemo(
    () =>
      items.links.map((l) => {
        const id = linkId(l.depends_on, l.item);
        return {
          id,
          source: String(l.depends_on),
          target: String(l.item),
          type: 'routed' as const,
          data: { points: current?.routes.get(id) ?? [] },
          label: l.needs,
          className: 'flow-edge edge-main',
          markerEnd: { type: MarkerType.ArrowClosed, width: 16, height: 16 },
          focusable: false,
        };
      }),
    [items, current],
  );
  return (
    <div className="chart" ref={box}>
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        onNodeClick={(_, node) => onSelect(Number(node.id))}
        nodesDraggable={false}
        nodesConnectable={false}
        minZoom={0.1}
        maxZoom={4}
        zoomOnScroll={false}
        zoomOnPinch={false}
        panOnScroll={false}
      >
        <Background />
        <Controls showInteractive={false} />
      </ReactFlow>
    </div>
  );
}

export function ItemGraph(props: Props) {
  return (
    <ReactFlowProvider>
      <Graph {...props} />
    </ReactFlowProvider>
  );
}
