// How the flowchart is drawn: each node's size and the lines of its text, measured in the fonts the page draws them
// in, and the chart itself once laid out, the arrows as SVG under the nodes, which are the page's own parts: a cell a
// box on the card with its id, its line as a small tag (`engine` muted for an engine's cell, as on the matrix) and its
// title; a fork the block it decides on, or its keys; a gate its `requires`, each key as its file writes it after the
// matrix's square; a tag of what the outside writes with the matrix's sign; an arrow's label the value of its branch,
// where it leaves a fork, and the names of the triggers it stands for. Three kinds of line: the main path thick in the
// text's colour, over the others; an arrow a write runs thin in the muted colour, behind it, drawn only where it was
// routed over the layout (`data.ts`); a way back thin in the way back's purple. An arrow from a tag is dashed; the
// arrow under the pointer is in the accent. The cell chosen is on the accent's tint, the cells that write what it is
// given are edged in the accent, and beside it stand the blocks it is given, each with the kind it is given in as the
// matrix marks it.

import { type ReactNode, useState } from 'react';

import type { Condition, Flow } from '../api';
import { type Chosen, ConditionGlyph, DataGlyphs } from '../Chart';
import { NO_MARK } from '../matrix';
import { Sign } from '../parts';
import { headFacts, keyWords } from '../words';
import { type DataLabel } from './data';
import { type Box, type Laid, pathOf, type Size } from './layout';
import { type Chart, type ChartEdge, type ChartNode, contextOf, litBy, type Outside } from './model';

/// A size the page's tokens give, in pixels: read from the page where it runs, else the size the token has.
function sizeOf(token: string, fallback: number): number {
  if (typeof document === 'undefined') return fallback;
  const n = Number.parseFloat(getComputedStyle(document.documentElement).getPropertyValue(token));
  return Number.isFinite(n) ? n : fallback;
}

/// A font stack the page's tokens give, read from the page where it runs.
function stackOf(token: string, fallback: string): string {
  if (typeof document === 'undefined') return fallback;
  return getComputedStyle(document.documentElement).getPropertyValue(token).trim() || fallback;
}

/// The fonts the chart's text is drawn and measured in, as the page's tokens give them: a cell's id and every name
/// and value in monospace at the small size, a cell's title semibold at the body size, a line's tag at the labels'
/// size.
type Fonts = { id: string; title: string; badge: string; name: string };

let fonts: Fonts | null = null;

function chartFonts(): Fonts {
  if (fonts === null) {
    const sans = stackOf('--font', 'sans-serif');
    const mono = stackOf('--mono', 'monospace');
    const small = sizeOf('--size-small', 12);
    fonts = {
      id: `${small}px ${mono}`,
      title: `600 ${sizeOf('--size-body', 13)}px ${sans}`,
      badge: `${sizeOf('--size-label', 11)}px ${mono}`,
      name: `${small}px ${mono}`,
    };
  }
  return fonts;
}

let canvas: CanvasRenderingContext2D | null | undefined;

/// The width a text takes in a font: measured where the page runs, estimated elsewhere.
export function measure(text: string, font: string): number {
  if (canvas === undefined) {
    try {
      canvas = typeof document === 'undefined' ? null : document.createElement('canvas').getContext('2d');
    } catch {
      canvas = null;
    }
  }
  if (canvas) {
    canvas.font = font;
    return canvas.measureText(text).width;
  }
  const size = Number(/(\d+)px/.exec(font)?.[1] ?? '12');
  return text.length * size * (font.startsWith('600') ? 0.62 : 0.6);
}

/// A text in lines no wider than `wrap` in `font`.
function wrapped(text: string, font: string, wrap: number): string[] {
  const lines: string[] = [];
  let line = '';
  for (const w of text.split(/\s+/).filter((x) => x !== '')) {
    const next = line === '' ? w : `${line} ${w}`;
    if (line !== '' && measure(next, font) > wrap) {
      lines.push(line);
      line = w;
    } else {
      line = next;
    }
  }
  lines.push(line);
  return lines;
}

