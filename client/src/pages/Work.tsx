// A work, for diagnosis: the state machine of the flow version it is pinned to, filling the window under the bar,
// with what the work did on it: the cell it is at ringed, each cell it visited with its rounds and the rounds sent
// back or failed, the arrows it took standing out. The bar holds the work's facts; its rounds are listed on the left,
// and a round opens in the panel with its cell brought into view, a cell with its rounds.

import { useEffect, useRef, useState } from 'react';

import type { Compiled, FlowView, NextRound, RoundSummary, WorkDetail } from '../api';
import { InBar } from '../bar';
import { Json, Loaded, OutcomeBadge, Section, WorkBadge } from '../components';
import { FlowScreen } from '../flow/FlowScreen';
import { cost, duration } from '../format';
import { overlays, taken, chart } from '../graph/flowModel';
import { postJson } from '../http';
import { useActivity, useResource, useStatus } from '../live';
import { href, Link, navigate } from '../route';
import { t } from '../text';
import { RoundView } from './RoundView';

function Timeline(props: { rounds: RoundSummary[]; selected: number | null; onRound: (r: RoundSummary) => void }) {
  if (props.rounds.length === 0) return <p className="muted">{t('no_rounds')}</p>;
  const newest = [...props.rounds].reverse();
  return (
    <table className="list timeline">
      <tbody>
        {newest.map((r) => (
          <tr
            key={r.id}
            className={`clickable${r.id === props.selected ? ' selected' : ''}`}
            onClick={() => props.onRound(r)}
          >
            <td className="muted">#{r.id}</td>
            <td>
              <span className="cell-id">{r.cell ?? '–'}</span>
            </td>
            <td>
              <OutcomeBadge outcome={r.outcome} />
            </td>
            <td className="clip" title={r.transition ?? r.why ?? ''}>
              {r.transition ?? r.why ?? ''}
            </td>
            <td className="num muted">{r.ended_at !== null ? duration(r.ended_at - r.started_at) : ''}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function Bar({ d }: { d: WorkDetail }) {
  const w = d.work;
  const facts = [
    t('work_flow', { version: w.flow_version, graph: w.graph }),
    w.stage ? t('work_stage', { stage: w.stage }) : null,
    t('work_rounds', { n: w.rounds }),
    w.sent_back > 0 ? t('sent_back_n', { n: w.sent_back }) : null,
    w.failed > 0 ? t('failed_n', { n: w.failed }) : null,
    w.open_asks > 0 ? t('open_asks_n', { n: w.open_asks }) : null,
    w.open_design_cards > 0 ? t('open_design_cards_n', { n: w.open_design_cards }) : null,
    w.branch ? `${t('branch')}: ${w.branch}` : null,
    w.pr !== null ? `PR #${w.pr}` : null,
    w.paused ? t('paused_why', { why: w.paused }) : null,
    w.ended ? t('ended_why', { why: w.ended }) : null,
  ]
    .filter((x) => x !== null)
    .join('\n');
  return (
    <InBar>
      <div className="bar-work" title={facts}>
        <strong>{t('work_title', { id: w.id })}</strong>
        {w.item !== null ? (
          <Link to={`/items/${w.item}`} className="bar-item">
            #{w.item} {w.title}
          </Link>
        ) : null}
        <WorkBadge state={w.state} />
        <span className="muted">
          {t('work_at', { cell: w.at })} {w.at_title ?? ''}
        </span>
        {w.sent_back > 0 ? <span className="warn">↩{w.sent_back}</span> : null}
        {w.failed > 0 ? <span className="bad">✕{w.failed}</span> : null}
        {w.open_asks > 0 ? <span className="warn">{t('open_asks_n', { n: w.open_asks })}</span> : null}
        {w.open_design_cards > 0 ? (
          <span className="warn">{t('open_design_cards_n', { n: w.open_design_cards })}</span>
        ) : null}
        <span className="muted">{cost(w.cost_usd)}</span>
        <Link to={`/flow/${w.flow_version}`} className="muted">
          {t('work_flow_short', { version: w.flow_version })}
        </Link>
      </div>
    </InBar>
  );
}

/// The work's next round while the engine holds: what it will be, the button that runs it, and while it runs, how long
/// it has run and what the agent is doing; when it ends, its round opens.
function Next({ d, onRound }: { d: WorkDetail; onRound: (r: number) => void }) {
  const status = useStatus();
  const w = d.work;
  const activity = useActivity(w.id);
  const asked = useRef(false);
  const [error, setError] = useState<string | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const stepping = status?.stepping === w.id;
  const can = status !== null && status.mode === 'hold' && status.halt === null && status.stepping === null && w.state === 'ready';
  const next = useResource<NextRound>(`work:${w.id}`, can ? `/api/v1/works/${w.id}/next` : null);
  useEffect(() => {
    if (!stepping) return;
    const tick = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(tick);
  }, [stepping]);
  // The round asked for has ended: open it.
  const last = status?.last_step;
  useEffect(() => {
    if (!asked.current || stepping || !last || last.work !== w.id) return;
    asked.current = false;
    const r = last.rounds[last.rounds.length - 1];
    if (r !== undefined) onRound(r);
  }, [stepping, last, w.id, onRound]);
  if (stepping) {
    const since = w.running?.started_at ?? null;
    return (
      <section className="next">
        <div className="next-head">
          <strong>{t('next_running')}</strong>
          {since !== null ? <span className="muted">{duration(now - since)}</span> : null}
        </div>
        <ol className="next-activity">
          {activity.map((a, i) => (
            <li key={i}>{a.what}</li>
          ))}
        </ol>
      </section>
    );
  }
  if (!can) return null;
  const n = next.data;
  const kind =
    n === undefined
      ? ''
      : n.kind === 'agent'
        ? t('next_kind_agent', { line: n.line ?? '' })
        : n.kind === 'engine'
          ? t('next_kind_engine', { step: n.step ?? '' })
          : n.kind === 'tool'
            ? t('next_kind_tool')
            : t('next_kind_owner');
  const run = () => {
    setError(null);
    asked.current = true;
    postJson(`/api/v1/works/${w.id}/step`, {}).catch((e: unknown) => {
      asked.current = false;
      setError(e instanceof Error ? e.message : String(e));
    });
  };
  return (
    <section className="next">
      <div className="next-head">
        <strong>{t('next_title')}</strong>
        {n !== undefined ? (
          <span>
            <span className="cell-id">{n.cell}</span> {n.title} · <span className="muted">{kind}</span>
          </span>
        ) : null}
      </div>
      {next.error !== null ? <p className="bad">{next.error}</p> : null}
      {n?.message ? (
        <details>
          <summary>{t('next_message')}</summary>
          <pre className="j-text next-message">{n.message}</pre>
        </details>
      ) : null}
      <div className="next-run">
        <button type="button" className="next-button" onClick={run} disabled={n === undefined}>
          {t('next_run')}
        </button>
        <span className="muted">{t('next_real')}</span>
      </div>
      {error !== null ? <p className="bad">{t('request_failed', { error })}</p> : null}
    </section>
  );
}

/// The work's rounds and records, in a drawer over the left of the chart.
function Drawer({ d, round, onRound }: { d: WorkDetail; round: number | null; onRound: (r: number) => void }) {
  const [open, setOpen] = useState(true);
  return (
    <div className={`drawer${open ? '' : ' drawer-closed'}`}>
      <button type="button" className="drawer-toggle" onClick={() => setOpen((o) => !o)} aria-expanded={open}>
        {open ? '‹' : '›'} {t('rounds')} ({d.rounds.length})
      </button>
      {open ? (
        <div className="drawer-body">
          <Next d={d} onRound={onRound} />
          <Timeline rounds={d.rounds} selected={round} onRound={(r) => onRound(r.id)} />
          {d.messages.length > 0 ? (
            <Section title={t('work_messages')} open={false}>
              {d.messages.map((m) => (
                <div key={m.id} className="boxed">
                  <div className="muted">
                    {m.cell !== null ? t('message_cell', { cell: m.cell }) : null} ·{' '}
                    <button className="link" onClick={() => onRound(m.round)}>
                      {t('round_title', { id: m.round })}
                    </button>
                  </div>
                  <pre className="j-text">{m.text}</pre>
                </div>
              ))}
            </Section>
          ) : null}
          {d.asks.length > 0 ? (
            <Section title={t('asks')} open={false}>
              {d.asks.map((a) => (
                <div key={a.id} className="boxed">
                  <div>
                    #{a.id} · {a.answer === null ? t('ask_waits') : t('ask_answered')} ·{' '}
                    <button className="link" onClick={() => onRound(a.round)}>
                      {t('round_title', { id: a.round })}
                    </button>
                  </div>
                  <Json value={a.body} />
                  {a.answer !== null ? <Json value={a.answer} /> : null}
                </div>
              ))}
            </Section>
          ) : null}
          {d.design_cards.length > 0 ? (
            <Section title={t('design_cards')} open={false}>
              {d.design_cards.map((c) => (
                <div key={c.id} className="boxed">
                  <div>
                    #{c.id} · {c.status} ·{' '}
                    <button className="link" onClick={() => onRound(c.round)}>
                      {t('round_title', { id: c.round })}
                    </button>
                  </div>
                  <Json value={c.body} />
                  <Json value={c.rulings} />
                </div>
              ))}
            </Section>
          ) : null}
          {d.interjections.length > 0 ? (
            <Section title={t('interjections')} open={false}>
              {d.interjections.map((x) => (
                <div key={x.id} className="boxed">
                  <div className="muted">
                    #{x.id} {x.speaker} → {x.line} ·{' '}
                    {x.delivered_round !== null
                      ? t('delivered', { round: x.delivered_round })
                      : t('waiting_delivery')}
                  </div>
                  <pre className="j-text">{x.text}</pre>
                </div>
              ))}
            </Section>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}

function Body({ id, d, compiled, view, round, cell }: {
  id: number;
  d: WorkDetail;
  compiled: Compiled;
  view: FlowView;
  round: number | null;
  cell: string | null;
}) {
  const g = compiled.graphs.find((x) => x.name === d.work.graph);
  const v = view.graphs.find((x) => x.name === d.work.graph);
  if (!g || !v) return <p className="muted page">{t('none')}</p>;
  const roundCell = round !== null ? (d.rounds.find((r) => r.id === round)?.cell ?? null) : null;
  const go = (r: number | null, c: string | null) => navigate(href({ page: 'work', id, round: r, cell: c }), true);
  const model = chart(g, v);
  return (
    <>
      <Drawer d={d} round={round} onRound={(r) => go(r, null)} />
      <FlowScreen
        compiled={compiled}
        g={g}
        view={v}
        legend="work"
        marks={{
          overlays: overlays(d.rounds, d.work.at),
          taken: taken(model, g, d.rounds),
          added: new Set(),
          changed: new Set(),
        }}
        cell={cell}
        onCell={(c) => go(null, c)}
        start={roundCell ?? d.work.at}
        rounds={d.rounds}
        round={round}
        onRound={(r) => go(r, null)}
        renderRound={(r) => <RoundView id={r} rounds={d.rounds} />}
      />
    </>
  );
}

export function Work({ id, round, cell }: { id: number; round: number | null; cell: string | null }) {
  const detail = useResource<WorkDetail>(`work:${id}`, `/api/v1/works/${id}`);
  const version = detail.data?.work.flow_version ?? null;
  const compiled = useResource<Compiled>(null, version !== null ? `/api/v1/flows/${version}` : null);
  const view = useResource<FlowView>(null, version !== null ? `/api/v1/flows/${version}/view` : null);
  return (
    <div className="screen">
      <Loaded r={detail}>
        {(d) => (
          <>
            <Bar d={d} />
            <Loaded r={compiled}>
              {(c) => (
                <Loaded r={view}>
                  {(v) => <Body id={id} d={d} compiled={c} view={v} round={round} cell={cell} />}
                </Loaded>
              )}
            </Loaded>
          </>
        )}
      </Loaded>
    </div>
  );
}
