// What a usage tab reads from the rounds of one work or several, their sessions and their keep-warm pings, apart from
// how it draws it: where the time went, what each model and each session did and cost, where the cost went by kind of
// token and by cell, how long each tool ran, and what each ping cost and where the work waited. Each round's level is
// the one it recorded, never the configuration's now; a round's subagents are the models whose count grew in its
// session during it; the cost by kind of token is weighed by the models' prices as each round's cost line is; the cost
// counts the pings; and each stretch no round ran is a block of its own, with what the record shows was waited on.
// Several works, such as an architect's over a span of time, count as one, their rounds in the order they started.

import type { Ping, RoundSummary, TokenCosts, WorkSession } from './api';

/// What a cell is, as the usage tab names and sorts it.
export type CellInfo = { kind: string; title: string };

/// Who did a round's work: an agent (a model), the tools, or the engine's own step.
export type Doer = 'model' | 'tool' | 'engine';

export function doer(r: RoundSummary, cells: Map<string, CellInfo>): Doer {
  if (r.line !== null && r.not_asked === null) return 'model';
  return r.cell !== null && cells.get(r.cell)?.kind === 'tool' ? 'tool' : 'engine';
}

/// A round as the usage names it: its work and its number in the work.
export type RoundOf = { work: number; n: number };

/// What one work gives the usage: its number, its rounds in order, its sessions and their keep-warm pings.
export type WorkPart = { work: number; rounds: RoundSummary[]; sessions?: WorkSession[]; pings?: Ping[] };

export type Block = RoundOf & { cell: string; title: string; doer: Doer; start: number; end: number; cost: number | null };

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
  /// The session's row; none for rounds recorded without one.
  id: number | null;
  line: string;
  /// Its place among its line's sessions, from 1.
  nth: number;
  /// Why it started, as the session manager recorded it: line, cell, limit, model, carried or owner; none when not
  /// recorded.
  started: string | null;
  /// The cell of its first round.
  cell: string | null;
  /// For one started past the limit: the context the line's session before had at its end.
  before: number | null;
  rounds: number;
  turns: number;
  ms: number;
  context: number | null;
  window: number | null;
  compactions: number;
  /// What its rounds and its keep-warm pings cost.
  cost: number;
  pings: number;
  /// Its context after each of its rounds, each at the round's place among all the rounds counted, from 0.
  points: Array<RoundOf & { at: number; context: number; compacted: boolean }>;
};

/// A keep-warm ping, with the session it went to as the sessions are numbered by line.
export type PingRow = Ping & { nth: number };

export type Usage = {
  start: number;
  end: number;
  wall: number;
  time: { model: number; tool: number; engine: number; idle: number };
  /// What its rounds and its keep-warm pings cost.
  cost: number;
  /// What its keep-warm pings cost.
  pingCost: number;
  agentRounds: number;
  rounds: number;
  sentBack: number;
  tokens: { input: number; read: number; written: number; output: number; thinking: number; turns: number };
  compactions: number;
  /// A block for each round, in the order they started.
  blocks: Block[];
  models: ModelRow[];
  /// The models subagents ran on, each with the rounds they ran in.
  subagents: Array<{ model: string; rounds: RoundOf[] }>;
  /// What the tokens cost by kind, by the models' prices; none when no model had prices.
  kinds: TokenCosts | null;
  cells: Array<{ cell: string; title: string; cost: number; rounds: number }>;
  sessions: SessionRow[];
  pings: PingRow[];
  tools: Array<{ cell: string; title: string; ms: number; runs: number }>;
};

const sum = (xs: number[]) => xs.reduce((a, b) => a + b, 0);

