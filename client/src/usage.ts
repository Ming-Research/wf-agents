// What a work's usage tab reads from its rounds, apart from how it draws it: where its time went, what each model
// and each session did and cost, where the cost went by kind of token and by cell, and how long each tool ran. Each
// round's level is the one it recorded, never the configuration's now; a round's subagents are the models whose count
// grew in its session during it; the cost by kind of token is weighed by the models' prices as each round's cost line
// is; and the time no round ran is one block, since nothing records why a work stood still.

import type { RoundSummary } from './api';
import { costParts, type CostParts } from './round';

/// What a cell is, as the usage tab names and sorts it.
export type CellInfo = { kind: string; title: string };

/// Who did a round's work: an agent (a model), the tools, or the engine's own step.
export type Doer = 'model' | 'tool' | 'engine';

export function doer(r: RoundSummary, cells: Map<string, CellInfo>): Doer {
  if (r.line !== null && r.not_asked === null) return 'model';
  return r.cell !== null && cells.get(r.cell)?.kind === 'tool' ? 'tool' : 'engine';
}

export type Block = { n: number; cell: string; title: string; doer: Doer; start: number; end: number; cost: number | null };

export type ModelRow = {
  model: string;
  effort: string | null;
  level: string | null;
  rounds: number;
  turns: number;
  ms: number;
  read: number;
  written: number;
  output: number;
  thinking: number;
  cost: number;
};

export type SessionRow = {
  line: string;
  rounds: number;
  turns: number;
  ms: number;
  context: number | null;
  window: number | null;
  compactions: number;
  cost: number;
  points: Array<{ n: number; context: number; compacted: boolean }>;
};

export type Usage = {
  start: number;
  end: number;
  wall: number;
  time: { model: number; tool: number; engine: number; idle: number };
  cost: number;
  agentRounds: number;
  rounds: number;
  sentBack: number;
  tokens: { input: number; read: number; written: number; output: number; thinking: number; turns: number };
  compactions: number;
  blocks: Block[];
  models: ModelRow[];
  /// The models subagents ran on, each with the rounds they ran in.
  subagents: Array<{ model: string; rounds: number[] }>;
  /// What the work's tokens cost by kind, by the models' prices; none when no model had prices.
  kinds: CostParts | null;
  cells: Array<{ cell: string; title: string; cost: number; rounds: number }>;
  sessions: SessionRow[];
  tools: Array<{ cell: string; title: string; ms: number; runs: number }>;
};

const sum = (xs: number[]) => xs.reduce((a, b) => a + b, 0);

