// How a flow's chart is drawn: each node's size and the lines of its text, measured in the font the page draws
// them in, and the chart itself as SVG once laid out. Who does a cell's work decides its colours: an agent's round
// takes its line's, a cell on no line is white and dotted, the engine's own step is purple, dashed and rounded, a tool
// the engine runs grey and dashed, the owner orange and rounded. Under a step the engine decides, what its values read;
// on a cell that lacks a move most cells that could make make, a small mark saying so. An agent's cell carries over its
// top left corner the reasoning effort it runs at, as a badge in the level's colour. What the engine does on a line or
// every time a cell returns is a purple hexagon. A row's number is a badge of one size with its digits in it.
// Hovering an arrow or its label lights up the whole row, every arrow that draws it from its source to its target.

import { type ReactNode, useRef, useState } from 'react';

import type { CompiledCell, CompiledGraph, EngineNames } from '../api';
import { t } from '../text';
import { actionWords, type Chart, type ChartEdge, type ChartNode, type Overlay, related, type Wait } from './flowModel';
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

/// The widest a cell's text runs before it wraps, and the height of a line of it and of a small line under it.
const WRAP = 230;
const LINE = 18;
const SMALL_LINE = 15;
const PAD_X = 16;
const PAD_Y = 11;
/// The height of the row of marks of the moves a cell lacks.
const MARKS = 20;

/// A text in lines no wider than `wrap` in `font`, the first starting `first` in.
function wrapped(text: string, font: string, wrap: number, first = 0): string[] {
  const lines: string[] = [];
  let line = '';
  let width = first;
  for (const w of text.split(/\s+/).filter((x) => x !== '')) {
    const add = measure(line === '' ? w : ` ${w}`, font);
    if (line !== '' && width + add > wrap) {
      lines.push(line);
      line = w;
      width = measure(w, font);
    } else {
      line = line === '' ? w : `${line} ${w}`;
      width += add;
    }
  }
  lines.push(line);
  return lines;
}

/// A cell's text, `<id> <title>`, in lines no wider than the cell's text may be; the id leads the first line.
export function cellLines(c: CompiledCell): string[] {
  return wrapped(c.title, FONT, WRAP, measure(`${c.id} `, BOLD));
}

/// The reasoning efforts an agent's cell may run at, low to high, as the flow's compiler allows them (`EFFORTS` in
/// `crates/flow/src/format.rs`), each with the short form its badge on the chart shows.
export const EFFORTS: ReadonlyArray<readonly [string, string]> = [
  ['low', 'low'],
  ['medium', 'med'],
  ['high', 'high'],
  ['xhigh', 'xhigh'],
  ['max', 'max'],
];

/// The badge of the reasoning effort a cell runs at: its level, the short form it shows, the class that gives it the
/// level's colour (a grey one for a level the page does not know), and the whole word on hovering.
export type Effort = { level: string; text: string; cls: string; title: string };

/// A cell's effort badge; none for a cell that declares no effort, as an engine's, a tool's or the owner's.
export function effortOf(c: CompiledCell): Effort | null {
  const level = c.effort;
  if (c.kind !== 'agent' || level == null || level === '') return null;
  const known = EFFORTS.find(([l]) => l === level);
  return {
    level,
    text: known !== undefined ? known[1] : level,
    cls: known !== undefined ? `fc-effort-${level}` : 'fc-effort-other',
    title: t('chart_effort', { effort: level }),
  };
}

/// The efforts the cells run at, each once, low to high, a level the page does not know last.
export function effortsOf(cells: CompiledCell[]): Effort[] {
  const rank = (e: Effort) => {
    const i = EFFORTS.findIndex(([l]) => l === e.level);
    return i < 0 ? EFFORTS.length : i;
  };
  const all = new Map<string, Effort>();
  for (const c of cells) {
    const e = effortOf(c);
    if (e !== null && !all.has(e.level)) all.set(e.level, e);
  }
  return [...all.values()].sort((a, b) => rank(a) - rank(b));
}

const EFFORT_FONT = '700 10px ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif';
const EFFORT_HEIGHT = 16;

/// The width of every effort badge, that of the widest short form, so that the badges are of one size.
export function effortWidth(): number {
  return Math.ceil(Math.max(...EFFORTS.map(([, s]) => measure(s, EFFORT_FONT)))) + 12;
}

/// An effort badge, its top left corner at `x`, `y`, `width` wide: the level's short form, the whole word on hovering.
export function EffortBadge({ effort, x, y, width }: { effort: Effort; x: number; y: number; width: number }) {
  return (
    <g className={`fc-effort ${effort.cls}`}>
      <title>{effort.title}</title>
      <rect x={x} y={y} width={width} height={EFFORT_HEIGHT} rx={EFFORT_HEIGHT / 2} />
      <text x={x + width / 2} y={y + 11.5} textAnchor="middle">
        {effort.text}
      </text>
    </g>
  );
}

