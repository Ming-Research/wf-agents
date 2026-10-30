// A flow's state machine, filling the space it is given: the chart is built from the compiled flow and what the
// interface says of it, laid out by elkjs and drawn as SVG in a pan-and-zoom view, with a "?" in the bottom-left
// corner that opens what every mark means. Several graphs are drawn side by side on the one chart, each under its
// name, and move and zoom together; the stores the engine keeps what it writes in are drawn between the first two, or
// beside the one, and pointing at a cell, the actions on a line or a store, or opening one, draws the lines between
// them. On a work's page each cell carries what the work did there and the arrows the work took stand out; when two
// flows are compared, the cells added and changed are marked. An arrow's label names its rows, with no numbers, as a
// cell's page names them. A chart that opens on a cell opens at full size, centred on it; when a round starts running at another
// cell than the one it last centred on, it centres on that cell at the zoom it has. A map of the whole chart over its
// top left corner shows the part in view and moves the view, while some of the chart is out of view. A click on the
// chart's background, not a drag, lets go of what is picked.

import { type ReactNode, type Ref, useEffect, useImperativeHandle, useMemo, useRef, useState } from 'react';

import type { CompiledGraph, EngineNames, GraphView } from '../api';
import { t } from '../text';
import {
  EffortBadge,
  effortsOf,
  effortWidth,
  FlowSvg,
  Halo,
  labelSize,
  type Marks,
  SessionBadge,
  sessionWidth,
  sizes,
  styleOf,
} from './flowDraw';
import { besides, type Laid, layoutChart } from './flowLayout';
import { actionWords, type Chart, chart as build, MAIN, type Move } from './flowModel';
import { mapOf, Minimap } from './Minimap';
import { layStores, linkPath, LinksSvg, storeLinks, StoresSvg, storeSize } from './stores';
import { type Rect, Viewer, type ViewerHandle } from './Viewer';

/// A graph of the flow, with what the interface says of it.
export type Graph = { g: CompiledGraph; view: GraphView };

/// What a click on the chart picked, in which graph.
export type Pick =
  | { kind: 'cell'; graph: string; cell: string }
  | { kind: 'rule'; graph: string; rule: string; cell: string }
  | { kind: 'interjection'; graph: string; cell: string }
  | { kind: 'action'; graph: string; node: string }
  | { kind: 'store'; store: string }
  | { kind: 'out'; graph: string; cell: string; to: string }
  | { kind: 'in'; graph: string; cell: string; from: string }
  /// Nothing: the chart's background, a stage's box or a graph's name, which lets go of what is picked.
  | { kind: 'nothing' };

/// What a click that did not drag the chart lands on, as the element under the pointer answers it.
export type Under = { closest: (selector: string) => { getAttribute: (name: string) => string | null } | null };

/// What a click on the chart picks, from the element under the pointer and each sheet's graph and chart: a store, a
/// node of a graph, nothing for the chart's background (a stage's box, a graph's name, the room between them), and
/// none for an arrow or its label, which the drawing keeps lit itself.
export function pickAt(target: Under, graphs: string[], models: Chart[]): Pick | null {
  const s = target.closest('[data-store]')?.getAttribute('data-store');
  if (s) return { kind: 'store', store: s };
  const id = target.closest('[data-node]')?.getAttribute('data-node');
  if (!id) return target.closest('[data-edge]') === null ? { kind: 'nothing' } : null;
  const i = Number(target.closest('[data-sheet]')?.getAttribute('data-sheet') ?? -1);
  const n = models[i]?.nodes.find((x) => x.id === id);
  const graph = graphs[i];
  if (!n || graph === undefined) return null;
  switch (n.kind) {
    case 'cell':
    case 'does':
      return { kind: 'cell', graph, cell: n.cell };
    case 'rule':
      return { kind: 'rule', graph, rule: n.other ?? '', cell: n.cell };
    case 'interjection':
      return { kind: 'interjection', graph, cell: n.cell };
    case 'action':
      return { kind: 'action', graph, node: n.id };
    case 'out':
      return { kind: 'out', graph, cell: n.cell, to: n.other ?? '' };
    case 'in':
      return { kind: 'in', graph, cell: n.cell, from: n.other ?? '' };
  }
}

