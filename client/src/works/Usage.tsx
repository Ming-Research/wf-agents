// What a work used, from the rounds the work recorded, each by the keys the records keep it under and what the page
// works out from them said plainly: what it took and the share read from the cache; where the time went, as one bar
// and as a line of time with a block for each round and each tool's call, each gap saying its own length; each model
// at the level and effort its rounds recorded; the cost by cell; each session of a line with why it began and its
// context after each round; and how long each tool ran. Every mark says what it is on pointing at it. It is shown in
// the work's panel when the owner opens it from what the view's line says the work has taken.

import type { ReactNode } from 'react';

import { Attr, ListRow, Panel, PanelPart } from '../parts';
import type { SessionWhy } from '../works-api';
import { Keys } from './Round';
import type { PanelProps } from './StepPanel';
import { type RoundOf, type SessionRow, type Span, usageOf } from './used';
import { DOLLARS, type Key, clock, count, dollars, duration, keysLine, share, spentKeys, tokens } from './words';

const pct = (part: number, whole: number) => (whole > 0 ? (100 * part) / whole : 0);

/// How long rounds were under way together, said plainly.
const UNDER_WAY = 'under way';

/// A round by its keys: its run, its number in the run, and its cell.
const roundKeys = (r: RoundOf): Key[] => [
  ['run', String(r.run)],
  ['round', String(r.number)],
  ['cell', r.cell],
];

/// When a stretch began and ended, and how long it was.
const whenKeys = (s: Span): Key[] => [
  ['from', clock(s.start)],
  ['to', clock(s.end)],
  [UNDER_WAY, duration(s.end - s.start)],
];

/// What a stretch of the work's time was, on pointing at it: a round, a tool's call, or a gap, which says its length.
function spanTip(s: Span): string {
  if (s.round) {
    const took: Key[] = s.round.spent ? [[DOLLARS, dollars(s.round.spent.usdMicros)]] : [];
    return keysLine([...roundKeys(s.round), ...whenKeys(s), ...took]);
  }
  if (s.call && s.call.what.kind === 'tool') {
    return keysLine([['call', String(s.call.call)], ['tool', s.call.what.name], ...whenKeys(s)]);
  }
  return `Not running: ${keysLine([['from', clock(s.start)], ['to', clock(s.end)], ['for', duration(s.end - s.start)]])}`;
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
            title={spanTip(s)}
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
  const keys: Key[] = [['why', why.kind]];
  if (why.kind === 'limit') keys.push(['context', count(why.context)], ['limit', count(why.limit)]);
  if (why.kind === 'model') keys.push(['was', why.was]);
  return <Keys keys={keys} />;
}

/// A session as the usage names it: its line, and its number among the line's as the records count them.
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
                  {keysLine([
                    ...roundKeys(p.round),
                    ['line', s.line],
                    ['session', String(s.number)],
                    ['context_tokens', count(p.context)],
                    ...(p.compacted ? ([['compactions', String(p.round.compactions)]] as Key[]) : []),
                  ])}
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
  const head = { label: `What work W${work} used`, id: `W${work}`, title: 'Usage', belongs: 'of the work' };
  if (u.rounds.length === 0) {
    return (
      <Panel {...head} {...fold}>
        <p className="muted">No round has begun</p>
      </Panel>
    );
  }
  const whole: Key[] = [
    ['rounds', String(u.rounds.length)],
    ["rounds of agents' cells", String(u.agentRounds)],
    ['sent_back', String(u.sentBack)],
    ['compactions', String(u.compactions)],
    ["first round's begin to last round's end", duration(u.end - u.start)],
    [UNDER_WAY, duration(u.time.model + u.time.tool + u.time.engine)],
  ];
  return (
    <Panel {...head} {...fold}>
      <div className="usage-totals">
        <Keys keys={spentKeys(u.spent)} />
        <Keys keys={whole} />
        {page.summary.pinged && page.summary.pinged.rounds > 0 && (
          <Keys keys={[['keep-warm pings', String(page.summary.pinged.rounds)], ...spentKeys(page.summary.pinged)]} />
        )}
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
              <span className="row-note small">
                <Keys
                  keys={[
                    ['rounds', String(m.spent.rounds)],
                    [UNDER_WAY, duration(m.spent.workingMs)],
                    ...spentKeys(m.spent),
                  ]}
                />
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
              tip: keysLine([
                ['cell', c.cell],
                [DOLLARS, dollars(c.usdMicros)],
                ['share of the cost', share(c.usdMicros, u.spent.usdMicros)],
                ['rounds', String(c.rounds)],
              ]),
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
              <span className="row-note small">
                <Keys
                  keys={[
                    ['rounds', String(s.spent.rounds)],
                    [UNDER_WAY, duration(s.spent.workingMs)],
                    ['context_tokens', s.context === null ? '–' : count(s.context)],
                    ['compactions', String(s.compactions)],
                    ...spentKeys(s.spent),
                  ]}
                />
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
              tip: keysLine([
                ['tool', t.name],
                [UNDER_WAY, duration(t.ms)],
                ['calls', String(t.calls)],
              ]),
            }))}
          />
        </PanelPart>
      )}
    </Panel>
  );
}