/// A mark on a cell that lacks a move most cells that could make it make: what it says, and why on hovering.
export type Lack = { text: string; title: string };

/// What a cell's box says under its title: what the values its step decides by read, in lines, and the moves it lacks.
export type CellNote = { reads: string[]; lacks: Lack[] };

const CIRCLE = 36;
/// The room a hexagon's points take on each side of its text.
const POINT = 12;

/// Each node's size, each cell's lines and those of the actions a node names, and what each cell's box says under
/// its title.
export function sizes(
  g: CompiledGraph,
  c: Chart,
): { size: Map<string, Size>; lines: Map<string, string[]>; notes: Map<string, CellNote> } {
  const cells = new Map(g.cells.map((x) => [x.id, x]));
  const size = new Map<string, Size>();
  const lines = new Map<string, string[]>();
  const notes = new Map<string, CellNote>();
  for (const n of c.nodes) {
    const cell = cells.get(n.cell);
    if (n.kind === 'cell' && cell) {
      const ls = cellLines(cell);
      lines.set(n.id, ls);
      const about = c.notes.get(cell.id);
      const note: CellNote = {
        reads: about?.reads != null ? wrapped(about.reads, SMALL, WRAP) : [],
        lacks: (about?.lacks ?? []).map((m) => ({
          text: t('chart_lacks', { move: actionWords(m.row) }),
          title: t('chart_lacks_title', { cell: cell.id, move: actionWords(m.row), to: m.to }),
        })),
      };
      if (note.reads.length > 0 || note.lacks.length > 0) notes.set(n.id, note);
      const widths = [
        ...ls.map((l, i) => measure(l) + (i === 0 ? measure(`${cell.id} `, BOLD) : 0)),
        ...note.reads.map((l) => measure(l, SMALL)),
        note.lacks.reduce((w, l) => w + measure(l.text, SMALL) + 14, 0),
      ];
      const width = Math.max(110, Math.ceil(Math.max(...widths)) + 2 * PAD_X + (cell.kind === 'agent' ? 0 : 14));
      const height = ls.length * LINE + note.reads.length * SMALL_LINE + (note.lacks.length > 0 ? MARKS : 0) + 2 * PAD_Y;
      size.set(n.id, { width, height });
    } else if (n.kind === 'action' || n.kind === 'does') {
      const ls = n.actions.map(actionWords);
      lines.set(n.id, ls);
      const width = Math.ceil(Math.max(...ls.map((l) => measure(l, SMALL)))) + 2 * POINT + 8;
      size.set(n.id, { width, height: ls.length * SMALL_LINE + 12 });
    } else if (n.kind === 'out' || n.kind === 'in') {
      size.set(n.id, { width: Math.max(CIRCLE, Math.ceil(measure(n.other ?? '', SMALL)) + 14), height: CIRCLE });
    } else if (n.kind === 'rule') {
      size.set(n.id, { width: Math.ceil(measure(`rule:${n.other ?? ''}`, SMALL)) + 40, height: 28 });
    } else {
      size.set(n.id, { width: Math.ceil(measure(g.interject?.name ?? '', SMALL)) + 44, height: 28 });
    }
  }
  return { size, lines, notes };
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

/// The arrowhead's length and width, the same for every arrow but those lit for the round chosen, whose wide line
/// takes a head sized to it; and how far the ring around the cell a work is at stands off the cell, where the arrows
/// into and out of the cell end and start.
const HEAD = 7;
const LIT_HEAD = 16;
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
  /// On a work's page: the cell the halo goes round, where a round runs or has just run (`haloAt`).
  halo?: string | null;
};