/// The measures of a node, in pixels, in step with graph.css and the tokens: the room round a cell's text, its head's
/// row (its id and its line's tag) and the room under it, a line of a node's text (`--chart-node-line`), the widest a
/// cell's title runs before it wraps and the least a cell is wide; a tag's height; the matrix's mark and a sign beside
/// a name, and the room after either; a hairline on either side.
const PAD_X = 12;
const PAD_Y = 8;
const HEAD = 16;
const UNDER_HEAD = 2;
const LINE = 18;
const WRAP = 200;
const LEAST = 120;
const TAG = 22;
const GLYPH = 16;
const SIGN = 12;
const GAP = 4;
const EDGES = 3;
/// How far what a chosen cell is given stands from it where nothing else places it.
const BESIDE = 12;

/// What is drawn of each node: its size, and of a cell the lines of its title.
export type Measured = { size: Map<string, Size>; lines: Map<string, string[]> };

/// The width of a tag of what the outside writes: its signs and its name.
function tagWidth(n: ChartNode): number {
  const signs = (n.outside?.people ? 1 : 0) + (n.outside && n.outside.sources.length > 0 ? 1 : 0);
  return Math.ceil(EDGES + PAD_X + signs * (SIGN + GAP) + measure(n.name ?? '', chartFonts().name) + PAD_X + 2);
}

export function sizes(chart: Chart, flow: Flow): Measured {
  const f = chartFonts();
  const cells = new Map(flow.cells.map((c) => [c.id, c]));
  const size = new Map<string, Size>();
  const lines = new Map<string, string[]>();
  for (const n of chart.nodes) {
    if (n.kind === 'cell') {
      const cell = cells.get(n.cell);
      const ls = wrapped(cell?.title ?? '', f.title, WRAP);
      lines.set(n.id, ls);
      const head = measure(n.cell, f.id) + GAP * 2 + measure(cell ? headFacts(cell).by : '', f.badge) + 2 * GAP;
      const width = Math.max(LEAST, ...ls.map((l) => measure(l, f.title)), head);
      size.set(n.id, {
        width: Math.ceil(width + 2 * PAD_X + EDGES + 2),
        height: EDGES + 2 * PAD_Y + HEAD + UNDER_HEAD + ls.length * LINE,
      });
    } else if (n.kind === 'way-in' || n.kind === 'fork') {
      // A fork on one block says the block; any other, each key; a gate, each key after the matrix's square.
      const widths =
        n.kind === 'fork' && n.name !== null
          ? [measure(n.name, f.name)]
          : [
              ...n.when.map((k) => measure(keyWords(k), f.name)),
              ...n.requires.map((k) => GLYPH + GAP + measure(keyWords(k), f.name)),
            ];
      const rows = widths.length;
      size.set(n.id, {
        width: Math.ceil(Math.max(...widths) + 2 * PAD_X + EDGES + 2),
        height: EDGES + 2 * PAD_Y + rows * LINE,
      });
    } else {
      size.set(n.id, { width: tagWidth(n), height: TAG });
    }
  }
  return { size, lines };
}

/// The size of a label that says these words.
export function wordsSize(words: string): Size {
  return { width: Math.ceil(measure(words, chartFonts().name)) + 2 * GAP, height: LINE };
}

/// What an arrow's label says: of a branch of a fork, the branch first; then the names it carries.
export function labelWords(e: ChartEdge): string {
  return [e.branch ?? '', e.label].filter((w) => w !== '').join(' ');
}

/// The size of an arrow's label; none for an arrow without one.
export function labelSize(e: ChartEdge): Size | null {
  const words = labelWords(e);
  return words === '' ? null : wordsSize(words);
}

/// About where what a chosen cell is given stands beside it, worked out before it is drawn, so that the arrows drawn
/// over the chart keep clear of it: its tags one under another, each with the matrix's mark, its name and its signs.
export function contextBox(flow: Flow, cell: string, at: Box): Box | null {
  const tags = contextOf(flow, cell);
  if (tags.length === 0) return null;
  const f = chartFonts();
  const widths = tags.map(
    (t) =>
      GLYPH + 2 * GAP + measure(t.block, f.name) + ((t.outside?.people ? 1 : 0) + (t.outside?.sources.length ? 1 : 0)) * (SIGN + GAP),
  );
  return {
    x: at.x + at.width + BESIDE,
    y: at.y,
    width: Math.ceil(Math.max(...widths) + 4 * GAP + EDGES),
    height: tags.length * LINE + 2 * GAP + EDGES,
  };
}

/// The kinds of line: the main path, an arrow a write runs (and the way into a cell the main path does not go
/// through), a way back, and an arrow from a tag of what the outside writes.
type Line = 'main' | 'data' | 'back' | 'outside';

