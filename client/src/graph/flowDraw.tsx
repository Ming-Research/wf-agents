// How a flow's chart is drawn: each node's size and the lines of its text, measured in the font the page draws
// them in, and the chart itself as SVG once laid out. Who does a cell's work decides its colours: an agent's round
// takes its line's, a cell on no line is white and dotted, the engine's own step is purple, dashed and rounded, a tool
// the engine runs grey and dashed, the owner orange and rounded.

import type { ReactNode } from 'react';

import type { CompiledCell, CompiledGraph } from '../api';
import { t } from '../text';
import type { Chart, ChartEdge, ChartNode, Overlay } from './flowModel';
import { type Box, type Laid, pathOf, type Size } from './flowLayout';

/// The lines that have colours of their own; a line not here is drawn in the colour of other lines.
const LINES = ['engineer', 'review', 'adversarial', 'advisor', 'architect'];

/// The style a cell is drawn in: its line's for an agent's round, `fresh` for one on no line, else its kind's.
export function styleOf(c: CompiledCell): string {
  if (c.kind === 'agent') {
    if (c.line === null) return 'fresh';
    return LINES.includes(c.line) ? `line-${c.line}` : 'line-other';
  }
  return ['engine', 'tool', 'owner'].includes(c.kind) ? `kind-${c.kind}` : 'kind-engine';
}

const FONT = '13px ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif';
const BOLD = `600 ${FONT}`;
const SMALL = '12px ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif';

let canvas: CanvasRenderingContext2D | null | undefined;

/// The width a text takes in a font: measured where the page runs, estimated elsewhere.
export function measure(text: string, font = FONT): number {
  if (canvas === undefined) {
    canvas = typeof document === 'undefined' ? null : document.createElement('canvas').getContext('2d');
  }
  if (canvas) {
    canvas.font = font;
    return canvas.measureText(text).width;
  }
  const size = font.includes('12px') ? 12 : 13;
  return text.length * size * (font.startsWith('600') ? 0.6 : 0.55);
}

/// The widest a cell's text runs before it wraps, and the height of a line of it.
const WRAP = 230;
const LINE = 18;
const PAD_X = 16;
const PAD_Y = 11;

/// A cell's text, `<id> <title>`, in lines no wider than the cell's text may be; the id leads the first line.
export function cellLines(c: CompiledCell): string[] {
  const words = c.title.split(/\s+/).filter((w) => w !== '');
  const lines: string[] = [];
  let line = '';
  let width = measure(`${c.id} `, BOLD);
  for (const w of words) {
    const add = measure(line === '' ? w : ` ${w}`);
    if (line !== '' && width + add > WRAP) {
      lines.push(line);
      line = w;
      width = measure(w);
    } else {
      line = line === '' ? w : `${line} ${w}`;
      width += add;
    }
  }
  lines.push(line);
  return lines;
}

const CIRCLE = 36;
const SHARED = 58;

/// Each node's size, and each cell's lines.
export function sizes(g: CompiledGraph, c: Chart): { size: Map<string, Size>; lines: Map<string, string[]> } {
  const cells = new Map(g.cells.map((x) => [x.id, x]));
  const size = new Map<string, Size>();
  const lines = new Map<string, string[]>();
  for (const n of c.nodes) {
    const cell = cells.get(n.cell);
    if (n.kind === 'cell' && cell) {
      const ls = cellLines(cell);
      lines.set(n.id, ls);
      const widths = ls.map((l, i) => measure(l) + (i === 0 ? measure(`${cell.id} `, BOLD) : 0));
      const width = Math.max(110, Math.ceil(Math.max(...widths)) + 2 * PAD_X + (cell.kind === 'agent' ? 0 : 14));
      size.set(n.id, { width, height: ls.length * LINE + 2 * PAD_Y });
    } else if (n.kind === 'shared') {
      size.set(n.id, { width: SHARED, height: SHARED });
    } else if (n.kind === 'out' || n.kind === 'in') {
      size.set(n.id, { width: Math.max(CIRCLE, Math.ceil(measure(n.other ?? '', SMALL)) + 14), height: CIRCLE });
    } else if (n.kind === 'rule') {
      size.set(n.id, { width: Math.ceil(measure(`rule:${n.other ?? ''}`, SMALL)) + 40, height: 28 });
    } else {
      size.set(n.id, { width: Math.ceil(measure(g.interject?.name ?? '', SMALL)) + 44, height: 28 });
    }
  }
  return { size, lines };
}

/// The size of an arrow's label; none for an arrow without one.
export function labelSize(e: ChartEdge): Size | null {
  if (e.label === '') return null;
  return { width: Math.ceil(measure(e.label, SMALL)) + 12, height: 20 };
}

export type Marks = {
  /// The node whose panel is open.
  selected: string | null;
  /// On a work's page: what the work did at each cell, and how many times it took each arrow.
  overlays: Map<string, Overlay> | null;
  taken: Map<string, number> | null;
  /// When two flows are compared: the cells added and changed.
  added: Set<string>;
  changed: Set<string>;
};

