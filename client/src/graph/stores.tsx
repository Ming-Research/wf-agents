// The places the engine keeps what it writes and a cell reads later, as the chart draws them beside the graphs: in
// three boxes by what they belong to (the project, an item, a work and its rounds), a computed one dashed; and the
// lines from a node of a graph to the stores it reads or writes, drawn only for what is pointed at or open, since every
// cell reads some and they would cover the chart. A store's reads come from the cells' inputs and the values their rows
// switch on, its writes from the table of what each action and step writes.

import type { CompiledCell, CompiledGraph, EngineNames, Layer, Store } from '../api';
import { t } from '../text';
import { chartFonts, measure } from './flowDraw';
import type { Box, Size } from './flowLayout';
import { type Chart, rowActions, valuesOf } from './flowModel';

/// The boxes the stores are drawn in, top to bottom: the project's, an item's, and a work's with its rounds'.
export const BOXES: Array<{ id: 'project' | 'item' | 'work'; layers: Layer[] }> = [
  { id: 'project', layers: ['project'] },
  { id: 'item', layers: ['item'] },
  { id: 'work', layers: ['work', 'round'] },
];

/// The title of a box of stores.
export function boxTitle(id: string): string {
  return t(id === 'project' ? 'stores_project' : id === 'item' ? 'stores_item' : 'stores_work');
}

/// A line between a node of a graph and a store: the node reads it, or writes it.
export type StoreLink = { node: string; store: string; write: boolean };

/// The stores a cell reads: what its inputs read, what the values its rows switch on read, and what its step reads.
export function readsOf(engine: EngineNames, cell: CompiledCell): string[] {
  const stores = engine.stores ?? {};
  const out: string[] = [];
  const add = (s: string) => {
    if (s in stores && !out.includes(s)) out.push(s);
  };
  for (const x of cell.inputs) {
    for (const src of x.from) {
      if (typeof src !== 'object' || Array.isArray(src)) continue;
      for (const [name, s] of Object.entries(stores)) {
        if ('db' in src && (s.relations ?? []).includes(src.db)) add(name);
        if ('engine' in src && (s.inputs ?? []).includes(src.engine)) add(name);
      }
    }
  }
  for (const v of valuesOf(cell, new Set(Object.keys(engine.values)))) (engine.reads?.[v]?.stores ?? []).forEach(add);
  if (cell.step != null) (engine.reads?.[cell.step]?.stores ?? []).forEach(add);
  return out;
}

/// The stores the actions or steps named write, as the engine's table says.
export function writesOf(engine: EngineNames, names: string[]): string[] {
  const out: string[] = [];
  for (const n of names) for (const s of engine.writes?.[n] ?? []) if (!out.includes(s)) out.push(s);
  return out;
}

/// Which node of a graph's chart reads or writes each store: a cell reads what `readsOf` says and writes what its step
/// writes; the actions on a line, and those a cell does every time, write what the table says each action writes.
export function storeLinks(engine: EngineNames, g: CompiledGraph, c: Chart): StoreLink[] {
  const cells = new Map(g.cells.map((x) => [x.id, x]));
  const out: StoreLink[] = [];
  for (const n of c.nodes) {
    if (n.kind === 'cell') {
      const cell = cells.get(n.cell);
      if (cell === undefined) continue;
      for (const s of readsOf(engine, cell)) out.push({ node: n.id, store: s, write: false });
      if (cell.step != null) for (const s of writesOf(engine, [cell.step])) out.push({ node: n.id, store: s, write: true });
    } else if (n.kind === 'action' || n.kind === 'does') {
      for (const s of writesOf(engine, n.actions)) out.push({ node: n.id, store: s, write: true });
    }
  }
  return out;
}