/// A work's usage, from its rounds in order, with what each cell is; `now` ends a round still running.
export function usageOf(rounds: RoundSummary[], cells: Map<string, CellInfo>, now: number): Usage {
  const end = (r: RoundSummary) => r.ended_at ?? now;
  const start = rounds.length > 0 ? Math.min(...rounds.map((r) => r.started_at)) : now;
  const last = rounds.length > 0 ? Math.max(...rounds.map(end)) : now;
  const time = { model: 0, tool: 0, engine: 0, idle: 0 };
  const blocks: Block[] = [];
  for (const r of rounds) {
    const d = doer(r, cells);
    time[d] += end(r) - r.started_at;
    blocks.push({
      n: r.n,
      cell: r.cell ?? '',
      title: r.cell !== null ? (cells.get(r.cell)?.title ?? '') : '',
      doer: d,
      start: r.started_at,
      end: end(r),
      cost: r.cost_usd,
    });
  }
  const wall = last - start;
  time.idle = Math.max(0, wall - time.model - time.tool - time.engine);
  const agent = rounds.filter((r) => r.tokens !== null && r.cost_usd !== null);
  const tok = (f: (x: NonNullable<RoundSummary['tokens']>) => number) => sum(agent.map((r) => (r.tokens !== null ? f(r.tokens) : 0)));
  const models = new Map<string, ModelRow>();
  const subagents = new Map<string, number[]>();
  const kinds: CostParts = { input: 0, read: 0, written: 0, output: 0 };
  let priced = false;
  const byCell = new Map<string, { cell: string; title: string; cost: number; rounds: number }>();
  const sessions = new Map<string, SessionRow>();
  for (const r of agent) {
    const x = r.tokens;
    if (x === null) continue;
    const cost = r.cost_usd ?? 0;
    const model = r.model ?? '';
    const key = `${model}\u0000${x.effort ?? ''}\u0000${x.level ?? ''}`;
    const m = models.get(key) ?? {
      model,
      effort: x.effort,
      level: x.level,
      rounds: 0,
      turns: 0,
      ms: 0,
      read: 0,
      written: 0,
      output: 0,
      thinking: 0,
      cost: 0,
    };
    m.rounds += 1;
    m.turns += x.steps;
    m.ms += end(r) - r.started_at;
    m.read += x.read;
    m.written += x.written_5m + x.written_1h;
    m.output += x.output;
    m.thinking += x.thinking;
    m.cost += cost;
    models.set(key, m);
    for (const o of x.others) subagents.set(o, [...(subagents.get(o) ?? []), r.n]);
    const parts = costParts(x, r.model);
    if (parts !== null) {
      priced = true;
      kinds.input += parts.input;
      kinds.read += parts.read;
      kinds.written += parts.written;
      kinds.output += parts.output;
    }
    const cell = r.cell ?? '';
    const c = byCell.get(cell) ?? { cell, title: cells.get(cell)?.title ?? '', cost: 0, rounds: 0 };
    c.cost += cost;
    c.rounds += 1;
    byCell.set(cell, c);
    const line = r.line ?? '';
    const s = sessions.get(line) ?? { line, rounds: 0, turns: 0, ms: 0, context: null, window: null, compactions: 0, cost: 0, points: [] };
    s.rounds += 1;
    s.turns += x.steps;
    s.ms += end(r) - r.started_at;
    s.compactions += r.compactions;
    s.cost += cost;
    if (x.context_end !== null) {
      s.context = x.context_end;
      s.points.push({ n: r.n, context: x.context_end, compacted: r.compactions > 0 });
    }
    s.window = x.window ?? s.window;
    sessions.set(line, s);
  }
  const tools = new Map<string, { cell: string; title: string; ms: number; runs: number }>();
  for (const r of rounds) {
    if (doer(r, cells) !== 'tool' || r.cell === null) continue;
    const x = tools.get(r.cell) ?? { cell: r.cell, title: cells.get(r.cell)?.title ?? '', ms: 0, runs: 0 };
    x.ms += end(r) - r.started_at;
    x.runs += 1;
    tools.set(r.cell, x);
  }
  return {
    start,
    end: last,
    wall,
    time,
    cost: sum(agent.map((r) => r.cost_usd ?? 0)),
    agentRounds: agent.length,
    rounds: rounds.length,
    sentBack: rounds.filter((r) => r.outcome === 'sent_back').length,
    tokens: {
      input: tok((x) => x.input),
      read: tok((x) => x.read),
      written: tok((x) => x.written_5m + x.written_1h),
      output: tok((x) => x.output),
      thinking: tok((x) => x.thinking),
      turns: tok((x) => x.steps),
    },
    compactions: sum(rounds.map((r) => r.compactions)),
    blocks,
    models: [...models.values()].sort((a, b) => b.cost - a.cost),
    subagents: [...subagents.entries()].map(([model, ns]) => ({ model, rounds: ns })),
    kinds: priced ? kinds : null,
    cells: [...byCell.values()].sort((a, b) => b.cost - a.cost),
    sessions: [...sessions.values()].sort((a, b) => b.cost - a.cost),
    tools: [...tools.values()].sort((a, b) => b.ms - a.ms),
  };
}
