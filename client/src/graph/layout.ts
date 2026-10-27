// Places a graph's boxes with elkjs's layered layout, and routes its edges around them: the flow's cells, or the
// items and their dependencies.

import ELK from 'elkjs/lib/elk.bundled.js';
import type { ElkNode } from 'elkjs/lib/elk-api';

const elk = new ELK();

/// A box to place; `layer` puts it in the first or the last layer, such as a graph's start and end.
export type Box = { id: string; width: number; height: number; layer?: 'FIRST' | 'LAST' };
export type Link = { id: string; source: string; target: string };
export type Place = { x: number; y: number };
/// Where each box goes, and the points each edge's route passes through.
export type Laid = { places: Map<string, Place>; routes: Map<string, Place[]> };

/// The rectangle that holds boxes of one size placed at `places`; none for no box.
export function bounds(
  places: Place[],
  width: number,
  height: number,
): { x: number; y: number; width: number; height: number } | null {
  if (places.length === 0) return null;
  const xs = places.map((p) => p.x);
  const ys = places.map((p) => p.y);
  const x = Math.min(...xs);
  const y = Math.min(...ys);
  return { x, y, width: Math.max(...xs) + width - x, height: Math.max(...ys) + height - y };
}

/// Lays the boxes out in layers along `direction`, keeping the order they are given in where it can. `aspect` is the
/// width to height the parts of a graph that are not joined to each other are packed to.
export async function layout(boxes: Box[], links: Link[], direction: 'DOWN' | 'RIGHT' = 'DOWN', aspect = 1.6): Promise<Laid> {
  const ids = new Set(boxes.map((b) => b.id));
  const graph: ElkNode = {
    id: 'root',
    layoutOptions: {
      'elk.algorithm': 'layered',
      'elk.direction': direction,
      'elk.aspectRatio': String(aspect),
      'elk.edgeRouting': 'ORTHOGONAL',
      'elk.spacing.nodeNode': '36',
      'elk.spacing.edgeNode': '18',
      'elk.spacing.edgeEdge': '10',
      'elk.layered.spacing.nodeNodeBetweenLayers': '64',
      'elk.layered.spacing.edgeNodeBetweenLayers': '18',
      'elk.layered.nodePlacement.strategy': 'NETWORK_SIMPLEX',
      'elk.layered.considerModelOrder.strategy': 'NODES_AND_EDGES',
      // A loop is broken where an edge goes back to a box given earlier, so the boxes keep the order they are
      // given in from top to bottom: for a flow, the order its cells are written in.
      'elk.layered.cycleBreaking.strategy': 'MODEL_ORDER',
    },
    children: boxes.map((b) => ({
      id: b.id,
      width: b.width,
      height: b.height,
      layoutOptions: b.layer ? { 'elk.layered.layering.layerConstraint': b.layer } : undefined,
    })),
    edges: links
      .filter((l) => ids.has(l.source) && ids.has(l.target))
      .map((l) => ({ id: l.id, sources: [l.source], targets: [l.target] })),
  };
  const out = await elk.layout(graph);
  const places = new Map<string, Place>();
  for (const c of out.children ?? []) places.set(c.id, { x: c.x ?? 0, y: c.y ?? 0 });
  const routes = new Map<string, Place[]>();
  for (const e of out.edges ?? []) {
    const s = e.sections?.[0];
    if (s) routes.set(e.id, [s.startPoint, ...(s.bendPoints ?? []), s.endPoint]);
  }
  return { places, routes };
}
