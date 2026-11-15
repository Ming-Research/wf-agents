import { describe, expect, it } from 'vitest';

import type { Flow } from '../api';
import { extrasOf, routeTrunks, trunksOf } from './data';
import engineerDefinition from './engineer-flow.json';
import type { Box, Route } from './layout';
import { type ChartEdge, cellNode, chartOf, shownArrows } from './model';

const engineer = (engineerDefinition as unknown as { flow: Flow }).flow;
const chart = chartOf(engineer);

/// The trunks of a set of arrows, each as its cell, its names and the cells it runs, with what else runs each.
const said = (arrows: ChartEdge[]) =>
  trunksOf(arrows).map(
    (t) =>
      `${t.from} ${t.names.map((n) => n.name).join(', ')} > ${t.arrows
        .map((a) => [a.to, ...extrasOf(t, a).map((c) => `+${c.name}`)].join(' '))
        .join('; ')}`,
  );

describe('the trunks of the arrows a write runs', () => {
  it('draws the arrows out of a cell that carry one name as one trunk, what else runs a cell at its branch', () => {
    expect(said(shownArrows(chart, 'D1', false))).toEqual([
      'D1 plan > D3 +reply_plan_review; D4 +reply_tree_review; I1 +reply_plan_doubts',
      'D1 acceptance > T2',
    ]);
  });

  it('keeps the arrows into the cell chosen apart, one trunk for each cell they come from', () => {
    expect(said(shownArrows(chart, 'T2', false))).toEqual([
      'R1 acceptance > T2',
      'R2 acceptance > T2',
      'D1 acceptance > T2',
      'D2 acceptance > T2',
      'I1 acceptance, reply_testing > T2',
    ]);
  });

  it('puts every arrow in one trunk, the name the most of them carry first', () => {
    const all = trunksOf(shownArrows(chart, null, true));
    expect(all.flatMap((t) => t.arrows.map((a) => a.id)).sort()).toEqual(
      shownArrows(chart, null, true)
        .map((a) => a.id)
        .sort(),
    );
    expect(said(shownArrows(chart, null, true))).toEqual([
      'R1 plan > D2; D3 +reply_plan_review; D4 +reply_tree_review; I1 +reply_plan_doubts',
      'R1 acceptance > T2',
      'R2 acceptance > T2',
      'D1 plan > D3 +reply_plan_review; D4 +reply_tree_review; I1 +reply_plan_doubts',
      'D1 acceptance > T2',
      'D2 plan > D4; I1',
      'D2 acceptance > T2',
      'I1 acceptance, reply_testing > T2',
      'I1 code, reply_check > M1',
    ]);
  });
});

describe('the routes of the arrows a write runs', () => {
  // A cell, a cell under it on the main path, and two cells under that which the first runs; the main path straight
  // down the middle of them.
  const box = (y: number): Box => ({ x: 0, y, width: 100, height: 40 });
  const nodes = new Map<string, Box>([
    ['c:a', box(0)],
    ['c:b', box(100)],
    ['c:c', box(200)],
    ['c:d', box(300)],
  ]);
  const line = (y0: number, y1: number): Route => ({ points: [{ x: 50, y: y0 }, { x: 50, y: y1 }], label: null, hops: [] });
  const routes = new Map([
    ['m:ab', line(40, 100)],
    ['m:bc', line(140, 200)],
    ['m:cd', line(240, 300)],
  ]);
  const main = new Set(routes.keys());
  const arrowTo = (to: string, names: string[]): ChartEdge => ({
    id: `a:a>${to}`,
    source: cellNode('a'),
    target: cellNode(to),
    kind: 'arrow',
    from: 'a',
    to,
    reaches: [],
    carries: names.map((name) => ({ name, group: false })),
    back: false,
    main: false,
    branch: null,
    folds: [],
    label: names.join(', '),
  });
  const size = (words: string) => ({ width: words.length * 7 + 8, height: 18 });
  const pieces = (r: Route) => r.points.slice(1).map((q, i) => [r.points[i], q] as const);
  const through = (r: Route, b: Box) =>
    pieces(r).some(
      ([p, q]) =>
        Math.max(p.x, q.x) > b.x &&
        Math.min(p.x, q.x) < b.x + b.width &&
        Math.max(p.y, q.y) > b.y &&
        Math.min(p.y, q.y) < b.y + b.height,
    );

  it('runs a trunk down a corridor beside the cells between, a branch off it into each cell it runs', () => {
    const arrows = [arrowTo('c', ['x', 'y']), arrowTo('d', ['x'])];
    const drawn = routeTrunks(trunksOf(arrows), nodes, routes, main, [], size);
    const toC = drawn.routes.get('a:a>c') as Route;
    const toD = drawn.routes.get('a:a>d') as Route;
    // Both leave the cell at one place and share their corridor, beside b and clear of it.
    expect(toC.points[0]).toEqual(toD.points[0]);
    expect(toC.points[0].y).toBe(40);
    const corridor = toC.points[2].x;
    expect(toD.points[2].x).toBe(corridor);
    expect(corridor < 0 || corridor > 100).toBe(true);
    for (const r of [toC, toD]) expect(through(r, nodes.get('c:b') as Box)).toBe(false);
    // Each comes into the top of its cell, on the corridor's side of the main path, which neither crosses.
    expect(toC.points[toC.points.length - 1].y).toBe(200);
    expect(toD.points[toD.points.length - 1].y).toBe(300);
    for (const r of [toC, toD]) {
      for (const [p, q] of pieces(r)) {
        if (p.y === q.y) expect(Math.min(p.x, q.x) < 50 && Math.max(p.x, q.x) > 50).toBe(false);
      }
    }
    // The name both carry once, beside the trunk; what runs c besides, near its end.
    expect(drawn.labels.map((l) => l.carries.map((c) => c.name).join(', '))).toEqual(['x', 'y']);
  });

  it('runs straight down where nothing stands between', () => {
    const drawn = routeTrunks(trunksOf([arrowTo('b', ['x'])]), nodes, routes, main, [], size);
    const r = drawn.routes.get('a:a>b') as Route;
    expect(r.points).toHaveLength(2);
    expect(r.points[0].x).toBe(r.points[1].x);
    expect(r.points[0].y).toBe(40);
    expect(r.points[1].y).toBe(100);
  });
});
