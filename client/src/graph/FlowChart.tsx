// The flow as a flowchart, filling the space it is given: the chart is built from the flow's definition (`model.ts`),
// laid out by elkjs (`layout.ts`) and drawn in a pan-and-zoom view (`Viewer.tsx`), with a map of the whole chart over
// its top left corner while some of it is out of view (`Minimap.tsx`). The layout is the main path's and the ways
// back's; the arrows a write runs are drawn over it (`data.ts`): those into and out of the cell chosen, or every one
// while the owner shows them all, the layout staying as it is. Choosing a cell, a name an arrow carries, a key of the way into a cell or a tag opens what the same thing
// opens on the matrix, in the same panel; a click on the chart's background lets go of what is chosen. The chart opens
// on its top at a zoom one can read, or whole where that is larger; a cell chosen, here or elsewhere, as in a panel, is
// brought into what the window over the chart leaves, the zoom kept.
//
// Brought over from the chart of `old-main` (`client/src/graph/FlowChart.tsx`), but for what this page has no use for:
// several graphs side by side, the stores, a work's marks and a comparison's.

import { useEffect, useMemo, useRef, useState } from 'react';

import type { Flow } from '../api';
import type { Chosen } from '../Chart';
import { besideCell, routeTrunks, trunksOf, withArrows } from './data';
import { contextBox, labelSize, sizes, Sheet, wordsSize } from './draw';
import { type Laid, layoutChart } from './layout';
import { mapOf, Minimap } from './Minimap';
import { cellNode, chartOf, shownArrows } from './model';
import { Viewer, type ViewerHandle } from './Viewer';

/// The least zoom the chart opens at; and the room round the layout.
const READABLE = 0.62;
const PAD = 24;

export function FlowChart({ flow, dataArrows = false, chosen, onChoose, covered = 0 }: {
  flow: Flow;
  /// Whether every arrow a write runs is drawn; else only those into and out of the cell chosen.
  dataArrows?: boolean;
  chosen: Chosen;
  onChoose: (chosen: Chosen) => void;
  /// How much of the chart's right a window that floats over it covers, in pixels; none while it is away.
  covered?: number;
}) {
  const chart = useMemo(() => chartOf(flow), [flow]);
  const measured = useMemo(() => sizes(chart, flow), [chart, flow]);
  const [laid, setLaid] = useState<{ chart: typeof chart; laid: Laid } | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    const edges = new Map(chart.edges.map((e) => [e.id, e]));
    layoutChart(
      chart,
      (id) => measured.size.get(id) ?? { width: 40, height: 20 },
      (id) => {
        const e = edges.get(id);
        return e && (e.kind === 'arrow' || e.kind === 'step') ? labelSize(e) : null;
      },
    ).then(
      (l) => {
        if (live) setLaid({ chart, laid: l });
      },
      (e: unknown) => {
        if (live) setFailed(e instanceof Error ? e.message : String(e));
      },
    );
    return () => {
      live = false;
    };
  }, [chart, measured]);
  const current = laid !== null && laid.chart === chart ? laid.laid : null;
  const size = useMemo(
    () => (current ? { width: current.bounds.width + 2 * PAD, height: current.bounds.height + 2 * PAD } : null),
    [current],
  );
  const viewer = useRef<ViewerHandle>(null);
  const engines = useMemo(() => new Set(flow.cells.filter((c) => c.kind === 'engine').map((c) => c.id)), [flow]);
  const map = useMemo(
    () => (current && size ? mapOf(size, chart, current, PAD, engines) : null),
    [current, size, chart, engines],
  );
  // Once laid out, open on the top of the chart at a zoom one can read, or on the whole of it where that is larger.
  const opened = useRef<Laid | null>(null);
  useEffect(() => {
    if (current === null || opened.current === current || !viewer.current) return;
    opened.current = current;
    viewer.current.start(READABLE, null);
  }, [current]);
  // The arrows a write runs that are drawn, routed over the layout: the chosen cell's, or every one; the layout stays.
  const chosenCell = chosen && 'cell' in chosen ? chosen.cell : null;
  const shown = useMemo(() => shownArrows(chart, chosenCell, dataArrows), [chart, chosenCell, dataArrows]);
  const over = useMemo(() => {
    if (current === null) return null;
    const main = new Set(chart.edges.filter((e) => e.main).map((e) => e.id));
    const labels = [...current.edges.values()].flatMap((r) => (r.label !== null ? [r.label] : []));
    const drawn = routeTrunks(trunksOf(shown), current.nodes, current.edges, main, labels, wordsSize);
    const edges = withArrows(current.edges, drawn.routes);
    // What the chosen cell is given, beside it where it hides the least of what is drawn.
    const at = chosenCell !== null ? current.nodes.get(cellNode(chosenCell)) : undefined;
    const given = chosenCell !== null && at !== undefined ? contextBox(flow, chosenCell, at) : null;
    const aside =
      given !== null && at !== undefined
        ? besideCell(
            at,
            given,
            [...current.nodes.values(), ...labels, ...drawn.labels.map((l) => l.box)],
            edges,
            new Set([...main, ...drawn.routes.keys()]),
          )
        : null;
    return { laid: { ...current, edges }, labels: drawn.labels, aside };
  }, [current, shown, chart, flow, chosenCell]);
  // A cell chosen is brought into view with what it is given beside it, the zoom kept, the chart moving only as far as
  // brings them wholly into what a window over the chart leaves: again as the window comes out.
  const cover = useRef(covered);
  cover.current = covered;
  const under = covered > 0;
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (chosenCell === null || current === null) return;
    const b = current.nodes.get(cellNode(chosenCell));
    if (!b) return;
    const cell = { x: b.x + PAD, y: b.y + PAD, width: b.width, height: b.height };
    const given = root.current?.querySelector<HTMLElement>('.graph-context');
    const right = given ? Math.max(cell.x + cell.width, given.offsetLeft + given.offsetWidth) : cell.x + cell.width;
    const bottom = given ? Math.max(cell.y + cell.height, given.offsetTop + given.offsetHeight) : cell.y + cell.height;
    viewer.current?.reveal({ ...cell, width: right - cell.x, height: bottom - cell.y }, cover.current);
  }, [chosenCell, current, under]);
  return (
    <div
      ref={root}
      className="graph"
      onClick={(ev) => {
        // The chart's background lets go of what is chosen; the map and what is drawn on the chart do not.
        const on = (ev.target as Element).closest(
          'button, [data-edge], [data-node], .graph-context, .graph-map, .graph-zoom',
        );
        if (on === null) onChoose(null);
      }}
    >
      {failed !== null && <p className="message">The chart could not be laid out: {failed}</p>}
      {current === null && failed === null && <p className="message muted">Laying the chart out</p>}
      {current !== null && size !== null && (
        <Viewer
          ref={viewer}
          width={size.width}
          height={size.height}
          over={map !== null ? <Minimap map={map} viewer={viewer} /> : null}
        >
          <Sheet
            flow={flow}
            chart={chart}
            laid={over?.laid ?? current}
            measured={measured}
            pad={PAD}
            dataLabels={over?.labels ?? []}
            aside={over?.aside ?? null}
            chosen={chosen}
            onChoose={onChoose}
          />
        </Viewer>
      )}
    </div>
  );
}

export default FlowChart;
