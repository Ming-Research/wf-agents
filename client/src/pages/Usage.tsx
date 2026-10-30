// A work's usage tab, as the owner's approved mock drew it from work 9: tiles of its cost, time, rounds, tokens and
// compactions; where its time went, as one stacked bar and as a timeline of its wall time with a block per round; its
// models, each at the level and effort it ran at; where its cost went, by kind of token and by cell; its sessions, each
// with why it started and its context after each round; the keep-warm pings sent to them while the work waited; and
// how long each tool ran. Every mark says what it is on hovering; labels are
// in HTML so that they keep their size as the charts shrink with the panel; colours come from the page's tokens, in
// light and dark alike.

import { type ReactNode, type RefObject, useEffect, useRef, useState } from 'react';

import type { CompiledGraph, WorkDetail } from '../api';
import { Bars, pct, share, Stack, Tile, useTip } from '../charts';
import { cost, duration, roundName } from '../format';
import { roundAtText } from '../parts';
import { tokenCount, tokens } from '../round';
import { t, tn } from '../text';
import { axisTicks, clockOf, type Gap, gapsOf, type SessionRow, type Usage as UsageData, usageOf } from '../usage';
import { modelName } from './RoundView';

/// A time of day, hours and minutes.
const clock = clockOf;

/// How wide an element is drawn now, followed as the panel it is in is dragged.
function useWidth<T extends HTMLElement>(): [RefObject<T | null>, number] {
  const ref = useRef<T>(null);
  const [width, setWidth] = useState(0);
  useEffect(() => {
    const el = ref.current;
    if (el === null) return;
    setWidth(el.getBoundingClientRect().width);
    const seen = new ResizeObserver((entries) => setWidth(entries[0]?.contentRect.width ?? 0));
    seen.observe(el);
    return () => seen.disconnect();
  }, []);
  return [ref, width];
}

/// The work's wall time, from its first round's start to its last round's end, a block for each round of a model or
/// of the tools over the time no round ran, with its times marked under it at a step its width has room for.
function Timeline({ u, work, gaps }: { u: UsageData; work: number; gaps: Gap[] }) {
  const x = (ms: number) => pct(ms - u.start, u.wall);
  const [axis, width] = useWidth<HTMLDivElement>();
  const ticks = axisTicks(u.start, u.end, width);
  return (
    <div className="us-timeline">
      <div className="us-track">
        {gaps.map((g) => (
          <span
            key={`gap-${g.before}-${g.after}`}
            className="us-block us-gap"
            style={{ left: `${x(g.start)}%`, width: `max(${pct(g.end - g.start, u.wall)}%, 2px)` }}
            data-tip={[
              t('us_gap_tip', {
                from: clock(g.start),
                to: clock(g.end),
                time: duration(g.end - g.start),
                before: roundName(work, g.before),
                after: roundName(work, g.after),
              }),
              ...g.waited,
            ].join(' · ')}
          />
        ))}
        {u.blocks
          .filter((b) => b.doer !== 'engine')
          .map((b) => (
            <span
              key={b.n}
              className={b.doer === 'model' ? 'us-block us-model' : 'us-block us-tool'}
              style={{ left: `${x(b.start)}%`, width: `max(${pct(b.end - b.start, u.wall)}%, 2px)` }}
              data-tip={[
                roundAtText(roundName(work, b.n), b.cell, b.title),
                duration(b.end - b.start),
                b.cost !== null ? cost(b.cost) : null,
              ]
                .filter((s) => s !== null)
                .join(' · ')}
            />
          ))}
      </div>
      <div className="us-axis" ref={axis}>
        {ticks.map((k) => (
          <span
            key={`${k.end ?? ''}${k.at}`}
            className={k.end === 'end' ? 'us-axis-end' : k.end === null ? 'us-tick' : undefined}
            style={{ left: `${100 * k.at}%` }}
          >
            {k.label}
          </span>
        ))}
      </div>
    </div>
  );
}

