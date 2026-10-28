// How a flow's chart is drawn: each node's size and the lines of its text, measured in the font the page draws
// them in, and the chart itself as SVG once laid out. Who does a cell's work decides its colours: an agent's round
// takes its line's, a cell on no line is white and dotted, the engine's own step is purple, dashed and rounded, a tool
// the engine runs grey and dashed, the owner orange and rounded. A row's number is a badge of one size with its digits
// in it. Hovering an arrow or its label lights up the whole row, every arrow that draws it from its source to its
// target.

import { type ReactNode, useId, useState } from 'react';

import type { CompiledCell, CompiledGraph } from '../api';
import { t } from '../text';
import { type Chart, type ChartEdge, type ChartNode, type Overlay, related } from './flowModel';
import { type Box, type Laid, pathOf, type Size } from './flowLayout';

/// The lines that have colours of their own; a line not here is drawn in the colour of other lines.
const LINES = ['engineer', 'review', 'adversarial', 'advisor', 'architect'];

/// The style a cell is drawn in: its line's for an agent's round, `fresh` for one on no line, else its kind's.
export function styleOf(c: CompiledCell): string {
  if (c.kind === 'agent') {
    if (c.line == null) return 'fresh';
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

/// A row number's badge: its diameter, the room between two badges, and the room a dash between a run's first and
/// last row takes.
const BADGE = 16;
const BADGE_GAP = 2;
const DASH = 10;
/// The room between the numbers and the names in a label.
const NAMES_GAP = 4;

/// Where a label's row numbers go from its left edge: the centre of each badge and of each dash between a run's first
/// and last row, and the width they take.
export function badgeRow(numbers: Array<[number, number]>): {
  items: Array<{ x: number; row: number | null }>;
  width: number;
} {
  const items: Array<{ x: number; row: number | null }> = [];
  let at = 0;
  for (const [first, last] of numbers) {
    if (at > 0) at += BADGE_GAP;
    items.push({ x: at + BADGE / 2, row: first });
    at += BADGE;
    if (last !== first) {
      items.push({ x: at + DASH / 2, row: null });
      at += DASH;
      items.push({ x: at + BADGE / 2, row: last });
      at += BADGE;
    }
  }
  return { items, width: at };
}

/// The size of an arrow's label, its row numbers drawn or not; none for an arrow without one.
export function labelSize(e: ChartEdge, numbered: boolean): Size | null {
  const nos = numbered ? badgeRow(e.numbers).width : 0;
  const names = e.label === '' ? 0 : measure(e.label, SMALL);
  if (nos === 0 && names === 0) return null;
  return { width: Math.ceil(nos + (nos > 0 && names > 0 ? NAMES_GAP : 0) + names) + 12, height: 20 };
}

/// The arrowhead's length and width, the same for every arrow, and how far the ring around the cell a work is at
/// stands off the cell, where the arrows into and out of the cell end and start.
const HEAD = 7;
const RING = 5;
const RING_CUT = RING + 2;

export type Marks = {
  /// The node whose panel is open.
  selected: string | null;
  /// On a work's page: what the work did at each cell, and how many times it took each arrow.
  overlays: Map<string, Overlay> | null;
  taken: Map<string, number> | null;
  /// When two flows are compared: the cells added and changed.
  added: Set<string>;
  changed: Set<string>;
  /// On a work's page, with a round chosen: the arrows it came in by and left by.
  lit?: Set<string>;
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
  const name = `${cell.id} ${cell.title}`;
  return (
    <g className={cls} data-node={n.id}>
      <title>{marks.overlays && !o ? `${name}: ${t('chart_unvisited')}` : name}</title>
      {o?.current && o.running ? <Halo box={box} round={round} /> : null}
      {o?.current ? (
        <rect
          className={o.running ? 'fc-ring fc-ring-running' : 'fc-ring'}
          x={box.x - RING}
          y={box.y - RING}
          width={box.width + 2 * RING}
          height={box.height + 2 * RING}
          rx={round ? (box.height + 2 * RING) / 2 : 8}
        >
          <title>{`${name}: ${t('chart_current')}${o.running ? `; ${t('chart_running')}` : ''}`}</title>
        </rect>
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
      {o?.current && o.held !== null ? <HeldMark x={box.x - RING - 4} y={box.y - 10} why={o.held} /> : null}
      {badges}
    </g>
  );
}

/// The halo round the cell a round runs at, so that the eye finds it at any zoom: a wide blurred glow whose colour
/// changes, three arcs of colour running round the cell and a ring spreading outwards from it; still, in the same
/// colours, for a browser that asks for less motion (the styles say how it moves). `small` draws it for the legend.
export function Halo({ box, round, small = false }: { box: Box; round: boolean; small?: boolean }) {
  const blur = `fc-halo-blur-${useId().replace(/[^A-Za-z0-9_-]/g, '')}`;
  // How far out each part stands from the cell: the glow, the ring that spreads, the arcs (the legend draws it small).
  const [glow, pulse, arcs] = small ? [5, 2, 4] : [12, RING, RING + 4];
  const at = (pad: number) => ({
    x: box.x - pad,
    y: box.y - pad,
    width: box.width + 2 * pad,
    height: box.height + 2 * pad,
    rx: round ? (box.height + 2 * pad) / 2 : 6 + pad / 2,
  });
  return (
    <g className={small ? 'fc-halo fc-halo-small' : 'fc-halo'} aria-hidden="true">
      <filter id={blur} x="-50%" y="-50%" width="200%" height="200%">
        <feGaussianBlur stdDeviation={small ? 1.5 : 8} />
      </filter>
      <rect className="fc-halo-glow" filter={`url(#${blur})`} {...at(glow)} />
      <rect className="fc-halo-pulse" {...at(pulse)} />
      <rect className="fc-halo-arc fc-halo-a" pathLength={100} {...at(arcs)} />
      <rect className="fc-halo-arc fc-halo-b" pathLength={100} {...at(arcs)} />
      <rect className="fc-halo-arc fc-halo-c" pathLength={100} {...at(arcs)} />
    </g>
  );
}

/// The mark of a work held back at the cell it is at, the next to run and where it goes on: two bars, as a pause is
/// drawn, saying why on hovering.
export function HeldMark({ x, y, why }: { x: number; y: number; why: string }) {
  return (
    <g className="fc-held">
      <title>{why}</title>
      <rect x={x} y={y} width={18} height={18} rx={9} />
      <rect className="fc-held-bar" x={x + 5.5} y={y + 5} width={2.5} height={8} rx={0.5} />
      <rect className="fc-held-bar" x={x + 10} y={y + 5} width={2.5} height={8} rx={0.5} />
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

/// A row number's badge in a label, centred on `x`.
function RowBadge({ x, y, row }: { x: number; y: number; row: number }) {
  return (
    <g className="fc-no">
      <circle cx={x} cy={y} r={BADGE / 2} />
      <text x={x} y={y + 3.5} textAnchor="middle">
        {row + 1}
      </text>
    </g>
  );
}

/// What hovering an arrow says: the rows it draws, and how many times the work took them.
function edgeTitle(e: ChartEdge, cell: CompiledCell | undefined, n: number): string {
  const rows = [...new Set(e.rows.map((i) => cell?.rows[i]?.name ?? ''))].filter((x) => x !== '');
  const what = e.kind === 'shared' ? `${t('chart_shared_title')}: ${rows.join(', ')}` : rows.join(', ');
  return n > 0 ? `${what}\n${t('chart_taken', { n })}` : what;
}

export type DrawProps = {
  g: CompiledGraph;
  chart: Chart;
  laid: Laid;
  lines: Map<string, string[]>;
  marks: Marks;
  /// Whether the labels carry the rows' numbers: the flow's author reads them; a work's chart leaves them out.
  numbered: boolean;
};

/// The chart as SVG, at the size its layout takes.
export function FlowSvg({ g, chart, laid, lines, marks, numbered }: DrawProps) {
  const [hover, setHover] = useState<string | null>(null);
  const cells = new Map(g.cells.map((c) => [c.id, c]));
  const title = (id: string) => {
    const c = cells.get(id);
    return c ? `${c.id} ${c.title}` : id;
  };
  const hot = related(chart.edges, hover);
  const ringed = [...(marks.overlays ?? [])].find(([, o]) => o.current)?.[0];
  const ring = ringed !== undefined ? `c:${ringed}` : null;
  const kindOf = (e: ChartEdge) => {
    const n = marks.taken?.get(e.id) ?? 0;
    if (hot.has(e.id)) return 'hot';
    if (marks.lit?.has(e.id)) return 'lit';
    if (e.kind === 'rule' || e.kind === 'interjection') return 'rule';
    if (n > 0) return 'taken';
    if (marks.taken) return 'quiet';
    return e.main ? 'main' : 'plain';
  };
  // The arrows lit up are drawn last, over the others: the one pointed at over those of the round chosen.
  const over = (e: ChartEdge) => (hot.has(e.id) ? 2 : marks.lit?.has(e.id) ? 1 : 0);
  const edges = [...chart.edges].sort((a, b) => over(a) - over(b));
  const b = laid.bounds;
  const pad = 24;
  return (
    <svg
      className="fc"
      width={b.width + 2 * pad}
      height={b.height + 2 * pad}
      viewBox={`${b.x - pad} ${b.y - pad} ${b.width + 2 * pad} ${b.height + 2 * pad}`}
      onPointerOver={(ev) => {
        const id = (ev.target as Element).closest('[data-edge]')?.getAttribute('data-edge') ?? null;
        if (id !== hover) setHover(id);
      }}
      onPointerLeave={() => setHover(null)}
    >
      <defs>
        {['plain', 'main', 'taken', 'rule', 'quiet', 'hot', 'lit'].map((k) => (
          <marker
            key={k}
            id={`fc-arrow-${k}`}
            className={`fc-head fc-head-${k}`}
            viewBox="0 0 10 10"
            refX="0"
            refY="5"
            markerUnits="userSpaceOnUse"
            markerWidth={HEAD}
            markerHeight={HEAD}
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
      {edges.map((e) => {
        const r = laid.edges.get(e.id);
        if (!r) return null;
        const kind = kindOf(e);
        const cls = ['fc-edge', `fc-edge-${kind}`, e.kind === 'shared' ? 'fc-edge-shared' : ''].join(' ');
        // An arrow ends where its head starts, and at the ring around the cell the work is at, as it starts there.
        const d = pathOf(r, {
          start: e.source === ring ? RING_CUT : 0,
          end: HEAD + (e.target === ring ? RING_CUT : 0),
        });
        return (
          <g key={e.id} data-edge={e.id}>
            <title>{edgeTitle(e, e.cell !== null ? cells.get(e.cell) : undefined, marks.taken?.get(e.id) ?? 0)}</title>
            <path className="fc-hit" d={d} />
            <path className={cls} d={d} markerEnd={`url(#fc-arrow-${kind})`} />
          </g>
        );
      })}
      {chart.edges.map((e) => {
        const r = laid.edges.get(e.id);
        if (!r?.label) return null;
        const n = marks.taken?.get(e.id) ?? 0;
        const l = r.label;
        const text = n > 0 ? `${e.label} ×${n}`.trim() : e.label;
        const nos = numbered ? badgeRow(e.numbers) : { items: [], width: 0 };
        const names = text === '' ? 0 : Math.ceil(measure(text, SMALL));
        const inner = nos.width + (nos.width > 0 && names > 0 ? NAMES_GAP : 0) + names;
        const w = Math.max(l.width, inner + 12);
        const left = l.x + l.width / 2 - inner / 2;
        const mid = l.y + l.height / 2;
        const cls = [
          'fc-label',
          e.main ? 'fc-label-main' : '',
          n > 0 ? 'fc-label-taken' : '',
          hot.has(e.id) ? 'fc-label-hot' : '',
          marks.lit?.has(e.id) ? 'fc-label-lit' : '',
        ].join(' ');
        return (
          <g key={`l:${e.id}`} className={cls} data-edge={e.id}>
            <title>{edgeTitle(e, e.cell !== null ? cells.get(e.cell) : undefined, n)}</title>
            <rect x={l.x + (l.width - w) / 2} y={l.y} width={w} height={l.height} rx={3} />
            {nos.items.map((it, i) =>
              it.row === null ? (
                <text key={i} className="fc-no-dash" x={left + it.x} y={mid + 4} textAnchor="middle">
                  –
                </text>
              ) : (
                <RowBadge key={i} x={left + it.x} y={mid} row={it.row} />
              ),
            )}
            {text !== '' ? (
              <text x={left + inner - names} y={mid + 4}>
                {text}
              </text>
            ) : null}
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
