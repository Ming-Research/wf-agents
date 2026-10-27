// The items' dependency graph: an arrow goes from an item to the item that depends on it, so what is needed comes
// first. elkjs places the items, React Flow draws them.

import {
  Background,
  Controls,
  Handle,
  MarkerType,
  Position,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
  type Edge,
  type Node,
  type NodeProps,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { useEffect, useMemo, useState } from 'react';

import type { Items, ItemSummary } from '../api';
import { ItemBadge } from '../components';
import { layout, type Place } from './layout';

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

function Fit({ shape }: { shape: string | null }) {
  const rf = useReactFlow();
  useEffect(() => {
    if (shape === null) return;
    const timer = setTimeout(() => void rf.fitView({ padding: 0.1, duration: 200 }), 60);
    return () => clearTimeout(timer);
  }, [shape, rf]);
  return null;
}

type Props = { items: Items; selected?: number | null; onSelect: (item: number) => void };

function Graph({ items, selected, onSelect }: Props) {
  const shape = useMemo(
    () =>
      items.items.map((i) => i.id).join(' ') +
      '|' +
      items.links.map((l) => `${l.depends_on}->${l.item}`).join(' '),
    [items],
  );
  const [placed, setPlaced] = useState<{ shape: string; places: Map<string, Place> } | null>(null);
  useEffect(() => {
    let live = true;
    layout(
      items.items.map((i) => ({ id: String(i.id), width: WIDTH, height: HEIGHT })),
      items.links.map((l) => ({
        id: `${l.depends_on}->${l.item}`,
        source: String(l.depends_on),
        target: String(l.item),
      })),
    ).then(
      (places) => {
        if (live) setPlaced({ shape, places });
      },
      () => {
        if (live) setPlaced({ shape, places: new Map() });
      },
    );
    return () => {
      live = false;
    };
    // The items change with every work's step; the layout only when the shape does.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [shape]);
  const places = placed?.places;
  const nodes: ItemFlowNode[] = useMemo(
    () =>
      items.items.map((item, i) => ({
        id: String(item.id),
        type: 'item' as const,
        position: places?.get(String(item.id)) ?? { x: (i % 4) * (WIDTH + 30), y: Math.floor(i / 4) * (HEIGHT + 40) },
        data: { item, selected: item.id === selected },
        draggable: false,
        connectable: false,
      })),
    [items, places, selected],
  );
  const edges: Edge[] = useMemo(
    () =>
      items.links.map((l) => ({
        id: `${l.depends_on}->${l.item}`,
        source: String(l.depends_on),
        target: String(l.item),
        type: 'smoothstep',
        label: l.needs,
        className: 'flow-edge edge-main',
        markerEnd: { type: MarkerType.ArrowClosed, width: 16, height: 16 },
        focusable: false,
      })),
    [items],
  );
  return (
    <div className="chart">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        onNodeClick={(_, node) => onSelect(Number(node.id))}
        nodesDraggable={false}
        nodesConnectable={false}
        minZoom={0.1}
        fitView
      >
        <Background />
        <Controls showInteractive={false} />
        <Fit shape={placed?.shape ?? null} />
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