/// The usage of one work or several, from each one's rounds, sessions and keep-warm pings, with what each cell is;
/// `now` ends a round still running.
export function usageOf(works: WorkPart[], cells: Map<string, CellInfo>, now: number): Usage {
  // Every round with its work, in the order they started; a work's own rounds keep their order.
  const all = works
    .flatMap((w) => w.rounds.map((r, i) => ({ work: w.work, i, r })))
    .sort((a, b) => a.r.started_at - b.r.started_at || a.work - b.work || a.i - b.i);
  const rounds = all.map((x) => x.r);
  const workOf = new Map(all.map((x) => [x.r, x.work]));
  const at = new Map(rounds.map((r, i) => [r, i]));
  const of = (r: RoundSummary): RoundOf => ({ work: workOf.get(r) ?? 0, n: r.n });
  // A session a work went on in from the work before it, and its pings, are each work's: they count once.
  const once = <T extends { id: number }>(xs: T[]): T[] => [...new Map(xs.map((x) => [x.id, x])).values()];
  const workSessions = once(works.flatMap((w) => w.sessions ?? [])).sort((a, b) => a.id - b.id);
  const pings = once(works.flatMap((w) => w.pings ?? [])).sort((a, b) => a.started_at - b.started_at);
  const end = (r: RoundSummary) => r.ended_at ?? now;
  const start = rounds.length > 0 ? Math.min(...rounds.map((r) => r.started_at)) : now;
  const last = rounds.length > 0 ? Math.max(...rounds.map(end)) : now;
  const time = { model: 0, tool: 0, engine: 0, idle: 0 };
  const blocks: Block[] = [];
  for (const r of rounds) {
    const d = doer(r, cells);
    time[d] += end(r) - r.started_at;
    blocks.push({
      ...of(r),
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
  const subagents = new Map<string, RoundOf[]>();
  const kinds: TokenCosts = { input: 0, read: 0, written: 0, output: 0 };
  let priced = false;
  const byCell = new Map<string, { cell: string; title: string; cost: number; rounds: number }>();
  const sessions = new Map<string, SessionRow>();
  // Each session's place among its line's, by the order they started.
  const nth = new Map<number, number>();
  const perLine = new Map<string, number>();
  for (const w of workSessions) {
    const k = (perLine.get(w.line) ?? 0) + 1;
    perLine.set(w.line, k);
    nth.set(w.id, k);
  }
  const started = new Map(workSessions.map((w) => [w.id, w.started]));
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
    for (const o of x.others) subagents.set(o, [...(subagents.get(o) ?? []), of(r)]);
    const parts = x.costs;
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
    const session = r.session !== null ? String(r.session) : `line ${line}`;
    const s = sessions.get(session) ?? {
      id: r.session,
      line,
      nth: r.session !== null ? (nth.get(r.session) ?? 1) : 1,
      started: r.session !== null ? (started.get(r.session) ?? null) : null,
      cell: r.cell,
      before: null,
      rounds: 0,
      turns: 0,
      ms: 0,
      context: null,
      window: null,
      compactions: 0,
      cost: 0,
      pings: 0,
      points: [],
    };
    s.rounds += 1;
    s.turns += x.steps;
    s.ms += end(r) - r.started_at;
    s.compactions += r.compactions;
    s.cost += cost;
    if (x.context_end !== null) {
      s.context = x.context_end;
      s.points.push({ ...of(r), at: at.get(r) ?? 0, context: x.context_end, compacted: r.compactions > 0 });
    }
    s.window = x.window ?? s.window;
    sessions.set(session, s);
  }
  // A session started past the limit says how large the line's session before it had grown.
  const rows = [...sessions.values()].sort((a, b) => (a.id ?? 0) - (b.id ?? 0));
  for (const s of rows) {
    if (s.started !== 'limit') continue;
    const prior = rows.filter((x) => x.line === s.line && (x.id ?? 0) < (s.id ?? 0)).pop();
    s.before = prior?.context ?? null;
  }
  const bySession = new Map(rows.filter((s) => s.id !== null).map((s) => [s.id as number, s]));
  for (const p of pings) {
    const s = bySession.get(p.session);
    if (s === undefined) continue;
    s.pings += 1;
    s.cost += p.cost_usd ?? 0;
  }
  const pingCost = sum(pings.map((p) => p.cost_usd ?? 0));
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
    cost: sum(agent.map((r) => r.cost_usd ?? 0)) + pingCost,
    pingCost,
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
    sessions: rows,
    pings: pings.map((p) => ({ ...p, nth: nth.get(p.session) ?? 1 })),
    tools: [...tools.values()].sort((a, b) => b.ms - a.ms),
  };
}

/// A stretch of the wall time in which no round ran: from when to when, the round that ended last before it and the
/// round that started after it, and what the record shows was waited on then.
export type Gap = { start: number; end: number; before: RoundOf; after: RoundOf; waited: string[] };

/// The stretches in which no round ran, each from the latest end of the rounds before it to the next round's start,
/// those shorter than a second left out; each with the events given that fall in it, such as an ask answered or a
/// design card ruled on, which say what the work waited for.
export function gapsOf(blocks: Block[], events: Array<{ at: number; what: string }> = []): Gap[] {
  const sorted = [...blocks].sort((a, b) => a.start - b.start);
  const gaps: Gap[] = [];
  let end = -Infinity;
  let before: RoundOf | null = null;
  for (const b of sorted) {
    if (before !== null && b.start - end >= 1000) {
      const start = end;
      const waited = events.filter((e) => e.at >= start && e.at <= b.start).map((e) => e.what);
      gaps.push({ start, end: b.start, before, after: { work: b.work, n: b.n }, waited });
    }
    if (b.end >= end) {
      end = b.end;
      before = { work: b.work, n: b.n };
    }
  }
  return gaps;
}

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/// The steps the axis of a work's wall time may mark it by, the finest first.
const STEPS = [
  MINUTE,
  2 * MINUTE,
  5 * MINUTE,
  10 * MINUTE,
  15 * MINUTE,
  30 * MINUTE,
  HOUR,
  2 * HOUR,
  3 * HOUR,
  6 * HOUR,
  12 * HOUR,
  DAY,
  2 * DAY,
  7 * DAY,
];

/// A time of day as the axis writes it, hours and minutes.
export function clockOf(ms: number): string {
  const d = new Date(ms);
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
}

/// A day as the axis writes it, its month's short name and its date.
function dayOf(ms: number): string {
  return new Date(ms).toLocaleDateString('en', { month: 'short', day: 'numeric' });
}

/// The time one step after `at`: a step of days by the calendar, so that a mark stays at midnight.
function after(at: number, step: number): number {
  if (step < DAY) return at + step;
  const d = new Date(at);
  d.setDate(d.getDate() + Math.round(step / DAY));
  return d.getTime();
}

/// The first time after `start` that falls on a whole `step` of the local clock: a whole minute, hour or day.
function firstOn(start: number, step: number): number {
  const d = new Date(start);
  if (step < HOUR) {
    const m = step / MINUTE;
    d.setSeconds(0, 0);
    d.setMinutes(Math.floor(d.getMinutes() / m) * m);
  } else if (step < DAY) {
    const h = step / HOUR;
    d.setMinutes(0, 0, 0);
    d.setHours(Math.floor(d.getHours() / h) * h);
  } else {
    d.setHours(0, 0, 0, 0);
  }
  let at = d.getTime();
  while (at <= start) at = after(at, step);
  return at;
}

/// A mark on the axis of a work's wall time: where it is, as a share of the axis from 0 to 1, what it says, and
/// whether it is one of the two ends, which say when the work's time began and ended.
export type Tick = { at: number; label: string; end: 'start' | 'end' | null };

/// The marks of the axis under a work's wall time from `start` to `end` drawn `width` pixels wide: its two ends, with
/// the day when the time spans more than one, and between them the times on a whole step of the clock, the finest step
/// whose marks stand at least `least` pixels apart, so that no two labels run into each other; a mark at midnight says
/// the day, and one too near an end is left out.
export function axisTicks(start: number, end: number, width: number, least = 64): Tick[] {
  const span = end - start;
  const days = dayOf(start) !== dayOf(end);
  const first: Tick = { at: 0, label: days ? `${dayOf(start)} ${clockOf(start)}` : clockOf(start), end: 'start' };
  const last: Tick = { at: 1, label: days ? `${dayOf(end)} ${clockOf(end)}` : clockOf(end), end: 'end' };
  if (span <= 0 || width <= 0) return [first];
  let step = STEPS.find((s) => (s / span) * width >= least);
  if (step === undefined) {
    step = STEPS[STEPS.length - 1];
    while ((step / span) * width < least) step *= 2;
  }
  // An end that says the day is wider than a time of day: the marks keep further from it.
  const room = (days ? 1.5 : 1) * least;
  const marks: Tick[] = [];
  for (let at = firstOn(start, step); at < end; at = after(at, step)) {
    const x = (at - start) / span;
    if (x * width < room || (1 - x) * width < room) continue;
    const d = new Date(at);
    const midnight = d.getHours() === 0 && d.getMinutes() === 0;
    marks.push({ at: x, label: midnight && days ? dayOf(at) : clockOf(at), end: null });
  }
  return [first, ...marks, last];
}