export type FlowChartHandle = {
  /// Brings a cell into view, of the graph named or the first that has it: keeping the zoom when it is already in
  /// sight, or zooming in to read it.
  focus: (cell: string, keep?: boolean, graph?: string) => void;
};

/// The least zoom the chart opens at when it opens on no cell.
const READABLE = 0.62;
const PAD = 24;

export type FlowChartProps = {
  /// The graphs, left to right; the caller keeps the list the same object while its graphs are the same, since each
  /// new list is laid out again.
  graphs: Graph[];
  /// What is marked on each graph, in the same order.
  marks: Marks[];
  /// The flow's engine part: the stores, what each action writes and what each value reads.
  engine: EngineNames | null;
  /// The store whose panel is open.
  store?: string | null;
  /// The cell the chart opens on; with none, the top of the first graph.
  start: string | null;
  /// The graph of that cell; with none, the first that has it.
  startGraph?: string | null;
  onPick: (p: Pick) => void;
  /// What the legend explains besides the kinds of box: a work's marks, or the marks of a comparison.
  legend: 'flow' | 'work' | 'compare';
  ref?: Ref<FlowChartHandle>;
};

function Swatch({ children }: { children: ReactNode }) {
  return (
    <svg className="fc legend-swatch" width="34" height="18" viewBox="0 0 34 18">
      {children}
    </svg>
  );
}

/// A badge of the work's marks on a cell, as the legend shows it.
function BadgeSwatch({ k, text }: { k: string; text: string }) {
  return (
    <Swatch>
      <g className={`fc-badge ${k}`}>
        <rect x="5" y="1" width="24" height="16" rx="8" />
        <text x="17" y="13" textAnchor="middle">
          {text}
        </text>
      </g>
    </Swatch>
  );
}

/// What the legend says of a move of an agent's cell: where it goes, and which cells make it, as the chart draws it.
export function moveWords(m: Move): string {
  const name = t('legend_move', { move: actionWords(m.row), to: m.to });
  const who = m.lines === null ? t('legend_move_every') : t('legend_move_lines', { lines: m.lines.join(', ') });
  const missing = m.could.filter((c) => !m.cells.includes(c)).join(', ');
  switch (m.tier) {
    case 'legend':
      return `${name}: ${who}`;
    case 'marked':
      return `${name}: ${who}, ${t(m.engine ? 'legend_move_but' : 'legend_move_marked', { cells: missing })}`;
    case 'lines':
      return `${name}: ${t('legend_move_drawn', { n: m.cells.length, of: m.could.length })}`;
  }
}

