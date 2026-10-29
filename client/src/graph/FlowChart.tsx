// A flow's state machine, filling the space it is given: the chart is built from the compiled flow and what the
// interface says of it, laid out by elkjs and drawn as SVG in a pan-and-zoom view, with a "?" in the bottom-left
// corner that opens what every mark means. Several graphs are drawn side by side on the one chart, each under its
// name, and move and zoom together. On a work's page each cell carries what the work did there and the arrows the work
// took stand out, and the rows' numbers, which serve the flow's author, are left out; when two flows are compared, the
// cells added and changed are marked. A chart that opens on a cell opens at full size, centred on it; when a round
// starts running at another cell than the one it last centred on, it centres on that cell at the zoom it has.

import { type ReactNode, type Ref, useEffect, useImperativeHandle, useMemo, useRef, useState } from 'react';

import type { CompiledGraph, GraphView } from '../api';
import { t } from '../text';
import { FlowSvg, Halo, HeldMark, labelSize, type Marks, sizes, styleOf } from './flowDraw';
import { besides, type Laid, layoutChart } from './flowLayout';
import { type Chart, chart as build, MAIN } from './flowModel';
import { type Rect, Viewer, type ViewerHandle } from './Viewer';

/// A graph of the flow, with what the interface says of it.
export type Graph = { g: CompiledGraph; view: GraphView };

/// What a click on the chart picked, in which graph.
export type Pick =
  | { kind: 'cell'; graph: string; cell: string }
  | { kind: 'rule'; graph: string; rule: string; cell: string }
  | { kind: 'interjection'; graph: string; cell: string }
  | { kind: 'shared'; graph: string; cell: string }
  | { kind: 'out'; graph: string; cell: string; to: string }
  | { kind: 'in'; graph: string; cell: string; from: string };

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

/// What every mark on the chart means, opened from the "?" in its corner: the kinds of cell, the rules and the
/// interjection, the circles, and the marks of a work or of a comparison; then how the chart moves.
function Legend({ graphs, legend }: { graphs: CompiledGraph[]; legend: FlowChartProps['legend'] }) {
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
  const rules = cells.some((c) => (c.rules ?? []).length > 0);
  const flags = graphs.some(
    (g) => g.interject !== null && g.cells.some((c) => c.kind !== 'agent' && c.rows.some((r) => r.interject)),
  );
  const shared = [...new Set(cells.flatMap((c) => (c.kind === 'agent' ? c.rows.filter((r) => r.common) : [])).map((r) => r.name))];
  const work = legend === 'work';
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
                <HeldMark x={8} y={0} why={t('legend_held')} />
              </Swatch>
              {t('legend_held')}
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
                <path className="fc-edge fc-edge-lit" d="M 2 9 L 17 9" markerEnd="url(#fc-arrow-lit)" />
              </Swatch>
              {t('legend_lit')}
            </li>
            <li>
              <Swatch>
                <path className="fc-edge fc-edge-quiet" d="M 2 9 L 25 9" markerEnd="url(#fc-arrow-quiet)" />
              </Swatch>
              {t('legend_not_taken')}
            </li>
            <li>
              <Swatch>
                <g className="fc-cell kind-tool fc-unvisited">
                  <rect x="4" y="2" width="26" height="14" rx="2" />
                </g>
              </Swatch>
              {t('legend_unvisited')}
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
            {label(s)}
          </li>
        ))}
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
        {shared.length > 0 ? (
          <li>
            <Swatch>
              <path className="fc-edge fc-edge-shared" d="M 1 9 L 14 9" />
              <g className="fc-circle fc-shared">
                <ellipse cx="24" cy="9" rx="8" ry="7" />
              </g>
            </Swatch>
            {t('legend_shared', { rows: shared.join(', ') })}
          </li>
        ) : null}
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
            <li>
              <Swatch>
                <g className="fc-no">
                  <circle cx="17" cy="9" r="8" />
                  <text x="17" y="12.5" textAnchor="middle">
                    2
                  </text>
                </g>
              </Swatch>
              {t('legend_number')}
            </li>
          </>
        ) : null}
        {legend === 'compare' ? (
          <>
            <li>
              <Swatch>
                <g className="fc-cell kind-tool fc-added">
                  <rect x="4" y="2" width="26" height="14" rx="2" />
                </g>
              </Swatch>
              {t('legend_added')}
            </li>
            <li>
              <Swatch>
                <g className="fc-cell kind-tool fc-changed">
                  <rect x="4" y="2" width="26" height="14" rx="2" />
                </g>
              </Swatch>
              {t('legend_changed')}
            </li>
          </>
        ) : null}
      </ul>
      <p className="legend-keys">{t('legend_keys')}</p>
    </details>
  );
}