function CellShape({ n, cell, box, lines, marks }: {
  n: ChartNode;
  cell: CompiledCell;
  box: Box;
  lines: string[];
  marks: Marks;
}) {
  const o = marks.overlays?.get(cell.id) ?? null;
  const round = cell.kind === 'engine' || cell.kind === 'owner';
  const cls = [
    'fc-cell',
    styleOf(cell),
    marks.selected === n.id ? 'fc-selected' : '',
    o?.current ? 'fc-current' : '',
    marks.overlays && !o ? 'fc-unvisited' : '',
    marks.added.has(cell.id) ? 'fc-added' : marks.changed.has(cell.id) ? 'fc-changed' : '',
  ]
    .filter((x) => x !== '')
    .join(' ');
  const textTop = box.y + PAD_Y + 13;
  const badges: ReactNode[] = [];
  if (o) {
    const items: Array<[string, string, string]> = [];
    if (o.running) items.push(['fc-b-running', '●', t('chart_running')]);
    if (o.rounds > 0) items.push(['fc-b-rounds', String(o.rounds), t('chart_rounds', { n: o.rounds })]);
    if (o.sentBack > 0) items.push(['fc-b-back', `↩${o.sentBack}`, t('chart_sent_back', { n: o.sentBack })]);
    if (o.failed > 0) items.push(['fc-b-failed', `✕${o.failed}`, t('chart_failed', { n: o.failed })]);
    let x = box.x + box.width + 4;
    for (const [k, text, title] of items.reverse()) {
      const w = Math.ceil(measure(text, SMALL)) + 10;
      x -= w + 3;
      badges.push(
        <g key={k} className={`fc-badge ${k}`}>
          <title>{title}</title>
          <rect x={x} y={box.y - 10} width={w} height={18} rx={9} />
          <text x={x + w / 2} y={box.y + 3} textAnchor="middle">
            {text}
          </text>
        </g>,
      );
    }
  }
  const mark = marks.added.has(cell.id) ? t('chart_added') : marks.changed.has(cell.id) ? t('chart_changed') : null;
  return (
    <g className={cls} data-node={n.id}>
      <title>{`${cell.id} ${cell.title}`}</title>
      {o?.current ? (
        <rect
          className="fc-ring"
          x={box.x - 6}
          y={box.y - 6}
          width={box.width + 12}
          height={box.height + 12}
          rx={round ? (box.height + 12) / 2 : 9}
        />
      ) : null}
      <rect x={box.x} y={box.y} width={box.width} height={box.height} rx={round ? box.height / 2 : 3} />
      <text x={box.x + box.width / 2} y={textTop} textAnchor="middle">
        {lines.map((l, i) => (
          <tspan key={i} x={box.x + box.width / 2} dy={i === 0 ? 0 : LINE}>
            {i === 0 ? <tspan className="fc-id">{`${cell.id} `}</tspan> : null}
            {l}
          </tspan>
        ))}
      </text>
      {mark !== null ? (
        <text className="fc-mark" x={box.x + 4} y={box.y - 4}>
          {mark}
        </text>
      ) : null}
      {badges}
    </g>
  );
}

function Circle({ n, box, title, text, cls }: { n: ChartNode; box: Box; title: string; text: string[]; cls: string }) {
  const cx = box.x + box.width / 2;
  const cy = box.y + box.height / 2;
  return (
    <g className={`fc-circle ${cls}`} data-node={n.id}>
      <title>{title}</title>
      <ellipse cx={cx} cy={cy} rx={box.width / 2} ry={box.height / 2} />
      <text x={cx} y={cy + 4 - ((text.length - 1) * 13) / 2} textAnchor="middle">
        {text.map((l, i) => (
          <tspan key={i} x={cx} dy={i === 0 ? 0 : 13}>
            {l}
          </tspan>
        ))}
      </text>
    </g>
  );
}

export type DrawProps = {
  g: CompiledGraph;
  chart: Chart;
  laid: Laid;
  lines: Map<string, string[]>;
  marks: Marks;
};