/// What every mark on the chart means, opened from the "?" in its corner: the kinds of cell, the efforts agents' cells
/// run at, low to high, the rules and the interjection, the circles, the main path and the rows into and out of the
/// cell lit, and the marks of a work, with the bar beside an agent round's cost in its rounds list, or of a comparison;
/// then how the chart moves.
function Legend({ charts, stores, legend }: {
  charts: Array<{ g: CompiledGraph; chart: Chart }>;
  stores: boolean;
  legend: FlowChartProps['legend'];
}) {
  const graphs = charts.map((x) => x.g);
  const cells = graphs.flatMap((g) => g.cells);
  const styles: string[] = [];
  for (const c of cells) {
    const s = styleOf(c);
    if (!styles.includes(s)) styles.push(s);
  }
  const order = (s: string) => (s.startsWith('line-') ? 0 : s === 'fresh' ? 1 : 2);
  styles.sort((a, b) => order(a) - order(b));
  const lines = new Map(cells.filter((c) => c.kind === 'agent' && c.line).map((c) => [styleOf(c), c.line ?? '']));
  const label = (s: string) => {
    if (s === 'fresh') return t('legend_fresh');
    if (s.startsWith('line-')) return t('legend_line', { line: s === 'line-other' ? t('legend_other_line') : (lines.get(s) ?? '') });
    return s === 'kind-engine' ? t('legend_engine') : s === 'kind-tool' ? t('legend_tool') : t('legend_owner');
  };
  const efforts = effortsOf(cells);
  const effortW = effortWidth();
  const rules = cells.some((c) => (c.rules ?? []).length > 0);
  const flags = graphs.some(
    (g) => g.interject !== null && g.cells.some((c) => c.kind !== 'agent' && c.rows.some((r) => r.interject)),
  );
  const work = legend === 'work';
  const several = charts.length > 1;
  const acting = charts.some((x) => x.chart.nodes.some((n) => n.kind === 'action' || n.kind === 'does'));
  const reading = charts.some((x) => [...x.chart.notes.values()].some((n) => n.reads !== null));
  const lacking = charts.some((x) => [...x.chart.notes.values()].some((n) => n.lacks.length > 0));
  return (
    <details className="legend">
      <summary aria-label={t('legend')} title={t('legend')}>
        ?
      </summary>
      <ul>
        {work ? (
          <>
            <li>
              <Swatch>
                <rect className="fc-ring" x="4" y="3" width="26" height="12" rx="3" />
              </Swatch>
              {t('legend_current')}
            </li>
            <li>
              <Swatch>
                <Halo box={{ x: 11, y: 7, width: 12, height: 4 }} round={false} small />
                <rect className="fc-ring fc-ring-running" x="9" y="5" width="16" height="8" rx="3" />
              </Swatch>
              {t('legend_running')}
            </li>
            <li>
              <Swatch>
                <g className="fc-wait">
                  <rect x="3" y="2" width="28" height="14" rx="7" />
                </g>
              </Swatch>
              {t('legend_waits')}
            </li>
            <li>
              <BadgeSwatch k="fc-b-rounds" text="3" />
              {t('legend_rounds')}
            </li>
            <li>
              <BadgeSwatch k="fc-b-back" text="↩1" />
              {t('legend_sent_back')}
            </li>
            <li>
              <BadgeSwatch k="fc-b-failed" text="✕1" />
              {t('legend_failed')}
            </li>
            <li>
              <Swatch>
                <path className="fc-edge fc-edge-taken" d="M 2 9 L 25 9" markerEnd="url(#fc-arrow-taken)" />
              </Swatch>
              {t('legend_taken')}
            </li>
            <li>
              <Swatch>
                <path className="fc-edge fc-edge-lit-in" d="M 2 9 L 17 9" markerEnd="url(#fc-arrow-lit-in)" />
              </Swatch>
              {t('legend_lit_in')}
            </li>
            <li>
              <Swatch>
                <path className="fc-edge fc-edge-lit-out" d="M 2 9 L 17 9" markerEnd="url(#fc-arrow-lit-out)" />
              </Swatch>
              {t('legend_lit_out')}
            </li>
            <li>
              <Swatch>
                <path className="fc-edge fc-edge-quiet" d="M 2 9 L 25 9" markerEnd="url(#fc-arrow-quiet)" />
              </Swatch>
              {t('legend_not_taken')}
            </li>
            <li>
              <Swatch>
                <g className="fc-cell fc-any fc-unvisited">
                  <rect x="4" y="2" width="26" height="14" rx="2" />
                </g>
              </Swatch>
              {t('legend_unvisited')}
            </li>
            <li>
              <Swatch>
                <rect className="fc-spent-read" x="2" y="5" width="9" height="8" />
                <rect className="fc-spent-new" x="11" y="5" width="6" height="8" />
                <rect className="fc-spent-output" x="17" y="5" width="4" height="8" />
                <text className="fc-spent-steps" x="24" y="13">
                  5
                </text>
              </Swatch>
              {t('legend_spent')}
            </li>
            <li>
              <Swatch>
                <rect className="fc-spent-read" x="8" y="5" width="18" height="8" />
              </Swatch>
              {t('spent_key_read')}
            </li>
            <li>
              <Swatch>
                <rect className="fc-spent-new" x="8" y="5" width="18" height="8" />
              </Swatch>
              {t('spent_key_new')}
            </li>
            <li>
              <Swatch>
                <rect className="fc-spent-output" x="8" y="5" width="18" height="8" />
              </Swatch>
              {t('spent_key_output')}
            </li>
          </>
        ) : null}
        {styles.map((s) => (
          <li key={s}>
            <Swatch>
              <g className={`fc-cell ${s}`}>
                <rect x="4" y="2" width="26" height="14" rx={s === 'kind-engine' || s === 'kind-owner' ? 7 : 2} />
              </g>
            </Swatch>
            {s === 'fresh' ? (
              <svg className="fc legend-swatch" width={sessionWidth()} height="18" viewBox={`0 0 ${sessionWidth()} 18`}>
                <SessionBadge x={0} y={1} />
              </svg>
            ) : null}
            {label(s)}
          </li>
        ))}
        {efforts.length > 0 ? (
          <li>
            <svg
              className="fc legend-swatch"
              width={efforts.length * (effortW + 3)}
              height="18"
              viewBox={`0 0 ${efforts.length * (effortW + 3)} 18`}
            >
              {efforts.map((e, i) => (
                <EffortBadge key={e.level} effort={e} x={i * (effortW + 3)} y={1} width={effortW} />
              ))}
            </svg>
            {t('legend_effort')}
          </li>
        ) : null}
        {rules ? (
          <li>
            <Swatch>
              <g className="fc-rule">
                <polygon points="9,2 30,2 25,16 4,16" />
              </g>
            </Swatch>
            {t('legend_rule')}
          </li>
        ) : null}
        {flags ? (
          <li>
            <Swatch>
              <g className="fc-flag">
                <polygon points="4,2 30,2 30,16 4,16 10,9" />
              </g>
            </Swatch>
            {t('legend_interjection')}
          </li>
        ) : null}
        {acting ? (
          <li>
            <Swatch>
              <g className="fc-act">
                <polygon points="6,3 28,3 33,9 28,15 6,15 1,9" />
              </g>
            </Swatch>
            {t('legend_actions')}
          </li>
        ) : null}
        {reading ? (
          <li>
            <Swatch>
              <g className="fc-cell kind-engine">
                <rect x="4" y="1" width="26" height="16" rx="8" />
              </g>
              <path className="fc-reads-line" d="M 10 12 L 24 12" />
            </Swatch>
            {t('legend_reads')}
          </li>
        ) : null}
        {lacking ? (
          <li>
            <Swatch>
              <g className="fc-lack">
                <rect x="3" y="2" width="28" height="14" rx="7" />
              </g>
            </Swatch>
            {t('legend_lacks')}
          </li>
        ) : null}
        {stores ? (
          <>
            <li>
              <Swatch>
                <g className="fc-store">
                  <rect x="4" y="2" width="26" height="14" rx="5" />
                </g>
              </Swatch>
              {t('legend_store')}
            </li>
            <li>
              <Swatch>
                <g className="fc-store fc-store-computed">
                  <rect x="4" y="2" width="26" height="14" rx="5" />
                </g>
              </Swatch>
              {t('legend_store_computed')}
            </li>
            <li>
              <Swatch>
                <path className="fc-link fc-link-write" d="M 2 9 L 31 9" />
              </Swatch>
              {t('legend_link_write')}
            </li>
            <li>
              <Swatch>
                <path className="fc-link fc-link-read" d="M 2 9 L 31 9" />
              </Swatch>
              {t('legend_link_read')}
            </li>
          </>
        ) : null}
        {charts.map(({ g, chart: c }) =>
          c.moves.length + c.shared.length > 0 ? (
            <li key={g.name} className="legend-moves">
              <div>
                <strong>{several ? t('legend_moves_of', { graph: g.name }) : t('legend_moves')}</strong>
                <ul>
                  {c.moves.map((m) => (
                    <li key={m.row}>{moveWords(m)}</li>
                  ))}
                  {c.shared.length > 0 ? (
                    <li>
                      {t('legend_shared_actions', {
                        actions: c.shared.map((a) => t('legend_shared_action', { action: actionWords(a.action), at: a.at })).join(', '),
                      })}
                    </li>
                  ) : null}
                </ul>
              </div>
            </li>
          ) : null,
        )}
        <li>
          <Swatch>
            <g className="fc-circle fc-out">
              <ellipse cx="17" cy="9" rx="7" ry="7" />
            </g>
          </Swatch>
          {t('legend_out')}
        </li>
        <li>
          <Swatch>
            <g className="fc-circle fc-in">
              <ellipse cx="17" cy="9" rx="7" ry="7" />
            </g>
          </Swatch>
          {t('legend_in')}
        </li>
        {!work ? (
          <>
            <li>
              <Swatch>
                <path className="fc-edge fc-edge-main" d="M 2 9 L 25 9" markerEnd="url(#fc-arrow-main)" />
              </Swatch>
              {t('legend_main')}
            </li>
          </>
        ) : null}
        <li>
          <Swatch>
            <path className="fc-edge fc-edge-in" d="M 2 9 L 25 9" markerEnd="url(#fc-arrow-in)" />
          </Swatch>
          {t('legend_way_in')}
        </li>
        <li>
          <Swatch>
            <path className="fc-edge fc-edge-out" d="M 2 9 L 25 9" markerEnd="url(#fc-arrow-out)" />
          </Swatch>
          {t('legend_way_out')}
        </li>
        {legend === 'compare' ? (
          <>
            <li>
              <Swatch>
                <g className="fc-cell fc-any fc-added">
                  <rect x="4" y="2" width="26" height="14" rx="2" />
                </g>
              </Swatch>
              {t('legend_added')}
            </li>
            <li>
              <Swatch>
                <g className="fc-cell fc-any fc-changed">
                  <rect x="4" y="2" width="26" height="14" rx="2" />
                </g>
              </Swatch>
              {t('legend_changed')}
            </li>
          </>
        ) : null}
      </ul>
      <p className="legend-keys">{t('legend_keys')}</p>
      <p className="legend-keys">{t(work ? 'legend_map_work' : 'legend_map')}</p>
    </details>
  );
}

