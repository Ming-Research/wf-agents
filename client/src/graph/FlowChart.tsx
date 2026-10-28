// A flow's state machine, filling the space it is given: the chart is built from the compiled flow and what the
// interface says of it, laid out by elkjs and drawn as SVG in a pan-and-zoom view, with a "?" in the bottom-left
// corner that opens what every mark means. On a work's page each cell carries what the work did there and the arrows
// the work took stand out, and the rows' numbers, which serve the flow's author, are left out; when two flows are
// compared, the cells added and changed are marked. A chart that opens on a cell opens at full size, centred on it.

import { type ReactNode, type Ref, useEffect, useImperativeHandle, useMemo, useRef, useState } from 'react';

import type { CompiledGraph, GraphView } from '../api';
import { t } from '../text';
import { FlowSvg, Halo, HeldMark, labelSize, type Marks, sizes, styleOf } from './flowDraw';
import { type Laid, layoutChart } from './flowLayout';
import { chart as build, MAIN } from './flowModel';
import { type Rect, Viewer, type ViewerHandle } from './Viewer';

/// What a click on the chart picked.
export type Pick =
  | { kind: 'cell'; cell: string }
  | { kind: 'rule'; rule: string; cell: string }
  | { kind: 'interjection'; cell: string }
  | { kind: 'shared'; cell: string }
  | { kind: 'out'; cell: string; to: string }
  | { kind: 'in'; cell: string; from: string };

export type FlowChartHandle = {
  /// Brings a cell into view: keeping the zoom when it is already in sight, or zooming in to read it.
  focus: (cell: string, keep?: boolean) => void;
};

/// The least zoom the chart opens at when it opens on no cell.
const READABLE = 0.62;
const PAD = 24;

export type FlowChartProps = {
  g: CompiledGraph;
  view: GraphView;
  marks: Marks;
  /// The cell the chart opens on; with none, the top of the flow.
  start: string | null;
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
function Legend({ g, legend }: { g: CompiledGraph; legend: FlowChartProps['legend'] }) {
  const styles: string[] = [];
  for (const c of g.cells) {
    const s = styleOf(c);
    if (!styles.includes(s)) styles.push(s);
  }
  const order = (s: string) => (s.startsWith('line-') ? 0 : s === 'fresh' ? 1 : 2);
  styles.sort((a, b) => order(a) - order(b));
  const lines = new Map(g.cells.filter((c) => c.kind === 'agent' && c.line).map((c) => [styleOf(c), c.line ?? '']));
  const label = (s: string) => {
    if (s === 'fresh') return t('legend_fresh');
    if (s.startsWith('line-')) return t('legend_line', { line: s === 'line-other' ? t('legend_other_line') : (lines.get(s) ?? '') });
    return s === 'kind-engine' ? t('legend_engine') : s === 'kind-tool' ? t('legend_tool') : t('legend_owner');
  };
  const rules = g.cells.some((c) => (c.rules ?? []).length > 0);
  const flags = g.interject !== null && g.cells.some((c) => c.kind !== 'agent' && c.rows.some((r) => r.interject));
  const shared = [...new Set(g.cells.flatMap((c) => (c.kind === 'agent' ? c.rows.filter((r) => r.common) : [])).map((r) => r.name))];
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
                <Halo box={{ x: 8, y: 6, width: 18, height: 6 }} round={false} small />
                <rect className="fc-ring fc-ring-running" x="6" y="4" width="22" height="10" rx="3" />
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
                <path className="fc-edge fc-edge-lit" d="M 2 9 L 25 9" markerEnd="url(#fc-arrow-lit)" />
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
  const { g, view, marks, start, onPick, legend, ref } = props;
  const numbered = legend !== 'work';
  const model = useMemo(() => build(g, view), [g, view]);
  const measured = useMemo(() => sizes(g, model), [g, model]);
  const [laid, setLaid] = useState<{ model: typeof model; laid: Laid } | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    const edges = new Map(model.edges.map((e) => [e.id, e]));
    layoutChart(
      model,
      (id) => measured.size.get(id) ?? { width: 40, height: 40 },
      (id) => {
        const e = edges.get(id);
        return e ? labelSize(e, numbered) : null;
      },
    ).then(
      (l) => {
        if (live) setLaid({ model, laid: l });
      },
      (e: unknown) => {
        if (live) setFailed(e instanceof Error ? e.message : String(e));
      },
    );
    return () => {
      live = false;
    };
  }, [model, measured, numbered]);
  const current = laid !== null && laid.model === model ? laid.laid : null;
  const viewer = useRef<ViewerHandle>(null);
  // Chart pixels: the drawing starts PAD before the layout's bounds.
  const rect = (id: string): Rect | null => {
    const b = current?.nodes.get(id);
    if (!b || !current) return null;
    return { x: b.x - current.bounds.x + PAD, y: b.y - current.bounds.y + PAD, width: b.width, height: b.height };
  };
  const rectRef = useRef(rect);
  rectRef.current = rect;
  useImperativeHandle(
    ref,
    () => ({
      focus: (cell, keep = false) => {
        const r = rectRef.current(`c:${cell}`);
        if (r) viewer.current?.focus(r, keep);
      },
    }),
    [],
  );
  // Once laid out, open at full size centred on the cell asked for, or on the top of the main part at a zoom one can
  // read.
  const opened = useRef<Laid | null>(null);
  useEffect(() => {
    if (current === null || opened.current === current || !viewer.current) return;
    opened.current = current;
    const at = start !== null ? rectRef.current(`c:${start}`) : null;
    if (at) {
      viewer.current.place(at, 1);
      return;
    }
    const main = current.parts.get(MAIN);
    const around = main
      ? { x: main.x - current.bounds.x + PAD, y: main.y - current.bounds.y + PAD, width: main.width, height: main.height }
      : null;
    viewer.current.start(READABLE, around);
  }, [current, start]);
  const nodes = useMemo(() => new Map(model.nodes.map((n) => [n.id, n])), [model]);
  const pick = (target: Element) => {
    const id = target.closest('[data-node]')?.getAttribute('data-node');
    const n = id ? nodes.get(id) : undefined;
    if (!n) return;
    switch (n.kind) {
      case 'cell':
        onPick({ kind: 'cell', cell: n.cell });
        break;
      case 'rule':
        onPick({ kind: 'rule', rule: n.other ?? '', cell: n.cell });
        break;
      case 'interjection':
        onPick({ kind: 'interjection', cell: n.cell });
        break;
      case 'shared':
        onPick({ kind: 'shared', cell: n.cell });
        break;
      case 'out':
        onPick({ kind: 'out', cell: n.cell, to: n.other ?? '' });
        break;
      case 'in':
        onPick({ kind: 'in', cell: n.cell, from: n.other ?? '' });
        break;
    }
  };
  return (
    <div className="flowchart">
      {failed !== null ? <div className="error flowchart-note">{t('chart_layout_failed', { error: failed })}</div> : null}
      {current === null && failed === null ? <div className="muted flowchart-note">{t('chart_laying_out')}</div> : null}
      {current !== null ? (
        <Viewer
          ref={viewer}
          width={current.bounds.width + 2 * PAD}
          height={current.bounds.height + 2 * PAD}
          onPick={pick}
        >
          <FlowSvg g={g} chart={model} laid={current} lines={measured.lines} marks={marks} numbered={numbered} />
        </Viewer>
      ) : null}
      <Legend g={g} legend={legend} />
    </div>
  );
}
