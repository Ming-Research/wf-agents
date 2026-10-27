// The items' dependency graph: an arrow goes from an item to the item that depends on it, so what is needed comes
// first. elkjs places the items and routes the arrows, React Flow draws them.

import {
  Background,
  Controls,
  Handle,
  MarkerType,
  Position,
  ReactFlow,
  ReactFlowProvider,
  useNodesInitialized,
  useReactFlow,
  type Node,
  type NodeProps,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { useEffect, useMemo, useState } from 'react';

import type { Items, ItemSummary } from '../api';
import { ItemBadge } from '../components';
import { layout, type Laid } from './layout';
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

function Fit({ shape }: { shape: string | null }) {
  const rf = useReactFlow();
  const measured = useNodesInitialized();
  useEffect(() => {
    if (shape === null || !measured) return;
    void rf.fitView({ padding: 0.1, duration: 200, maxZoom: 1 });
  }, [shape, measured, rf]);
  return null;
}

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
    <div className="chart">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        onNodeClick={(_, node) => onSelect(Number(node.id))}
        nodesDraggable={false}
        nodesConnectable={false}
        minZoom={0.1}
      >
        <Background />
        <Controls showInteractive={false} />
        <Fit shape={current !== null ? shape : null} />
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
