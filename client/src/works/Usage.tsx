// What a work used, as the old page's Usage tab showed it, from the rounds the work recorded: what it cost, how long it
// took, its rounds and tokens and the share read from the cache; where the time went, as one bar and as a line of time
// with a block for each round and each tool's call, each gap saying its own length; each model at the level and effort
// its rounds recorded; the cost by cell; each session of a line with why it began and its context after each round; and
// how long each tool ran. Every mark says what it is on pointing at it. It is shown in the work's panel when the owner
// opens it from what the view's line says the work has taken.

import type { ReactNode } from 'react';

import { Attr, ListRow, Panel, PanelPart } from '../parts';
import type { SessionWhy } from '../works-api';
import type { PanelProps } from './StepPanel';
import { type RoundOf, type SessionRow, type Span, usageOf } from './used';
import { clock, count, dollars, duration, share, tokens } from './words';

const pct = (part: number, whole: number) => (whole > 0 ? (100 * part) / whole : 0);

/// A round as the usage names it: its run and its number in the run, at its cell.
const roundName = (r: RoundOf) => `run ${r.run}, round ${r.number}, ${r.cell}`;

/// A number that stands for the whole work, under its name, with what more it says under it.
function Tile({ name, value, more }: { name: string; value: string; more?: ReactNode }) {
  return (
    <div className="usage-tile">
      <span className="label">{name}</span>
      <span className="usage-value">{value}</span>
      {more !== undefined && <span className="muted small">{more}</span>}
    </div>
  );
}

/// What a stretch of the work's time was, on pointing at it.
function spanWords(s: Span): string {
  const when = `${clock(s.start)} to ${clock(s.end)}, ${duration(s.end - s.start)}`;
  if (s.round) {
    const cost = s.round.spent ? `, ${dollars(s.round.spent.usdMicros)}` : s.round.underWay ? ', under way' : '';
    return `${roundName(s.round)}: ${when}${cost}`;
  }
  if (s.call && s.call.what.kind === 'tool') return `call ${s.call.call}, the tool ${s.call.what.name}: ${when}`;
  return `Not running: ${when}`;
}

/// The work's time from its first round's begin to its last round's end, a block for each stretch.
function Timeline({ spans, start, end }: { spans: Span[]; start: number; end: number }) {
  const whole = end - start;
  return (
    <div className="usage-timeline">
      <div className="us-track">
        {spans.map((s, i) => (
          <span
            key={i}
            className={`us-block us-${s.kind}`}
            title={spanWords(s)}
            style={{ left: `${pct(s.start - start, whole)}%`, width: `max(${pct(s.end - s.start, whole)}%, 2px)` }}
          />
        ))}
      </div>
      <div className="usage-axis muted small">
        <span>{clock(start)}</span>
        <span>{clock(end)}</span>
      </div>
    </div>
  );
}

/// How long models ran, the tools and the engine's acts, and how long nothing did, as one bar with its legend; the
/// tools and the engine's acts in one colour, as the line of time and the console's track draw them.
function TimeBar({ time }: { time: { model: number; tool: number; engine: number; idle: number } }) {
  const parts = [
    { kind: 'model', name: 'Models', ms: time.model },
    { kind: 'tool', name: "Tools and the engine's steps", ms: time.tool + time.engine },
    { kind: 'gap', name: 'Not running', ms: time.idle },
  ].filter((p) => p.ms > 0);
  const whole = parts.reduce((n, p) => n + p.ms, 0);
  return (
    <>
      <div className="us-track usage-stack">
        {parts.map((p) => (
          <span
            key={p.name}
            className={`us-block us-${p.kind}`}
            title={`${p.name}: ${duration(p.ms)}`}
            style={{ width: `${pct(p.ms, whole)}%` }}
          />
        ))}
      </div>
      <ul className="legend" aria-label="What the bar's colours mean">
        {parts.map((p) => (
          <li key={p.name}>
            <span className={`usage-swatch us-${p.kind}`} aria-hidden="true" /> {p.name} {duration(p.ms)}
          </li>
        ))}
      </ul>
    </>
  );
}

