// A flow's state machine: elkjs places the cells and routes the rows between them, React Flow draws them. On a
// work's page each cell carries what the work did there: the current cell, its round count, its rounds sent back or
// failed, and the ways the work went.

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

import type { CompiledCell, CompiledGraph, RoundSummary } from '../api';
import type { Key } from '../i18n';
import { useT } from '../text';
import { build, type Overlay } from './flowGraph';
import { layout, type Laid } from './layout';
import { RoutedEdge, type Routed } from './RoutedEdge';

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
const edgeTypes = { routed: RoutedEdge };

/// Once the cells are laid out and measured: shows the cell `focus` names, or else the whole graph.
function Fit({ focus, shape }: { focus: string | null; shape: string | null }) {
  const rf = useReactFlow();
  const measured = useNodesInitialized();
  useEffect(() => {
    if (shape === null || !measured) return;
    if (focus === null) void rf.fitView({ padding: 0.06, duration: 200, minZoom: 0.3 });
    else void rf.fitView({ nodes: [{ id: focus }], duration: 300, minZoom: 0.9, maxZoom: 0.9 });
  }, [focus, shape, measured, rf]);
  return null;
}

export type FlowChartProps = {
  graph: CompiledGraph;
  /// The work's rounds and the cell it is at, on a work's page.
  rounds?: RoundSummary[];
  at?: string | null;
  selected: string | null;
  /// The cell to show; with none, the whole graph is shown.
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
  const [laid, setLaid] = useState<{ shape: string; laid: Laid } | null>(null);
  useEffect(() => {
    let live = true;
    layout(
      built.nodes.map((n) => ({ id: n.id, width: WIDTH, height: HEIGHT })),
      built.edges.map((e) => ({ id: e.id, source: e.source, target: e.target })),
    ).then(
      (l) => {
        if (live) setLaid({ shape, laid: l });
      },
      () => {
        // A layout that fails leaves the cells in a column, joined by straight lines.
        if (live) setLaid({ shape, laid: { places: new Map(), routes: new Map() } });
      },
    );
    return () => {
      live = false;
    };
    // `built` changes with every round; the layout only when the shape does.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [shape]);
  // A layout of an earlier shape is not used: its routes do not fit.
  const current = laid !== null && laid.shape === shape ? laid.laid : null;
  const nodes: CellFlowNode[] = useMemo(
    () =>
      built.nodes.map((n, i) => ({
        id: n.id,
        type: 'cell' as const,
        position: current?.places.get(n.id) ?? { x: 0, y: i * (HEIGHT + 30) },
        width: WIDTH,
        height: HEIGHT,
        data: { cell: n.cell, overlay: n.overlay, selected: n.id === selected },
        draggable: false,
        connectable: false,
      })),
    [built, current, selected],
  );
  const edges: Routed[] = useMemo(
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
          type: 'routed' as const,
          data: { points: current?.routes.get(e.id) ?? [] },
          className: classes.join(' '),
          label,
          markerEnd: { type: MarkerType.ArrowClosed, width: 16, height: 16 },
          focusable: false,
        };
      }),
    [built, current, selected],
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
        edgeTypes={edgeTypes}
        onNodeClick={(_, node) => onSelect(node.id)}
        nodesDraggable={false}
        nodesConnectable={false}
        minZoom={0.1}
      >
        <Background />
        <Controls showInteractive={false} />
        <Fit focus={focus ?? null} shape={current !== null ? shape : null} />
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
