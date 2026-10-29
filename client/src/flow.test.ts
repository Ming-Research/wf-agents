// The flow page's parts run on the flow the page reads: `flow/flow.json` as the engine stores it compiled, which leaves
// out what a field does not hold, and the view the interface serves of it. `WF_AGENTS_GENERATE=1 cargo test -p api
// --test typescript` writes both to `stored-flow.json`, as CI does before the page's tests. The made-up flows of the
// other tests say what each part does; these, that it does it on the real flow.

import { describe, expect, it } from 'vitest';

import type { Compiled, FlowView } from './api';
import { cellWhere, enteredFrom, leavesTo, ownMoves } from './flow/CellPanel';
import { moveWords } from './graph/FlowChart';
import { EFFORTS, effortOf, effortsOf, labelSize, sizes } from './graph/flowDraw';
import { layoutChart } from './graph/flowLayout';
import { cellActions, cellNode, chart, moves } from './graph/flowModel';
import { storeLinks, storeUses } from './graph/stores';
import { stagesOf } from './item/checklist';
import stored from './stored-flow.json?raw';

const { compiled, view } = JSON.parse(stored) as { compiled: Compiled; view: FlowView };
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
            return e ? labelSize(e, true) : null;
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