export function FlowChart(props: FlowChartProps) {
  const { graphs, marks, start, startGraph, onPick, legend, ref } = props;
  const numbered = legend !== 'work';
  const models = useMemo(() => graphs.map(({ g, view }) => build(g, view)), [graphs]);
  const measured = useMemo(() => graphs.map(({ g }, i) => sizes(g, models[i])), [graphs, models]);
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
            return e ? labelSize(e, numbered) : null;
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
  }, [models, measured, numbered]);
  const current = laid !== null && laid.models === models ? laid.laid : null;
  // Each graph at the size its drawing takes, side by side.
  const placed = useMemo(
    () =>
      current === null
        ? null
        : besides(current.map((l) => ({ width: l.bounds.width + 2 * PAD, height: l.bounds.height + 2 * PAD }))),
    [current],
  );
  const viewer = useRef<ViewerHandle>(null);
  // Chart pixels: each drawing starts PAD before its layout's bounds, at its place among the graphs. A node is found
  // in the graph named, or else in the first graph that has it.
  const rect = (id: string, graph?: string): Rect | null => {
    if (current === null || placed === null) return null;
    const named = graph !== undefined ? graphs.findIndex((x) => x.g.name === graph) : -1;
    const i = named >= 0 ? named : current.findIndex((l) => l.nodes.has(id));
    const l = current[i];
    const b = l?.nodes.get(id);
    if (!l || !b) return null;
    return {
      x: placed.x[i] + b.x - l.bounds.x + PAD,
      y: placed.top + b.y - l.bounds.y + PAD,
      width: b.width,
      height: b.height,
    };
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
  const nodes = useMemo(() => models.map((m) => new Map(m.nodes.map((n) => [n.id, n]))), [models]);
  const pick = (target: Element) => {
    const i = Number(target.closest('[data-sheet]')?.getAttribute('data-sheet') ?? -1);
    const id = target.closest('[data-node]')?.getAttribute('data-node');
    const n = id ? nodes[i]?.get(id) : undefined;
    const graph = graphs[i]?.g.name;
    if (!n || graph === undefined) return;
    switch (n.kind) {
      case 'cell':
        onPick({ kind: 'cell', graph, cell: n.cell });
        break;
      case 'rule':
        onPick({ kind: 'rule', graph, rule: n.other ?? '', cell: n.cell });
        break;
      case 'interjection':
        onPick({ kind: 'interjection', graph, cell: n.cell });
        break;
      case 'shared':
        onPick({ kind: 'shared', graph, cell: n.cell });
        break;
      case 'out':
        onPick({ kind: 'out', graph, cell: n.cell, to: n.other ?? '' });
        break;
      case 'in':
        onPick({ kind: 'in', graph, cell: n.cell, from: n.other ?? '' });
        break;
    }
  };
  const several = graphs.length > 1;
  return (
    <div className="flowchart">
      {failed !== null ? <div className="error flowchart-note">{t('chart_layout_failed', { error: failed })}</div> : null}
      {current === null && failed === null ? <div className="muted flowchart-note">{t('chart_laying_out')}</div> : null}
      {current !== null && placed !== null ? (
        <Viewer ref={viewer} width={placed.width} height={placed.height} onPick={pick}>
          <div className="fc-sheets" style={{ width: placed.width, height: placed.height }}>
            {current.map((l, i) => {
              const { g } = graphs[i];
              return (
                <div key={g.name} className="fc-sheet" data-sheet={i} style={{ left: placed.x[i] }}>
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
                      marks={marks[i] ?? NO_MARKS}
                      numbered={numbered}
                      defs={i === 0}
                    />
                  </div>
                </div>
              );
            })}
          </div>
        </Viewer>
      ) : null}
      <Legend graphs={graphs.map((x) => x.g)} legend={legend} />
    </div>
  );
}

/// What a graph shows when nothing marks it.
const NO_MARKS: Marks = { selected: null, overlays: null, taken: null, added: new Set(), changed: new Set() };