/// The chart as SVG, at the size its layout takes.
export function FlowSvg({ g, chart, laid, lines, marks }: DrawProps) {
  const cells = new Map(g.cells.map((c) => [c.id, c]));
  const title = (id: string) => {
    const c = cells.get(id);
    return c ? `${c.id} ${c.title}` : id;
  };
  const b = laid.bounds;
  const pad = 24;
  return (
    <svg
      className="fc"
      width={b.width + 2 * pad}
      height={b.height + 2 * pad}
      viewBox={`${b.x - pad} ${b.y - pad} ${b.width + 2 * pad} ${b.height + 2 * pad}`}
    >
      <defs>
        {['plain', 'main', 'taken', 'rule', 'quiet'].map((k) => (
          <marker
            key={k}
            id={`fc-arrow-${k}`}
            className={`fc-head fc-head-${k}`}
            viewBox="0 0 10 10"
            refX="9"
            refY="5"
            markerWidth={k === 'main' || k === 'taken' ? 6 : 7}
            markerHeight={k === 'main' || k === 'taken' ? 6 : 7}
            orient="auto-start-reverse"
          >
            <path d="M 0 0 L 10 5 L 0 10 z" />
          </marker>
        ))}
      </defs>
      {chart.groups.map((gr) => {
        const box = laid.groups.get(gr.id);
        if (!box) return null;
        return (
          <g key={gr.id} className="fc-group">
            <rect x={box.x} y={box.y} width={box.width} height={box.height} rx={4} />
            <text x={box.x + 12} y={box.y + 21}>
              {gr.id === 'asking' ? t('chart_asking_part') : t('chart_stage', { stage: gr.label })}
            </text>
          </g>
        );
      })}
      {chart.edges.map((e) => {
        const r = laid.edges.get(e.id);
        if (!r) return null;
        const n = marks.taken?.get(e.id) ?? 0;
        const kind =
          e.kind === 'rule' || e.kind === 'interjection'
            ? 'rule'
            : n > 0
              ? 'taken'
              : marks.taken
                ? 'quiet'
                : e.main
                  ? 'main'
                  : 'plain';
        const cls = ['fc-edge', `fc-edge-${kind}`, e.kind === 'shared' ? 'fc-edge-shared' : ''].join(' ');
        return (
          <path key={e.id} className={cls} d={pathOf(r)} markerEnd={`url(#fc-arrow-${kind})`} data-edge={e.id} />
        );
      })}
      {chart.edges.map((e) => {
        const r = laid.edges.get(e.id);
        if (!r?.label) return null;
        const n = marks.taken?.get(e.id) ?? 0;
        const l = r.label;
        const text = n > 0 ? `${e.label} ×${n}` : e.label;
        const w = n > 0 ? Math.max(l.width, Math.ceil(measure(text, SMALL)) + 12) : l.width;
        return (
          <g key={`l:${e.id}`} className={`fc-label${e.main ? ' fc-label-main' : ''}${n > 0 ? ' fc-label-taken' : ''}`}>
            <rect x={l.x + (l.width - w) / 2} y={l.y} width={w} height={l.height} rx={3} />
            <text x={l.x + l.width / 2} y={l.y + l.height / 2 + 4} textAnchor="middle">
              {text}
            </text>
          </g>
        );
      })}
      {chart.nodes.map((n) => {
        const box = laid.nodes.get(n.id);
        if (!box) return null;
        const cell = cells.get(n.cell);
        switch (n.kind) {
          case 'cell':
            return cell ? (
              <CellShape key={n.id} n={n} cell={cell} box={box} lines={lines.get(n.id) ?? []} marks={marks} />
            ) : null;
          case 'shared':
            return (
              <Circle
                key={n.id}
                n={n}
                box={box}
                cls="fc-shared"
                title={n.rows.map((i) => `${cell?.rows[i]?.name ?? ''} → ${cell?.rows[i]?.to ?? ''}`).join('\n')}
                text={[t('chart_shared_1'), t('chart_shared_2')]}
              />
            );
          case 'out':
            return (
              <Circle
                key={n.id}
                n={n}
                box={box}
                cls="fc-out"
                title={t('chart_out_title', { cell: title(n.other ?? '') })}
                text={[n.other ?? '']}
              />
            );
          case 'in':
            return (
              <Circle
                key={n.id}
                n={n}
                box={box}
                cls="fc-in"
                title={t('chart_in_title', { cell: title(n.other ?? '') })}
                text={[n.other ?? '']}
              />
            );
          case 'rule': {
            const s = 9;
            const pts = [
              [box.x + s, box.y],
              [box.x + box.width, box.y],
              [box.x + box.width - s, box.y + box.height],
              [box.x, box.y + box.height],
            ];
            return (
              <g key={n.id} className={`fc-rule${marks.selected === n.id ? ' fc-selected' : ''}`} data-node={n.id}>
                <title>{t('chart_rule_title', { rule: n.other ?? '' })}</title>
                <polygon points={pts.map((p) => p.join(',')).join(' ')} />
                <text x={box.x + box.width / 2} y={box.y + box.height / 2 + 4} textAnchor="middle">
                  {`rule:${n.other ?? ''}`}
                </text>
              </g>
            );
          }
          case 'interjection': {
            const notch = 10;
            const pts = [
              [box.x, box.y],
              [box.x + box.width, box.y],
              [box.x + box.width, box.y + box.height],
              [box.x, box.y + box.height],
              [box.x + notch, box.y + box.height / 2],
            ];
            return (
              <g
                key={n.id}
                className={`fc-flag${marks.selected === n.id ? ' fc-selected' : ''}`}
                data-node={n.id}
              >
                <title>{t('chart_interjection_title')}</title>
                <polygon points={pts.map((p) => p.join(',')).join(' ')} />
                <text x={box.x + box.width / 2 + notch / 2} y={box.y + box.height / 2 + 4} textAnchor="middle">
                  {g.interject?.name ?? ''}
                </text>
              </g>
            );
          }
        }
        return null;
      })}
    </svg>
  );
}
