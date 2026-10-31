// What the flow's panels share: the flow shown, what the interface says of it, and how a panel opens another.

import { createContext, useContext } from 'react';

import type { CellView, Compiled, CompiledCell, CompiledGraph, GraphView } from '../api';

/// What the panel shows. A cell, a rule, the interjection and the actions on a line are of a graph; a chart that shows
/// several graphs names it, and a panel that opens one of them opens it in its own graph. A store is every graph's.
export type Subject =
  | { kind: 'cell'; cell: string; graph?: string; rows?: number[] }
  | { kind: 'rule'; rule: string; graph?: string }
  | { kind: 'interjection'; graph?: string }
  | { kind: 'action'; node: string; graph?: string }
  | { kind: 'store'; store: string }
  | { kind: 'changes' }
  | { kind: 'round'; work: number; round: number };

export type Flow = {
  compiled: Compiled;
  g: CompiledGraph;
  view: GraphView;
  cell: (id: string) => CompiledCell | undefined;
  cellView: (id: string) => CellView | undefined;
  /// Opens a subject in the panel; the panel offers to go back to what it showed before.
  open: (s: Subject) => void;
};

export const FlowContext = createContext<Flow | null>(null);

export function useFlow(): Flow {
  const f = useContext(FlowContext);
  if (f === null) throw new Error('a flow panel outside a flow');
  return f;
}

/// A subject in a graph: a cell, a rule, the interjection or the actions on a line that names no graph is of `graph`.
export function inGraph(s: Subject, graph: string): Subject {
  if (s.kind === 'cell' || s.kind === 'rule' || s.kind === 'interjection' || s.kind === 'action') {
    return s.graph !== undefined ? s : { ...s, graph };
  }
  return s;
}

/// What tells two subjects apart, whatever order their fields were written in.
export function subjectKey(s: Subject): string {
  switch (s.kind) {
    case 'cell':
      return `cell:${s.graph ?? ''}:${s.cell}:${(s.rows ?? []).join(',')}`;
    case 'rule':
      return `rule:${s.graph ?? ''}:${s.rule}`;
    case 'interjection':
      return `interjection:${s.graph ?? ''}`;
    case 'action':
      return `action:${s.graph ?? ''}:${s.node}`;
    case 'store':
      return `store:${s.store}`;
    case 'changes':
      return 'changes';
    case 'round':
      return `round:${s.work}-${s.round}`;
  }
}

/// The flow's lookups, for a compiled flow, one of its graphs and what the interface says of it; what a panel opens
/// from it is of that graph.
export function flowOf(compiled: Compiled, g: CompiledGraph, view: GraphView, open: (s: Subject) => void): Flow {
  const cells = new Map(g.cells.map((c) => [c.id, c]));
  const views = new Map(view.cells.map((c) => [c.id, c]));
  return {
    compiled,
    g,
    view,
    cell: (id) => cells.get(id),
    cellView: (id) => views.get(id),
    open: (s) => open(inGraph(s, g.name)),
  };
}
