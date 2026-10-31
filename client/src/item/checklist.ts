// Where a work stands, as a checklist: one line per cell it has been at, in the order it first got there, those done
// ticked; the cell it is at now; then the cells still ahead on the flow's main path, as the chart marks it, to the
// end. Loops cannot be foreseen, so only the main path is ahead, and a cell reached again stays one line that counts
// how many times. The engine's steps that only route the work on have no line, and the rounds of a cell that serves
// another (the tools an agent asks for) count under that cell. Pure, so that it is tested on its own.

import type { CompiledCell, CompiledGraph, RoundSummary } from '../api';

export type StepState = 'done' | 'now' | 'ahead';

export type Step = {
  cell: string;
  title: string;
  /// The stage it is shown under: its cell's, or for a cell at no stage (one that asks, answers or chooses the way
  /// on), the stage the work was at when it first got there.
  stage: string | null;
  state: StepState;
  /// How many times the work got there.
  passes: number;
  /// The row its latest round took.
  row: string | null;
  /// The rounds counted under it, oldest first: its own, and those of a cell that served it.
  rounds: RoundSummary[];
};

/// Where the work is: the cell it is at, the cell it left to ask when it asks, and whether it still goes on.
export type Where = { at: string; resumeAt: string | null; live: boolean };

/// A cell's stage; none for a cell at no stage, which a compiled flow writes by leaving the stage out.
const stageOf = (c: CompiledCell | undefined): string | null => c?.stage ?? null;

/// A cell drawn apart from the main part of the chart, as the chart draws it: not in the asking part, not the start,
/// and no row leads to it but the rows every agent's cell shares, such as the cell that runs the tools an agent asks
/// for. It serves whichever cell called it.
export function serves(g: CompiledGraph, id: string): boolean {
  if (g.asking.includes(id) || id === g.start) return false;
  return !g.cells.some((c) => c.rows.some((r) => r.to === id && !r.common));
}

/// Whether a cell has a line of its own: an agent's or the owner's cell; a tool cell that serves no other; an engine
/// step that waits or asks someone and brings the answer back. A step that only routes the work on has none.
export function hasLine(g: CompiledGraph, c: CompiledCell): boolean {
  if (c.kind === 'agent' || c.kind === 'owner') return true;
  if (c.kind === 'tool') return !serves(g, c.id);
  return c.rows.some((r) => r.waits || r.to === '@asker');
}

/// The cells of the main path, in order: every cell the rows marked main lead to from the start (not the rows every
/// agent's cell shares, nor a row back to the cell itself), each after every cell that leads to it, and otherwise in
/// the graph's order.
export function mainPath(g: CompiledGraph): string[] {
  const place = new Map(g.cells.map((c, i) => [c.id, i]));
  const next = new Map<string, string[]>();
  for (const c of g.cells) {
    for (const r of c.rows) {
      if (!r.main || r.common || r.to === c.id || !place.has(r.to) || serves(g, r.to)) continue;
      const to = next.get(c.id) ?? [];
      if (!to.includes(r.to)) to.push(r.to);
      next.set(c.id, to);
    }
  }
  const reached = new Set<string>();
  const stack = [g.start];
  while (stack.length > 0) {
    const x = stack.pop() as string;
    if (reached.has(x)) continue;
    reached.add(x);
    stack.push(...(next.get(x) ?? []));
  }
  const into = new Map<string, number>([...reached].map((x) => [x, 0]));
  for (const x of reached) for (const y of next.get(x) ?? []) into.set(y, (into.get(y) ?? 0) + 1);
  const byPlace = (a: string, b: string) => (place.get(a) ?? 0) - (place.get(b) ?? 0);
  const ready = [...reached].filter((x) => into.get(x) === 0);
  const order: string[] = [];
  while (ready.length > 0) {
    ready.sort(byPlace);
    const x = ready.shift() as string;
    order.push(x);
    for (const y of next.get(x) ?? []) {
      const n = (into.get(y) ?? 0) - 1;
      into.set(y, n);
      if (n === 0) ready.push(y);
    }
  }
  // A main path that went round, which the flow's does not, leaves cells over: they follow in the graph's order.
  for (const x of [...reached].sort(byPlace)) if (!order.includes(x)) order.push(x);
  return order;
}

