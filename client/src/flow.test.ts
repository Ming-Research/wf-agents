// The flow page's parts run on the flow the page reads: `flow/flow.json` as the engine stores it compiled, which leaves
// out what a field does not hold, and the view the interface serves of it. `WF_AGENTS_GENERATE=1 cargo test -p api
// --test typescript` writes both to `stored-flow.json`, as CI does before the page's tests. The made-up flows of the
// other tests say what each part does; these, that it does it on the real flow.

import { describe, expect, it } from 'vitest';

import type { Compiled, FlowView } from './api';
import { cellWhere, enteredFrom, leavesTo, ownMoves } from './flow/CellPanel';
import { moveWords } from './graph/FlowChart';
import { cellRadius, cornerWidth, EFFORTS, effortOf, effortsOf, labelSize, sizes, startsAnew, textBoxes } from './graph/flowDraw';
import { besides, type Laid, layoutChart } from './graph/flowLayout';
import { cellActions, cellNode, type Chart, chart, moves } from './graph/flowModel';
import { LEAST, MAP_HEIGHT, MAP_WIDTH, mapOf } from './graph/Minimap';
import { layStores, storeLinks, storeSize, storeUses } from './graph/stores';
import { stagesOf } from './item/checklist';
import stored from './stored-flow.json?raw';

const { compiled, view } = JSON.parse(stored) as { compiled: Compiled; view: FlowView };

/// How a graph's layout reads, in the chart's pixels: the total length of the arrows of rows (`rows`); the longest
/// level piece of any arrow (`run`); how many times the arrows bend (`bends`); how far each node drawn beside a cell
/// (its circles, rules, interjection and what it does every time) is from the
/// cell, the farthest (`far`) and all of them together (`beside`); and how many arrows of rows, arrows and nodes
/// beside a cell there are.
type Measures = {
  rows: number;
  run: number;
  bends: number;
  far: number;
  beside: number;
  rowArrows: number;
  arrows: number;
  besideNodes: number;
};

function measures(c: Chart, laid: Laid): Measures {
  let rows = 0;
  let run = 0;
  let bends = 0;
  let rowArrows = 0;
  let arrows = 0;
  for (const e of c.edges) {
    const p = laid.edges.get(e.id)?.points ?? [];
    if (p.length < 2) continue;
    arrows += 1;
    if (e.kind === 'row') rowArrows += 1;
    for (let i = 0; i + 1 < p.length; i++) {
      const len = Math.hypot(p[i + 1].x - p[i].x, p[i + 1].y - p[i].y);
      if (e.kind === 'row') rows += len;
      if (Math.abs(p[i + 1].y - p[i].y) < 0.5) run = Math.max(run, len);
      if (i > 0) {
        const a = p[i - 1];
        const turn = (p[i].x - a.x) * (p[i + 1].y - p[i].y) - (p[i].y - a.y) * (p[i + 1].x - p[i].x);
        if (Math.abs(turn) > 0.5) bends += 1;
      }
    }
  }
  let far = 0;
  let beside = 0;
  let besideNodes = 0;
  for (const n of c.nodes) {
    if (!['in', 'out', 'rule', 'interjection', 'does'].includes(n.kind)) continue;
    const a = laid.nodes.get(n.id);
    const b = laid.nodes.get(cellNode(n.cell));
    if (a === undefined || b === undefined) continue;
    const dx = Math.max(0, a.x - (b.x + b.width), b.x - (a.x + a.width));
    const dy = Math.max(0, a.y - (b.y + b.height), b.y - (a.y + a.height));
    far = Math.max(far, Math.hypot(dx, dy));
    beside += Math.hypot(dx, dy);
    besideNodes += 1;
  }
  return {
    rows: Math.round(rows),
    run: Math.round(run),
    bends,
    far: Math.round(far),
    beside: Math.round(beside),
    rowArrows,
    arrows,
    besideNodes,
  };
}

