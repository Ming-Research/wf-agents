// A work's usage tab, as the owner's approved mock drew it from work 9: tiles of its cost, time, rounds, tokens and
// compactions; where its time went, as one stacked bar and as a timeline of its wall time with a block per round; its
// models, each at the level and effort it ran at; where its cost went, by kind of token and by cell; its sessions, with
// each one's context after each round; and how long each tool ran. Every mark says what it is on hovering; labels are
// in HTML so that they keep their size as the charts shrink with the panel; colours come from the page's tokens, in
// light and dark alike.

import { type MouseEvent, type ReactNode, useState } from 'react';

import type { CompiledGraph, WorkDetail } from '../api';
import { cost, duration, roundName } from '../format';
import { tokens } from '../round';
import { t } from '../text';
import { type Usage as UsageData, usageOf } from '../usage';
import { modelName } from './RoundView';

const pct = (x: number, total: number) => (total > 0 ? (100 * x) / total : 0);
const share = (x: number, total: number) => `${Math.round(pct(x, total))}%`;

/// A time of day, hours and minutes.
function clock(ms: number): string {
  const d = new Date(ms);
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
}

/// One part of a stacked bar: its share, its colour and what it says on hovering.
type Part = { value: number; colour: string; tip: string; label: string };

/// A stacked bar across the width, in HTML so that it keeps its height, with its legend under it.
function Stack({ parts }: { parts: Part[] }) {
  const total = parts.reduce((a, p) => a + p.value, 0);
  return (
    <>
      <div className="us-stack">
        {parts
          .filter((p) => p.value > 0)
          .map((p, i) => (
            <span key={i} style={{ width: `${pct(p.value, total)}%`, background: p.colour }} data-tip={p.tip} />
          ))}
      </div>
      <div className="us-legend">
        {parts.map((p, i) => (
          <span key={i}>
            <i style={{ background: p.colour }} />
            {p.label}
          </span>
        ))}
      </div>
    </>
  );
}

/// Bars one under another, each its label, its bar and its value, the longest the full width.
function Bars({ items, colour }: { items: Array<{ label: string; value: number; text: string; tip: string }>; colour: string }) {
  const max = Math.max(...items.map((x) => x.value), 0);
  return (
    <div className="us-bars">
      {items.map((x, i) => (
        <div key={i} className="us-bar">
          <span className="us-bar-label" title={x.label}>
            {x.label}
          </span>
          <span className="us-bar-track">
            <span className="us-bar-fill" style={{ width: `${Math.max(pct(x.value, max), 0.6)}%`, background: colour }} data-tip={x.tip} />
            <span className="us-bar-value">{x.text}</span>
          </span>
        </div>
      ))}
    </div>
  );
}

/// The work's wall time, from its first round's start to its last round's end, a block for each round of a model or
/// of the tools over the time no round ran, with the hours marked under it.
function Timeline({ u, work }: { u: UsageData; work: number }) {
  const x = (ms: number) => pct(ms - u.start, u.wall);
  const hours: number[] = [];
  const first = new Date(u.start);
  first.setMinutes(0, 0, 0);
  for (let h = first.getTime() + 3_600_000; h < u.end; h += 3_600_000) hours.push(h);
  return (
    <div className="us-timeline">
      <div className="us-track" data-tip={t('us_not_running_tip', { time: duration(u.time.idle) })}>
        {u.blocks
          .filter((b) => b.doer !== 'engine')
          .map((b) => (
            <span
              key={b.n}
              className={b.doer === 'model' ? 'us-block us-model' : 'us-block us-tool'}
              style={{ left: `${x(b.start)}%`, width: `max(${pct(b.end - b.start, u.wall)}%, 2px)` }}
              data-tip={[
                `${roundName(work, b.n)} · ${b.cell} ${b.title}`,
                duration(b.end - b.start),
                b.cost !== null ? cost(b.cost) : null,
              ]
                .filter((s) => s !== null)
                .join(' · ')}
            />
          ))}
      </div>
      <div className="us-axis">
        <span style={{ left: '0%' }}>{clock(u.start)}</span>
        {hours.map((h) => (
          <span key={h} className="us-tick" style={{ left: `${x(h)}%` }}>
            {clock(h)}
          </span>
        ))}
        <span style={{ left: '100%' }} className="us-axis-end">
          {clock(u.end)}
        </span>
      </div>
    </div>
  );
}