/// What the record shows a work waited on, with when it ended: each answer to an ask of the owner and each ruling on
/// a design card.
function waits(d: WorkDetail): Array<{ at: number; what: string }> {
  const out: Array<{ at: number; what: string }> = [];
  for (const a of d.asks) {
    const answer = a.answer;
    if (answer !== null && typeof answer === 'object' && !Array.isArray(answer) && typeof answer.at === 'number') {
      out.push({ at: answer.at, what: t('us_gap_ask', { id: a.id, at: clock(answer.at) }) });
    }
  }
  for (const c of d.design_cards) {
    for (const r of Array.isArray(c.rulings) ? c.rulings : []) {
      if (r !== null && typeof r === 'object' && !Array.isArray(r) && typeof r.at === 'number') {
        out.push({ at: r.at, what: t('us_gap_card', { id: c.id, at: clock(r.at) }) });
      }
    }
  }
  return out;
}

const LINE_COLOURS = ['var(--chart-1)', 'var(--chart-2)', 'var(--chart-3)', 'var(--chart-4)'];

/// A session as the tab names it: its line, and its number among the line's when the line had more than one.
function sessionName(u: UsageData, s: { line: string; nth: number }): string {
  return u.sessions.filter((x) => x.line === s.line).length > 1 ? `${s.line} ${s.nth}` : s.line;
}

/// Why a session started, in a few words.
function startedText(s: SessionRow): string {
  switch (s.started) {
    case 'cell':
      return t('us_started_cell', { cell: s.cell ?? '' });
    case 'limit':
      return s.before !== null ? t('us_started_limit_at', { n: tokens(s.before) }) : t('us_started_limit');
    case 'carried':
      return t('us_started_carried');
    case 'model':
      return t('us_started_model');
    case 'owner':
      return t('us_started_owner');
    case 'line':
      return s.cell !== null && s.line === s.cell ? t('us_started_own') : t('us_started_line');
    default:
      return '';
  }
}

/// Each session's context after each of its rounds, a line per session labelled at its end, a compaction marked
/// where one happened; the scale and the labels in HTML over the drawing.
function ContextChart({ u, work, colour }: { u: UsageData; work: number; colour: (line: string) => string }) {
  const W = 760;
  const H = 180;
  const points = u.sessions.flatMap((s) => s.points);
  if (points.length === 0) return null;
  const ns = u.blocks.map((b) => b.n);
  const nMin = Math.min(...ns);
  const nMax = Math.max(...ns);
  const top = Math.max(...points.map((p) => p.context));
  const step = top > 400_000 ? 200_000 : top > 100_000 ? 100_000 : 25_000;
  const yMax = Math.ceil(top / step) * step;
  const left = 6;
  const right = 90;
  const X = (n: number) => left + ((W - left - right) * (n - nMin)) / Math.max(nMax - nMin, 1);
  const Y = (v: number) => 8 + (H - 24) * (1 - v / yMax);
  const grid: number[] = [];
  for (let v = 0; v <= yMax; v += step) grid.push(v);
  return (
    <div className="us-chart" style={{ aspectRatio: `${W} / ${H}` }}>
      <svg viewBox={`0 0 ${W} ${H}`} width="100%" height="100%">
        {grid.map((v) => (
          <line key={v} x1={left} x2={W - right} y1={Y(v)} y2={Y(v)} className="us-grid" />
        ))}
        {u.sessions.map((s) => (
          <g key={s.id ?? s.line}>
            {s.points.length > 1 ? (
              <polyline
                fill="none"
                stroke={colour(s.line)}
                strokeWidth={2}
                points={s.points.map((p) => `${X(p.n)},${Y(p.context)}`).join(' ')}
              />
            ) : null}
            {s.points.map((p) => (
              <circle
                key={p.n}
                cx={X(p.n)}
                cy={Y(p.context)}
                r={p.compacted ? 6 : 4}
                fill={colour(s.line)}
                className={p.compacted ? 'us-point us-compacted' : 'us-point'}
                data-tip={`${roundName(work, p.n)} · ${sessionName(u, s)} · ${t('us_context_tip', { n: tokens(p.context) })}${p.compacted ? ` · ${t('us_compacted')}` : ''}`}
              />
            ))}
          </g>
        ))}
      </svg>
      {grid.map((v) => (
        <span key={v} className="us-y" style={{ top: `${(100 * Y(v)) / H}%` }}>
          {tokens(v)}
        </span>
      ))}
      {u.sessions.map((s) => {
        const p = s.points[s.points.length - 1];
        if (p === undefined) return null;
        return (
          <span
            key={s.id ?? s.line}
            className="us-end-label"
            style={{ left: `${(100 * (X(p.n) + 8)) / W}%`, top: `${(100 * Y(p.context)) / H}%`, color: colour(s.line) }}
          >
            {sessionName(u, s)}
          </span>
        );
      })}
      <span className="us-x" style={{ left: `${(100 * X(nMin)) / W}%` }}>
        {roundName(work, nMin)}
      </span>
      <span className="us-x us-x-end" style={{ left: `${(100 * X(nMax)) / W}%` }}>
        {roundName(work, nMax)}
      </span>
    </div>
  );
}