function CellShape({ n, cell, box, lines, note, marks }: {
  n: ChartNode;
  cell: CompiledCell;
  box: Box;
  lines: string[];
  note: CellNote | undefined;
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
  // The effort badge over the top left corner, centred on the line the work's badges at the top right sit on; a
  // comparison's mark goes after it.
  const effort = effortOf(cell);
  const effortW = effort !== null ? effortWidth() : 0;
  const markX = effort !== null ? box.x - 4 + effortW + 4 : box.x + 4;
  const name = `${cell.id} ${cell.title}`;
  return (
    <g className={cls} data-node={n.id}>
      <title>{marks.overlays && !o ? `${name}: ${t('chart_unvisited')}` : name}</title>
      {marks.halo === cell.id ? <Halo box={box} round={round} /> : null}
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
      {note !== undefined && note.reads.length > 0 ? (
        <text className="fc-reads" x={box.x + box.width / 2} y={textTop + lines.length * LINE - 2} textAnchor="middle">
          <title>{t('chart_reads_title')}</title>
          {note.reads.map((l, i) => (
            <tspan key={i} x={box.x + box.width / 2} dy={i === 0 ? 0 : SMALL_LINE}>
              {l}
            </tspan>
          ))}
        </text>
      ) : null}
      {note !== undefined && note.lacks.length > 0 ? (
        <Lacks lacks={note.lacks} x={box.x + box.width / 2} y={box.y + box.height - PAD_Y - MARKS + 3} />
      ) : null}
      {effort !== null ? <EffortBadge effort={effort} x={box.x - 4} y={box.y - 9} width={effortW} /> : null}
      {mark !== null ? (
        <text className="fc-mark" x={markX} y={box.y - 4}>
          {mark}
        </text>
      ) : null}
      {o?.current && o.waits !== null ? <WaitTag right={box.x - RING - 3} y={box.y - 10} wait={o.waits} /> : null}
      {badges}
    </g>
  );
}

/// The marks of the moves a cell lacks, in a row centred on `x`, each saying why on hovering.
function Lacks({ lacks, x, y }: { lacks: Lack[]; x: number; y: number }) {
  const widths = lacks.map((l) => Math.ceil(measure(l.text, SMALL)) + 10);
  const total = widths.reduce((a, w) => a + w, 0) + 4 * (lacks.length - 1);
  let at = x - total / 2;
  return (
    <>
      {lacks.map((l, i) => {
        const left = at;
        at += widths[i] + 4;
        return (
          <g key={l.text} className="fc-lack">
            <title>{l.title}</title>
            <rect x={left} y={y} width={widths[i]} height={16} rx={8} />
            <text x={left + widths[i] / 2} y={y + 12} textAnchor="middle">
              {l.text}
            </text>
          </g>
        );
      })}
    </>
  );
}

/// The halo round the cell a round runs at, so that the eye finds it at any zoom: three arcs of colour running round
/// the cell; still, in the same colours, for a browser that asks for less motion (the styles say how it moves).
/// `small` draws it for the legend.
export function Halo({ box, round, small = false }: { box: Box; round: boolean; small?: boolean }) {
  // How far out the arcs stand from the cell (the legend draws them small).
  const pad = small ? 4 : RING + 4;
  const at = {
    x: box.x - pad,
    y: box.y - pad,
    width: box.width + 2 * pad,
    height: box.height + 2 * pad,
    rx: round ? (box.height + 2 * pad) / 2 : 6 + pad / 2,
  };
  return (
    <g className={small ? 'fc-halo fc-halo-small' : 'fc-halo'} aria-hidden="true">
      <rect className="fc-halo-arc fc-halo-a" pathLength={100} {...at} />
      <rect className="fc-halo-arc fc-halo-b" pathLength={100} {...at} />
      <rect className="fc-halo-arc fc-halo-c" pathLength={100} {...at} />
    </g>
  );
}

/// The tag beside the cell a work is at while no round of it runs there, its right edge at `right`: why it waits, in a
/// few words, still, and in full on hovering.
export function WaitTag({ right, y, wait }: { right: number; y: number; wait: Wait }) {
  const width = Math.ceil(measure(wait.tag, SMALL)) + 12;
  return (
    <g className="fc-wait">
      <title>{wait.why}</title>
      <rect x={right - width} y={y} width={width} height={18} rx={9} />
      <text x={right - width / 2} y={y + 13} textAnchor="middle">
        {wait.tag}
      </text>
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

/// What the engine does on a line, or every time a cell returns, as a hexagon naming each action on a line of its own,
/// what each does on hovering.
function Actions({ n, box, lines, title, selected }: { n: ChartNode; box: Box; lines: string[]; title: string; selected: boolean }) {
  const p = Math.min(POINT, box.height / 2);
  const pts = [
    [box.x + p, box.y],
    [box.x + box.width - p, box.y],
    [box.x + box.width, box.y + box.height / 2],
    [box.x + box.width - p, box.y + box.height],
    [box.x + p, box.y + box.height],
    [box.x, box.y + box.height / 2],
  ];
  const top = box.y + box.height / 2 - ((lines.length - 1) * SMALL_LINE) / 2 + 4;
  return (
    <g className={`fc-act${n.kind === 'does' ? ' fc-does' : ''}${selected ? ' fc-selected' : ''}`} data-node={n.id}>
      <title>{title}</title>
      <polygon points={pts.map((q) => q.join(',')).join(' ')} />
      <text x={box.x + box.width / 2} y={top} textAnchor="middle">
        {lines.map((l, i) => (
          <tspan key={i} x={box.x + box.width / 2} dy={i === 0 ? 0 : SMALL_LINE}>
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

/// What hovering an arrow says: the rows it draws, each with its cell for the arrow out of the actions on a line, and
/// how many times the work took them.
function edgeTitle(e: ChartEdge, cells: Map<string, CompiledCell>, n: number): string {
  const cell = e.cell !== null ? cells.get(e.cell) : undefined;
  const rows = [
    ...new Set([
      ...e.rows.map((i) => cell?.rows[i]?.name ?? ''),
      ...e.through.map((x) => `${x.cell} ${cells.get(x.cell)?.rows[x.row]?.name ?? ''}`),
    ]),
  ].filter((x) => x !== '');
  const what = rows.join(', ');
  return n > 0 ? `${what}\n${t('chart_taken', { n })}` : what;
}

/// What hovering the actions a node names says: each action and what it does.
export function actionsTitle(actions: string[], engine: EngineNames | null): string {
  return actions.map((a) => `${actionWords(a)}: ${engine?.actions[a] ?? a}`).join('\n');
}

export type DrawProps = {
  g: CompiledGraph;
  chart: Chart;
  laid: Laid;
  lines: Map<string, string[]>;
  notes: Map<string, CellNote>;
  /// The flow's engine part, which says what each action does.
  engine: EngineNames | null;
  marks: Marks;
  /// Whether the labels carry the rows' numbers: the flow's author reads them; a work's chart leaves them out.
  numbered: boolean;
  /// Whether this drawing defines the arrowheads every drawing on the page refers to: the first of the graphs drawn
  /// side by side does, so that an id is defined once.
  defs?: boolean;
  /// Told the node under the pointer, as it changes, and none once the pointer leaves the nodes.
  onNode?: (node: string | null) => void;
};

/// The chart as SVG, at the size its layout takes.
export function FlowSvg({ g, chart, laid, lines, notes, engine, marks, numbered, defs = true, onNode }: DrawProps) {
  const [hover, setHover] = useState<string | null>(null);
  const pointed = useRef<string | null>(null);
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
    if (e.kind === 'rule' || e.kind === 'interjection' || e.kind === 'does') return 'rule';
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
        const under = ev.target as Element;
        const id = under.closest('[data-edge]')?.getAttribute('data-edge') ?? null;
        if (id !== hover) setHover(id);
        const node = under.closest('[data-node]')?.getAttribute('data-node') ?? null;
        if (node !== pointed.current) {
          pointed.current = node;
          onNode?.(node);
        }
      }}
      onPointerLeave={() => {
        setHover(null);
        if (pointed.current !== null) {
          pointed.current = null;
          onNode?.(null);
        }
      }}
    >
      {defs ? (
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
              markerWidth={k === 'lit' ? LIT_HEAD : HEAD}
              markerHeight={k === 'lit' ? LIT_HEAD : HEAD}
              orient="auto-start-reverse"
            >
              <path d="M 0 0 L 10 5 L 0 10 z" />
            </marker>
          ))}
        </defs>
      ) : null}
      {chart.groups.map((gr) => {
        const box = laid.groups.get(gr.id);
        if (!box) return null;
        return (
          <g key={gr.id} className="fc-group">
            <rect x={box.x} y={box.y} width={box.width} height={box.height} rx={4} />
            <text x={box.x + 12} y={box.y + 21}>
              {gr.id === 'asking'
                ? t(gr.letter ? 'chart_asking_part_letter' : 'chart_asking_part', { letter: gr.letter })
                : t(gr.letter ? 'chart_stage_letter' : 'chart_stage', { stage: gr.label, letter: gr.letter })}
            </text>
          </g>
        );
      })}
      {edges.map((e) => {
        const r = laid.edges.get(e.id);
        if (!r) return null;
        const kind = kindOf(e);
        const cls = ['fc-edge', `fc-edge-${kind}`, e.kind === 'does' ? 'fc-edge-does' : ''].join(' ');
        // An arrow ends where its head starts, and at the ring around the cell the work is at, as it starts there.
        const d = pathOf(r, {
          start: e.source === ring ? RING_CUT : 0,
          end: (e.kind === 'does' ? 0 : kind === 'lit' ? LIT_HEAD : HEAD) + (e.target === ring ? RING_CUT : 0),
        });
        return (
          <g key={e.id} data-edge={e.id}>
            <title>{edgeTitle(e, cells, marks.taken?.get(e.id) ?? 0)}</title>
            <path className="fc-hit" d={d} />
            <path className={cls} d={d} markerEnd={e.kind === 'does' ? undefined : `url(#fc-arrow-${kind})`} />
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
            <title>{edgeTitle(e, cells, n)}</title>
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
              <CellShape
                key={n.id}
                n={n}
                cell={cell}
                box={box}
                lines={lines.get(n.id) ?? []}
                note={notes.get(n.id)}
                marks={marks}
              />
            ) : null;
          case 'action':
          case 'does':
            return (
              <Actions
                key={n.id}
                n={n}
                box={box}
                lines={lines.get(n.id) ?? []}
                title={actionsTitle(n.actions, engine)}
                selected={marks.selected === n.id}
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