/// Bars of a share of a whole, each with its name and its value, the longest the whole width.
function Bars({ rows }: { rows: { key: string; name: ReactNode; value: number; text: string; tip: string }[] }) {
  const most = Math.max(0, ...rows.map((r) => r.value));
  return (
    <ul>
      {rows.map((r) => (
        <ListRow key={r.key}>
          <span className="usage-bar-name">{r.name}</span>
          <span className="usage-bar" title={r.tip}>
            <span style={{ width: `${pct(r.value, most)}%` }} />
          </span>
          <span className="mono">{r.text}</span>
        </ListRow>
      ))}
    </ul>
  );
}

/// Why a session began, by the kind the records name and the values it carries; the line's first began with it.
function WhyKeys({ why }: { why: SessionWhy | null }) {
  if (why === null) return <span className="muted">the line's first</span>;
  return (
    <span className="keys">
      <Attr name="why" value={why.kind} />
      {why.kind === 'limit' && (
        <>
          <Attr name="context" value={count(why.context)} />
          <Attr name="limit" value={count(why.limit)} />
        </>
      )}
      {why.kind === 'model' && <Attr name="was" value={why.was} />}
    </span>
  );
}

/// A session as the usage names it: its line, and its number among the line's.
const sessionName = (s: SessionRow) => `${s.line} ${s.number}`;

/// Each session's context after each of its rounds, a line for each session labelled at its end, a compaction marked
/// by a larger point; each point says its round and its context on pointing at it.
function ContextChart({ sessions, rounds }: { sessions: SessionRow[]; rounds: number }) {
  const points = sessions.flatMap((s) => s.points);
  if (points.length === 0) return <p className="muted small">No round recorded its context</p>;
  const [W, H, left, right, top, bottom] = [320, 120, 4, 60, 6, 6];
  const most = Math.max(...points.map((p) => p.context));
  const x = (at: number) => left + ((W - left - right) * at) / Math.max(rounds - 1, 1);
  const y = (context: number) => top + (H - top - bottom) * (1 - context / most);
  return (
    <svg className="usage-chart" viewBox={`0 0 ${W} ${H}`} role="img" aria-label="The context after each round">
      <line className="usage-grid" x1={left} x2={W - right} y1={y(most)} y2={y(most)} />
      <line className="usage-grid" x1={left} x2={W - right} y1={y(0)} y2={y(0)} />
      <text className="usage-chart-text" x={W - right + 4} y={y(most) + 4}>
        {tokens(most)}
      </text>
      {sessions.map((s) => {
        const last = s.points[s.points.length - 1];
        return (
          <g key={sessionName(s)}>
            {s.points.length > 1 && (
              <polyline className="usage-line" points={s.points.map((p) => `${x(p.at)},${y(p.context)}`).join(' ')} />
            )}
            {s.points.map((p) => (
              <circle key={p.at} className="usage-point" cx={x(p.at)} cy={y(p.context)} r={p.compacted ? 4 : 2.5}>
                <title>
                  {`${roundName(p.round)}: ${sessionName(s)}, context_tokens ${count(p.context)}${p.compacted ? ', compacted' : ''}`}
                </title>
              </circle>
            ))}
            {last && (
              <text className="usage-chart-text" x={x(last.at) + 6} y={y(last.context) + 4}>
                {sessionName(s)}
              </text>
            )}
          </g>
        );
      })}
    </svg>
  );
}