const LINE_COLOURS = ['var(--us-1)', 'var(--us-2)', 'var(--us-3)', 'var(--us-4)'];

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
          <g key={s.line}>
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
                data-tip={`${roundName(work, p.n)} · ${s.line} · ${t('us_context_tip', { n: tokens(p.context) })}${p.compacted ? ` · ${t('us_compacted')}` : ''}`}
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
            key={s.line}
            className="us-end-label"
            style={{ left: `${(100 * (X(p.n) + 8)) / W}%`, top: `${(100 * Y(p.context)) / H}%`, color: colour(s.line) }}
          >
            {s.line}
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

function Tile({ k, v, s }: { k: string; v: string; s: string }) {
  return (
    <div className="us-tile">
      <div className="us-k">{k}</div>
      <div className="us-v">{v}</div>
      <div className="us-s">{s}</div>
    </div>
  );
}

function Section({ title, note, children }: { title: string; note?: string; children: ReactNode }) {
  return (
    <section className="us-sec">
      <h4>{title}</h4>
      {note !== undefined ? <p className="us-note">{note}</p> : null}
      {children}
    </section>
  );
}

/// The usage of a work, from its rounds.
export function Usage({ d, g }: { d: WorkDetail; g: CompiledGraph }) {
  const [tip, setTip] = useState<{ text: string; x: number; y: number } | null>(null);
  const cells = new Map(g.cells.map((c) => [c.id, { kind: c.kind, title: c.title }]));
  const u = usageOf(d.rounds, cells, Date.now());
  const work = d.work.id;
  const lines = u.sessions.map((s) => s.line);
  const colour = (line: string) => LINE_COLOURS[Math.max(lines.indexOf(line), 0) % LINE_COLOURS.length];
  const move = (e: MouseEvent<HTMLDivElement>) => {
    const at = (e.target as Element).closest('[data-tip]');
    const text = at?.getAttribute('data-tip') ?? null;
    setTip(text !== null ? { text, x: e.clientX, y: e.clientY } : null);
  };
  const tk = u.tokens;
  const readAll = tk.read + tk.written + tk.input;
  const k = u.kinds;
  const kTotal = k !== null ? k.input + k.read + k.written + k.output : 0;
  const top = u.cells.slice(0, 8);
  const rest = u.cells.slice(8);
  if (d.rounds.length === 0) return <p className="muted">{t('no_rounds')}</p>;
  return (
    <div className="us" onMouseMove={move} onMouseLeave={() => setTip(null)}>
      <div className="us-tiles">
        <Tile k={t('us_cost')} v={cost(u.cost)} s={t('us_agent_rounds', { n: u.agentRounds })} />
        <Tile k={t('us_time')} v={duration(u.wall)} s={t('us_running', { time: duration(u.time.model + u.time.tool + u.time.engine) })} />
        <Tile k={t('us_rounds')} v={String(u.rounds)} s={t('us_sent_back', { n: u.sentBack })} />
        <Tile k={t('us_read')} v={tokens(readAll)} s={t('us_from_cache', { share: share(tk.read, readAll) })} />
        <Tile k={t('us_written')} v={tokens(tk.output)} s={t('us_thinking', { n: tokens(tk.thinking) })} />
        <Tile k={t('us_compactions')} v={String(u.compactions)} s={t('us_turns', { n: tk.turns })} />
      </div>
      <Section title={t('us_time_title')} note={t('us_time_note')}>
        <Stack
          parts={[
            { value: u.time.model, colour: 'var(--us-1)', label: t('us_models'), tip: `${t('us_models')} · ${duration(u.time.model)}` },
            { value: u.time.tool, colour: 'var(--us-2)', label: t('us_tools'), tip: `${t('us_tools')} · ${duration(u.time.tool)}` },
            ...(u.time.engine >= 1000
              ? [{ value: u.time.engine, colour: 'var(--us-3)', label: t('us_engine'), tip: `${t('us_engine')} · ${duration(u.time.engine)}` }]
              : []),
            { value: u.time.idle, colour: 'var(--us-idle)', label: t('us_not_running'), tip: t('us_not_running_tip', { time: duration(u.time.idle) }) },
          ]}
        />
        <Timeline u={u} work={work} />
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
        <table className="us-table">
          <thead>
            <tr>
              <th>{t('us_col_model')}</th>
              <th>{t('us_col_rounds')}</th>
              <th>{t('us_col_turns')}</th>
              <th>{t('us_col_time')}</th>
              <th>{t('us_col_read')}</th>
              <th>{t('us_col_written')}</th>
              <th>{t('us_col_output')}</th>
              <th>{t('us_col_cost')}</th>
            </tr>
          </thead>
          <tbody>
            {u.models.map((m) => (
              <tr key={`${m.model}-${m.effort ?? ''}-${m.level ?? ''}`}>
                <td>
                  <b>{modelName(m.model)}</b>
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
                  <b>{cost(m.cost)}</b>
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
              { value: k.read, colour: 'var(--us-1)', label: t('us_kind_read', { share: share(k.read, kTotal) }), tip: t('us_kind_read_tip', { n: tokens(tk.read), share: share(k.read, kTotal) }) },
              { value: k.written, colour: 'var(--us-2)', label: t('us_kind_written', { share: share(k.written, kTotal) }), tip: t('us_kind_written_tip', { n: tokens(tk.written), share: share(k.written, kTotal) }) },
              { value: k.output, colour: 'var(--us-3)', label: t('us_kind_output', { share: share(k.output, kTotal) }), tip: t('us_kind_output_tip', { n: tokens(tk.output), share: share(k.output, kTotal) }) },
              { value: k.input, colour: 'var(--us-idle)', label: t('us_kind_input', { share: share(k.input, kTotal) }), tip: t('us_kind_input_tip', { n: tokens(tk.input), share: share(k.input, kTotal) }) },
            ]}
          />
        ) : null}
        <p className="us-note us-gap">{t('us_by_cell')}</p>
        <Bars
          colour="var(--us-1)"
          items={[
            ...top.map((c) => ({
              label: `${c.cell} ${c.title}`,
              value: c.cost,
              text: cost(c.cost),
              tip: `${c.cell} ${c.title} · ${cost(c.cost)} · ${t(c.rounds === 1 ? 'us_one_round' : 'us_n_rounds', { n: c.rounds })}`,
            })),
            ...(rest.length > 0
              ? [
                  {
                    label: t('us_other_cells', { n: rest.length }),
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
        <table className="us-table">
          <thead>
            <tr>
              <th>{t('us_col_line')}</th>
              <th>{t('us_col_rounds')}</th>
              <th>{t('us_col_turns')}</th>
              <th>{t('us_col_time')}</th>
              <th>{t('us_col_context')}</th>
              <th>{t('us_col_compactions')}</th>
              <th>{t('us_col_cost')}</th>
            </tr>
          </thead>
          <tbody>
            {u.sessions.map((s) => (
              <tr key={s.line}>
                <td>
                  <i className="us-swatch" style={{ background: colour(s.line) }} />
                  <b>{s.line}</b>
                </td>
                <td className="num">{s.rounds}</td>
                <td className="num">{s.turns}</td>
                <td className="num">{duration(s.ms)}</td>
                <td className="num">
                  {s.context !== null ? tokens(s.context) : ''}
                  {s.window !== null ? <span className="us-sub"> {t('us_of_window', { n: tokens(s.window) })}</span> : null}
                </td>
                <td className="num">{s.compactions}</td>
                <td className="num">
                  <b>{cost(s.cost)}</b>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        <p className="us-note us-gap">{t('us_context_title')}</p>
        <ContextChart u={u} work={work} colour={colour} />
      </Section>
      {u.tools.length > 0 ? (
        <Section title={t('us_tools_title')} note={t('us_tools_note')}>
          <Bars
            colour="var(--us-2)"
            items={u.tools.map((x) => ({
              label: `${x.cell} ${x.title}`,
              value: x.ms,
              text: duration(x.ms),
              tip: `${x.cell} ${x.title} · ${duration(x.ms)} · ${t(x.runs === 1 ? 'us_one_run' : 'us_n_runs', { n: x.runs })}`,
            }))}
          />
        </Section>
      ) : null}
      {tip !== null ? (
        <div className="us-tip" style={{ left: Math.min(tip.x + 12, window.innerWidth - 440), top: tip.y + 14 }}>
          {tip.text}
        </div>
      ) : null}
    </div>
  );
}
