// Places a graph's boxes with elkjs's layered layout: the flow's cells, or the items and their dependencies.

import ELK from 'elkjs/lib/elk.bundled.js';
import type { ElkNode } from 'elkjs/lib/elk-api';

const elk = new ELK();

export type Box = { id: string; width: number; height: number };
export type Link = { id: string; source: string; target: string };
export type Place = { x: number; y: number };

/// Where each box goes, laid out in layers along `direction`, keeping the order the boxes are given in where it can.
export async function layout(boxes: Box[], links: Link[], direction: 'DOWN' | 'RIGHT' = 'DOWN'): Promise<Map<string, Place>> {
  const ids = new Set(boxes.map((b) => b.id));
  const graph: ElkNode = {
    id: 'root',
    layoutOptions: {
      'elk.algorithm': 'layered',
      'elk.direction': direction,
      'elk.spacing.nodeNode': '28',
      'elk.layered.spacing.nodeNodeBetweenLayers': '56',
      'elk.layered.nodePlacement.strategy': 'NETWORK_SIMPLEX',
      'elk.layered.considerModelOrder.strategy': 'NODES_AND_EDGES',
    },
    children: boxes.map((b) => ({ id: b.id, width: b.width, height: b.height })),
    edges: links
      .filter((l) => ids.has(l.source) && ids.has(l.target))
      .map((l) => ({ id: l.id, sources: [l.source], targets: [l.target] })),
  };
  const out = await elk.layout(graph);
  const places = new Map<string, Place>();
  for (const c of out.children ?? []) places.set(c.id, { x: c.x ?? 0, y: c.y ?? 0 });
  return places;
}
