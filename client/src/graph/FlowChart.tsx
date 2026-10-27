// A flow's state machine: elkjs places the cells, React Flow draws them. On a work's page each cell carries what the
// work did there: the current cell, its round count, its rounds sent back or failed, and the ways the work went.

import {
  Background,
  Controls,
  Handle,
  MarkerType,
  MiniMap,
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

import type { CompiledCell, CompiledGraph, RoundSummary } from '../api';
import type { Key } from '../i18n';
import { useT } from '../text';
import { build, type Overlay } from './flowGraph';
import { layout, type Place } from './layout';

const WIDTH = 230;
const HEIGHT = 72;

const KINDS: Record<string, Key> = {
  agent: 'kind_agent',
  engine: 'kind_engine',
  tool: 'kind_tool',
  owner: 'kind_owner',
};

type CellData = { cell: CompiledCell; overlay: Overlay | null; selected: boolean };
type CellFlowNode = Node<CellData, 'cell'>;

function CellNode({ data }: NodeProps<CellFlowNode>) {
  const t = useT();
  const { cell, overlay, selected } = data;
  const kindKey = KINDS[cell.kind];
  const kind = kindKey ? t(kindKey) : cell.kind;
  const classes = [
    'cell',
    `cell-${cell.kind}`,
    selected ? 'cell-selected' : '',
    overlay?.current ? 'cell-current' : '',
    overlay && overlay.rounds > 0 ? 'cell-visited' : '',
  ].join(' ');
  return (
    <div className={classes} style={{ width: WIDTH, height: HEIGHT }} title={cell.title}>
      <Handle type="target" position={Position.Top} />
      <div className="cell-head">
        <span className="cell-id">{cell.id}</span>
        <span className="cell-kind">{cell.line ?? kind}</span>
        {cell.gate ? <span className="cell-gate">{t('cell_gate')}</span> : null}
        {cell.stage ? <span className="cell-stage">{cell.stage}</span> : null}
      </div>
      <div className="cell-title">{cell.title}</div>
      {overlay && (overlay.rounds > 0 || overlay.current) ? (
        <div className="cell-badges">
          {overlay.rounds > 0 ? <span className="cb cb-rounds">{overlay.rounds}</span> : null}
          {overlay.sentBack > 0 ? <span className="cb cb-back">↩ {overlay.sentBack}</span> : null}
          {overlay.failed > 0 ? <span className="cb cb-failed">✕ {overlay.failed}</span> : null}
          {overlay.running ? <span className="cb cb-running">●</span> : null}
        </div>
      ) : null}
      <Handle type="source" position={Position.Bottom} />
    </div>
  );
}

const nodeTypes = { cell: CellNode };

/// Fits the view to the whole graph once it is laid out, or moves it to the cell `focus` names.
function Fit({ focus, shape }: { focus: string | null; shape: string | null }) {
  const rf = useReactFlow();
  useEffect(() => {
    if (shape === null) return;
    // After React Flow has measured the cells it was just given.
    const timer = setTimeout(() => {
      if (focus === null) void rf.fitView({ padding: 0.08, duration: 200 });
      else void rf.fitView({ nodes: [{ id: focus }], duration: 400, maxZoom: 1.1, padding: 0.6 });
    }, 60);
    return () => clearTimeout(timer);
  }, [focus, shape, rf]);
  return null;
}

export type FlowChartProps = {
  graph: CompiledGraph;
  /// The work's rounds and the cell it is at, on a work's page.
  rounds?: RoundSummary[];
  at?: string | null;
  selected: string | null;
  /// A cell to move the view to.
  focus?: string | null;
  onSelect: (cell: string) => void;
};

function Chart({ graph, rounds, at, selected, focus, onSelect }: FlowChartProps) {
  const t = useT();
  const [common, setCommon] = useState(false);
  const built = useMemo(() => build(graph, { common, rounds, at }), [graph, common, rounds, at]);
  // The layout depends only on which cells and edges there are, not on what the work did at them.
  const shape = useMemo(
    () => built.edges.map((e) => e.id).join(' ') + '|' + built.nodes.map((n) => n.id).join(' '),
    [built],
  );
  const [placed, setPlaced] = useState<{ shape: string; places: Map<string, Place> } | null>(null);
  useEffect(() => {
    let live = true;
    layout(
      built.nodes.map((n) => ({ id: n.id, width: WIDTH, height: HEIGHT })),
      built.edges.map((e) => ({ id: e.id, source: e.source, target: e.target })),
    ).then(
      (places) => {
        if (live) setPlaced({ shape, places });
      },
      () => {
        // A layout that fails leaves the cells in a column.
        if (live) setPlaced({ shape, places: new Map() });
      },
    );
    return () => {
      live = false;
    };
    // `built` changes with every round; the layout only when the shape does.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [shape]);
  const places = placed?.places;
  const nodes: CellFlowNode[] = useMemo(
    () =>
      built.nodes.map((n, i) => ({
        id: n.id,
        type: 'cell' as const,
        position: places?.get(n.id) ?? { x: 0, y: i * (HEIGHT + 30) },
        data: { cell: n.cell, overlay: n.overlay, selected: n.id === selected },
        draggable: false,
        connectable: false,
      })),
    [built, places, selected],
  );
  const edges: Edge[] = useMemo(
    () =>
      built.edges.map((e) => {
        const classes = ['flow-edge'];
        if (e.taken > 0) classes.push('edge-taken');
        if (e.main) classes.push('edge-main');
        if (e.common) classes.push('edge-common');
        if (e.source === selected || e.target === selected) classes.push('edge-near');
        const label =
          e.taken > 0 ? `${e.rows.join(', ')} ×${e.taken}` : e.source === selected ? e.rows.join(', ') : undefined;
        return {
          id: e.id,
          source: e.source,
          target: e.target,
          type: 'smoothstep',
          className: classes.join(' '),
          label,
          markerEnd: { type: MarkerType.ArrowClosed, width: 16, height: 16 },
          animated: e.taken > 0 && e.target === at,
          focusable: false,
        };
      }),
    [built, selected, at],
  );
  return (
    <div className="chart">
      <label className="chart-option">
        <input type="checkbox" checked={common} onChange={(e) => setCommon(e.target.checked)} />
        {t('common_rows')}
      </label>
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        onNodeClick={(_, node) => onSelect(node.id)}
        nodesDraggable={false}
        nodesConnectable={false}
        minZoom={0.1}
        fitView
      >
        <Background />
        <Controls showInteractive={false} />
        <MiniMap pannable zoomable />
        <Fit focus={focus ?? null} shape={placed?.shape ?? null} />
      </ReactFlow>
    </div>
  );
}

export function FlowChart(props: FlowChartProps) {
  return (
    <ReactFlowProvider>
      <Chart {...props} />
    </ReactFlowProvider>
  );
}
