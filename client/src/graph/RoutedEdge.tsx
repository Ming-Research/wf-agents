// An edge drawn along the route elkjs laid out for it, so that edges go around the boxes instead of through them;
// before the layout has come, a straight line.

import { BaseEdge, type Edge, type EdgeProps } from '@xyflow/react';

import type { Place } from './layout';

export type RoutedData = { points: Place[] };
export type Routed = Edge<RoutedData, 'routed'>;

export function RoutedEdge({ id, data, sourceX, sourceY, targetX, targetY, markerEnd, style, label }: EdgeProps<Routed>) {
  const points =
    data && data.points.length >= 2
      ? data.points
      : [
          { x: sourceX, y: sourceY },
          { x: targetX, y: targetY },
        ];
  const path = `M ${points.map((p) => `${p.x},${p.y}`).join(' L ')}`;
  // The label sits on the segment in the middle of the route.
  const i = Math.max(0, Math.floor((points.length - 1) / 2));
  const a = points[i];
  const b = points[Math.min(points.length - 1, i + 1)];
  return (
    <BaseEdge
      id={id}
      path={path}
      markerEnd={markerEnd}
      style={style}
      label={label}
      labelX={(a.x + b.x) / 2}
      labelY={(a.y + b.y) / 2}
      labelBgPadding={[4, 2]}
      labelBgBorderRadius={3}
    />
  );
}
