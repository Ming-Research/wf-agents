// What a work used, worked out from the rounds its page records and nothing else: what it cost, how long it took and
// where its time went, its tokens and the share read from the cache, each model by the level and effort its rounds
// recorded they ran at, the cost by cell, each session of a line with why it began and its context round by round,
// and how long each tool ran. A round under way counts to `now`. Pure, so that it is tested on its own.

import type { Flow } from '../api';
import type { CallRow, RanOn, RoundSession, SessionWhy, Spent, Step, WorkPage } from '../works-api';

/// A round of the work, with what the usage reads of it.
export interface RoundOf {
  run: number;
  cell: string;
  number: number;
  /// Whether its cell is an agent's, whose rounds are a model's time; an engine's act's are the tools'.
  agent: boolean;
  began: number;
  /// When it ended; `now` while it is under way.
  ended: number;
  underWay: boolean;
  spent: Spent | null;
  context: number | null;
  compactions: number;
  session: RoundSession | null;
  /// What its last try ran on; none where the records hold no try of it.
  ran: RanOn | null;
  /// Whether it was given why what the round before handed in was sent back.
  sentBack: boolean;
}

/// What a stretch of the work's time was: a model at work, the tools or the engine's acts, or nothing running.
export type SpanKind = 'model' | 'tool' | 'gap';

/// A stretch of the work's time, from its first round's begin to its last round's end.
export interface Span {
  kind: SpanKind;
  start: number;
  end: number;
  /// The round it is, for a model's or an engine act's stretch; none for a tool's call and for a gap.
  round: RoundOf | null;
  /// The call it is, for a tool's stretch.
  call: CallRow | null;
}

/// The rounds that ran on one model at one level and effort, as the rounds recorded them; `ran` none for the rounds
/// that recorded none.
export interface ModelRow {
  ran: RanOn | null;
  rounds: number;
  ms: number;
  inputTokens: number;
  cachedTokens: number;
  outputTokens: number;
  usdMicros: number;
}

export interface CellRow {
  cell: string;
  rounds: number;
  usdMicros: number;
}

/// A session of a line: why it began (none for the line's first), its rounds, and its context after each.
export interface SessionRow {
  line: string;
  number: number;
  why: SessionWhy | null;
  rounds: number;
  ms: number;
  /// The context its last round that knew it ended with.
  context: number | null;
  compactions: number;
  usdMicros: number;
  /// Each of its rounds that knew its context: its place among the work's rounds, the context it ended with, and
  /// whether the session was compacted during it.
  points: { at: number; context: number; compacted: boolean; round: RoundOf }[];
}

export interface ToolRow {
  name: string;
  calls: number;
  ms: number;
}

export interface Usage {
  /// Every round, in the order they began.
  rounds: RoundOf[];
  agentRounds: number;
  sentBack: number;
  spent: Spent;
  compactions: number;
  /// From the first round's begin to the last round's end.
  start: number;
  end: number;
  /// How long models, the tools and the engine's acts ran, and how long nothing did, in that span.
  time: { model: number; tool: number; engine: number; idle: number };
  /// The work's time in stretches, in order, each gap between two that ran a stretch of its own.
  spans: Span[];
  models: ModelRow[];
  cells: CellRow[];
  sessions: SessionRow[];
  tools: ToolRow[];
}

const nothing = (): Spent => ({ usdMicros: 0, inputTokens: 0, cachedTokens: 0, outputTokens: 0, workingMs: 0, rounds: 0 });

function add(a: Spent, b: Spent): Spent {
  return {
    usdMicros: a.usdMicros + b.usdMicros,
    inputTokens: a.inputTokens + b.inputTokens,
    cachedTokens: a.cachedTokens + b.cachedTokens,
    outputTokens: a.outputTokens + b.outputTokens,
    workingMs: a.workingMs + b.workingMs,
    rounds: a.rounds + b.rounds,
  };
}

/// Whether a cell is an agent's, by the flow the work runs on; a cell the flow no longer has is taken for one where a
/// try of its rounds was recorded.
function agentCell(flow: Flow, cell: string, tried: boolean): boolean {
  const c = flow.cells.find((x) => x.id === cell);
  return c ? c.kind === 'agent' : tried;
}

/// Every round of the work, in the order they began.
export function roundsOf(page: WorkPage, now: number): RoundOf[] {
  const out: RoundOf[] = [];
  for (const s of page.steps) {
    if (s.kind !== 'run') continue;
    for (const r of s.rounds) {
      out.push({
        run: s.run,
        cell: s.cell,
        number: r.number,
        agent: agentCell(page.flow, s.cell, r.tries.length > 0),
        began: r.began,
        ended: Math.max(r.began, r.ended ?? now),
        underWay: r.ended === null,
        spent: r.spent,
        context: r.context,
        compactions: r.compactions,
        session: r.session,
        ran: r.tries[r.tries.length - 1]?.ran ?? null,
        sentBack: r.given.kind === 'sentBack',
      });
    }
  }
  return out.sort((a, b) => a.began - b.began);
}