/// What the test holds a graph's layout to: the length of an arrow of a row and the bends of an arrow on average, the
/// longest level run, the farthest node beside a cell, and how far such a node is on average; the totals over the
/// arrows and nodes there are, so that a flow that grows does not count as a layout that got worse.
type Limits = { rowLength: number; run: number; bendsPerArrow: number; far: number; besideEach: number };

function limitsOf(m: Measures): Limits {
  return {
    rowLength: m.rows / Math.max(1, m.rowArrows),
    run: m.run,
    bendsPerArrow: m.bends / Math.max(1, m.arrows),
    far: m.far,
    besideEach: m.beside / Math.max(1, m.besideNodes),
  };
}

/// The most each graph's layout may measure: a tenth over what it measured on the flow of e9bddd0 (CI run
/// 40865042355), each cell laid out with what is drawn beside it, so that a change that makes the layout worse fails
/// here; for the architect graph, on the flow of 962c176 (CI run 40995651263), once the setting of a policy was a
/// stage of it, which made it a graph of more cells and longer rows. Before cells were laid out so, on the flow of
/// 6e258c1 (CI run 40855485231), the engineer graph measured rows 42384, run 1046, bends 214, far 388, beside 5522, and
/// the architect graph rows 82062, run 1731, bends 286, far 198, beside 1539.
const MOST: Record<string, Limits> = {
  engineer: { rowLength: 590, run: 2162, bendsPerArrow: 1.71, far: 722, besideEach: 76 },
  architect: { rowLength: 2039, run: 4650, bendsPerArrow: 3.18, far: 49, besideEach: 24 },
};
const engine = compiled.engine;
const graphs = compiled.graphs.map((g) => {
  const v = view.graphs.find((x) => x.name === g.name);
  if (v === undefined) throw new Error(`the view has no ${g.name} graph`);
  return { g, v };
});