function lineOf(e: ChartEdge): Line {
  if (e.kind === 'outside') return 'outside';
  if (e.main) return 'main';
  return e.back ? 'back' : 'data';
}

/// The arrowhead's length: the main path's larger, as its line is thicker.
const HEAD_LENGTH = 7;
const MAIN_HEAD_LENGTH = 10;

/// The heads, one for each kind of line and for the line under the pointer, the main path's its own.
const HEADS = ['main', 'data', 'back', 'outside', 'pointed', 'pointed-main'] as const;
type Head = (typeof HEADS)[number];

function headOf(line: Line, pointed: boolean): Head {
  if (pointed) return line === 'main' ? 'pointed-main' : 'pointed';
  return line;
}

const headLength = (head: Head) => (head === 'main' || head === 'pointed-main' ? MAIN_HEAD_LENGTH : HEAD_LENGTH);

/// The order the lines are drawn in, the later over the earlier: the arrows behind, the main path over them.
const DEPTH: Record<Line, number> = { data: 0, outside: 0, back: 1, main: 2 };

/// What an arrow is said to be on the pointer's tip: the cells it joins and the names it carries; of a step, those of
/// each arrow folded into it.
function said(e: ChartEdge): string {
  const each = e.kind === 'step' ? e.folds : [e];
  const lines = each.map((a) => `${a.from} → ${a.reaches.length > 0 ? a.reaches.join(', ') : a.to} ${a.label}`);
  return [...(e.branch !== null ? [`${e.to} on ${e.branch}`] : []), ...lines].join('\n');
}

/// What choosing a name an arrow carries opens: a group's panel or a block's, as on the matrix.
function chosenOf(name: string, group: boolean): Chosen {
  return group ? { group: name } : { block: name };
}

/// Whether two choices are the same.
function same(a: Chosen, b: Chosen): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

/// The signs of who writes a block besides the cells, as the matrix's names carry them.
function Signs({ outside }: { outside: Outside | null }) {
  if (outside === null) return null;
  return (
    <>
      {outside.people && <Sign name="people" />}
      {outside.sources.length > 0 && (
        <span className="graph-sign" title={`outside ${outside.sources.join(', ')}`}>
          <Sign name="outside" />
        </span>
      )}
    </>
  );
}

export type SheetProps = {
  flow: Flow;
  chart: Chart;
  laid: Laid;
  measured: Measured;
  /// The room round the layout, in pixels.
  pad: number;
  /// The labels of the arrows a write runs that are drawn over the layout, whose routes `laid` holds.
  dataLabels: DataLabel[];
  /// Where what the chosen cell is given stands, in the layout's pixels; none for beside the cell at its right.
  aside: { x: number; y: number } | null;
  chosen: Chosen;
  onChoose: (chosen: Chosen) => void;
};