/// Each stretch of the work's time from its first round's begin to its last round's end: a block for each round and
/// each tool's call, and one for each stretch in which nothing ran.
export function spansOf(rounds: RoundOf[], calls: CallRow[], now: number): Span[] {
  const ran: Span[] = [
    ...rounds.map((r): Span => ({ kind: r.agent ? 'model' : 'tool', start: r.began, end: r.ended, round: r, call: null })),
    ...calls
      .filter((c) => c.what.kind === 'tool')
      .map((c): Span => ({ kind: 'tool', start: c.made, end: Math.max(c.made, c.done ?? now), round: null, call: c })),
  ].sort((a, b) => a.start - b.start || a.end - b.end);
  if (rounds.length === 0) return [];
  const first = Math.min(...rounds.map((r) => r.began));
  const last = Math.max(...rounds.map((r) => r.ended));
  const out: Span[] = [];
  let reached = first;
  for (const s of ran) {
    if (s.end < first || s.start > last) continue;
    if (s.start > reached) out.push({ kind: 'gap', start: reached, end: s.start, round: null, call: null });
    out.push(s);
    reached = Math.max(reached, s.end);
  }
  return out;
}

/// Why each session of the line after its first began: the line's new sessions of an id of their own, in order; a
/// session lost and begun again, or compacted, goes on as the one it was.
function whyOf(steps: Step[], line: string): SessionWhy[] {
  return steps.flatMap((s) =>
    s.kind === 'session' && s.line === line && s.why.kind !== 'lost' && s.why.kind !== 'compacted' ? [s.why] : [],
  );
}

/// What the work used, from its page.
export function usageOf(page: WorkPage, now: number): Usage {
  const rounds = roundsOf(page, now);
  const calls = page.steps.flatMap((s) => (s.kind === 'run' ? s.calls : []));
  const spans = spansOf(rounds, calls, now);
  const start = rounds.length > 0 ? Math.min(...rounds.map((r) => r.began)) : 0;
  const end = rounds.length > 0 ? Math.max(...rounds.map((r) => r.ended)) : 0;
  const length = (s: Span) => Math.max(0, s.end - s.start);
  const time = {
    model: rounds.filter((r) => r.agent).reduce((n, r) => n + (r.ended - r.began), 0),
    engine: rounds.filter((r) => !r.agent).reduce((n, r) => n + (r.ended - r.began), 0),
    tool: spans.filter((s) => s.call !== null).reduce((n, s) => n + length(s), 0),
    idle: spans.filter((s) => s.kind === 'gap').reduce((n, s) => n + length(s), 0),
  };

  const models = new Map<string, ModelRow>();
  const cells = new Map<string, CellRow>();
  for (const r of rounds) {
    const cell = cells.get(r.cell) ?? { cell: r.cell, rounds: 0, usdMicros: 0 };
    cell.rounds += 1;
    cell.usdMicros += r.spent?.usdMicros ?? 0;
    cells.set(r.cell, cell);
    if (!r.agent || r.spent === null) continue;
    const key = r.ran === null ? '' : `${r.ran.model}\u0000${r.ran.level}\u0000${r.ran.effort ?? ''}`;
    const m = models.get(key) ?? {
      ran: r.ran,
      rounds: 0,
      ms: 0,
      inputTokens: 0,
      cachedTokens: 0,
      outputTokens: 0,
      usdMicros: 0,
    };
    m.rounds += 1;
    m.ms += r.ended - r.began;
    m.inputTokens += r.spent.inputTokens;
    m.cachedTokens += r.spent.cachedTokens;
    m.outputTokens += r.spent.outputTokens;
    m.usdMicros += r.spent.usdMicros;
    models.set(key, m);
  }

  const sessions = new Map<string, SessionRow>();
  rounds.forEach((r, at) => {
    if (r.session === null) return;
    const { line, number } = r.session;
    const key = `${line}\u0000${number}`;
    const s = sessions.get(key) ?? {
      line,
      number,
      why: number > 1 ? (whyOf(page.steps, line)[number - 2] ?? null) : null,
      rounds: 0,
      ms: 0,
      context: null,
      compactions: 0,
      usdMicros: 0,
      points: [],
    };
    s.rounds += 1;
    s.ms += r.ended - r.began;
    s.compactions += r.compactions;
    s.usdMicros += r.spent?.usdMicros ?? 0;
    if (r.context !== null) {
      s.context = r.context;
      s.points.push({ at, context: r.context, compacted: r.compactions > 0, round: r });
    }
    sessions.set(key, s);
  });

  const tools = new Map<string, ToolRow>();
  for (const c of calls) {
    if (c.what.kind !== 'tool') continue;
    const t = tools.get(c.what.name) ?? { name: c.what.name, calls: 0, ms: 0 };
    t.calls += 1;
    t.ms += Math.max(0, (c.done ?? now) - c.made);
    tools.set(c.what.name, t);
  }

  const byCost = <T extends { usdMicros: number }>(rows: T[]) => rows.sort((a, b) => b.usdMicros - a.usdMicros);
  return {
    rounds,
    agentRounds: rounds.filter((r) => r.agent).length,
    sentBack: rounds.filter((r) => r.sentBack).length,
    spent: rounds.reduce((s, r) => (r.spent ? add(s, r.spent) : s), nothing()),
    compactions: rounds.reduce((n, r) => n + r.compactions, 0),
    start,
    end,
    time,
    spans,
    models: byCost([...models.values()]),
    cells: byCost([...cells.values()]),
    sessions: [...sessions.values()],
    tools: [...tools.values()].sort((a, b) => b.ms - a.ms),
  };
}
