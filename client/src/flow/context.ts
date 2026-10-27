// What the flow's panels share: the flow shown, what the interface says of it, and how a panel opens another.

import { createContext, useContext } from 'react';

import type { CellView, Compiled, CompiledCell, CompiledGraph, GraphView } from '../api';

/// What the floating panel shows.
export type Subject =
  | { kind: 'cell'; cell: string; rows?: number[] }
  | { kind: 'rule'; rule: string }
  | { kind: 'interjection' }
  | { kind: 'changes' }
  | { kind: 'round'; round: number };

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

/// The flow's lookups, for a compiled flow, one of its graphs and what the interface says of it.
export function flowOf(compiled: Compiled, g: CompiledGraph, view: GraphView, open: (s: Subject) => void): Flow {
  const cells = new Map(g.cells.map((c) => [c.id, c]));
  const views = new Map(view.cells.map((c) => [c.id, c]));
  return { compiled, g, view, cell: (id) => cells.get(id), cellView: (id) => views.get(id), open };
}