/// What writes a store in a graph, and what reads it: by each action or step that writes it, the cells (and the rows,
/// for an action on a row) that carry it out; the cells that read it.
export function storeUses(
  engine: EngineNames,
  g: CompiledGraph,
  store: string,
): { writes: Array<{ what: string; step: boolean; at: Array<{ cell: string; row: string | null }> }>; reads: string[] } {
  const writes: Array<{ what: string; step: boolean; at: Array<{ cell: string; row: string | null }> }> = [];
  const add = (what: string, step: boolean, cell: string, row: string | null) => {
    let w = writes.find((x) => x.what === what && x.step === step);
    if (w === undefined) {
      w = { what, step, at: [] };
      writes.push(w);
    }
    if (!w.at.some((x) => x.cell === cell && x.row === row)) w.at.push({ cell, row });
  };
  const writing = (name: string) => (engine.writes?.[name] ?? []).includes(store);
  for (const c of g.cells) {
    for (const r of c.rows) for (const a of rowActions(r)) if (writing(a)) add(a, false, c.id, r.name);
    for (const a of [...(c.actions ?? []), ...(c.always ?? []).flatMap((x) => (x.do != null ? [x.do] : []))]) {
      if (writing(a)) add(a, false, c.id, null);
    }
    if (c.step != null && writing(c.step)) add(c.step, true, c.id, null);
  }
  const reads = g.cells.filter((c) => readsOf(engine, c).includes(store)).map((c) => c.id);
  return { writes, reads };
}

/// The stores as laid out: each box, with its title, and each store, by its name, in the sheet's own pixels.
export type LaidStores = { boxes: Array<{ id: string; box: Box }>; stores: Map<string, Box>; width: number; height: number };

const TITLE = 34;
const PAD = 16;
const GAP = 10;
const BETWEEN = 24;
const WRAP = 170;
const LINE = 19;

/// A store's title in lines no wider than a store's text may be.
export function storeLines(s: Store): string[] {
  const lines: string[] = [];
  let line = '';
  for (const w of s.title.split(/\s+/).filter((x) => x !== '')) {
    const next = line === '' ? w : `${line} ${w}`;
    if (line !== '' && measure(next, chartFonts().title) > WRAP) {
      lines.push(line);
      line = w;
    } else {
      line = next;
    }
  }
  lines.push(line);
  return lines;
}

/// A store's size: its title, and a line saying it is computed when it is.
export function storeSize(s: Store): Size {
  const lines = storeLines(s);
  const f = chartFonts();
  const width = Math.max(...lines.map((l) => measure(l, f.title)), s.computed ? measure(t('store_computed'), f.note) : 0);
  return { width: Math.ceil(width) + 24, height: (lines.length + (s.computed ? 1 : 0)) * LINE + 14 };
}

/// Lays the stores out in their boxes, one above the other, each store under the one before it.
export function layStores(stores: Record<string, Store>, size: (s: Store) => Size): LaidStores {
  const sized = Object.entries(stores).map(([name, s]) => ({ name, s, size: size(s) }));
  const stage = chartFonts().stage;
  const inner = Math.max(120, ...sized.map((x) => x.size.width), ...BOXES.map((b) => measure(boxTitle(b.id), stage) + 8));
  const width = inner + 2 * PAD;
  const boxes: LaidStores['boxes'] = [];
  const placed = new Map<string, Box>();
  let y = 0;
  for (const b of BOXES) {
    const inside = b.layers.flatMap((l) => sized.filter((x) => x.s.layer === l));
    if (inside.length === 0) continue;
    let at = y + TITLE;
    for (const x of inside) {
      placed.set(x.name, { x: PAD + (inner - x.size.width) / 2, y: at, width: x.size.width, height: x.size.height });
      at += x.size.height + GAP;
    }
    const height = at - GAP + PAD - y;
    boxes.push({ id: b.id, box: { x: 0, y, width, height } });
    y += height + BETWEEN;
  }
  return { boxes, stores: placed, width, height: Math.max(0, y - BETWEEN) };
}