export function FlowChart(props: FlowChartProps) {
  const { graphs, marks, engine, store = null, start, startGraph, onPick, legend, ref } = props;
  const models = useMemo(() => graphs.map(({ g, view }) => build(g, view, engine)), [graphs, engine]);
  const measured = useMemo(() => graphs.map(({ g }, i) => sizes(g, models[i])), [graphs, models]);
  // The stores, between the first two graphs or beside the one, and which node of each graph reads or writes each.
  const stores = useMemo(() => {
    const all = engine?.stores ?? {};
    return Object.keys(all).length > 0 ? { all, laid: layStores(all, storeSize) } : null;
  }, [engine]);
  const links = useMemo(
    () => models.map((m, i) => (engine !== null ? storeLinks(engine, graphs[i].g, m) : [])),
    [engine, graphs, models],
  );
  const [laid, setLaid] = useState<{ models: Chart[]; laid: Laid[] } | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    Promise.all(
      models.map((model, i) => {
        const edges = new Map(model.edges.map((e) => [e.id, e]));
        return layoutChart(
          model,
          (id) => measured[i].size.get(id) ?? { width: 40, height: 40 },
          (id) => {
            const e = edges.get(id);
            return e ? labelSize(e) : null;
          },
        );
      }),
    ).then(
      (l) => {
        if (live) setLaid({ models, laid: l });
      },
      (e: unknown) => {
        if (live) setFailed(e instanceof Error ? e.message : String(e));
      },
    );
    return () => {
      live = false;
    };
  }, [models, measured]);
  const current = laid !== null && laid.models === models ? laid.laid : null;
  // The sheet each graph is drawn on, the stores' sheet taking its place among them.
  const storesAt = graphs.length >= 2 ? 1 : graphs.length;
  const sheetOf = (i: number) => (stores !== null && i >= storesAt ? i + 1 : i);
  // Each sheet at the size its drawing takes, side by side.
  const placed = useMemo(() => {
    if (current === null) return null;
    const sheets = current.map((l) => ({ width: l.bounds.width + 2 * PAD, height: l.bounds.height + 2 * PAD }));
    if (stores !== null) {
      sheets.splice(storesAt, 0, { width: stores.laid.width + 2 * PAD, height: stores.laid.height + 2 * PAD });
    }
    return besides(sheets);
  }, [current, stores, storesAt]);
  const viewer = useRef<ViewerHandle>(null);
  // The map, from the layout the chart already has.
  const map = useMemo(() => {
    if (current === null || placed === null) return null;
    const sheets = current.map((l, i) => ({
      graph: graphs[i].g.name,
      cells: graphs[i].g.cells,
      chart: models[i],
      laid: l,
      left: placed.x[stores !== null && i >= storesAt ? i + 1 : i],
      overlays: marks[i]?.overlays ?? null,
    }));
    return mapOf(placed, sheets, placed.top, PAD, stores !== null ? { laid: stores.laid, left: placed.x[storesAt] } : null);
  }, [current, placed, graphs, models, marks, stores, storesAt]);
  // Chart pixels: each drawing starts PAD before its layout's bounds, at its place among the sheets. A node is found
  // in the graph named, or else in the first graph that has it.
  const rect = (id: string, graph?: string): Rect | null => {
    if (current === null || placed === null) return null;
    const named = graph !== undefined ? graphs.findIndex((x) => x.g.name === graph) : -1;
    const i = named >= 0 ? named : current.findIndex((l) => l.nodes.has(id));
    const l = current[i];
    const b = l?.nodes.get(id);
    if (!l || !b) return null;
    return {
      x: placed.x[sheetOf(i)] + b.x - l.bounds.x + PAD,
      y: placed.top + b.y - l.bounds.y + PAD,
      width: b.width,
      height: b.height,
    };
  };
  const storeRect = (name: string): Rect | null => {
    const b = stores?.laid.stores.get(name);
    if (placed === null || b === undefined) return null;
    return { x: placed.x[storesAt] + b.x + PAD, y: placed.top + b.y + PAD, width: b.width, height: b.height };
  };
  const rectRef = useRef(rect);
  rectRef.current = rect;
  useImperativeHandle(
    ref,
    () => ({
      focus: (cell, keep = false, graph) => {
        const r = rectRef.current(`c:${cell}`, graph);
        if (r) viewer.current?.focus(r, keep);
      },
    }),
    [],
  );
  // Once laid out, open at full size centred on the cell asked for, or on the top of the first graph's main part at a
  // zoom one can read, its name above it in sight.
  const opened = useRef<Laid[] | null>(null);
  useEffect(() => {
    if (current === null || placed === null || opened.current === current || !viewer.current) return;
    opened.current = current;
    const at = start !== null ? rectRef.current(`c:${start}`, startGraph ?? undefined) : null;
    if (at) {
      viewer.current.place(at, 1);
      return;
    }
    const first = current[0];
    const main = first?.parts.get(MAIN);
    const around =
      first && main
        ? {
            x: main.x - first.bounds.x + PAD,
            y: main.y - first.bounds.y + PAD,
            width: main.width,
            height: main.height,
          }
        : null;
    viewer.current.start(READABLE, around);
  }, [current, placed, start, startGraph]);
  // The cell a round runs at now, in its graph; the chart centres on it when it is another than the one it centred on
  // last, the one it opened on included.
  const centred = useRef<string | null>(start);
  const running = useMemo(() => {
    for (const [i, m] of marks.entries()) {
      for (const [cell, o] of m.overlays ?? []) {
        if (o.current && o.running) return { graph: graphs[i]?.g.name, cell };
      }
    }
    return null;
  }, [marks, graphs]);
  const runningKey = running !== null ? `${running.graph ?? ''}\u0000${running.cell}` : null;
  useEffect(() => {
    if (running === null || current === null || placed === null || !viewer.current) return;
    if (centred.current === running.cell) return;
    const r = rectRef.current(`c:${running.cell}`, running.graph);
    if (!r) return;
    centred.current = running.cell;
    viewer.current.centre(r);
    // Only when the cell that runs changes, or the chart is laid out again.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [runningKey, current, placed]);
  // What the lines to the stores are drawn for: what the pointer is on, a store before a node, or else what is open.
  const [pointedNode, setPointedNode] = useState<{ graph: number; node: string } | null>(null);
  const [pointedStore, setPointedStore] = useState<string | null>(null);
  const openNode = marks.findIndex((m) => m.selected !== null);
  const subject: { store: string } | { graph: number; node: string } | null =
    pointedStore !== null
      ? { store: pointedStore }
      : pointedNode !== null
        ? pointedNode
        : store !== null
          ? { store }
          : openNode >= 0
            ? { graph: openNode, node: marks[openNode].selected ?? '' }
            : null;
  const drawnLinks: Array<{ d: string; write: boolean }> = [];
  const litStores = new Set<string>();
  if (subject !== null && current !== null) {
    links.forEach((ls, i) => {
      for (const l of ls) {
        const on = 'store' in subject ? l.store === subject.store : subject.graph === i && l.node === subject.node;
        if (!on) continue;
        const a = rect(l.node, graphs[i].g.name);
        const b = storeRect(l.store);
        if (a === null || b === null) continue;
        drawnLinks.push({ d: linkPath(a, b, l.write), write: l.write });
        litStores.add(l.store);
      }
    });
  }
  const pick = (target: Element) => {
    const p = pickAt(target, graphs.map((x) => x.g.name), models);
    if (p !== null) onPick(p);
  };
  const several = graphs.length > 1;
  return (
    <div className="flowchart">
      {failed !== null ? <div className="error flowchart-note">{t('chart_layout_failed', { error: failed })}</div> : null}
      {current === null && failed === null ? <div className="muted flowchart-note">{t('chart_laying_out')}</div> : null}
      {current !== null && placed !== null ? (
        <Viewer
          ref={viewer}
          width={placed.width}
          height={placed.height}
          onPick={pick}
          over={map !== null ? <Minimap map={map} viewer={viewer} /> : null}
        >
          <div className="fc-sheets" style={{ width: placed.width, height: placed.height }}>
            {current.map((l, i) => {
              const { g } = graphs[i];
              return (
                <div key={g.name} className="fc-sheet" data-sheet={i} style={{ left: placed.x[sheetOf(i)] }}>
                  <div className="fc-sheet-body" style={{ marginTop: placed.top }}>
                    {several ? (
                      <>
                        <div className="fc-sheet-frame" />
                        <div className="fc-sheet-name">{t('chart_graph', { graph: g.name })}</div>
                      </>
                    ) : null}
                    <FlowSvg
                      g={g}
                      chart={models[i]}
                      laid={l}
                      lines={measured[i].lines}
                      notes={measured[i].notes}
                      engine={engine}
                      marks={marks[i] ?? NO_MARKS}
                      defs={i === 0}
                      onNode={(n) => setPointedNode(n === null ? null : { graph: i, node: n })}
                    />
                  </div>
                </div>
              );
            })}
            {stores !== null ? (
              <div className="fc-sheet" style={{ left: placed.x[storesAt] }}>
                <div className="fc-sheet-body" style={{ marginTop: placed.top }}>
                  <div className="fc-sheet-frame" />
                  <div className="fc-sheet-name">{t('chart_stores')}</div>
                  <StoresSvg
                    stores={stores.all}
                    laid={stores.laid}
                    pad={PAD}
                    selected={store}
                    lit={litStores}
                    onHover={setPointedStore}
                  />
                </div>
              </div>
            ) : null}
            {drawnLinks.length > 0 ? <LinksSvg width={placed.width} height={placed.height} links={drawnLinks} /> : null}
          </div>
        </Viewer>
      ) : null}
      <Legend charts={graphs.map((x, i) => ({ g: x.g, chart: models[i] }))} stores={stores !== null} legend={legend} />
    </div>
  );
}

/// What a graph shows when nothing marks it.
const NO_MARKS: Marks = { selected: null, overlays: null, taken: null, added: new Set(), changed: new Set() };