/// A part of the tab: its heading as a section's, with the rule under it, a note on what it shows, and what it shows.
function Section({ title, note, children }: { title: string; note?: string; children: ReactNode }) {
  return (
    <section className="us-sec">
      <h3 className="us-sec-title">{title}</h3>
      {note !== undefined ? <p className="us-note">{note}</p> : null}
      {children}
    </section>
  );
}

/// The usage of a work, from its rounds.
export function Usage({ d, g }: { d: WorkDetail; g: CompiledGraph }) {
  const tip = useTip();
  const cells = new Map(g.cells.map((c) => [c.id, { kind: c.kind, title: c.title }]));
  const u = usageOf(d.rounds, cells, Date.now(), d.sessions, d.pings);
  const work = d.work.id;
  const lines = [...new Set(u.sessions.map((s) => s.line))];
  const colour = (line: string) => LINE_COLOURS[Math.max(lines.indexOf(line), 0) % LINE_COLOURS.length];
  const tk = u.tokens;
  const readAll = tk.read + tk.written + tk.input;
  const k = u.kinds;
  const kTotal = k !== null ? k.input + k.read + k.written + k.output : 0;
  const top = u.cells.slice(0, 8);
  const rest = u.cells.slice(8);
  if (d.rounds.length === 0) return <p className="muted">{t('no_rounds')}</p>;
  return (
    <div className="us" onMouseMove={tip.move} onMouseLeave={tip.leave}>
      <div className="us-tiles">
        <Tile
          k={t('us_cost')}
          v={cost(u.cost)}
          s={
            u.pings.length > 0
              ? t('us_agent_rounds_pings', {
                  rounds: tn('us_agent_rounds', u.agentRounds),
                  pings: tn('us_pings', u.pings.length),
                  cost: cost(u.pingCost),
                })
              : tn('us_agent_rounds', u.agentRounds)
          }
        />
        <Tile k={t('us_time')} v={duration(u.wall)} s={t('us_running', { time: duration(u.time.model + u.time.tool + u.time.engine) })} />
        <Tile k={t('us_rounds')} v={String(u.rounds)} s={t('us_sent_back', { n: u.sentBack })} />
        <Tile k={t('us_read')} v={tokens(readAll)} s={t('us_from_cache', { share: share(tk.read, readAll) })} />
        <Tile k={t('us_written')} v={tokens(tk.output)} s={t('us_thinking', { n: tokens(tk.thinking) })} />
        <Tile k={t('us_compactions')} v={String(u.compactions)} s={tn('us_turns', tk.turns)} />
      </div>
      <Section title={t('us_time_title')} note={t('us_time_note')}>
        <Stack
          parts={[
            { value: u.time.model, colour: 'var(--chart-1)', label: t('us_models'), tip: `${t('us_models')} · ${duration(u.time.model)}` },
            { value: u.time.tool, colour: 'var(--chart-2)', label: t('us_tools'), tip: `${t('us_tools')} · ${duration(u.time.tool)}` },
            ...(u.time.engine >= 1000
              ? [{ value: u.time.engine, colour: 'var(--chart-3)', label: t('us_engine'), tip: `${t('us_engine')} · ${duration(u.time.engine)}` }]
              : []),
            { value: u.time.idle, colour: 'var(--chart-idle)', label: t('us_not_running'), tip: t('us_not_running_tip', { time: duration(u.time.idle) }) },
          ]}
        />
        <Timeline u={u} work={work} gaps={gapsOf(u.blocks, waits(d))} />
      </Section>
      <Section
        title={t('us_models_title')}
        note={
          u.subagents.length === 0
            ? t('us_models_note_none')
            : t('us_models_note', {
                list: u.subagents.map((s) => `${modelName(s.model)} (${s.rounds.map((n) => roundName(work, n)).join(', ')})`).join('; '),
              })
        }
      >
        <table className="list us-table">
          <thead>
            <tr>
              <th>{t('us_col_model')}</th>
              <th className="num">{t('us_col_rounds')}</th>
              <th className="num">{t('us_col_turns')}</th>
              <th className="num">{t('us_col_time')}</th>
              <th className="num">{t('us_col_read')}</th>
              <th className="num">{t('us_col_written')}</th>
              <th className="num">{t('us_col_output')}</th>
              <th className="num">{t('us_col_cost')}</th>
            </tr>
          </thead>
          <tbody>
            {u.models.map((m) => (
              <tr key={`${m.model}-${m.effort ?? ''}-${m.level ?? ''}`}>
                <td>
                  {modelName(m.model)}
                  <div className="us-sub">
                    {m.level !== null ? t('us_level', { level: m.level }) : t('us_level_unrecorded')}
                    {m.effort !== null ? ` · ${t('us_effort', { effort: m.effort })}` : ''}
                  </div>
                </td>
                <td className="num">{m.rounds}</td>
                <td className="num">{m.turns}</td>
                <td className="num">{duration(m.ms)}</td>
                <td className="num">{tokens(m.read)}</td>
                <td className="num">{tokens(m.written)}</td>
                <td className="num">
                  {tokens(m.output)}
                  <div className="us-sub">{t('us_thinking', { n: tokens(m.thinking) })}</div>
                </td>
                <td className="num" data-tip={t('us_share_tip', { share: share(m.cost, u.cost) })}>
                  {cost(m.cost)}
                  <div className="us-mini">
                    <div style={{ width: `${pct(m.cost, u.cost)}%` }} />
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </Section>
      <Section title={t('us_cost_title')} note={t('us_cost_note')}>
        {k !== null ? (
          <Stack
            parts={[
              { value: k.read, colour: 'var(--share-read)', label: t('us_kind_read', { share: share(k.read, kTotal) }), tip: t('us_kind_read_tip', { tokens: tokenCount(tk.read), share: share(k.read, kTotal) }) },
              { value: k.written, colour: 'var(--share-new)', label: t('us_kind_written', { share: share(k.written, kTotal) }), tip: t('us_kind_written_tip', { tokens: tokenCount(tk.written), share: share(k.written, kTotal) }) },
              { value: k.output, colour: 'var(--share-output)', label: t('us_kind_output', { share: share(k.output, kTotal) }), tip: t('us_kind_output_tip', { tokens: tokenCount(tk.output), share: share(k.output, kTotal) }) },
              { value: k.input, colour: 'var(--chart-idle)', label: t('us_kind_input', { share: share(k.input, kTotal) }), tip: t('us_kind_input_tip', { tokens: tokenCount(tk.input), share: share(k.input, kTotal) }) },
            ]}
          />
        ) : null}
        <h4 className="sub-heading us-part">{t('us_by_cell')}</h4>
        <Bars
          colour="var(--chart-1)"
          items={[
            ...top.map((c) => ({
              label: `${c.cell} ${c.title}`,
              value: c.cost,
              text: cost(c.cost),
              tip: `${c.cell} ${c.title} · ${cost(c.cost)} · ${tn('us_n_rounds', c.rounds)}`,
            })),
            ...(rest.length > 0
              ? [
                  {
                    label: tn('us_other_cells', rest.length),
                    value: rest.reduce((a, c) => a + c.cost, 0),
                    text: cost(rest.reduce((a, c) => a + c.cost, 0)),
                    tip: rest.map((c) => `${c.cell} ${cost(c.cost)}`).join(' · '),
                  },
                ]
              : []),
          ]}
        />
      </Section>
      <Section title={t('us_sessions_title')} note={t('us_sessions_note')}>
        <table className="list us-table">
          <thead>
            <tr>
              <th>{t('us_col_line')}</th>
              <th>{t('us_col_started')}</th>
              <th className="num">{t('us_col_rounds')}</th>
              <th className="num">{t('us_col_turns')}</th>
              <th className="num">{t('us_col_time')}</th>
              <th className="num">{t('us_col_context')}</th>
              <th className="num">{t('us_col_compactions')}</th>
              <th className="num">{t('us_col_cost')}</th>
            </tr>
          </thead>
          <tbody>
            {u.sessions.map((s) => (
              <tr key={s.id ?? s.line}>
                <td>
                  <i className="us-swatch" style={{ background: colour(s.line) }} />
                  {sessionName(u, s)}
                </td>
                <td>{startedText(s)}</td>
                <td className="num">{s.rounds}</td>
                <td className="num">{s.turns}</td>
                <td className="num">{duration(s.ms)}</td>
                <td className="num">
                  {s.context !== null ? tokens(s.context) : ''}
                  {s.window !== null ? <span className="us-sub"> {t('us_of_window', { n: tokens(s.window) })}</span> : null}
                </td>
                <td className="num">{s.compactions}</td>
                <td className="num">
                  {cost(s.cost)}
                  {s.pings > 0 ? <div className="us-sub">{tn('us_with_pings', s.pings)}</div> : null}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        <h4 className="sub-heading us-part">{t('us_context_title')}</h4>
        <ContextChart u={u} work={work} colour={colour} />
      </Section>
      {u.pings.length > 0 ? (
        <Section title={t('us_pings_title')} note={t('us_pings_note')}>
          <table className="list us-table">
            <thead>
              <tr>
                <th>{t('us_col_when')}</th>
                <th>{t('us_col_session')}</th>
                <th>{t('us_col_waited')}</th>
                <th className="num">{t('us_col_read')}</th>
                <th className="num">{t('us_col_written')}</th>
                <th className="num">{t('us_col_cost')}</th>
              </tr>
            </thead>
            <tbody>
              {u.pings.map((p) => (
                <tr key={p.id}>
                  <td>{clock(p.started_at)}</td>
                  <td>
                    <i className="us-swatch" style={{ background: colour(p.line) }} />
                    {sessionName(u, p)}
                  </td>
                  <td>
                    {t('us_waited', { cell: p.cell, on: p.on, what: p.what })}
                    {p.error !== null ? <div className="us-sub">{t('us_ping_failed', { why: p.error })}</div> : null}
                  </td>
                  <td className="num">{tokens(p.read)}</td>
                  <td className="num">{tokens(p.written)}</td>
                  <td className="num">
                    {p.cost_usd !== null ? cost(p.cost_usd) : ''}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </Section>
      ) : null}
      {u.tools.length > 0 ? (
        <Section title={t('us_tools_title')} note={t('us_tools_note')}>
          <Bars
            colour="var(--chart-2)"
            items={u.tools.map((x) => ({
              label: `${x.cell} ${x.title}`,
              value: x.ms,
              text: duration(x.ms),
              tip: `${x.cell} ${x.title} · ${duration(x.ms)} · ${tn('us_n_runs', x.runs)}`,
            }))}
          />
        </Section>
      ) : null}
      {tip.shown}
    </div>
  );
}