/// The stores beside the graphs, as SVG at the size their layout takes; `pad` is the room around them.
export function StoresSvg(props: {
  stores: Record<string, Store>;
  laid: LaidStores;
  pad: number;
  selected: string | null;
  lit: Set<string>;
  onHover: (store: string | null) => void;
}) {
  const { stores, laid, pad, selected, lit, onHover } = props;
  return (
    <svg
      className="fc"
      width={laid.width + 2 * pad}
      height={laid.height + 2 * pad}
      viewBox={`${-pad} ${-pad} ${laid.width + 2 * pad} ${laid.height + 2 * pad}`}
      onPointerOver={(ev) => onHover((ev.target as Element).closest('[data-store]')?.getAttribute('data-store') ?? null)}
      onPointerLeave={() => onHover(null)}
    >
      {laid.boxes.map((b) => (
        <g key={b.id} className="fc-group fc-layer">
          <rect x={b.box.x} y={b.box.y} width={b.box.width} height={b.box.height} rx={4} />
          <text x={b.box.x + 12} y={b.box.y + 21}>
            {boxTitle(b.id)}
          </text>
        </g>
      ))}
      {[...laid.stores].map(([name, box]) => {
        const s = stores[name];
        if (s === undefined) return null;
        const lines = storeLines(s);
        const cls = [
          'fc-store',
          s.computed ? 'fc-store-computed' : '',
          selected === name ? 'fc-selected' : '',
          lit.has(name) ? 'fc-store-lit' : '',
        ].join(' ');
        return (
          <g key={name} className={cls} data-store={name}>
            <title>{`${s.title}: ${s.desc}`}</title>
            <rect x={box.x} y={box.y} width={box.width} height={box.height} rx={8} />
            <text x={box.x + box.width / 2} y={box.y + 7 + 13} textAnchor="middle">
              {lines.map((l, i) => (
                <tspan key={i} x={box.x + box.width / 2} dy={i === 0 ? 0 : LINE}>
                  {l}
                </tspan>
              ))}
            </text>
            {s.computed ? (
              <text className="fc-store-note" x={box.x + box.width / 2} y={box.y + 7 + 13 + lines.length * LINE} textAnchor="middle">
                {t('store_computed')}
              </text>
            ) : null}
          </g>
        );
      })}
    </svg>
  );
}

export type Rect = { x: number; y: number; width: number; height: number };

/// The path of a line between a node and a store, in the chart's pixels: out of the side of each that faces the other,
/// curving between them; a write runs from the node to the store, a read from the store to the node.
export function linkPath(node: Rect, store: Rect, write: boolean): string {
  const left = node.x + node.width / 2 < store.x + store.width / 2;
  const a = { x: left ? node.x + node.width : node.x, y: node.y + node.height / 2 };
  const b = { x: left ? store.x : store.x + store.width, y: store.y + store.height / 2 };
  const [from, to] = write ? [a, b] : [b, a];
  const dx = (to.x - from.x) / 2;
  const f = (n: number) => Math.round(n * 10) / 10;
  return `M ${f(from.x)} ${f(from.y)} C ${f(from.x + dx)} ${f(from.y)} ${f(to.x - dx)} ${f(to.y)} ${f(to.x)} ${f(to.y)}`;
}

/// The lines between nodes and stores, over the whole chart, letting the pointer through to what is under them.
export function LinksSvg({ width, height, links }: { width: number; height: number; links: Array<{ d: string; write: boolean }> }) {
  return (
    <svg className="fc fc-links" width={width} height={height} viewBox={`0 0 ${width} ${height}`} aria-hidden="true">
      <defs>
        {['read', 'write'].map((k) => (
          <marker
            key={k}
            id={`fc-link-${k}`}
            className={`fc-link-head fc-link-head-${k}`}
            viewBox="0 0 10 10"
            refX="10"
            refY="5"
            markerUnits="userSpaceOnUse"
            markerWidth={8}
            markerHeight={8}
            orient="auto"
          >
            <path d="M 0 0 L 10 5 L 0 10 z" />
          </marker>
        ))}
      </defs>
      {links.map((l, i) => (
        <path
          key={i}
          className={l.write ? 'fc-link fc-link-write' : 'fc-link fc-link-read'}
          d={l.d}
          markerEnd={`url(#fc-link-${l.write ? 'write' : 'read'})`}
        />
      ))}
    </svg>
  );
}
