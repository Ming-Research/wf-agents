// The flow page's parts run on the flow the page reads: `flow/flow.json` as the engine stores it compiled, which leaves
// out what a field does not hold, and the view the interface serves of it. `WF_AGENTS_GENERATE=1 cargo test -p api
// --test typescript` writes both to `stored-flow.json`, as CI does before the page's tests. The made-up flows of the
// other tests say what each part does; these, that it does it on the real flow.

import { describe, expect, it } from 'vitest';

import type { Compiled, FlowView } from './api';
import { cellWhere, enteredFrom, leavesTo } from './flow/CellPanel';
import { moveWords } from './graph/FlowChart';
import { sizes } from './graph/flowDraw';
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