describe('the flow as the engine stores it', () => {
  it('leaves out what a field does not hold, which the page reads as none', () => {
    const cells = compiled.graphs.flatMap((g) => g.cells);
    const forms = cells.flatMap((c) => c.forms ?? []);
    expect(forms.some((f) => !('lines' in f))).toBe(true);
    expect(forms.some((f) => !('field' in f))).toBe(true);
    expect(cells.some((c) => !('stage' in c))).toBe(true);
  });

  it("badges every agent's cell, the gates' among them, with an effort of the scale, and no other cell", () => {
    const cells = compiled.graphs.flatMap((g) => g.cells);
    for (const c of cells) {
      const e = effortOf(c);
      if (c.kind === 'agent') {
        expect(EFFORTS.some(([level]) => level === c.effort), `${c.id} ${c.effort ?? ''}`).toBe(true);
        expect(e?.cls, c.id).toBe(`fc-effort-${c.effort ?? ''}`);
      } else {
        expect(e, c.id).toBeNull();
      }
    }
    expect(cells.some((c) => c.gate && effortOf(c) !== null)).toBe(true);
    // The legend gives every level the flow uses, low to high.
    const levels = effortsOf(cells).map((e) => e.level);
    expect(levels).toEqual(EFFORTS.map(([level]) => level).filter((l) => cells.some((c) => c.effort === l)));
    expect(levels.length).toBeGreaterThan(1);
  });

  it('badges a cell that starts a new session every round, and not one that starts it at its first round in a work', () => {
    const cells = compiled.graphs.flatMap((g) => g.cells);
    expect(cells.some(startsAnew)).toBe(true);
    expect(cells.some((c) => c.new_session)).toBe(true);
    expect(cells.filter((c) => c.new_session && startsAnew(c)).map((c) => c.id)).toEqual([]);
    // The badges over a cell's top left corner end inside it, with a cell's side padding to spare.
    for (const { g, v } of graphs) {
      const { size } = sizes(g, chart(g, v, engine));
      for (const c of g.cells) {
        const width = size.get(cellNode(c.id))?.width ?? 0;
        expect(width, `${g.name} ${c.id}`).toBeGreaterThanOrEqual(cornerWidth(c) + 24);
      }
    }
  });

  it("keeps every line of a cell's text inside its box, a rounded cell's curved ends included", () => {
    // A point of the box drawn `width` by `height` with corners of `r`: inside the straight part, or within `r` of the
    // middle of the corner it is in.
    const inside = (x: number, y: number, width: number, height: number, r: number) => {
      const cx = Math.min(Math.max(x, r), width - r);
      const cy = Math.min(Math.max(y, r), height - r);
      return x >= 0 && y >= 0 && x <= width && y <= height && Math.hypot(x - cx, y - cy) <= r + 1e-6;
    };
    let tall = 0;
    for (const { g, v } of graphs) {
      const { size, lines, notes } = sizes(g, chart(g, v, engine));
      for (const c of g.cells) {
        const at = cellNode(c.id);
        const s = size.get(at);
        if (s === undefined) continue;
        const r = cellRadius(c, s.height);
        if ((c.kind === 'engine' || c.kind === 'owner') && s.height > 40) tall += 1;
        for (const b of textBoxes(c, s, lines.get(at) ?? [], notes.get(at))) {
          const corners = [
            [b.x, b.y],
            [b.x + b.width, b.y],
            [b.x, b.y + b.height],
            [b.x + b.width, b.y + b.height],
          ];
          for (const [x, y] of corners) expect(inside(x, y, s.width, s.height, r), `${g.name} ${c.id} at ${x}, ${y}`).toBe(true);
        }
      }
    }
    // The flow has rounded cells of more than one line, whose ends a half-circle would have cut into.
    expect(tall).toBeGreaterThan(0);
  });

  it('draws every cell of every graph on the map of the flow page, and every store, inside the map', async () => {
    // The graphs side by side with the stores between the first two, each drawing 24 px inside its sheet, as the
    // chart places them.
    const pad = 24;
    const charts = graphs.map(({ g, v }) => chart(g, v, engine));
    const laid = await Promise.all(
      graphs.map(({ g }, i) => {
        const c = charts[i];
        const measured = sizes(g, c);
        const edges = new Map(c.edges.map((e) => [e.id, e]));
        return layoutChart(
          c,
          (id) => measured.size.get(id) ?? { width: 40, height: 40 },
          (id) => {
            const e = edges.get(id);
            return e ? labelSize(e) : null;
          },
        );
      }),
    );
    const all = engine.stores ?? {};
    const stores = Object.keys(all).length > 0 ? layStores(all, storeSize) : null;
    const storesAt = graphs.length >= 2 ? 1 : graphs.length;
    const sheets = laid.map((l) => ({ width: l.bounds.width + 2 * pad, height: l.bounds.height + 2 * pad }));
    if (stores !== null) sheets.splice(storesAt, 0, { width: stores.width + 2 * pad, height: stores.height + 2 * pad });
    const placed = besides(sheets);
    const map = mapOf(
      placed,
      laid.map((l, i) => ({
        graph: graphs[i].g.name,
        cells: graphs[i].g.cells,
        chart: charts[i],
        laid: l,
        left: placed.x[stores !== null && i >= storesAt ? i + 1 : i],
        overlays: null,
      })),
      placed.top,
      pad,
      stores !== null ? { laid: stores, left: placed.x[storesAt] } : null,
    );
    const { width, height } = map.geom;
    expect(width <= MAP_WIDTH + 1e-6 && height <= MAP_HEIGHT + 1e-6, `${width} by ${height}`).toBe(true);
    expect(Math.max(width / MAP_WIDTH, height / MAP_HEIGHT)).toBeCloseTo(1, 6);
    const inside = (s: { x: number; y: number; width: number; height: number }) =>
      s.x >= -0.5 && s.y >= -0.5 && s.x + s.width <= width + 0.5 && s.y + s.height <= height + 0.5;
    for (const { g } of graphs) {
      for (const cell of g.cells) {
        const drawn = map.shapes.filter((s) => s.kind === 'cell' && s.graph === g.name && s.cell === cell.id);
        expect(drawn.length, `${g.name} ${cell.id}`).toBe(1);
        const s = drawn[0];
        expect(inside(s) && s.width >= LEAST && s.height >= LEAST, `${g.name} ${cell.id}`).toBe(true);
      }
      expect(map.shapes.filter((s) => s.kind === 'box' && s.graph === g.name).length, g.name).toBeGreaterThan(0);
    }
    const drawnStores = map.shapes.filter((s) => s.kind === 'cell' && s.graph === null);
    expect(drawnStores.length).toBe(Object.keys(all).length);
    expect(map.shapes.every(inside)).toBe(true);
  }, 30_000);

  it("lays each graph out with short arrows, short level runs, few bends and what is beside a cell next to it", async () => {
    for (const { g, v } of graphs) {
      const c = chart(g, v, engine);
      const measured = sizes(g, c);
      const edges = new Map(c.edges.map((e) => [e.id, e]));
      const laid = await layoutChart(
        c,
        (id) => measured.size.get(id) ?? { width: 40, height: 40 },
        (id) => {
          const e = edges.get(id);
          return e ? labelSize(e) : null;
        },
      );
      const m = measures(c, laid);
      console.log(`layout of the ${g.name} graph: ${JSON.stringify({ ...m, ...limitsOf(m) })}`);
      const most = MOST[g.name];
      if (most === undefined) continue;
      const l = limitsOf(m);
      for (const k of ['rowLength', 'run', 'bendsPerArrow', 'far', 'besideEach'] as const) {
        expect(l[k], `${g.name} ${k}`).toBeLessThanOrEqual(most[k]);
      }
    }
    expect(Object.keys(MOST).every((name) => graphs.some(({ g }) => g.name === name))).toBe(true);
  }, 30_000);

  for (const { g, v } of graphs) {
    describe(`its ${g.name} graph`, () => {
      it('says each move a cell makes of its own, and the lines it is for', () => {
        const ms = moves(g);
        expect(ms.length).toBeGreaterThan(0);
        for (const m of ms) {
          expect(m.lines === null || m.lines.every((l) => typeof l === 'string'), m.row).toBe(true);
          expect(moveWords(m), m.row).not.toContain('undefined');
        }
      });

      it('puts the actions a cell does every time beside it, or in the legend', () => {
        const { shared, own } = cellActions(g);
        for (const a of shared) expect([typeof a.action, typeof a.at]).toEqual(['string', 'string']);
        for (const [id, list] of own) expect(list.every((a) => typeof a === 'string'), id).toBe(true);
      });

      it('draws every cell, every arrow between two of its nodes, and each node at a size', () => {
        const c = chart(g, v, engine);
        const nodes = new Set(c.nodes.map((n) => n.id));
        for (const cell of g.cells) expect(nodes.has(cellNode(cell.id)), cell.id).toBe(true);
        for (const e of c.edges) {
          expect(nodes.has(e.source) && nodes.has(e.target), e.id).toBe(true);
          if (e.cell !== null) expect(e.label, e.id).not.toBe('');
        }
        const { size } = sizes(g, c);
        for (const n of c.nodes) {
          const s = size.get(n.id);
          expect(s !== undefined && Number.isFinite(s.width) && s.width > 0 && s.height > 0, n.id).toBe(true);
        }
        expect(() => storeLinks(engine, g, c)).not.toThrow();
        for (const store of Object.keys(engine.stores ?? {})) expect(() => storeUses(engine, g, store)).not.toThrow();
      });

      it('lays the actions on lines into a cell out just above it, in its box, with no cell between', async () => {
        const c = chart(g, v, engine);
        const measured = sizes(g, c);
        const edges = new Map(c.edges.map((e) => [e.id, e]));
        const laid = await layoutChart(
          c,
          (id) => measured.size.get(id) ?? { width: 40, height: 40 },
          (id) => {
            const e = edges.get(id);
            return e ? labelSize(e) : null;
          },
        );
        const box = (id: string) => laid.nodes.get(id) ?? { x: 0, y: 0, width: 0, height: 0 };
        const nodes = new Map(c.nodes.map((n) => [n.id, n]));
        for (const n of c.nodes) {
          const into = nodes.get(cellNode(n.cell));
          if (n.kind !== 'action' || into === undefined || into.part !== n.part) continue;
          expect(n.group, n.id).toBe(into.group);
          const a = box(n.id);
          const t = box(into.id);
          expect(a.y + a.height <= t.y, n.id).toBe(true);
          const between = c.nodes.filter((x) => {
            const b = box(x.id);
            return x.kind === 'cell' && x.part === n.part && b.y >= a.y + a.height && b.y + b.height <= t.y;
          });
          expect(between.map((x) => x.cell), n.id).toEqual([]);
        }
      });

      it('draws the way back from an ask only into the cells whose own answers asked', () => {
        const c = chart(g, v, engine);
        const views = new Map(v.cells.map((x) => [x.id, x]));
        const asked = (t: string, by: string) =>
          (g.cells.find((x) => x.id === t)?.rows ?? []).some(
            (r, j) => !r.common && r.fallback !== true && (views.get(t)?.targets[j] ?? []).includes(by),
          );
        for (const x of g.cells) {
          x.rows.forEach((r, i) => {
            if (r.to !== '@asker') return;
            for (const t of views.get(x.id)?.targets[i] ?? []) {
              // Straight into the cell, through the actions beside it, or through the circles between parts.
              const into = (to: string) =>
                to === cellNode(t) || to === `o:${t}:${x.id}` || to.startsWith(`a:${t}:`) || to.startsWith(`a:${x.id}>${t}:`);
              const drawn = c.edges.some((e) => e.cell === x.id && e.rows.includes(i) && into(e.target));
              expect(drawn, `${x.id} ${r.name} into ${t}`).toBe(asked(t, x.id));
            }
          });
        }
      });

      it("draws no move as the main path, only a cell's own answers", () => {
        const c = chart(g, v, engine);
        const cells = new Map(g.cells.map((x) => [x.id, x]));
        for (const e of c.edges) {
          const rows = [
            ...(e.cell !== null ? e.rows.map((i) => cells.get(e.cell ?? '')?.rows[i]) : []),
            ...e.through.map((x) => cells.get(x.cell)?.rows[x.row]),
          ];
          expect(e.main, e.id).toBe(rows.some((r) => r !== undefined && r.main && !r.common));
        }
      });

      it("lists in a cell's panel every move of its own, or names it as in the legend", () => {
        for (const c of g.cells) {
          const own = ownMoves(g, c);
          const listed = new Set([...own.rows.map((i) => c.rows[i].name), ...own.legend]);
          for (const i of leavesTo(c).moves) expect(listed.has(c.rows[i].name), `${c.id} ${c.rows[i].name}`).toBe(true);
          for (const m of own.lacks) expect(c.rows.some((r) => r.name === m.row), `${c.id} ${m.row}`).toBe(false);
        }
      });

      it("opens every cell's panel", () => {
        for (const c of g.cells) {
          const cv = v.cells.find((x) => x.id === c.id);
          expect(cv, c.id).toBeDefined();
          expect(cellWhere(g, c) ?? '', c.id).not.toContain('undefined');
          const leaves = leavesTo(c);
          const all = [...leaves.answers, ...leaves.moves, ...leaves.fallback].sort((a, b) => a - b);
          expect(all, c.id).toEqual(c.rows.map((_, i) => i));
          expect(() => enteredFrom(g, cv?.entered_from ?? [], c.id), c.id).not.toThrow();
        }
      });

      it("groups a work's progress by the graph's stages", () => {
        expect(stagesOf(g).every((s) => typeof s === 'string')).toBe(true);
      });
    });
  }

});