/// What the work used.
export function UsagePanel({ page, fold }: PanelProps) {
  const u = usageOf(page, Date.now());
  const work = page.summary.number;
  const read = u.spent.inputTokens + u.spent.cachedTokens;
  const head = { label: `What work W${work} used`, id: `W${work}`, title: 'Usage', belongs: 'of the work' };
  if (u.rounds.length === 0) {
    return (
      <Panel {...head} {...fold}>
        <p className="muted">No round has begun</p>
      </Panel>
    );
  }
  return (
    <Panel {...head} {...fold}>
      <div className="usage-tiles">
        <Tile name="Cost" value={dollars(u.spent.usdMicros)} more={`${u.agentRounds} agent rounds`} />
        <Tile
          name="Time"
          value={duration(u.end - u.start)}
          more={`running ${duration(u.time.model + u.time.tool + u.time.engine)}`}
        />
        <Tile name="Rounds" value={String(u.rounds.length)} more={`${u.sentBack} sent back`} />
        <Tile name="Tokens read" value={tokens(read)} more={`${share(u.spent.cachedTokens, read)} from cache`} />
        <Tile name="Tokens written" value={tokens(u.spent.outputTokens)} />
        <Tile name="Compactions" value={String(u.compactions)} />
      </div>
      <PanelPart label="Where the time went">
        <TimeBar time={u.time} />
        <Timeline spans={u.spans} start={u.start} end={u.end} />
      </PanelPart>
      <PanelPart label="Models" count={u.models.length}>
        <ul>
          {u.models.map((m) => (
            <ListRow key={m.ran === null ? '' : `${m.ran.model} ${m.ran.level} ${m.ran.effort ?? ''}`}>
              {m.ran === null ? (
                <span className="muted">not recorded</span>
              ) : (
                <>
                  <Attr name="model" value={m.ran.model} />
                  <Attr name="level" value={m.ran.level} />
                  {m.ran.effort !== null && <Attr name="effort" value={m.ran.effort} />}
                </>
              )}
              <span className="row-note keys small">
                <Attr name="rounds" value={m.rounds} />
                <Attr name="time" value={duration(m.ms)} />
                <Attr name="input_tokens" value={tokens(m.inputTokens)} />
                <Attr name="cached_tokens" value={tokens(m.cachedTokens)} />
                <Attr name="output_tokens" value={tokens(m.outputTokens)} />
                <Attr name="cache_hit" value={share(m.cachedTokens, m.inputTokens + m.cachedTokens)} />
                <Attr name="cost" value={`${dollars(m.usdMicros)}, ${share(m.usdMicros, u.spent.usdMicros)}`} />
              </span>
            </ListRow>
          ))}
        </ul>
      </PanelPart>
      <PanelPart label="Where the cost went" count={u.cells.length}>
        <Bars
          rows={u.cells.map((c) => {
            const title = page.flow.cells.find((x) => x.id === c.cell)?.title;
            return {
              key: c.cell,
              name: (
                <>
                  <span className="mono">{c.cell}</span>
                  {title !== undefined && title !== c.cell && <span className="muted"> {title}</span>}
                </>
              ),
              value: c.usdMicros,
              text: dollars(c.usdMicros),
              tip: `${c.cell}: ${dollars(c.usdMicros)}, ${share(c.usdMicros, u.spent.usdMicros)} of the cost, ${c.rounds} rounds`,
            };
          })}
        />
      </PanelPart>
      <PanelPart label="Sessions" count={u.sessions.length}>
        <ul>
          {u.sessions.map((s) => (
            <ListRow key={sessionName(s)}>
              <Attr name="line" value={s.line} />
              <Attr name="session" value={s.number} />
              <WhyKeys why={s.why} />
              <span className="row-note keys small">
                <Attr name="rounds" value={s.rounds} />
                <Attr name="time" value={duration(s.ms)} />
                <Attr name="context_tokens" value={s.context === null ? '–' : tokens(s.context)} />
                <Attr name="compactions" value={s.compactions} />
                <Attr name="cost" value={dollars(s.usdMicros)} />
              </span>
            </ListRow>
          ))}
        </ul>
        <p className="usage-sub">Context after each round</p>
        <ContextChart sessions={u.sessions} rounds={u.rounds.length} />
      </PanelPart>
      {u.tools.length > 0 && (
        <PanelPart label="Tools" count={u.tools.length}>
          <Bars
            rows={u.tools.map((t) => ({
              key: t.name,
              name: <span className="mono">{t.name}</span>,
              value: t.ms,
              text: duration(t.ms),
              tip: `the tool ${t.name}: ${duration(t.ms)}, ${t.calls} calls`,
            }))}
          />
        </PanelPart>
      )}
    </Panel>
  );
}