/// The cell of the main path the work goes on from: the cell it is at, the cell it left to ask, the cell it serves,
/// or else the first cell of the main path its rows lead to.
function anchor(g: CompiledGraph, path: string[], where: Where, now: string | null): string | null {
  const on = new Set(path);
  const resume = g.asking.includes(where.at) ? where.resumeAt : null;
  for (const x of [where.at, resume, now]) if (x !== null && on.has(x)) return x;
  const cells = new Map(g.cells.map((c) => [c.id, c]));
  const seen = new Set<string>();
  const queue = [where.at];
  while (queue.length > 0) {
    const x = queue.shift() as string;
    if (seen.has(x)) continue;
    seen.add(x);
    if (x !== where.at && on.has(x)) return x;
    for (const r of cells.get(x)?.rows ?? []) if (!r.common && cells.has(r.to)) queue.push(r.to);
  }
  return null;
}

/// The checklist of a work's rounds on its graph.
export function progress(g: CompiledGraph, rounds: RoundSummary[], where: Where): Step[] {
  const cells = new Map(g.cells.map((c) => [c.id, c]));
  const steps = new Map<string, Step>();
  let stage: string | null = null;
  const step = (id: string): Step => {
    const had = steps.get(id);
    if (had !== undefined) return had;
    const c = cells.get(id);
    const s: Step = { cell: id, title: c?.title ?? id, stage: stageOf(c) ?? stage, state: 'done', passes: 0, row: null, rounds: [] };
    steps.set(id, s);
    return s;
  };
  const asking = new Set(g.asking);
  // The cell of the line the latest round counted under; and of those, the latest outside the asking part, since
  // coming back from an ask is not getting to a cell again.
  let last: string | null = null;
  let lastOut: string | null = null;
  const reached = (id: string) => (asking.has(id) ? last !== id : lastOut !== id);
  for (const r of rounds) {
    // A gate the engine answered itself, since the project lacks what it needs, was not a step of the work.
    if (r.cell === null || r.not_asked !== null) continue;
    const c = cells.get(r.cell);
    if (c === undefined) continue;
    if (hasLine(g, c)) {
      const s = step(c.id);
      if (reached(c.id)) s.passes += 1;
      last = c.id;
      if (!asking.has(c.id)) lastOut = c.id;
      s.rounds.push(r);
      if (r.transition !== null) s.row = r.transition;
      stage = stageOf(c) ?? stage;
    } else if (serves(g, c.id) && last !== null) {
      steps.get(last)?.rounds.push(r);
    }
  }
  if (where.live) {
    const c = cells.get(where.at);
    const now = c === undefined ? null : hasLine(g, c) ? c.id : serves(g, c.id) ? last : null;
    if (now !== null) {
      const s = step(now);
      // Got there again, and no round of it ran yet.
      if (last !== now && reached(now)) s.passes += 1;
      s.state = 'now';
    }
    const path = mainPath(g);
    const from = anchor(g, path, where, now);
    if (from !== null) {
      for (const id of path.slice(path.indexOf(from) + 1)) {
        const x = cells.get(id);
        if (x === undefined || !hasLine(g, x) || steps.has(id)) continue;
        step(id).state = 'ahead';
      }
    }
  }
  return [...steps.values()];
}

/// The stages of a graph, in the order of its cells.
export function stagesOf(g: CompiledGraph): string[] {
  const out: string[] = [];
  for (const c of g.cells) {
    const s = stageOf(c);
    if (s !== null && !out.includes(s)) out.push(s);
  }
  return out;
}

/// The lines under the flow's stages, in the stages' order; lines at no stage first.
export function byStage(g: CompiledGraph, steps: Step[]): Array<{ stage: string | null; steps: Step[] }> {
  const order = stagesOf(g);
  const groups = new Map<string | null, Step[]>();
  for (const s of steps) groups.set(s.stage, [...(groups.get(s.stage) ?? []), s]);
  const rank = (s: string | null) => (s === null ? -1 : order.indexOf(s));
  return [...groups.entries()].sort(([a], [b]) => rank(a) - rank(b)).map(([stage, list]) => ({ stage, steps: list }));
}