/// The chart, at the size its layout takes with `pad` round it: every line `laid` has a route for.
export function Sheet({ flow, chart, laid, measured, pad, dataLabels, aside, chosen, onChoose }: SheetProps) {
  const [pointed, setPointed] = useState<string | null>(null);
  const cells = new Map(flow.cells.map((c) => [c.id, c]));
  const chosenCell = chosen && 'cell' in chosen ? chosen.cell : null;
  const lit = chosenCell !== null ? litBy(flow, chosenCell) : new Set<string>();
  const width = laid.bounds.width + 2 * pad;
  const height = laid.bounds.height + 2 * pad;
  // A choice made again lets go of it, as on the matrix.
  const choose = (c: Chosen) => onChoose(same(c, chosen) ? null : c);
  const at = (id: string) => {
    const b = laid.nodes.get(id);
    return b ? { left: b.x + pad, top: b.y + pad, width: b.width, height: b.height } : null;
  };
  // The arrows a write runs are drawn only where they were routed over the layout (`data.ts`).
  const drawn = chart.edges.filter((e) => e.kind !== 'tie' && laid.edges.has(e.id));
  // The main path over the other lines, and the arrow under the pointer over them all.
  const depth = (e: ChartEdge) => (e.id === pointed ? 3 : DEPTH[lineOf(e)]);
  const ordered = [...drawn].sort((a, b) => depth(a) - depth(b));
  // An arrow takes the pointer and opens what it carries: a write's arrow and a step of the main path.
  const named = (e: ChartEdge) => e.kind === 'arrow' || e.kind === 'step';

  const nodeOf = (n: ChartNode): ReactNode => {
    const box = at(n.id);
    if (box === null) return null;
    if (n.kind === 'cell') {
      const cell = cells.get(n.cell);
      if (!cell) return null;
      const facts = headFacts(cell);
      const isChosen = chosenCell === cell.id;
      const name = [`${cell.id} ${cell.title}`, facts.by, facts.when, facts.waits].filter((x) => x !== null).join(', ');
      return (
        <button
          key={n.id}
          type="button"
          className={['graph-node graph-cell', isChosen ? 'chosen' : '', lit.has(cell.id) ? 'lit' : ''].join(' ')}
          style={box}
          data-node={n.id}
          aria-pressed={isChosen}
          aria-label={name}
          title={name}
          onClick={() => choose({ cell: cell.id })}
        >
          <span className="graph-cell-head">
            <span className="cell-id">{cell.id}</span>
            <span className={`graph-badge ${cell.kind === 'agent' ? 'line-name' : ''}`}>{facts.by}</span>
          </span>
          {(measured.lines.get(n.id) ?? [cell.title]).map((l, i) => (
            <span key={i} className="graph-cell-title">
              {l}
            </span>
          ))}
        </button>
      );
    }
    if (n.kind === 'fork' && n.name !== null) {
      // A fork on one block: the block, which opens its panel; its values are on its branches.
      const c: Chosen = { block: n.name };
      return (
        <div key={n.id} className="graph-node graph-way-in graph-fork" style={box} data-node={n.id}>
          <button
            type="button"
            className="graph-row"
            aria-pressed={same(c, chosen)}
            aria-label={`fork on ${n.name}`}
            title={n.when.map((k) => `${n.cell} when ${keyWords(k)}`).join('\n')}
            onClick={() => choose(c)}
          >
            <span className="graph-row-key">{n.name}</span>
          </button>
        </div>
      );
    }
    if (n.kind === 'way-in' || n.kind === 'fork') {
      const row = (key: Condition, requires: boolean, i: number) => {
        const c: Chosen = key.on === 'block' ? { block: key.block } : { test: key.test.name };
        const words = keyWords(key);
        return (
          <button
            key={`${requires ? 'r' : 'w'}${i}`}
            type="button"
            className="graph-row"
            aria-pressed={same(c, chosen)}
            aria-label={`${n.cell} ${requires ? 'requires' : 'when'} ${words}`}
            title={`${requires ? 'requires' : 'when'} ${words}`}
            onClick={() => choose(c)}
          >
            {requires && <ConditionGlyph spot={{ when: false, requires: true }} />}
            <span className="graph-row-key">{words}</span>
          </button>
        );
      };
      return (
        <div
          key={n.id}
          className={`graph-node graph-way-in ${n.kind === 'fork' ? 'graph-fork' : ''}`}
          style={box}
          data-node={n.id}
        >
          {n.when.map((k, i) => row(k, false, i))}
          {n.requires.map((k, i) => row(k, true, i))}
        </div>
      );
    }
    const c = chosenOf(n.name ?? '', n.group);
    return (
      <button
        key={n.id}
        type="button"
        className="graph-node graph-tag"
        style={box}
        data-node={n.id}
        aria-pressed={same(c, chosen)}
        aria-label={`${n.name} into ${n.cell}`}
        onClick={() => choose(c)}
      >
        <Signs outside={n.outside} />
        <span className="graph-tag-name">{n.name}</span>
      </button>
    );
  };

  // The blocks the chosen cell is given, beside it at its right.
  const context = (() => {
    if (chosenCell === null) return null;
    const box = at(`c:${chosenCell}`);
    const tags = contextOf(flow, chosenCell);
    if (box === null || tags.length === 0) return null;
    return (
      <div
        className="graph-context"
        style={aside !== null ? { left: aside.x + pad, top: aside.y + pad } : { left: box.left + box.width + BESIDE, top: box.top }}
        role="group"
        aria-label={`${chosenCell} context`}
      >
        {tags.map((t) => (
          <button
            key={t.block}
            type="button"
            className="graph-context-tag"
            onClick={() => choose({ block: t.block })}
          >
            <DataGlyphs
              spot={{
                mark: { ...NO_MARK, given: true, kind: t.kind },
                line: 'none',
                back: 'none',
                backTo: false,
              }}
            />
            <span className="graph-context-name">{t.block}</span>
            <Signs outside={t.outside} />
          </button>
        ))}
      </div>
    );
  })();

  return (
    <div
      className="graph-sheet"
      style={{ width, height }}
      onPointerOver={(ev) => {
        const id = (ev.target as Element).closest('[data-edge]')?.getAttribute('data-edge') ?? null;
        if (id !== pointed) setPointed(id);
      }}
      onPointerLeave={() => setPointed(null)}
    >
      <svg className="graph-lines" width={width} height={height} viewBox={`${-pad} ${-pad} ${width} ${height}`}>
        <defs>
          {HEADS.map((k) => (
            <marker
              key={k}
              id={`graph-head-${k}`}
              className={`graph-head graph-head-${k}`}
              viewBox="0 0 10 10"
              refX="0"
              refY="5"
              markerUnits="userSpaceOnUse"
              markerWidth={headLength(k)}
              markerHeight={headLength(k)}
              orient="auto-start-reverse"
            >
              <path d="M 0 0 L 10 5 L 0 10 z" />
            </marker>
          ))}
        </defs>
        {ordered.map((e) => {
          const r = laid.edges.get(e.id);
          if (!r) return null;
          const on = pointed === e.id;
          const line = lineOf(e);
          const head = headOf(line, on);
          const d = pathOf(r, { end: headLength(head) });
          return (
            <g key={e.id} data-edge={named(e) ? e.id : undefined} data-line={line}>
              {named(e) && <path className="graph-hit" d={d} />}
              <path
                className={`graph-edge graph-edge-${line} ${on ? 'pointed' : ''}`}
                d={d}
                markerEnd={`url(#graph-head-${head})`}
              />
            </g>
          );
        })}
      </svg>
      {drawn.map((e) => {
        const l = laid.edges.get(e.id)?.label;
        if (!l || (e.carries.length === 0 && e.branch === null)) return null;
        // The arrow each name is of: of a step, the arrow folded into it that carries the name.
        const of = (name: string) => e.folds.find((f) => f.carries.some((c) => c.name === name)) ?? e;
        return (
          <div
            key={`l:${e.id}`}
            className={`graph-label ${lineOf(e)} ${pointed === e.id ? 'pointed' : ''}`}
            style={{ left: l.x + pad, top: l.y + pad, width: l.width, height: l.height }}
            data-edge={e.id}
            title={said(e)}
          >
            {e.branch !== null && (
              <span className="graph-branch">
                {e.branch}
                {e.carries.length > 0 && ' '}
              </span>
            )}
            {e.carries.map((c, i) => (
              <span key={c.name}>
                {i > 0 && ', '}
                <button
                  type="button"
                  className="graph-name"
                  aria-pressed={same(chosenOf(c.name, c.group), chosen)}
                  aria-label={`${c.group ? 'Group' : 'Block'} ${c.name}, ${of(c.name).from} to ${of(c.name).to}`}
                  onClick={() => choose(chosenOf(c.name, c.group))}
                >
                  {c.name}
                </button>
              </span>
            ))}
          </div>
        );
      })}
      {dataLabels.map((l) => (
        <div
          key={l.id}
          className={`graph-label data ${l.arrows.some((a) => a.id === pointed) ? 'pointed' : ''}`}
          style={{ left: l.box.x + pad, top: l.box.y + pad, width: l.box.width, height: l.box.height }}
          data-edge={l.arrows[0].id}
          title={l.arrows.map((a) => `${a.from} → ${a.to} ${a.label}`).join('\n')}
        >
          {l.carries.map((c, i) => (
            <span key={c.name}>
              {i > 0 && ', '}
              <button
                type="button"
                className="graph-name"
                aria-pressed={same(chosenOf(c.name, c.group), chosen)}
                aria-label={`${c.group ? 'Group' : 'Block'} ${c.name}, ${l.arrows[0].from} to ${l.arrows
                  .map((a) => a.to)
                  .join(', ')}`}
                onClick={() => choose(chosenOf(c.name, c.group))}
              >
                {c.name}
              </button>
            </span>
          ))}
        </div>
      ))}
      {chart.nodes.map(nodeOf)}
      {context}
    </div>
  );
}
